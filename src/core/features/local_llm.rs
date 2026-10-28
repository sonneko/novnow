//! ローカル LLM のための MCP 設定の管理（§15.8.4）。事業者の SDK も使わず、
//! ここからネットワークへ要求もしない。
use crate::core::model::project::LocalLlm;
use serde_json::{json, Value};
pub fn is_loopback(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    if rest.bytes().any(|b| b.is_ascii_control() || b == b' ') {
        return false;
    }
    let authority = rest.split('/').next().unwrap_or("");
    if authority.contains(['@', '?', '#', '\\']) {
        return false;
    }
    if authority.starts_with("[::1]") {
        return authority == "[::1]" || authority.strip_prefix("[::1]:").is_some_and(valid_port);
    }
    let (host, port) = authority
        .split_once(':')
        .map_or((authority, None), |(h, p)| (h, Some(p)));
    matches!(host, "localhost" | "127.0.0.1") && port.is_none_or(valid_port)
}
fn valid_port(s: &str) -> bool {
    s.parse::<u16>().is_ok_and(|n| n > 0)
}
pub fn configure(text: &str, config: &LocalLlm) -> Result<String, &'static str> {
    if config.enabled && !is_loopback(&config.url) {
        return Err("loopback URL required");
    }
    let mut value: Value = if text.trim().is_empty() {
        json!({"schema_version":1,"servers":{}})
    } else {
        serde_json::from_str(text).map_err(|_| "invalid MCP JSON")?
    };
    let servers = value
        .get_mut("servers")
        .and_then(Value::as_object_mut)
        .ok_or("invalid servers")?;
    servers.retain(|_, v| v.get("managed_by").and_then(Value::as_str) != Some("novnow-local-llm"));
    if config.enabled {
        let name = (0..)
            .map(|n| {
                if n == 0 {
                    "novnow-local-llm".to_owned()
                } else {
                    format!("novnow-local-llm-{n}")
                }
            })
            .find(|n| !servers.contains_key(n))
            .unwrap();
        servers.insert(name, json!({"transport":"http", "url":config.url, "roles":["*"], "model":config.model, "capabilities":["sampling"], "managed_by":"novnow-local-llm"}));
    }
    serde_json::to_string_pretty(&value)
        .map(|mut s| {
            s.push('\n');
            s
        })
        .map_err(|_| "serialization failed")
}
