//! materials/index.json — 外部資料のインデックス。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// 資料のステータス（表示用の補助。JSON では文字列として保持する）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialStatus {
    /// 承認済み。
    Approval,
    /// LLM からの提案。
    LlmSuggestion,
    /// 却下。
    Dismissed,
    /// 未知のステータス。
    Unknown,
}

impl MaterialStatus {
    pub fn parse(s: &str) -> Self {
        match s {
            "approval" => MaterialStatus::Approval,
            "llm_suggestion" => MaterialStatus::LlmSuggestion,
            "dismissed" => MaterialStatus::Dismissed,
            _ => MaterialStatus::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MaterialStatus::Approval => "approval",
            MaterialStatus::LlmSuggestion => "llm_suggestion",
            MaterialStatus::Dismissed => "dismissed",
            MaterialStatus::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Material {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub abs: String,
    /// 生のステータス文字列（未知値を失わないため String で保持）。
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub url: String,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Material {
    pub fn status_kind(&self) -> MaterialStatus {
        MaterialStatus::parse(&self.status)
    }
}

/// materials/index.json を読む（配列 / オブジェクト両対応）。
pub fn parse_materials(text: &str) -> anyhow::Result<(Vec<Material>, Vec<String>)> {
    let value: Value = serde_json::from_str(text)?;
    let mut list: Vec<Material> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    match value {
        Value::Array(items) => {
            for (i, item) in items.into_iter().enumerate() {
                match serde_json::from_value::<Material>(item) {
                    Ok(m) => list.push(m),
                    Err(e) => warnings.push(format!("materials[{i}]: {e}")),
                }
            }
        }
        Value::Object(obj) => {
            for (key, item) in obj {
                match serde_json::from_value::<Material>(item) {
                    Ok(mut m) => {
                        if m.path.is_empty() {
                            m.path = key.clone();
                        }
                        list.push(m);
                    }
                    Err(e) => warnings.push(format!("materials.{key}: {e}")),
                }
            }
        }
        _ => anyhow::bail!("materials/index.json は配列かオブジェクトである必要があります"),
    }

    Ok((list, warnings))
}
