## Rust DEX Aggregator

A high-performance decentralized exchange (DEX) aggregator built in Rust for finding optimal token swap routes across multiple DEXes on Ethereum mainnet.

## Overview

This aggregator implements intelligent routing algorithms to find the best swap prices by comparing routes across Uniswap V2, SushiSwap, and other AMM-based DEXes. It supports multi-hop routing (up to 4 hops) and provides real-time price quotes with detailed cost analysis.

## Features

### Core Functionality
- Multi-DEX aggregation (Uniswap V2, SushiSwap)
- Multi-hop routing with BFS pathfinding algorithm
- Real-time price quote calculations
- Gas estimation and price impact analysis
- Pool caching system for improved performance
- Support for 20+ major ERC-20 tokens

### Optimization Strategies
- **Price**: Maximizes output amount
- **Gas**: Minimizes gas costs
- **Slippage**: Minimizes price impact
- **Balanced**: Optimizes across all factors (default)

### CLI Features
- Token symbol support (WETH, USDC, DAI, etc.)
- Colorized output with detailed route breakdowns
- Hop-by-hop swap calculations
- Alternative route comparison
- Cache management (import/export/stats)
- Real-time pool data refresh

## Architecture

```
rust-aggregator/
├── src/
│   ├── main.rs           # CLI interface and command handlers
│   ├── lib.rs            # Public API and Aggregator struct
│   ├── config.rs         # Configuration management
│   ├── pools.rs          # Pool fetching and caching
│   ├── router.rs         # Route finding and optimization
│   ├── quote.rs          # Quote calculation engine
│   ├── types.rs          # Core data structures
│   └── utils.rs          # Helper functions and formatting
├── cache/                # Pool data cache directory
└── .env                  # Configuration file
```

## Installation

### Prerequisites
- Rust 1.70 or higher
- Ethereum RPC endpoint (Alchemy, Infura, or public RPC)

### Build from Source

```bash
git clone https://github.com/Pandey-A/Rust-DEX-Agg.git
cd rust-aggregator
cargo build --release
```

The compiled binary will be available at `target/release/dex` (or `dex.exe` on Windows).

## Configuration

Create a `.env` file in the project root:

```env
# Required
RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY

# Optional (defaults shown)
CHAIN_ID=1
CACHE_ENABLED=true
CACHE_TTL_SECONDS=300
CACHE_PATH=./cache/pools.json
DEFAULT_SLIPPAGE_BPS=50
MAX_HOPS=3
GAS_PRICE_GWEI=30

# DEX Factory Addresses (defaults for Ethereum mainnet)
UNISWAP_V2_FACTORY=0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f
SUSHISWAP_FACTORY=0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac
```

## Usage

### Fetch Pool Data

Before getting quotes, fetch liquidity pool data from DEXes:

```bash
# Fetch from all supported DEXes
cargo run --release -- fetch-all-dexes --limit 100

# Fetch from specific factory
cargo run --release -- fetch-pools \
  --factory 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f \
  --name Uniswap \
  --limit 500
```

### Get Swap Quotes

Basic quote using token symbols:

```bash
cargo run --release -- quote WETH USDC 1.0
```

Quote with token addresses:

```bash
cargo run --release -- quote \
  0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2 \
  0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48 \
  1.0
```

Quote with optimization strategy:

```bash
cargo run --release -- quote WETH USDC 1.0 --optimize price
cargo run --release -- quote WETH USDC 1.0 --optimize gas
cargo run --release -- quote WETH USDC 1.0 --optimize slippage
```

Quote with real-time data refresh:

```bash
cargo run --release -- quote WETH USDC 1.0 --refresh
```

Show alternative routes for comparison:

```bash
cargo run --release -- quote USDC USDT 1000.0 --show-alternatives 5
```

### Cache Management

View cache statistics:

```bash
cargo run --release -- cache stats
```

Export cache to file:

```bash
cargo run --release -- cache export ./backup/pools.json
```

Import cache from file:

```bash
cargo run --release -- cache import ./backup/pools.json
```

Clear all cached data:

```bash
cargo run --release -- cache clear
```

### List Pools

List all cached pools:

```bash
cargo run --release -- list-pools
```

Filter by token address:

```bash
cargo run --release -- list-pools --token 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2
```

## Supported Tokens

The aggregator recognizes the following token symbols:

| Symbol | Token Name | Decimals |
|--------|------------|----------|
| ETH/WETH | Wrapped Ether  | 18 |
| USDC | USD Coin            | 6 |
| USDT | Tether USD          | 6 |
| DAI | Dai Stablecoin       | 18|
| WBTC/BTC | Wrapped Bitcoin | 8 |
| UNI | Uniswap Token       | 18 |
| LINK | Chainlink          | 18 |
| AAVE | Aave Token         | 18 |
| SUSHI | SushiToken        | 18 |
| COMP | Compound           | 18 |

Plus additional major tokens. Full token addresses can also be used directly.

## Output Example

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Best Route Found
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

   Route Path:
    USDC (0xa0b8...) → WETH (0xc02a...) → USDT (0xdac1...)

  Hops: 2 hops

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Hop-by-Hop Breakdown
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Hop 1 USDC → WETH
    Swap: 1000 USDC → 0.258764574299385897 WETH
    Rate: 0.000259 WETH per USDC
    DEX: Uniswap (0xb4e1...)

  Hop 2 WETH → USDT
    Swap: 0.258764574299385897 WETH → 1001.35596 USDT
    Rate: 3869.756758 USDT per WETH
    DEX: Uniswap (0x0d4a...)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Quote Details
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Input: 1000 USDC
  Output: 1001.35596 USDT
  Rate: 1.001356 USDT per USDC

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Cost Analysis
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

   Gas Estimate: 200000
   Price Impact: 0.20%
```
## Demo


https://github.com/user-attachments/assets/b6044d10-ce32-40e6-a562-26f145e6e40d


## Algorithm Details

### Route Finding
The router uses breadth-first search (BFS) to discover all possible routes between token pairs, supporting paths up to 4 hops. Each route is scored based on the selected optimization strategy.

### Quote Calculation
Quotes are calculated using the constant product formula (x * y = k) from UniswapV2, accounting for the 0.3% swap fee at each hop.

### Optimization Scoring
Each route receives a composite score based on:
- Output amount (higher is better)
- Gas cost estimate (lower is better)
- Price impact in basis points (lower is better)

The weights vary by strategy:
- **Price**: 80% output, 10% gas, 10% slippage
- **Gas**: 20% output, 70% gas, 10% slippage
- **Slippage**: 20% output, 10% gas, 70% slippage
- **Balanced**: 50% output, 25% gas, 25% slippage

## API Usage

The aggregator can be used as a library in other Rust projects:

```rust
use rust_aggregator::{Aggregator, Config, OptimizationStrategy};
use ethers::types::{Address, U256};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize aggregator
    let config = Config::from_env()?;
    let aggregator = Aggregator::new(config).await?;
    
    // Fetch pools
    aggregator.fetch_all_pools(Some(100)).await?;
    
    // Get quote
    let token_in = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse()?;
    let token_out = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse()?;
    let amount = U256::from(1_000_000_000_000_000_000u64); // 1.0 WETH
    
    let quote = aggregator.get_best_quote(
        token_in,
        token_out,
        amount,
        OptimizationStrategy::Balanced
    )?;
    
    println!("Best route: {}", quote.description);
    println!("Output: {}", quote.amount_out);
    
    Ok(())
}
```

## Performance

- Initial pool fetch: 30-60 seconds for 500-1000 pools per DEX
- Quote calculation: 10-50ms for routes with up to 200 possible paths
- Route finding: O(n * m) where n is number of pools and m is max hops
- Memory usage: ~10-20MB for 1000 cached pools

## Limitations

- Currently supports only UniswapV2-style AMMs
- Does not execute actual swaps (quote-only)
- Limited to Ethereum mainnet
- No support for UniswapV3 concentrated liquidity
- Rate limited by RPC provider

## Future Enhancements

- UniswapV3 integration with tick-based liquidity
- Transaction execution with MEV protection
- Multi-chain support (Polygon, Arbitrum, Optimism)
- Historical price tracking and analytics
- GraphQL/REST API server
- WebSocket support for real-time updates
- Flash loan arbitrage detection

## Dependencies

- **ethers-rs**: Ethereum interaction
- **tokio**: Async runtime
- **clap**: CLI argument parsing
- **colored**: Terminal output formatting
- **serde**: JSON serialization
- **dashmap**: Concurrent HashMap
- **tracing**: Structured logging

## Contributing

Contributions are welcome. Please ensure all tests pass before submitting pull requests:

```bash
cargo test
cargo clippy
cargo fmt
```




