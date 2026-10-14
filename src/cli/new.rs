//! `novnow new [dir]`（§14.5）: 雛形（`core::template`）からプロジェクトを作る。
//!
//! - `dir` の既定は `.`。無ければ作る（親ディレクトリごと。`FsStorage::create`）。
//! - **既存のファイルは絶対に上書きしない**（MUST）。無いファイルだけを作る（`FsStorage::exists`）。
//!   競合が 1 件でもあれば、作ったファイルと上書きしなかったファイルの一覧を標準出力に出す。
//! - 既存のディレクトリが **版が新しすぎる**ときは何も書かない（`PLANS.md` §9.2 B-106）。
//! - 書くのは関門（`FsGate`）を通る `FsStorage` だけ（§13.1.2）。履歴には書かない。

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;

use crate::adapters::block_on;
use crate::adapters::project_load;
use crate::adapters::storage_fs::{FsGate, FsStorage};
use crate::core::model::project::{validate, ProjectError};
use crate::core::policy::ProjectPath;
use crate::core::template;
use crate::ports::env::Env;

use super::open::{describe, report, Problem};
use super::{Cli, Exit, Paint, Stream, GUI_AVAILABLE};

/// 題が決まらないとき（ルートディレクトリ等）の `project.json` の `title`。
const UNTITLED: &str = "untitled";

/// 実行する。
pub fn run<E: Env>(cli: &mut Cli<'_, E>, dir: &Path) -> Exit {
    let target = cli.absolute(dir);
    let gate = Arc::new(FsGate::new(cli.env()));
    match fs::metadata(&target) {
        Ok(metadata) if !metadata.is_dir() => {
            report(cli, &target, Problem::NotADirectory);
            return Exit::Io;
        }
        Ok(_) => {
            if let Err(exit) = refuse_legacy(cli, &gate, &target) {
                return exit;
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return report(cli, &target, Problem::Unreadable(error.to_string())),
    }
    let storage = match FsStorage::create(gate, &target) {
        Ok(storage) => storage,
        Err(error) => return write_failed(cli, &target, &describe(cli, &error)),
    };
    let title = storage
        .root()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| UNTITLED.to_owned());
    let mut created = Vec::new();
    let mut kept = Vec::new();
    for file in template::new_project(&title) {
        let Ok(path) = ProjectPath::new(file.path) else {
            continue;
        };
        if storage.exists(&path) {
            kept.push(file.path.to_owned());
            continue;
        }
        if let Err(error) = storage.write_bytes(&path, file.text.as_bytes()) {
            let message = describe(cli, &error);
            return write_failed(cli, storage.root(), &message);
        }
        created.push(file.path.to_owned());
    }
    finish(cli, storage.root(), &created, &kept);
    Exit::Ok
}

/// 既存のディレクトリが版が新しすぎるなら、何も書かずに表示して終了コードを返す。
fn refuse_legacy<E: Env>(
    cli: &mut Cli<'_, E>,
    gate: &Arc<FsGate>,
    target: &Path,
) -> Result<(), Exit> {
    let storage = match FsStorage::open(gate.clone(), target) {
        Ok(storage) => storage,
        Err(error) => {
            let message = describe(cli, &error);
            return Err(report(cli, target, Problem::Unreadable(message)));
        }
    };
    let listing = match block_on(project_load::listing(&storage)) {
        Ok(listing) => listing,
        Err(error) => {
            let message = describe(cli, &error);
            return Err(report(cli, storage.root(), Problem::Unreadable(message)));
        }
    };
    match validate(&listing) {
        Err(error @ ProjectError::TooNew { .. }) => {
            Err(report(cli, storage.root(), Problem::Project(&error)))
        }
        _ => Ok(()),
    }
}

/// 結果を標準出力に出す（§14.5: 競合があれば両方の一覧）。
fn finish<E: Env>(cli: &mut Cli<'_, E>, root: &Path, created: &[String], kept: &[String]) {
    let out = Stream::Out;
    let headline = if kept.is_empty() {
        "cli.new.created"
    } else if created.is_empty() {
        "cli.new.nothing"
    } else {
        "cli.new.completed"
    };
    let message = cli.text(headline);
    cli.headline(out, Paint::Success, message);
    cli.target(out, root);
    if !kept.is_empty() {
        if !created.is_empty() {
            cli.blank(out);
            cli.list(out, "cli.label.created", created);
        }
        cli.blank(out);
        cli.list(out, "cli.label.kept", kept);
    }
    cli.blank(out);
    let path = if cli.is_cwd(root) {
        ".".to_owned()
    } else {
        cli.command_path(root)
    };
    if GUI_AVAILABLE {
        cli.hint(out, "cli.hint.open", &format!("novnow {path}"));
    } else {
        cli.hint(out, "cli.hint.check", &format!("novnow check {path}"));
    }
}

/// 書けなかった（4）。
fn write_failed<E: Env>(cli: &mut Cli<'_, E>, target: &Path, message: &str) -> Exit {
    let text = cli.format("cli.error.write_failed", &[message]);
    cli.headline(Stream::Err, Paint::Error, &text);
    cli.target(Stream::Err, target);
    Exit::Io
}
