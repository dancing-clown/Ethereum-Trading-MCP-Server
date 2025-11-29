use ethereum_trading_mcp_server::{Config, McpServer};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // 初始化追踪
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_line_number(true)
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("启动以太坊交易 MCP 服务器...");

    // 从环境加载配置
    let config = Config::from_env().unwrap_or_else(|_| {
        info!("使用默认配置（未找到 RPC_URL 环境变量）");
        Config::from_url("https://eth.llamarpc.com".to_string())
    });

    // 创建并初始化 MCP 服务器
    let mcp_server = Arc::new(McpServer::new(config));

    match mcp_server.initialize().await {
        Ok(_) => info!("MCP 服务器初始化成功"),
        Err(e) => {
            error!("MCP 服务器初始化失败: {}", e);
            return Err(e.into());
        }
    }

    // 启动 TCP 服务器
    let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let listener = TcpListener::bind(&addr).await?;

    info!("MCP 服务器监听 http://{}", addr);
    info!("可用工具: get_balance, get_token_price, swap_tokens");

    loop {
        let (socket, peer_addr) = listener.accept().await?;
        let mcp_server = Arc::clone(&mcp_server);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, mcp_server).await {
                error!("Error handling connection from {}: {}", peer_addr, e);
            }
        });
    }
}

async fn handle_connection(
    socket: tokio::net::TcpStream,
    mcp_server: Arc<McpServer>,
) -> eyre::Result<()> {
    let (reader, mut writer) = socket.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    while buf_reader.read_line(&mut line).await? > 0 {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        // 解析 JSON-RPC 请求
        match serde_json::from_str::<ethereum_trading_mcp_server::server::JsonRpcRequest>(trimmed) {
            Ok(request) => {
                info!("收到请求: {} (id: {:?})", request.method, request.id);

                let response = mcp_server.handle_request(request).await;

                let response_json = serde_json::to_string(&response)?;
                writer.write_all(response_json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
            Err(e) => {
                error!("无法解析 JSON-RPC 请求: {}", e);

                let error_response = json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32700,
                        "message": "Parse error",
                        "data": e.to_string()
                    },
                    "id": null
                });

                let response_json = serde_json::to_string(&error_response)?;
                writer.write_all(response_json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }

        line.clear();
    }

    Ok(())
}
