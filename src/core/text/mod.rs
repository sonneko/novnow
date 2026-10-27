/// Shift_JIS の読み替えと貼り付けの文字化け判定（§14.6、Issue #29 X1）。
pub mod encoding;
pub mod incremental;
pub mod lexer;
pub mod position;
pub mod rope;
/// SHA-256（資料 MD の `checksum`、§16.6。`docs/history.md` B-131）。
pub mod sha256;
