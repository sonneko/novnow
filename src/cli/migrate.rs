//! Explicit plain-text migration into a fresh project directory.
use std::path::Path;
use std::sync::Arc;

use super::{Cli, Exit, Paint, Stream};
use crate::adapters::storage_fs::{FsGate, FsStorage};
use crate::core::{migration, policy::ProjectPath};
use crate::ports::env::Env;

pub fn run<E: Env>(cli: &mut Cli<'_, E>, source: &Path, dir: &Path, preview: bool) -> Exit {
    let source = cli.absolute(source);
    let target = cli.absolute(dir);
    let result = (|| {
        let text = std::fs::read_to_string(&source).map_err(|e| e.to_string())?;
        let title = target
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");
        let files = migration::convert(title, &text).map_err(|e| format!("{e:?}"))?;
        if preview {
            return Ok(Some(files));
        }
        let storage = FsStorage::create_fresh(Arc::new(FsGate::new(cli.env())), &target)
            .map_err(|e| format!("{e:?}"))?;
        for file in files {
            let path = ProjectPath::new(file.path).map_err(|e| format!("{e:?}"))?;
            storage
                .write_bytes(&path, file.text.as_bytes())
                .map_err(|e| format!("{e:?}"))?;
        }
        Ok::<_, String>(None)
    })();
    match result {
        Ok(Some(files)) => {
            cli.headline(Stream::Out, Paint::Success, cli.text("cli.migrate.preview"));
            for file in files {
                cli.line(Stream::Out, file.path);
                if file.path == "contents/01-01.cnov" {
                    cli.line(Stream::Out, &file.text);
                }
            }
            Exit::Ok
        }
        Ok(None) => {
            cli.headline(
                Stream::Out,
                Paint::Success,
                cli.text("cli.migrate.completed"),
            );
            cli.target(Stream::Out, &target);
            Exit::Ok
        }
        Err(error) => {
            let message = cli.format("cli.migrate.failed", &[&error]);
            cli.headline(Stream::Err, Paint::Error, &message);
            Exit::Io
        }
    }
}
