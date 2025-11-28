use crate::types::{AggregatorError, Result};
use ethers::types::{Address, U256};
use std::str::FromStr;

/// Calculate UniswapV2 output amount using the constant product formula
/// amountOut = (amountIn * feeFactor * reserveOut) / (reserveIn * 10000 + amountIn * feeFactor)
/// where feeFactor = 10000 - fee_bps (e.g., 9970 for 0.3% fee)
pub fn calculate_uniswap_v2_output(
    amount_in: U256,
    reserve_in: U256,
    reserve_out: U256,
    fee_bps: u32,
) -> Result<U256> {
    // Validate inputs
    if amount_in.is_zero() {
        return Err(AggregatorError::InvalidAmount("Amount in cannot be zero".to_string()));
    }
    if reserve_in.is_zero() || reserve_out.is_zero() {
        return Err(AggregatorError::InsufficientLiquidity("Pool has zero reserves".to_string()));
    }

    // Calculate fee factor (10000 - fee_bps)
    let fee_factor = U256::from(10000 - fee_bps);
    let fee_base = U256::from(10000);

    // amountInWithFee = amountIn * feeFactor
    let amount_in_with_fee = amount_in
        .checked_mul(fee_factor)
        .ok_or(AggregatorError::MathError)?;

    // numerator = amountInWithFee * reserveOut
    let numerator = amount_in_with_fee
        .checked_mul(reserve_out)
        .ok_or(AggregatorError::MathError)?;

    // denominator = reserveIn * 10000 + amountInWithFee
    let denominator = reserve_in
        .checked_mul(fee_base)
        .ok_or(AggregatorError::MathError)?
        .checked_add(amount_in_with_fee)
        .ok_or(AggregatorError::MathError)?;

    // amountOut = numerator / denominator
    let amount_out = numerator
        .checked_div(denominator)
        .ok_or(AggregatorError::MathError)?;

    if amount_out.is_zero() {
        return Err(AggregatorError::InsufficientLiquidity(
            "Output amount would be zero".to_string(),
        ));
    }

    Ok(amount_out)
}

/// Calculate price impact in basis points
pub fn calculate_price_impact(
    amount_in: U256,
    reserve_in: U256,
    amount_out: U256,
    reserve_out: U256,
) -> u32 {
    if reserve_in.is_zero() || reserve_out.is_zero() {
        return 10000; // 100% impact if no liquidity
    }

    // Spot price before swap: reserveOut / reserveIn
    // Execution price: amountOut / amountIn
    // Price impact = (1 - executionPrice / spotPrice) * 10000

    // To avoid floating point, we use: impact = (spotPrice - executionPrice) / spotPrice * 10000
    // Which becomes: impact = (amountIn * reserveOut - amountOut * reserveIn) / (amountIn * reserveOut) * 10000

    let numerator = match amount_in.checked_mul(reserve_out) {
        Some(val) => match amount_out.checked_mul(reserve_in) {
            Some(val2) => val.saturating_sub(val2),
            None => return 10000,
        },
        None => return 10000,
    };

    let denominator = match amount_in.checked_mul(reserve_out) {
        Some(val) => val,
        None => return 10000,
    };

    if denominator.is_zero() {
        return 10000;
    }

    // Calculate impact in basis points
    let impact = numerator
        .checked_mul(U256::from(10000))
        .and_then(|v| v.checked_div(denominator))
        .unwrap_or(U256::from(10000));

    impact.as_u32().min(10000)
}

/// Calculate the fee amount from an input amount
pub fn calculate_fee(amount: U256, fee_bps: u32) -> U256 {
    amount
        .checked_mul(U256::from(fee_bps))
        .and_then(|v| v.checked_div(U256::from(10000)))
        .unwrap_or(U256::zero())
}

/// Get token decimals for known tokens
/// Returns 18 (default) if token is unknown
pub fn get_token_decimals(token_address: Address) -> u8 {
    let addr_str = format!("{:?}", token_address).to_lowercase();
    
    match addr_str.as_str() {
        // Stablecoins (6 decimals)
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" => 6, // USDC
        "0xdac17f958d2ee523a2206206994597c13d831ec7" => 6, // USDT
        
        // Stablecoins (18 decimals)
        "0x6b175474e89094c44da98b954eedeac495271d0f" => 18, // DAI
        "0x0000000000085d4780b73119b644ae5ecd22b376" => 18, // TUSD
        "0x57ab1ec28d129707052df4df418d58a2d46d5f51" => 18, // sUSD
        
        // Major tokens (18 decimals)
        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" => 18, // WETH
        "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599" => 8,  // WBTC
        "0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2" => 18, // MKR
        "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984" => 18, // UNI
        "0x514910771af9ca656af840dff83e8264ecf986ca" => 18, // LINK
        "0x7d1afa7b718fb893db30a3abc0cfc608aacfebb0" => 18, // MATIC
        "0x0d8775f648430679a709e98d2b0cb6250d2887ef" => 18, // BAT
        "0xdd974d5c2e2928dea5f71b9825b8b646686bd200" => 18, // KNC
        
        // Default: 18 decimals (most ERC20 tokens use 18)
        _ => 18,
    }
}

/// Get token symbol for known tokens
/// Returns shortened address if token is unknown
pub fn get_token_symbol(token_address: Address) -> String {
    let addr_str = format!("{:?}", token_address).to_lowercase();
    
    match addr_str.as_str() {
        // Stablecoins
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" => "USDC".to_string(),
        "0xdac17f958d2ee523a2206206994597c13d831ec7" => "USDT".to_string(),
        "0x6b175474e89094c44da98b954eedeac495271d0f" => "DAI".to_string(),
        "0x0000000000085d4780b73119b644ae5ecd22b376" => "TUSD".to_string(),
        "0x57ab1ec28d129707052df4df418d58a2d46d5f51" => "sUSD".to_string(),
        "0x4fabb145d64652a948d72533023f6e7a623c7c53" => "BUSD".to_string(),
        
        // Major tokens
        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" => "WETH".to_string(),
        "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599" => "WBTC".to_string(),
        "0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2" => "MKR".to_string(),
        "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984" => "UNI".to_string(),
        "0x514910771af9ca656af840dff83e8264ecf986ca" => "LINK".to_string(),
        "0x7d1afa7b718fb893db30a3abc0cfc608aacfebb0" => "MATIC".to_string(),
        "0x0d8775f648430679a709e98d2b0cb6250d2887ef" => "BAT".to_string(),
        "0xdd974d5c2e2928dea5f71b9825b8b646686bd200" => "KNC".to_string(),
        "0x6b3595068778dd592e39a122f4f5a5cf09c90fe2" => "SUSHI".to_string(),
        "0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9" => "AAVE".to_string(),
        "0xc00e94cb662c3520282e6f5717214004a7f26888" => "COMP".to_string(),
        "0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce" => "SHIB".to_string(),
        "0x6810e776880c02933d47db1b9fc05908e5386b96" => "GNO".to_string(),
        "0xa0b73e1ff0b80914ab6fe0444e65848c4c34450b" => "CRO".to_string(),
        
        // Default: return shortened address
        _ => {
            let addr = format!("{:?}", token_address);
            if addr.len() > 10 {
                format!("{}...{}", &addr[0..6], &addr[addr.len()-4..])
            } else {
                addr
            }
        }
    }
}

/// Parse token symbol or address to Address
/// Accepts both symbols (e.g., "WETH", "USDC") and addresses (e.g., "0xc02a...")
pub fn parse_token(input: &str) -> Result<Address> {
    // If it starts with 0x, treat as address
    if input.starts_with("0x") || input.starts_with("0X") {
        return parse_address(input);
    }
    
    // Otherwise, treat as symbol (case-insensitive)
    let symbol_upper = input.to_uppercase();
    let address_str = match symbol_upper.as_str() {
        // Stablecoins
        "USDC" => "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        "USDT" => "0xdac17f958d2ee523a2206206994597c13d831ec7",
        "DAI" => "0x6b175474e89094c44da98b954eedeac495271d0f",
        "TUSD" => "0x0000000000085d4780b73119b644ae5ecd22b376",
        "SUSD" => "0x57ab1ec28d129707052df4df418d58a2d46d5f51",
        "BUSD" => "0x4fabb145d64652a948d72533023f6e7a623c7c53",
        
        // Major tokens
        "WETH" | "ETH" => "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        "WBTC" | "BTC" => "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599",
        "MKR" => "0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2",
        "UNI" => "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984",
        "LINK" => "0x514910771af9ca656af840dff83e8264ecf986ca",
        "MATIC" => "0x7d1afa7b718fb893db30a3abc0cfc608aacfebb0",
        "BAT" => "0x0d8775f648430679a709e98d2b0cb6250d2887ef",
        "KNC" => "0xdd974d5c2e2928dea5f71b9825b8b646686bd200",
        "SUSHI" => "0x6b3595068778dd592e39a122f4f5a5cf09c90fe2",
        "AAVE" => "0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9",
        "COMP" => "0xc00e94cb662c3520282e6f5717214004a7f26888",
        "SHIB" => "0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce",
        "GNO" => "0x6810e776880c02933d47db1b9fc05908e5386b96",
        "CRO" => "0xa0b73e1ff0b80914ab6fe0444e65848c4c34450b",
        
        _ => return Err(AggregatorError::ParseError(
            format!("Unknown token symbol: {}. Use full address (0x...) or supported symbol (WETH, USDC, DAI, etc.)", input)
        )),
    };
    
    parse_address(address_str)
}

/// Parse a token amount string with decimal support
/// Examples: "1.0", "0.5", "1000"
pub fn parse_token_amount(amount_str: &str, decimals: u8) -> Result<U256> {
    let parts: Vec<&str> = amount_str.split('.').collect();
    
    if parts.is_empty() || parts.len() > 2 {
        return Err(AggregatorError::ParseError(
            format!("Invalid amount format: {}", amount_str)
        ));
    }

    let integer_part = parts[0].parse::<u128>()
        .map_err(|_| AggregatorError::ParseError(format!("Invalid integer part: {}", parts[0])))?;

    let decimal_part = if parts.len() == 2 {
        let dec_str = parts[1];
        if dec_str.len() > decimals as usize {
            return Err(AggregatorError::ParseError(
                format!("Too many decimal places. Max: {}", decimals)
            ));
        }
        // Pad with zeros to reach full decimals
        let padded = format!("{:0<width$}", dec_str, width = decimals as usize);
        padded.parse::<u128>()
            .map_err(|_| AggregatorError::ParseError(format!("Invalid decimal part: {}", dec_str)))?
    } else {
        0
    };

    // Calculate: (integer_part * 10^decimals) + decimal_part
    let multiplier = 10u128.pow(decimals as u32);
    let total = integer_part
        .checked_mul(multiplier)
        .and_then(|v| v.checked_add(decimal_part))
        .ok_or(AggregatorError::MathError)?;

    Ok(U256::from(total))
}

/// Format a token amount with decimals for display
pub fn format_token_amount(amount: U256, decimals: u8) -> String {
    if amount.is_zero() {
        return "0".to_string();
    }

    let divisor = U256::from(10u128.pow(decimals as u32));
    let integer_part = amount / divisor;
    let remainder = amount % divisor;

    if remainder.is_zero() {
        return format!("{}", integer_part);
    }

    // Format with decimals
    let decimal_str = format!("{:0>width$}", remainder, width = decimals as usize);
    let trimmed = decimal_str.trim_end_matches('0');
    
    if trimmed.is_empty() {
        format!("{}", integer_part)
    } else {
        format!("{}.{}", integer_part, trimmed)
    }
}

/// Format a number with thousands separators
pub fn format_with_commas(value: f64) -> String {
    let formatted = format!("{:.2}", value);
    let parts: Vec<&str> = formatted.split('.').collect();
    let integer_part = parts[0];
    let decimal_part = if parts.len() > 1 { parts[1] } else { "00" };

    let mut result = String::new();
    for (i, ch) in integer_part.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }

    format!("{}.{}", result.chars().rev().collect::<String>(), decimal_part)
}

/// Parse an Ethereum address from string
pub fn parse_address(addr_str: &str) -> Result<Address> {
    Address::from_str(addr_str)
        .map_err(|_| AggregatorError::InvalidTokenAddress(addr_str.to_string()))
}

/// Convert wei to ether as f64
pub fn wei_to_ether(wei: U256) -> f64 {
    let eth_decimals = 18;
    let divisor = 10f64.powi(eth_decimals);
    wei.as_u128() as f64 / divisor
}

/// Convert gwei to wei
pub fn gwei_to_wei(gwei: u64) -> U256 {
    U256::from(gwei) * U256::from(1_000_000_000u64)
}

/// Estimate gas cost in USD
pub fn estimate_gas_cost_usd(gas_used: U256, gas_price_gwei: u64, eth_price_usd: f64) -> f64 {
    let gas_price_wei = gwei_to_wei(gas_price_gwei);
    let total_cost_wei = gas_used * gas_price_wei;
    let total_cost_eth = wei_to_ether(total_cost_wei);
    total_cost_eth * eth_price_usd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_uniswap_v2_output() {
        // Test case: 1 ETH in, reserves 100 ETH / 180000 USDC, 0.3% fee
        let amount_in = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
        let reserve_in = U256::from(100_000_000_000_000_000_000u128); // 100 ETH
        let reserve_out = U256::from(180_000_000_000u128); // 180k USDC (6 decimals)
        let fee_bps = 30; // 0.3%

        let result = calculate_uniswap_v2_output(amount_in, reserve_in, reserve_out, fee_bps);
        assert!(result.is_ok());
        
        let amount_out = result.unwrap();
        assert!(amount_out > U256::zero());
    }

    #[test]
    fn test_parse_token_amount() {
        let amount = parse_token_amount("1.0", 18).unwrap();
        assert_eq!(amount, U256::from(1_000_000_000_000_000_000u128));

        let amount = parse_token_amount("0.5", 18).unwrap();
        assert_eq!(amount, U256::from(500_000_000_000_000_000u128));

        let amount = parse_token_amount("1000", 6).unwrap();
        assert_eq!(amount, U256::from(1_000_000_000u128));
    }

    #[test]
    fn test_format_token_amount() {
        let amount = U256::from(1_000_000_000_000_000_000u128);
        assert_eq!(format_token_amount(amount, 18), "1");

        let amount = U256::from(1_500_000_000_000_000_000u128);
        assert_eq!(format_token_amount(amount, 18), "1.5");

        let amount = U256::from(1_234_560_000_000_000_000u128);
        assert_eq!(format_token_amount(amount, 18), "1.23456");
    }

    #[test]
    fn test_format_with_commas() {
        assert_eq!(format_with_commas(1829.43), "1,829.43");
        assert_eq!(format_with_commas(1000000.50), "1,000,000.50");
    }

    #[test]
    fn test_gwei_to_wei() {
        let wei = gwei_to_wei(30);
        assert_eq!(wei, U256::from(30_000_000_000u64));
    }
}
