use alloy::primitives::{Address, U256};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::{EthereumError, Result};
use crate::precision;
use crate::rpc::RpcClient;
use crate::tokens::TokenRegistry;
use crate::tools::balance::BalanceTool;

const ETH_IDENTIFIER: &str = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";
const WETH_ADDRESS: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";

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

    /// 检查是否为 ETH 或 ETH 标识符
    fn is_eth(&self, identifier: &str) -> bool {
        let identifier_upper = identifier.to_uppercase();
        identifier_upper == "ETH" || identifier_upper == ETH_IDENTIFIER.to_uppercase()
    }

    /// 将 ETH 转换为 WETH 地址用于 Uniswap 交换
    fn eth_to_weth(&self, token_addr: Address) -> Result<Address> {
        if token_addr.to_string().to_uppercase() == ETH_IDENTIFIER.to_uppercase() {
            WETH_ADDRESS
                .parse::<Address>()
                .map_err(|_| EthereumError::ConfigError("无效的 WETH 地址".to_string()))
        } else {
            Ok(token_addr)
        }
    }

    /// 模拟代币交换（使用 Uniswap V2 真实数据）
    pub async fn simulate_swap(&self, request: SwapRequest) -> Result<SwapResponse> {
        info!(
            "模拟交换: {} {} -> {}",
            request.amount, request.from_token, request.to_token
        );

        // 验证地址
        let from_token_raw = self.resolve_token(&request.from_token)?;
        let to_token_raw = self.resolve_token(&request.to_token)?;

        // 检查是否为 ETH，用于后续小数位数判断
        let from_is_eth = self.is_eth(&request.from_token);
        let to_is_eth = self.is_eth(&request.to_token);

        // 将 ETH 转换为 WETH 用于 Uniswap 交换
        let from_token = self.eth_to_weth(from_token_raw)?;
        let to_token = self.eth_to_weth(to_token_raw)?;
        let wallet_address = request
            .wallet_address
            .parse::<Address>()
            .map_err(|_| EthereumError::InvalidAddress("无效的钱包地址".to_string()))?;

        // 验证金额
        let input_amount_decimal = match request.amount.parse::<Decimal>() {
            Ok(amt) => amt,
            Err(_) => {
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
        };

        // 检查钱包余额
        let balance_check = async {
            let bt = self
                .balance_tool
                .as_ref()
                .ok_or_else(|| EthereumError::Unknown("余额工具不可用".to_string()))?;

            let req = crate::tools::balance::BalanceRequest {
                address: wallet_address.to_string(),
                token_address: Some(from_token.to_string()),
            };
            info!("检查钱包余额请求: {:?}", req);
            bt.get_balance(req).await
        };

        match balance_check.await {
            Ok(balance) => {
                let wallet_balance = balance.balance.parse::<Decimal>().unwrap_or(Decimal::ZERO);

                if wallet_balance < input_amount_decimal {
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
                            wallet_balance, input_amount_decimal
                        )),
                    });
                }
            }
            Err(e) => {
                warn!("检查余额失败: {:?}", e);
                // 即使余额检查失败，也继续进行模拟
            }
        }

        // 获取代币小数位数
        // ETH/WETH 固定为 18 位小数，无需查询合约
        let from_decimals = if from_is_eth {
            18 // ETH/WETH 固定为 18 位小数
        } else {
            match self.rpc.get_token_decimals(from_token).await {
                Ok(d) => d,
                Err(e) => {
                    warn!("获取源代币小数位数失败: {}", e);
                    return Ok(SwapResponse {
                        from_token: request.from_token,
                        to_token: request.to_token,
                        input_amount: request.amount,
                        estimated_output: "0".to_string(),
                        min_output: "0".to_string(),
                        gas_cost_eth: "0".to_string(),
                        slippage_percentage: request.slippage.to_string(),
                        simulation_success: false,
                        error: Some(format!("无法获取源代币信息: {}", e)),
                    });
                }
            }
        };

        let to_decimals = if to_is_eth {
            18 // ETH/WETH 固定为 18 位小数
        } else {
            match self.rpc.get_token_decimals(to_token).await {
                Ok(d) => d,
                Err(e) => {
                    warn!("获取目标代币小数位数失败: {}", e);
                    return Ok(SwapResponse {
                        from_token: request.from_token,
                        to_token: request.to_token,
                        input_amount: request.amount,
                        estimated_output: "0".to_string(),
                        min_output: "0".to_string(),
                        gas_cost_eth: "0".to_string(),
                        slippage_percentage: request.slippage.to_string(),
                        simulation_success: false,
                        error: Some(format!("无法获取目标代币信息: {}", e)),
                    });
                }
            }
        };

        // 将输入金额转换为 U256（wei 格式）
        let amount_in_u256 = match precision::from_decimal(input_amount_decimal, from_decimals) {
            Ok(amt) => amt,
            Err(e) => {
                return Ok(SwapResponse {
                    from_token: request.from_token,
                    to_token: request.to_token,
                    input_amount: request.amount,
                    estimated_output: "0".to_string(),
                    min_output: "0".to_string(),
                    gas_cost_eth: "0".to_string(),
                    slippage_percentage: request.slippage.to_string(),
                    simulation_success: false,
                    error: Some(format!("金额转换失败: {}", e)),
                });
            }
        };

        // 构建交换路径
        let path = vec![from_token, to_token];

        // 从 Uniswap V2 Router 获取实际输出金额
        let amounts_out = match self.rpc.get_amounts_out(amount_in_u256, path.clone()).await {
            Ok(amounts) => amounts,
            Err(e) => {
                warn!("从 Uniswap 获取输出金额失败: {}", e);
                return Ok(SwapResponse {
                    from_token: request.from_token,
                    to_token: request.to_token,
                    input_amount: request.amount,
                    estimated_output: "0".to_string(),
                    min_output: "0".to_string(),
                    gas_cost_eth: "0".to_string(),
                    slippage_percentage: request.slippage.to_string(),
                    simulation_success: false,
                    error: Some(format!("无法从 Uniswap 获取价格: {}", e)),
                });
            }
        };

        // 获取输出金额（路径中的最后一个元素）
        if amounts_out.is_empty() {
            return Ok(SwapResponse {
                from_token: request.from_token,
                to_token: request.to_token,
                input_amount: request.amount,
                estimated_output: "0".to_string(),
                min_output: "0".to_string(),
                gas_cost_eth: "0".to_string(),
                slippage_percentage: request.slippage.to_string(),
                simulation_success: false,
                error: Some("Uniswap 返回空的输出金额".to_string()),
            });
        }

        let estimated_output_u256 = amounts_out[amounts_out.len() - 1];
        let estimated_output = match precision::to_decimal(estimated_output_u256, to_decimals) {
            Ok(amt) => amt,
            Err(e) => {
                return Ok(SwapResponse {
                    from_token: request.from_token,
                    to_token: request.to_token,
                    input_amount: request.amount,
                    estimated_output: "0".to_string(),
                    min_output: "0".to_string(),
                    gas_cost_eth: "0".to_string(),
                    slippage_percentage: request.slippage.to_string(),
                    simulation_success: false,
                    error: Some(format!("输出金额转换失败: {}", e)),
                });
            }
        };

        // 计算最小输出（应用滑点）
        let min_output =
            match precision::calculate_min_output_with_slippage(estimated_output, request.slippage)
            {
                Ok(amt) => amt,
                Err(e) => {
                    return Ok(SwapResponse {
                        from_token: request.from_token,
                        to_token: request.to_token,
                        input_amount: request.amount,
                        estimated_output: estimated_output.normalize().to_string(),
                        min_output: "0".to_string(),
                        gas_cost_eth: "0".to_string(),
                        slippage_percentage: request.slippage.to_string(),
                        simulation_success: false,
                        error: Some(format!("滑点计算失败: {}", e)),
                    });
                }
            };

        // 获取当前 Gas 价格
        let gas_price = self.rpc.get_gas_price().await.unwrap_or(20_000_000_000u128);

        // 估算 Gas（使用 eth_estimateGas）
        let min_output_u256 = match precision::from_decimal(min_output, to_decimals) {
            Ok(amt) => amt,
            Err(_) => U256::ZERO,
        };

        let deadline = U256::from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                + 300,
        ); // 5 分钟后过期

        let gas_estimate = match self
            .rpc
            .simulate_swap_exact_tokens_for_tokens(
                amount_in_u256,
                min_output_u256,
                path,
                wallet_address,
                deadline,
                wallet_address,
            )
            .await
        {
            Ok((_, gas)) => gas,
            Err(e) => {
                warn!("Gas 估算失败，使用默认值: {}", e);
                150_000u64 // 默认 Gas 估算
            }
        };

        let gas_cost_wei = U256::from(gas_estimate) * U256::from(gas_price);
        let gas_cost_eth = match precision::to_decimal(gas_cost_wei, 18) {
            Ok(cost) => cost,
            Err(_) => Decimal::ZERO,
        };

        info!(
            "交换模拟完成: {} {} -> {} (输出: {}, Gas: {})",
            input_amount_decimal,
            request.from_token,
            request.to_token,
            estimated_output,
            gas_estimate
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

    #[test]
    fn test_swap_request_validation() {
        let rpc = RpcClient::new("https://eth.llamarpc.com".to_string());
        futures::executor::block_on(async {
            let swap_tool = SwapTool::new(rpc.await.unwrap());

            // Test invalid amount format
            let request = SwapRequest {
                from_token: "ETH".to_string(),
                to_token: "USDC".to_string(),
                amount: "invalid".to_string(),
                slippage: Decimal::from_str_exact("0.5").unwrap(),
                wallet_address: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(),
            };

            let result = swap_tool.simulate_swap(request).await;
            assert!(result.is_ok());

            let response = result.unwrap();
            assert!(!response.simulation_success);
            assert!(response.error.as_ref().unwrap().contains("无效的金额格式"));
        });
    }

    #[test]
    fn test_swap_decimal_handling() {
        // Test decimal parsing for common swap amounts
        assert!("1.5".parse::<Decimal>().is_ok());
        assert!("0.001".parse::<Decimal>().is_ok());
        assert!("1000.25".parse::<Decimal>().is_ok());
        assert!(!Decimal::from_str_exact("invalid").is_ok());
    }

    #[test]
    fn test_slippage_calculation_in_swap() {
        let estimated_output = Decimal::from_str_exact("1000").unwrap();
        let slippage = Decimal::from_str_exact("0.5").unwrap(); // 0.5%

        // Expected: 1000 * (1 - 0.005) = 995
        let min_output =
            precision::calculate_min_output_with_slippage(estimated_output, slippage).unwrap();
        assert_eq!(min_output, Decimal::from_str_exact("995").unwrap());
    }
}
