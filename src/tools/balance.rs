use alloy::primitives::Address;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info};

use crate::error::Result;
use crate::precision;
use crate::rpc::RpcClient;
use crate::tokens::TokenRegistry;

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
    token_registry: TokenRegistry,
}

impl BalanceTool {
    pub fn new(rpc: RpcClient) -> Self {
        BalanceTool {
            rpc,
            token_registry: TokenRegistry::new(),
        }
    }

    /// Validate Ethereum address format
    fn validate_address(addr_str: &str) -> Result<Address> {
        addr_str.parse::<Address>().map_err(|_| {
            crate::error::EthereumError::InvalidAddress(format!(
                "Invalid Ethereum address: {}",
                addr_str
            ))
        })
    }

    /// Get balance for ETH or ERC20 token
    pub async fn get_balance(&self, request: BalanceRequest) -> Result<BalanceResponse> {
        debug!("Getting balance for address: {}", request.address);

        // Validate the wallet address
        let wallet_address = Self::validate_address(&request.address)?;

        // Determine if querying ETH or ERC20
        if let Some(token_addr_str) = &request.token_address {
            self.get_erc20_balance(wallet_address, token_addr_str).await
        } else {
            self.get_eth_balance(wallet_address).await
        }
    }

    /// Get ETH balance
    async fn get_eth_balance(&self, address: Address) -> Result<BalanceResponse> {
        info!("Fetching ETH balance for: {:?}", address);

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

    /// Get ERC20 token balance
    async fn get_erc20_balance(
        &self,
        wallet_address: Address,
        token_addr_str: &str,
    ) -> Result<BalanceResponse> {
        info!(
            "Fetching ERC20 balance for: {:?} on token: {}",
            wallet_address, token_addr_str
        );

        let token_address = Self::validate_address(token_addr_str)?;

        // Get token decimals and symbol in parallel
        let decimals = self.rpc.get_token_decimals(token_address).await?;
        let symbol = self
            .rpc
            .get_token_symbol(token_address)
            .await
            .unwrap_or_else(|_| "UNKNOWN".to_string());

        // Get token balance
        let raw_balance = self
            .rpc
            .get_token_balance(token_address, wallet_address)
            .await?;

        // Convert to human-readable format
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
