//! `project.*` と `structure.*` の実装（§19.7）。

use serde_json::{json, Value};

use crate::app::error::{Class, Error, Result};
use crate::app::ops::{commit, via_mcp, Args};
use crate::app::session::{bool_or, str_arg, str_or, usize_opt, Session};
use crate::core::features::sections;
use crate::ui::i18n::{self, Locale};

/// `project.info`（§19.7）。既存の MCP ツールと同じ関数を使う（§19.1.1）。
pub fn info(session: &mut Session, args: &Args) -> Result<Value> {
    via_mcp(session, "novnow.project.info", args)
}

/// `project.validate`（§14.2.4 の一覧。`novnow check` と同じ判定）。
pub fn validate(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    use crate::core::model::project::{validate as check, Listing};
    let locale = session.locale;
    let project = session.aggregate();
    let mut files = std::collections::BTreeSet::new();
    for document in project.documents() {
        files.insert(document.path.clone());
    }
    let listing = Listing {
        files,
        dirs: std::collections::BTreeSet::new(),
        dirs_implicit: true,
        project_toml: project
            .document(crate::core::model::project::PROJECT_FILE)
            .map(|document| document.text.clone()),
        invalid_utf8: Vec::new(),
    };
    let structure = match check(&listing) {
        Ok(()) => Value::Null,
        Err(error) => match error {
            crate::core::model::project::ProjectError::Incomplete { missing } => json!({
                "kind": "S022",
                "message": i18n::format(
                    locale,
                    "api.error.incomplete",
                    &[&missing
                        .iter()
                        .map(|path| path.to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join(", ")]
                ),
            }),
            crate::core::model::project::ProjectError::TooNew { found } => json!({
                "kind": "S023",
                "message": i18n::format(locale, "api.error.too_new", &[&found.to_string()]),
            }),
            crate::core::model::project::ProjectError::Encoding { files } => json!({
                "kind": "S024",
                "message": i18n::format(
                    locale,
                    "api.error.encoding",
                    &[&files
                        .iter()
                        .map(|path| path.to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join(", ")]
                ),
            }),
        },
    };
    let diagnostics: Vec<Value> = session
        .aggregate()
        .documents()
        .flat_map(|document| {
            let text = document.text.clone();
            crate::core::diagnostics::document(document, session.aggregate())
                .into_iter()
                .map(|found| {
                    crate::app::ops::diagnostic_json(locale, &document.path, &text, &found)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let errors = diagnostics
        .iter()
        .filter(|found| found["severity"] == json!("error"))
        .count();
    Ok(json!({
        "diagnostics": diagnostics,
        "errors": errors,
        "ok": structure.is_null() && errors == 0,
        "structure": structure,
    }))
}

/// `project.new`（`novnow new` と同じ構造を作る。§19.7 では L—・M—）。
pub fn new(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let path = str_arg(locale, args, "path")?;
    let title = str_or(args, "title", "");
    create_project(locale, &path, title)
}

/// 雛形を書いて新しいプロジェクトを作る（ネイティブのみ。§13.1.2 の関門を通る）。
fn create_project(locale: Locale, path: &str, title: &str) -> Result<Value> {
    let _ = locale;
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::app::session::CreateOptions;
        let created = Session::create(
            path,
            CreateOptions {
                title: title.to_owned(),
                ..CreateOptions::default()
            },
        )?;
        let files: Vec<String> = created
            .aggregate()
            .documents()
            .map(|document| document.path.clone())
            .collect();
        Ok(json!({ "files": files, "path": path, "title": title }))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (locale, path, title);
        Err(Error::new(
            Class::Io,
            i18n::text(locale, "api.error.no_fs").to_owned(),
        ))
    }
}

/// `project.reload`（パレット `>reload`。§19.7 では M—・C—）。
pub fn reload(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(root) = session.root.clone() {
        let interface = session.interface;
        let locale = session.locale;
        let reopened = Session::open_with(&root, interface)?;
        let files: Vec<String> = reopened
            .aggregate()
            .documents()
            .map(|document| document.path.clone())
            .collect();
        *session = reopened;
        session.set_locale(locale);
        return Ok(json!({ "files": files.len(), "reloaded": true }));
    }
    let _ = session.locale;
    Ok(json!({ "files": session.aggregate().documents().count(), "reloaded": false }))
}

/// `project.stats`（執筆統計。`.novnow/stats.json` の算出と同じ材料）。
pub fn stats(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let project = session.aggregate();
    let metrics = crate::core::stage_two::metrics::project(project);
    let list = sections::all(project);
    let settings = project.settings();
    let speakers: Vec<Value> = metrics
        .speakers
        .iter()
        .map(|(name, speech)| json!({ "chars": speech.chars, "lines": speech.lines, "name": name }))
        .collect();
    Ok(json!({
        "chapters": list.iter().map(|found| found.id.chapter).collect::<std::collections::BTreeSet<_>>().len(),
        "dialogue_chars": metrics.dialogue,
        "dialogue_ratio": metrics.ratio(),
        "newline": settings.newline,
        "sections": list.len(),
        "speakers": speakers,
        "title": settings.title,
        "total_chars": metrics.total,
    }))
}

/// `project.recent_list`（`.novnow/recent.json`。§19.7 では M—）。
pub fn recent_list(session: &mut Session, args: &Args) -> Result<Value> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (args, session);
        use crate::adapters::env_system::SystemEnv;
        use crate::adapters::recent_file::RecentFile;
        use crate::adapters::storage_fs::FsGate;
        use std::sync::Arc;
        let recent = RecentFile::new(Arc::new(FsGate::new(&SystemEnv)));
        let paths = recent.load();
        let items: Vec<Value> = paths
            .items
            .iter()
            .map(|item| json!({ "opened_at": item.opened_at, "path": item.path }))
            .collect();
        Ok(json!({ "projects": items }))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = args;
        Err(Error::new(
            Class::Io,
            i18n::text(session.locale, "api.error.no_fs").to_owned(),
        ))
    }
}

/// `project.recent_remove`（`>` + `forget`。§19.7 では M—）。
pub fn recent_remove(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let path = str_arg(locale, args, "path")?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::adapters::env_system::SystemEnv;
        use crate::adapters::recent_file::RecentFile;
        use crate::adapters::storage_fs::FsGate;
        use std::sync::Arc;
        let recent = RecentFile::new(Arc::new(FsGate::new(&SystemEnv)));
        let paths = recent
            .remove(&path)
            .map_err(|error| Error::io(format!("{error:?}"), None))?;
        Ok(json!({ "projects": paths.items.len(), "removed": path }))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (path, session);
        Err(Error::new(
            Class::Io,
            i18n::text(locale, "api.error.no_fs").to_owned(),
        ))
    }
}

/// `project.import_zip`（§13.5。§19.7 では M—（バイナリ））。
pub fn import_zip(session: &mut Session, args: &Args) -> Result<Value> {
    use crate::adapters::zip;
    use base64_decode::decode;
    let locale = session.locale;
    let bytes = match args.get("bytes").and_then(Value::as_str) {
        Some(text) => decode(text).map_err(|_| Error::usage(locale, "bytes"))?,
        None => {
            let path = str_arg(locale, args, "path")?;
            read_file(&path).map_err(|message| Error::io(message, Some(&path)))?
        }
    };
    let imported =
        zip::import(&bytes).map_err(|error| Error::new(Class::Io, format!("{error:?}")))?;
    let (texts, skipped) = imported.into_texts();
    let mut written = Vec::new();
    for (path, text) in &texts {
        crate::app::ops::write_document(session, path.as_str(), text.clone());
        written.push(path.as_str().to_owned());
    }
    commit(session)?;
    Ok(json!({
        "skipped": skipped.iter().map(|item| json!({ "name": item.name, "reason": format!("{:?}", item.reason) })).collect::<Vec<_>>(),
        "written": written,
    }))
}

/// `project.export_zip`（§13.5。§19.7 では M—（バイナリ））。
pub fn export_zip(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    session.bench.commit();
    let mut files: Vec<(crate::core::policy::write_policy::ProjectPath, Vec<u8>)> = Vec::new();
    for document in session.aggregate().documents() {
        if let Ok(path) = crate::core::policy::write_policy::ProjectPath::new(&document.path) {
            files.push((path, document.text.as_bytes().to_vec()));
        }
    }
    let name = crate::adapters::zip::file_name(&session.host.name());
    let bytes = crate::adapters::zip::export(&files)
        .map_err(|error| Error::new(Class::Io, format!("{error:?}")))?;
    match str_or(args, "out", "") {
        "" => Ok(json!({
            "bytes": base64_decode::encode(&bytes),
            "name": name,
            "size": bytes.len(),
        })),
        out => {
            let written = session
                .host
                .write_chosen(out, &bytes, crate::ports::dialog::ExportKind::Zip)
                .map_err(|message| Error {
                    path: Some(out.to_owned()),
                    ..Error::new(Class::Io, message)
                })?;
            let _ = locale;
            Ok(json!({ "name": name, "out": written, "size": bytes.len() }))
        }
    }
}

/// `structure.tree`（`Ctrl+Shift+T`。既存の MCP ツールと同じ）。
pub fn tree(session: &mut Session, args: &Args) -> Result<Value> {
    via_mcp(session, "novnow.structure.tree", args)
}

/// `structure.resolve`（ジャンプバー `Ctrl+J` の解決）。
pub fn resolve(session: &mut Session, args: &Args) -> Result<Value> {
    use crate::core::id::PartialSceneId;
    let locale = session.locale;
    let query = str_arg(locale, args, "query")?;
    let parsed = PartialSceneId::parse_with_sigil(query.trim())
        .or_else(|_| PartialSceneId::parse(query.trim()))
        .map_err(|_| Error::usage(locale, "query"))?;
    let target = parsed.expand_low();
    let list = sections::all(session.aggregate());
    let found = list.iter().find(|item| item.id == target);
    match found {
        Some(section) => Ok(json!({
            "id": crate::app::ops::id_text(session, section.id),
            "lines": [section.lines.start, section.lines.end],
            "path": section.path,
            "resolved": true,
            "title": section.scene_title,
        })),
        None => Ok(json!({
            "id": crate::app::ops::id_text(session, target),
            "resolved": false,
        })),
    }
}

/// `structure.breadcrumb`（§19.7 では M—・C—）。
pub fn breadcrumb(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    Ok(json!({
        "breadcrumb": session.bench.breadcrumb().unwrap_or_default(),
        "scene": session.bench.current_id().map(|id| crate::app::ops::id_text(session, id)),
    }))
}

/// ファイルを読む（Zip の取り込み・`export.text --out` の逆。読み取りは規則 10 の対象外）。
#[cfg(not(target_arch = "wasm32"))]
fn read_file(path: &str) -> std::result::Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn read_file(path: &str) -> std::result::Result<Vec<u8>, String> {
    Err(format!("cannot read {path} here"))
}

/// base64（追加依存を置かないための最小実装。§19.3.2「Zip 操作は bytes」）。
pub(crate) mod base64_decode {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    /// エンコード（改行なし）。
    pub fn encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let value = u32::from(chunk[0]) << 16
                | u32::from(*chunk.get(1).unwrap_or(&0)) << 8
                | u32::from(*chunk.get(2).unwrap_or(&0));
            out.push(TABLE[((value >> 18) & 63) as usize] as char);
            out.push(TABLE[((value >> 12) & 63) as usize] as char);
            out.push(if chunk.len() > 1 {
                TABLE[((value >> 6) & 63) as usize] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                TABLE[(value & 63) as usize] as char
            } else {
                '='
            });
        }
        out
    }

    /// デコード（`=` のパディングと空白を許す）。
    pub fn decode(text: &str) -> std::result::Result<Vec<u8>, ()> {
        let mut out = Vec::with_capacity(text.len() / 4 * 3);
        let mut buffer: u32 = 0;
        let mut bits = 0u32;
        for character in text.bytes() {
            if character == b'=' || character.is_ascii_whitespace() {
                continue;
            }
            let value = TABLE.iter().position(|item| *item == character).ok_or(())? as u32;
            buffer = (buffer << 6) | value;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buffer >> bits) as u8);
            }
        }
        Ok(out)
    }
}

/// `diagnostics.run` のスコープ（`single` / `file` / `full`。§9.1.1）。
pub fn scope_of(args: &Args) -> &'static str {
    match str_or(args, "scope", "full") {
        "single" => "single",
        "file" => "file",
        _ => "full",
    }
}

/// `plot.outline_fold` の折り畳み状態（§19.7 の注記: 永続化は `folded` キーの導入とともに）。
pub fn fold_flag(args: &Args) -> bool {
    bool_or(args, "fold", true)
}

/// 章番号（`plot.*` の `chapter` 引数。無ければ今の節の章）。
pub fn chapter_of(session: &Session, args: &Args) -> Option<u16> {
    usize_opt(args, "chapter")
        .map(|value| value as u16)
        .or_else(|| session.bench.current_id().map(|id| id.chapter))
}
