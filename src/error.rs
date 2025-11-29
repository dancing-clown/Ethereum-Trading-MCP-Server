use thiserror::Error;

#[derive(Error, Debug)]
pub enum EthereumError {
    #[error("无效地址: {0}")]
    InvalidAddress(String),

    #[error("无效数量: {0}")]
    InvalidAmount(String),

    #[error("RPC错误: {0}")]
    RpcError(String),

    #[error("余额不足: 需要 {required}，可用 {available}")]
    InsufficientBalance { required: String, available: String },

    #[error("无效ERC20合约: {0}")]
    InvalidERC20(String),

    #[error("代币未找到: {0}")]
    TokenNotFound(String),

    #[error("Price oracle error: {0}")]
    PriceOracleError(String),

    #[error("模拟交换失败: {0}")]
    SwapSimulationFailed(String),

    #[error("Gas模拟失败: {0}")]
    GasEstimationFailed(String),

    #[error("配置错误: {0}")]
    ConfigError(String),

    #[error("精度转换错误: {0}")]
    PrecisionError(String),

    #[error("无效代币对: {0}")]
    InvalidTokenPair(String),

    #[error("网络错误: {0}")]
    NetworkError(String),

    #[error("未知错误: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, EthereumError>;
