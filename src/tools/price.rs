use alloy::primitives::{Address, U256};
use alloy::sol;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{EthereumError, Result};
use crate::precision;
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
    pub quote_currency: String,
    pub price: String,
    pub timestamp: u64,
}

// Uniswap V2 Pair contract interface
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract IUniswapV2Pair {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
        function token0() external view returns (address);
        function token1() external view returns (address);
    }
}

// Uniswap V2 Factory contract interface
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract IUniswapV2Factory {
        function getPair(address tokenA, address tokenB) external view returns (address pair);
    }
}

pub struct PriceTool {
    rpc: RpcClient,
    token_registry: TokenRegistry,
}

// Uniswap V2 主网地址
const UNISWAP_V2_FACTORY: &str = "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f";
const WETH_ADDRESS: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
const USDC_ADDRESS: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

impl PriceTool {
    pub fn new(rpc: RpcClient) -> Self {
        PriceTool {
            rpc,
            token_registry: TokenRegistry::new(),
        }
    }

    /// 从 Uniswap V2 池获取代币价格
    async fn get_price_from_uniswap_pool(
        &self,
        token_address: Address,
        quote_token: Address,
    ) -> Result<Decimal> {
        debug!(
            "从 Uniswap V2 池获取价格: token={:?}, quote={:?}",
            token_address, quote_token
        );

        let factory_address = UNISWAP_V2_FACTORY
            .parse::<Address>()
            .map_err(|_| EthereumError::ConfigError("无效的工厂地址".to_string()))?;

        let provider = self.rpc.get_provider()?;
        let factory = IUniswapV2Factory::new(factory_address, provider.clone());

        // 获取交易对地址
        let pair_address = factory
            .getPair(token_address, quote_token)
            .call()
            .await
            .map_err(|e| {
                warn!("获取交易对失败: {}", e);
                EthereumError::PriceOracleError(format!("无法获取交易对: {}", e))
            })?
            .pair;

        if pair_address == Address::ZERO {
            return Err(EthereumError::PriceOracleError("交易对不存在".to_string()));
        }

        // 获取储备量
        let pair = IUniswapV2Pair::new(pair_address, provider.clone());
        let reserves_result = pair.getReserves().call().await.map_err(|e| {
            warn!("获取储备量失败: {}", e);
            EthereumError::PriceOracleError(format!("无法获取储备量: {}", e))
        })?;

        let token0 = pair
            .token0()
            .call()
            .await
            .map_err(|e| {
                warn!("获取 token0 失败: {}", e);
                EthereumError::PriceOracleError(format!("无法获取 token0: {}", e))
            })?
            ._0;

        // 确定储备量的顺序
        let (reserve_token, reserve_quote) = if token0 == token_address {
            (reserves_result.reserve0, reserves_result.reserve1)
        } else {
            (reserves_result.reserve1, reserves_result.reserve0)
        };

        if reserve_quote == 0 {
            return Err(EthereumError::PriceOracleError(
                "报价代币储备为零".to_string(),
            ));
        }

        // 获取代币小数位数
        let token_decimals = self.rpc.get_token_decimals(token_address).await?;
        let quote_decimals = self.rpc.get_token_decimals(quote_token).await?;

        // 计算价格: (reserve_quote / 10^quote_decimals) / (reserve_token / 10^token_decimals)
        let reserve_token_decimal =
            precision::to_decimal(U256::from(reserve_token), token_decimals)?;
        let reserve_quote_decimal =
            precision::to_decimal(U256::from(reserve_quote), quote_decimals)?;

        let price = if reserve_token_decimal.is_zero() {
            return Err(EthereumError::PriceOracleError("代币储备为零".to_string()));
        } else {
            reserve_quote_decimal / reserve_token_decimal
        };

        info!(
            "从 Uniswap 池获取价格: {} = {} (报价代币)",
            token_address, price
        );

        Ok(price)
    }

    /// 获取代币价格信息
    pub async fn get_price(&self, request: PriceRequest) -> Result<PriceResponse> {
        debug!("正在获取代币价格: {}", request.token_identifier);

        let token_identifier = &request.token_identifier.to_uppercase();
        let quote_currency = request
            .quote_currency
            .unwrap_or_else(|| "USD".to_string())
            .to_uppercase();

        // 验证报价货币
        if quote_currency != "USD" && quote_currency != "ETH" {
            return Err(EthereumError::PriceOracleError(format!(
                "不支持的报价货币: {}",
                quote_currency
            )));
        }

        // 解析代币地址
        let token_address = if let Ok(addr) = token_identifier.parse::<Address>() {
            addr
        } else {
            self.token_registry
                .symbol_to_address(token_identifier)
                .ok_or_else(|| {
                    EthereumError::TokenNotFound(format!("代币不存在: {}", token_identifier))
                })?
        };

        // 获取代币符号
        let symbol = if let Ok(sym) = self.rpc.get_token_symbol(token_address).await {
            sym
        } else {
            self.token_registry
                .address_to_symbol(token_address)
                .unwrap_or_else(|| "UNKNOWN".to_string())
        };

        // 获取价格
        let price = if quote_currency == "ETH" {
            // 直接获取相对于 WETH 的价格
            let weth_address = WETH_ADDRESS
                .parse::<Address>()
                .map_err(|_| EthereumError::ConfigError("无效的 WETH 地址".to_string()))?;

            if token_address == weth_address {
                Decimal::from(1)
            } else {
                self.get_price_from_uniswap_pool(token_address, weth_address)
                    .await?
            }
        } else {
            // USD 价格: 先获取相对于 WETH 的价格，再乘以 ETH/USD 价格
            let weth_address = WETH_ADDRESS
                .parse::<Address>()
                .map_err(|_| EthereumError::ConfigError("无效的 WETH 地址".to_string()))?;

            let usdc_address = USDC_ADDRESS
                .parse::<Address>()
                .map_err(|_| EthereumError::ConfigError("无效的 USDC 地址".to_string()))?;

            let price_in_eth = if token_address == weth_address {
                Decimal::from(1)
            } else {
                self.get_price_from_uniswap_pool(token_address, weth_address)
                    .await?
            };

            // 获取 ETH/USDC 价格
            let eth_usdc_price = self
                .get_price_from_uniswap_pool(weth_address, usdc_address)
                .await?;

            price_in_eth * eth_usdc_price
        };

        info!("获取 {} 的价格: {} {}", symbol, price, quote_currency);

        Ok(PriceResponse {
            quote_currency,
            price: price.normalize().to_string(),
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
            quote_currency: "USD".to_string(),
            price: "2500".to_string(),
            timestamp: 1735689600,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("USD"));
        assert!(json.contains("2500"));
        assert!(json.contains("quote_currency"));
    }

    #[test]
    fn test_price_response_with_eth_quote() {
        let response = PriceResponse {
            quote_currency: "ETH".to_string(),
            price: "0.5".to_string(),
            timestamp: 1735689600,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("ETH"));
        assert!(json.contains("0.5"));
    }

    #[test]
    fn test_token_symbol_normalization() {
        let request = PriceRequest {
            token_identifier: "eth".to_string(),
            quote_currency: None,
        };
        assert_eq!(request.token_identifier.to_uppercase(), "ETH");
    }

    #[test]
    fn test_price_request_with_quote_currency() {
        let request = PriceRequest {
            token_identifier: "USDT".to_string(),
            quote_currency: Some("ETH".to_string()),
        };
        assert_eq!(request.quote_currency, Some("ETH".to_string()));
    }
}
