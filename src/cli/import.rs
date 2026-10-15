//! `novnow import <zip> <dir>` — never overwrite existing project files.
//! Archive names are vetted by the common archive policy; writes go through FsStorage.
use std::path::Path;
use std::sync::Arc;

use crate::adapters::block_on;
use crate::adapters::project_load;
use crate::adapters::storage_fs::{FsGate, FsStorage};
use crate::adapters::zip;
use crate::core::model::project::validate;
use crate::ports::env::Env;

use super::{Cli, Exit, Paint, Stream};

pub fn run<E: Env>(cli: &mut Cli<'_, E>, zip_path: &Path, dir: &Path) -> Exit {
    let zip_path = cli.absolute(zip_path);
    let target = cli.absolute(dir);
    let result = (|| {
        let bytes = std::fs::read(&zip_path).map_err(|e| e.to_string())?;
        let archive = zip::import(&bytes).map_err(|e| format!("{e:?}"))?;
        let gate = Arc::new(FsGate::new(cli.env()));
        let storage = FsStorage::create(gate, &target).map_err(|e| format!("{e:?}"))?;
        let report = zip::import_project(&storage, &archive).map_err(|e| format!("{e:?}"))?;
        Ok::<_, String>((archive, report, storage))
    })();
    let (archive, report, storage) = match result {
        Ok(found) => found,
        Err(error) => {
            let message = cli.format("cli.import.failed", &[&error]);
            cli.headline(Stream::Err, Paint::Error, &message);
            cli.target(Stream::Err, &target);
            return Exit::Io;
        }
    };
    cli.headline(
        Stream::Out,
        Paint::Success,
        cli.text("cli.import.completed"),
    );
    cli.target(Stream::Out, storage.root());
    if !report.written.is_empty() {
        cli.list(
            Stream::Out,
            "cli.label.created",
            &report
                .written
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect::<Vec<_>>(),
        );
    }
    if !report.kept.is_empty() {
        cli.list(
            Stream::Out,
            "cli.label.kept",
            &report
                .kept
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect::<Vec<_>>(),
        );
    }
    if !archive.skipped.is_empty() {
        cli.list(
            Stream::Out,
            "cli.import.skipped",
            &archive
                .skipped
                .iter()
                .map(|item| item.name.clone())
                .collect::<Vec<_>>(),
        );
    }
    match block_on(project_load::listing(&storage)) {
        Ok(listing) => match validate(&listing) {
            Ok(()) => Exit::Ok,
            Err(error) => {
                let message = cli.format("cli.import.incomplete", &[&format!("{error:?}")]);
                cli.headline(Stream::Err, Paint::Warning, &message);
                Exit::Project
            }
        },
        Err(error) => {
            let message = cli.format("cli.import.failed", &[&format!("{error:?}")]);
            cli.headline(Stream::Err, Paint::Error, &message);
            Exit::Io
        }
    }
}
