//! パレットの候補とコマンドへの解決（§9.7.1）。候補の採点・履歴・モード切替は
//! `app::palette`（egui に依存しない層）が担い、ここは画面とのやり取りの形を作る。

use crate::app::workspace::Workspace;
use crate::ui::app::Session;
use crate::ui::i18n::{self, Locale};
use crate::ui::palette;
use crate::ui::shell::Command;
use crate::ui::workbench::Workbench;

/// パレットの候補 1 つ。
pub(crate) struct PaletteItem {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) action: PaletteAction,
}

#[derive(Debug, Clone)]
pub(crate) enum PaletteAction {
    Section(usize),
    /// `@` の章 → 話 → 場面 → 節の木で、1 つ下の階層を開く（§9.7.1）。
    SceneDrill(String),
    Command(Command),
    /// `M` / `#` の資料（material ワークスペースで開く）。
    Material(String),
    /// `'` のキャラ（`characters.md` の H1 の順。cast で開く）。
    Character(usize),
    /// `'` の用語（`words.md` の H1 の順。全文を出す）。
    Term(usize),
    /// エージェントのコマンド（`>agent …` / `>prompt …`）。
    Agent(AgentCommand),
    /// `>zip`: Zip でエクスポートする（§13.5）。
    ExportZip,
    /// パレットを閉じる。
    Close,
    /// `>agent` 階層の入口。
    AgentRoot,
    LineWidth,
    Welcome,
    Migration,
    McpConfig,
}

/// パレットのエージェントのコマンド（§15.7.6・§15.5.4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentCommand {
    /// `>agent delegation <role> <level>`。
    Level(String, crate::core::model::delegation::Delegation),
    /// `>agent stop-all`。
    StopAll,
    /// `>prompt reset <name>`（`all` は全部）。
    PromptReset(String),
    /// `>prompt diff <name>`。
    PromptDiff(String),
}

/// `>prompt reset` / `>prompt diff` の名前（既定の役割・共有の 2 つ）。
pub(crate) fn prompt_names() -> Vec<&'static str> {
    crate::core::template::DEFAULT_PROMPTS
        .iter()
        .map(|(path, _)| crate::core::agents::prompts::file_name(path))
        .collect()
}

/// パレットのエージェントのコマンドの候補（`>agent delegation <role> <level>` は 7 × 4 件）。
pub(crate) fn agent_palette_items(locale: Locale) -> Vec<PaletteItem> {
    use crate::core::model::delegation::Delegation;
    let mut out = Vec::new();
    for role in crate::core::agents::BUILTIN_ROLES {
        for level in [
            Delegation::Off,
            Delegation::Propose,
            Delegation::Apply,
            Delegation::Autonomous,
        ] {
            let key = format!("agent delegation {role} {}", level.as_str());
            out.push(PaletteItem {
                label: format!(
                    ">{key}  {}",
                    i18n::format(locale, "command.agent_delegation", &[role, level.as_str()])
                ),
                key,
                action: PaletteAction::Agent(AgentCommand::Level((*role).to_owned(), level)),
            });
        }
    }
    out.push(PaletteItem {
        key: "agent mcp".into(),
        label: format!(">agent mcp  {}", i18n::text(locale, "help.edit_mcp")),
        action: PaletteAction::McpConfig,
    });
    out.push(PaletteItem {
        key: "agent stop-all".to_owned(),
        label: format!(
            ">agent stop-all  {}",
            i18n::text(locale, "command.agent_stop_all")
        ),
        action: PaletteAction::Agent(AgentCommand::StopAll),
    });
    let mut names = prompt_names();
    names.push("all");
    for name in names {
        let key = format!("prompt reset {name}");
        out.push(PaletteItem {
            label: format!(
                ">{key}  {}",
                i18n::format(locale, "command.prompt_reset", &[name])
            ),
            key,
            action: PaletteAction::Agent(AgentCommand::PromptReset(name.to_owned())),
        });
    }
    for name in prompt_names() {
        let key = format!("prompt diff {name}");
        out.push(PaletteItem {
            label: format!(
                ">{key}  {}",
                i18n::format(locale, "command.prompt_diff", &[name])
            ),
            key,
            action: PaletteAction::Agent(AgentCommand::PromptDiff(name.to_owned())),
        });
    }
    out
}

/// 用語の全文（パレットの `'用語` の `Enter`。`Ctrl+Shift+W` と同じ形: 1 行目が用語）。
pub(crate) fn term_lines(
    project: &crate::core::features::project::Project,
    index: usize,
) -> Vec<String> {
    use crate::core::text::lexer::FieldKey;
    let words = project.words();
    let Some(word) = words.words.get(index) else {
        return Vec::new();
    };
    let mut out = vec![word.term.trim().to_owned()];
    if let Some(definition) = words.word_fields(index, FieldKey::Definition).first() {
        out.extend(definition.full_value().lines().map(str::to_owned));
    }
    out
}

/// `materials/index.json` の byte `at` を含む行までで最後の `"id": "…"` の値（点検から資料を開く）。
pub(crate) fn material_id_before(text: &str, at: usize) -> Option<String> {
    let mut end = at.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let line_end = text[end..]
        .find('\n')
        .map_or(text.len(), |offset| end + offset);
    let key = text[..line_end].rfind("\"id\"")?;
    let rest = text[key + 4..].trim_start().strip_prefix(':')?;
    let rest = rest.trim_start().strip_prefix('"')?;
    let value: String = rest.chars().take_while(|c| *c != '"').collect();
    (!value.is_empty()).then_some(value)
}

/// `characters.md` の `index` 番目のキャラの H1 名前の先頭 byte。
pub(crate) fn character_definition_pos(
    project: &crate::core::features::project::Project,
    index: usize,
) -> Option<usize> {
    project
        .definition_of(&crate::core::resolve::dictionary::Target::Character {
            index,
            name: String::new(),
        })
        .map(|loc| loc.range.start)
}

/// `words.md` の `index` 番目の用語の H1 の先頭 byte。
pub(crate) fn word_definition_pos(
    project: &crate::core::features::project::Project,
    index: usize,
) -> Option<usize> {
    project
        .definition_of(&crate::core::resolve::dictionary::Target::Word {
            index,
            term: String::new(),
        })
        .map(|loc| loc.range.start)
}

/// 今のワークスペースの編集対象の文書とカーソル位置（byte）。
pub(crate) fn active_document_and_caret(
    session: &Session,
    workspace: Workspace,
) -> Option<(crate::core::model::document::Document, usize)> {
    if workspace == Workspace::Write {
        let (document, start) = session.bench.spliced_document()?;
        let caret = start + session.selection.as_ref().map_or(0, |range| range.start);
        Some((document, caret))
    } else {
        let (document, start) = session.bench.slice_document()?;
        let caret = start + session.bench.slice_caret().map_or(0, |range| range.start);
        Some((document, caret))
    }
}

use crate::app::commands::{PALETTE_COMMANDS, SHORTCUT_PALETTE_COMMANDS};

/// 最近選んだ項目へ追加する（新しい順・重複なし・最大 10 件）。
pub(crate) fn push_recent(recent: &mut Vec<String>, key: &str) {
    recent.retain(|found| found != key);
    recent.insert(0, key.to_owned());
    recent.truncate(10);
}

/// 入力が空のときだけ、最近選んだ項目を先頭へ差し込む（固定スコアの計算には入れない）。
/// それ以外の項目の相対順序は変えない。
pub(crate) fn recent_first(mut items: Vec<PaletteItem>, recent: &[String]) -> Vec<PaletteItem> {
    if recent.is_empty() {
        return items;
    }
    let mut head: Vec<PaletteItem> = Vec::new();
    for key in recent {
        if let Some(at) = items.iter().position(|item| &item.key == key) {
            head.push(items.remove(at));
        }
    }
    head.extend(items);
    head
}

pub(crate) fn palette_items(
    bench: &Workbench,
    locale: Locale,
    mode: palette::Mode,
    query: &str,
    input: &str,
    recent: &[String],
) -> Vec<PaletteItem> {
    let mut result: Vec<PaletteItem> = match mode {
        palette::Mode::Command => {
            let mut all: Vec<PaletteItem> = PALETTE_COMMANDS
                .iter()
                .map(|(name, key, command)| PaletteItem {
                    key: (*name).to_owned(),
                    label: format!(">{name}  {}", i18n::text(locale, key)),
                    action: PaletteAction::Command(*command),
                })
                .collect();
            all.extend(
                SHORTCUT_PALETTE_COMMANDS
                    .iter()
                    .map(|(name, shortcut, command)| PaletteItem {
                        key: (*name).to_owned(),
                        label: format!(">{name}  {shortcut}"),
                        action: PaletteAction::Command(*command),
                    }),
            );
            all.push(PaletteItem {
                key: "welcome".into(),
                label: format!(">welcome  {}", i18n::text(locale, "help.welcome_title")),
                action: PaletteAction::Welcome,
            });
            all.push(PaletteItem {
                key: "migration".into(),
                label: format!(">migration  {}", i18n::text(locale, "help.migration_title")),
                action: PaletteAction::Migration,
            });
            all.push(PaletteItem {
                key: "mcp".into(),
                label: format!(">mcp  {}", i18n::text(locale, "help.edit_mcp")),
                action: PaletteAction::McpConfig,
            });
            all.push(PaletteItem {
                key: "line-width".into(),
                label: format!(">line-width  {}", i18n::text(locale, "palette.line_width")),
                action: PaletteAction::LineWidth,
            });
            all.push(PaletteItem {
                key: "zip".to_owned(),
                label: format!(">zip  {}", i18n::text(locale, "command.export_zip")),
                action: PaletteAction::ExportZip,
            });
            if query.starts_with("agent") || query.starts_with("prompt") {
                all.extend(agent_palette_items(locale));
            } else {
                all.push(PaletteItem {
                    // 末尾の空白が、Tab 補完での副コマンドの区切りになる（`>agent ` まで入る）。
                    key: "agent ".into(),
                    label: ">agent  Agent settings ›".into(),
                    action: PaletteAction::AgentRoot,
                });
            }
            let ranked = palette::rank(query, &all, |item| (item.key.as_str(), item.key.as_str()));
            ranked
                .into_iter()
                .map(|item| PaletteItem {
                    key: item.key.clone(),
                    label: item.label.clone(),
                    action: item.action.clone(),
                })
                .collect()
        }
        palette::Mode::Help => {
            let mut all: Vec<_> = PALETTE_COMMANDS
                .iter()
                .map(|(name, key, command)| PaletteItem {
                    key: (*name).to_owned(),
                    label: format!("? {name}  {}", i18n::text(locale, key)),
                    action: PaletteAction::Command(*command),
                })
                .collect();
            all.extend(
                SHORTCUT_PALETTE_COMMANDS
                    .iter()
                    .map(|(name, shortcut, command)| PaletteItem {
                        key: (*name).to_owned(),
                        label: format!("? {name}  {shortcut}"),
                        action: PaletteAction::Command(*command),
                    }),
            );
            palette::rank(query, &all, |item| (&item.key, &item.label))
                .into_iter()
                .map(|item| PaletteItem {
                    key: item.key.clone(),
                    label: item.label.clone(),
                    action: item.action.clone(),
                })
                .collect()
        }
        palette::Mode::Scene => {
            // いちばん広い階層から始める。枝を選ぶと末尾に `-` が付いて次のフレームで
            // 絞られ、葉を選ぶとその節へ飛ぶ。打ち込んだ前置きでも絞り込める（§9.7.1）。
            let depth = query.split('-').count().saturating_sub(1).min(3);
            let digits = bench.project().digits();
            let mut nodes = std::collections::BTreeMap::<String, PaletteItem>::new();
            for (index, section) in bench.sections().iter().enumerate() {
                let full = section.id.display(digits);
                let prefix = full
                    .split('-')
                    .take(depth + 1)
                    .collect::<Vec<_>>()
                    .join("-");
                if !prefix.starts_with(query) {
                    continue;
                }
                let branch = depth < 3;
                let title = match depth {
                    0 => bench.project().all_plot().and_then(|plot| {
                        plot.blocks
                            .iter()
                            .find(|block| {
                                block.id.is_some_and(|id| id.chapter == section.id.chapter)
                            })
                            .and_then(|block| plot.lines.get(block.header))
                            .map(|line| {
                                line.raw()
                                    .split_once(char::is_whitespace)
                                    .map_or("", |(_, title)| title)
                            })
                    }),
                    1 => bench
                        .project()
                        .chapter_plot(section.id.chapter)
                        .and_then(|plot| {
                            plot.blocks
                                .iter()
                                .find(|block| {
                                    block.id.is_some_and(|id| id.episode == section.id.episode)
                                })
                                .and_then(|block| plot.lines.get(block.header))
                                .map(|line| {
                                    line.raw()
                                        .split_once(char::is_whitespace)
                                        .map_or("", |(_, title)| title)
                                })
                        }),
                    _ => Some(section.scene_title.as_str()),
                }
                .unwrap_or("");
                let label = if branch {
                    format!("@{prefix}-*  {title}")
                } else {
                    format!("@{prefix}  {}", section.scene_title)
                };
                nodes.entry(prefix.clone()).or_insert(PaletteItem {
                    key: prefix.clone(),
                    label,
                    action: if branch {
                        PaletteAction::SceneDrill(prefix)
                    } else {
                        PaletteAction::Section(index)
                    },
                });
            }
            nodes.into_values().take(palette::LIMIT).collect()
        }
        palette::Mode::Fuzzy => {
            let digits = bench.project().digits();
            let all: Vec<PaletteItem> = bench
                .sections()
                .iter()
                .enumerate()
                .map(|(index, section)| {
                    let id = section.id.display(digits);
                    PaletteItem {
                        label: format!("@{id}  {}", section.scene_title),
                        key: id,
                        action: PaletteAction::Section(index),
                    }
                })
                .collect();
            let ranked =
                palette::rank(query, &all, |item| (item.key.as_str(), item.label.as_str()));
            ranked
                .into_iter()
                .map(|item| PaletteItem {
                    key: item.key.clone(),
                    label: item.label.clone(),
                    action: item.action.clone(),
                })
                .collect()
        }
        palette::Mode::Material => {
            // `#取材 甲子園` の `#` はカテゴリの語として §16.3 の検索にそのまま渡す（`#` だけなら全部）
            let text = if input.trim_start().starts_with('#') {
                input.trim()
            } else {
                query
            };
            let text = if text == "#" { "" } else { text };
            crate::core::features::materials_ops::search(bench.project(), text)
                .into_iter()
                .take(palette::LIMIT)
                .map(|hit| {
                    let category = if hit.category.is_empty() {
                        String::new()
                    } else {
                        format!("#{}", hit.category)
                    };
                    PaletteItem {
                        label: format!("{}  {}  {}  {}", hit.id, hit.title, category, hit.date)
                            .trim_end()
                            .to_owned(),
                        key: hit.id.clone(),
                        action: PaletteAction::Material(hit.id),
                    }
                })
                .collect()
        }
        palette::Mode::Dictionary => {
            use crate::core::text::lexer::FieldKey;
            let project = bench.project();
            // (鍵, 照合する文字列, 候補)。照合は名前・読み・別名
            let mut all: Vec<(String, String, PaletteItem)> = Vec::new();
            let characters = project.characters();
            for (index, character) in characters.characters.iter().enumerate() {
                let name = character.name.trim().to_owned();
                let mut haystack = name.clone();
                for key in [FieldKey::Reading, FieldKey::Alias] {
                    for field in characters.character_fields(index, key) {
                        haystack.push(' ');
                        haystack.push_str(&field.full_value());
                    }
                }
                all.push((
                    name.clone(),
                    haystack,
                    PaletteItem {
                        label: format!("'{name}  {}", i18n::text(locale, "palette.character")),
                        key: name,
                        action: PaletteAction::Character(index),
                    },
                ));
            }
            let words = project.words();
            for (index, word) in words.words.iter().enumerate() {
                let term = word.term.trim().to_owned();
                // §6.1.4（F-24）: `characters.md` にもある名前は登場人物側に統合して、
                // パレットに同じ名前を 2 行出さない
                if project.dictionary().is_character_word_duplicate(&term) {
                    continue;
                }
                let mut haystack = term.clone();
                for key in [FieldKey::Reading, FieldKey::Alias] {
                    for field in words.word_fields(index, key) {
                        haystack.push(' ');
                        haystack.push_str(&field.full_value());
                    }
                }
                let definition = words
                    .word_fields(index, FieldKey::Definition)
                    .first()
                    .map(|field| field.full_value())
                    .unwrap_or_default();
                let first = definition.lines().next().unwrap_or("").to_owned();
                all.push((
                    term.clone(),
                    haystack,
                    PaletteItem {
                        label: format!("'{term}  {}  {first}", i18n::text(locale, "palette.term"))
                            .trim_end()
                            .to_owned(),
                        key: term,
                        action: PaletteAction::Term(index),
                    },
                ));
            }
            palette::rank(query, &all, |entry| (entry.0.as_str(), entry.1.as_str()))
                .into_iter()
                .map(|entry| PaletteItem {
                    key: entry.2.key.clone(),
                    label: entry.2.label.clone(),
                    action: entry.2.action.clone(),
                })
                .collect()
        }
    };
    result.push(PaletteItem {
        key: "~close".into(),
        label: i18n::text(locale, "palette.close").into(),
        action: PaletteAction::Close,
    });
    // 最近履歴は `>` / `@` で入力が空のときだけ（採点には入れない）
    if query.is_empty() && matches!(mode, palette::Mode::Command | palette::Mode::Scene) {
        return recent_first(result, recent);
    }
    result
}

/// キーボードヘルプ（§9.8 のうち M7・M8 で動くもの）。左はキー、右はカタログのキー。
/// ヘルプのキー一覧（キー、説明の文言キー、コマンド表の名前）。
/// 名前がある項目はボタンになり、ヘルプを閉じてから実行される。名前は
/// `crate::app::commands` の表にあるものだけ（契約テストで突き合わせる）。
pub(crate) const HELP_KEYS: &[(&str, &str, Option<&str>)] = &[
    ("Alt+Enter / Alt+Backspace", "help.guide", None),
    ("Ctrl+P", "help.palette", Some("palette")),
    ("Ctrl+G", "help.jump", Some("jump")),
    ("Ctrl+S", "help.save", Some("save")),
    ("Alt+↑ / Alt+↓", "help.section", None),
    ("Alt+← / Alt+→", "help.scene", None),
    ("Ctrl+Alt+← / →", "help.file", None),
    ("Ctrl+Enter", "help.confirm_next", Some("confirm-next")),
    ("Ctrl+;", "help.new_section", Some("new-section")),
    ("Ctrl+1 / 2 / 3", "help.focus", None),
    ("Ctrl+\\", "help.reference", Some("reference")),
    ("Ctrl+Shift+\\", "help.swap", Some("swap-reference")),
    ("Ctrl+Alt+↑ / ↓", "help.context_ratio", None),
    ("Ctrl+Alt+Shift+← / →", "help.main_ratio", None),
    ("Ctrl+Plus / Minus / 0", "help.font", None),
    ("Ctrl+Alt+L", "help.language", Some("language")),
    ("Ctrl+Alt+T", "help.theme", Some("theme")),
    ("Alt+H", "help.inlay_hints", Some("inlay-hints")),
    ("F5", "help.reload", Some("reload")),
    ("Ctrl+F / Ctrl+H", "help.find", None),
    ("F3 / Shift+F3", "help.find_next", None),
    ("Ctrl+Shift+F", "help.search", Some("search")),
    ("Ctrl+Space", "help.complete", Some("complete")),
    ("Ctrl+Shift+M", "help.modes", Some("modes")),
    ("Ctrl+Shift+W", "help.term", Some("term")),
    (
        "Ctrl+Shift+; / Ctrl+Shift+Delete / Ctrl+:",
        "help.delete_section",
        Some("delete-section"),
    ),
    ("Ctrl+Tab", "help.recent", Some("recent-next")),
    ("Ctrl+O", "help.open", Some("open")),
    ("Ctrl+.", "help.quick_fix", Some("quick-fix")),
    ("Ctrl+T / Ctrl+Shift+T", "help.register", None),
    ("Ctrl+Shift+D", "help.diff", Some("diff")),
    ("Alt+1..9 / Alt+0", "help.workspace", None),
    ("Ctrl+B", "help.up", Some("up")),
    ("Ctrl+Shift+I", "help.capture", Some("capture")),
    ("Ctrl+Shift+E", "help.review", Some("review")),
    ("Ctrl+M", "help.material", Some("material-search")),
    ("Ctrl+Shift+N", "help.insert", Some("new-item")),
    ("Ctrl+Delete", "help.drop", Some("drop")),
    ("Ctrl+Shift+← / →", "help.tab", None),
    ("Ctrl+Shift+V", "help.scan", Some("scan")),
    ("F2", "help.rename", Some("rename")),
    ("Ctrl+Shift+[ / ]", "help.fold", None),
    ("Esc", "help.escape", None),
];

/// ヘルプ項目のコマンド名 → コマンド（コマンド表が単一の真実）。
pub(crate) fn help_command_of(name: &str) -> Option<crate::app::command::Command> {
    crate::app::commands::PALETTE_COMMANDS
        .iter()
        .chain(crate::app::commands::SHORTCUT_PALETTE_COMMANDS.iter())
        .find(|(found, _, _)| *found == name)
        .map(|(_, _, command)| *command)
}

#[cfg(test)]
mod help_keys_tests {
    use super::{help_command_of, HELP_KEYS};

    /// 案内項目とコマンド表の対応（名前がある項目は必ず表に実在する）。
    #[test]
    fn every_help_item_name_resolves_in_the_command_tables() {
        for (keys, _, name) in HELP_KEYS {
            if let Some(name) = name {
                assert!(
                    help_command_of(name).is_some(),
                    "{keys}: command name {name} is not in the command tables"
                );
            }
        }
    }
}
