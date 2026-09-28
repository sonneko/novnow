//! words.json — 用語辞書。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Word {
    #[serde(default)]
    pub term: String,
    #[serde(default)]
    pub reading: String,
    #[serde(default)]
    pub definition: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Word {
    pub fn summary(&self) -> String {
        if self.definition.is_empty() {
            self.reading.clone()
        } else {
            self.definition.clone()
        }
    }

    pub fn all_terms(&self) -> Vec<String> {
        let mut v = vec![self.term.clone()];
        v.extend(self.aliases.iter().cloned());
        v.retain(|s| !s.is_empty());
        v
    }
}

pub type WordMap = HashMap<String, Word>;

/// words.json をゆるく読む（配列 / オブジェクト両対応）。
pub fn parse_words(text: &str) -> anyhow::Result<(WordMap, Vec<String>)> {
    let value: Value = serde_json::from_str(text)?;
    let mut map: WordMap = HashMap::new();
    let mut warnings: Vec<String> = Vec::new();

    match value {
        Value::Array(items) => {
            for (i, item) in items.into_iter().enumerate() {
                match serde_json::from_value::<Word>(item) {
                    Ok(w) => {
                        if w.term.is_empty() {
                            warnings.push(format!("words[{i}]: term が空のため無視しました"));
                            continue;
                        }
                        map.insert(w.term.clone(), w);
                    }
                    Err(e) => warnings.push(format!("words[{i}]: {e}")),
                }
            }
        }
        Value::Object(obj) => {
            for (key, item) in obj {
                let mut w: Word = serde_json::from_value(item).unwrap_or_default();
                if w.term.is_empty() {
                    w.term = key.clone();
                }
                map.insert(key, w);
            }
        }
        _ => anyhow::bail!("words.json は配列かオブジェクトである必要があります"),
    }

    Ok((map, warnings))
}
