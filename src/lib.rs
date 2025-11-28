pub mod config;
pub mod pools;
pub mod quote;
pub mod router;
pub mod types;
pub mod utils;

pub use config::Config;
pub use pools::{PoolManager, CacheStats};
pub use quote::{QuoteEngine, QuoteResult};
pub use router::Router;
pub use types::{
    AggregatorError, MarketContext, OptimizationStrategy, PoolInfo, RouteQuote, RouteHop,
    Result, TokenInfo,
};

use ethers::providers::{Http, Provider};
use ethers::types::{Address, U256};
use std::sync::Arc;

/// Main aggregator interface
pub struct Aggregator {
    pool_manager: Arc<PoolManager>,
    config: Config,
}

impl Aggregator {
    /// Create a new aggregator instance
    pub async fn new(config: Config) -> Result<Self> {
        let provider = Provider::<Http>::try_from(config.rpc_url.clone())
            .map_err(|e| AggregatorError::RpcError(format!("Failed to create provider: {}", e)))?;

        let pool_manager = Arc::new(PoolManager::new(Arc::new(provider), config.clone()));

        // Auto-load cache if it exists
        let cache_path = &config.cache_path;
        if std::path::Path::new(cache_path).exists() {
            let _ = pool_manager.import_from_file(cache_path);
            // Silently ignore errors - cache is optional
        }

        Ok(Self {
            pool_manager,
            config,
        })
    }

    /// Fetch pools from all configured DEX factories
    pub async fn fetch_all_pools(&self, limit_per_dex: Option<usize>) -> Result<usize> {
        let mut total_fetched = 0;

        for (dex_name, factory_addr) in self.config.get_all_factories() {
            let pools = self
                .pool_manager
                .fetch_pools(factory_addr, dex_name, limit_per_dex)
                .await?;
            total_fetched += pools.len();
        }

        Ok(total_fetched)
    }

    /// Fetch pools from a specific factory
    pub async fn fetch_pools(
        &self,
        factory_address: Address,
        dex_name: String,
        limit: Option<usize>,
    ) -> Result<Vec<PoolInfo>> {
        self.pool_manager
            .fetch_pools(factory_address, dex_name, limit)
            .await
    }

    /// Get the best quote for a swap
    pub fn get_best_quote(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        optimization: OptimizationStrategy,
    ) -> Result<RouteQuote> {
        let quotes = self.get_top_quotes(token_in, token_out, amount_in, optimization, 1)?;
        Ok(quotes.into_iter().next().unwrap())
    }

    /// Get top N quotes for a swap, sorted by score
    pub fn get_top_quotes(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        optimization: OptimizationStrategy,
        limit: usize,
    ) -> Result<Vec<RouteQuote>> {
        let pools = self.pool_manager.get_all_pools();

        if pools.is_empty() {
            return Err(AggregatorError::PoolNotFound(
                "No pools cached. Run fetch-pools first.".to_string(),
            ));
        }

        let router = Router::new(optimization, self.config.max_hops);
        let context = MarketContext {
            gas_price_gwei: self.config.gas_price_gwei,
            eth_price_usd: 1800.0,
            block_number: 0,
        };

        router.find_top_routes(&pools, token_in, token_out, amount_in, &context, limit)
    }

    /// Get all cached pools
    pub fn get_pools(&self) -> Vec<PoolInfo> {
        self.pool_manager.get_all_pools()
    }

    /// Get pools containing a specific token
    pub fn get_pools_with_token(&self, token: Address) -> Vec<PoolInfo> {
        self.pool_manager.get_pools_with_token(&token)
    }

    /// Get configuration
    pub fn get_config(&self) -> &Config {
        &self.config
    }

    /// Export cache to file
    pub fn export_cache(&self, path: &str) -> Result<()> {
        self.pool_manager.export_to_file(path)
    }

    /// Import cache from file
    pub fn import_cache(&self, path: &str) -> Result<usize> {
        self.pool_manager.import_from_file(path)
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        self.pool_manager.get_cache_stats()
    }

    /// Clear all cached pools
    pub fn clear_cache(&self) {
        self.pool_manager.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_aggregator() {
        let config = Config::default();
        let result = Aggregator::new(config).await;
        assert!(result.is_ok());
    }
}
