use crate::config::Config;
use crate::types::{AggregatorError, PoolInfo, Result};
use dashmap::DashMap;
use ethers::prelude::*;
use ethers::types::{Address, U256};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};

// UniswapV2 Factory ABI (simplified)
abigen!(
    UniswapV2Factory,
    r#"[
        function allPairsLength() external view returns (uint256)
        function allPairs(uint256) external view returns (address)
    ]"#,
);

// UniswapV2 Pair ABI (simplified)
abigen!(
    UniswapV2Pair,
    r#"[
        function token0() external view returns (address)
        function token1() external view returns (address)
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
    ]"#,
);

/// Pool manager for fetching and caching pool data
pub struct PoolManager {
    provider: Arc<Provider<Http>>,
    pools: Arc<DashMap<Address, PoolInfo>>,
}

impl PoolManager {
    /// Create a new pool manager
    pub fn new(provider: Arc<Provider<Http>>, _config: Config) -> Self {
        Self {
            provider,
            pools: Arc::new(DashMap::new()),
        }
    }

    /// Fetch pools from a factory contract
    pub async fn fetch_pools(
        &self,
        factory_address: Address,
        dex_name: String,
        limit: Option<usize>,
    ) -> Result<Vec<PoolInfo>> {
        info!("Fetching pools from {} factory: {:?}", dex_name, factory_address);

        let factory = UniswapV2Factory::new(factory_address, self.provider.clone());

        // Get total number of pairs
        let pair_count = factory
            .all_pairs_length()
            .call()
            .await
            .map_err(|e| AggregatorError::ContractError(format!("Failed to get pair count: {}", e)))?;

        info!("Total pairs in factory: {}", pair_count);

        let fetch_limit = limit.unwrap_or(pair_count.as_usize()).min(pair_count.as_usize());
        info!("Fetching {} pools", fetch_limit);

        let mut pools = Vec::new();

        // Fetch pools in batches
        for i in 0..fetch_limit {
            match self.fetch_pool_at_index(&factory, i, &dex_name).await {
                Ok(pool) => {
                    self.pools.insert(pool.address, pool.clone());
                    pools.push(pool);
                    
                    if (i + 1) % 10 == 0 {
                        info!("Fetched {}/{} pools", i + 1, fetch_limit);
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch pool at index {}: {}", i, e);
                }
            }
        }

        info!("Successfully fetched {} pools from {}", pools.len(), dex_name);
        Ok(pools)
    }

    /// Fetch a single pool at a specific index
    async fn fetch_pool_at_index(
        &self,
        factory: &UniswapV2Factory<Provider<Http>>,
        index: usize,
        dex_name: &str,
    ) -> Result<PoolInfo> {
        // Get pair address
        let pair_address = factory
            .all_pairs(U256::from(index))
            .call()
            .await
            .map_err(|e| AggregatorError::ContractError(format!("Failed to get pair address: {}", e)))?;

        // Fetch pool info
        self.fetch_pool_info(pair_address, dex_name.to_string()).await
    }

    /// Fetch information for a specific pool
    pub async fn fetch_pool_info(&self, pair_address: Address, dex_name: String) -> Result<PoolInfo> {
        let pair = UniswapV2Pair::new(pair_address, self.provider.clone());

        // Get tokens
        let token0 = pair
            .token_0()
            .call()
            .await
            .map_err(|e| AggregatorError::ContractError(format!("Failed to get token0: {}", e)))?;

        let token1 = pair
            .token_1()
            .call()
            .await
            .map_err(|e| AggregatorError::ContractError(format!("Failed to get token1: {}", e)))?;

        // Get reserves
        let reserves = pair
            .get_reserves()
            .call()
            .await
            .map_err(|e| AggregatorError::ContractError(format!("Failed to get reserves: {}", e)))?;

        let block_number = self
            .provider
            .get_block_number()
            .await
            .map_err(|e| AggregatorError::RpcError(format!("Failed to get block number: {}", e)))?;

        let pool = PoolInfo {
            address: pair_address,
            token0,
            token1,
            reserve0: U256::from(reserves.0),
            reserve1: U256::from(reserves.1),
            fee_bps: 30, // UniswapV2 default fee is 0.3%
            dex_name,
            last_updated: block_number.as_u64(),
        };

        debug!("Fetched pool: {:?}", pool.address);
        Ok(pool)
    }

    /// Get all cached pools
    pub fn get_all_pools(&self) -> Vec<PoolInfo> {
        self.pools.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Get pool by address
    pub fn get_pool(&self, address: &Address) -> Option<PoolInfo> {
        self.pools.get(address).map(|entry| entry.value().clone())
    }

    /// Get pools containing a specific token
    pub fn get_pools_with_token(&self, token: &Address) -> Vec<PoolInfo> {
        self.pools
            .iter()
            .filter(|entry| {
                let pool = entry.value();
                pool.token0 == *token || pool.token1 == *token
            })
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get pools for a token pair
    pub fn get_pools_for_pair(&self, token_a: &Address, token_b: &Address) -> Vec<PoolInfo> {
        self.pools
            .iter()
            .filter(|entry| {
                let pool = entry.value();
                (pool.token0 == *token_a && pool.token1 == *token_b)
                    || (pool.token0 == *token_b && pool.token1 == *token_a)
            })
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Export pools to JSON file
    pub fn export_to_file(&self, path: &str) -> Result<()> {
        let pools = self.get_all_pools();
        let cache_data = CacheData {
            pools,
            timestamp: chrono::Utc::now().timestamp() as u64,
        };

        // Create directory if it doesn't exist
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AggregatorError::CacheError(format!("Failed to create cache directory: {}", e)))?;
        }

        let json = serde_json::to_string_pretty(&cache_data)
            .map_err(|e| AggregatorError::CacheError(format!("Failed to serialize cache: {}", e)))?;

        fs::write(path, json)
            .map_err(|e| AggregatorError::CacheError(format!("Failed to write cache file: {}", e)))?;

        info!("Exported {} pools to {}", cache_data.pools.len(), path);
        Ok(())
    }

    /// Import pools from JSON file
    pub fn import_from_file(&self, path: &str) -> Result<usize> {
        let json = fs::read_to_string(path)
            .map_err(|e| AggregatorError::CacheError(format!("Failed to read cache file: {}", e)))?;

        let cache_data: CacheData = serde_json::from_str(&json)
            .map_err(|e| AggregatorError::CacheError(format!("Failed to parse cache: {}", e)))?;

        let count = cache_data.pools.len();
        for pool in cache_data.pools {
            self.pools.insert(pool.address, pool);
        }

        // Format timestamp to human-readable date
        let datetime = chrono::DateTime::from_timestamp(cache_data.timestamp as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| format!("timestamp: {}", cache_data.timestamp));

        info!("Imported {} pools from {} (cached at {})", 
            count, path, datetime);
        Ok(count)
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        let pools = self.get_all_pools();
        let total_pools = pools.len();
        
        let mut dex_counts: HashMap<String, usize> = HashMap::new();
        let total_liquidity_usd = 0.0; // Placeholder for now
        
        for pool in &pools {
            *dex_counts.entry(pool.dex_name.clone()).or_insert(0) += 1;
        }

        CacheStats {
            total_pools,
            dex_counts,
            total_liquidity_usd,
        }
    }

    /// Clear all cached pools
    pub fn clear(&self) {
        self.pools.clear();
        info!("Cleared all cached pools");
    }
}

/// Cache data structure for serialization
#[derive(Debug, Serialize, Deserialize)]
struct CacheData {
    pools: Vec<PoolInfo>,
    timestamp: u64,
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    pub total_pools: usize,
    pub dex_counts: HashMap<String, usize>,
    pub total_liquidity_usd: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_stats() {
        let config = Config::default();
        let provider = Arc::new(Provider::<Http>::try_from(config.rpc_url.clone()).unwrap());
        let manager = PoolManager::new(provider, config);

        let stats = manager.get_cache_stats();
        assert_eq!(stats.total_pools, 0);
    }
}
