pub mod balance;
pub mod price;
pub mod swap;

pub use balance::BalanceTool;
pub use price::PriceTool;
pub use swap::SwapTool;

use serde::{Deserialize, Serialize};

/// Standard tool request format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Standard tool response format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

impl ToolResponse {
    pub fn success(data: serde_json::Value) -> Self {
        ToolResponse {
            success: true,
            data,
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        ToolResponse {
            success: false,
            data: serde_json::json!({}),
            error: Some(message),
        }
    }
}
