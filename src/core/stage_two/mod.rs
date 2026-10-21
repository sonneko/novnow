//! Stage-two pure-domain extensions. No clock, file system, UI or network access.
pub mod diagnose;
pub mod local_llm;
pub mod metadata;
pub mod metrics;
pub mod search;
pub mod stock;
pub use self::metadata::{chapter_blurb, inherited, records};
