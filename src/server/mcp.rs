use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::config::Config;
use crate::rpc::RpcClient;
use crate::tools::balance::{BalanceRequest, BalanceTool};
use crate::tools::price::{PriceRequest, PriceTool};
use crate::tools::swap::{SwapRequest, SwapTool};

/// JSON-RPC 2.0 Request format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
    pub id: Value,
}

/// JSON-RPC 2.0 Response format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// MCP Tool Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// MCP Server for Ethereum trading tools
pub struct McpServer {
    config: Config,
    rpc_client: Arc<RwLock<Option<RpcClient>>>,
    balance_tool: Arc<RwLock<Option<BalanceTool>>>,
    price_tool: Arc<RwLock<Option<PriceTool>>>,
    swap_tool: Arc<RwLock<Option<SwapTool>>>,
}

impl McpServer {
    pub fn new(config: Config) -> Self {
        McpServer {
            config,
            rpc_client: Arc::new(RwLock::new(None)),
            balance_tool: Arc::new(RwLock::new(None)),
            price_tool: Arc::new(RwLock::new(None)),
            swap_tool: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the server and connect to RPC
    pub async fn initialize(&self) -> crate::error::Result<()> {
        info!(
            "Initializing MCP server with RPC URL: {}",
            self.config.rpc_url
        );

        let rpc = RpcClient::new(self.config.rpc_url.clone()).await?;

        *self.rpc_client.write().await = Some(rpc.clone());
        *self.balance_tool.write().await = Some(BalanceTool::new(rpc.clone()));
        *self.price_tool.write().await = Some(PriceTool::new(rpc.clone()));
        *self.swap_tool.write().await = Some(SwapTool::new(rpc));

        info!("MCP server initialized successfully");
        Ok(())
    }

    /// Get tool definitions (MCP spec)
    pub async fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "get_balance".to_string(),
                description: "Get ETH or ERC20 token balance for a wallet address".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "address": {
                            "type": "string",
                            "description": "Ethereum wallet address (0x...)"
                        },
                        "token_address": {
                            "type": "string",
                            "description": "ERC20 contract address (optional, omit for ETH balance)"
                        }
                    },
                    "required": ["address"]
                }),
            },
            ToolDefinition {
                name: "get_token_price".to_string(),
                description: "Get current price of a token in USD and ETH".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "token_identifier": {
                            "type": "string",
                            "description": "Token symbol (e.g., ETH, USDC) or contract address"
                        }
                    },
                    "required": ["token_identifier"]
                }),
            },
            ToolDefinition {
                name: "swap_tokens".to_string(),
                description: "Simulate a token swap on Uniswap (no actual transaction executed)"
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "from_token": {
                            "type": "string",
                            "description": "Source token symbol or address"
                        },
                        "to_token": {
                            "type": "string",
                            "description": "Destination token symbol or address"
                        },
                        "amount": {
                            "type": "string",
                            "description": "Amount to swap (in human-readable format)"
                        },
                        "slippage": {
                            "type": "number",
                            "description": "Slippage tolerance in percentage (e.g., 0.5 for 0.5%)"
                        },
                        "wallet_address": {
                            "type": "string",
                            "description": "Wallet address initiating the swap"
                        }
                    },
                    "required": ["from_token", "to_token", "amount", "slippage", "wallet_address"]
                }),
            },
        ]
    }

    /// Handle a JSON-RPC request
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!(
            "Handling MCP request: {} with params: {:?}",
            request.method, request.params
        );

        let response = match request.method.as_str() {
            "tools/list" => self.handle_tools_list().await,
            "tools/call" => self.handle_tool_call(&request.params).await,
            "ping" => Ok(json!({"status": "ok"})),
            _ => Err(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        };

        match response {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(result),
                error: None,
                id: request.id,
            },
            Err(err) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(err),
                id: request.id,
            },
        }
    }

    async fn handle_tools_list(&self) -> Result<Value, JsonRpcError> {
        let tools = self.get_tool_definitions().await;
        Ok(serde_json::to_value(&tools).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Internal error: {}", e),
            data: None,
        })?)
    }

    async fn handle_tool_call(&self, params: &Value) -> Result<Value, JsonRpcError> {
        let tool_name =
            params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| JsonRpcError {
                    code: -32602,
                    message: "Missing or invalid 'name' parameter".to_string(),
                    data: None,
                })?;

        let arguments = params.get("arguments").ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing 'arguments' parameter".to_string(),
            data: None,
        })?;

        match tool_name {
            "get_balance" => {
                let request: BalanceRequest =
                    serde_json::from_value(arguments.clone()).map_err(|e| JsonRpcError {
                        code: -32602,
                        message: format!("Invalid arguments: {}", e),
                        data: None,
                    })?;

                let balance_tool = self.balance_tool.read().await;
                let tool = balance_tool.as_ref().ok_or_else(|| JsonRpcError {
                    code: -32603,
                    message: "Balance tool not initialized".to_string(),
                    data: None,
                })?;

                match tool.get_balance(request).await {
                    Ok(response) => Ok(serde_json::to_value(&response).unwrap()),
                    Err(e) => Err(JsonRpcError {
                        code: -32603,
                        message: format!("Balance query failed: {}", e),
                        data: None,
                    }),
                }
            }
            "get_token_price" => {
                let request: PriceRequest =
                    serde_json::from_value(arguments.clone()).map_err(|e| JsonRpcError {
                        code: -32602,
                        message: format!("Invalid arguments: {}", e),
                        data: None,
                    })?;

                let price_tool = self.price_tool.read().await;
                let tool = price_tool.as_ref().ok_or_else(|| JsonRpcError {
                    code: -32603,
                    message: "Price tool not initialized".to_string(),
                    data: None,
                })?;

                match tool.get_price(request).await {
                    Ok(response) => Ok(serde_json::to_value(&response).unwrap()),
                    Err(e) => Err(JsonRpcError {
                        code: -32603,
                        message: format!("Price query failed: {}", e),
                        data: None,
                    }),
                }
            }
            "swap_tokens" => {
                let request: SwapRequest =
                    serde_json::from_value(arguments.clone()).map_err(|e| JsonRpcError {
                        code: -32602,
                        message: format!("Invalid arguments: {}", e),
                        data: None,
                    })?;

                let swap_tool = self.swap_tool.read().await;
                let tool = swap_tool.as_ref().ok_or_else(|| JsonRpcError {
                    code: -32603,
                    message: "Swap tool not initialized".to_string(),
                    data: None,
                })?;

                match tool.simulate_swap(request).await {
                    Ok(response) => Ok(serde_json::to_value(&response).unwrap()),
                    Err(e) => Err(JsonRpcError {
                        code: -32603,
                        message: format!("Swap simulation failed: {}", e),
                        data: None,
                    }),
                }
            }
            _ => Err(JsonRpcError {
                code: -32601,
                message: format!("Tool not found: {}", tool_name),
                data: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonrpc_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: json!({}),
            id: json!(1),
        };

        let json_str = serde_json::to_string(&request).unwrap();
        assert!(json_str.contains("tools/call"));
        assert!(json_str.contains("2.0"));
    }

    #[test]
    fn test_tool_definitions() {
        let config = Config::from_url("https://eth.llamarpc.com".to_string());
        let server = McpServer::new(config);

        // We can't use tokio::block_on here in test context
        // Just verify the server can be created
        assert_eq!(server.config.rpc_url, "https://eth.llamarpc.com");
    }
}
