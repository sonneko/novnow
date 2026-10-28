//! `section.*`・`nav.*`・`document.*`・`plot.*`・`save.*`・`search.*` の実装（§19.7）。

use serde_json::{json, Value};

use crate::app::error::{Class, Error, Result};
use crate::app::event::AppEvent;
use crate::app::ops::{audit, commit, gate, via_mcp, Args};
use crate::app::session::{
    bool_or, nav_of, scene_arg, str_arg, str_or, usize_opt, usize_or, Find, Place, Session,
};
use crate::app::workbench::{DeleteOutcome, Nav};
use crate::app::workspace::Workspace;
use crate::core::features::{plots, sections, Project};
use crate::core::id::SceneId;
use crate::ui::i18n::{self, Locale};

// =========================================================================== section.*

/// `section.list`（指定文書内の節一覧。LSP の `textDocument/documentSymbol`）。
pub fn section_list(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let all = sections::all(session.aggregate());
    let filter = args.get("path").and_then(Value::as_str);
    let items: Vec<Value> = all
        .iter()
        .filter(|found| filter.is_none_or(|path| found.path == path))
        .map(|found| {
            json!({
                "id": crate::app::ops::id_text(session, found.id),
                "lines": [found.lines.start, found.lines.end],
                "path": found.path,
                "summary": found.scene_title,
            })
        })
        .collect();
    let _ = locale;
    Ok(json!({ "count": items.len(), "sections": items }))
}

/// `section.get`（既存の MCP ツールと同じ形。§19.1.1）。
pub fn section_get(session: &mut Session, args: &Args) -> Result<Value> {
    via_mcp(session, "novnow.section.get", args)
}

/// `section.propose`（本文を書き換えず `agents/proposals/` に置く。§8.7.1・§19.7）。
///
/// 既存の MCP ツール `novnow.section.propose` と同じ関数（`via_mcp`）を使う。提案は本文を
/// 変えないので関門を置かない（§19.1.5）。作られる提案の `origin` は、GUI の提案モード
/// （`human`）と区別するためツール核の印 `mcp` のままである（§8.7.2）。
pub fn section_propose(session: &mut Session, args: &Args) -> Result<Value> {
    via_mcp(session, "novnow.section.propose", args)
}

/// `section.write`（§9.6 の状態機経由。MCP では関門付き）。
pub fn section_write(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let scene = scene_arg(locale, args, "scene")?;
    let text = str_arg(locale, args, "text")?;
    let (path, spliced) = splice_section(session, scene, &text)?;
    write_document(session, &path, spliced);
    if let Some(role) = role {
        audit(
            session,
            "direct_write",
            &format!(
                "op:section.write {}",
                crate::app::ops::id_text(session, scene)
            ),
        );
        let _ = role;
    }
    session.push(AppEvent::DocumentChanged {
        path: path.clone(),
        change: "written".to_owned(),
    });
    let saved = commit(session)?;
    Ok(
        json!({ "chars": text.chars().count(), "path": path, "scene": crate::app::ops::id_text(session, scene), "save": saved }),
    )
}

/// `section.insert`（`Ctrl+;`。§9.4.1）。
pub fn section_insert(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let text = str_or(args, "text", "").to_owned();
    if let Some(after) = args.get("after").and_then(Value::as_str) {
        let scene = scene_arg(locale, args, "after")?;
        let _ = after;
        if !session
            .bench
            .jump(&crate::app::ops::id_text(session, scene))
        {
            return Err(Error::not_found(
                locale,
                &crate::app::ops::id_text(session, scene),
                None,
            ));
        }
    }
    let created = session.bench.new_section(session.clock);
    if !created {
        return Err(Error::new(
            Class::Validation,
            i18n::text(locale, "api.error.insert_failed").to_owned(),
        ));
    }
    if !text.is_empty() {
        // 新設した節の本文を入れる（ファイル全体の置き換えではない。§9.4.1）
        session.bench.buffer_mut().push_str(&text);
        session.bench.commit();
    }
    let id = session
        .bench
        .current_id()
        .map(|scene| crate::app::ops::id_text(session, scene));
    // 本文がまだ無い節は文書に現れない（§10.0「空の節を作らず、その位置にカーソルだけ置く」）。
    // その位置にカーソルだけ置いたことを `pending` で正直に返す。
    let pending = session.bench.current_index().is_none();
    if role.is_some() {
        audit(session, "direct_write", "op:section.insert");
    }
    let saved = commit(session)?;
    Ok(json!({ "id": id, "pending": pending, "save": saved }))
}

/// `section.delete`（`>` + `delete-section`。§8.1.2。本文と対応するプロットを一緒に削除）。
pub fn section_delete(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    // `>` + `delete-section` は今の節（§9.4.2）
    let scene = match args.get("scene").and_then(Value::as_str) {
        Some(_) => scene_arg(locale, args, "scene")?,
        None => session
            .bench
            .current_id()
            .ok_or_else(|| Error::missing(locale, "scene"))?,
    };
    match session.bench.delete_section_at(scene, session.clock) {
        DeleteOutcome::Deleted => {}
        DeleteOutcome::Ambiguous => {
            return Err(Error::new(
                Class::Validation,
                i18n::text(locale, "delete.ambiguous").to_owned(),
            ));
        }
        DeleteOutcome::NotFound => {
            return Err(Error::not_found(
                locale,
                &crate::app::ops::id_text(session, scene),
                None,
            ));
        }
    }
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!(
                "op:section.delete {}",
                crate::app::ops::id_text(session, scene)
            ),
        );
    }
    let saved = commit(session)?;
    Ok(json!({ "deleted": crate::app::ops::id_text(session, scene), "save": saved }))
}

/// `section.split`（節の分割。§8.1.1・§9.4.1。MCP では `section.delete` と同じ関門付き）。
pub fn section_split(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let at = usize_opt(args, "at").ok_or_else(|| Error::missing(locale, "at"))?;
    let scene = match args.get("scene").and_then(Value::as_str) {
        Some(_) => scene_arg(locale, args, "scene")?,
        None => session
            .bench
            .current_id()
            .ok_or_else(|| Error::missing(locale, "scene"))?,
    };
    if !session
        .bench
        .jump(&crate::app::ops::id_text(session, scene))
    {
        return Err(Error::not_found(
            locale,
            &crate::app::ops::id_text(session, scene),
            None,
        ));
    }
    let split = session.bench.split_section(at, session.clock);
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!(
                "op:section.split {}",
                crate::app::ops::id_text(session, scene)
            ),
        );
    }
    let saved = commit(session)?;
    Ok(json!({
        "scene": session.bench.current_id().map(|id| crate::app::ops::id_text(session, id)),
        "split": split,
        "save": saved,
    }))
}

/// `section.confirm_next`（`Ctrl+Enter`。§12.5。M—・C—）。
pub fn confirm_next(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let confirmed = session.bench.confirm_next(session.clock);
    Ok(json!({
        "confirmed": confirmed,
        "scene": session.bench.current_id().map(|id| crate::app::ops::id_text(session, id)),
    }))
}

// =========================================================================== nav.*

/// `nav.file`（文書の直接指定）。
pub fn nav_file(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let path = str_arg(locale, args, "path")?;
    let scene = match args.get("scene").and_then(Value::as_str) {
        Some(text) => text.to_owned(),
        None => {
            let list = sections::all(session.aggregate());
            match list.iter().find(|found| found.path == path) {
                Some(found) => crate::app::ops::id_text(session, found.id),
                None => return Err(Error::not_found(locale, &path, Some(&path))),
            }
        }
    };
    session.remember();
    let moved = session.bench.jump(&scene);
    if !moved {
        return Err(Error::not_found(locale, &scene, Some(&path)));
    }
    Ok(json!({ "path": path, "scene": scene }))
}

/// `nav.section`（`Ctrl+G`）。
pub fn nav_section(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let query = str_arg(locale, args, "scene")?;
    session.remember();
    if !session.bench.jump(&query) {
        return Err(Error::not_found(locale, &query, None));
    }
    Ok(json!({
        "path": session.bench.current().map(|found| found.path.clone()),
        "scene": session.bench.current_id().map(|id| crate::app::ops::id_text(session, id)),
    }))
}

/// `nav.jump`（`Ctrl+L` の行、または `Ctrl+J` のクエリ）。
pub fn nav_jump(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    session.remember();
    if let Some(line) = usize_opt(args, "line") {
        let Some((document, start)) = session.bench.spliced_document() else {
            return Err(Error::new(
                Class::NotFound,
                i18n::text(locale, "api.error.no_document").to_owned(),
            ));
        };
        let starts = crate::core::features::project::line_starts_of(&document.text);
        let index = line.saturating_sub(1);
        let byte = starts.get(index).copied().ok_or_else(|| {
            Error::not_found(locale, &format!("line {line}"), Some(&document.path))
        })?;
        let _ = start;
        session
            .bench
            .reveal(&document.path, byte.saturating_sub(start.min(byte)));
        return Ok(json!({ "byte": byte, "line": line, "path": document.path }));
    }
    let query = str_arg(locale, args, "query")?;
    if !session.bench.jump(&query) {
        return Err(Error::not_found(locale, &query, None));
    }
    Ok(json!({
        "query": query,
        "scene": session.bench.current_id().map(|id| crate::app::ops::id_text(session, id)),
    }))
}

/// `nav.prev`（`Ctrl+[`）。
pub fn nav_prev(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    step_history(session, false)
}

/// `nav.next`（`Ctrl+]`）。
pub fn nav_next(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    step_history(session, true)
}

fn step_history(session: &mut Session, forward: bool) -> Result<Value> {
    let locale = session.locale;
    let target = if forward {
        session.history_at.checked_add(1)
    } else {
        session.history_at.checked_sub(1)
    };
    let Some(index) = target.filter(|index| *index < session.history.len()) else {
        return Ok(json!({ "moved": false }));
    };
    let place = session.history[index].clone();
    session.history_at = index;
    let moved = session
        .bench
        .jump(&crate::app::ops::id_text(session, place.scene));
    if !moved {
        return Err(Error::not_found(
            locale,
            &crate::app::ops::id_text(session, place.scene),
            Some(&place.path),
        ));
    }
    Ok(
        json!({ "moved": true, "path": place.path, "scene": crate::app::ops::id_text(session, place.scene) }),
    )
}

/// `nav.back`（`Ctrl+H` 親ディレクトリへ）。
pub fn nav_back(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let Some(current) = session.bench.current_id() else {
        return Ok(json!({ "moved": false }));
    };
    let parent = SceneId {
        section: 0,
        ..current
    };
    let list = sections::all(session.aggregate());
    let target = list
        .iter()
        .rfind(|found| {
            SceneId {
                section: 0,
                ..found.id
            } == parent
        })
        .map(|found| found.id);
    match target {
        Some(id) => {
            session.remember();
            session.bench.jump(&crate::app::ops::id_text(session, id));
            Ok(json!({ "moved": true, "scene": crate::app::ops::id_text(session, id) }))
        }
        None => Ok(json!({ "moved": false })),
    }
}

/// `nav.forward`（`Ctrl+Shift+H` 子の先頭へ）。
pub fn nav_forward(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let Some(current) = session.bench.current_id() else {
        return Ok(json!({ "moved": false }));
    };
    let list = sections::all(session.aggregate());
    let target = list
        .iter()
        .find(|found| {
            found.id.chapter == current.chapter
                && found.id.episode == current.episode
                && found.id.scene > current.scene
        })
        .or_else(|| {
            list.iter().find(|found| {
                found.id.chapter == current.chapter && found.id.episode > current.episode
            })
        })
        .map(|found| found.id);
    match target {
        Some(id) => {
            session.remember();
            session.bench.jump(&crate::app::ops::id_text(session, id));
            Ok(json!({ "moved": true, "scene": crate::app::ops::id_text(session, id) }))
        }
        None => Ok(json!({ "moved": false })),
    }
}

/// `nav.up`（`Ctrl+B`。1 つ上の階層へ。§10.1・§10.3）。
pub fn nav_up(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let from = session.switcher.current();
    let to = match from {
        Workspace::Write => Workspace::PlotEpisode,
        Workspace::PlotEpisode => Workspace::PlotOutline,
        _ => {
            return Ok(
                json!({ "moved": false, "workspace": crate::app::session::workspace_name(from) }),
            )
        }
    };
    let moved = session.switch_workspace(to);
    Ok(json!({
        "moved": moved,
        "workspace": crate::app::session::workspace_name(session.switcher.current()),
    }))
}

/// `nav.switcher`（§11.1 の切替パネルの一覧）。
pub fn nav_switcher(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let items: Vec<Value> = Workspace::ALL
        .iter()
        .filter(|workspace| workspace.available())
        .map(|workspace| {
            json!({
                "current": *workspace == session.switcher.current(),
                "name": crate::app::session::workspace_name(*workspace),
                "number": workspace.number(),
            })
        })
        .collect();
    Ok(json!({
        "current": crate::app::session::workspace_name(session.switcher.current()),
        "previous": session.switcher.previous().map(crate::app::session::workspace_name),
        "workspaces": items,
    }))
}

// =========================================================================== document.*

/// `document.get`（`.md` / `.txt` の全文）。
pub fn document_get(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let path = str_arg(locale, args, "path")?;
    session.bench.commit();
    let Some(document) = session.aggregate().document(&path) else {
        return Err(Error::not_found(locale, &path, Some(&path)));
    };
    Ok(json!({
        "format": format!("{:?}", document.format),
        "path": document.path,
        "text": document.text,
    }))
}

/// `document.write`（全文の書き込み。MCP では関門付き）。
pub fn document_write(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let path = str_arg(locale, args, "path")?;
    let text = str_arg(locale, args, "text")?;
    write_document(session, &path, text.clone());
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!("op:document.write {}", path),
        );
    }
    session.push(AppEvent::DocumentChanged {
        path: path.clone(),
        change: "written".to_owned(),
    });
    let saved = commit(session)?;
    Ok(json!({ "chars": text.chars().count(), "path": path, "save": saved }))
}

/// `document.rename`（`>` + `rename-document`。原子的: 失敗時はどのファイルも変わらない）。
pub fn document_rename(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let from = str_arg(locale, args, "from")?;
    let to = str_arg(locale, args, "to")?;
    let renamed = session.bench.rename_document(&from, &to, session.clock);
    if !renamed {
        return Err(Error::not_found(locale, &from, Some(&from)));
    }
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!("op:document.rename {from} -> {to}"),
        );
    }
    let saved = commit(session)?;
    session.push(AppEvent::DocumentChanged {
        path: to.clone(),
        change: "renamed".to_owned(),
    });
    Ok(json!({ "from": from, "save": saved, "to": to }))
}

// =========================================================================== plot.*

/// `plot.get`（§10.3.1。既存の MCP ツールと同じ）。
pub fn plot_get(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let level = str_or(args, "level", "episode");
    match level {
        "episode" => via_mcp(session, "novnow.plot.get", args),
        "chapter" | "all" => {
            let chapter = match level {
                "all" => None,
                _ => Some(
                    chapter_of(session, args).ok_or_else(|| Error::missing(locale, "chapter"))?,
                ),
            };
            let blocks: Vec<Value> = plots::outline_blocks(session.aggregate(), chapter)
                .iter()
                .map(|block| {
                    json!({
                        "end": block.end,
                        "id": crate::app::ops::id_text(session, block.id),
                        "lines": [block.lines.start, block.lines.end],
                        "path": block.path,
                        "state": block.state.map(|state| format!("{state:?}")),
                        "summary": block.summary,
                        "title": block.title,
                    })
                })
                .collect();
            Ok(json!({ "blocks": blocks, "level": level }))
        }
        _ => Err(Error::usage(locale, "level")),
    }
}

/// `plot.write`（§10.3.2。`outline` / `chapter` / `episode`。MCP では関門付き）。
pub fn plot_write(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let level = str_or(args, "level", "episode");
    let text = str_arg(locale, args, "text")?;
    let (path, next) = match level {
        "episode" => {
            let scene = scene_arg(locale, args, "scene")?;
            let (path, spliced) = splice_plot_section(session, scene, &text)?;
            (path, spliced)
        }
        "chapter" => {
            let chapter =
                chapter_of(session, args).ok_or_else(|| Error::missing(locale, "chapter"))?;
            (session.aggregate().chapter_plot_path(chapter), text.clone())
        }
        "all" | "outline" => (
            crate::core::features::project::ALL_PLOT_PATH.to_owned(),
            text.clone(),
        ),
        _ => return Err(Error::usage(locale, "level")),
    };
    write_document(session, &path, next);
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!("op:plot.write {level} {path}"),
        );
    }
    session.push(AppEvent::DocumentChanged {
        path: path.clone(),
        change: "written".to_owned(),
    });
    let saved = commit(session)?;
    Ok(json!({ "chars": text.chars().count(), "level": level, "path": path, "save": saved }))
}

/// `plot.set_state`（§10.3.3 の確認状態 ✓。MCP では関門付き）。
pub fn plot_set_state(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let scene = scene_arg(locale, args, "scene")?;
    let fixed = bool_or(args, "state", true);
    let value = if fixed { "fixed" } else { "draft" };
    let project = session.aggregate();
    let (path, next) = match plot_state_edit(project, scene, value) {
        Some(edited) => edited,
        None => {
            return Err(Error::not_found(
                locale,
                &crate::app::ops::id_text(session, scene),
                None,
            ))
        }
    };
    write_document(session, &path, next);
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!(
                "op:plot.set_state {} {value}",
                crate::app::ops::id_text(session, scene)
            ),
        );
    }
    let saved = commit(session)?;
    Ok(
        json!({ "path": path, "save": saved, "scene": crate::app::ops::id_text(session, scene), "state": value }),
    )
}

/// `plot.insert_section`（`Ctrl+Shift+N`。§10.2。MCP では関門付き）。
pub fn plot_insert_section(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let scene = match args.get("scene").and_then(Value::as_str) {
        Some(_) => scene_arg(locale, args, "scene")?,
        None => session
            .bench
            .current_id()
            .ok_or_else(|| Error::missing(locale, "scene"))?,
    };
    let title = str_or(args, "title", "");
    let project = session.aggregate();
    let (chapter, episode) = (scene.chapter, scene.episode);
    let path = project.episode_plot_path(chapter, episode);
    let rows = plots::episode_rows(project, chapter, episode);
    let text = project.text(&path).to_owned();
    // 場面の最後の節の後ろに節を足す（場面が無ければ見出しごと足す、§10.2）
    let after = rows
        .iter()
        .rposition(|row| row.id.chapter == scene.chapter && row.id.episode == scene.episode);
    let (mut next, at) = plots::insert_section(&text, &rows, after, scene, project.digits());
    if !title.is_empty() {
        let mut lines: Vec<&str> = next.split('\n').collect();
        lines.insert(at.min(lines.len()), title);
        next = lines.join("\n");
    }
    write_document(session, &path, next);
    if role.is_some() {
        audit(
            session,
            "direct_write",
            &format!(
                "op:plot.insert_section {}",
                crate::app::ops::id_text(session, scene)
            ),
        );
    }
    let saved = commit(session)?;
    Ok(json!({ "path": path, "save": saved, "scene": crate::app::ops::id_text(session, scene) }))
}

/// `plot.outline_fold`（`Ctrl+Shift+[` / `]`。§10.3.1）。
pub fn outline_fold(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let chapter = usize_or(args, "chapter", 0) as u16;
    if chapter == 0 {
        return Err(Error::usage(locale, "chapter"));
    }
    let fold = bool_or(args, "fold", true);
    if fold {
        session.folded.insert(chapter);
    } else {
        session.folded.remove(&chapter);
    }
    let folded: Vec<u16> = session.folded.iter().copied().collect();
    Ok(json!({ "chapter": chapter, "fold": fold, "folded": folded }))
}

// =========================================================================== save.*

/// `save.now`（`Ctrl+S`）。
pub fn save_now(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    session.flush()
}

/// `save.state`（§9.6 の 6 状態）。
pub fn save_state(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let unsaved: Vec<String> = session.bench.unsaved().map(str::to_owned).collect();
    let conflicts: Vec<String> = session.bench.conflicts().map(str::to_owned).collect();
    Ok(json!({
        "conflicts": conflicts,
        "state": session.save_state(),
        "unsaved": unsaved,
    }))
}

/// `save.keep_mine`（§9.6 の衝突解決）。
pub fn keep_mine(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let paths = conflict_paths(session, args)?;
    for path in &paths {
        session.bench.keep_mine(path, session.clock);
        session.conflicts.remove(path);
    }
    if paths.is_empty() {
        return Err(Error::new(
            Class::Conflict,
            i18n::text(locale, "api.error.no_conflict").to_owned(),
        ));
    }
    let saved = session.flush()?;
    Ok(json!({ "kept": paths, "save": saved }))
}

/// `save.reload_external`（§9.6）。
pub fn reload_external(session: &mut Session, args: &Args) -> Result<Value> {
    let paths = conflict_paths(session, args)?;
    let mut reloaded = Vec::new();
    for path in &paths {
        let text = session.conflicts.get(path).cloned().unwrap_or(None);
        session.bench.take_external(path, text);
        session.conflicts.remove(path);
        reloaded.push(path.clone());
    }
    Ok(json!({ "reloaded": reloaded }))
}

/// `save.diff_external`（§8.5 の `diff-against-external`）。
pub fn diff_external(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let paths = conflict_paths(session, args)?;
    let mut diffs = Vec::new();
    for path in &paths {
        let mine = session.aggregate().text(path).to_owned();
        let external = session
            .conflicts
            .get(path)
            .cloned()
            .unwrap_or(None)
            .unwrap_or_default();
        diffs.push(json!({
            "external_chars": external.chars().count(),
            "lines": diff_lines(&mine, &external),
            "mine_chars": mine.chars().count(),
            "path": path,
        }));
    }
    if diffs.is_empty() {
        return Err(Error::new(
            Class::Conflict,
            i18n::text(locale, "api.error.no_conflict").to_owned(),
        ));
    }
    Ok(json!({ "diffs": diffs }))
}

fn conflict_paths(session: &mut Session, args: &Args) -> Result<Vec<String>> {
    Ok(match args.get("path").and_then(Value::as_str) {
        Some(path) => vec![path.to_owned()],
        None => session.bench.conflicts().map(str::to_owned).collect(),
    })
}

/// 行単位の差分（`-` が自分の版에만、`+` が外部の版에만ある行。§9.6 の表示と同じ材料）。
fn diff_lines(mine: &str, external: &str) -> Vec<Value> {
    let mine_lines: Vec<&str> = mine.split('\n').collect();
    let external_lines: Vec<&str> = external.split('\n').collect();
    let mut out = Vec::new();
    for (index, line) in mine_lines.iter().enumerate() {
        if external_lines.get(index) != Some(line) {
            out.push(json!({ "line": index + 1, "side": "mine", "text": line }));
        }
    }
    for (index, line) in external_lines.iter().enumerate() {
        if mine_lines.get(index) != Some(line) {
            out.push(json!({ "line": index + 1, "side": "external", "text": line }));
        }
    }
    out
}

// =========================================================================== search.*

/// `search.file`（`Ctrl+F`。編集中文書。置換は MCP では関門付き）。
pub fn search_file(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let query = str_arg(locale, args, "query")?;
    let regex = bool_or(args, "regex", false);
    let (path, text, start) = editing_document(session)?;
    let hits = matches_in(&text, &query, regex, locale)?;
    let replace = args.get("replace").and_then(Value::as_str);
    if let Some(replace) = replace {
        gate(session, args)?;
        let next = replace_all(&text, &hits, replace);
        let spliced = splice_slice(session, &path, start, next);
        write_document(session, &path, spliced);
        let saved = commit(session)?;
        return Ok(json!({ "count": hits.len(), "path": path, "replaced": true, "save": saved }));
    }
    session.find = Some(Find {
        query: query.clone(),
        replace: None,
        hits: hits.clone(),
        selected: 0,
    });
    Ok(json!({
        "count": hits.len(),
        "hits": hits.iter().map(|range| json!([range.start, range.end])).collect::<Vec<_>>(),
        "path": path,
        "query": query,
    }))
}

/// `search.replace_file`（§9.7.5 の全置換。MCP では関門付き）。
pub fn replace_file(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    gate(session, args)?;
    let query = str_arg(locale, args, "query")?;
    let replace = str_arg(locale, args, "replace")?;
    let regex = bool_or(args, "regex", false);
    let (path, text, start) = editing_document(session)?;
    let hits = matches_in(&text, &query, regex, locale)?;
    let next = replace_all(&text, &hits, &replace);
    let spliced = splice_slice(session, &path, start, next);
    write_document(session, &path, spliced.clone());
    let saved = commit(session)?;
    audit(
        session,
        "direct_write",
        &format!("op:search.replace_file {}", path),
    );
    Ok(json!({ "count": hits.len(), "path": path, "save": saved }))
}

/// `search.project`（`Ctrl+Shift+F`。LSP の `workspace/symbol`）。
pub fn search_project(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let query = str_arg(locale, args, "query")?;
    let regex = bool_or(args, "regex", false);
    session.bench.commit();
    let hits = if regex {
        let pattern = regex::Regex::new(&query).map_err(|_| Error::usage(locale, "query"))?;
        let mut out = Vec::new();
        for document in session.aggregate().documents() {
            for (index, line) in document.text.split('\n').enumerate() {
                if pattern.is_match(line) {
                    out.push(crate::app::assist::Hit {
                        path: document.path.clone(),
                        line: index,
                        text: line.trim().to_owned(),
                        range: 0..line.len(),
                    });
                }
            }
        }
        out
    } else {
        crate::app::assist::search(session.aggregate(), &query)
    };
    let items: Vec<Value> = hits
        .iter()
        .map(|hit| {
            json!({
                "line": hit.line + 1,
                "path": hit.path,
                "text": hit.text,
            })
        })
        .collect();
    Ok(json!({ "count": items.len(), "hits": items, "query": query }))
}

/// `search.find_next`（`F3`）。
pub fn find_next(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    step_find(session, true)
}

/// `search.find_prev`（`Shift+F3`）。
pub fn find_prev(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    step_find(session, false)
}

fn step_find(session: &mut Session, forward: bool) -> Result<Value> {
    let locale = session.locale;
    let Some(find) = session.find.clone() else {
        return Err(Error::new(
            Class::Usage,
            i18n::text(locale, "api.error.no_find").to_owned(),
        ));
    };
    let here = find.hits.get(find.selected).cloned().unwrap_or(0..0);
    let next = crate::app::assist::step(&find.hits, here, forward);
    match next {
        Some(range) => {
            if let Some(current) = session.find.as_mut() {
                if let Some(index) = current.hits.iter().position(|hit| *hit == range) {
                    current.selected = index;
                }
            }
            Ok(json!({
                "range": [range.start, range.end],
                "selected": session.find.as_ref().map(|find| find.selected),
            }))
        }
        None => Ok(json!({ "range": Value::Null, "selected": find.selected })),
    }
}

// =========================================================================== 共通部品

/// 文書を書き換える（メモリ上の集約と編集状態の両方へ）。
pub fn write_document(session: &mut Session, path: &str, text: String) {
    crate::app::ops::write_document(session, path, text);
}

/// 節の本文を差し替えた文書全文（`sections::splice`）。
fn splice_section(session: &mut Session, scene: SceneId, text: &str) -> Result<(String, String)> {
    let locale = session.locale;
    session.bench.commit();
    let list = sections::all(session.aggregate());
    let Some(found) = list.iter().find(|item| item.id == scene) else {
        return Err(Error::not_found(
            locale,
            &crate::app::ops::id_text(session, scene),
            None,
        ));
    };
    let path = found.path.clone();
    let document_text = session.aggregate().text(&path).to_owned();
    let next = sections::splice(&document_text, found, text);
    Ok((path, next))
}

/// 話プロットの節を差し替えた文書全文。
fn splice_plot_section(
    session: &mut Session,
    scene: SceneId,
    text: &str,
) -> Result<(String, String)> {
    let locale = session.locale;
    let project = session.aggregate();
    let path = project.episode_plot_path(scene.chapter, scene.episode);
    let rows = plots::episode_rows(project, scene.chapter, scene.episode);
    let Some(row) = rows.iter().find(|row| row.id == scene) else {
        return Err(Error::not_found(
            locale,
            &crate::app::ops::id_text(session, scene),
            Some(&path),
        ));
    };
    let document = project.text(&path).to_owned();
    let mut lines: Vec<&str> = document.split('\n').collect();
    let start = row.lines.start.min(lines.len());
    let end = row.lines.end.min(lines.len());
    let mut next: Vec<&str> = text.split('\n').collect();
    if next.last() == Some(&"") {
        next.pop();
    }
    lines.splice(start..end, next);
    Ok((path, lines.join("\n")))
}

/// `state:` 行の差し替え（話プロット → 全体 / 章プロットの順に探す）。
fn plot_state_edit(project: &Project, scene: SceneId, value: &str) -> Option<(String, String)> {
    let rows = plots::episode_rows(project, scene.chapter, scene.episode);
    let path = project.episode_plot_path(scene.chapter, scene.episode);
    if let Some(row) = rows.iter().find(|row| row.id == scene) {
        let text = project.text(&path).to_owned();
        return Some((
            path.clone(),
            set_state_line(&text, row.lines.start, row.lines.end, value),
        ));
    }
    for chapter in [Some(scene.chapter), None] {
        let blocks = plots::outline_blocks(project, chapter);
        if let Some(block) = blocks.iter().find(|block| block.id == scene) {
            let text = project.text(&block.path).to_owned();
            return Some((
                block.path.clone(),
                set_state_line(&text, block.lines.start, block.end, value),
            ));
        }
    }
    None
}

fn set_state_line(text: &str, start: usize, end: usize, value: &str) -> String {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let start = start.min(lines.len());
    let end = end.min(lines.len());
    for line in &mut lines[start..end] {
        if line.trim_start().starts_with("state:") {
            *line = "state: ";
            return lines
                .join("\n")
                .replace("state: \n", &format!("state: {value}\n"));
        }
    }
    lines.insert(end, "state: draft");
    let joined = lines.join("\n");
    joined.replacen("state: draft", &format!("state: {value}"), 1)
}

/// 編集中の文書（節エディタか Slice のどちらか）と、その文書の中の開始 byte。
fn editing_document(session: &mut Session) -> Result<(String, String, usize)> {
    let locale = session.locale;
    session.bench.commit();
    if let Some((document, start)) = session.bench.spliced_document() {
        return Ok((document.path.clone(), document.text.clone(), start));
    }
    if let Some((document, start)) = session.bench.slice_document() {
        return Ok((document.path.clone(), document.text.clone(), start));
    }
    Err(Error::new(
        Class::NotFound,
        i18n::text(locale, "api.error.no_document").to_owned(),
    ))
}

/// Slice の中身を差し替えた文書全文。
fn splice_slice(session: &mut Session, path: &str, _start: usize, text: String) -> String {
    if session
        .bench
        .slice()
        .is_some_and(|slice| slice.path == path)
    {
        let document = session.aggregate().text(path).to_owned();
        if let Some(slice) = session.bench.slice().cloned() {
            let mut lines: Vec<&str> = document.split('\n').collect();
            let start = slice.lines.start.min(lines.len());
            let end = slice.lines.end.min(lines.len());
            let mut next: Vec<&str> = text.split('\n').collect();
            if next.last() == Some(&"") {
                next.pop();
            }
            lines.splice(start..end, next);
            return lines.join("\n");
        }
    }
    text
}

/// 検索の一致（`regex` が `true` なら正規表現、§9.7.4）。
fn matches_in(
    text: &str,
    query: &str,
    regex: bool,
    locale: Locale,
) -> Result<Vec<std::ops::Range<usize>>> {
    if !regex {
        return Ok(crate::app::assist::find_all(text, query));
    }
    let pattern = regex::Regex::new(query).map_err(|_| Error::usage(locale, "query"))?;
    Ok(pattern
        .find_iter(text)
        .map(|found| found.range())
        .collect::<Vec<_>>())
}

/// 一致を後ろから置換する（範囲がずれないように）。
fn replace_all(text: &str, hits: &[std::ops::Range<usize>], replace: &str) -> String {
    let mut out = text.to_owned();
    for range in hits.iter().rev() {
        out.replace_range(range.clone(), replace);
    }
    out
}

/// 章（引数が無ければ編集中の節の章）。
pub(crate) fn chapter_of(session: &Session, args: &Args) -> Option<u16> {
    args.get("chapter")
        .and_then(Value::as_u64)
        .map(|value| value as u16)
        .or_else(|| session.bench.current_id().map(|scene| scene.chapter))
}

/// `nav.*` が使う構造移動（`Workbench::go`）。
pub fn go(session: &mut Session, nav: Nav) -> bool {
    session.remember();
    session.bench.go(nav)
}

/// 構造移動の名前（`nav.*` の内部用）。
pub fn nav_name(name: &str) -> Option<Nav> {
    nav_of(name)
}

/// 移動履歴の現在位置（`session.history`）。
pub fn place_of(session: &Session) -> Option<&Place> {
    session.history.get(session.history_at.saturating_sub(1))
}
