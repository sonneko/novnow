//! LSP のメソッドと 1:1 で対応する純粋関数（§3.4）。
//!
//! 引数は `(&Document, 位置, &Project)` の組み合わせだけ、戻り値は LSP の構造体に機械的に
//! 写せるプレーンな型（§3.4 MUST）。位置は**ファイル内の byte**（§3.4「core は内部的に
//! byte offset を使う」、`PLANS.md` §9.2 B-1）で、UTF-16 との変換は `text/position.rs` の
//! [`crate::core::text::position::LineTable`] が受け持つ（将来の LSP アダプタと UI の仕事）。
//!
//! **文言は返さない**（§12.2）。表示名が要るものは種別の enum を返し、UI が `ui/i18n/` で引く。
//! 引数の順は §3.4 の表のとおり（`semantic_tokens::full(doc, project)` など。`PLANS.md` §9.2 B-78）。

pub mod code_action;
pub mod completion;
pub mod definition;
pub mod document_symbol;
pub mod folding;
pub mod formatting;
pub mod hover;
pub mod inlay_hint;
pub mod project;
pub mod references;
pub mod rename;
pub mod semantic_tokens;

pub use self::project::Project;
