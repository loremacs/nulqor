//! Nulqor MCP stdio server.
//!
//! Implements the Model Context Protocol (JSON-RPC 2.0 over stdio) and proxies
//! the five observer tools to the Nulqor HTTP API running at NULQOR_API_URL
//! (default: http://localhost:8080).
//!
//! Tools exposed:
//!   register_observer  — join the shared transcript as an observer
//!   catch_up           — fetch new messages since last ack
//!   ack_observer       — acknowledge seen messages
//!   send_message       — post a message / trigger a Subject generation
//!   list_observers     — list registered observer names
//!
//! MCP protocol version: 2024-11-05
//!
//! Usage (add to .cursor/mcp.json):
//!   "command": "cargo"
//!   "args":    ["run", "--manifest-path", "tools/mcp-server/Cargo.toml", "--"]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

// ---------------------------------------------------------------------------
// JSON-RPC types
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct Request {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct Response {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

impl Response {
    fn ok(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
    }

    fn err(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(RpcError { code, message: message.into() }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "register_observer",
                "description": "Register this agent as an observer of the Nulqor shared transcript. Call once before any other tool. Returns observer name and initial sequence number.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Observer name (e.g. 'builder'). Omit to auto-assign."
                        }
                    }
                }
            },
            {
                "name": "catch_up",
                "description": "Fetch transcript messages added since the last ack. Returns an array of message_added events. Call after register_observer and after each send_message to read the reply.",
                "inputSchema": {
                    "type": "object",
                    "required": ["observer_name"],
                    "properties": {
                        "observer_name": {
                            "type": "string",
                            "description": "The name returned by register_observer."
                        },
                        "auto_ack": {
                            "type": "boolean",
                            "description": "If true, automatically acknowledge after returning events."
                        }
                    }
                }
            },
            {
                "name": "ack_observer",
                "description": "Acknowledge all events returned by the last catch_up. Must be called before the next catch_up to advance the sequence pointer.",
                "inputSchema": {
                    "type": "object",
                    "required": ["observer_name"],
                    "properties": {
                        "observer_name": {
                            "type": "string",
                            "description": "The name returned by register_observer."
                        }
                    }
                }
            },
            {
                "name": "send_message",
                "description": "Post a user message to the shared transcript and trigger a Subject model generation. The Subject reply will appear in the next catch_up.",
                "inputSchema": {
                    "type": "object",
                    "required": ["message", "observer_name"],
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "The text content to send."
                        },
                        "observer_name": {
                            "type": "string",
                            "description": "Must match a registered observer name."
                        },
                        "model": {
                            "type": "string",
                            "description": "Optional model override."
                        },
                        "agent": {
                            "type": "string",
                            "description": "Optional agent persona name."
                        }
                    }
                }
            },
            {
                "name": "list_observers",
                "description": "List all currently registered observer names and their sequence positions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }
        ]
    })
}

// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

fn api_base() -> String {
    std::env::var("NULQOR_API_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

fn http_get(path: &str) -> Result<Value, String> {
    let url = format!("{}{path}", api_base());
    reqwest::blocking::get(&url)
        .map_err(|e| format!("GET {url}: {e}"))?
        .json::<Value>()
        .map_err(|e| format!("GET {url} parse: {e}"))
}

fn http_post(path: &str, body: Value) -> Result<Value, String> {
    let url = format!("{}{path}", api_base());
    reqwest::blocking::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| format!("POST {url}: {e}"))?
        .json::<Value>()
        .map_err(|e| format!("POST {url} parse: {e}"))
}

// ---------------------------------------------------------------------------
// Tool dispatch
// ---------------------------------------------------------------------------

fn call_tool(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "register_observer" => {
            let observer_name = args["name"].as_str().unwrap_or("").to_owned();
            let result = http_post(
                "/observers/register",
                json!({ "name": observer_name }),
            )?;
            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }

        "catch_up" => {
            let observer = args["observer_name"]
                .as_str()
                .ok_or("catch_up: observer_name required")?;
            let auto_ack = args["auto_ack"].as_bool().unwrap_or(false);
            let result = http_get(&format!(
                "/observers/catch-up?observer={observer}&auto_ack={auto_ack}"
            ))?;
            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }

        "ack_observer" => {
            let observer = args["observer_name"]
                .as_str()
                .ok_or("ack_observer: observer_name required")?
                .to_owned();
            let result = http_post("/observers/ack", json!({ "name": observer }))?;
            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }

        "send_message" => {
            let result = http_post("/message", args.clone())?;
            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }

        "list_observers" => {
            let result = http_get("/observers")?;
            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }

        other => Err(format!("unknown tool: {other}")),
    }
}

// ---------------------------------------------------------------------------
// MCP request dispatch
// ---------------------------------------------------------------------------

fn dispatch(req: &Request) -> Response {
    let id = req.id.clone().unwrap_or(Value::Null);

    match req.method.as_str() {
        "initialize" => Response::ok(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "nulqor-mcp-server",
                    "version": "0.1.0"
                }
            }),
        ),

        "initialized" => {
            // Notification — no response needed, but handle gracefully
            Response::ok(id, json!({}))
        }

        "ping" => Response::ok(id, json!({})),

        "tools/list" => Response::ok(id, tool_list()),

        "tools/call" => {
            let params = req.params.as_ref().unwrap_or(&Value::Null);
            let tool_name = match params["name"].as_str() {
                Some(n) => n,
                None => {
                    return Response::err(id, -32602, "tools/call: 'name' param required");
                }
            };
            let args = &params["arguments"];

            match call_tool(tool_name, args) {
                Ok(text) => Response::ok(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": text }],
                        "isError": false
                    }),
                ),
                Err(e) => Response::ok(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": format!("ERROR: {e}") }],
                        "isError": true
                    }),
                ),
            }
        }

        other => Response::err(id, -32601, format!("method not found: {other}")),
    }
}

// ---------------------------------------------------------------------------
// Main loop — one JSON-RPC message per line (stdio transport)
// ---------------------------------------------------------------------------

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    eprintln!(
        "[nulqor-mcp-server] ready — API: {}",
        api_base()
    );

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            Ok(_) => continue,
            Err(e) => {
                eprintln!("[nulqor-mcp-server] stdin error: {e}");
                break;
            }
        };

        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response::err(
                    Value::Null,
                    -32700,
                    format!("parse error: {e}"),
                );
                let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap());
                let _ = out.flush();
                continue;
            }
        };

        // Skip pure notifications (no id) — they don't need a response
        if req.id.is_none() && req.method == "notifications/initialized" {
            continue;
        }

        let resp = dispatch(&req);
        let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap());
        let _ = out.flush();
    }
}
