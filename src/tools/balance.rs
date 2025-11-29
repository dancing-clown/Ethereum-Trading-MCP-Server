use alloy::primitives::Address;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::Result;
use crate::precision;
use crate::rpc::RpcClient;
// use crate::tokens::TokenRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceRequest {
    pub address: String,
    #[serde(default)]
    pub token_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub address: String,
    pub balance: String,
    pub decimals: u8,
    pub raw: String,
    pub token_type: String,
}

pub struct BalanceTool {
    rpc: RpcClient,
    // token_registry: TokenRegistry,
}

impl BalanceTool {
    pub fn new(rpc: RpcClient) -> Self {
        BalanceTool {
            rpc,
            // token_registry: TokenRegistry::new(),
        }
    }

    /// 验证以太坊地址格式
    fn validate_address(addr_str: &str) -> Result<Address> {
        addr_str.parse::<Address>().map_err(|_| {
            crate::error::EthereumError::InvalidAddress(format!("无效的以太坊地址: {}", addr_str))
        })
    }

    /// 获取 ETH 或 ERC20 代币的余额
    pub async fn get_balance(&self, request: BalanceRequest) -> Result<BalanceResponse> {
        debug!("正在获取地址的余额: {}", request.address);

        // 验证钱包地址
        let wallet_address = Self::validate_address(&request.address)?;

        // 确定查询 ETH 还是 ERC20
        if let Some(token_addr_str) = &request.token_address {
            self.get_erc20_balance(wallet_address, token_addr_str).await
        } else {
            self.get_eth_balance(wallet_address).await
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
        })
    }

    /// 获取 ERC20 代币余额
    async fn get_erc20_balance(
        &self,
        wallet_address: Address,
        token_addr_str: &str,
    ) -> Result<BalanceResponse> {
        info!(
            "正在获取 ERC20 余额: {:?} 在代币: {}",
            wallet_address, token_addr_str
        );

        let token_address = Self::validate_address(token_addr_str)?;

        // 并行获取代币小数位数和符号
        let decimals = self.rpc.get_token_decimals(token_address).await?;
        let symbol = self
            .rpc
            .get_token_symbol(token_address)
            .await
            .unwrap_or_else(|_| "UNKNOWN".to_string());

        // 获取代币余额
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
            token_type: symbol,
        })
    }
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
}
