use serde_json::{json, Value};
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

type Reader = BufReader<OwnedReadHalf>;
type Writer = OwnedWriteHalf;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    println!("╔═══════════════════════════════════════════════════════╗");
    println!("║   Ethereum Trading MCP Server - Test Client v1.0     ║");
    println!("╚═══════════════════════════════════════════════════════╝\n");

    // Connect to server
    let addr = "127.0.0.1:8080";
    println!("Connecting to server at {}...", addr);

    let socket = TcpStream::connect(addr).await?;
    let (reader, writer) = socket.into_split();
    let reader = BufReader::new(reader);

    println!("✓ Connected successfully!\n");

    let mut client = TestClient::new(reader, writer);

    loop {
        println!("\n╔═══════════════════════════════════════════════════════╗");
        println!("║ Available Commands:                                  ║");
        println!("║ 1. get_balance    - Query wallet balance            ║");
        println!("║ 2. get_token_price - Get token price in USD/ETH    ║");
        println!("║ 3. swap_tokens    - Simulate a token swap          ║");
        println!("║ 4. tools/list     - List available tools           ║");
        println!("║ 5. exit           - Close connection               ║");
        println!("╚═══════════════════════════════════════════════════════╝");
        print!("\nEnter command number (1-5): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim();

        match choice {
            "1" => {
                client.get_balance().await?;
            }
            "2" => {
                client.get_token_price().await?;
            }
            "3" => {
                client.swap_tokens().await?;
            }
            "4" => {
                client.list_tools().await?;
            }
            "5" => {
                println!("\nGoodbye!");
                break;
            }
            _ => println!("Invalid choice. Please enter 1-5."),
        }
    }

    Ok(())
}

struct TestClient {
    reader: Reader,
    writer: Writer,
    request_id: i32,
}

impl TestClient {
    fn new(reader: Reader, writer: Writer) -> Self {
        TestClient {
            reader,
            writer,
            request_id: 1,
        }
    }

    async fn send_request(&mut self, request: Value) -> eyre::Result<()> {
        let request_json = serde_json::to_string(&request)?;
        println!(
            "\n→ Sending request:\n{}",
            serde_json::to_string_pretty(&request)?
        );

        self.writer.write_all(request_json.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;

        // Read response
        let mut response_line = String::new();
        self.reader.read_line(&mut response_line).await?;

        if !response_line.is_empty() {
            println!("\n← Response received:");
            let response: Value = serde_json::from_str(&response_line)?;
            println!("{}", serde_json::to_string_pretty(&response)?);

            if let Some(error) = response.get("error") {
                if !error.is_null() {
                    println!(
                        "\n⚠️  Error: {}",
                        error.get("message").unwrap_or(&Value::Null)
                    );
                }
            }
        }

        self.request_id += 1;
        Ok(())
    }

    async fn get_balance(&mut self) -> eyre::Result<()> {
        println!("\n╔═══════════════════════════════════════════════════════╗");
        println!("║ Get Balance Tool                                     ║");
        println!("╚═══════════════════════════════════════════════════════╝");

        print!("\nEnter Ethereum address (0x...): ");
        io::stdout().flush()?;
        let mut address = String::new();
        io::stdin().read_line(&mut address)?;
        let address = address.trim().to_string();

        print!("Enter token address (press Enter for ETH): ");
        io::stdout().flush()?;
        let mut token_addr = String::new();
        io::stdin().read_line(&mut token_addr)?;
        let token_addr = token_addr.trim();

        let token_address = if token_addr.is_empty() {
            Value::Null
        } else {
            Value::String(token_addr.to_string())
        };

        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "get_balance",
                "arguments": {
                    "address": address,
                    "token_address": token_address
                }
            },
            "id": self.request_id
        });

        self.send_request(request).await?;
        Ok(())
    }

    async fn get_token_price(&mut self) -> eyre::Result<()> {
        println!("\n╔═══════════════════════════════════════════════════════╗");
        println!("║ Get Token Price Tool                                 ║");
        println!("╚═══════════════════════════════════════════════════════╝");

        print!("\nEnter token symbol or address (e.g., ETH, USDC): ");
        io::stdout().flush()?;
        let mut token = String::new();
        io::stdin().read_line(&mut token)?;
        let token = token.trim().to_string();

        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "get_token_price",
                "arguments": {
                    "token_identifier": token
                }
            },
            "id": self.request_id
        });

        self.send_request(request).await?;
        Ok(())
    }

    async fn swap_tokens(&mut self) -> eyre::Result<()> {
        println!("\n╔═══════════════════════════════════════════════════════╗");
        println!("║ Swap Tokens Tool (Simulation Only)                   ║");
        println!("╚═══════════════════════════════════════════════════════╝");

        print!("\nEnter source token (e.g., ETH): ");
        io::stdout().flush()?;
        let mut from_token = String::new();
        io::stdin().read_line(&mut from_token)?;
        let from_token = from_token.trim().to_string();

        print!("Enter destination token (e.g., USDC): ");
        io::stdout().flush()?;
        let mut to_token = String::new();
        io::stdin().read_line(&mut to_token)?;
        let to_token = to_token.trim().to_string();

        print!("Enter amount to swap: ");
        io::stdout().flush()?;
        let mut amount = String::new();
        io::stdin().read_line(&mut amount)?;
        let amount = amount.trim().to_string();

        print!("Enter slippage tolerance (e.g., 0.5 for 0.5%): ");
        io::stdout().flush()?;
        let mut slippage = String::new();
        io::stdin().read_line(&mut slippage)?;
        let slippage: f64 = slippage.trim().parse().unwrap_or(0.5);

        print!("Enter wallet address (0x...): ");
        io::stdout().flush()?;
        let mut wallet = String::new();
        io::stdin().read_line(&mut wallet)?;
        let wallet = wallet.trim().to_string();

        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "swap_tokens",
                "arguments": {
                    "from_token": from_token,
                    "to_token": to_token,
                    "amount": amount,
                    "slippage": slippage,
                    "wallet_address": wallet
                }
            },
            "id": self.request_id
        });

        self.send_request(request).await?;
        Ok(())
    }

    async fn list_tools(&mut self) -> eyre::Result<()> {
        println!("\n╔═══════════════════════════════════════════════════════╗");
        println!("║ Listing Available Tools                              ║");
        println!("╚═══════════════════════════════════════════════════════╝");

        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "params": {},
            "id": self.request_id
        });

        self.send_request(request).await?;
        Ok(())
    }
}
