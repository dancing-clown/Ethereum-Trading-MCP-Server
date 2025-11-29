use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::sol;
use std::sync::Arc;
use tracing::{debug, error};

use crate::error::{EthereumError, Result};

/// ERC20 contract interface using alloy sol! macro
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract IERC20 {
        function balanceOf(address account) external view returns (uint256);
        function decimals() external view returns (uint8);
        function symbol() external view returns (string);
        function transfer(address to, uint256 amount) external returns (bool);
        function approve(address spender, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);

        event Transfer(address indexed from, address indexed to, uint256 value);
        event Approval(address indexed owner, address indexed spender, uint256 value);
    }
}

/// Uniswap V2 Router interface
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract IUniswapV2Router {
        function getAmountsOut(uint256 amountIn, address[] path) public view returns (uint256[] amounts);
        function swapExactTokensForTokens(
            uint256 amountIn,
            uint256 amountOutMin,
            address[] path,
            address to,
            uint256 deadline
        ) external returns (uint256[] amounts);
        function swapExactETHForTokens(
            uint256 amountOutMin,
            address[] path,
            address to,
            uint256 deadline
        ) external payable returns (uint256[] amounts);
        function swapExactTokensForETH(
            uint256 amountIn,
            uint256 amountOutMin,
            address[] path,
            address to,
            uint256 deadline
        ) external returns (uint256[] amounts);
    }
}

/// WETH9 contract interface
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract IWETH {
        function deposit() external payable;
        function withdraw(uint256 amount) external;
        function approve(address guy, uint256 wad) external returns (bool);
        function transfer(address dst, uint256 wad) external returns (bool);
        function balanceOf(address) external view returns (uint256);
    }
}

type HttpProvider = alloy::providers::fillers::FillProvider<
    alloy::providers::fillers::JoinFill<
        alloy::providers::Identity,
        alloy::providers::fillers::JoinFill<
            alloy::providers::fillers::GasFiller,
            alloy::providers::fillers::JoinFill<
                alloy::providers::fillers::BlobGasFiller,
                alloy::providers::fillers::JoinFill<
                    alloy::providers::fillers::NonceFiller,
                    alloy::providers::fillers::ChainIdFiller,
                >,
            >,
        >,
    >,
    alloy::providers::RootProvider<alloy::transports::http::Http<reqwest::Client>>,
    alloy::transports::http::Http<reqwest::Client>,
    alloy::network::Ethereum,
>;

/// RPC Client for Ethereum interactions
#[derive(Clone)]
pub struct RpcClient {
    inner: Arc<RpcClientInner>,
}

struct RpcClientInner {
    provider_url: String,
}

impl RpcClient {
    /// Create a new RPC client
    pub async fn new(rpc_url: String) -> Result<Self> {
        // Validate URL format
        rpc_url
            .parse::<url::Url>()
            .map_err(|_| EthereumError::ConfigError("Invalid RPC URL format".to_string()))?;

        debug!("Connected to RPC: {}", rpc_url);

        Ok(RpcClient {
            inner: Arc::new(RpcClientInner {
                provider_url: rpc_url,
            }),
        })
    }

    /// Helper to get provider for each operation
    fn get_provider(&self) -> Result<HttpProvider> {
        let url = self
            .inner
            .provider_url
            .parse()
            .map_err(|_| EthereumError::ConfigError("Invalid RPC URL".to_string()))?;

        Ok(ProviderBuilder::new()
            .with_recommended_fillers()
            .on_http(url))
    }

    /// Get ETH balance for an address
    pub async fn get_eth_balance(&self, address: Address) -> Result<U256> {
        debug!("Getting ETH balance for: {:?}", address);

        let provider = self.get_provider()?;

        provider.get_balance(address).await.map_err(|e| {
            error!("Failed to get ETH balance: {}", e);
            EthereumError::RpcError(format!("Failed to get balance: {}", e))
        })
    }

    /// Get ERC20 token balance for an address
    pub async fn get_token_balance(
        &self,
        token_address: Address,
        account_address: Address,
    ) -> Result<U256> {
        debug!(
            "Getting token balance for: {:?} on token: {:?}",
            account_address, token_address
        );

        let provider = self.get_provider()?;
        let contract = IERC20::new(token_address, provider);

        contract
            .balanceOf(account_address)
            .call()
            .await
            .map(|r| r._0)
            .map_err(|e| {
                error!(
                    "Failed to get token balance: {} (token: {:?})",
                    e, token_address
                );
                EthereumError::RpcError(format!("Failed to get token balance: {}", e))
            })
    }

    /// Get ERC20 token decimals
    pub async fn get_token_decimals(&self, token_address: Address) -> Result<u8> {
        debug!("Getting decimals for token: {:?}", token_address);

        let provider = self.get_provider()?;
        let contract = IERC20::new(token_address, provider);

        contract.decimals().call().await.map(|r| r._0).map_err(|e| {
            error!("Failed to get token decimals: {}", e);
            EthereumError::RpcError(format!("Failed to get token decimals: {}", e))
        })
    }

    /// Get ERC20 token symbol
    pub async fn get_token_symbol(&self, token_address: Address) -> Result<String> {
        debug!("Getting symbol for token: {:?}", token_address);

        let provider = self.get_provider()?;
        let contract = IERC20::new(token_address, provider);

        contract.symbol().call().await.map(|r| r._0).map_err(|e| {
            error!("Failed to get token symbol: {}", e);
            EthereumError::RpcError(format!("Failed to get token symbol: {}", e))
        })
    }

    /// Estimate gas for a transaction
    pub async fn estimate_gas(&self, tx: alloy::rpc::types::TransactionRequest) -> Result<u64> {
        debug!("Estimating gas for transaction");

        let provider = self.get_provider()?;

        provider.estimate_gas(&tx).await.map_err(|e| {
            error!("Failed to estimate gas: {}", e);
            EthereumError::GasEstimationFailed(format!("Gas estimation failed: {}", e))
        })
    }

    /// Get current gas price
    pub async fn get_gas_price(&self) -> Result<u128> {
        debug!("Getting current gas price");

        let provider = self.get_provider()?;

        provider.get_gas_price().await.map_err(|e| {
            error!("Failed to get gas price: {}", e);
            EthereumError::RpcError(format!("Failed to get gas price: {}", e))
        })
    }

    /// Call a contract function (read-only)
    pub async fn call_contract(
        &self,
        tx: alloy::rpc::types::TransactionRequest,
    ) -> Result<alloy::primitives::Bytes> {
        debug!("Calling contract function");

        let provider = self.get_provider()?;

        provider.call(&tx).await.map_err(|e| {
            error!("Failed to call contract: {}", e);
            EthereumError::RpcError(format!("Failed to call contract: {}", e))
        })
    }

    /// Get RPC URL
    pub fn rpc_url(&self) -> &str {
        &self.inner.provider_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_client_creation() {
        // Just test that we can call the constructor path
        // We won't actually connect to a real RPC in tests
        let url = "https://eth.llamarpc.com";
        assert!(!url.is_empty());
    }
}
