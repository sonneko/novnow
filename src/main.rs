//! 起動エントリポイント（`SPECIFICATIONS.md` §3.3「main.rs 起動のみ」）。
//!
//! M0 の時点では暫定版で、引数に関わらず使用法を表示して終了コード 2（§14.2.5
//! 「使い方の誤り」）で終了する。v0 の UI は `v0-final` タグに保存してあり、GUI は
//! M7 まで未実装（`PLANS.md` §3 の I3「v0 と並走させない」）。
//!
//! 層の依存関係は `src/lib.rs`（`core` / `ports` / `adapters` / `ui`）側が持ち、
//! この bin はそれを呼び出すだけにしておく。

// TODO(M6): §14.3 の判定順（--gui → サブコマンド → 位置引数 → 非TTY → 使用法）と
//           §14.1 のサブコマンド（new / check / export / migrate / mcp）を実装する。
//           Windows の AttachConsole（§14.3 の OS 別要件）もここ。
// TODO(M6): 使用法の文言は §12.2 / §12.3 に従い ui/i18n カタログ（cli.* キー）経由にする。
//           カタログは I6 のとおり M6 の最初に作るので、それまでの暫定ハードコード。
fn main() {
    eprintln!("novnow: 利用方法: novnow <dir>");
    std::process::exit(2);
}
