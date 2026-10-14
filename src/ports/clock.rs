//! 時刻のポート（§3.1・§3.2「core は時刻を読まない」）。
//!
//! 実装は `adapters::clock_system`。テストは固定の時刻を返す実装を使う。

/// 時計。
pub trait Clock {
    /// 今の UNIX 時刻（UTC、ミリ秒）。
    fn now_millis(&self) -> i64;

    /// ローカル時刻の UTC からのずれ（分。東京なら `540`）。分からない環境では `0`（UTC）。
    ///
    /// ネイティブ版は標準ライブラリにタイムゾーンを得る API が無いので `0` を返す
    /// （`PLANS.md` §9.2 B-92。`opened_at` は順序のためだけの値で UTC でよい、§13.2.2 第 2.9 版）。
    fn utc_offset_minutes(&self) -> i32 {
        0
    }
}
