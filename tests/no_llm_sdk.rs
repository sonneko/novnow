//! LLM へのアクセスは MCP 経由だけ（A5・§5.11・§15.8.4 (1)・§19.8）。
//!
//! `Cargo.toml` と `Cargo.lock` に、LLM プロバイダの SDK・OpenAI 互換のクライアント・HTTP クライアントが
//! 入っていないことを確かめる（novnow 本体が HTTP で LLM API を直接叩く手段を持たない）。

const MANIFEST: &str = include_str!("../Cargo.toml");
const LOCK: &str = include_str!("../Cargo.lock");

/// 入れてはいけないクレート（名前が完全に一致するもの）。
const FORBIDDEN: &[&str] = &[
    // LLM プロバイダの SDK・OpenAI 互換のクライアント
    "openai",
    "async-openai",
    "openai-api-rs",
    "openai_dive",
    "anthropic",
    "anthropic-sdk",
    "misanthropy",
    "ollama-rs",
    "genai",
    "rig-core",
    "llm",
    "llm-chain",
    "langchain-rust",
    "google-generative-ai-rs",
    "mistralai-client",
    // HTTP クライアント（LLM API を直接叩けてしまう）
    "reqwest",
    "ureq",
    "hyper",
    "isahc",
    "surf",
    "attohttpc",
    "curl",
    "minreq",
];

/// 名前に含まれていたら疑う語（完全一致でなくても、新しい SDK を見落とさないため）。
const SUSPICIOUS: &[&str] = &[
    "openai",
    "anthropic",
    "ollama",
    "gemini",
    "claude",
    "mistral",
];

fn lock_names() -> Vec<&'static str> {
    LOCK.lines()
        .filter_map(|line| line.strip_prefix("name = \""))
        .filter_map(|rest| rest.strip_suffix('"'))
        .collect()
}

#[test]
fn the_lock_has_no_llm_sdk_and_no_http_client() {
    let names = lock_names();
    assert!(names.len() > 100, "Cargo.lock を読めていない");
    let found: Vec<&&str> = names
        .iter()
        .filter(|name| FORBIDDEN.contains(name))
        .collect();
    assert!(
        found.is_empty(),
        "LLM の SDK / HTTP クライアントが依存に入っている: {found:?}"
    );
    let suspicious: Vec<&&str> = names
        .iter()
        .filter(|name| SUSPICIOUS.iter().any(|word| name.contains(word)))
        .collect();
    assert!(
        suspicious.is_empty(),
        "LLM の SDK らしい依存: {suspicious:?}"
    );
}

#[test]
fn the_manifest_declares_none_of_them() {
    for line in MANIFEST.lines() {
        let name = line.split(['=', ' ', '.']).next().unwrap_or("").trim();
        assert!(!FORBIDDEN.contains(&name), "Cargo.toml: {line}");
    }
}
