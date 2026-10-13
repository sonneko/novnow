//! `novnow check <dir>`（§14.1）: 妥当性検査と M4 の診断を出して終了する（CI 用）。**何も書かない**。
//!
//! 1. §14.2 と同じく開く（見つからない・ディレクトリでない → 3、読めない → 4）。
//! 2. `project_load::load` で全文書を読み、§4.3 の判定（不備・v0・新しすぎる → 3、不正 UTF-8 → 4。
//!    表示は `novnow <dir>` と同じ）。
//! 3. 全文書に `diagnostics::document`（M4。GUI の波線と同じもの）。1 行 1 件で標準出力に
//!    `パス:行:列: コード 文言`（行と列は 1 始まり、列は文字数。位置を持たない診断は `パス: コード 文言`）。
//!    並びはパス → 位置 → コード。最後に件数の要約を 1 行。
//! 4. E が 1 件でもあれば 5、無ければ 0（W と I は 0 のまま、§14.2.5）。

use std::path::Path;

use crate::adapters::block_on;
use crate::adapters::project_load;
use crate::core::diagnostics::{self, Diagnostic, Severity};
use crate::core::model::document::Document;
use crate::core::text::position::LineTable;
use crate::ports::env::Env;
use crate::ui::i18n;

use super::open::{describe, open_storage, report, Problem};
use super::{Cli, Exit, Paint, Stream};

/// 実行する。
pub fn run<E: Env>(cli: &mut Cli<'_, E>, dir: &Path) -> Exit {
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
    let mut documents: Vec<&Document> = loaded.project.documents().collect();
    documents.sort_by(|a, b| a.path.cmp(&b.path));
    let mut counts = Counts::default();
    for document in &documents {
        for (position, found) in located(document, &loaded.project) {
            counts.add(found.severity());
            let line = render(cli, document, position, &found);
            cli.line(Stream::Out, &line);
        }
    }
    let summary = cli.format(
        "cli.check.summary",
        &[
            &documents.len().to_string(),
            &counts.errors.to_string(),
            &counts.warnings.to_string(),
            &counts.infos.to_string(),
        ],
    );
    if counts.total() > 0 {
        cli.blank(Stream::Out);
    }
    cli.line(Stream::Out, &summary);
    if counts.errors > 0 {
        Exit::Diagnostics
    } else {
        Exit::Ok
    }
}

/// 1 始まりの（行, 列）。列は文字（Unicode スカラー値）で数える。
type Position = Option<(usize, usize)>;

/// 文書の診断に位置を付けて、位置 → コードの順に並べる（位置を持たないものが先）。
fn located(
    document: &Document,
    project: &crate::core::features::Project,
) -> Vec<(Position, Diagnostic)> {
    let text = document.text.as_str();
    let table = LineTable::new(text);
    let mut out: Vec<(Position, Diagnostic)> = diagnostics::document(document, project)
        .into_iter()
        .map(|found| {
            let position = found.range.as_ref().map(|range| {
                let at = table.char_position_of_byte(text, range.start);
                (at.line + 1, at.character + 1)
            });
            (position, found)
        })
        .collect();
    out.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.code.as_str().cmp(b.1.code.as_str()))
    });
    out
}

/// 1 行にする。
fn render<E: Env>(
    cli: &Cli<'_, E>,
    document: &Document,
    position: Position,
    found: &Diagnostic,
) -> String {
    let paint = match found.severity() {
        Severity::Error => Paint::Error,
        Severity::Warning => Paint::Warning,
        Severity::Info => Paint::Info,
    };
    let code = cli.paint(Stream::Out, paint, found.code.as_str());
    let message = i18n::diagnostic(cli.locale(), found);
    match position {
        Some((line, column)) => format!("{}:{line}:{column}: {code} {message}", document.path),
        None => format!("{}: {code} {message}", document.path),
    }
}

/// 重大度ごとの件数。
#[derive(Debug, Default)]
struct Counts {
    errors: usize,
    warnings: usize,
    infos: usize,
}

impl Counts {
    fn add(&mut self, severity: Severity) {
        match severity {
            Severity::Error => self.errors += 1,
            Severity::Warning => self.warnings += 1,
            Severity::Info => self.infos += 1,
        }
    }

    fn total(&self) -> usize {
        self.errors + self.warnings + self.infos
    }
}
