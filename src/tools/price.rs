use alloy::primitives::Address;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

use crate::error::{EthereumError, Result};
use crate::rpc::RpcClient;
use crate::tokens::TokenRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceRequest {
    // 代币名称或者代币地址
    pub token_identifier: String, // 可以是符号或合约地址
    // 报价货币，默认是 USD
    pub quote_currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceResponse {
    pub unit: String,
    pub price: String,
    pub timestamp: u64,
}

pub struct PriceTool {
    _rpc: RpcClient,
    token_registry: TokenRegistry,
    // 在生产环境中，将使用 CoinGecko/CoinMarketCap API
    // 目前，我们将使用硬编码的价格预言机
    price_cache: HashMap<String, (Decimal, Decimal)>,
}

impl PriceTool {
    pub fn new(rpc: RpcClient) -> Self {
        let mut price_cache = HashMap::new();

        // 常见代币的模拟价格数据（在生产环境中，从 Uniswap 池或 API 获取）
        // 格式: (USD 价格, ETH 价格)
        price_cache.insert(
            "ETH".to_string(),
            (Decimal::from_str_exact("2500").unwrap(), Decimal::from(1)),
        );
        price_cache.insert(
            "USDC".to_string(),
            (
                Decimal::from_str_exact("1.0").unwrap(),
                Decimal::from_str_exact("0.0004").unwrap(),
            ),
        );
        price_cache.insert(
            "USDT".to_string(),
            (
                Decimal::from_str_exact("1.002").unwrap(),
                Decimal::from_str_exact("0.0004").unwrap(),
            ),
        );
        price_cache.insert(
            "DAI".to_string(),
            (
                Decimal::from_str_exact("1.0").unwrap(),
                Decimal::from_str_exact("0.0004").unwrap(),
            ),
        );

        PriceTool {
            _rpc: rpc,
            token_registry: TokenRegistry::new(),
            price_cache,
        }
    }

    /// 获取代币价格信息
    pub async fn get_price(&self, request: PriceRequest) -> Result<PriceResponse> {
        debug!("正在获取代币价格: {}", request.token_identifier);

        let token_identifier = &request.token_identifier.to_uppercase();

        // 尝试将符号解析为地址或反之
        let (symbol, _address) = if let Ok(addr) = token_identifier.parse::<Address>() {
            // 是一个地址
            let symbol = self
                .token_registry
                .address_to_symbol(addr)
                .unwrap_or_else(|| "UNKNOWN".to_string());
            (symbol, Some(addr))
        } else {
            // 是一个符号
            let addr = self
                .token_registry
                .symbol_to_address(token_identifier)
                .ok_or_else(|| {
                    EthereumError::TokenNotFound(format!("代币不存在: {}", token_identifier))
                })?;
            (token_identifier.clone(), Some(addr))
        };

        // 从缓存获取价格（在生产环境中，将从 Uniswap 或价格 API 获取）
        let (price_usd, _) = self.price_cache.get(&symbol).copied().ok_or_else(|| {
            EthereumError::PriceOracleError(format!("价格数据不可用: {}", symbol))
        })?;

        info!("获取 {} 的价格: ${}", symbol, price_usd);

        Ok(PriceResponse {
            unit: symbol,
            price: price_usd.normalize().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_response_serialization() {
        let response = PriceResponse {
            unit: "ETH".to_string(),
            price: "2500".to_string(),
            timestamp: 1735689600,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("ETH"));
        assert!(json.contains("2500"));
    }

    #[test]
    fn test_token_symbol_normalization() {
        let request = PriceRequest {
            token_identifier: "eth".to_string(),
            quote_currency: None,
        };
        assert_eq!(request.token_identifier.to_uppercase(), "ETH");
    }
}
