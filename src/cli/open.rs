//! `novnow <dir>` の起動前の検査（§14.2）と、その表示（§14.2.2〜§14.2.4）。`check` / `export` /
//! `mcp` も同じ検査と表示を使う。
//!
//! 検査は文字コードを読まない `project_load::listing` ＋ `validate` で足りる（`PLANS.md` §5.7
//! 作業項目 7）。問題があれば**標準エラーに出して終了コードを返すだけ**で、他には何もしない
//! （GUI もダイアログも出さず、履歴にも書かず、ディレクトリも作らない。§14.2.3）。
//!
//! | 状況 | 表示 | 終了コード |
//! |---|---|---|
//! | 見つからない | 見出し・対象・構造の解説・`novnow new <dir>` | 3 |
//! | ディレクトリでない | 見出し・対象 | 3 |
//! | 必須のものが欠けている | §14.2.2 / §14.2.3 そのまま | 3 |
//! | v0 形式 | §14.2.4 | 3 |
//! | 版が新しすぎる | 見出し・対象・更新の案内 | 3 |
//! | 読めない（権限など） | 見出し（OS のメッセージ）・対象 | 4 |
//! | UTF-8 として読めない（`check` だけが調べる） | 見出し・対象・ファイルの一覧 | 4 |

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::adapters::block_on;
use crate::adapters::project_load;
use crate::adapters::storage_fs::{FsGate, FsStorage};
use crate::core::model::project::{
    validate, ProjectError, PROJECT_FILE, PROJECT_STRUCTURE_DOC_URL,
};
use crate::ports::env::Env;
use crate::ports::storage::StorageError;
use crate::ui::i18n;

use super::{Cli, Exit, Paint, Stream};

/// 表示する問題。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem<'p> {
    /// 見つからない。
    NotFound,
    /// ディレクトリでない。
    NotADirectory,
    /// 読めない（OS のメッセージ）。
    Unreadable(String),
    /// §4.3 の判定に落ちた。
    Project(&'p ProjectError),
}

/// §14.2 の起動前の検査。通ればプロジェクトのルート（解決済みの絶対パス）を返す。
pub fn precheck<E: Env>(cli: &mut Cli<'_, E>, dir: &Path) -> Result<PathBuf, Exit> {
    let storage = open_storage(cli, dir)?;
    let listing = match block_on(project_load::listing(&storage)) {
        Ok(listing) => listing,
        Err(error) => {
            let message = describe(cli, &error);
            return Err(report(cli, storage.root(), Problem::Unreadable(message)));
        }
    };
    if let Err(error) = validate(&listing) {
        return Err(report(cli, storage.root(), Problem::Project(&error)));
    }
    Ok(storage.root().to_path_buf())
}

/// 引数のディレクトリを読み取り用に開く（関門を作り、`FsStorage::open` で許可を切り替えるだけで、
/// 何も書かない）。見つからない・ディレクトリでない・読めないときは表示して終了コードを返す。
pub fn open_storage<E: Env>(cli: &mut Cli<'_, E>, dir: &Path) -> Result<FsStorage, Exit> {
    let target = cli.absolute(dir);
    match fs::metadata(&target) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Err(report(cli, &target, Problem::NotADirectory)),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(report(cli, &target, Problem::NotFound));
        }
        Err(error) => {
            return Err(report(cli, &target, Problem::Unreadable(error.to_string())));
        }
    }
    let gate = Arc::new(FsGate::new(cli.env()));
    FsStorage::open(gate, &target).map_err(|error| {
        let message = describe(cli, &error);
        report(cli, &target, Problem::Unreadable(message))
    })
}

/// 問題を標準エラーに出して、終了コードを返す（§14.2.5）。`target` は表示する対象。
pub fn report<E: Env>(cli: &mut Cli<'_, E>, target: &Path, problem: Problem<'_>) -> Exit {
    let err = Stream::Err;
    match problem {
        Problem::NotFound => {
            headline(cli, cli.text("cli.error.not_found"), target);
            cli.blank(err);
            cli.hint(err, "cli.hint.structure_doc", PROJECT_STRUCTURE_DOC_URL);
            let new = format!("novnow new {}", cli.command_path(target));
            cli.hint(err, "cli.hint.create_here", &new);
            Exit::Project
        }
        Problem::NotADirectory => {
            headline(cli, cli.text("cli.error.not_a_directory"), target);
            Exit::Project
        }
        Problem::Unreadable(message) => {
            let headline_text = cli.format("cli.error.unreadable", &[&message]);
            headline(cli, &headline_text, target);
            Exit::Io
        }
        Problem::Project(ProjectError::Incomplete { missing }) => {
            // §14.2.2 / §14.2.3 の文面そのまま（tests/cli.rs が仕様書と突き合わせる）
            headline(cli, cli.text("cli.error.incomplete"), target);
            cli.blank(err);
            let items: Vec<String> = missing
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            cli.list(err, "cli.error.missing_items", &items);
            cli.blank(err);
            cli.hint(err, "cli.hint.structure_doc", PROJECT_STRUCTURE_DOC_URL);
            let new = if cli.is_cwd(target) {
                "novnow new".to_owned()
            } else {
                format!("novnow new {}", cli.command_path(target))
            };
            cli.hint(err, "cli.hint.create_here", &new);
            Exit::Project
        }
        Problem::Project(ProjectError::LegacyV0 { detected }) => {
            // §14.2.4
            let message = if detected == PROJECT_FILE {
                cli.text("cli.error.legacy_schema").to_owned()
            } else {
                cli.format("cli.error.legacy_v0", &[detected])
            };
            headline(cli, &message, target);
            cli.blank(err);
            let migrate = format!("novnow migrate {}", cli.command_path(target));
            cli.hint(err, "cli.hint.migrate", &migrate);
            cli.hint(err, "cli.hint.structure_doc", PROJECT_STRUCTURE_DOC_URL);
            Exit::Project
        }
        Problem::Project(ProjectError::TooNew { found }) => {
            let message = cli.format("cli.error.too_new", &[&found.to_string()]);
            headline(cli, &message, target);
            cli.blank(err);
            let update = cli.text("cli.hint.update");
            cli.line(err, update);
            Exit::Project
        }
        Problem::Project(ProjectError::Encoding { files }) => {
            headline(cli, cli.text("cli.error.encoding"), target);
            cli.blank(err);
            let items: Vec<String> = files
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            cli.list(err, "cli.label.invalid_utf8", &items);
            Exit::Io
        }
    }
}

/// `novnow: <見出し>` と `  対象: <path>`。
fn headline<E: Env>(cli: &mut Cli<'_, E>, message: &str, target: &Path) {
    cli.headline(Stream::Err, Paint::Error, message);
    cli.target(Stream::Err, target);
}

/// ストレージの失敗を 1 行にする（OS のメッセージはそのまま。E060 はカタログの文言）。
pub fn describe<E: Env>(cli: &Cli<'_, E>, error: &StorageError) -> String {
    match error {
        StorageError::Io { path, message, .. } => format!("{path}: {message}"),
        StorageError::Policy(violation) => i18n::diagnostic(cli.locale(), &violation.diagnostic()),
        StorageError::NotFound(path) | StorageError::Encoding(path) => path.as_str().to_owned(),
        other => format!("{other:?}"),
    }
}
