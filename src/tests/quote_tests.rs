use rust_aggregator::{
    quote::QuoteEngine, types::PoolInfo, utils,
};
use ethers::types::{Address, U256};

fn create_test_pool(reserve0: u128, reserve1: u128) -> PoolInfo {
    PoolInfo {
        address: Address::zero(),
        token0: Address::from_low_u64_be(1),
        token1: Address::from_low_u64_be(2),
        reserve0: U256::from(reserve0),
        reserve1: U256::from(reserve1),
        fee_bps: 30,
        dex_name: "TestDEX".to_string(),
        last_updated: 0,
    }
}

#[test]
fn test_uniswap_v2_formula() {
    // Test the constant product formula with known values
    let pool = create_test_pool(
        100_000_000_000_000_000_000,  // 100 ETH
        200_000_000_000_000_000_000,  // 200 ETH
    );

    let amount_in = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
    let result = QuoteEngine::calculate_pool_output(&pool, pool.token0, amount_in);

    assert!(result.is_ok());
    let quote = result.unwrap();
    
    // Output should be less than 2 ETH due to slippage and fees
    assert!(quote.amount_out > U256::zero());
    assert!(quote.amount_out < U256::from(2_000_000_000_000_000_000u128));
}

#[test]
fn test_price_impact_calculation() {
    // Small trade should have low price impact
    let pool = create_test_pool(
        1000_000_000_000_000_000_000,  // 1000 ETH
        2000_000_000_000_000_000_000,  // 2000 ETH
    );

    let small_amount = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
    let result = QuoteEngine::calculate_pool_output(&pool, pool.token0, small_amount);
    
    assert!(result.is_ok());
    let quote = result.unwrap();
    
    // Price impact should be low for small trade
    assert!(quote.price_impact_bps < 100); // Less than 1%
}

#[test]
fn test_large_trade_price_impact() {
    // Large trade should have higher price impact
    let pool = create_test_pool(
        100_000_000_000_000_000_000,  // 100 ETH
        200_000_000_000_000_000_000,  // 200 ETH
    );

    let large_amount = U256::from(50_000_000_000_000_000_000u128); // 50 ETH (50% of pool)
    let result = QuoteEngine::calculate_pool_output(&pool, pool.token0, large_amount);
    
    assert!(result.is_ok());
    let quote = result.unwrap();
    
    // Price impact should be significant for large trade
    assert!(quote.price_impact_bps > 100); // More than 1%
}

#[test]
fn test_zero_amount_error() {
    let pool = create_test_pool(
        100_000_000_000_000_000_000,
        200_000_000_000_000_000_000,
    );

    let zero_amount = U256::zero();
    let result = QuoteEngine::calculate_pool_output(&pool, pool.token0, zero_amount);
    
    assert!(result.is_err());
}

#[test]
fn test_insufficient_liquidity() {
    let pool = create_test_pool(
        0,  // No liquidity
        200_000_000_000_000_000_000,
    );

    let amount_in = U256::from(1_000_000_000_000_000_000u128);
    let result = QuoteEngine::calculate_pool_output(&pool, pool.token0, amount_in);
    
    assert!(result.is_err());
}

#[test]
fn test_parse_token_amount() {
    // Test various formats
    let cases = vec![
        ("1.0", 18, U256::from(1_000_000_000_000_000_000u128)),
        ("0.5", 18, U256::from(500_000_000_000_000_000u128)),
        ("1000", 6, U256::from(1_000_000_000u128)),
        ("1.23456", 6, U256::from(1_234_560u128)),
    ];

    for (input, decimals, expected) in cases {
        let result = utils::parse_token_amount(input, decimals);
        assert!(result.is_ok(), "Failed to parse: {}", input);
        assert_eq!(result.unwrap(), expected, "Wrong value for: {}", input);
    }
}

#[test]
fn test_format_token_amount() {
    let cases = vec![
        (U256::from(1_000_000_000_000_000_000u128), 18, "1"),
        (U256::from(1_500_000_000_000_000_000u128), 18, "1.5"),
        (U256::from(1_234_560_000_000_000_000u128), 18, "1.23456"),
        (U256::from(1_000_000u128), 6, "1"),
    ];

    for (amount, decimals, expected) in cases {
        let result = utils::format_token_amount(amount, decimals);
        assert_eq!(result, expected, "Wrong format for: {}", amount);
    }
}

#[test]
fn test_best_pool_selection() {
    // Create two pools with different liquidity
    let pool1 = PoolInfo {
        address: Address::from_low_u64_be(100),
        reserve0: U256::from(100_000_000_000_000_000_000u128),
        reserve1: U256::from(200_000_000_000_000_000_000u128),
        ..create_test_pool(0, 0)
    };

    let pool2 = PoolInfo {
        address: Address::from_low_u64_be(101),
        reserve0: U256::from(200_000_000_000_000_000_000u128),
        reserve1: U256::from(400_000_000_000_000_000_000u128),
        ..create_test_pool(0, 0)
    };

    let pools = vec![pool1, pool2];
    let amount_in = U256::from(1_000_000_000_000_000_000u128);

    let result = QuoteEngine::find_best_direct_pool(
        &pools,
        Address::from_low_u64_be(1),
        Address::from_low_u64_be(2),
        amount_in,
    );

    assert!(result.is_ok());
    
    // Pool 2 should provide better output due to deeper liquidity
    let quote = result.unwrap();
    assert_eq!(quote.pool.address, Address::from_low_u64_be(101));
}

#[test]
fn test_exchange_rate_calculation() {
    let pool = create_test_pool(
        100_000_000_000_000_000_000,  // 100 ETH
        180_000_000_000u128,           // 180k USDC (6 decimals)
    );

    let amount_in = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
    let result = QuoteEngine::calculate_pool_output(&pool, pool.token0, amount_in);
    
    assert!(result.is_ok());
    let quote = result.unwrap();
    
    // Exchange rate should be approximately 1800 (accounting for fees and slippage)
    let rate = quote.exchange_rate();
    assert!(rate > 0.0);
}
