//! `agents/mcp.json`（§5.11）。
//!
//! - サーバは**定義順**に読む（§15.8.1「複数一致時は定義順の先頭」）。`serde_json` の `Map` はキーの
//!   順に並べ替えるので、`servers` と `env` は順序を保つ自前の訪問者で読む。
//! - `env` の値は `${NAME}` の参照だけを許す。それ以外（直書き）は **W033**（§5.11）。参照が解決
//!   できない（環境変数が無い）サーバは使わず **E030**（環境は adapters が引く。ここは名前を返すだけ）。
//! - `transport` は `stdio` だけを使う（`http` / `sse` は MAY で、v1 は持たない。`PLANS.md` §9.2 B-147）。
//! - `roles` の `*` はどの役割にも一致する（§15.8.4 の例）。

use std::fmt;

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use serde_json::Value;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};

/// ファイルのパス。
pub const PATH: &str = "agents/mcp.json";

/// サーバ 1 つ。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerConfig {
    /// `servers` のキー。
    pub name: String,
    /// `stdio` など（空なら `stdio`）。
    pub transport: String,
    /// 起動するコマンド。
    pub command: String,
    pub url: String,
    pub model: String,
    /// 引数。
    pub args: Vec<String>,
    /// 環境変数（書かれた順。値は書かれたまま＝`${NAME}`）。
    pub env: Vec<(String, String)>,
    /// `sampling` / `tools`。
    pub capabilities: Vec<String>,
    /// 呼んでよいツール。
    pub allow_tools: Vec<String>,
    /// 受け持つ役割（`*` は全部）。
    pub roles: Vec<String>,
    /// タイムアウト（ミリ秒。無ければ既定の 60 秒）。
    pub timeout_ms: Option<u64>,
}

impl ServerConfig {
    /// stdio で起動できるか（`transport` が空か `stdio`、`command` がある）。
    pub fn is_stdio(&self) -> bool {
        (self.transport.is_empty() || self.transport == "stdio") && !self.command.trim().is_empty()
    }

    /// 役割を受け持つか。
    pub fn serves(&self, role: &str) -> bool {
        self.roles.iter().any(|found| found == role || found == "*")
    }

    /// `sampling` を持つか（無くて `tools` があれば、`allow_tools` の先頭のツールを呼ぶ）。
    pub fn samples(&self) -> bool {
        self.capabilities.iter().any(|found| found == "sampling")
            || !self.capabilities.iter().any(|found| found == "tools")
    }

    /// `env` で参照している環境変数の名前（書かれた順・重複なし）。
    pub fn env_refs(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (_, value) in &self.env {
            for name in refs_in(value) {
                if !out.contains(&name) {
                    out.push(name);
                }
            }
        }
        out
    }

    /// 直書きの `env`（W033）。
    pub fn literal_env(&self) -> Vec<&str> {
        self.env
            .iter()
            .filter(|(_, value)| !is_reference(value))
            .map(|(key, _)| key.as_str())
            .collect()
    }

    /// `env` を解決する。解決できない名前があれば `Err`（E030 の `{0}`）。
    pub fn resolve_env(
        &self,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<Vec<(String, String)>, Vec<String>> {
        let mut missing = Vec::new();
        let mut out = Vec::new();
        for (key, value) in &self.env {
            let mut resolved = String::new();
            let mut rest = value.as_str();
            while let Some(start) = rest.find("${") {
                resolved.push_str(&rest[..start]);
                let after = &rest[start + 2..];
                let Some(end) = after.find('}') else {
                    resolved.push_str(&rest[start..]);
                    rest = "";
                    break;
                };
                let name = &after[..end];
                match lookup(name) {
                    Some(found) => resolved.push_str(&found),
                    None => {
                        if !missing.iter().any(|seen: &String| seen == name) {
                            missing.push(name.to_owned());
                        }
                    }
                }
                rest = &after[end + 1..];
            }
            resolved.push_str(rest);
            out.push((key.clone(), resolved));
        }
        if missing.is_empty() {
            Ok(out)
        } else {
            Err(missing)
        }
    }
}

/// `${NAME}` だけの値か（前後の空白は許す）。
pub fn is_reference(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("${")
        && value.ends_with('}')
        && value[2..value.len() - 1]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && value.len() > 3
}

fn refs_in(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            break;
        };
        out.push(after[..end].to_owned());
        rest = &after[end + 1..];
    }
    out
}

/// 設定全体。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpConfig {
    /// サーバ（定義順）。
    pub servers: Vec<ServerConfig>,
}

impl McpConfig {
    /// 役割を受け持つ最初のサーバ（定義順、stdio のものだけ）。
    pub fn server_for(&self, role: &str) -> Option<&ServerConfig> {
        self.servers.iter().find(|server| {
            (server.is_stdio()
                || (server.transport == "http"
                    && crate::core::stage_two::local_llm::is_loopback(&server.url)))
                && server.serves(role)
        })
    }

    /// 名前で引く。
    pub fn server(&self, name: &str) -> Option<&ServerConfig> {
        self.servers.iter().find(|server| server.name == name)
    }
}

/// 順序を保つ JSON オブジェクト。
struct Ordered(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Walk;
        impl<'de> Visitor<'de> for Walk {
            type Value = Ordered;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    out.push((key, value));
                }
                Ok(Ordered(out))
            }
        }
        deserializer.deserialize_map(Walk)
    }
}

#[derive(Deserialize)]
struct RawServer {
    #[serde(default)]
    transport: String,
    #[serde(default)]
    command: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: Option<Ordered>,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    allow_tools: Vec<String>,
    #[serde(default)]
    roles: Vec<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct RawConfig {
    #[serde(default)]
    servers: Option<Ordered>,
}

/// 読む。JSON として読めなければ `Err`（E025 は §4.3-4 の検査が出す）。形の合わないサーバは飛ばす。
pub fn parse(text: &str) -> Result<McpConfig, String> {
    let raw: RawConfig = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let mut out = McpConfig::default();
    for (name, value) in raw.servers.map(|found| found.0).unwrap_or_default() {
        let Ok(server) = serde_json::from_value::<RawServer>(value) else {
            continue;
        };
        let env = server
            .env
            .map(|found| {
                found
                    .0
                    .into_iter()
                    .map(|(key, value)| {
                        let text = match value {
                            Value::String(text) => text,
                            other => other.to_string(),
                        };
                        (key, text)
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.servers.push(ServerConfig {
            name,
            transport: server.transport,
            command: server.command,
            url: server.url,
            model: server.model,
            args: server.args,
            env,
            capabilities: server.capabilities,
            allow_tools: server.allow_tools,
            roles: server.roles,
            timeout_ms: server.timeout_ms,
        });
    }
    Ok(out)
}

/// `agents/mcp.json` の診断（W033: 直書きの `env`。位置はそのキーの文字列）。
pub fn diagnostics(text: &str) -> Vec<Diagnostic> {
    let Ok(config) = parse(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for server in &config.servers {
        let from = text.find(&format!("\"{}\"", server.name)).unwrap_or(0);
        for key in server.literal_env() {
            let needle = format!("\"{key}\"");
            let range = text[from..]
                .find(&needle)
                .map(|at| from + at..from + at + needle.len());
            out.push(Diagnostic::simple(DiagnosticCode::W033, range));
        }
    }
    out
}

/// E030（環境変数が無い）の診断（文書位置を持たない通知）。
pub fn unresolved(names: &[String]) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::E030,
        None,
        vec![Arg::text(names.join(", "))],
    )
}
