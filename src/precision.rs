use alloy::primitives::U256;
use rust_decimal::prelude::*;

use crate::error::{EthereumError, Result};

/// Convert raw token amount (in smallest units) to human-readable decimal format
///
/// # Arguments
/// * `raw_amount` - The raw amount in the smallest unit (wei for ETH, 10^decimals for tokens)
/// * `decimals` - The number of decimal places for the token
///
/// # Example
/// ```ignore
/// let raw = U256::from(1_000_000_000_000_000_000u64); // 1 wei (but for token with 18 decimals)
/// let decimal = to_decimal(raw, 18)?; // Returns Decimal::from(1)
/// ```
pub fn to_decimal(raw_amount: U256, decimals: u8) -> Result<Decimal> {
    let mut divisor = Decimal::from(1);
    for _ in 0..decimals {
        divisor = divisor * Decimal::from(10);
    }

    let amount_str = raw_amount.to_string();
    let amount_decimal = Decimal::from_str(&amount_str)
        .map_err(|e| EthereumError::PrecisionError(format!("Failed to parse amount: {}", e)))?;

    amount_decimal
        .checked_div(divisor)
        .ok_or_else(|| EthereumError::PrecisionError("Division overflow".to_string()))
}

/// Convert decimal amount to raw token amount (in smallest units)
///
/// # Arguments
/// * `decimal_amount` - The human-readable decimal amount
/// * `decimals` - The number of decimal places for the token
///
/// # Example
/// ```ignore
/// let decimal = Decimal::from(1);
/// let raw = from_decimal(decimal, 18)?; // Returns U256 representing 1e18
/// ```
pub fn from_decimal(decimal_amount: Decimal, decimals: u8) -> Result<U256> {
    let mut multiplier = Decimal::from(1);
    for _ in 0..decimals {
        multiplier = multiplier * Decimal::from(10);
    }

    let raw_decimal = decimal_amount
        .checked_mul(multiplier)
        .ok_or_else(|| EthereumError::PrecisionError("Multiplication overflow".to_string()))?;

    // Convert to u128 if it fits, otherwise to string and parse as U256
    let raw_u128 = raw_decimal
        .to_u128()
        .ok_or_else(|| EthereumError::PrecisionError("Amount too large".to_string()))?;

    Ok(U256::from(raw_u128))
}

/// Calculate minimum output with slippage tolerance
///
/// # Arguments
/// * `expected_output` - The expected output amount
/// * `slippage_percentage` - The slippage tolerance as a percentage (e.g., 0.5 for 0.5%)
///
/// # Example
/// ```ignore
/// let min_output = calculate_min_output_with_slippage(Decimal::from(100), Decimal::from_str("0.5")?)?;
/// // min_output = 99.5 (100 - 0.5%)
/// ```
pub fn calculate_min_output_with_slippage(
    expected_output: Decimal,
    slippage_percentage: Decimal,
) -> Result<Decimal> {
    if slippage_percentage < Decimal::ZERO || slippage_percentage > Decimal::from(100) {
        return Err(EthereumError::PrecisionError(
            "Slippage must be between 0 and 100".to_string(),
        ));
    }

    let slippage_multiplier = Decimal::from(1) - (slippage_percentage / Decimal::from(100));

    expected_output
        .checked_mul(slippage_multiplier)
        .ok_or_else(|| EthereumError::PrecisionError("Multiplication overflow".to_string()))
}

/// Convert U256 to decimal with proper formatting
pub fn u256_to_decimal(value: U256, decimals: u8) -> Result<String> {
    let decimal = to_decimal(value, decimals)?;
    Ok(decimal.normalize().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_decimal_eth() {
        // 1 ETH in wei (10^18)
        let raw = U256::from(1_000_000_000_000_000_000u64);
        let result = to_decimal(raw, 18).unwrap();
        assert_eq!(result, Decimal::from(1));
    }

    #[test]
    fn test_to_decimal_usdt() {
        // 1 USDT (10^6)
        let raw = U256::from(1_000_000u64);
        let result = to_decimal(raw, 6).unwrap();
        assert_eq!(result, Decimal::from(1));
    }

    #[test]
    fn test_from_decimal_eth() {
        let decimal = Decimal::from(1);
        let result = from_decimal(decimal, 18).unwrap();
        assert_eq!(result, U256::from(1_000_000_000_000_000_000u64));
    }

    #[test]
    fn test_from_decimal_usdt() {
        let decimal = Decimal::from(1);
        let result = from_decimal(decimal, 6).unwrap();
        assert_eq!(result, U256::from(1_000_000u64));
    }

    #[test]
    fn test_slippage_calculation() {
        let expected = Decimal::from_str("100").unwrap();
        let slippage = Decimal::from_str("0.5").unwrap();
        let min_output = calculate_min_output_with_slippage(expected, slippage).unwrap();
        let expected_min = Decimal::from_str("99.5").unwrap();
        assert_eq!(min_output, expected_min);
    }

    #[test]
    fn test_roundtrip_conversion() {
        let original = Decimal::from_str("123.456").unwrap();
        let raw = from_decimal(original, 18).unwrap();
        let converted = to_decimal(raw, 18).unwrap();
        // Allow for small rounding errors
        assert!((original - converted).abs() < Decimal::from_str("0.000000001").unwrap());
    }
}
