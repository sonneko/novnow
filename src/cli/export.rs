//! `novnow export <dir> [--range <範囲>] [--state <状態>] [--out <パス>|-]`（§14.1・§17.2、M9）。
//!
//! 1. オプションを読む（知らないオプション・値の無いオプション・読めない範囲や状態 → 2）。
//!    ② のオプション（`--include-stock` / `--include-blurb` / `--blurb-only` /
//!    `--keep-illustration-memo`）も知らないオプションになる（`PLANS.md` §1.2）。
//! 2. `novnow check` と同じく開いて読む（不備 → 3、読めない → 4）。
//! 3. `core::export`（§17.1）。設定は `project.json` の `export`、形式は `--out` の名前（`.md` なら
//!    Markdown）。当てはまる節が無ければ、何も書かずに標準エラーに知らせて 0。
//! 4. 書く。
//!    - `--out -`: 標準出力へ（ほかには何も出さない）。
//!    - `--out <パス>`: そのファイル 1 個に（§13.1.1 の特例。プロジェクトの外でもよい）。
//!      `ChosenPath::from_command_line` に包み、先に `FsGate::check_chosen_export` で確かめる
//!      （名前が `.txt` / `.md` で終わらない・履歴ファイル → E060 で書かずに 4）。親ディレクトリは
//!      作らない（無ければ 4）。
//!    - 省略: プロジェクトの中の `exports/<title>-<範囲>-<YYYYMMDD-HHMM>.txt`（規則 10 のまま）。
//!      日時は UTC（ネイティブはタイムゾーンが分からない、`PLANS.md` §9.2 B-92・B-123）。
//!
//!    書いたら標準出力に `novnow: 書き出しました（N 節・M 字）` と書き出し先。
//!
//! 判断の記録は `PLANS.md` §9.2 B-122〜B-125。

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::adapters::block_on;
use crate::adapters::clock_system::{compact_stamp, SystemClock};
use crate::adapters::project_load;
use crate::adapters::storage_fs::FsStorage;
use crate::core::export::{self, Format, State};
use crate::core::features::sections;
use crate::core::features::Project;
use crate::core::id::RangeExpr;
use crate::core::policy::ProjectPath;
use crate::ports::clock::Clock;
use crate::ports::dialog::{ChosenPath, ExportKind};
use crate::ports::env::Env;
use crate::ui::i18n;

use super::open::{describe, open_storage, report, Problem};
use super::{display_path, ArgError, Cli, Exit, Paint, Stream};

/// `exports/`（§4.1。プロジェクトの中の既定の書き出し先）。
const EXPORTS_DIR: &str = "exports";

/// 標準出力を表す `--out` の値。
const STDOUT: &str = "-";

/// 書き出し先。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    /// `--out -`。
    Stdout,
    /// `--out <パス>`（書かれたまま。相対ならカレントディレクトリ基準）。
    File(PathBuf),
    /// 省略（`exports/…`）。
    Default,
}

/// `novnow export` のオプション（§17.2）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    /// `--range`（`None` なら全体）。
    pub range: Option<RangeExpr>,
    /// `--state`。
    pub state: Option<State>,
    /// `--out`。
    pub out: Output,
}

impl Args {
    /// `<dir>` より後ろの引数を読む。同じオプションが 2 回あれば後のほうを使う。
    pub fn parse(options: &[OsString]) -> Result<Self, ArgError> {
        let mut args = Self {
            range: None,
            state: None,
            out: Output::Default,
        };
        let mut rest = options.iter().map(|arg| arg.to_string_lossy().into_owned());
        while let Some(arg) = rest.next() {
            let (name, inline) = match arg.split_once('=') {
                Some((name, value)) if name.starts_with("--") => {
                    (name.to_owned(), Some(value.to_owned()))
                }
                _ => (arg.clone(), None),
            };
            if !matches!(name.as_str(), "--range" | "--state" | "--out") {
                return Err(if arg.starts_with('-') && arg.len() > 1 {
                    ArgError::UnknownOption(arg)
                } else {
                    ArgError::UnexpectedArgument(arg)
                });
            }
            // `--range --state fixed` のように次がオプションなら、値が無いとみなす（`-` は値）
            let value = match inline.or_else(|| rest.next()) {
                Some(value) if !value.is_empty() && !value.starts_with("--") => value,
                _ => return Err(ArgError::MissingValue(name)),
            };
            match name.as_str() {
                "--range" => {
                    let range =
                        export::parse_range(&value).map_err(|_| ArgError::BadRange(value))?;
                    args.range = Some(range);
                }
                "--state" => {
                    let state = State::parse(&value).ok_or(ArgError::BadState(value))?;
                    args.state = Some(state);
                }
                _ => {
                    args.out = if value == STDOUT {
                        Output::Stdout
                    } else {
                        Output::File(PathBuf::from(value))
                    };
                }
            }
        }
        Ok(args)
    }
}

/// 実行する。
pub fn run<E: Env>(cli: &mut Cli<'_, E>, dir: &Path, options: &[OsString]) -> Exit {
    let args = match Args::parse(options) {
        Ok(args) => args,
        Err(error) => return cli.arg_error(&error),
    };
    let storage = match open_storage(cli, dir) {
        Ok(storage) => storage,
        Err(exit) => return exit,
    };
    let loaded = match block_on(project_load::load(&storage)) {
        Ok(loaded) => loaded,
        Err(error) => {
            let message = describe(cli, &error);
            return report(cli, storage.root(), Problem::Unreadable(message));
        }
    };
    if let Err(error) = &loaded.validation {
        return report(cli, storage.root(), Problem::Project(error));
    }
    let project = &loaded.project;
    let mut options = export::Options::from_settings(&project.settings().export);
    options.state = args.state;
    if let Output::File(path) = &args.out {
        options.format = Format::of_name(&path.to_string_lossy());
    }
    let chosen = export::select(project, args.range.as_ref(), args.state);
    if chosen.is_empty() {
        let message = cli.text("cli.export.empty");
        cli.headline(Stream::Err, Paint::Warning, message);
        return Exit::Ok;
    }
    let text = export::render(project, &chosen, &options);
    let written = match &args.out {
        Output::Stdout => {
            cli.write(Stream::Out, &text);
            return Exit::Ok;
        }
        Output::File(path) => write_chosen(cli, &storage, path, &text),
        Output::Default => write_default(cli, &storage, project, &args, options.format, &text),
    };
    let target = match written {
        Ok(target) => target,
        Err(exit) => return exit,
    };
    let summary = cli.format(
        "cli.export.written",
        &[
            &chosen.len().to_string(),
            &sections::count_chars(&text).to_string(),
        ],
    );
    cli.headline(Stream::Out, Paint::Success, &summary);
    let label = cli.text("cli.label.output");
    cli.line(
        Stream::Out,
        &format!("  {label}: {}", display_path(&target)),
    );
    Exit::Ok
}

/// `--out <パス>`（§13.1.1 の特例）。書いた絶対パスを返す。
fn write_chosen<E: Env>(
    cli: &mut Cli<'_, E>,
    storage: &FsStorage,
    path: &Path,
    text: &str,
) -> Result<PathBuf, Exit> {
    let target = cli.absolute(path);
    let chosen = ChosenPath::from_command_line(target.clone(), ExportKind::Text);
    // 名前・履歴ファイルの判定を先に（違反を関門の口に渡すと debug ビルドは panic する）
    if let Err(violation) = storage.gate().check_chosen_export(&chosen) {
        let message = i18n::diagnostic(cli.locale(), &violation.diagnostic());
        return Err(write_failed(cli, &target, &message));
    }
    storage
        .gate()
        .write_chosen_export(chosen, text.as_bytes())
        .map_err(|error| {
            let message = describe(cli, &error);
            write_failed(cli, &target, &message)
        })
}

/// 省略時の `exports/<title>-<範囲>-<YYYYMMDD-HHMM>.txt`。書いた絶対パスを返す。
fn write_default<E: Env>(
    cli: &mut Cli<'_, E>,
    storage: &FsStorage,
    project: &Project,
    args: &Args,
    format: Format,
    text: &str,
) -> Result<PathBuf, Exit> {
    let title = match project.settings().title.trim() {
        "" => storage
            .root()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        title => title.to_owned(),
    };
    let clock = SystemClock;
    let stamp = compact_stamp(clock.now_millis(), clock.utc_offset_minutes());
    let range = export::range_label(args.range.as_ref(), project.digits());
    let name = export::file_name(&title, &range, &stamp, format);
    let path = ProjectPath::new(&format!("{EXPORTS_DIR}/{name}")).or_else(|_| {
        let fallback = export::file_name("", &range, &stamp, format);
        ProjectPath::new(&format!("{EXPORTS_DIR}/{fallback}"))
    });
    let path = match path {
        Ok(path) => path,
        Err(error) => {
            let message = format!("{error:?}");
            return Err(write_failed(cli, storage.root(), &message));
        }
    };
    let target = storage.absolute(&path);
    storage
        .write_bytes(&path, text.as_bytes())
        .map(|_| target.clone())
        .map_err(|error| {
            let message = describe(cli, &error);
            write_failed(cli, &target, &message)
        })
}

/// 書けなかった（4）。
fn write_failed<E: Env>(cli: &mut Cli<'_, E>, target: &Path, message: &str) -> Exit {
    let text = cli.format("cli.error.write_failed", &[message]);
    cli.headline(Stream::Err, Paint::Error, &text);
    cli.target(Stream::Err, target);
    Exit::Io
}
