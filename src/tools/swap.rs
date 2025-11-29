use alloy::primitives::Address;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{EthereumError, Result};
use crate::precision;
use crate::rpc::RpcClient;
use crate::tokens::TokenRegistry;
use crate::tools::balance::BalanceTool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapRequest {
    pub from_token: String, // 符号或地址
    pub to_token: String,   // 符号或地址
    pub amount: String,     // 人类可读格式的金额
    pub slippage: Decimal,  // 滑点容差百分比（例如 0.5 表示 0.5%）
    pub wallet_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapResponse {
    pub from_token: String,
    pub to_token: String,
    pub input_amount: String,
    pub estimated_output: String,
    pub min_output: String,
    pub gas_cost_eth: String,
    pub slippage_percentage: String,
    pub simulation_success: bool,
    pub error: Option<String>,
}

pub struct SwapTool {
    rpc: RpcClient,
    token_registry: TokenRegistry,
    balance_tool: Option<BalanceTool>,
}

impl SwapTool {
    pub fn new(rpc: RpcClient) -> Self {
        let balance_tool = Some(BalanceTool::new(rpc.clone()));
        SwapTool {
            rpc,
            token_registry: TokenRegistry::new(),
            balance_tool,
        }
    }

    /// 验证并将代币标识符解析为地址
    fn resolve_token(&self, identifier: &str) -> Result<Address> {
        let identifier_upper = identifier.to_uppercase();

        // 首先尝试解析为地址
        if let Ok(addr) = identifier_upper.parse::<Address>() {
            return Ok(addr);
        }

        // 尝试作为符号查找
        self.token_registry
            .symbol_to_address(&identifier_upper)
            .ok_or_else(|| EthereumError::InvalidTokenPair(format!("无法解析代币: {}", identifier)))
    }

    /// 模拟代币交换
    pub async fn simulate_swap(&self, request: SwapRequest) -> Result<SwapResponse> {
        debug!(
            "模拟交换: {} {} -> {}",
            request.amount, request.from_token, request.to_token
        );

        // 验证地址
        let from_token = self.resolve_token(&request.from_token)?;
        // let to_token = self.resolve_token(&request.to_token)?;
        let wallet_address = request
            .wallet_address
            .parse::<Address>()
            .map_err(|_| EthereumError::InvalidAddress("无效的钱包地址".to_string()))?;

        // 验证金额
        if request.amount.parse::<Decimal>().is_err() {
            return Ok(SwapResponse {
                from_token: request.from_token,
                to_token: request.to_token,
                input_amount: request.amount,
                estimated_output: "0".to_string(),
                min_output: "0".to_string(),
                gas_cost_eth: "0".to_string(),
                slippage_percentage: request.slippage.to_string(),
                simulation_success: false,
                error: Some("无效的金额格式".to_string()),
            });
        }

        // 检查钱包余额
        match self
            .balance_tool
            .as_ref()
            .ok_or_else(|| EthereumError::Unknown("余额工具不可用".to_string()))
            .and_then(|bt| {
                futures::executor::block_on(async {
                    bt.get_balance(crate::tools::balance::BalanceRequest {
                        address: wallet_address.to_string(),
                        token_address: if from_token.to_string().to_uppercase()
                            == "0xEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE"
                        {
                            None
                        } else {
                            Some(from_token.to_string())
                        },
                    })
                    .await
                })
            }) {
            Ok(balance) => {
                let wallet_balance = balance.balance.parse::<Decimal>().unwrap_or(Decimal::ZERO);
                let input_amount = request.amount.parse::<Decimal>().unwrap_or(Decimal::ZERO);

                if wallet_balance < input_amount {
                    return Ok(SwapResponse {
                        from_token: request.from_token,
                        to_token: request.to_token,
                        input_amount: request.amount,
                        estimated_output: "0".to_string(),
                        min_output: "0".to_string(),
                        gas_cost_eth: "0".to_string(),
                        slippage_percentage: request.slippage.to_string(),
                        simulation_success: false,
                        error: Some(format!(
                            "余额不足: {} 可用, {} 需要",
                            wallet_balance, input_amount
                        )),
                    });
                }
            }
            Err(e) => {
                warn!("检查余额失败: {:?}", e);
                // 即使余额检查失败，也继续进行模拟
            }
        }

        // 模拟 Uniswap 交换
        // 在生产环境中，这将:
        // 1. 构建交换交易
        // 2. 调用 eth_call 进行模拟
        // 3. 解码返回值
        // 4. 估算 Gas

        let input_amount = request.amount.parse::<Decimal>().unwrap_or(Decimal::ZERO);

        // 模拟输出计算（简化: 由于流动性，速率提高 1-2%）
        let estimated_output = input_amount * Decimal::from_str_exact("0.99").unwrap();
        let min_output =
            precision::calculate_min_output_with_slippage(estimated_output, request.slippage)?;

        // 模拟 Gas 估算: ~150k gas 单位
        let gas_price = self.rpc.get_gas_price().await.unwrap_or(20_000_000_000u128);

        let estimated_gas = 150_000u64;
        let gas_cost_wei =
            alloy::primitives::U256::from(estimated_gas) * alloy::primitives::U256::from(gas_price);
        let gas_cost_eth = precision::to_decimal(gas_cost_wei, 18)?;

        info!(
            "交换模拟完成: {} {} -> {} (输出: {})",
            input_amount, request.from_token, request.to_token, estimated_output
        );

        Ok(SwapResponse {
            from_token: request.from_token,
            to_token: request.to_token,
            input_amount: request.amount,
            estimated_output: estimated_output.normalize().to_string(),
            min_output: min_output.normalize().to_string(),
            gas_cost_eth: gas_cost_eth.normalize().to_string(),
            slippage_percentage: request.slippage.to_string(),
            simulation_success: true,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_response_serialization() {
        let response = SwapResponse {
            from_token: "ETH".to_string(),
            to_token: "USDC".to_string(),
            input_amount: "1".to_string(),
            estimated_output: "2500".to_string(),
            min_output: "2487.5".to_string(),
            gas_cost_eth: "0.003".to_string(),
            slippage_percentage: "0.5".to_string(),
            simulation_success: true,
            error: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("ETH"));
        assert!(json.contains("USDC"));
        assert!(json.contains("2500"));
    }

    #[test]
    fn test_token_identifier_parsing() {
        let registry = TokenRegistry::new();
        let usdc = registry.symbol_to_address("USDC");
        assert!(usdc.is_some());
    }
}
