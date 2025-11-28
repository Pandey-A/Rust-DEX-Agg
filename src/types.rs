use ethers::types::{Address, U256};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Custom error types for the aggregator
#[derive(Error, Debug)]
pub enum AggregatorError {
    #[error("RPC error: {0}")]
    RpcError(String),

    #[error("Pool not found: {0}")]
    PoolNotFound(String),

    #[error("Insufficient liquidity in pool {0}")]
    InsufficientLiquidity(String),

    #[error("No route found between {from} and {to}")]
    NoRouteFound { from: String, to: String },

    #[error("Invalid token address: {0}")]
    InvalidTokenAddress(String),

    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Contract call failed: {0}")]
    ContractError(String),

    #[error("Math overflow or underflow")]
    MathError,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result type alias for aggregator operations
pub type Result<T> = std::result::Result<T, AggregatorError>;

/// Information about a liquidity pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolInfo {
    /// Pool contract address
    pub address: Address,
    
    /// First token address
    pub token0: Address,
    
    /// Second token address
    pub token1: Address,
    
    /// Reserve of token0
    pub reserve0: U256,
    
    /// Reserve of token1
    pub reserve1: U256,
    
    /// Fee in basis points (e.g., 30 = 0.3%)
    pub fee_bps: u32,
    
    /// DEX name (e.g., "Uniswap", "SushiSwap")
    pub dex_name: String,
    
    /// Block number when last updated
    pub last_updated: u64,
}

impl PoolInfo {
    /// Get the other token in the pair
    pub fn get_other_token(&self, token: &Address) -> Option<Address> {
        if token == &self.token0 {
            Some(self.token1)
        } else if token == &self.token1 {
            Some(self.token0)
        } else {
            None
        }
    }

    /// Get reserves for a specific input/output token pair
    pub fn get_reserves(&self, token_in: &Address) -> Option<(U256, U256)> {
        if token_in == &self.token0 {
            Some((self.reserve0, self.reserve1))
        } else if token_in == &self.token1 {
            Some((self.reserve1, self.reserve0))
        } else {
            None
        }
    }

    /// Calculate current price ratio (token1 per token0)
    pub fn price_ratio(&self) -> f64 {
        if self.reserve0.is_zero() {
            return 0.0;
        }
        let r0 = self.reserve0.as_u128() as f64;
        let r1 = self.reserve1.as_u128() as f64;
        r1 / r0
    }
}

/// A single hop in a route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHop {
    /// Pool used for this hop
    pub pool: Address,
    
    /// Token in for this hop
    pub token_in: Address,
    
    /// Token out for this hop
    pub token_out: Address,
    
    /// DEX name
    pub dex_name: String,
    
    /// Amount in for this hop
    pub amount_in: U256,
    
    /// Amount out for this hop
    pub amount_out: U256,
    
    /// Fee paid in this hop (in token_in)
    pub fee: U256,
    
    /// Gas estimate for this hop
    pub gas_estimate: U256,
}

/// Complete route information with quote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteQuote {
    /// Input token
    pub token_in: Address,
    
    /// Output token
    pub token_out: Address,
    
    /// Input amount
    pub amount_in: U256,
    
    /// Expected output amount
    pub amount_out: U256,
    
    /// Route hops
    pub hops: Vec<RouteHop>,
    
    /// Total fee across all hops
    pub total_fee: U256,
    
    /// Total gas estimate
    pub gas_estimate: U256,
    
    /// Price impact in basis points
    pub price_impact_bps: u32,
    
    /// Optimization score
    pub score: f64,
    
    /// Route description
    pub description: String,
}

impl RouteQuote {
    /// Get the effective exchange rate
    pub fn exchange_rate(&self) -> f64 {
        if self.amount_in.is_zero() {
            return 0.0;
        }
        let amount_in = self.amount_in.as_u128() as f64;
        let amount_out = self.amount_out.as_u128() as f64;
        amount_out / amount_in
    }

    /// Get number of hops
    pub fn hop_count(&self) -> usize {
        self.hops.len()
    }

    /// Generate a human-readable route path
    pub fn route_path(&self) -> String {
        if self.hops.is_empty() {
            return "Direct".to_string();
        }
        
        let mut path = vec![format!("{:?}", self.token_in)];
        for hop in &self.hops {
            path.push(format!("{:?}", hop.token_out));
        }
        path.join(" → ")
    }
}

/// Token metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub address: Address,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
}

impl fmt::Display for TokenInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.symbol, self.name)
    }
}

/// Optimization strategy for route selection
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OptimizationStrategy {
    /// Maximize output amount (default)
    Price,
    
    /// Minimize gas cost
    Gas,
    
    /// Minimize slippage/price impact
    Slippage,
    
    /// Balanced optimization
    Balanced,
}

impl OptimizationStrategy {
    /// Get weights for composite scoring
    /// Returns (price_weight, gas_weight, slippage_weight)
    pub fn get_weights(&self) -> (f64, f64, f64) {
        match self {
            OptimizationStrategy::Price => (1.0, 0.1, 0.1),
            OptimizationStrategy::Gas => (0.3, 1.0, 0.1),
            OptimizationStrategy::Slippage => (0.3, 0.1, 1.0),
            OptimizationStrategy::Balanced => (0.5, 0.3, 0.2),
        }
    }
}

impl fmt::Display for OptimizationStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptimizationStrategy::Price => write!(f, "Price"),
            OptimizationStrategy::Gas => write!(f, "Gas"),
            OptimizationStrategy::Slippage => write!(f, "Slippage"),
            OptimizationStrategy::Balanced => write!(f, "Balanced"),
        }
    }
}

/// Market context for intelligent routing
#[derive(Debug, Clone)]
pub struct MarketContext {
    /// Current gas price in gwei
    pub gas_price_gwei: u64,
    
    /// ETH price in USD (for gas cost calculation)
    pub eth_price_usd: f64,
    
    /// Current block number
    pub block_number: u64,
}

impl Default for MarketContext {
    fn default() -> Self {
        Self {
            gas_price_gwei: 30,
            eth_price_usd: 1800.0,
            block_number: 0,
        }
    }
}
