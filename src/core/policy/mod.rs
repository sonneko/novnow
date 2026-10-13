//! ストレージの規則（§3.3 の `policy/`、仕様 第 2.9 版）。
//!
//! **純粋関数だけ**を置く（§3.2: core は I/O を持たない）。実際の読み書きは `adapters/` が持ち、
//! ここが決めた規則に従う。
//!
//! | ファイル | 規則 | 仕様 |
//! |---|---|---|
//! | [`write_policy`] | 規則 10 の関門の判定（書いてよいのはプロジェクト内と履歴ファイル 1 個）・プロジェクト内の相対パス | §13.1 |
//! | [`recent`] | パス履歴の形（キーはこれ以外を足さない）と上限 32 件 | §13.2 / §13.3.4 |
//! | [`archive`] | Zip に入れるもの・取り込むもの（`.novnow/` とドットファイル、単一ルートの剥がし、`..` の拒否） | §13.5 |
//! | [`external_change`] | ポーリングで見つけた外部変更の検出と反応（Clean は再読込、Dirty は衝突） | §8.5 |

pub mod archive;
pub mod external_change;
pub mod recent;
pub mod write_policy;

pub use self::external_change::{FileEntry, Stamp};
pub use self::write_policy::{
    Allowed, PathError, PolicyViolation, ProjectPath, Violation, WritePolicy,
};
