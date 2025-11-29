use alloy::primitives::U256;
use rust_decimal::prelude::*;

use crate::error::{EthereumError, Result};

/// 将原始代币金额（最小单位）转换为人类可读的十进制格式
///
/// # 参数
/// * `raw_amount` - 最小单位的原始金额（ETH 的 wei，代币的 10^decimals）
/// * `decimals` - 代币的小数位数
///
/// # 示例
/// ```ignore
/// let raw = U256::from(1_000_000_000_000_000_000u64); // 1 wei（但对于 18 位小数的代币）
/// let decimal = to_decimal(raw, 18)?; // 返回 Decimal::from(1)
/// ```
pub fn to_decimal(raw_amount: U256, decimals: u8) -> Result<Decimal> {
    let mut divisor = Decimal::from(1);
    for _ in 0..decimals {
        divisor *= Decimal::from(10);
    }

    let amount_str = raw_amount.to_string();
    let amount_decimal = Decimal::from_str(&amount_str)
        .map_err(|e| EthereumError::PrecisionError(format!("Failed to parse amount: {}", e)))?;

    amount_decimal
        .checked_div(divisor)
        .ok_or_else(|| EthereumError::PrecisionError("Division overflow".to_string()))
}

/// 将十进制金额转换为原始代币金额（最小单位）
///
/// # 参数
/// * `decimal_amount` - 人类可读的十进制金额
/// * `decimals` - 代币的小数位数
///
/// # 示例
/// ```ignore
/// let decimal = Decimal::from(1);
/// let raw = from_decimal(decimal, 18)?; // 返回代表 1e18 的 U256
/// ```
pub fn from_decimal(decimal_amount: Decimal, decimals: u8) -> Result<U256> {
    let mut multiplier = Decimal::from(1);
    for _ in 0..decimals {
        multiplier *= Decimal::from(10);
    }

    let raw_decimal = decimal_amount
        .checked_mul(multiplier)
        .ok_or_else(|| EthereumError::PrecisionError("Multiplication overflow".to_string()))?;

    // 如果适合，转换为 u128，否则转换为字符串并解析为 U256
    let raw_u128 = raw_decimal
        .to_u128()
        .ok_or_else(|| EthereumError::PrecisionError("金额过大".to_string()))?;

    Ok(U256::from(raw_u128))
}

/// 计算带有滑点容差的最小输出
///
/// # 参数
/// * `expected_output` - 预期输出金额
/// * `slippage_percentage` - 滑点容差百分比（例如 0.5 表示 0.5%）
///
/// # 示例
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

/// 将 U256 转换为十进制，并进行适当的格式化
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
        // 1 ETH（wei 中 10^18）
        let raw = U256::from(1_000_000_000_000_000_000u64);
        let result = to_decimal(raw, 18).unwrap();
        assert_eq!(result, Decimal::from(1));
    }

    #[test]
    fn test_to_decimal_usdt() {
        // 1 USDT (10^6)
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
        // 允许小的舍入错误
        assert!((original - converted).abs() < Decimal::from_str("0.000000001").unwrap());
    }
}
