use alloy::primitives::Address;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::Result;
use crate::precision;
use crate::rpc::RpcClient;
use crate::tokens::TokenRegistry;

// ETH 地址的特殊标识符（通常用于区分 ETH 和 ERC20）
const ETH_IDENTIFIER: &str = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceRequest {
    /// 钱包地址
    pub address: String,
    /// 代币地址或符号（可选）
    /// - 如果不提供，默认查询 ETH
    /// - 如果提供为 ETH 特殊地址或符号，查询 ETH
    /// - 如果提供为合约地址，查询对应的 ERC20 代币
    pub token_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceResponse {
    /// 查询的钱包地址
    pub address: String,
    /// 余额（人类可读格式）
    pub balance: String,
    /// 代币小数位数
    pub decimals: u8,
    /// 原始余额（最小单位）
    pub raw: String,
    /// 代币符号或 "ETH"
    pub token_type: String,
    /// 代币合约地址（ETH 时为 ETH 特殊地址）
    pub token_address: String,
}

pub struct BalanceTool {
    rpc: RpcClient,
    token_registry: TokenRegistry,
}

impl BalanceTool {
    pub fn new(rpc: RpcClient) -> Self {
        BalanceTool {
            rpc,
            token_registry: TokenRegistry::new(),
        }
    }

    /// 验证以太坊地址格式
    fn validate_address(addr_str: &str) -> Result<Address> {
        addr_str.parse::<Address>().map_err(|_| {
            crate::error::EthereumError::InvalidAddress(format!("无效的以太坊地址: {}", addr_str))
        })
    }

    /// 智能识别代币类型并获取余额
    ///
    /// 智能化逻辑：
    /// 1. 如果 token_address 为 None → 查询 ETH
    /// 2. 如果 token_address 是 ETH 标识符 → 查询 ETH
    /// 3. 如果 token_address 是代币符号 → 从注册表查找地址并查询 ERC20
    /// 4. 如果 token_address 是合约地址 → 查询对应的 ERC20
    pub async fn get_balance(&self, request: BalanceRequest) -> Result<BalanceResponse> {
        debug!(
            "正在获取地址的余额: {} (代币: {:?})",
            request.address, request.token_address
        );

        // 验证钱包地址
        let wallet_address = Self::validate_address(&request.address)?;

        // 智能识别代币类型
        let token_info = self.resolve_token_info(request.token_address).await?;

        debug!("解析代币信息: {:?}", token_info);

        // 根据解析结果调用相应的方法
        match token_info.is_eth {
            true => self.get_eth_balance(wallet_address).await,
            false => {
                self.get_erc20_balance(wallet_address, &token_info.address, &token_info.symbol)
                    .await
            }
        }
    }

    /// 智能解析代币信息
    async fn resolve_token_info(&self, token_address: Option<String>) -> Result<TokenInfo> {
        match token_address {
            None => {
                // 未提供 token_address，默认查询 ETH
                Ok(TokenInfo {
                    address: ETH_IDENTIFIER.to_string(),
                    symbol: "ETH".to_string(),
                    is_eth: true,
                })
            }
            Some(token_id) => {
                let token_id_upper = token_id.to_uppercase();

                // 检查是否为 ETH 特殊标识符
                if token_id_upper == ETH_IDENTIFIER.to_uppercase() || token_id_upper == "ETH" {
                    return Ok(TokenInfo {
                        address: ETH_IDENTIFIER.to_string(),
                        symbol: "ETH".to_string(),
                        is_eth: true,
                    });
                }

                // 尝试解析为地址
                if let Ok(address) = token_id.parse::<Address>() {
                    // 是有效地址，判断是否为 ETH 标识符
                    if address.to_string().to_uppercase() == ETH_IDENTIFIER.to_uppercase() {
                        Ok(TokenInfo {
                            address: ETH_IDENTIFIER.to_string(),
                            symbol: "ETH".to_string(),
                            is_eth: true,
                        })
                    } else {
                        // 是合约地址，需要获取符号
                        let symbol = self
                            .rpc
                            .get_token_symbol(address)
                            .await
                            .unwrap_or_else(|_| "UNKNOWN".to_string());

                        Ok(TokenInfo {
                            address: address.to_string(),
                            symbol,
                            is_eth: false,
                        })
                    }
                } else {
                    // 不是地址，尝试作为符号查找
                    if let Some(address) = self.token_registry.symbol_to_address(&token_id_upper) {
                        Ok(TokenInfo {
                            address: address.to_string(),
                            symbol: token_id_upper,
                            is_eth: false,
                        })
                    } else {
                        warn!("未找到代币符号: {}", token_id);
                        Err(crate::error::EthereumError::TokenNotFound(format!(
                            "未找到代币: {}",
                            token_id
                        )))
                    }
                }
            }
        }
    }

    /// 获取 ETH 余额
    async fn get_eth_balance(&self, address: Address) -> Result<BalanceResponse> {
        info!("正在获取 ETH 余额: {:?}", address);

        let raw_balance = self.rpc.get_eth_balance(address).await?;
        let balance = precision::to_decimal(raw_balance, 18)?;

        Ok(BalanceResponse {
            address: address.to_string(),
            balance: balance.normalize().to_string(),
            decimals: 18,
            raw: raw_balance.to_string(),
            token_type: "ETH".to_string(),
            token_address: ETH_IDENTIFIER.to_string(),
        })
    }

    /// 获取 ERC20 代币余额
    async fn get_erc20_balance(
        &self,
        wallet_address: Address,
        token_addr_str: &str,
        token_symbol: &str,
    ) -> Result<BalanceResponse> {
        info!(
            "正在获取 ERC20 余额: {:?} 在代币: {} ({})",
            wallet_address, token_addr_str, token_symbol
        );

        let token_address = Self::validate_address(token_addr_str)?;

        // 并行获取代币小数位数和余额
        let decimals = self.rpc.get_token_decimals(token_address).await?;
        let raw_balance = self
            .rpc
            .get_token_balance(token_address, wallet_address)
            .await?;

        // 转换为人类可读的格式
        let balance = precision::to_decimal(raw_balance, decimals)?;

        Ok(BalanceResponse {
            address: wallet_address.to_string(),
            balance: balance.normalize().to_string(),
            decimals,
            raw: raw_balance.to_string(),
            token_type: token_symbol.to_string(),
            token_address: token_address.to_string(),
        })
    }
}

#[derive(Debug)]
struct TokenInfo {
    address: String,
    symbol: String,
    is_eth: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_address_valid() {
        let addr = "0x1234567890123456789012345678901234567890";
        assert!(BalanceTool::validate_address(addr).is_ok());
    }

    #[test]
    fn test_validate_address_invalid() {
        let addr = "invalid_address";
        assert!(BalanceTool::validate_address(addr).is_err());
    }

    #[test]
    fn test_validate_address_lowercase() {
        let addr = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";
        assert!(BalanceTool::validate_address(addr).is_ok());
    }

    #[test]
    fn test_balance_request_serialization() {
        // 测试不带 token_address 的请求
        let request = BalanceRequest {
            address: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(),
            token_address: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"));

        // 测试带 token_address 的请求
        let request_with_token = BalanceRequest {
            address: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(),
            token_address: Some("USDT".to_string()),
        };
        let json = serde_json::to_string(&request_with_token).unwrap();
        assert!(json.contains("USDT"));
    }

    #[test]
    fn test_balance_response_with_token_address() {
        let response = BalanceResponse {
            address: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(),
            balance: "100.5".to_string(),
            decimals: 6,
            raw: "100500000".to_string(),
            token_type: "USDT".to_string(),
            token_address: "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("USDT"));
        assert!(json.contains("0xdAC17F958D2ee523a2206206994597C13D831ec7"));
        assert!(json.contains("100.5"));
    }

    #[test]
    fn test_eth_identifier_constant() {
        // 确保 ETH 标识符是正确的格式
        assert!(ETH_IDENTIFIER.starts_with("0x"));
        assert_eq!(ETH_IDENTIFIER.len(), 42);
    }

    #[tokio::test]
    async fn test_resolve_token_info_none() {
        let rpc = RpcClient::new("https://eth.llamarpc.com".to_string())
            .await
            .unwrap();
        let tool = BalanceTool::new(rpc);

        let token_info = tool.resolve_token_info(None).await.unwrap();
        assert!(token_info.is_eth);
        assert_eq!(token_info.symbol, "ETH");
    }

    #[tokio::test]
    async fn test_resolve_token_info_eth_symbol() {
        let rpc = RpcClient::new("https://eth.llamarpc.com".to_string())
            .await
            .unwrap();
        let tool = BalanceTool::new(rpc);

        let token_info = tool
            .resolve_token_info(Some("ETH".to_string()))
            .await
            .unwrap();
        assert!(token_info.is_eth);
        assert_eq!(token_info.symbol, "ETH");
    }

    #[tokio::test]
    async fn test_resolve_token_info_eth_identifier() {
        let rpc = RpcClient::new("https://eth.llamarpc.com".to_string())
            .await
            .unwrap();
        let tool = BalanceTool::new(rpc);

        let token_info = tool
            .resolve_token_info(Some(ETH_IDENTIFIER.to_string()))
            .await
            .unwrap();
        assert!(token_info.is_eth);
        assert_eq!(token_info.symbol, "ETH");
    }

    #[tokio::test]
    async fn test_resolve_token_info_usdt_symbol() {
        let rpc = RpcClient::new("https://eth.llamarpc.com".to_string())
            .await
            .unwrap();
        let tool = BalanceTool::new(rpc);

        let token_info = tool
            .resolve_token_info(Some("USDT".to_string()))
            .await
            .unwrap();
        assert!(!token_info.is_eth);
        assert_eq!(token_info.symbol, "USDT");
        assert_eq!(
            token_info.address.to_uppercase(),
            "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_uppercase()
        );
    }
}
