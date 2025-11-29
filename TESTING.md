# 测试工具使用指南

本项目提供了一个交互式的 MCP 客户端工具，用于测试 Ethereum Trading MCP Server 的所有功能。

## 快速开始

### 1. 启动服务器

在一个终端中启动 MCP 服务器：

```bash
cargo run --release
# 或使用调试模式：
RUST_LOG=debug cargo run
```

服务器将在 `127.0.0.1:8080` 上运行。

### 2. 在另一个终端启动客户端

```bash
cargo run --release --bin mcp-client
```

你会看到类似以下的输出：

```bash
╔═══════════════════════════════════════════════════════╗
║   Ethereum Trading MCP Server - Test Client v1.0     ║
╚═══════════════════════════════════════════════════════╝

Connecting to server at 127.0.0.1:8080...
✓ Connected successfully!
```

## 可用命令

### 1. get_balance - 查询钱包余额

用于查询 ETH 或 ERC20 代币的余额。

**操作步骤：**

1. 选择命令 `1`
2. 输入以太坊地址（0x...）
3. 输入代币地址（可选，按Enter使用ETH）

**示例：**

```bash
Enter Ethereum address (0x...): 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045
Enter token address (press Enter for ETH):
```

**预期响应：**

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

### 2. get_token_price - 查询代币价格

获取代币的当前 USD 和 ETH 价格。

**操作步骤：**

1. 选择命令 `2`
2. 输入代币符号（如 ETH, USDC, USDT）或地址

**示例：**

```bash
Enter token symbol or address (e.g., ETH, USDC): ETH
```

**预期响应：**

```json
{
  "jsonrpc": "2.0",
  "result": {
    "price": "1.00",
    "timestamp": 1735689600,
  },
  "id": 2
}
```

### 3. swap_tokens - 模拟代币交换

模拟 Uniswap 上的代币交换（不会执行实际交易）。

**操作步骤：**

1. 选择命令 `3`
2. 输入源代币（如 ETH）
3. 输入目标代币（如 USDC）
4. 输入交换数量
5. 输入滑点容差（百分比，如 0.5 表示 0.5%）
6. 输入钱包地址

**示例：**
```
Enter source token (e.g., ETH): ETH
Enter destination token (e.g., USDC): USDC
Enter amount to swap: 1
Enter slippage tolerance (e.g., 0.5 for 0.5%): 0.5
Enter wallet address (0x...): 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045
```

**预期响应：**

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
  "id": 3
}
```

### 4. tools/list - 列出可用工具

列出服务器支持的所有工具及其描述。

**操作步骤：**

1. 选择命令 `4`

**预期响应：**

```json
{
  "jsonrpc": "2.0",
  "result": [
    {
      "name": "get_balance",
      "description": "Get ETH or ERC20 token balance for a wallet address",
      "input_schema": {...}
    },
    {
      "name": "get_token_price",
      "description": "Get current price of a token in USD and ETH",
      "input_schema": {...}
    },
    {
      "name": "swap_tokens",
      "description": "Simulate a token swap on Uniswap (no actual transaction executed)",
      "input_schema": {...}
    }
  ],
  "id": 4
}
```

### 5. exit - 退出客户端

关闭客户端连接。

## 测试场景

### 场景 1: 查询知名钱包的 ETH 余额

```bash
命令: 1
地址: 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045 (Vitalik Buterin)
代币: (空)
```

### 场景 2: 查询 USDT 代币价格

```bash
命令: 2
代币: USDT
```

### 场景 3: 模拟 ETH 到 USDC 的交换

```bash
命令: 3
源代币: ETH
目标代币: USDC
数量: 10
滑点: 1.0
钱包: 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045
```

## 故障排除

### 连接被拒绝

如果出现 "Connection refused" 错误：

1. 确保服务器已启动（在另一个终端运行 `cargo run --release`）
2. 检查服务器是否监听在 `127.0.0.1:8080`
3. 如果端口被占用，修改 `src/main.rs` 中的地址

### 地址格式错误

- 以太坊地址必须是 42 个字符（0x + 40 个十六进制字符）
- 地址可以是任何大小写混合的形式（不需要校验和）

### 代币未找到

如果代币未被识别：

1. 确保代币符号正确（如 ETH、USDC、USDT）
2. 或者提供完整的合约地址
3. 检查 `src/tokens.rs` 中的代币注册表

## 客户端二进制位置

编译后，客户端二进制文件位于：

```bash
./target/release/mcp-client
```

你也可以直接运行它：

```bash
./target/release/mcp-client
```

## 与 curl 测试

如果不想使用交互式客户端，你也可以用 `curl` 直接测试：

### 查询余额

```bash
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"get_balance","arguments":{"address":"0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045","token_address":null}},"id":1}' | nc localhost 8080
```

### 查询价格

```bash
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"get_token_price","arguments":{"token_identifier":"ETH"}},"id":2}' | nc localhost 8080
```

### 模拟交换

```bash
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"swap_tokens","arguments":{"from_token":"ETH","to_token":"USDC","amount":"1","slippage":0.5,"wallet_address":"0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"}},"id":3}' | nc localhost 8080
```

## 测试清单

- [ ] 客户端能够连接到服务器
- [ ] get_balance 返回有效的 ETH 余额
- [ ] get_balance 能够查询 ERC20 代币余额
- [ ] get_token_price 返回有效的价格信息
- [ ] swap_tokens 返回有效的交换模拟结果
- [ ] tools/list 返回所有工具列表
- [ ] 客户端能够优雅地处理错误输入
- [ ] 多个请求序列能够正确处理

## 下一步

- 尝试用真实的以太坊地址进行测试
- 测试无效地址的错误处理
- 尝试不同的代币组合
- 在调试模式下查看服务器日志
