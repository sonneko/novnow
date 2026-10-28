//! `lang.*`・`diagnostics.*` の実装（§19.7）。LSP の各メソッドと同じ純粋関数を呼ぶ（§19.5）。

use serde_json::{json, Value};

use crate::app::error::{Class, Error, Result};
use crate::app::ops::{apply_plan, commit, gate, write_document, Args};
use crate::app::session::{str_arg, str_or, usize_opt, usize_or, Session};
use crate::core::features::{
    self as features, code_action, completion, definition, document_symbol, folding, formatting,
    hover, inlay_hint, place, project as project_features, references, rename, semantic_tokens,
};
use crate::core::id::PartialSceneId;
use crate::core::model::document::Document;
use crate::core::{diagnostics, diagnostics::Diagnostic};
use crate::ui::i18n::{self, Locale};

/// 対象の文書（`path` が無ければ編集中の文書）。
fn target(session: &mut Session, args: &Args) -> Result<Document> {
    let locale = session.locale;
    session.bench.commit();
    let path = match args.get("path").and_then(Value::as_str) {
        Some(path) if !path.is_empty() => path.to_owned(),
        _ => session
            .bench
            .current()
            .map(|found| found.path.clone())
            .or_else(|| session.bench.slice().map(|slice| slice.path.clone()))
            .ok_or_else(|| {
                Error::new(
                    Class::Usage,
                    i18n::text(locale, "api.error.no_document").to_owned(),
                )
            })?,
    };
    session
        .aggregate()
        .document(&path)
        .cloned()
        .ok_or_else(|| Error::not_found(locale, &path, Some(&path)))
}

/// 診断の JSON（行・列つき。§19.1.7 の `diagnostics`）。
fn diagnostic_json(document: &Document, found: &Diagnostic, locale: Locale) -> Value {
    let starts = project_features::line_starts_of(&document.text);
    let (line, column) = found
        .range
        .as_ref()
        .map(|range| {
            let line = project_features::line_of(&document.text, range.start);
            let column = range.start - starts.get(line).copied().unwrap_or(0);
            (line + 1, column + 1)
        })
        .unwrap_or((0, 0));
    json!({
        "code": found.code.as_str(),
        "column": column,
        "line": line,
        "message": i18n::diagnostic(locale, &found.clone()),
        "path": document.path,
        "severity": crate::app::ops::severity_name(found.severity()),
    })
}

// =========================================================================== lang.*

/// `lang.tokens`（LSP `textDocument/semanticTokens/full`）。
pub fn tokens(session: &mut Session, args: &Args) -> Result<Value> {
    let document = target(session, args)?;
    let items = semantic_tokens::full(&document, session.aggregate());
    let value = serde_json::to_value(&items).unwrap_or(Value::Array(Vec::new()));
    Ok(json!({ "count": items.len(), "path": document.path, "tokens": value }))
}

/// `lang.completion`（LSP `textDocument/completion`）。
pub fn completion_op(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let document = target(session, args)?;
    let pos = usize_or(args, "pos", 0);
    if pos > document.text.len() {
        return Err(Error::usage(locale, "pos"));
    }
    let items = completion::at(&document, pos, session.aggregate());
    let value = serde_json::to_value(&items).unwrap_or(Value::Array(Vec::new()));
    Ok(json!({ "count": items.len(), "items": value, "path": document.path }))
}

/// `lang.hover`（LSP `textDocument/hover`）。
pub fn hover_op(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let document = target(session, args)?;
    let pos = usize_or(args, "pos", 0);
    if pos > document.text.len() {
        return Err(Error::usage(locale, "pos"));
    }
    let found = hover::at(&document, pos, session.aggregate());
    let value = found
        .as_ref()
        .and_then(|found| serde_json::to_value(found).ok())
        .unwrap_or(Value::Null);
    Ok(json!({ "hover": value, "path": document.path }))
}

/// `lang.hints`（LSP `textDocument/inlayHint`）。
pub fn hints(session: &mut Session, args: &Args) -> Result<Value> {
    let document = target(session, args)?;
    let start = usize_or(args, "start", 0);
    let end = usize_opt(args, "end").unwrap_or(document.text.len());
    let items = inlay_hint::range(
        &document,
        start..end.min(document.text.len()),
        session.aggregate(),
    );
    let value = serde_json::to_value(&items).unwrap_or(Value::Array(Vec::new()));
    Ok(json!({ "count": items.len(), "hints": value, "path": document.path }))
}

/// `lang.code_action`（LSP `textDocument/codeAction`。`apply` で当てる）。
pub fn code_action_op(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let document = target(session, args)?;
    let start = usize_or(args, "start", 0);
    let end = usize_or(args, "end", start);
    let items = code_action::at(&document, start..end, session.aggregate());
    if args.get("apply").and_then(Value::as_bool).unwrap_or(false) {
        gate(session, args)?;
        let index = usize_or(args, "index", 0);
        let Some(found) = items.get(index) else {
            return Err(Error::not_found(
                locale,
                &format!("code_action {index}"),
                None,
            ));
        };
        let mut written = Vec::new();
        for change in found.edit.changes.iter() {
            let Some(current) = session.aggregate().document(&change.path) else {
                continue;
            };
            let next = features::apply_edits(&current.text, &change.edits);
            write_document(session, &change.path, next);
            written.push(change.path.clone());
        }
        let saved = commit(session)?;
        return Ok(json!({ "applied": index, "save": saved, "written": written }));
    }
    let value = serde_json::to_value(&items).unwrap_or(Value::Array(Vec::new()));
    Ok(json!({ "actions": value, "count": items.len(), "path": document.path }))
}

/// `lang.definition`（LSP `textDocument/definition`）。
pub fn definition_op(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let document = target(session, args)?;
    let pos = usize_or(args, "pos", 0);
    if pos > document.text.len() {
        return Err(Error::usage(locale, "pos"));
    }
    let found = definition::at(&document, pos, session.aggregate());
    let value = found
        .as_ref()
        .and_then(|found| serde_json::to_value(found).ok())
        .unwrap_or(Value::Null);
    Ok(json!({ "definition": value, "path": document.path }))
}

/// `lang.references`（LSP `textDocument/references`）。
pub fn references_op(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let document = target(session, args)?;
    let pos = usize_or(args, "pos", 0);
    if pos > document.text.len() {
        return Err(Error::usage(locale, "pos"));
    }
    let items = references::at(&document, pos, session.aggregate());
    let value = serde_json::to_value(&items).unwrap_or(Value::Array(Vec::new()));
    Ok(json!({ "count": items.len(), "path": document.path, "references": value }))
}

/// `lang.symbols`（LSP `textDocument/documentSymbol`）。
pub fn symbols(session: &mut Session, args: &Args) -> Result<Value> {
    let document = target(session, args)?;
    let items = document_symbol::all(&document);
    let value = serde_json::to_value(&items).unwrap_or(Value::Array(Vec::new()));
    Ok(json!({ "count": items.len(), "path": document.path, "symbols": value }))
}

/// `lang.format`（LSP `textDocument/formatting`。`apply` で当てる）。
pub fn format_op(session: &mut Session, args: &Args) -> Result<Value> {
    let document = target(session, args)?;
    let edits = formatting::document(&document, session.aggregate());
    let value = serde_json::to_value(&edits).unwrap_or(Value::Array(Vec::new()));
    if args.get("apply").and_then(Value::as_bool).unwrap_or(false) {
        gate(session, args)?;
        let next = features::apply_edits(&document.text, &edits);
        let path = document.path.clone();
        write_document(session, &path, next);
        let saved = commit(session)?;
        return Ok(json!({ "applied": edits.len(), "path": path, "save": saved }));
    }
    Ok(json!({ "count": edits.len(), "edits": value, "path": document.path }))
}

/// `lang.rename_preview`（LSP `textDocument/rename` の準備）。
pub fn rename_preview(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let document = target(session, args)?;
    let pos = usize_or(args, "pos", 0);
    let Some(prepared) = rename::prepare(&document, pos, session.aggregate()) else {
        return Err(Error::new(
            Class::NotFound,
            i18n::text(locale, "api.error.not_renamable").to_owned(),
        ));
    };
    let value = serde_json::to_value(&prepared).unwrap_or(Value::Null);
    match args.get("to").and_then(Value::as_str) {
        Some(to) => {
            let edit = rename::apply(&document, pos, to, session.aggregate());
            let changes = edit.as_ref().map(|edit| {
                edit.changes
                    .iter()
                    .map(|change| json!({ "edits": change.edits.len(), "path": change.path }))
                    .collect::<Vec<_>>()
            });
            Ok(json!({
                "changes": changes.unwrap_or_default(),
                "error": edit.err().map(|error| format!("{error:?}")),
                "path": document.path,
                "prepared": value,
            }))
        }
        None => Ok(json!({ "path": document.path, "prepared": value })),
    }
}

/// `lang.rename_apply`（LSP `textDocument/rename`。書き込みなので関門付き）。
pub fn rename_apply(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    gate(session, args)?;
    let document = target(session, args)?;
    let pos = usize_or(args, "pos", 0);
    let to = str_arg(locale, args, "to")?;
    let edit = rename::apply(&document, pos, &to, session.aggregate()).map_err(|error| {
        Error::new(
            Class::Validation,
            i18n::format(locale, "api.error.rename_failed", &[&format!("{error:?}")]),
        )
    })?;
    let mut written = Vec::new();
    for change in edit.changes.iter() {
        let Some(current) = session.aggregate().document(&change.path) else {
            continue;
        };
        let next = features::apply_edits(&current.text, &change.edits);
        write_document(session, &change.path, next);
        written.push(change.path.clone());
    }
    let saved = commit(session)?;
    Ok(json!({ "renamed": to, "save": saved, "written": written }))
}

/// `lang.folding`（LSP `textDocument/foldingRange`）。
pub fn folding_op(session: &mut Session, args: &Args) -> Result<Value> {
    let document = target(session, args)?;
    let items = folding::ranges(&document);
    let value = serde_json::to_value(&items).unwrap_or(Value::Array(Vec::new()));
    Ok(json!({ "count": items.len(), "path": document.path, "ranges": value }))
}

// =========================================================================== diagnostics.*

/// `diagnostics.run`（LSP `textDocument/publishDiagnostics`。`novnow check` と同じ検査）。
pub fn diagnostics_run(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let scope = str_or(args, "scope", "").to_owned();
    let path = args.get("path").and_then(Value::as_str).map(str::to_owned);
    let scene = args.get("scene").and_then(Value::as_str).map(str::to_owned);
    session.bench.commit();
    let fallback = if scope.is_empty() {
        None
    } else {
        Some(scope.as_str())
    };
    let wanted = scene.as_deref().or(fallback);
    let partial = wanted.and_then(|text| PartialSceneId::parse_with_sigil(text).ok());
    let mut documents: Vec<_> = session.aggregate().documents().collect();
    documents.sort_by(|a, b| a.path.cmp(&b.path));
    let mut items = Vec::new();
    let mut errors = 0;
    // 読み取り専用のプロジェクトでは W041 を 1 件先頭に置く（§4.1。プロジェクト全体を見るときだけ）
    if path.is_none() && partial.is_none() && scope.is_empty() {
        if let Some(notice) = crate::app::ops::project::read_only_notice(session, locale) {
            items.push(notice);
        }
    }
    for document in documents {
        if let Some(wanted) = path.as_deref() {
            if document.path != wanted {
                continue;
            }
        } else if partial.is_none() && !scope.is_empty() && !document.path.starts_with(&scope) {
            continue;
        }
        let scenes = partial
            .as_ref()
            .map(|_| session.aggregate().scenes_by_line(document));
        for found in diagnostics::document(document, session.aggregate()) {
            let row = diagnostic_json(document, &found, locale);
            if let (Some(wanted), Some(scenes)) = (partial.as_ref(), scenes.as_ref()) {
                let line = row.get("line").and_then(Value::as_u64).unwrap_or(1) as usize;
                let inside = scenes
                    .get(line.saturating_sub(1))
                    .copied()
                    .flatten()
                    .is_some_and(|id| wanted.expand_low() <= id && id <= wanted.expand_high());
                if !inside {
                    continue;
                }
            }
            if found.severity() == crate::core::diagnostics::Severity::Error {
                errors += 1;
            }
            items.push(row);
        }
    }
    Ok(json!({
        "count": items.len(),
        "diagnostics": items,
        "errors": errors,
    }))
}

/// `diagnostics.place`（§9.5.1 の「どこ」）。
pub fn place_op(session: &mut Session, args: &Args) -> Result<Value> {
    let document = target(session, args)?;
    let byte = usize_opt(args, "pos");
    let found = place::at(&document, byte, session.aggregate());
    Ok(json!({
        "path": document.path,
        "place": found.map(place_json).unwrap_or(Value::Null),
    }))
}

/// 「どこ」を作家の言葉で（§9.5.1）。
fn place_json(found: place::Place) -> Value {
    match found {
        place::Place::Contents(id) => json!({ "id": id, "kind": "contents" }),
        place::Place::EpisodePlot(id) => json!({ "id": id, "kind": "episode_plot" }),
        place::Place::ChapterPlot(id) => json!({ "id": id, "kind": "chapter_plot" }),
        place::Place::WholePlot(id) => json!({ "id": id, "kind": "whole_plot" }),
        place::Place::Character { name, mode } => {
            json!({ "kind": "character", "mode": mode, "name": name })
        }
        place::Place::Word(term) => json!({ "kind": "word", "term": term }),
        place::Place::Idea { id, title } => json!({ "id": id, "kind": "idea", "title": title }),
        place::Place::Material { id, title } => {
            json!({ "id": id, "kind": "material", "title": title })
        }
        place::Place::Prompt { role, section } => {
            json!({ "kind": "prompt", "role": role, "section": section })
        }
        place::Place::Settings => json!({ "kind": "settings" }),
    }
}

/// 編集を当てる（クイックフィックス・辞書への登録の共通口）。
pub fn apply_workspace_edit(
    session: &mut Session,
    edit: &features::WorkspaceEdit,
) -> Result<Vec<String>> {
    let mut written = Vec::new();
    for change in edit.changes.iter() {
        let Some(document) = session.aggregate().document(&change.path) else {
            continue;
        };
        let next = features::apply_edits(&document.text, &change.edits);
        write_document(session, &change.path, next);
        written.push(change.path.clone());
    }
    let _ = apply_plan;
    Ok(written)
}
