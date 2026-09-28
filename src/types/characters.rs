//! characters.json — キャラクタ辞書。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// キャラクタ 1 件。未知フィールドは `extra` に温存する。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Character {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub personality: String,
    #[serde(default)]
    pub speech: String,
    #[serde(default)]
    pub notes: String,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Character {
    /// 情報パネルに出す一行サマリ。
    pub fn summary(&self) -> String {
        for s in [&self.role, &self.personality, &self.notes] {
            if !s.is_empty() {
                return s.clone();
            }
        }
        "(詳細未設定)".to_string()
    }

    /// 本名 + 別名。
    pub fn all_names(&self) -> Vec<String> {
        let mut v = vec![self.name.clone()];
        v.extend(self.aliases.iter().cloned());
        v.retain(|s| !s.is_empty());
        v
    }
}

pub type CharacterMap = HashMap<String, Character>;

/// characters.json をゆるく読む。
///
/// * 配列形式 `[ { "name": "..." }, ... ]`
/// * オブジェクト形式 `{ "アルス": { ... }, ... }`
///
/// の両方を受け付ける。
pub fn parse_characters(text: &str) -> anyhow::Result<(CharacterMap, Vec<String>)> {
    let value: Value = serde_json::from_str(text)?;
    let mut map: CharacterMap = HashMap::new();
    let mut warnings: Vec<String> = Vec::new();

    match value {
        Value::Array(items) => {
            for (i, item) in items.into_iter().enumerate() {
                match serde_json::from_value::<Character>(item) {
                    Ok(c) => {
                        if c.name.is_empty() {
                            warnings.push(format!("characters[{i}]: name が空のため無視しました"));
                            continue;
                        }
                        map.insert(c.name.clone(), c);
                    }
                    Err(e) => warnings.push(format!("characters[{i}]: {e}")),
                }
            }
        }
        Value::Object(obj) => {
            for (key, item) in obj {
                let mut c: Character = serde_json::from_value(item).unwrap_or_default();
                if c.name.is_empty() {
                    c.name = key.clone();
                }
                map.insert(key, c);
            }
        }
        _ => anyhow::bail!("characters.json は配列かオブジェクトである必要があります"),
    }

    Ok((map, warnings))
}
