/// 日本語の文字種判定（§6.2.6 / §7.2.2 / §9.4.4 / §14.6.1 が使う）。
pub mod charclass;
/// Shift_JIS の読み替えと貼り付けの文字化け判定（§14.6、Issue #29 X1）。
pub mod encoding;
pub mod incremental;
pub mod lexer;
pub mod position;
pub mod rope;
/// 文の区切り（`。！？`。§9.4.4 の W 系と字数が使う）。
pub mod sentence;
/// SHA-256（資料 MD の `checksum`、§16.6。`docs/history.md` B-131）。
pub mod sha256;
