//! Plain-text migration into a fresh project directory (§14.6).
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{Cli, Exit, Paint, Stream};
use crate::adapters::storage_fs::{FsGate, FsStorage};
use crate::core::migration::{self, SourceUnit};
use crate::core::policy::ProjectPath;
use crate::core::text::encoding;
use crate::ports::env::Env;

pub fn run<E: Env>(cli: &mut Cli<'_, E>, inputs: &[PathBuf], dir: &Path, preview: bool) -> Exit {
    let target = cli.absolute(dir);
    let result = (|| {
        let mut units = Vec::new();
        for input in inputs {
            expand_input(&cli.absolute(input), &mut units)?;
        }
        let title = target
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");
        let book = migration::structure(&migration::analyze(&units));
        let files = migration::render(title, &book).map_err(|e| format!("{e:?}"))?;
        if preview {
            return Ok(Some((book, files)));
        }
        let storage = FsStorage::create_fresh(Arc::new(FsGate::new(cli.env())), &target)
            .map_err(|e| format!("{e:?}"))?;
        for file in files {
            let path = ProjectPath::new(&file.path).map_err(|e| format!("{e:?}"))?;
            storage
                .write_bytes(&path, file.text.as_bytes())
                .map_err(|e| format!("{e:?}"))?;
        }
        Ok::<_, String>(None)
    })();
    match result {
        Ok(Some((book, files))) => {
            cli.headline(Stream::Out, Paint::Success, cli.text("cli.migrate.preview"));
            let (chapters, episodes, scenes, sections) = book.counts();
            let counts = [
                chapters.to_string(),
                episodes.to_string(),
                scenes.to_string(),
                sections.to_string(),
            ];
            let structure = cli.format(
                "cli.migrate.structure",
                &[&counts[0], &counts[1], &counts[2], &counts[3]],
            );
            cli.line(Stream::Out, &structure);
            for file in files {
                cli.line(Stream::Out, &file.path);
                if file.path.starts_with("contents/") {
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

/// Expand one input argument into decoded units: a file as-is, a folder as
/// its direct `.txt` / `.md` children in natural order, the first of which
/// starts a chapter (§14.6). Subfolders are never entered.
fn expand_input(path: &Path, units: &mut Vec<SourceUnit>) -> Result<(), String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.is_file() {
        units.push(read_unit(path, false)?);
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(format!("not a file or folder: {}", path.display()));
    }
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_lowercase();
        let is_text = lower.ends_with(".txt") || lower.ends_with(".md");
        if is_text && entry.file_type().map_err(|e| e.to_string())?.is_file() {
            names.push(name);
        }
    }
    if names.is_empty() {
        return Err(format!("no .txt / .md files in folder: {}", path.display()));
    }
    names.sort_by(|a, b| migration::natural_cmp(a, b));
    for (index, name) in names.iter().enumerate() {
        units.push(read_unit(&path.join(name), index == 0)?);
    }
    Ok(())
}

fn read_unit(path: &Path, starts_chapter: bool) -> Result<SourceUnit, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let decoded = encoding::decode(&bytes).map_err(|e| format!("{}: {e:?}", path.display()))?;
    Ok(SourceUnit {
        name: path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        text: decoded.text,
        starts_chapter,
    })
}
