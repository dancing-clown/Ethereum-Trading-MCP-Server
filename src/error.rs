use thiserror::Error;

#[derive(Error, Debug)]
pub enum EthereumError {
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("RPC error: {0}")]
    RpcError(String),

    #[error("Balance insufficient: required {required}, available {available}")]
    InsufficientBalance { required: String, available: String },

    #[error("Invalid ERC20 contract: {0}")]
    InvalidERC20(String),

    #[error("Token not found: {0}")]
    TokenNotFound(String),

    #[error("Price oracle error: {0}")]
    PriceOracleError(String),

    #[error("Swap simulation failed: {0}")]
    SwapSimulationFailed(String),

    #[error("Gas estimation failed: {0}")]
    GasEstimationFailed(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Precision conversion error: {0}")]
    PrecisionError(String),

    #[error("Invalid token pair: {0}")]
    InvalidTokenPair(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, EthereumError>;
