use crate::types::{AggregatorError, PoolInfo, Result, RouteHop};
use crate::utils;
use ethers::types::{Address, U256};
use tracing::debug;

/// Quote engine for calculating swap outputs
pub struct QuoteEngine;

impl QuoteEngine {
    /// Calculate output amount for a single pool swap
    pub fn calculate_pool_output(
        pool: &PoolInfo,
        token_in: Address,
        amount_in: U256,
    ) -> Result<QuoteResult> {
        let (reserve_in, reserve_out) = pool
            .get_reserves(&token_in)
            .ok_or_else(|| {
                AggregatorError::InvalidTokenAddress(format!(
                    "Token {:?} not in pool {:?}",
                    token_in, pool.address
                ))
            })?;

        let token_out = pool
            .get_other_token(&token_in)
            .ok_or_else(|| AggregatorError::InvalidTokenAddress("Invalid token pair".to_string()))?;

        // Calculate output using UniswapV2 formula
        let amount_out = utils::calculate_uniswap_v2_output(
            amount_in,
            reserve_in,
            reserve_out,
            pool.fee_bps,
        )?;

        // Calculate fee
        let fee = utils::calculate_fee(amount_in, pool.fee_bps);

        // Calculate price impact
        let price_impact_bps = utils::calculate_price_impact(
            amount_in,
            reserve_in,
            amount_out,
            reserve_out,
        );

        // Estimate gas (approximate for UniswapV2 swap)
        let gas_estimate = U256::from(100_000); // ~100k gas for single swap

        debug!(
            "Pool {:?}: {} in -> {} out (price impact: {} bps)",
            pool.address, amount_in, amount_out, price_impact_bps
        );

        Ok(QuoteResult {
            pool: pool.clone(),
            token_in,
            token_out,
            amount_in,
            amount_out,
            fee,
            price_impact_bps,
            gas_estimate,
        })
    }

    /// Calculate output for a multi-hop route
    pub fn calculate_route_output(
        pools: &[PoolInfo],
        tokens: &[Address],
        amount_in: U256,
    ) -> Result<Vec<RouteHop>> {
        if pools.is_empty() || tokens.len() != pools.len() + 1 {
            return Err(AggregatorError::InvalidAmount(
                "Invalid route: pools and tokens mismatch".to_string(),
            ));
        }

        let mut hops = Vec::new();
        let mut current_amount = amount_in;

        for (i, pool) in pools.iter().enumerate() {
            let token_in = tokens[i];
            let token_out = tokens[i + 1];

            let quote = Self::calculate_pool_output(pool, token_in, current_amount)?;

            let hop = RouteHop {
                pool: pool.address,
                token_in,
                token_out,
                dex_name: pool.dex_name.clone(),
                amount_in: current_amount,
                amount_out: quote.amount_out,
                fee: quote.fee,
                gas_estimate: quote.gas_estimate,
            };

            hops.push(hop);
            current_amount = quote.amount_out;
        }

        Ok(hops)
    }

    /// Get best direct pool for a token pair
    pub fn find_best_direct_pool(
        pools: &[PoolInfo],
        token_in: Address,
        token_out: Address,
        amount_in: U256,
    ) -> Result<QuoteResult> {
        let matching_pools: Vec<&PoolInfo> = pools
            .iter()
            .filter(|p| {
                (p.token0 == token_in && p.token1 == token_out)
                    || (p.token0 == token_out && p.token1 == token_in)
            })
            .collect();

        if matching_pools.is_empty() {
            return Err(AggregatorError::NoRouteFound {
                from: format!("{:?}", token_in),
                to: format!("{:?}", token_out),
            });
        }

        // Calculate quotes for all matching pools
        let mut best_quote: Option<QuoteResult> = None;

        for pool in matching_pools {
            match Self::calculate_pool_output(pool, token_in, amount_in) {
                Ok(quote) => {
                    if let Some(ref current_best) = best_quote {
                        if quote.amount_out > current_best.amount_out {
                            best_quote = Some(quote);
                        }
                    } else {
                        best_quote = Some(quote);
                    }
                }
                Err(e) => {
                    debug!("Failed to calculate quote for pool {:?}: {}", pool.address, e);
                }
            }
        }

        best_quote.ok_or_else(|| AggregatorError::NoRouteFound {
            from: format!("{:?}", token_in),
            to: format!("{:?}", token_out),
        })
    }
}

/// Result of a quote calculation
#[derive(Debug, Clone)]
pub struct QuoteResult {
    pub pool: PoolInfo,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub amount_out: U256,
    pub fee: U256,
    pub price_impact_bps: u32,
    pub gas_estimate: U256,
}

impl QuoteResult {
    /// Get the exchange rate
    pub fn exchange_rate(&self) -> f64 {
        if self.amount_in.is_zero() {
            return 0.0;
        }
        let amount_in = self.amount_in.as_u128() as f64;
        let amount_out = self.amount_out.as_u128() as f64;
        amount_out / amount_in
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pool() -> PoolInfo {
        PoolInfo {
            address: Address::zero(),
            token0: Address::from_low_u64_be(1),
            token1: Address::from_low_u64_be(2),
            reserve0: U256::from(100_000_000_000_000_000_000u128), // 100 tokens
            reserve1: U256::from(200_000_000_000_000_000_000u128), // 200 tokens
            fee_bps: 30,
            dex_name: "TestDEX".to_string(),
            last_updated: 0,
        }
    }

    #[test]
    fn test_calculate_pool_output() {
        let pool = create_test_pool();
        let token_in = pool.token0;
        let amount_in = U256::from(1_000_000_000_000_000_000u128); // 1 token

        let result = QuoteEngine::calculate_pool_output(&pool, token_in, amount_in);
        assert!(result.is_ok());

        let quote = result.unwrap();
        assert!(quote.amount_out > U256::zero());
        assert_eq!(quote.token_in, token_in);
        assert_eq!(quote.token_out, pool.token1);
    }

    #[test]
    fn test_calculate_pool_output_invalid_token() {
        let pool = create_test_pool();
        let invalid_token = Address::from_low_u64_be(999);
        let amount_in = U256::from(1_000_000_000_000_000_000u128);

        let result = QuoteEngine::calculate_pool_output(&pool, invalid_token, amount_in);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_best_direct_pool() {
        let pool1 = PoolInfo {
            reserve0: U256::from(100_000_000_000_000_000_000u128),
            reserve1: U256::from(200_000_000_000_000_000_000u128),
            ..create_test_pool()
        };

        let pool2 = PoolInfo {
            reserve0: U256::from(150_000_000_000_000_000_000u128),
            reserve1: U256::from(250_000_000_000_000_000_000u128),
            ..create_test_pool()
        };

        let pools = vec![pool1, pool2];
        let token_in = Address::from_low_u64_be(1);
        let token_out = Address::from_low_u64_be(2);
        let amount_in = U256::from(1_000_000_000_000_000_000u128);

        let result = QuoteEngine::find_best_direct_pool(&pools, token_in, token_out, amount_in);
        assert!(result.is_ok());
    }
}
