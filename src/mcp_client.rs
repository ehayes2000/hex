use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Command as AsyncCommand, Child};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug)]
pub struct McpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub tools: Vec<McpTool>,
}

struct ServerConnection {
    process: Child,
    writer: Mutex<BufWriter<tokio::process::ChildStdin>>,
    reader: Mutex<BufReader<tokio::process::ChildStdout>>,
}

pub struct McpClient {
    servers: Vec<McpServer>,
    connections: HashMap<String, ServerConnection>,
    request_id: AtomicU64,
}

impl McpClient {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            connections: HashMap::new(),
            request_id: AtomicU64::new(2),
        }
    }

    pub fn add_server(&mut self, server: McpServer) {
        self.servers.push(server);
    }

    pub async fn initialize(&mut self) -> Result<()> {
        let servers = std::mem::take(&mut self.servers);
        for mut server in servers {
            self.start_server(&mut server).await?;
            self.servers.push(server);
        }
        Ok(())
    }

    async fn start_server(&mut self, server: &mut McpServer) -> Result<()> {
        let mut cmd = AsyncCommand::new(&server.command);
        cmd.args(&server.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &server.env {
            cmd.env(key, value);
        }

        let mut child = cmd
            .spawn()
            .context(format!("Failed to spawn MCP server: {}", server.name))?;

        // Send initialization message
        let init_msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "sampling": {}
                },
                "clientInfo": {
                    "name": "hex",
                    "version": "0.1.0"
                }
            }
        });

        let stdin = child.stdin.take().context("Failed to get stdin")?;
        let stdout = child.stdout.take().context("Failed to get stdout")?;
        
        let mut writer = BufWriter::new(stdin);
        let mut reader = BufReader::new(stdout);
        
        writer.write_all(init_msg.to_string().as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        // Read initialization response
        let mut response = String::new();
        reader.read_line(&mut response).await?;

        println!("MCP Server {} initialized", server.name);

        // Get available tools
        let tools_msg = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });

        writer.write_all(tools_msg.to_string().as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        let mut tools_response = String::new();
        reader.read_line(&mut tools_response).await?;

        if let Ok(parsed) = serde_json::from_str::<Value>(&tools_response) {
            if let Some(tools_array) = parsed["result"]["tools"].as_array() {
                server.tools = tools_array
                    .iter()
                    .filter_map(|tool| {
                        Some(McpTool {
                            name: tool["name"].as_str()?.to_string(),
                            description: tool["description"].as_str()?.to_string(),
                            input_schema: tool["inputSchema"].clone(),
                        })
                    })
                    .collect();
            }
        }

        // Store the connection for later use
        let connection = ServerConnection {
            process: child,
            writer: Mutex::new(writer),
            reader: Mutex::new(reader),
        };
        self.connections.insert(server.name.clone(), connection);

        Ok(())
    }

    pub fn get_all_tools(&self) -> Vec<McpTool> {
        self.servers
            .iter()
            .flat_map(|server| server.tools.iter().cloned())
            .collect()
    }

    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<String> {
        // Find which server has this tool
        let server = self
            .servers
            .iter()
            .find(|s| s.tools.iter().any(|t| t.name == tool_name))
            .context("Tool not found in any server")?;

        // Get the connection for this server
        let connection = self.connections
            .get(&server.name)
            .context("Server connection not found")?;

        // Generate unique request ID
        let request_id = self.request_id.fetch_add(1, Ordering::SeqCst);

        // Create tool call message
        let tool_call_msg = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        // Send the request
        {
            let mut writer = connection.writer.lock().await;
            writer.write_all(tool_call_msg.to_string().as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }

        // Read the response
        let response = {
            let mut reader = connection.reader.lock().await;
            let mut response_line = String::new();
            reader.read_line(&mut response_line).await?;
            response_line
        };

        // Parse the JSON-RPC response
        let parsed_response: Value = serde_json::from_str(&response)
            .context("Failed to parse tool call response")?;

        // Check for error in response
        if let Some(error) = parsed_response.get("error") {
            return Err(anyhow::anyhow!("Tool call error: {}", error));
        }

        // Extract the result
        let result = parsed_response
            .get("result")
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|text| text.as_str())
            .unwrap_or("No result returned");

        Ok(result.to_string())
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}
