use alloy::primitives::Address;
use std::collections::HashMap;

/// 常见代币定义和映射
pub struct TokenRegistry {
    symbol_to_address: HashMap<String, Address>,
    address_to_symbol: HashMap<Address, String>,
}

impl TokenRegistry {
    /// 创建一个包含常见主网代币的新代币注册表
    pub fn new() -> Self {
        let mut symbol_to_address = HashMap::new();
        let mut address_to_symbol = HashMap::new();

        // 以太坊主网代币映射
        let tokens = vec![
            // 主网代币
            (
                "ETH".to_string(),
                "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
            ),
            (
                "WETH".to_string(),
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
            ),
            (
                "USDC".to_string(),
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
            ),
            (
                "USDT".to_string(),
                "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
            ),
            (
                "DAI".to_string(),
                "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string(),
            ),
            (
                "LINK".to_string(),
                "0x514910771AF9Ca656af840dff83E8264EcF986CA".to_string(),
            ),
            (
                "UNI".to_string(),
                "0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984".to_string(),
            ),
            (
                "AAVE".to_string(),
                "0x7Fc66500c84A76Ad7e9c93437E434122A1f9AcDd".to_string(),
            ),
            (
                "FRAX".to_string(),
                "0x853d955aCEf822Db058eb8505911ED77F175b999".to_string(),
            ),
        ];

        for (symbol, address_str) in tokens {
            if let Ok(address) = address_str.parse::<Address>() {
                symbol_to_address.insert(symbol.clone(), address);
                address_to_symbol.insert(address, symbol);
            }
        }

        TokenRegistry {
            symbol_to_address,
            address_to_symbol,
        }
    }

    /// 从符号获取地址
    pub fn symbol_to_address(&self, symbol: &str) -> Option<Address> {
        self.symbol_to_address.get(&symbol.to_uppercase()).copied()
    }

    /// 从地址获取符号
    pub fn address_to_symbol(&self, address: Address) -> Option<String> {
        self.address_to_symbol.get(&address).cloned()
    }

    /// 注册一个新代币
    pub fn register(&mut self, symbol: String, address: Address) {
        self.symbol_to_address.insert(symbol.clone(), address);
        self.address_to_symbol.insert(address, symbol);
    }

    /// 获取所有已注册的符号
    pub fn symbols(&self) -> Vec<String> {
        self.symbol_to_address.keys().cloned().collect()
    }
}

impl Default for TokenRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_registry_eth() {
        let registry = TokenRegistry::new();
        let eth_addr = registry.symbol_to_address("ETH");
        assert!(eth_addr.is_some());
    }

    #[test]
    fn test_token_registry_usdt() {
        let registry = TokenRegistry::new();
        let usdt_addr = registry.symbol_to_address("USDT");
        assert!(usdt_addr.is_some());
    }

    #[test]
    fn test_reverse_lookup() {
        let registry = TokenRegistry::new();
        if let Some(usdt_addr) = registry.symbol_to_address("USDT") {
            let symbol = registry.address_to_symbol(usdt_addr);
            assert_eq!(symbol, Some("USDT".to_string()));
        }
    }
}
