//! Bounded loopback HTTP MCP transport. This speaks JSON-RPC MCP, never a provider API.
use crate::{
    core::agents::{cap_output, runner},
    ports::mcp::{McpCall, McpError, McpReply, McpServer},
};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
fn failed(s: impl Into<String>) -> McpError {
    McpError::Protocol(s.into())
}
struct Link<'a> {
    address: SocketAddr,
    host: String,
    path: String,
    session: Option<String>,
    deadline: Instant,
    cancel: &'a AtomicBool,
}
impl Link<'_> {
    fn guard(&self) -> Result<(), McpError> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(McpError::Cancelled)
        } else if Instant::now() >= self.deadline {
            Err(McpError::Timeout)
        } else {
            Ok(())
        }
    }
    fn post(&mut self, value: Value) -> Result<Value, McpError> {
        self.guard()?;
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        let mut stream =
            TcpStream::connect_timeout(&self.address, remaining.min(Duration::from_secs(3)))
                .map_err(|e| failed(e.to_string()))?;
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .map_err(|e| failed(e.to_string()))?;
        stream
            .set_write_timeout(Some(remaining.min(Duration::from_secs(3))))
            .map_err(|e| failed(e.to_string()))?;
        let body = value.to_string();
        let session = self
            .session
            .as_ref()
            .map(|s| format!("Mcp-Session-Id: {s}\r\n"))
            .unwrap_or_default();
        write!(stream,"POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nMCP-Protocol-Version: 2025-03-26\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",self.path,self.host,session,body.len(),body).map_err(|e|failed(e.to_string()))?;
        let mut response = Vec::new();
        let mut buffer = [0; 8192];
        let mut header = None;
        let mut length = None;
        let mut chunked = false;
        loop {
            self.guard()?;
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    response.extend_from_slice(&buffer[..n]);
                    if response.len() > 2 * 1024 * 1024 {
                        return Err(failed("response too large"));
                    }
                    if header.is_none() {
                        if let Some(at) = response.windows(4).position(|s| s == b"\r\n\r\n") {
                            let text = String::from_utf8_lossy(&response[..at]);
                            if !text.lines().next().is_some_and(|l| {
                                l.split_whitespace()
                                    .nth(1)
                                    .is_some_and(|n| matches!(n, "200" | "202" | "204"))
                            }) {
                                return Err(failed("HTTP MCP request rejected"));
                            }
                            for line in text.lines() {
                                if let Some((name, value)) = line.split_once(':') {
                                    if name.eq_ignore_ascii_case("transfer-encoding") {
                                        chunked = value.trim().eq_ignore_ascii_case("chunked");
                                    }
                                    if name.eq_ignore_ascii_case("content-length") {
                                        length = value.trim().parse::<usize>().ok();
                                    }
                                    if name.eq_ignore_ascii_case("mcp-session-id") {
                                        let s = value.trim();
                                        if s.bytes().all(|b| b.is_ascii_graphic()) {
                                            self.session = Some(s.to_owned());
                                        }
                                    }
                                }
                            }
                            header = Some(at + 4);
                        }
                    }
                    if let Some(at) = header {
                        if chunked && decode_chunks(&response[at..])?.is_some() {
                            break;
                        }
                        if length.is_some_and(|n| response.len() - at >= n) {
                            break;
                        }
                        let text = String::from_utf8_lossy(&response[at..]);
                        if value.get("id").is_none() && length == Some(0) {
                            break;
                        }
                        if text
                            .lines()
                            .filter_map(|l| l.strip_prefix("data: "))
                            .any(|l| {
                                serde_json::from_str::<Value>(l)
                                    .ok()
                                    .is_some_and(|v| v.get("id") == value.get("id"))
                            })
                        {
                            break;
                        }
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(e) => return Err(failed(e.to_string())),
            }
        }
        let at = header.ok_or_else(|| failed("missing HTTP headers"))?;
        let decoded = if chunked {
            decode_chunks(&response[at..])?.ok_or_else(|| failed("truncated HTTP chunks"))?
        } else {
            response[at..].to_vec()
        };
        let text = String::from_utf8_lossy(&decoded);
        if value.get("id").is_none() {
            return Ok(Value::Null);
        }
        let rpc = serde_json::from_str::<Value>(&text)
            .ok()
            .or_else(|| {
                text.lines()
                    .filter_map(|l| l.strip_prefix("data: "))
                    .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                    .find(|v| v.get("id") == value.get("id"))
            })
            .ok_or_else(|| failed("invalid MCP JSON response"))?;
        if rpc.get("id") != value.get("id") || rpc.get("error").is_some() {
            return Err(failed("MCP response error"));
        }
        rpc.get("result")
            .cloned()
            .ok_or_else(|| failed("missing MCP result"))
    }
}
/// Bounded incremental chunk decoder. `None` means more bytes are needed.
fn decode_chunks(body: &[u8]) -> Result<Option<Vec<u8>>, McpError> {
    let mut at = 0;
    let mut output = Vec::new();
    loop {
        let Some(end) = body[at..].windows(2).position(|b| b == b"\r\n") else {
            return Ok(None);
        };
        let size = std::str::from_utf8(&body[at..at + end])
            .ok()
            .and_then(|s| usize::from_str_radix(s.split(';').next().unwrap_or("").trim(), 16).ok())
            .ok_or_else(|| failed("invalid HTTP chunk"))?;
        at += end + 2;
        if size == 0 {
            return Ok(Some(output));
        }
        if size > 2 * 1024 * 1024 || output.len() + size > 2 * 1024 * 1024 {
            return Err(failed("response too large"));
        }
        let Some(end) = at.checked_add(size + 2) else {
            return Err(failed("invalid HTTP chunk"));
        };
        if end > body.len() {
            return Ok(None);
        }
        if &body[end - 2..end] != b"\r\n" {
            return Err(failed("invalid HTTP chunk delimiter"));
        }
        output.extend_from_slice(&body[at..end - 2]);
        at = end;
    }
}

pub fn call(server: &McpServer, call: &McpCall, cancel: &AtomicBool) -> Result<McpReply, McpError> {
    let url = server
        .args
        .first()
        .ok_or_else(|| failed("missing MCP URL"))?;
    if !crate::core::features::local_llm::is_loopback(url) {
        return Err(failed("loopback MCP URL required"));
    }
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| failed("TLS loopback MCP is not supported; use HTTP on loopback"))?;
    let (host, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_owned()), |(h, p)| (h, format!("/{p}")));
    let (ip, port) = if let Some(v6) = host.strip_prefix("[::1]") {
        (
            "::1",
            v6.strip_prefix(':')
                .and_then(|p| p.parse().ok())
                .unwrap_or(80),
        )
    } else {
        let (h, p) = host
            .split_once(':')
            .map_or((host, 80), |(h, p)| (h, p.parse::<u16>().unwrap_or(80)));
        (if h == "localhost" { "127.0.0.1" } else { h }, p)
    };
    let address = SocketAddr::new(
        ip.parse().map_err(|_| failed("invalid loopback address"))?,
        port,
    );
    let mut link = Link {
        address,
        host: host.to_owned(),
        path,
        session: None,
        deadline: Instant::now() + Duration::from_millis(server.timeout_ms.max(1)),
        cancel,
    };
    let init=link.post(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"novnow","version":env!("CARGO_PKG_VERSION")}}}))?;
    link.post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))?;
    let result=match call {McpCall::Sampling{params}=>{let mut params=params.clone();if let Some(model)=server.args.get(1).filter(|s|!s.is_empty()){params["modelPreferences"]=json!({"hints":[{"name":model}]});}link.post(json!({"jsonrpc":"2.0","id":2,"method":"sampling/createMessage","params":params}))?},McpCall::Tool{name,..}if name=="__novnow_probe"=>{return Ok(McpReply{text:init.to_string(),model:server.args.get(1).cloned().unwrap_or_default(),truncated:false});},McpCall::Tool{name,arguments}=>link.post(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":name,"arguments":arguments}}))?};
    let (text, model) = runner::reply_of(&result).ok_or_else(|| failed("no MCP text content"))?;
    let (text, truncated) = cap_output(&text);
    Ok(McpReply {
        text,
        model,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunk_bounds() {
        assert_eq!(
            decode_chunks(b"3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n").unwrap(),
            Some(b"abcde".to_vec())
        );
        assert_eq!(decode_chunks(b"3\r\na").unwrap(), None);
        assert!(decode_chunks(b"ffffffff\r\n").is_err());
    }
    #[test]
    fn mock_loopback_mcp_initializes_preserves_session_and_reads_chunked_sampling() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let worker = std::thread::spawn(move || {
            for step in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                loop {
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&buffer[..n]);
                    if let Some(at) = request.windows(4).position(|s| s == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..at]);
                        let len = headers
                            .lines()
                            .find_map(|l| l.strip_prefix("Content-Length: "))
                            .unwrap()
                            .parse::<usize>()
                            .unwrap();
                        if request.len() >= at + 4 + len {
                            break;
                        }
                    }
                }
                let raw = String::from_utf8(request).unwrap();
                let body: Value =
                    serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
                if step > 0 {
                    assert!(raw.contains("Mcp-Session-Id: test-session"));
                }
                match step {
                    0 => {
                        assert_eq!(body["method"], "initialize");
                        let response=json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-03-26","capabilities":{"sampling":{}}}}).to_string();
                        write!(stream,"HTTP/1.1 200 OK\r\nMcp-Session-Id: test-session\r\nContent-Length: {}\r\n\r\n{}",response.len(),response).unwrap();
                    }
                    1 => {
                        assert_eq!(body["method"], "notifications/initialized");
                        write!(stream, "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n")
                            .unwrap();
                    }
                    _ => {
                        assert_eq!(body["method"], "sampling/createMessage");
                        assert_eq!(
                            body["params"]["modelPreferences"]["hints"][0]["name"],
                            "local-model"
                        );
                        let response=json!({"jsonrpc":"2.0","id":2,"result":{"content":{"type":"text","text":"response"},"model":"local-model"}}).to_string();
                        write!(stream,"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",response.len(),response).unwrap();
                    }
                }
            }
        });
        let server = McpServer {
            name: "local".into(),
            command: "http".into(),
            args: vec![format!("http://127.0.0.1:{port}/mcp"), "local-model".into()],
            env: Vec::new(),
            timeout_ms: 2000,
        };
        let reply = call(
            &server,
            &McpCall::Sampling {
                params: json!({"messages":[]}),
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(reply.text, "response");
        worker.join().unwrap();
        assert!(matches!(
            call(
                &server,
                &McpCall::Sampling { params: json!({}) },
                &AtomicBool::new(true)
            ),
            Err(McpError::Cancelled)
        ));
    }
}
