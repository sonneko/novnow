//! `export.text`・`settings.*`・`session.*`・`help.topic`・`api.list` の実装（§19.7）。

use serde_json::{json, Value};

use crate::app::command::Command;
use crate::app::error::{Class, Error, Result};
use crate::app::event::AppEvent;
use crate::app::ops::{self, Args};
use crate::app::session::{str_arg, str_or, usize_opt, ExportQuery, Session};
use crate::app::settings::{LanguageChoice, PaneSide, UiSettings};
use crate::core::export::{self, Format, State};
use crate::ui::i18n::{self, Locale};

/// 書き出し先の特例（§13.1.1）。
type Kind = crate::ports::dialog::ExportKind;

// =========================================================================== export.text

/// `export.text`（§17.2。テキストの返却だけなら無関門、`out` 指定のときだけ関門）。
pub fn export_text(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let range = str_or(args, "range", "").to_owned();
    let parsed = if range.is_empty() {
        None
    } else {
        Some(export::parse_range(&range).map_err(|_| Error::usage(locale, "range"))?)
    };
    let query = ExportQuery {
        range: range.clone(),
        state: str_or(args, "state", "").to_owned(),
        format: str_or(args, "format", "").to_owned(),
        out: str_or(args, "out", "").to_owned(),
        blurb: args.get("blurb").and_then(Value::as_bool).unwrap_or(false),
        blurb_only: args
            .get("blurb_only")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        keep_illustration_memo: args
            .get("keep_illustration_memo")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        stock: args.get("stock").and_then(Value::as_bool).unwrap_or(false),
    };
    let mut options = export::Options::from_settings(&session.aggregate().settings().export);
    if let Some(state) = State::parse(&query.state) {
        options.state = Some(state);
    } else if !query.state.is_empty() {
        return Err(Error::usage(locale, "state"));
    }
    if !query.format.is_empty() {
        options.format = Format::of_name(&query.format);
    }
    options.include_blurb |= query.blurb;
    options.blurb_only |= query.blurb_only;
    options.keep_illustration_memo |= query.keep_illustration_memo;
    options.include_stock |= query.stock;
    session.bench.commit();
    let chosen = export::select_with(session.aggregate(), parsed.as_ref(), &options);
    let text = export::render(session.aggregate(), &chosen, &options);
    let label = export::range_label(parsed.as_ref(), session.aggregate().digits());
    let title = session.aggregate().settings().title.clone();
    let stamp = session.host.stamp();
    let name = export::file_name(&title, &label, &stamp, options.format);
    if query.out.is_empty() {
        return Ok(json!({
            "chars": text.chars().count(),
            "format": format!("{:?}", options.format),
            "name": name,
            "range": label,
            "sections": chosen.len(),
            "text": text,
        }));
    }
    // `out` は「利用者が明示した書き出し先」の特例（§13.1.1・§17.2）。MCP では `role` が要る
    let role = ops::gate(session, args)?;
    let written = session
        .host
        .write_chosen(&query.out, text.as_bytes(), Kind::Text)
        .map_err(|message| Error {
            path: Some(query.out.clone()),
            ..Error::new(Class::Io, message)
        })?;
    if role.is_some() {
        ops::audit(
            session,
            "direct_write",
            &format!("op:export.text {}", query.out),
        );
    }
    session.push(AppEvent::DocumentChanged {
        path: written.clone(),
        change: "written".to_owned(),
    });
    Ok(json!({
        "chars": text.chars().count(),
        "name": name,
        "out": written,
        "range": label,
        "sections": chosen.len(),
    }))
}

// =========================================================================== settings.*

/// `settings.get`（§4.4.1 の各キー）。
pub fn settings_get(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let text = session.settings.to_json();
    let parsed: Value = serde_json::from_str(&text).unwrap_or(json!({}));
    Ok(json!({
        "language": language_name(session.locale),
        "path": crate::app::settings::UI_JSON,
        "settings": parsed,
    }))
}

/// `settings.set`（§4.4.1。MCP には出さない安全上の除外、§19.4.3）。
pub fn settings_set(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let key = str_arg(locale, args, "key")?;
    let value = match args.get("value") {
        Some(found) => found.clone(),
        None => return Err(Error::missing(locale, "value")),
    };
    let mut raw: Value = serde_json::from_str(&session.settings.to_json()).unwrap_or(json!({}));
    if let Some(map) = raw.as_object_mut() {
        map.insert(key.clone(), value.clone());
    }
    let next = UiSettings::parse(&raw.to_string());
    session.settings = next;
    session.flush_settings()?;
    session.push(AppEvent::SettingsChanged { key: key.clone() });
    if key == "language" {
        let language = language_of(session.locale, value.as_str().unwrap_or("system"));
        session.set_locale(language);
    }
    Ok(json!({ "key": key, "language": language_name(session.locale), "value": value }))
}

fn language_name(locale: Locale) -> &'static str {
    match locale {
        Locale::Ja => "ja",
        Locale::En => "en",
    }
}

fn language_of(current: Locale, name: &str) -> Locale {
    match name {
        "ja" => Locale::Ja,
        "en" => Locale::En,
        _ => current,
    }
}

// =========================================================================== session.*

/// `session.workspace_get`（§11.1 の 4 種）。
pub fn workspace_get(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    Ok(json!({
        "current": crate::app::session::workspace_name(session.switcher.current()),
        "previous": session
            .switcher
            .previous()
            .map(crate::app::session::workspace_name),
        "workspaces": crate::app::workspace::Workspace::ALL
            .iter()
            .map(|workspace| {
                json!({
                    "available": workspace.available(),
                    "name": crate::app::session::workspace_name(*workspace),
                    "number": workspace.number(),
                })
            })
            .collect::<Vec<_>>(),
    }))
}

/// `session.workspace_switch`（`Ctrl+1`〜`4`・`Ctrl+Shift+W`）。
pub fn workspace_switch(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let name = str_arg(locale, args, "workspace")?;
    let Some(workspace) = crate::app::session::workspace_of(&name) else {
        return Err(Error::usage(locale, "workspace"));
    };
    if !workspace.available() {
        return Err(Error::new(
            Class::Usage,
            i18n::format(locale, "api.error.workspace_unavailable", &[&name]),
        ));
    }
    let moved = session.switch_workspace(workspace);
    Ok(json!({
        "current": crate::app::session::workspace_name(session.switcher.current()),
        "moved": moved,
    }))
}

/// `session.recent_document`（`Ctrl+Tab` の循環）。
pub fn recent_document(session: &mut Session, args: &Args) -> Result<Value> {
    let back = args.get("back").and_then(Value::as_bool).unwrap_or(false);
    let places: Vec<String> = session
        .recent
        .iter()
        .map(|place| place.path.clone())
        .collect();
    let mut index = session.recent_at;
    if places.is_empty() {
        return Ok(json!({ "moved": false, "recent": places }));
    }
    index = if back {
        index.checked_sub(1).unwrap_or(places.len() - 1)
    } else {
        (index + 1) % places.len()
    };
    session.recent_at = index;
    let place = session.recent[index].clone();
    session
        .bench
        .jump(&crate::app::ops::id_text(session, place.scene));
    Ok(json!({
        "moved": true,
        "path": place.path,
        "recent": places,
        "scene": crate::app::ops::id_text(session, place.scene),
    }))
}

/// `session.command`（§19.2.3: パレットの全コマンドを名前で実行する）。
///
/// どのコマンド名も受け付ける（MUST）。中身は 4 通り:
///
/// 1. データ操作に相当するコマンドは対応する正規操作（`api.list` の `command` 列）へそのまま
///    渡す（§19.1.4「手書きの複製を置かない」）。名前だけでは必須の引数が埋まらないときは、
///    GUI が入力欄を開くのと同じく `needs`（続き）を返す。
/// 2. 設定だけを変えるコマンド（テーマ・言語・文字サイズ・ペイン）はセッションの設定へ当てる。
/// 3. 構造移動（§9.8.2）はセッションの位置だけを変える（`moved`）。
/// 4. GUI の枠だけを変えるコマンド（パレットを開く・フォーカス・モーダル・タブ）は `gui_only`
///    として受け付ける（セッションの状態は変えない）。
pub fn command(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let name = str_arg(locale, args, "name")?;
    let Some((command, label)) = crate::app::session::command_of_name(&name) else {
        return Err(Error::usage(locale, "name"));
    };
    let mut payload = args.clone();
    payload.remove("name");
    // (1) パレットが文脈から引数を補うコマンド（今の章・直前の文書・今の節。§19.2.3）。
    //     カタログの `command` 列より先に見る（名前だけでは埋まらない引数を GUI と同じ既定で埋める）
    let mapped: Option<(&'static str, Value)> = match command {
        Command::FindNext(false) => Some(("search.find_next", json!({}))),
        Command::FindNext(true) => Some(("search.find_prev", json!({}))),
        Command::RecentFile(back) => Some(("session.recent_document", json!({ "back": back }))),
        Command::Modes => Some(("dictionary.modes_list", json!({}))),
        Command::Term => Some(("dictionary.term_full", Value::Object(payload.clone()))),
        Command::MaterialSearch => Some(("materials.search", Value::Object(payload.clone()))),
        Command::Complete => Some(("lang.completion", Value::Object(payload.clone()))),
        Command::QuickFix => Some(("lang.code_action", Value::Object(payload.clone()))),
        Command::Fold(fold) => Some((
            "plot.outline_fold",
            json!({ "fold": fold, "chapter": payload.get("chapter").cloned().unwrap_or(json!(1)) }),
        )),
        Command::Workspace(number) => Some((
            "session.workspace_switch",
            json!({ "workspace": workspace_of_number(number).unwrap_or("write") }),
        )),
        Command::PreviousWorkspace => session.switcher.previous().map(|workspace| {
            (
                "session.workspace_switch",
                json!({ "workspace": crate::app::session::workspace_name(workspace) }),
            )
        }),
        _ => None,
    };
    if let Some((op, arguments)) = mapped {
        let given = match &arguments {
            Value::Object(map) => map.clone(),
            _ => Args::new(),
        };
        if !ops::missing(op, &given).is_empty() {
            return Ok(continuation(&name, label, op, &given));
        }
        let data = session.run(op, arguments)?;
        return Ok(json!({ "command": name, "data": data, "label": label, "op": op }));
    }
    // (2) カタログの `command` 列に載る操作（§19.8.2 の単一のカタログから引く）
    if let Some(op) = ops::OPS
        .iter()
        .find(|def| def.command == Some(name.as_str()))
        .map(|def| def.id)
    {
        if !ops::missing(op, &payload).is_empty() {
            return Ok(continuation(&name, label, op, &payload));
        }
        let data = session.run(op, Value::Object(payload))?;
        return Ok(json!({ "command": name, "data": data, "label": label, "op": op }));
    }
    // 直前のワークスペースへ戻す（§9.8.1）。戻り先が無ければ動かないだけ。
    if matches!(command, Command::PreviousWorkspace) {
        return Ok(json!({
            "command": name,
            "label": label,
            "moved": false,
            "workspace": crate::app::session::workspace_name(session.switcher.current()),
        }));
    }
    // (3) 構造移動（§9.8.2。セッションの状態だけを変える）
    if let Command::Nav(nav) = command {
        let moved = crate::app::ops::editing::go(session, nav);
        return Ok(json!({
            "command": name,
            "label": label,
            "moved": moved,
            "scene": session
                .workbench()
                .current_id()
                .map(|scene| crate::app::ops::id_text(session, scene)),
        }));
    }
    // (4) 設定だけを変えるコマンド（§9.8.1・§9.1）
    let key = match command {
        Command::CycleTheme => {
            session.settings.theme = session.settings.theme.next();
            "theme"
        }
        Command::ToggleLanguage => {
            session.settings.language = match session.locale {
                Locale::Ja => LanguageChoice::En,
                Locale::En => LanguageChoice::Ja,
            };
            session.set_locale(match session.locale {
                Locale::Ja => Locale::En,
                Locale::En => Locale::Ja,
            });
            "language"
        }
        Command::FontUp => {
            session.settings.step_font(true);
            "font_scale"
        }
        Command::FontDown => {
            session.settings.step_font(false);
            "font_scale"
        }
        Command::FontReset => {
            session.settings.font_scale = 1.0;
            "font_scale"
        }
        Command::ToggleReference => {
            session.settings.reference_visible = !session.settings.reference_visible;
            "reference_visible"
        }
        Command::InlayHints => {
            session.settings.inlay_hints = !session.settings.inlay_hints;
            "inlay_hints"
        }
        Command::MainRatio(grow) => {
            session.settings.step_ratio(true, grow);
            "main_ratio"
        }
        Command::ContextRatio(grow) => {
            session.settings.step_ratio(false, grow);
            "context_ratio"
        }
        Command::SwapReference => {
            session.settings.reference_pane_side = match session.settings.reference_pane_side {
                PaneSide::Right => PaneSide::Left,
                PaneSide::Left => PaneSide::Right,
            };
            "reference_pane_side"
        }
        Command::StatsMode => {
            session.settings.status_stats_mode = !session.settings.status_stats_mode;
            "status_stats_mode"
        }
        Command::SuggestComma => {
            session.settings.suggest_comma = !session.settings.suggest_comma;
            "suggest_comma"
        }
        // GUI の枠だけを変えるコマンド（パレットを開く・ペインへ移動する・タブを替える）。
        // 名前では受け付けるが、セッションの状態は変えない（§19.2.3「全コマンドを名前で実行する」）。
        _ => {
            return Ok(json!({
                "command": name,
                "gui_only": true,
                "label": label,
            }))
        }
    };
    session.flush_settings()?;
    session.push(AppEvent::SettingsChanged {
        key: key.to_owned(),
    });
    Ok(json!({
        "command": name,
        "key": key,
        "label": label,
        "value": serde_json::to_value(session.settings.to_json()).unwrap_or(Value::Null),
    }))
}

/// コマンドの名前だけでは埋まらない必須引数があるときの戻り（§19.2.3）。
///
/// GUI ではコマンドを選ぶと入力欄が開く。セッションには入力欄が無いので、呼び出し側が
/// 型付き操作（`op`）へ何を渡せばよいのか（`needs`）を返す。
fn continuation(name: &str, label: &str, op: &str, given: &Args) -> Value {
    json!({
        "command": name,
        "label": label,
        "needs": ops::missing(op, given),
        "op": op,
    })
}

/// `session.poll_events`（§19.2.4 のイベントキュー）。
pub fn poll_events(session: &mut Session, args: &Args) -> Result<Value> {
    let limit = usize_opt(args, "limit").unwrap_or(64);
    let drained: Vec<Value> = session
        .poll_events()
        .into_iter()
        .take(limit)
        .map(|event| event.to_json())
        .collect();
    Ok(json!({ "count": drained.len(), "events": drained }))
}

// =========================================================================== help.topic / api.list

/// ヘルプの主題（`>` + `help`・`welcome`・`migration`）。
const TOPICS: &[&str] = &["help", "welcome", "migration", "shortcuts", "api"];

/// `help.topic`（ヘルプ本文。§19.7）。
pub fn help_topic(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    // `>help` はショートカット一覧（§19.7）
    let topic = str_or(args, "topic", "shortcuts").to_owned();
    if !TOPICS.contains(&topic.as_str()) {
        return Err(Error::usage(locale, "topic"));
    }
    let (title, text) = match topic.as_str() {
        "welcome" => (
            i18n::text(locale, "help.welcome_title").to_owned(),
            i18n::text(locale, "help.welcome").to_owned(),
        ),
        "migration" => (
            i18n::text(locale, "help.migration_title").to_owned(),
            i18n::text(locale, "help.migration").to_owned(),
        ),
        "shortcuts" => (
            i18n::text(locale, "help.title").to_owned(),
            shortcuts(locale),
        ),
        "api" => (
            i18n::text(locale, "help.website").to_owned(),
            api_summary(locale),
        ),
        _ => (
            i18n::text(locale, "help.palette").to_owned(),
            commands_text(locale),
        ),
    };
    Ok(json!({ "text": text, "title": title, "topic": topic }))
}

/// コマンドの一覧（`>` の中身。§19.2.3）。ラベルはカタログのキー（`command.theme`）か
/// ショートカットの文字列（`Ctrl+P`）なので、前者だけカタログから引く。
fn commands_text(locale: Locale) -> String {
    let mut out = Vec::new();
    for name in crate::app::session::command_names() {
        let Some((_, label)) = crate::app::session::command_of_name(name) else {
            continue;
        };
        let shown = label_text(locale, label);
        match ops::OPS
            .iter()
            .find(|def| def.command == Some(name))
            .map(|def| def.id)
        {
            Some(op) => out.push(format!("{name} ({shown}): {op}")),
            None => out.push(format!("{name} ({shown})")),
        }
    }
    out.sort();
    out.join("\n")
}

/// ショートカットの一覧（§12 の表。ラベルがキーでないもの＝キーの文字列）。
fn shortcuts(locale: Locale) -> String {
    let mut out = Vec::new();
    for name in crate::app::session::command_names() {
        let Some((_, label)) = crate::app::session::command_of_name(name) else {
            continue;
        };
        if is_catalog_key(label) {
            continue;
        }
        out.push(format!("{}: {}", label_text(locale, label), name));
    }
    out.sort();
    out.join("\n")
}

/// ラベルがカタログのキー（`command.theme`）か。
fn is_catalog_key(label: &str) -> bool {
    label.starts_with("command") && label.contains('.')
}

/// ラベルの表示（キーならカタログから、キーでなければそのまま）。
fn label_text(locale: Locale, label: &str) -> String {
    if is_catalog_key(label) {
        i18n::text(locale, label).to_owned()
    } else {
        label.to_owned()
    }
}

/// 正規操作の要約（`api.list` の一覧と同じ）。
fn api_summary(locale: Locale) -> String {
    let mut out = Vec::new();
    for op in ops::OPS {
        out.push(format!("{}: {}", op.id, ops::description(locale, op.id)));
    }
    out.join("\n")
}

/// `api.list`（§19.8.2 の機械可読カタログ。全インターフェースが同じものから生成する）。
pub fn api_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    Ok(ops::catalog(session.locale))
}

/// 設定の言語（`settings.language` → `Locale`）。
pub fn locale_of(choice: LanguageChoice) -> Locale {
    match choice {
        LanguageChoice::Ja => Locale::Ja,
        LanguageChoice::En => Locale::En,
        LanguageChoice::System => Locale::Ja,
    }
}

/// ワークスペースの番号（§10 の表の `#`）から名前へ。
fn workspace_of_number(number: u8) -> Option<&'static str> {
    crate::app::workspace::Workspace::ALL
        .iter()
        .find(|workspace| workspace.number() == number)
        .map(|workspace| crate::app::session::workspace_name(*workspace))
}
