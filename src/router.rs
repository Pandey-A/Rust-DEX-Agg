use crate::quote::QuoteEngine;
use crate::types::{AggregatorError, MarketContext, OptimizationStrategy, PoolInfo, RouteQuote, Result};
use ethers::types::{Address, U256};
use std::collections::{HashMap, HashSet, VecDeque};
use tracing::{debug, info};

/// Router for finding optimal swap routes
pub struct Router {
    optimization: OptimizationStrategy,
    max_hops: usize,
}

impl Router {
    /// Create a new router
    pub fn new(optimization: OptimizationStrategy, max_hops: usize) -> Self {
        Self {
            optimization,
            max_hops: max_hops.min(4), // Cap at 4 hops for performance
        }
    }

    /// Find the best route between two tokens
    pub fn find_best_route(
        &self,
        pools: &[PoolInfo],
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        context: &MarketContext,
    ) -> Result<RouteQuote> {
        let routes = self.find_top_routes(pools, token_in, token_out, amount_in, context, 1)?;
        Ok(routes.into_iter().next().unwrap())
    }

    /// Find top N routes between two tokens, sorted by score
    pub fn find_top_routes(
        &self,
        pools: &[PoolInfo],
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        context: &MarketContext,
        limit: usize,
    ) -> Result<Vec<RouteQuote>> {
        info!(
            "Finding best route from {:?} to {:?} with {} strategy",
            token_in, token_out, self.optimization
        );

        // Find all possible routes
        let routes = self.find_all_routes(pools, token_in, token_out)?;

        if routes.is_empty() {
            return Err(AggregatorError::NoRouteFound {
                from: format!("{:?}", token_in),
                to: format!("{:?}", token_out),
            });
        }

        info!("Found {} possible routes", routes.len());

        // Calculate quotes for all routes
        let mut route_quotes = Vec::new();

        for route in routes {
            match self.calculate_route_quote(&route, pools, amount_in, context) {
                Ok(quote) => route_quotes.push(quote),
                Err(e) => {
                    debug!("Failed to calculate route quote: {}", e);
                }
            }
        }

        if route_quotes.is_empty() {
            return Err(AggregatorError::NoRouteFound {
                from: format!("{:?}", token_in),
                to: format!("{:?}", token_out),
            });
        }

        // Sort by score (best first)
        route_quotes.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        if !route_quotes.is_empty() {
            info!(
                "Best route: {} with score {:.2}",
                route_quotes[0].description, route_quotes[0].score
            );
        }

        // Return top N routes
        Ok(route_quotes.into_iter().take(limit).collect())
    }

    /// Find all possible routes up to max_hops
    fn find_all_routes(
        &self,
        pools: &[PoolInfo],
        token_in: Address,
        token_out: Address,
    ) -> Result<Vec<Route>> {
        // Build adjacency map: token -> (pool, other_token)
        let adjacency = self.build_adjacency_map(pools);

        let mut all_routes = Vec::new();

        // Try direct routes (1 hop)
        if let Some(connections) = adjacency.get(&token_in) {
            for (pool_addr, next_token) in connections {
                if *next_token == token_out {
                    all_routes.push(Route {
                        tokens: vec![token_in, token_out],
                        pools: vec![*pool_addr],
                    });
                }
            }
        }

        // Try multi-hop routes (2+ hops) using BFS
        if self.max_hops > 1 {
            let multi_hop_routes = self.bfs_routes(&adjacency, token_in, token_out, self.max_hops);
            all_routes.extend(multi_hop_routes);
        }

        // Remove duplicate routes
        all_routes = self.deduplicate_routes(all_routes);

        Ok(all_routes)
    }

    /// Build adjacency map for token graph
    fn build_adjacency_map(&self, pools: &[PoolInfo]) -> HashMap<Address, Vec<(Address, Address)>> {
        let mut adjacency: HashMap<Address, Vec<(Address, Address)>> = HashMap::new();

        for pool in pools {
            // Skip pools with zero reserves
            if pool.reserve0.is_zero() || pool.reserve1.is_zero() {
                continue;
            }

            adjacency
                .entry(pool.token0)
                .or_insert_with(Vec::new)
                .push((pool.address, pool.token1));

            adjacency
                .entry(pool.token1)
                .or_insert_with(Vec::new)
                .push((pool.address, pool.token0));
        }

        adjacency
    }

    /// Use BFS to find multi-hop routes
    fn bfs_routes(
        &self,
        adjacency: &HashMap<Address, Vec<(Address, Address)>>,
        start: Address,
        end: Address,
        max_depth: usize,
    ) -> Vec<Route> {
        let mut routes = Vec::new();
        let mut queue: VecDeque<(Address, Vec<Address>, Vec<Address>)> = VecDeque::new();

        // Queue: (current_token, path_tokens, path_pools)
        queue.push_back((start, vec![start], vec![]));

        while let Some((current, path_tokens, path_pools)) = queue.pop_front() {
            // Check depth limit
            if path_pools.len() >= max_depth {
                continue;
            }

            // Get connections from current token
            if let Some(connections) = adjacency.get(&current) {
                for (pool_addr, next_token) in connections {
                    // Avoid revisiting tokens (prevent cycles)
                    if path_tokens.contains(next_token) {
                        continue;
                    }

                    let mut new_path_tokens = path_tokens.clone();
                    new_path_tokens.push(*next_token);

                    let mut new_path_pools = path_pools.clone();
                    new_path_pools.push(*pool_addr);

                    // Found complete route
                    if *next_token == end {
                        routes.push(Route {
                            tokens: new_path_tokens.clone(),
                            pools: new_path_pools.clone(),
                        });
                    } else {
                        // Continue searching
                        queue.push_back((*next_token, new_path_tokens, new_path_pools));
                    }
                }
            }
        }

        routes
    }

    /// Remove duplicate routes
    fn deduplicate_routes(&self, routes: Vec<Route>) -> Vec<Route> {
        let mut seen = HashSet::new();
        let mut unique_routes = Vec::new();

        for route in routes {
            let key = format!("{:?}", route.pools);
            if seen.insert(key) {
                unique_routes.push(route);
            }
        }

        unique_routes
    }

    /// Calculate quote for a specific route
    fn calculate_route_quote(
        &self,
        route: &Route,
        pools: &[PoolInfo],
        amount_in: U256,
        context: &MarketContext,
    ) -> Result<RouteQuote> {
        // Get pool objects
        let route_pools: Vec<PoolInfo> = route
            .pools
            .iter()
            .filter_map(|addr| pools.iter().find(|p| p.address == *addr).cloned())
            .collect();

        if route_pools.len() != route.pools.len() {
            return Err(AggregatorError::PoolNotFound("Pool not found in cache".to_string()));
        }

        // Calculate hops
        let hops = QuoteEngine::calculate_route_output(&route_pools, &route.tokens, amount_in)?;

        // Calculate totals
        let amount_out = hops.last().map(|h| h.amount_out).unwrap_or(U256::zero());
        let total_fee = hops.iter().map(|h| h.fee).fold(U256::zero(), |acc, f| acc + f);
        let gas_estimate = hops
            .iter()
            .map(|h| h.gas_estimate)
            .fold(U256::zero(), |acc, g| acc + g);

        // Calculate price impact (approximate for multi-hop)
        let price_impact_bps = self.estimate_route_price_impact(&hops);

        // Calculate optimization score
        let score = self.calculate_score(amount_out, gas_estimate, price_impact_bps, context);

        // Generate description
        let description = self.generate_route_description(&route.tokens);

        Ok(RouteQuote {
            token_in: route.tokens[0],
            token_out: *route.tokens.last().unwrap(),
            amount_in,
            amount_out,
            hops,
            total_fee,
            gas_estimate,
            price_impact_bps,
            score,
            description,
        })
    }

    /// Estimate total price impact for a route
    fn estimate_route_price_impact(&self, hops: &[crate::types::RouteHop]) -> u32 {
        // For multi-hop, approximate cumulative impact
        // This is a simplification; real impact calculation is more complex
        hops.len() as u32 * 10 // ~0.1% per hop base impact
    }

    /// Calculate optimization score for a route
    fn calculate_score(
        &self,
        amount_out: U256,
        gas_estimate: U256,
        price_impact_bps: u32,
        context: &MarketContext,
    ) -> f64 {
        let (price_weight, gas_weight, slippage_weight) = self.optimization.get_weights();

        // Normalize output amount (higher is better)
        let output_score = amount_out.as_u128() as f64;

        // Calculate gas cost in USD (lower is better, so negate)
        let gas_cost_usd = crate::utils::estimate_gas_cost_usd(
            gas_estimate,
            context.gas_price_gwei,
            context.eth_price_usd,
        );
        let gas_score = -gas_cost_usd * 1000.0; // Scale up for visibility

        // Slippage penalty (lower is better, so negate)
        let slippage_score = -(price_impact_bps as f64);

        // Composite score
        let score = (output_score * price_weight)
            + (gas_score * gas_weight)
            + (slippage_score * slippage_weight);

        score
    }

    /// Generate human-readable route description
    fn generate_route_description(&self, tokens: &[Address]) -> String {
        tokens
            .iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>()
            .join(" → ")
    }
}

/// A route through pools
#[derive(Debug, Clone)]
struct Route {
    tokens: Vec<Address>,
    pools: Vec<Address>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pools() -> Vec<PoolInfo> {
        vec![
            PoolInfo {
                address: Address::from_low_u64_be(100),
                token0: Address::from_low_u64_be(1),
                token1: Address::from_low_u64_be(2),
                reserve0: U256::from(100_000_000_000_000_000_000u128),
                reserve1: U256::from(200_000_000_000_000_000_000u128),
                fee_bps: 30,
                dex_name: "TestDEX".to_string(),
                last_updated: 0,
            },
            PoolInfo {
                address: Address::from_low_u64_be(101),
                token0: Address::from_low_u64_be(2),
                token1: Address::from_low_u64_be(3),
                reserve0: U256::from(200_000_000_000_000_000_000u128),
                reserve1: U256::from(300_000_000_000_000_000_000u128),
                fee_bps: 30,
                dex_name: "TestDEX".to_string(),
                last_updated: 0,
            },
        ]
    }

    #[test]
    fn test_build_adjacency_map() {
        let pools = create_test_pools();
        let router = Router::new(OptimizationStrategy::Price, 3);
        let adjacency = router.build_adjacency_map(&pools);

        assert!(adjacency.contains_key(&Address::from_low_u64_be(1)));
        assert!(adjacency.contains_key(&Address::from_low_u64_be(2)));
        assert!(adjacency.contains_key(&Address::from_low_u64_be(3)));
    }

    #[test]
    fn test_find_all_routes() {
        let pools = create_test_pools();
        let router = Router::new(OptimizationStrategy::Price, 3);

        let routes = router
            .find_all_routes(&pools, Address::from_low_u64_be(1), Address::from_low_u64_be(3))
            .unwrap();

        assert!(!routes.is_empty());
    }
}
