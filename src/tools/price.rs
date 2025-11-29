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
    pub token_identifier: String, // Can be symbol or contract address
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceResponse {
    pub token: String,
    pub price_usd: String,
    pub price_eth: String,
    pub timestamp: u64,
    pub data_source: String,
}

pub struct PriceTool {
    _rpc: RpcClient,
    token_registry: TokenRegistry,
    // In production, would use CoinGecko/CoinMarketCap API
    // For now, we'll use a hardcoded price oracle
    price_cache: HashMap<String, (Decimal, Decimal)>,
}

impl PriceTool {
    pub fn new(rpc: RpcClient) -> Self {
        let mut price_cache = HashMap::new();

        // Mock price data for common tokens (in production, fetch from Uniswap pools or APIs)
        // Format: (USD price, ETH price)
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

    /// Get token price information
    pub async fn get_price(&self, request: PriceRequest) -> Result<PriceResponse> {
        debug!("Getting price for token: {}", request.token_identifier);

        let token_identifier = &request.token_identifier.to_uppercase();

        // Try to resolve symbol to address or vice versa
        let (symbol, _address) = if let Ok(addr) = token_identifier.parse::<Address>() {
            // It's an address
            let symbol = self
                .token_registry
                .address_to_symbol(addr)
                .unwrap_or_else(|| "UNKNOWN".to_string());
            (symbol, Some(addr))
        } else {
            // It's a symbol
            let addr = self
                .token_registry
                .symbol_to_address(token_identifier)
                .ok_or_else(|| {
                    EthereumError::TokenNotFound(format!("Token not found: {}", token_identifier))
                })?;
            (token_identifier.clone(), Some(addr))
        };

        // Get price from cache (in production, would fetch from Uniswap or price API)
        let (price_usd, price_eth) = self.price_cache.get(&symbol).copied().ok_or_else(|| {
            EthereumError::PriceOracleError(format!("Price data not available for: {}", symbol))
        })?;

        info!("Retrieved price for {}: ${}", symbol, price_usd);

        Ok(PriceResponse {
            token: symbol,
            price_usd: price_usd.normalize().to_string(),
            price_eth: price_eth.normalize().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            data_source: "Mock Oracle".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_response_serialization() {
        let response = PriceResponse {
            token: "ETH".to_string(),
            price_usd: "2500".to_string(),
            price_eth: "1".to_string(),
            timestamp: 1735689600,
            data_source: "Mock Oracle".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("ETH"));
        assert!(json.contains("2500"));
    }

    #[test]
    fn test_token_symbol_normalization() {
        let request = PriceRequest {
            token_identifier: "eth".to_string(),
        };
        assert_eq!(request.token_identifier.to_uppercase(), "ETH");
    }
}
