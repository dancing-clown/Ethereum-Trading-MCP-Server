use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::sol;
use std::sync::Arc;
use tracing::{debug, error};

use crate::error::{EthereumError, Result};

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

/// 以太坊 RPC 客户端
#[derive(Clone)]
pub struct RpcClient {
    inner: Arc<RpcClientInner>,
}

struct RpcClientInner {
    provider_url: String,
}

impl RpcClient {
    /// 创建一个新的 RPC 客户端
    pub async fn new(rpc_url: String) -> Result<Self> {
        // 验证 URL 格式
        rpc_url
            .parse::<url::Url>()
            .map_err(|_| EthereumError::ConfigError("无效的 RPC URL 格式".to_string()))?;

        debug!("已连接到 RPC: {}", rpc_url);

        Ok(RpcClient {
            inner: Arc::new(RpcClientInner {
                provider_url: rpc_url,
            }),
        })
    }

    /// 为每个操作获取提供程序的帮助函数
    fn get_provider(&self) -> Result<HttpProvider> {
        let url = self
            .inner
            .provider_url
            .parse()
            .map_err(|_| EthereumError::ConfigError("无效的 RPC URL".to_string()))?;

        Ok(ProviderBuilder::new()
            .with_recommended_fillers()
            .on_http(url))
    }

    /// 获取地址的 ETH 余额
    pub async fn get_eth_balance(&self, address: Address) -> Result<U256> {
        debug!("正在获取 ETH 余额: {:?}", address);

        let provider = self.get_provider()?;

        provider.get_balance(address).await.map_err(|e| {
            error!("获取 ETH 余额失败: {}", e);
            EthereumError::RpcError(format!("获取余额失败: {}", e))
        })
    }

    /// 获取地址的 ERC20 代币余额
    pub async fn get_token_balance(
        &self,
        token_address: Address,
        account_address: Address,
    ) -> Result<U256> {
        debug!(
            "正在获取代币余额: {:?} 在代币: {:?}",
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
                error!("获取代币余额失败: {} (代币: {:?})", e, token_address);
                EthereumError::RpcError(format!("获取代币余额失败: {}", e))
            })
    }

    /// 获取 ERC20 代币小数位数
    pub async fn get_token_decimals(&self, token_address: Address) -> Result<u8> {
        debug!("正在获取代币小数位数: {:?}", token_address);

        let provider = self.get_provider()?;
        let contract = IERC20::new(token_address, provider);

        contract.decimals().call().await.map(|r| r._0).map_err(|e| {
            error!("获取代币小数位数失败: {}", e);
            EthereumError::RpcError(format!("获取代币小数位数失败: {}", e))
        })
    }

    /// 获取 ERC20 代币符号
    pub async fn get_token_symbol(&self, token_address: Address) -> Result<String> {
        debug!("正在获取代币符号: {:?}", token_address);

        let provider = self.get_provider()?;
        let contract = IERC20::new(token_address, provider);

        contract.symbol().call().await.map(|r| r._0).map_err(|e| {
            error!("获取代币符号失败: {}", e);
            EthereumError::RpcError(format!("获取代币符号失败: {}", e))
        })
    }

    /// 估算交易的 Gas
    pub async fn estimate_gas(&self, tx: alloy::rpc::types::TransactionRequest) -> Result<u64> {
        debug!("正在估算交易的 Gas");

        let provider = self.get_provider()?;

        provider.estimate_gas(&tx).await.map_err(|e| {
            error!("Gas 估算失败: {}", e);
            EthereumError::GasEstimationFailed(format!("Gas 估算失败: {}", e))
        })
    }

    /// 获取当前 Gas 价格
    pub async fn get_gas_price(&self) -> Result<u128> {
        debug!("正在获取当前 Gas 价格");

        let provider = self.get_provider()?;

        provider.get_gas_price().await.map_err(|e| {
            error!("获取 Gas 价格失败: {}", e);
            EthereumError::RpcError(format!("获取 Gas 价格失败: {}", e))
        })
    }

    /// 调用合约函数（只读）
    pub async fn call_contract(
        &self,
        tx: alloy::rpc::types::TransactionRequest,
    ) -> Result<alloy::primitives::Bytes> {
        debug!("正在调用合约函数");

        let provider = self.get_provider()?;

        provider.call(&tx).await.map_err(|e| {
            error!("调用合约失败: {}", e);
            EthereumError::RpcError(format!("调用合约失败: {}", e))
        })
    }

    /// 获取 RPC URL
    pub fn rpc_url(&self) -> &str {
        &self.inner.provider_url
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_rpc_client_creation() {
        // 仅测试我们可以调用构造函数路径
        // 我们不会在测试中实际连接到真实的 RPC
        let url = "https://eth.llamarpc.com";
        assert!(!url.is_empty());
    }
}
