//! novnow の起動（§14.3）。ここは薄く保つ: 判定とサブコマンドは `novnow::cli`、GUI は M7 の `ui`。
//!
//! - Windows の GUI 付きビルドは GUI サブシステム（`windows_subsystem = "windows"`）。エクスプローラから
//!   起動しても**コンソールウィンドウを作らない**（`AllocConsole` は使わない）。端末から起動された
//!   ときだけ `AttachConsole(ATTACH_PARENT_PROCESS)` で親のコンソールにつなぎ、CLI として振る舞う。
//! - 実プロセスの標準入出力・環境変数・カレントディレクトリを `cli::Cli` に渡すのはここだけ。
//! - Web（trunk の `data-bin="novnow"`、§13.3）も同じ実行ファイルを入口にする（`ui` feature のとき
//!   `eframe::WebRunner` を起動する。M7）。
//! - GUI は `ui` feature のビルドだけが持つ（`novnow::cli::GUI_AVAILABLE`）。持たないビルドで GUI を
//!   求められたら、案内を出して終了コード 2（§14.2.5）。

#![cfg_attr(all(windows, feature = "ui"), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    native::main()
}

/// Web の入口（`ui` feature のときだけ GUI を起動する）。
#[cfg(all(target_arch = "wasm32", feature = "ui"))]
fn main() {
    novnow::ui::web::start();
}

/// Web の入口（GUI を含まないビルドでは何もしない）。
#[cfg(all(target_arch = "wasm32", not(feature = "ui")))]
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
            Outcome::Gui(project) => gui(&mut cli, project),
        };
        ExitCode::from(exit.code())
    }

    /// GUI を起動する（`ui` feature のビルドだけ。§14.3）。
    #[cfg(feature = "ui")]
    fn gui<E: novnow::ports::env::Env>(
        cli: &mut Cli<'_, E>,
        project: Option<PathBuf>,
    ) -> cli::Exit {
        match novnow::ui::app::run(project) {
            Ok(()) => cli::Exit::Ok,
            Err(message) => {
                cli.gui_failed(&message);
                cli::Exit::Io
            }
        }
    }

    /// GUI を含まないビルド（既定の feature）では案内を出して終了コード 2（§14.2.5）。
    #[cfg(not(feature = "ui"))]
    fn gui<E: novnow::ports::env::Env>(
        cli: &mut Cli<'_, E>,
        _project: Option<PathBuf>,
    ) -> cli::Exit {
        cli.gui_unavailable()
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
