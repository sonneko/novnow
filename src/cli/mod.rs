//! CLI（§14、`PLANS.md` §5.7 M6）。起動の判定（§14.3）とサブコマンド（§14.1）。
//!
//! `main.rs` は薄く保ち（Windows のコンソールへの接続と、実プロセスの標準入出力・環境・カレント
//! ディレクトリを渡すだけ）、中身はここに置く。テストは [`Cli`] にバッファと偽の `Env` を渡して、
//! プロセスの状態を書き換えずに走らせる（`tests/cli.rs`）。
//!
//! # 約束
//!
//! - **§14.2 の検査に落ちたら「他には何もしない」**: 標準エラーに出して終了する。GUI もダイアログも
//!   出さず、履歴にも書かず、ディレクトリも作らない（指示 2）。
//! - ファイルを書くのは `novnow new` と `novnow export` だけで、`FsGate` / `FsStorage` を通る
//!   （§13.1.2。`export --out <パス>` は §13.1.1 の特例の口 `FsGate::write_chosen_export`）。
//!   履歴（`RecentFile::push`）には書かない（積むのは GUI がプロジェクトを開けたとき、§14.4）。
//! - 文言はすべて `ui::i18n` のカタログ（§12.2）。言語は `NOVNOW_LANG` → OS のロケール → `ja`
//!   （§14.2.3。`ui.json` は読まない）。
//! - 色は出力先が端末で、`NO_COLOR` が無く、Windows でないときだけ（§14.2.3）。
//!
//! 判断の記録は `PLANS.md` §9.2 B-100〜B-107。

pub mod check;
pub mod export;
pub mod import;
pub mod mcp;
pub mod migrate;
pub mod new;
pub mod open;

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::core::model::project::PROJECT_STRUCTURE_DOC_URL;
use crate::core::policy::write_policy::normalize;
use crate::ports::env::{Env, Os};
use crate::ui::i18n::{self, Locale};

/// novnow の版（`--version`）。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// サブコマンド（§14.1）。第 1 引数がこのどれかと**完全一致**したときだけサブコマンドになる。
pub const SUBCOMMANDS: [&str; 6] = ["new", "check", "export", "mcp", "import", "migrate"];

/// この実行ファイルが GUI を持つか（`ui` feature のビルドだけ。M7）。`novnow new` の案内
/// （開く / 検査する）と、GUI を含まないビルドでの GUI 起動の拒否（§14.2.5）に使う。
pub const GUI_AVAILABLE: bool = cfg!(feature = "ui");

// ============================================================ 終了コード

/// 終了コード（§14.2.5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// 0: 正常終了。
    Ok,
    /// 2: 使い方の誤り（引数不足・未知のオプション・GUI を含まないビルドで GUI を求められた）。
    Usage,
    /// 3: プロジェクトが不完全 / スキーマが新しすぎる（見つからない・ディレクトリでないも含む）。
    Project,
    /// 4: I/O エラー（権限・不正 UTF-8・書き込みの失敗）。
    Io,
    /// 5: `check` が E 診断を検出（CI 用）。
    Diagnostics,
}

impl Exit {
    /// プロセスの終了コード。
    pub fn code(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Usage => 2,
            Self::Project => 3,
            Self::Io => 4,
            Self::Diagnostics => 5,
        }
    }
}

// ============================================================ 起動の判定（§14.3）

/// 標準入出力が端末につながっているか（§14.3 の判定と色付け）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Terminal {
    /// 標準入力。
    pub stdin: bool,
    /// 標準出力。
    pub stdout: bool,
    /// 標準エラー。
    pub stderr: bool,
}

impl Terminal {
    /// このプロセスの標準入出力を調べる。
    pub fn detect() -> Self {
        use std::io::IsTerminal;
        Self {
            stdin: std::io::stdin().is_terminal(),
            stdout: std::io::stdout().is_terminal(),
            stderr: std::io::stderr().is_terminal(),
        }
    }
}

/// 起動のしかた（§14.3 の `Launch` に、使い方・版・引数の誤りを足したもの）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// `--help` / `-h`: 使い方を標準出力に出して 0。
    Help,
    /// `--version` / `-V`: 版を標準出力に出して 0。
    Version,
    /// 端末から引数なしで起動された: GUI を出さず、使い方を標準エラーに出して 2（§14.3）。
    Usage,
    /// GUI（`--gui`、または引数なしで端末につながっていない＝ GUI から起動された）。
    Gui,
    /// `novnow <dir>`: §14.2 の検査をしてから GUI で開く。
    Open(PathBuf),
    /// サブコマンド。
    Command(Command),
    /// 引数の誤り（2）。
    Error(ArgError),
}

/// サブコマンド（§14.1）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `novnow new [dir]`（既定 `.`）。
    New(PathBuf),
    /// `novnow check <dir>`。
    Check(PathBuf),
    /// `novnow export <dir> [options]`（§17.2。オプションは `cli::export` が読む）。
    Export {
        /// プロジェクト。
        dir: PathBuf,
        /// `<dir>` より後ろの引数（§17.2 のオプション）。
        options: Vec<OsString>,
    },
    /// `novnow import <zip> <dir>`。
    Import { zip: PathBuf, dir: PathBuf },
    /// `novnow mcp <dir>`（中身は M10）。
    Mcp(PathBuf),
    Migrate {
        source: PathBuf,
        dir: PathBuf,
        preview: bool,
    },
}

impl Command {
    /// サブコマンドの名前。
    pub fn name(&self) -> &'static str {
        match self {
            Self::New(_) => "new",
            Self::Check(_) => "check",
            Self::Export { .. } => "export",
            Self::Mcp(_) => "mcp",
            Self::Migrate { .. } => "migrate",
            Self::Import { .. } => "import",
        }
    }
}

/// 引数の誤り（終了コード 2）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgError {
    /// 知らないオプション（フラグを取るのは `export` だけ、§14.1）。
    UnknownOption(String),
    /// 余分な引数。
    UnexpectedArgument(String),
    /// ディレクトリが要るサブコマンドに無い。
    MissingDir(&'static str),
    /// 値を取るオプションに値が無い（`novnow export . --range`）。
    MissingValue(String),
    /// `--range` の値が範囲式として読めない（§17.2）。
    BadRange(String),
    /// `--state` の値が `draft` / `review` / `fixed` のどれでもない（§17.2）。
    BadState(String),
}

/// 起動のしかたを決める（§14.3。**純粋**: 引数と端末かどうかだけを見る）。
///
/// 順は固定（`PLANS.md` §5.7 作業項目 1）: `--gui` → `--help` → `--version` → サブコマンド（第 1 引数の
/// 完全一致）→ 知らないオプション → 位置引数 → 引数なしなら、端末につながっていれば使い方（2）、
/// いなければ GUI。`--gui` / `--help` / `--version` はどの位置にあっても効く。
pub fn decide(args: &[OsString], terminal: Terminal) -> Launch {
    let has = |flag: &str| args.iter().any(|arg| arg == flag);
    if has("--gui") {
        return Launch::Gui;
    }
    if has("--help") || has("-h") {
        return Launch::Help;
    }
    if has("--version") || has("-V") {
        return Launch::Version;
    }
    let Some(first) = args.first() else {
        return if terminal.stdin || terminal.stdout {
            Launch::Usage
        } else {
            Launch::Gui
        };
    };
    if let Some(name) = SUBCOMMANDS.into_iter().find(|name| first == *name) {
        return subcommand(name, &args[1..]);
    }
    if let Some(option) = as_option(first) {
        return Launch::Error(ArgError::UnknownOption(option));
    }
    if let Some(extra) = args.get(1) {
        return Launch::Error(ArgError::UnexpectedArgument(lossy(extra)));
    }
    Launch::Open(PathBuf::from(first))
}

/// サブコマンドの引数を読む。
fn subcommand(name: &'static str, rest: &[OsString]) -> Launch {
    if name == "migrate" {
        let (rest, preview) = if rest.last().is_some_and(|arg| arg == "--preview") {
            (&rest[..rest.len() - 1], true)
        } else {
            (rest, false)
        };
        return match rest {
            [source, dir] if as_option(source).is_none() && as_option(dir).is_none() => {
                Launch::Command(Command::Migrate {
                    source: source.into(),
                    dir: dir.into(),
                    preview,
                })
            }
            _ => Launch::Error(ArgError::MissingDir("migrate")),
        };
    }
    if name == "import" {
        return match rest {
            [zip, dir] if as_option(zip).is_none() && as_option(dir).is_none() => {
                Launch::Command(Command::Import {
                    zip: PathBuf::from(zip),
                    dir: PathBuf::from(dir),
                })
            }
            [zip, ..] if as_option(zip).is_some() => {
                Launch::Error(ArgError::UnknownOption(lossy(zip)))
            }
            [_, dir, ..] if as_option(dir).is_some() => {
                Launch::Error(ArgError::UnknownOption(lossy(dir)))
            }
            [_, _, extra, ..] => Launch::Error(ArgError::UnexpectedArgument(lossy(extra))),
            _ => Launch::Error(ArgError::MissingDir("import")),
        };
    }
    if name == "export" {
        // フラグを取るのは export だけ（§14.1）。ここでは最初の引数をディレクトリとし、
        // 残りはそのまま渡す（読むのは `cli::export`）
        return match rest.first() {
            Some(dir) if as_option(dir).is_none() => Launch::Command(Command::Export {
                dir: PathBuf::from(dir),
                options: rest[1..].to_vec(),
            }),
            _ => Launch::Error(ArgError::MissingDir("export")),
        };
    }
    if let Some(option) = rest.iter().find_map(|arg| as_option(arg)) {
        return Launch::Error(ArgError::UnknownOption(option));
    }
    if let Some(extra) = rest.get(1) {
        return Launch::Error(ArgError::UnexpectedArgument(lossy(extra)));
    }
    let dir = rest.first().map(PathBuf::from);
    let command = match (name, dir) {
        ("new", dir) => Command::New(dir.unwrap_or_else(|| PathBuf::from("."))),
        (_, None) => return Launch::Error(ArgError::MissingDir(name)),
        ("check", Some(dir)) => Command::Check(dir),
        (_, Some(dir)) => Command::Mcp(dir),
    };
    Launch::Command(command)
}

/// `-` で始まる 2 文字以上の引数（`-` だけはパスとして扱う）。
fn as_option(arg: &OsStr) -> Option<String> {
    let text = arg.to_string_lossy();
    (text.starts_with('-') && text.len() > 1).then(|| text.into_owned())
}

fn lossy(arg: &OsStr) -> String {
    arg.to_string_lossy().into_owned()
}

// ============================================================ 実行

/// [`run`] の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// この終了コードで終わる。
    Exit(Exit),
    /// GUI を起動する（`Some` は §14.2 の検査を通ったプロジェクトのルート）。GUI を含まない
    /// ビルドでは `main.rs` が [`Cli::gui_unavailable`] を呼ぶ。
    Gui(Option<PathBuf>),
}

/// 出力先。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// 標準出力（使い方・版・`check` の診断・`new` の結果）。
    Out,
    /// 標準エラー（失敗の説明）。
    Err,
}

/// 色（ANSI のエスケープ。[`Cli::paint`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    /// 失敗の見出し・E 診断。
    Error,
    /// W 診断。
    Warning,
    /// I 診断。
    Info,
    /// 成功の見出し。
    Success,
}

impl Paint {
    fn code(self) -> &'static str {
        match self {
            Self::Error => "1;31",
            Self::Warning => "1;33",
            Self::Info => "1;36",
            Self::Success => "1;32",
        }
    }
}

/// CLI の実行環境（環境変数・出力先・カレントディレクトリ・端末かどうか）。
pub struct Cli<'a, E: Env> {
    env: &'a E,
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    /// 標準入力の代わり（`novnow mcp` のテスト。無ければ標準入力を読む）。
    input: Option<&'a mut dyn std::io::BufRead>,
    cwd: PathBuf,
    terminal: Terminal,
    locale: Locale,
}

/// 引数を読んで実行する。
pub fn run<E: Env>(args: &[OsString], cli: &mut Cli<'_, E>) -> Outcome {
    match decide(args, cli.terminal) {
        Launch::Help => {
            let usage = cli.usage();
            cli.write(Stream::Out, &usage);
            Outcome::Exit(Exit::Ok)
        }
        Launch::Version => {
            cli.line(Stream::Out, &format!("novnow {VERSION}"));
            Outcome::Exit(Exit::Ok)
        }
        Launch::Usage => {
            let usage = cli.usage();
            cli.write(Stream::Err, &usage);
            Outcome::Exit(Exit::Usage)
        }
        Launch::Gui => Outcome::Gui(None),
        Launch::Open(dir) => match open::precheck(cli, &dir) {
            Ok(root) => Outcome::Gui(Some(root)),
            Err(exit) => Outcome::Exit(exit),
        },
        Launch::Command(command) => Outcome::Exit(match &command {
            Command::New(dir) => new::run(cli, dir),
            Command::Check(dir) => check::run(cli, dir),
            Command::Export { dir, options } => export::run(cli, dir, options),
            Command::Mcp(dir) => mcp::run(cli, dir),
            Command::Import { zip, dir } => import::run(cli, zip, dir),
            Command::Migrate {
                source,
                dir,
                preview,
            } => migrate::run(cli, source, dir, *preview),
        }),
        Launch::Error(error) => Outcome::Exit(cli.arg_error(&error)),
    }
}

impl<'a, E: Env> Cli<'a, E> {
    /// 作る。言語は §12.4 の順（`ui.json` は読まない。§14.2.3）で決める。
    pub fn new(
        env: &'a E,
        out: &'a mut dyn Write,
        err: &'a mut dyn Write,
        cwd: PathBuf,
        terminal: Terminal,
    ) -> Self {
        let locale = Locale::detect(None, env);
        Self {
            env,
            out,
            err,
            input: None,
            cwd,
            terminal,
            locale,
        }
    }

    /// 標準入力の代わりを渡す（`novnow mcp` をプロセスの中で試すとき）。
    pub fn set_input(&mut self, input: &'a mut dyn std::io::BufRead) {
        self.input = Some(input);
    }

    /// 標準入力の代わり（一度だけ取れる）。
    pub fn take_input(&mut self) -> Option<&'a mut dyn std::io::BufRead> {
        self.input.take()
    }

    /// 文言の言語を差し替える（GUI がプロジェクト選択モーダルに §14.2.2 の表示を流用するとき、
    /// 画面の言語に合わせる。§14.4）。
    pub fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
    }

    /// 文言の言語。
    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// 実行環境。
    pub fn env(&self) -> &'a E {
        self.env
    }

    /// カレントディレクトリ（相対パスの基準）。
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// GUI を含まないビルドで GUI を求められた（M6 はすべてのビルド）。説明して 2 を返す。
    pub fn gui_unavailable(&mut self) -> Exit {
        let message = self.text("cli.error.no_gui");
        self.headline(Stream::Err, Paint::Error, message);
        self.help_hint();
        Exit::Usage
    }

    /// GUI（eframe）を起動できなかった（グラフィックスの初期化に失敗したなど）。終了コードは呼び出し側。
    pub fn gui_failed(&mut self, detail: &str) {
        let message = self.format("cli.error.gui_failed", &[detail]);
        self.headline(Stream::Err, Paint::Error, &message);
    }

    // ---------------------------------------------------------------- 文言

    /// カタログの文言。
    pub fn text(&self, key: &'static str) -> &'static str {
        i18n::text(self.locale, key)
    }

    /// カタログの文言にプレースホルダの値を埋める。
    pub fn format(&self, key: &str, args: &[&str]) -> String {
        i18n::format(self.locale, key, args)
    }

    /// 使い方（`--help` と、端末から引数なしで起動されたとき）。`new` という名前のディレクトリの
    /// 開き方を必ず書く（§14.1）。
    pub fn usage(&self) -> String {
        const ROWS: [(&str, &str); 10] = [
            ("novnow <dir>", "cli.usage.open"),
            ("novnow new [dir]", "cli.usage.new"),
            ("novnow check <dir>", "cli.usage.check"),
            ("novnow export <dir> [options]", "cli.usage.export"),
            ("novnow mcp <dir>", "cli.usage.mcp"),
            ("novnow import <zip> <dir>", "cli.usage.import"),
            (
                "novnow migrate <text> <dir> [--preview]",
                "cli.usage.migrate",
            ),
            ("novnow --gui", "cli.usage.gui"),
            ("novnow --help | -h", "cli.usage.help"),
            ("novnow --version | -V", "cli.usage.version"),
        ];
        let mut text = format!(
            "novnow {VERSION} - {}\n\n{}:\n",
            self.text("cli.usage.tagline"),
            self.text("cli.usage.heading")
        );
        for (synopsis, key) in ROWS {
            text.push_str(&format!("  {synopsis:<32}{}\n", self.text(key)));
        }
        for key in [
            "cli.usage.export_options",
            "cli.usage.new_dir",
            "cli.usage.no_args",
            "cli.usage.exit_codes",
        ] {
            text.push('\n');
            text.push_str(self.text(key));
        }
        text.push_str(&format!(
            "\n\n{}: {PROJECT_STRUCTURE_DOC_URL}\n",
            self.text("cli.hint.structure_doc")
        ));
        text
    }

    // ---------------------------------------------------------------- パス

    /// 引数のディレクトリを絶対パスにする（相対ならカレントディレクトリ基準。字句だけで、存在は問わない）。
    pub fn absolute(&self, dir: &Path) -> PathBuf {
        let joined = if dir.is_absolute() {
            dir.to_path_buf()
        } else {
            self.cwd.join(dir)
        };
        normalize(&joined).unwrap_or(joined)
    }

    /// `target` がカレントディレクトリそのものか（`novnow new` の案内でパスを省くため）。
    pub fn is_cwd(&self, target: &Path) -> bool {
        let canonical = |path: &Path| std::fs::canonicalize(path).ok();
        match (canonical(target), canonical(&self.cwd)) {
            (Some(target), Some(cwd)) => target == cwd,
            _ => false,
        }
    }

    /// コマンドの例に書くパス（表示用に整え、空白や記号を含めば引用符で囲む）。
    pub fn command_path(&self, path: &Path) -> String {
        shell_quote(&display_path(path), self.env.os())
    }

    // ---------------------------------------------------------------- 出力

    /// 色を付けてよいか（§14.2.3: 端末のときだけ・`NO_COLOR` を尊重。Windows は付けない）。
    pub fn color(&self, stream: Stream) -> bool {
        let tty = match stream {
            Stream::Out => self.terminal.stdout,
            Stream::Err => self.terminal.stderr,
        };
        tty && self.env.var("NO_COLOR").is_none() && self.env.os() != Os::Windows
    }

    /// 色を付ける（付けてよいときだけ）。
    pub fn paint(&self, stream: Stream, paint: Paint, text: &str) -> String {
        if self.color(stream) {
            format!("\u{1b}[{}m{text}\u{1b}[0m", paint.code())
        } else {
            text.to_owned()
        }
    }

    /// そのまま書く（書けなくても止まらない。壊れたパイプ等は報告のしようが無い）。
    pub fn write(&mut self, stream: Stream, text: &str) {
        let sink: &mut dyn Write = match stream {
            Stream::Out => &mut *self.out,
            Stream::Err => &mut *self.err,
        };
        let _ = sink.write_all(text.as_bytes());
        let _ = sink.flush();
    }

    /// 1 行書く。
    pub fn line(&mut self, stream: Stream, text: &str) {
        self.write(stream, &format!("{text}\n"));
    }

    /// 空行。
    pub fn blank(&mut self, stream: Stream) {
        self.write(stream, "\n");
    }

    /// `novnow: <message>`（§14.2.2 の 1 行目）。
    pub fn headline(&mut self, stream: Stream, paint: Paint, message: &str) {
        let prefix = self.paint(stream, paint, "novnow:");
        self.line(stream, &format!("{prefix} {message}"));
    }

    /// `  対象: <path>`（§14.2.2 の 2 行目）。
    pub fn target(&mut self, stream: Stream, path: &Path) {
        let label = self.text("cli.label.target");
        self.line(stream, &format!("  {label}: {}", display_path(path)));
    }

    /// `<見出し>:` と `  - <項目>` の並び（§14.2.2 の「不足しているもの」）。
    pub fn list(&mut self, stream: Stream, heading: &'static str, items: &[String]) {
        let heading = self.text(heading);
        self.line(stream, &format!("{heading}:"));
        for item in items {
            self.line(stream, &format!("  - {item}"));
        }
    }

    /// `<案内>: <値>`（§14.2.2 の末尾 2 行）。
    pub fn hint(&mut self, stream: Stream, key: &'static str, value: &str) {
        let label = self.text(key);
        self.line(stream, &format!("{label}: {value}"));
    }

    /// `使い方: novnow --help`。
    fn help_hint(&mut self) {
        self.hint(Stream::Err, "cli.hint.help", "novnow --help");
    }

    /// 引数の誤りを説明して 2 を返す。
    fn arg_error(&mut self, error: &ArgError) -> Exit {
        let message = match error {
            ArgError::UnknownOption(option) => self.format("cli.error.unknown_option", &[option]),
            ArgError::UnexpectedArgument(extra) => {
                self.format("cli.error.unexpected_argument", &[extra])
            }
            ArgError::MissingDir(name) => self.format("cli.error.missing_dir", &[name]),
            ArgError::MissingValue(option) => self.format("cli.error.missing_value", &[option]),
            ArgError::BadRange(value) => self.format("cli.error.bad_range", &[value]),
            ArgError::BadState(value) => self.format("cli.error.bad_state", &[value]),
        };
        self.headline(Stream::Err, Paint::Error, &message);
        self.help_hint();
        Exit::Usage
    }
}

/// 表示用のパス。Windows の `canonicalize` が返す `\\?\C:\…` / `\\?\UNC\…` は普通の形に戻す
/// （コマンドの例としてそのまま貼れるように）。
pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        if rest.as_bytes().get(1) == Some(&b':') {
            return rest.to_owned();
        }
    }
    text.into_owned()
}

/// シェルに貼れるように引用する（空白や記号を含むときだけ）。日本語はそのまま。
pub fn shell_quote(text: &str, os: Os) -> String {
    let special = |ch: char| {
        ch.is_whitespace()
            || "'\"$`&|;<>()*?[]{}!#~".contains(ch)
            || (os != Os::Windows && ch == '\\')
    };
    if !text.is_empty() && !text.chars().any(special) {
        return text.to_owned();
    }
    if os == Os::Windows {
        // Windows のパスに `"` は使えない
        format!("\"{text}\"")
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}
