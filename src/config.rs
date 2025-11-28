use crate::types::{AggregatorError, Result};
use ethers::types::Address;
use std::env;
use std::str::FromStr;


#[derive(Debug, Clone)]
pub struct Config {

    pub rpc_url: String,
  
    pub chain_id: u64,

    pub uniswap_v2_factory: Address,
    
    pub sushiswap_factory: Address,
    
    pub cache_enabled: bool,
    
    pub cache_ttl: u64,
    
    pub cache_path: String,
    
    pub default_slippage_bps: u32,
    
    pub max_hops: usize,
    
    pub gas_price_gwei: u64,
}

impl Config {
   
    pub fn from_env() -> Result<Self> {
    
        let _ = dotenvy::dotenv();

        let rpc_url = env::var("RPC_URL")
            .map_err(|_| AggregatorError::ConfigError(
                "RPC_URL not set. Please set it in .env file".to_string()
            ))?;

        let chain_id = env::var("CHAIN_ID")
            .unwrap_or_else(|_| "1".to_string())
            .parse()
            .map_err(|_| AggregatorError::ConfigError("Invalid CHAIN_ID".to_string()))?;

        let uniswap_v2_factory = Self::parse_address(
            &env::var("UNISWAP_V2_FACTORY")
                .unwrap_or_else(|_| "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".to_string()),
        )?;

        let sushiswap_factory = Self::parse_address(
            &env::var("SUSHISWAP_FACTORY")
                .unwrap_or_else(|_| "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac".to_string()),
        )?;

        let cache_enabled = env::var("CACHE_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true);

        let cache_ttl = env::var("CACHE_TTL_SECONDS")
            .unwrap_or_else(|_| "300".to_string())
            .parse()
            .unwrap_or(300);

        let cache_path = env::var("CACHE_PATH")
            .unwrap_or_else(|_| "./cache/pools.json".to_string());

        let default_slippage_bps = env::var("DEFAULT_SLIPPAGE_BPS")
            .unwrap_or_else(|_| "50".to_string())
            .parse()
            .unwrap_or(50);

        let max_hops = env::var("MAX_HOPS")
            .unwrap_or_else(|_| "3".to_string())
            .parse()
            .unwrap_or(3);

        let gas_price_gwei = env::var("GAS_PRICE_GWEI")
            .unwrap_or_else(|_| "30".to_string())
            .parse()
            .unwrap_or(30);

        Ok(Self {
            rpc_url,
            chain_id,
            uniswap_v2_factory,
            sushiswap_factory,
            cache_enabled,
            cache_ttl,
            cache_path,
            default_slippage_bps,
            max_hops,
            gas_price_gwei,
        })
    }

    /// Parse an Ethereum address from string
    fn parse_address(addr_str: &str) -> Result<Address> {
        Address::from_str(addr_str)
            .map_err(|_| AggregatorError::InvalidTokenAddress(addr_str.to_string()))
    }

    /// Get factory addresses for all supported DEXs
    pub fn get_all_factories(&self) -> Vec<(String, Address)> {
        vec![
            ("Uniswap".to_string(), self.uniswap_v2_factory),
            ("SushiSwap".to_string(), self.sushiswap_factory),
        ]
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rpc_url: "https://eth.llamarpc.com".to_string(),
            chain_id: 1,
            uniswap_v2_factory: Address::from_str("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")
                .unwrap(),
            sushiswap_factory: Address::from_str("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac")
                .unwrap(),
            cache_enabled: true,
            cache_ttl: 300,
            cache_path: "./cache/pools.json".to_string(),
            default_slippage_bps: 50,
            max_hops: 3,
            gas_price_gwei: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.chain_id, 1);
        assert_eq!(config.max_hops, 3);
        assert!(config.cache_enabled);
    }

    #[test]
    fn test_parse_address() {
        let addr = Config::parse_address("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f");
        assert!(addr.is_ok());

        let invalid = Config::parse_address("invalid");
        assert!(invalid.is_err());
    }
}
