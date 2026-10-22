//! 外部インターフェース共通のエラーモデル（§19.1.7）。**egui に依存しない**。
//!
//! 5 つのインターフェース（Rust・Python・LSP・MCP・CLI）は同じ 9 分類を共有し、それぞれ
//! 別の表面へ写像する:
//!
//! | 分類 | JSON-RPC | CLI 終了コード（§14.2.5） |
//! |---|---|---|
//! | `usage` | `-32602` | 2 |
//! | `project` | `-32602` + `data.code` | 3 |
//! | `io` | `-32000` + `data.code` | 4 |
//! | `validation` | 結果内 `isError` / 診断 | 5 |
//! | `policy` | 結果内 `isError` + `data.code` | 5 |
//! | `not_found` | `-32001` + `data.code` | 3 |
//! | `conflict` | `-32002` + `data.code` | 4 |
//! | `timeout` | `-32003` | 4 |
//! | `cancelled` | `-32800` | 0（部分書き込みを残さない） |

use serde::Serialize;

use crate::ui::i18n::{self, Locale};

/// エラーの分類（§19.1.7 の表の 1 列目）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    /// 引数の形・型の誤り。
    Usage,
    /// プロジェクト構造の不正（S021・S022 など）。
    Project,
    /// ファイルシステムのエラー（S030・S034 など）。
    Io,
    /// 内容の検証エラー（E001〜E040）。
    Validation,
    /// 権限・関門の拒否（E036 など）。
    Policy,
    /// 対象 ID の不存在（セクション・タスクなど）。
    NotFound,
    /// 保存状態機の衝突（外部変更・ロック）。
    Conflict,
    /// LLM 応答・処理の時間切れ（§18.3.1）。
    Timeout,
    /// 利用者が操作を中断した。
    Cancelled,
}

impl Class {
    /// JSON・例外クラス名に使う名前（`usage` など）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::Project => "project",
            Self::Io => "io",
            Self::Validation => "validation",
            Self::Policy => "policy",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
        }
    }

    /// 名前から（`api.list` の例外列・CLI の `--json` の逆変換）。
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "usage" => Self::Usage,
            "project" => Self::Project,
            "io" => Self::Io,
            "validation" => Self::Validation,
            "policy" => Self::Policy,
            "not_found" => Self::NotFound,
            "conflict" => Self::Conflict,
            "timeout" => Self::Timeout,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }

    /// §19.1.7 の JSON-RPC コード。
    pub fn jsonrpc(self) -> i64 {
        match self {
            Self::Usage | Self::Project => -32602,
            Self::Io => -32000,
            Self::NotFound => -32001,
            Self::Conflict => -32002,
            Self::Timeout => -32003,
            Self::Cancelled => -32800,
            // `validation` / `policy` は JSON-RPC エラーにしない（MCP は `isError`、§15.8.3）
            Self::Validation | Self::Policy => -32000,
        }
    }

    /// JSON-RPC エラーにするか（`validation` / `policy` は結果内に載せる、§19.1.7）。
    pub fn is_protocol_error(self) -> bool {
        !matches!(self, Self::Validation | Self::Policy)
    }

    /// §14.2.5 / §19.6.3 の終了コード。
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Usage => 2,
            Self::Project | Self::NotFound => 3,
            Self::Io | Self::Conflict | Self::Timeout => 4,
            Self::Validation | Self::Policy => 5,
            Self::Cancelled => 0,
        }
    }
}

/// 全インターフェース共通のエラー（§19.1.7「`code`・`kind`・`message`・`path` を含む」）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// 分類（`code`）。
    pub class: Class,
    /// S / E 番号（無ければ空）。
    pub kind: String,
    /// 地域化済みのメッセージ（§7.4）。
    pub message: String,
    /// 該当ファイル（プロジェクトルートからの相対パス。あれば）。
    pub path: Option<String>,
}

impl Error {
    /// 分類とメッセージだけで作る。
    pub fn new(class: Class, message: impl Into<String>) -> Self {
        Self {
            class,
            kind: String::new(),
            message: message.into(),
            path: None,
        }
    }

    /// 引数の誤り（`usage`）。`arg` は引数名。
    pub fn usage(locale: Locale, arg: &str) -> Self {
        Self::new(
            Class::Usage,
            i18n::format(locale, "api.error.usage", &[arg]),
        )
    }

    /// 必須の引数が無い（`usage`）。
    pub fn missing(locale: Locale, arg: &str) -> Self {
        Self::new(
            Class::Usage,
            i18n::format(locale, "api.error.missing", &[arg]),
        )
    }

    /// 対象が無い（`not_found`）。
    pub fn not_found(locale: Locale, what: &str, path: Option<&str>) -> Self {
        Self {
            path: path.map(str::to_owned),
            ..Self::new(
                Class::NotFound,
                i18n::format(locale, "api.error.not_found", &[what]),
            )
        }
    }

    /// 権限・関門の拒否（`policy`）。S / E 番号を `kind` に持つ。
    pub fn policy(locale: Locale, kind: &str, message: impl Into<String>) -> Self {
        Self {
            kind: kind.to_owned(),
            ..Self::new(Class::Policy, message).with_locale(locale)
        }
    }

    /// 内容の検証エラー（`validation`）。
    pub fn validation(kind: &str, message: impl Into<String>) -> Self {
        Self {
            kind: kind.to_owned(),
            ..Self::new(Class::Validation, message)
        }
    }

    /// I/O の失敗（`io`）。
    pub fn io(message: impl Into<String>, path: Option<&str>) -> Self {
        Self {
            path: path.map(str::to_owned),
            ..Self::new(Class::Io, message)
        }
    }

    /// 保存状態機の衝突（`conflict`）。
    pub fn conflict(locale: Locale, path: &str) -> Self {
        Self {
            path: Some(path.to_owned()),
            ..Self::new(
                Class::Conflict,
                i18n::format(locale, "api.error.conflict", &[path]),
            )
        }
    }

    /// このインターフェースでは提供しない操作（`usage`。§19.7 の例外列）。
    pub fn unsupported(locale: Locale, op: &str, interface: &str) -> Self {
        Self::new(
            Class::Usage,
            i18n::format(locale, "api.error.unsupported", &[op, interface]),
        )
    }

    /// 状態を持つセッションが必要（`usage`。§19.7 の `M—・C—（状態性）`）。
    pub fn stateful_required(locale: Locale, op: &str) -> Self {
        Self::new(
            Class::Usage,
            i18n::format(locale, "api.error.stateful", &[op]),
        )
    }

    /// `kind`（S / E 番号）を付ける。
    pub fn with_kind(mut self, kind: &str) -> Self {
        self.kind = kind.to_owned();
        self
    }

    /// `path` を付ける。
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// メッセージが空のときだけカタログの既定文言で埋める。
    fn with_locale(mut self, locale: Locale) -> Self {
        if self.message.is_empty() {
            self.message = i18n::text(locale, "api.error.generic").to_owned();
        }
        self
    }

    /// `--json` / JSON-RPC / Python 例外が共有する形（§19.6.2 の `error`）。
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "code".to_owned(),
            serde_json::Value::String(self.class.as_str().to_owned()),
        );
        map.insert(
            "kind".to_owned(),
            serde_json::Value::String(self.kind.clone()),
        );
        map.insert(
            "message".to_owned(),
            serde_json::Value::String(self.message.clone()),
        );
        map.insert(
            "path".to_owned(),
            match &self.path {
                Some(path) => serde_json::Value::String(path.clone()),
                None => serde_json::Value::Null,
            },
        );
        serde_json::Value::Object(map)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.kind.is_empty() {
            write!(formatter, "{}", self.message)
        } else {
            write!(formatter, "{} {}", self.kind, self.message)
        }
    }
}

impl std::error::Error for Error {}

/// `Result<T, Error>` の別名（§19.2.2「すべての操作は `Result<T, Error>` を返す」）。
pub type Result<T> = std::result::Result<T, Error>;
