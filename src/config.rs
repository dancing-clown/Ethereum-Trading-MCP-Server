use crate::error::{EthereumError, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub rpc_url: String,
    pub private_key: Option<String>,
    pub chain_id: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();

        let rpc_url = env::var("RPC_URL")
            .map_err(|_| EthereumError::ConfigError("RPC_URL not set".to_string()))?;

        let private_key = env::var("PRIVATE_KEY").ok();

        let chain_id = env::var("CHAIN_ID")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<u64>()
            .map_err(|e| EthereumError::ConfigError(format!("Invalid CHAIN_ID: {}", e)))?;

        Ok(Config {
            rpc_url,
            private_key,
            chain_id,
        })
    }

    pub fn from_url(rpc_url: String) -> Self {
        Config {
            rpc_url,
            private_key: None,
            chain_id: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_url() {
        let config = Config::from_url("https://eth.llamarpc.com".to_string());
        assert_eq!(config.rpc_url, "https://eth.llamarpc.com");
        assert_eq!(config.chain_id, 1);
    }
}
