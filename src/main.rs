//! novnow の起動（§14.3）。ここは薄く保つ: 判定とサブコマンドは `novnow::cli`、GUI は M7 の `ui`。
//!
//! - Windows のリリースビルドは GUI サブシステム（`windows_subsystem = "windows"`）。エクスプローラから
//!   起動しても**コンソールウィンドウを作らない**（`AllocConsole` は使わない）。端末から起動された
//!   ときだけ `AttachConsole(ATTACH_PARENT_PROCESS)` で親のコンソールにつなぎ、CLI として振る舞う。
//! - 実プロセスの標準入出力・環境変数・カレントディレクトリを `cli::Cli` に渡すのはここだけ。
//! - Web（trunk の `data-bin="novnow"`、§13.3）も同じ実行ファイルを入口にする。GUI は M7 なので、
//!   それまでの Web の入口は空。

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    native::main()
}

/// Web の入口（GUI は M7。`eframe::WebRunner` をここから起動する）。
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::process::ExitCode;

    use novnow::adapters::env_system::SystemEnv;
    use novnow::cli::{self, Cli, Outcome, Terminal};

    pub fn main() -> ExitCode {
        #[cfg(windows)]
        attach_parent_console();
        let args: Vec<OsString> = std::env::args_os().skip(1).collect();
        let env = SystemEnv;
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let terminal = Terminal::detect();
        let stdout = std::io::stdout();
        let stderr = std::io::stderr();
        let mut out = stdout.lock();
        let mut err = stderr.lock();
        let mut cli = Cli::new(&env, &mut out, &mut err, cwd, terminal);
        let exit = match cli::run(&args, &mut cli) {
            Outcome::Exit(exit) => exit,
            // GUI は M7（`ui` feature）。それまではどのビルドも GUI を含まない
            Outcome::Gui(_project) => cli.gui_unavailable(),
        };
        ExitCode::from(exit.code())
    }

    /// 端末から起動されていれば、その（親プロセスの）コンソールに標準入出力をつなぐ（§14.3）。
    /// エクスプローラから起動されたときは失敗し、何もしない（コンソールを新しく作らない）。
    #[cfg(windows)]
    fn attach_parent_console() {
        #[link(name = "kernel32")]
        extern "system" {
            fn AttachConsole(process_id: u32) -> i32;
        }
        /// `ATTACH_PARENT_PROCESS`（`(DWORD)-1`）。
        const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
        // SAFETY: 引数は定数で、失敗しても何も起きない（戻り値 0）。
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}
