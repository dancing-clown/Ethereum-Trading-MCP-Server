# 以太坊交易 MCP 服务器

一个用 Rust 编写的生产级 Model Context Protocol (MCP) 服务器，使 AI 代理能够安全地查询以太坊余额、获取代币价格和模拟 Uniswap 代币交换（无需向区块链广播交易）。

## 功能特性

- **get_balance**: 查询任何以太坊地址的 ETH 或 ERC20 代币余额
- **get_token_price**: 获取当前代币在 USD 和 ETH 中的价格
- **swap_tokens**: 使用滑点计算模拟 Uniswap 代币交换（100% 安全 - 无实际交易）
- **精度优先**: 使用 `rust_decimal` 进行准确的十进制运算（对加密货币至关重要）
- **结构化日志**: 使用 `tracing` 记录所有操作，便于调试和监控
- **JSON-RPC 2.0 兼容**: 标准的工具通信协议
- **异步/等待**: 使用 Tokio 实现全异步实现，支持高并发

## 快速开始

### 前置条件

- **Rust 1.70+** ([安装](https://rustup.rs/))
- **以太坊 RPC 端点** (免费选项: [llamarpc.com](https://llamarpc.com), [Ankr](https://www.ankr.com/), 或付费: [Infura](https://infura.io), [Alchemy](https://www.alchemy.com))

### 安装和设置

1. **克隆和构建**:
   ```bash
   git clone https://github.com/your-repo/ethereum-trading-mcp-server.git
   cd ethereum-trading-mcp-server
   cargo build --release
   ```

2. **配置环境**:
   ```bash
   cp .env.example .env
   # 编辑 .env 并设置你的 RPC_URL
   # 示例（使用公共端点）:
   # RPC_URL=https://eth.llamarpc.com
   ```

3. **运行服务器**:
   ```bash
   cargo run --release
   # 服务器将在 127.0.0.1:8080 启动
   ```

4. **运行测试**:
   ```bash
   cargo test
   # 所有 20 个单元测试通过
   ```

## 架构

```
src/
├── main.rs              # TCP 服务器入口点，处理 JSON-RPC 消息
├── lib.rs               # 模块导出
├── config.rs            # 环境变量配置
├── error.rs             # 带有上下文的错误类型
├── precision.rs         # 加密货币金额的十进制运算
├── tokens.rs            # 代币符号 ↔ 地址映射注册表
├── rpc/
│   └── client.rs        # 使用 Alloy 的以太坊 RPC 客户端
├── tools/
│   ├── mod.rs
│   ├── balance.rs       # get_balance 工具实现
│   ├── price.rs         # get_token_price 工具实现
│   └── swap.rs          # swap_tokens 工具实现
└── server/
    └── mcp.rs           # MCP 协议服务器（JSON-RPC 2.0）
```

## API 示例

### 工具 1: get_balance

查询钱包的 ETH 或 ERC20 代币余额。

**请求** (JSON-RPC):

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_balance",
    "arguments": {
      "address": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
      "token_address": null
    }
  },
  "id": 1
}
```

**响应**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "address": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
    "balance": "5.123456789012345678",
    "decimals": 18,
    "raw": "5123456789012345678",
    "token_type": "ETH"
  },
  "id": 1
}
```

**查询 USDT 余额**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_balance",
    "arguments": {
      "address": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
      "token_address": "0xdAC17F958D2ee523a2206206994597C13D831ec7"
    }
  },
  "id": 2
}
```

### 工具 2: get_token_price

获取当前代币在 USD 和 ETH 中的价格。

**请求**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_token_price",
    "arguments": {
      "token_identifier": "USDT"
    }
  },
  "id": 3
}
```

**响应**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "token": "USDT",
    "price_usd": "1.002",
    "price_eth": "0.0004",
    "timestamp": 1735689600,
    "data_source": "Mock Oracle"
  },
  "id": 3
}
```

### 工具 3: swap_tokens

使用滑点保护模拟代币交换。

**请求**（用 0.5% 滑点交换 1 ETH 换 USDC）:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "swap_tokens",
    "arguments": {
      "from_token": "ETH",
      "to_token": "USDC",
      "amount": "1",
      "slippage": 0.5,
      "wallet_address": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
    }
  },
  "id": 4
}
```

**响应**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "from_token": "ETH",
    "to_token": "USDC",
    "input_amount": "1",
    "estimated_output": "2475",
    "min_output": "2462.0625",
    "gas_cost_eth": "0.003",
    "slippage_percentage": "0.5",
    "simulation_success": true,
    "error": null
  },
  "id": 4
}
```

## 设计决策

1. **选择 Alloy 而非 ethers-rs**: Alloy 是 ethers-rs 的现代继任者，具有：
   - U256 操作快 ~60%
   - ABI 编码快 ~10 倍（通过 `sol!` 宏）
   - 更好的异步/等待模式
   - 持续的维护和开发

2. **通过 rust_decimal 实现精度**: 加密货币需要精确的十进制运算：
   - 避免浮点舍入错误
   - 所有金额通过 `decimals` 字段转换
   - 安全的往返转换（wei ↔ 可读格式）

3. **模块化架构**: 清晰的分离：
   - `rpc/`: RPC 操作（与工具隔离）
   - `tools/`: 业务逻辑（get_balance、定价、交换）
   - `server/`: MCP 协议（请求/响应处理）
   - `tokens/`: 代币注册表（符号 ↔ 地址查询）

4. **通过 eth_call 实现安全模拟**:
   - 交换交易仅通过 `eth_call`（只读）模拟
   - 无签名或广播
   - 测试交易时零资产风险

5. **使用 Tracing 进行结构化日志**:
   - 所有 RPC 调用、错误和操作都被记录
   - 帮助调试生产问题
   - 可与日志服务集成（ELK、DataDog 等）

## 已知限制和假设

1. **价格数据**: 目前使用模拟/硬编码价格用于演示
   - 生产环境: 集成 Uniswap 子图或 CoinGecko API
   - 考虑速率限制和缓存策略

2. **交换模拟**: 简化的模拟实现
   - 实际实现: 解码 Uniswap 池状态、应用公式、估算 Gas
   - 当前版本: ~1% 滑点 + 固定 Gas 估算
   - 完整的 Uniswap V2 模拟需要池储备数据

3. **仅 ERC20**:
   - 支持 ETH（0xEeee...）和 ERC20 代币
   - 不支持 NFT、ERC1155 或其他标准

4. **单一网络**:
   - 配置用于主网（chain_id=1）
   - 多链支持需要动态提供程序选择

5. **模拟代币注册表**:
   - 仅预映射 ~10 个常见代币
   - 通过 `register()` 方法或外部源添加更多

## 测试

```bash
# 运行所有测试
cargo test

# 测试精度转换
cargo test precision::tests

# 测试代币注册表
cargo test tokens::tests

# 测试地址验证
cargo test balance::tests::test_validate

# 运行日志输出
RUST_LOG=debug cargo run
```

## 性能特征

- **余额查询**: ~200-400ms（取决于网络）
- **价格获取**: ~100-200ms（模拟预言机，真实 API 较慢）
- **交换模拟**: ~300-600ms（包括 Gas 估算）
- **并发请求**: 完整的异步支持（tokio）

## 安全考虑

✅ **安全的内容**:
- 读操作（余额、价格）不需要私钥
- 交换模拟是通过 eth_call **只读**
- 所有 RPC 调用都经过验证和错误处理
- 十进制精度防止算术漏洞

⚠️ **需要谨慎处理**:
- 私钥: 使用环境变量，永不硬编码
- RPC 端点: 使用 HTTPS，考虑速率限制
- 输入验证: 地址格式和金额解析已验证

## 故障排除

**"RPC_URL not set"**:

```bash
export RPC_URL=https://eth.llamarpc.com
cargo run
```

**"Connection refused"**（端口 8080 正在使用:

```bash
# 编辑 src/main.rs，将 "127.0.0.1:8080" 更改为 "127.0.0.1:8081"
```

**"Invalid address" 错误**:

- 确保完整的 42 字符地址（0x + 40 个十六进制字符）
- 未强制校验和验证（两者都有效）

**"Token not found"**:

- 检查 src/tokens.rs 中的 TokenRegistry
- 通过 .register() 方法添加缺失的代币

## 为生产环境构建

```bash
# 优化的发布版本
cargo build --release

# 使用生产日志运行
RUST_LOG=info cargo run --release

# 二进制文件位置
./target/release/ethereum-mcp-server
```

## 贡献

欢迎贡献！改进的领域：

- 真实的 Uniswap V2/V3 模拟
- 多链支持
- WebSocket 提供程序支持
- 交易签名（带有安全密钥管理）

## 许可证

MIT 许可证 - 详见 LICENSE 文件

## 参考资料

- [Alloy 文档](https://docs.rs/alloy/)
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [Ethereum JSON-RPC 规范](https://ethereum.org/en/developers/docs/apis/json-rpc/)
- [ERC20 标准](https://eips.ethereum.org/EIPS/eip-20)
- [Uniswap V2 文档](https://docs.uniswap.org/contracts/v2/overview)
