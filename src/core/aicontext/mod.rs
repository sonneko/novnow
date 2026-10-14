//! LLM に渡すコンテキスト（§15.4）。I/O を持たない（§3.2）。
//!
//! - [`packet`] — ContextPacket の構築（9 ブロックの中身・送る本文・`context-hash`・LLM への要求）。
//! - [`budget`] — 文字数予算（§15.4.2）と切り詰め（§15.4.3）。

pub mod budget;
pub mod packet;
