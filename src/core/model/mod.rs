//! ドメイン型（§3.3 の `model/`）。
//!
//! **依存の向きは `syntax/<形式>.rs` → `model` の 1 本だけ**で、`model` は `syntax` を
//! `use` しない（`docs/history.md` B-29）。行の位置・byte range・round-trip は `syntax` が持ち、
//! ここは解決（M3）・検査（M4）・UI（M6 以降）が扱う**意味のかたまり**を持つ。
//!
//! # serde（`docs/history.md` B-30）
//!
//! - `Serialize` + `Deserialize` の**両方**を付ける（§3.2 は serde / serde_json を許可済み）。
//!   `materials/index.json`（§5.8.2）・`project.toml`（§4.5）・`.novnow/ui.json`（§4.4.1）は
//!   JSON そのものがストレージ形式なので `Deserialize` が要り、ゴールデンテストの
//!   `.expected.json` 比較に `Serialize` が要る。
//! - **未知キー温存**（§4.5 MUST）は `#[serde(flatten)] unknown: BTreeMap<String, Value>`。
//!   `deny_unknown_fields` は**使わない**。
//! - `rename_all` は構造体に付けない（フィールド名は仕様書のキー名にそのまま合わせる）。
//!   ハイフンを含むキー（`default-mode` / `context-hash` / `human-comment` / `promoted-to`）だけ
//!   個別に `rename` する。enum の variant は仕様書の綴り（`propose` 等）に個別に `rename` する。
//! - **閉じた集合の値（`status` / `kind` / `delegation` 等）は文字列で持つ**ことが多い。
//!   serde の enum にすると未知の値 1 つでファイル全体が読めなくなり、§4.3-4
//!   「パース失敗を起動失敗にしない（P4: 作家の原稿を人質に取らない）」に反するため。
//!   型として読む手段は `*_kind()` / [`delegation::Delegation::parse`] などで別に置く。
//! - 既定値は §4.5 / §4.5.1 の表どおり。コンテナ単位の `#[serde(default)]` ＋ 手書きの
//!   `Default` で「キーが欠けても既定値で読める」ようにする。
//! - TOML は i18n カタログ（B-5）だけで、`model` に TOML の型は無い。

pub mod character;
pub mod delegation;
pub mod document;
pub mod idea;
pub mod material;
pub mod mode;
pub mod project;
pub mod role_pattern;
pub mod task;
pub mod word;

pub use self::character::{Character, Relation};
pub use self::delegation::Delegation;
pub use self::document::{Document, DocumentFormat};
pub use self::idea::{Idea, IdeaStatus};
pub use self::material::{
    Condition, ConditionKind, Material, MaterialId, MaterialIndex, MaterialStatus, Source,
    SourceKind,
};
pub use self::mode::Mode;
pub use self::project::{
    validate, AgentLimits, Agents, Export, Lexicon, Listing, LocalLlm, Newline, Project,
    ProjectError, SchemaState, Structure, Writing,
};
pub use self::role_pattern::{Anchor, RolePattern};
pub use self::task::{Initiator, Task, TaskStatus};
pub use self::word::Word;
