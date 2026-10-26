//! `materials.*`・`idea.*`・`cast.*`・`dictionary.*`・`review.*`・`reference.pane` の実装（§19.7）。

use serde_json::{json, Value};

use crate::app::error::{Class, Error, Result};
use crate::app::ops::{audit, commit, gate, via_mcp, write_document, Args};
use crate::app::session::{scene_arg, str_arg, str_or, usize_opt, usize_or, Session};
use crate::core::diagnostics;
use crate::core::features::{cast, ideas, materials_ops, plots, reference};
use crate::core::model::material::MaterialStatus;
use crate::core::resolve::dictionary::EntryKind;
use crate::ui::i18n;

/// 資料の索引（§5.8.2）。
const INDEX_PATH: &str = materials_ops::INDEX_PATH;

/// 今（RFC 3339。`saved_at` / `updated_at` に書く）。
fn stamp(session: &Session) -> String {
    session.host.now()
}

fn index_text(session: &mut Session) -> String {
    session.bench.commit();
    session.aggregate().text(INDEX_PATH).to_owned()
}

fn material_json(item: &crate::core::model::material::Material) -> Value {
    serde_json::to_value(item).unwrap_or_else(|_| json!({ "id": item.id }))
}

// =========================================================================== materials.*

/// `materials.list`（§16.1 の一覧）。
pub fn materials_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let text = index_text(session);
    let items: Vec<Value> = materials_ops::items(&text)
        .iter()
        .map(material_json)
        .collect();
    Ok(json!({ "count": items.len(), "materials": items }))
}

/// `materials.get`（索引の 1 件と資料 MD の本文）。
pub fn materials_get(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let id = str_arg(locale, args, "id")?;
    let text = index_text(session);
    let Some(item) = materials_ops::items(&text)
        .into_iter()
        .find(|item| item.id == id)
    else {
        return Err(Error::not_found(locale, &id, None));
    };
    let form = materials_ops::Review::of(&item);
    Ok(json!({
        "body": session.aggregate().text(&item.path),
        "material": material_json(&item),
        "review": {
            "category": form.category,
            "conditions": form.conditions,
            "source": form.source,
            "summary": form.summary,
            "tags": form.tags,
            "title": form.title,
        },
    }))
}

/// `materials.approve`（§16.1: `inbox/` → `docs/` へ移して `approved`）。
pub fn materials_approve(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let id = str_arg(locale, args, "id")?;
    let text = index_text(session);
    let Some(item) = materials_ops::items(&text)
        .into_iter()
        .find(|item| item.id == id)
    else {
        return Err(Error::not_found(locale, &id, None));
    };
    if item.status_kind() == Some(MaterialStatus::Approved) {
        return Ok(json!({ "already": true, "id": id }));
    }
    let Some(moved) =
        materials_ops::set_status(&text, &id, MaterialStatus::Approved, &stamp(session))
    else {
        return Err(Error::not_found(locale, &id, None));
    };
    let mut renamed = None;
    if let Some((from, to)) = &moved.rename {
        if session.aggregate().document(to).is_some() {
            return Err(Error::conflict(locale, to));
        }
        if session.aggregate().document(from).is_some() {
            session.bench.rename_document(from, to, session.clock);
            renamed = Some(to.clone());
        }
    }
    write_document(session, INDEX_PATH, moved.index);
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!("op:materials.approve {id}"),
        );
    }
    let saved = commit(session)?;
    Ok(json!({ "approved": id, "renamed": renamed, "save": saved }))
}

/// `materials.reject`（`status: rejected`。ファイルは inbox に残す）。
pub fn materials_reject(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let id = str_arg(locale, args, "id")?;
    let note = str_or(args, "note", "").to_owned();
    let text = index_text(session);
    let Some(item) = materials_ops::items(&text)
        .into_iter()
        .find(|item| item.id == id)
    else {
        return Err(Error::not_found(locale, &id, None));
    };
    let Some(moved) =
        materials_ops::set_status(&text, &id, MaterialStatus::Rejected, &stamp(session))
    else {
        return Err(Error::not_found(locale, &id, None));
    };
    write_document(session, INDEX_PATH, moved.index);
    if !note.is_empty() {
        // 却下の理由は資料 MD の末尾に残す（索引の形は §5.8.2 で固定なので触らない）
        let body = session.aggregate().text(&item.path).to_owned();
        let mut next = body.trim_end().to_owned();
        next.push('\n');
        next.push_str(&format!("note: {}\n", note.replace('\n', " ")));
        write_document(session, &item.path, next);
    }
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!("op:materials.reject {id}"),
        );
    }
    let saved = commit(session)?;
    Ok(json!({ "note": note, "rejected": id, "save": saved }))
}

/// `materials.update`（§16.1 の審査: タイトル・カテゴリ・conditions・summary・出典・タグ）。
pub fn materials_update(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let id = str_arg(locale, args, "id")?;
    let review = args.get("review").cloned().unwrap_or(json!({}));
    let text = index_text(session);
    let Some(item) = materials_ops::items(&text)
        .into_iter()
        .find(|item| item.id == id)
    else {
        return Err(Error::not_found(locale, &id, None));
    };
    let current = materials_ops::Review::of(&item);
    let pick = |key: &str, fallback: String| -> String {
        review
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or(fallback)
    };
    let draft = materials_ops::Review {
        title: pick("title", current.title),
        category: pick("category", current.category),
        summary: pick("summary", current.summary),
        tags: pick("tags", current.tags),
        conditions: pick("conditions", current.conditions),
        source: pick("source", current.source),
    };
    let Some(index) = materials_ops::update(&text, &id, &draft, &stamp(session)) else {
        return Err(Error::not_found(locale, &id, None));
    };
    write_document(session, INDEX_PATH, index);
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!("op:materials.update {id}"),
        );
    }
    let saved = commit(session)?;
    Ok(json!({ "save": saved, "updated": id }))
}

/// `materials.attach`（`>` + `attach-material`。添付した MD の登録とチェックサム更新）。
pub fn materials_attach(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let id = str_arg(locale, args, "id")?;
    let path = args.get("path").and_then(Value::as_str).map(str::to_owned);
    let text = index_text(session);
    let item = materials_ops::items(&text)
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| Error::not_found(locale, &id, None))?;
    let now = stamp(session);
    // 索引に無い資料 MD（添付されたファイル）なら登録する（W064 と同じ口）
    if let Some(path) = path.as_deref() {
        if !materials_ops::items(&text)
            .iter()
            .any(|found| found.path == path)
        {
            let body = session.aggregate().text(path).to_owned();
            let Some(created) = materials_ops::register(&text, path, &body, &now) else {
                return Err(Error::new(
                    Class::Validation,
                    i18n::format(locale, "api.error.register_failed", &[path]),
                ));
            };
            write_document(session, path, created.body);
            write_document(session, INDEX_PATH, created.index);
            if role.is_some() {
                audit(
                    session,
                    "direct_write",
                    &format!("op:materials.attach {}", created.id),
                );
            }
            let saved = commit(session)?;
            return Ok(json!({ "attached": created.id, "path": path, "save": saved }));
        }
    }
    let target = path.unwrap_or(item.path.clone());
    let body = session.aggregate().text(&target).to_owned();
    let Some(index) = materials_ops::touch(&text, &target, &now, &body) else {
        return Err(Error::not_found(locale, &target, Some(&target)));
    };
    write_document(session, INDEX_PATH, index);
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!("op:materials.attach {id}"),
        );
    }
    let saved = commit(session)?;
    Ok(json!({ "attached": id, "path": target, "save": saved }))
}

/// `materials.search`（§16.3 `Ctrl+M`。既存の MCP ツールと同じ照合）。
pub fn materials_search(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let query = str_arg(locale, args, "query")?;
    let category = str_or(args, "category", "");
    let scope = str_or(args, "scope", "");
    let mut terms = vec![query.as_str()];
    if !category.is_empty() {
        terms.push(category);
    }
    if !scope.is_empty() {
        terms.push(scope);
    }
    session.bench.commit();
    let hits: Vec<Value> = materials_ops::search(session.aggregate(), &terms.join(" "))
        .iter()
        .map(|hit| {
            json!({
                "category": hit.category,
                "date": hit.date,
                "id": hit.id,
                "path": hit.path,
                "score": hit.score,
                "source": hit.source,
                "status": hit.status,
                "summary": hit.summary,
                "title": hit.title,
            })
        })
        .collect();
    Ok(json!({ "count": hits.len(), "hits": hits, "query": query }))
}

/// `materials.propose`（§5.8.2 の提案。既存の MCP ツールと同じ）。
pub fn materials_propose(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let title = str_arg(locale, args, "title")?;
    let source = str_or(args, "source", "").to_owned();
    let body = str_or(args, "markdown", "").to_owned();
    let category = str_or(args, "category", "").to_owned();
    let role = args
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("researcher");
    let text = index_text(session);
    let now = stamp(session);
    let added_by = format!("agent:{role}");
    let Some(created) =
        materials_ops::propose_as(&text, &title, &category, &body, &source, &now, &added_by)
    else {
        return Err(Error::new(
            Class::Validation,
            i18n::text(locale, "api.error.index_invalid").to_owned(),
        )
        .with_kind("E025"));
    };
    write_document(session, &created.path, created.body.clone());
    write_document(session, INDEX_PATH, created.index);
    audit(
        session,
        "direct_write",
        &format!("op:materials.propose {}", created.id),
    );
    let saved = commit(session)?;
    Ok(json!({
        "body": created.body,
        "id": created.id,
        "path": created.path,
        "save": saved,
    }))
}

// =========================================================================== idea.*

/// `idea.list`（§10.4 の一覧）。
pub fn idea_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    session.bench.commit();
    let items: Vec<Value> = ideas::rows(session.aggregate())
        .iter()
        .map(|row| {
            json!({
                "id": row.id,
                "index": row.index,
                "lines": [row.lines.start, row.lines.end],
                "status": format!("{:?}", row.status),
                "tags": row.tags,
                "title": row.title,
            })
        })
        .collect();
    Ok(json!({ "count": items.len(), "ideas": items }))
}

/// `idea.capture`（§10.4 `Ctrl+Shift+I` の即時入力）。
pub fn idea_capture(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let title = str_arg(locale, args, "title")?;
    let kind = str_or(args, "kind", "").to_owned();
    let keywords = str_or(args, "keywords", "").to_owned();
    let text = session.aggregate().text(ideas::PATH).to_owned();
    let index = ideas::rows(session.aggregate()).len();
    let (mut next, id) = ideas::capture(&text, &title, &stamp(session));
    if !kind.is_empty() {
        next = ideas::set_field(&next, index, "kind", &kind);
    }
    if !keywords.is_empty() {
        next = ideas::set_field(&next, index, "tags", &keywords);
    }
    write_document(session, ideas::PATH, next);
    if role.is_some() {
        audit(session, "direct_write", &format!("op:idea.capture {id}"));
    }
    let saved = commit(session)?;
    Ok(json!({ "captured": id, "index": index, "save": saved }))
}

/// `idea.promote`（§10.4 の昇格: 話プロット / 用語 / 人物メモ）。
pub fn idea_promote(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let id = str_arg(locale, args, "id")?;
    let target = str_arg(locale, args, "target")?;
    let to = str_or(args, "to", "").to_owned();
    let text = session.aggregate().text(ideas::PATH).to_owned();
    let rows = ideas::rows(session.aggregate());
    let Some(row) = rows.iter().find(|row| row.id == id) else {
        return Err(Error::not_found(locale, &id, None));
    };
    let (title, body) =
        ideas::content(&text, row.index).ok_or_else(|| Error::not_found(locale, &id, None))?;
    let points: Vec<String> = body
        .split('\n')
        .map(|line| line.trim().trim_start_matches('-').trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect();
    let mut written = Vec::new();
    let placed = match target.as_str() {
        "plot" => {
            let scene = if to.is_empty() {
                session
                    .bench
                    .current_id()
                    .ok_or_else(|| Error::missing(locale, "to"))?
            } else {
                let mut parsed = args.clone();
                parsed.insert("to".to_owned(), json!(to));
                scene_arg(locale, &parsed, "to")?
            };
            let path = session
                .aggregate()
                .episode_plot_path(scene.chapter, scene.episode);
            let plot_text = session.aggregate().text(&path).to_owned();
            let (next, section) = ideas::append_plot_section(
                &plot_text,
                scene,
                &points,
                session.aggregate().digits(),
                session.aggregate(),
            );
            let digits = session.aggregate().digits();
            write_document(session, &path, next);
            written.push(path);
            json!(section.display(digits))
        }
        "term" => {
            let path = crate::core::resolve::dictionary::SourceFile::Words
                .path()
                .to_owned();
            let words = session.aggregate().text(&path).to_owned();
            let next = ideas::append_term(&words, &title, &points.join("\n"));
            write_document(session, &path, next);
            written.push(path);
            json!(title)
        }
        "character" | "cast" => {
            let index = if to.is_empty() {
                0
            } else {
                to.parse::<usize>()
                    .map_err(|_| Error::usage(locale, "to"))?
            };
            let path = cast::PATH.to_owned();
            let characters = session.aggregate().text(&path).to_owned();
            let next = ideas::append_character_memo(&characters, index, &points.join("\n"));
            write_document(session, &path, next);
            written.push(path);
            json!(index)
        }
        _ => return Err(Error::usage(locale, "target")),
    };
    let updated = ideas::set_field(&text, row.index, "status", "promoted");
    write_document(session, ideas::PATH, updated);
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!("op:idea.promote {id} {target}"),
        );
    }
    let saved = commit(session)?;
    Ok(json!({
        "promoted": id,
        "save": saved,
        "target": placed,
        "written": written,
    }))
}

/// `idea.drop`（§10.4 の削除: `status: dropped`）。
pub fn idea_drop(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let id = str_arg(locale, args, "id")?;
    let text = session.aggregate().text(ideas::PATH).to_owned();
    let rows = ideas::rows(session.aggregate());
    let Some(row) = rows.iter().find(|row| row.id == id) else {
        return Err(Error::not_found(locale, &id, None));
    };
    let next = ideas::set_field(&text, row.index, "status", "dropped");
    write_document(session, ideas::PATH, next);
    if role.is_some() {
        audit(session, "direct_write", &format!("op:idea.drop {id}"));
    }
    let saved = commit(session)?;
    Ok(json!({ "dropped": id, "save": saved }))
}

/// `idea.write`（アイデアの本文の書き換え）。
pub fn idea_write(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let id = str_arg(locale, args, "id")?;
    let text = str_arg(locale, args, "text")?;
    let current = session.aggregate().text(ideas::PATH).to_owned();
    let rows = ideas::rows(session.aggregate());
    let Some(row) = rows.iter().find(|row| row.id == id) else {
        return Err(Error::not_found(locale, &id, None));
    };
    let mut lines: Vec<&str> = current.split('\n').collect();
    let start = row.lines.start.min(lines.len());
    let end = row.lines.end.min(lines.len());
    let heading = lines.get(start).copied().unwrap_or_default();
    let mut next: Vec<String> = vec![heading.to_owned()];
    next.extend(text.split('\n').map(str::to_owned));
    let refs: Vec<&str> = next.iter().map(String::as_str).collect();
    lines.splice(start..end, refs);
    write_document(session, ideas::PATH, lines.join("\n"));
    if role.is_some() {
        audit(session, "direct_write", &format!("op:idea.write {id}"));
    }
    let saved = commit(session)?;
    Ok(json!({ "chars": text.chars().count(), "save": saved, "written": id }))
}

// =========================================================================== cast.*

/// `cast.get`（既存の MCP ツールと同じ。§15.4）。
pub fn cast_get(session: &mut Session, args: &Args) -> Result<Value> {
    via_mcp(session, "novnow.cast.get", args)
}

/// `cast.list`（§10.6 の登場人物一覧。`reading` 順）。
pub fn cast_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    session.bench.commit();
    let items: Vec<Value> = cast::characters(session.aggregate())
        .iter()
        .map(|row| {
            json!({
                "index": row.index,
                "lines": [row.lines.start, row.lines.end],
                "name": row.name,
                "reading": row.reading,
            })
        })
        .collect();
    Ok(json!({ "characters": items, "count": items.len() }))
}

/// `cast.timeline`（§10.6 のモードタイムライン）。
pub fn cast_timeline(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let index = usize_opt(args, "index").unwrap_or(0);
    let characters = cast::characters(session.aggregate());
    if characters.is_empty() {
        return Err(Error::new(
            Class::NotFound,
            i18n::text(locale, "api.error.no_character").to_owned(),
        ));
    }
    let found = cast::timeline(session.aggregate(), index);
    let bands: Vec<Value> = found
        .bands
        .iter()
        .map(|band| {
            json!({
                "covered": band.covered,
                "id": band.id,
                "label": band.label,
                "line": band.line,
                "range": band.range,
            })
        })
        .collect();
    Ok(json!({ "bands": bands, "chapters": found.chapters, "index": index }))
}

/// `cast.scan`（`>` + `scan-names`: 話者が決まらない台詞の一覧。§6.5.4）。
pub fn cast_scan(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    gate(session, args)?;
    session.bench.commit();
    let items: Vec<Value> = cast::unmatched_dialogue(session.aggregate())
        .iter()
        .map(|found| {
            json!({
                "byte": found.byte,
                "path": found.path,
                "scene": crate::app::ops::id_text(session, found.scene),
                "speaker": found.speaker,
                "text": found.text,
            })
        })
        .collect();
    Ok(json!({ "count": items.len(), "unmatched": items }))
}

// =========================================================================== dictionary.*

/// `dictionary.list`（§6.1 の辞書の全エントリ）。
pub fn dictionary_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    session.bench.commit();
    let mut entries: Vec<_> = session.aggregate().dictionary().entries().to_vec();
    entries.sort_by(|a, b| {
        format!("{:?}", a.kind)
            .cmp(&format!("{:?}", b.kind))
            .then_with(|| a.key.cmp(&b.key))
    });
    let items: Vec<Value> = entries
        .iter()
        .map(|entry| {
            json!({
                "key": entry.key,
                "kind": format!("{:?}", entry.kind),
                "normalized": entry.normalized,
            })
        })
        .collect();
    Ok(json!({ "count": items.len(), "entries": items }))
}

/// `dictionary.register_term`（`words.md` への登録。§6.2.1）。
pub fn register_term(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let term = str_arg(locale, args, "term")?;
    let definition = str_or(args, "definition", "").to_owned();
    let registered = register_entry(session, EntryKind::Word, &term)?;
    if !definition.is_empty() {
        let path = crate::core::resolve::dictionary::SourceFile::Words
            .path()
            .to_owned();
        let text = session.aggregate().text(&path).to_owned();
        let next = ideas::append_term(&text, &term, &definition);
        write_document(session, &path, next);
    }
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!("op:dictionary.register_term {term}"),
        );
    }
    let saved = commit(session)?;
    Ok(json!({ "registered": term, "save": saved, "written": registered }))
}

/// `dictionary.register_character`（`characters.md` への登録）。
pub fn register_character(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let name = str_arg(locale, args, "name")?;
    let reading = str_or(args, "reading", "").to_owned();
    let registered = register_entry(session, EntryKind::Character, &name)?;
    if !reading.is_empty() {
        let path = cast::PATH.to_owned();
        let text = session.aggregate().text(&path).to_owned();
        let rows = cast::characters(session.aggregate());
        if let Some(row) = rows.iter().find(|row| row.name == name) {
            let mut lines: Vec<&str> = text.split('\n').collect();
            let at = (row.lines.start + 1).min(lines.len());
            let line: String = format!("reading: {reading}");
            let slot = lines[at..row.lines.end.min(lines.len())]
                .iter()
                .position(|found| found.trim_start().starts_with("reading:"));
            match slot {
                Some(found) => lines[at + found] = &line,
                None => lines.insert(at, &line),
            }
            write_document(session, &path, lines.join("\n"));
        }
    }
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!("op:dictionary.register_character {name}"),
        );
    }
    let saved = commit(session)?;
    Ok(json!({ "registered": name, "save": saved, "written": registered }))
}

/// 辞書への登録（クイックフィックスと同じ編集を当てる。§12.3）。
fn register_entry(session: &mut Session, kind: EntryKind, name: &str) -> Result<Vec<String>> {
    let locale = session.locale;
    session.bench.commit();
    let edit = crate::core::features::code_action::register_edit(session.aggregate(), kind, name);
    let mut written = Vec::new();
    for change in edit.changes.iter() {
        let Some(document) = session.aggregate().document(&change.path) else {
            continue;
        };
        let next = crate::core::features::apply_edits(&document.text, &change.edits);
        write_document(session, &change.path, next);
        written.push(change.path.clone());
    }
    if written.is_empty() {
        return Err(Error::new(
            Class::Validation,
            i18n::format(locale, "api.error.register_failed", &[name]),
        ));
    }
    Ok(written)
}

/// `dictionary.term_full`（既存の MCP ツール `novnow.lexicon.search` と同じ）。
pub fn term_full(session: &mut Session, args: &Args) -> Result<Value> {
    via_mcp(session, "novnow.lexicon.search", args)
}

/// `dictionary.modes_list`（§6.5 の全モード一覧）。
pub fn modes_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    session.bench.commit();
    let mut items = Vec::new();
    for character in cast::characters(session.aggregate()) {
        let found = cast::timeline(session.aggregate(), character.index);
        for band in found.bands {
            items.push(json!({
                "character": character.name,
                "index": character.index,
                "id": band.id,
                "label": band.label,
                "range": band.range,
            }));
        }
    }
    Ok(json!({ "count": items.len(), "modes": items }))
}

// =========================================================================== review.* / reference.pane

/// `review.list`（§10.9 の見直し: 診断の一覧）。
pub fn review_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let locale = session.locale;
    session.bench.commit();
    let mut items = Vec::new();
    let mut documents: Vec<_> = session.aggregate().documents().collect();
    documents.sort_by(|a, b| a.path.cmp(&b.path));
    for document in documents {
        let starts = crate::core::features::project::line_starts_of(&document.text);
        for found in diagnostics::document(document, session.aggregate()) {
            let (line, column) = found
                .range
                .as_ref()
                .map(|range| crate::core::features::project::line_of(&document.text, range.start))
                .map_or((0, 0), |line| {
                    let column = found.range.as_ref().map_or(0, |range| {
                        range.start - starts.get(line).copied().unwrap_or(0)
                    });
                    (line + 1, column + 1)
                });
            items.push(json!({
                "byte": found.range.as_ref().map(|range| range.start),
                "code": found.code.as_str(),
                "column": column,
                "line": line,
                "message": i18n::diagnostic(locale, &found),
                "path": document.path,
                "severity": crate::app::ops::severity_name(found.severity()),
            }));
        }
    }
    Ok(json!({ "count": items.len(), "diagnostics": items }))
}

/// `review.goto`（見直し項目へ移動。M—・C—）。
pub fn review_goto(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let index = usize_or(args, "index", 0);
    let listed = review_list(session, args)?;
    let items = listed
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(item) = items.get(index) else {
        return Err(Error::not_found(locale, &format!("review {index}"), None));
    };
    let path = item.get("path").and_then(Value::as_str).unwrap_or_default();
    let byte = item.get("byte").and_then(Value::as_u64).unwrap_or(0) as usize;
    session.remember();
    session.bench.reveal(path, byte);
    Ok(json!({
        "byte": byte,
        "code": item.get("code").cloned().unwrap_or(Value::Null),
        "index": index,
        "path": path,
    }))
}

/// 行の移動先の JSON 表現（GUI と同じ `reference::Nav`）。
fn nav_json(session: &Session, nav: &reference::Nav) -> Value {
    match nav {
        reference::Nav::Plot(scene) => {
            json!({ "kind": "plot", "scene": crate::app::ops::id_text(session, *scene) })
        }
        reference::Nav::Outline(scene) => {
            let digits = session.aggregate().digits();
            let full = scene.display(digits);
            let mut parts: Vec<&str> = full.split('-').collect();
            let values = [scene.chapter, scene.episode, scene.scene, scene.section];
            while parts.len() > 1 && values[parts.len() - 1] == 0 {
                parts.pop();
            }
            json!({ "kind": "outline", "scene": parts.join("-") })
        }
        reference::Nav::Character(index) => json!({ "kind": "character", "index": index }),
        reference::Nav::Term(index) => json!({ "kind": "term", "index": index }),
        reference::Nav::OpenMaterial(id) => json!({ "kind": "material_open", "id": id }),
        reference::Nav::AddMaterial(id) => json!({ "kind": "material_add", "id": id }),
        reference::Nav::At(range) => json!({ "kind": "at", "byte": range.start }),
    }
}

/// `reference.pane`（§9.5.2 の 1〜9 セクション）。
pub fn reference_pane(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let scene = match args.get("scene").and_then(Value::as_str) {
        Some(_) => scene_arg(locale, args, "scene")?,
        None => session
            .bench
            .current_id()
            .ok_or_else(|| Error::missing(locale, "scene"))?,
    };
    session.bench.commit();
    let list = crate::core::features::sections::all(session.aggregate());
    let Some(found) = list.iter().find(|item| item.id == scene) else {
        return Err(Error::not_found(
            locale,
            &crate::app::ops::id_text(session, scene),
            None,
        ));
    };
    let Some(document) = session.aggregate().document(&found.path) else {
        return Err(Error::not_found(locale, &found.path, Some(&found.path)));
    };
    let pane = reference::at(document, found.lines.clone(), scene, session.aggregate());
    let upper: Vec<Value> = pane
        .upper
        .iter()
        .map(|row| json!({ "id": row.id, "nav": nav_json(session, &row.nav), "summary": row.summary }))
        .collect();
    let cast_items: Vec<Value> = pane
        .cast
        .iter()
        .map(|row| {
            json!({
                "emotion": row.emotion,
                "modes": row.modes,
                "name": row.name,
                "nav": nav_json(session, &row.nav),
                "personality": row.personality,
                "role": row.role,
                "tone": row.tone,
            })
        })
        .collect();
    let words: Vec<Value> = pane
        .words
        .iter()
        .map(|row| json!({ "definition": row.definition, "nav": nav_json(session, &row.nav), "term": row.term }))
        .collect();
    let materials: Vec<Value> = pane
        .materials
        .iter()
        .map(|row| {
            json!({
                "candidate": row.candidate,
                "id": row.id,
                "nav": nav_json(session, &row.nav),
                "source": row.source,
                "summary": row.summary,
                "title": row.title,
            })
        })
        .collect();
    let found: Vec<Value> = pane
        .diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "byte": diagnostic.range.as_ref().map(|range| range.start),
                "code": diagnostic.code.as_str(),
                "message": i18n::diagnostic(locale, diagnostic),
                "path": document.path,
                "severity": crate::app::ops::severity_name(diagnostic.severity()),
            })
        })
        .collect();
    Ok(json!({
        "cast": cast_items,
        "diagnostics": found,
        "illustrations": pane.illustrations,
        "materials": materials,
        "plot": pane.plot,
        "plot_nav": pane.plot_nav.as_ref().map(|nav| nav_json(session, nav)),
        "scene": crate::app::ops::id_text(session, scene),
        "senses": pane.senses,
        "space": pane.space,
        "upper": upper,
        "words": words,
    }))
}

/// 進捗ツリー（§10.5。`review.list` の進捗タブと同じ材料）。
pub fn progress_json(session: &Session) -> Value {
    let items: Vec<Value> = plots::progress(session.aggregate())
        .iter()
        .map(|row| {
            json!({
                "chars": row.chars,
                "depth": row.depth,
                "id": crate::app::ops::id_text(session, row.id),
                "prose": row.prose,
                "state": format!("{:?}", row.state),
                "target": row.target,
                "title": row.title,
            })
        })
        .collect();
    json!({ "count": items.len(), "progress": items })
}
