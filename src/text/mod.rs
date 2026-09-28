//! テキスト管理。
//!
//! * `buffer`     — Ropey ラッパ・カーソル位置管理
//! * `tokenizer`  — カスタム文法のトークナイザ
//! * `incremental`— 行単位のインクリメンタル再トークン化

pub mod buffer;
pub mod incremental;
pub mod tokenizer;
