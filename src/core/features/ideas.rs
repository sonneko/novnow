//! アイデアのワークスペース（§10.4 `idea`）の操作: 一覧・即時入力・状態の書き換え・昇格の転記。
//!
//! LSP には対応しない補助（`docs/history.md` B-130）。**文言は持たない**（§12.2）。どれも
//! 「文書の文字列 → 新しい文字列」の純粋関数で、書き込みは UI（`Workbench`）が行う。
//!
//! - ID は `ideas.md` の中の最大値 + 1（§5.7 MUST、再利用禁止）。破棄は `status: dropped` に
//!   するだけで行を消さない（§10.4）ので、普段の使い方で番号が戻ることはない。
//! - `status` / `created` / `promoted-to` は §5.0.6 の表に無いキー（A-15）なので、書かれたままの
//!   キーで探して書き換える。

use std::ops::Range;

use crate::core::features::Project;
use crate::core::id::{Digits, SceneId};
use crate::core::model::idea::IdeaStatus;
use crate::core::syntax::ideas_md;
use crate::core::text::lexer::FieldKey;

/// `ideas.md` のパス（§4.1）。
pub const PATH: &str = "ideas.md";

/// 一覧の 1 行（§10.4 の左）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeaRow {
    /// アイデアの順番（`ideas_md::Ideas::ideas` の index）。
    pub index: usize,
    /// 書かれたままの ID（`I0007`）。
    pub id: String,
    /// タイトル。
    pub title: String,
    /// `status:`（書かれていない・読めない値なら `inbox` とみなす。§5.7 の既定）。
    pub status: IdeaStatus,
    /// `tags:`（カンマ区切り）。
    pub tags: Vec<String>,
    /// エディタで編集する行（見出しから次の見出しの前まで。末尾の空行を含まない）。
    pub lines: Range<usize>,
}

/// `ideas.md` のアイデア（書かれた順）。
pub fn rows(project: &Project) -> Vec<IdeaRow> {
    let text = project.text(PATH);
    let parsed = ideas_md::parse(text);
    parsed
        .ideas
        .iter()
        .enumerate()
        .map(|(index, idea)| {
            let status = parsed
                .status(index)
                .and_then(|field| IdeaStatus::parse(first_word(field.value)))
                .unwrap_or_default();
            let tags = parsed
                .idea_fields(index, FieldKey::Tags)
                .iter()
                .flat_map(|field| field.value.split([',', '、']))
                .map(|tag| tag.trim().to_owned())
                .filter(|tag| !tag.is_empty())
                .collect();
            let mut end = idea.span.end.min(parsed.lines.len());
            while end > idea.heading + 1 && parsed.lines[end - 1].raw().trim().is_empty() {
                end -= 1;
            }
            IdeaRow {
                index,
                id: idea.id.to_owned(),
                title: idea.title.trim().to_owned(),
                status,
                tags,
                lines: idea.heading..end,
            }
        })
        .collect()
}

/// 値の最初の語（`inbox            # inbox | keep` の `# ` 行末コメントは ② なので、値に残っている）。
fn first_word(value: &str) -> &str {
    value.split_whitespace().next().unwrap_or("")
}

/// 次に採番する番号（最大値 + 1。§5.7）。
pub fn next_number(text: &str) -> u32 {
    ideas_md::parse(text)
        .ideas
        .iter()
        .filter_map(|idea| idea.number)
        .max()
        .unwrap_or(0)
        + 1
}

/// ID の表記（`I` + 4 桁以上）。
pub fn id_of(number: u32) -> String {
    format!(
        "{}{number:0width$}",
        ideas_md::ID_SIGIL,
        width = ideas_md::ID_DIGITS
    )
}

/// 即時入力（§10.4 `Ctrl+Shift+I`）: `title` を `status: inbox` のアイデアとして末尾に積んだ全体と、
/// 付けた ID。`created` は RFC 3339 の時刻。
pub fn capture(text: &str, title: &str, created: &str) -> (String, String) {
    let id = id_of(next_number(text));
    let mut out = text.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str(&format!(
        "# {id} {}\nstatus: inbox\ncreated: {created}\n",
        title.trim()
    ));
    (out, id)
}

/// `index` 番目のアイデアのフィールド `key`（書かれたままのキー）を `value` にした全体。
/// 無ければ、最後のフィールド行の後ろ（フィールドが無ければ見出しの直後）に足す。
pub fn set_field(text: &str, index: usize, key: &str, value: &str) -> String {
    let parsed = ideas_md::parse(text);
    let Some(idea) = parsed.ideas.get(index) else {
        return text.to_owned();
    };
    let found = idea.fields.iter().copied().find(|line| {
        parsed
            .field(*line)
            .is_some_and(|field| field.key_text == key)
    });
    let insert_after = idea.fields.last().copied().unwrap_or(idea.heading);
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let line = format!("{key}: {value}");
    match found {
        Some(at) => {
            if let Some(slot) = lines.get_mut(at) {
                *slot = line;
            }
        }
        None => lines.insert((insert_after + 1).min(lines.len()), line),
    }
    lines.join("\n")
}

/// 昇格の転記に使う中身（タイトルと本文）。
pub fn content(text: &str, index: usize) -> Option<(String, String)> {
    let parsed = ideas_md::parse(text);
    let idea = parsed.ideas.get(index)?;
    Some((
        idea.title.trim().to_owned(),
        parsed.body(index).trim().to_owned(),
    ))
}

// ============================================================ 昇格の転記先（§10.4）

/// 話プロット（B 形式）の場面 `scene` の最後に節を足した全体と、足した節の ID。場面が無ければ
/// 末尾に場面見出しを書く。`points` は節の要点の行。
pub fn append_plot_section(
    text: &str,
    scene: SceneId,
    points: &[String],
    digits: Digits,
    project: &Project,
) -> (String, SceneId) {
    let plot = crate::core::syntax::plot_episode::parse_with(text, &project.contents_options());
    let wanted = SceneId {
        section: 0,
        ..scene
    };
    let body: Vec<String> = points
        .iter()
        .flat_map(|point| point.split('\n'))
        .map(str::to_owned)
        .filter(|line| !line.trim().is_empty())
        .collect();
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let found = plot
        .scenes
        .iter()
        .find(|found| found.id.map(|id| SceneId { section: 0, ..id }) == Some(wanted));
    match found {
        Some(found) => {
            let next = found
                .sections
                .iter()
                .map(|section| section.number)
                .max()
                .unwrap_or(0)
                + 1;
            // 場面の最後の空でない行の後ろ。最後の節が `;` で閉じていなければ閉じてから足す
            let mut end = found.span.end.min(lines.len());
            while end > found.header + 1 && lines[end - 1].trim().is_empty() {
                end -= 1;
            }
            let last_is_break = found
                .sections
                .last()
                .is_some_and(|section| section.break_line.is_some_and(|at| at + 1 == end));
            let mut inserted = Vec::new();
            if !last_is_break && !found.sections.is_empty() {
                inserted.push(";".to_owned());
            }
            inserted.extend(body);
            inserted.push(";".to_owned());
            lines.splice(end..end, inserted);
            (
                lines.join("\n"),
                SceneId {
                    section: next,
                    ..wanted
                },
            )
        }
        None => {
            if lines.last().is_some_and(|line| line.is_empty()) {
                lines.pop();
            }
            if lines.last().is_some_and(|line| !line.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(format!(
                "@{}",
                crate::core::features::plots::short_id(wanted, digits)
            ));
            lines.extend(body);
            lines.push(";".to_owned());
            lines.push(String::new());
            (
                lines.join("\n"),
                SceneId {
                    section: 1,
                    ..wanted
                },
            )
        }
    }
}

/// `words.md` の末尾に用語を足した全体（§5.6。`definition` は必須なので、本文が無ければタイトル）。
pub fn append_term(text: &str, term: &str, definition: &str) -> String {
    let mut out = text.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
    let definition = if definition.trim().is_empty() {
        term
    } else {
        definition
    };
    out.push_str(&format!(
        "# {}\ndefinition: {}\n",
        term.trim(),
        definition.trim().replace('\n', " ")
    ));
    out
}

/// `characters.md` のキャラ `index`（H1 の順）の最後にメモの行を足した全体（§5.5.1「それ以外の
/// 行は直前のフィールド／セクションに属する自由記述」）。キャラが無ければそのまま。
pub fn append_character_memo(text: &str, index: usize, memo: &str) -> String {
    let parsed = crate::core::syntax::characters_md::parse(text);
    let Some(character) = parsed.characters.get(index) else {
        return text.to_owned();
    };
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let mut end = character.span.end.min(lines.len());
    while end > character.heading + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let memo: Vec<String> = memo
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    lines.splice(end..end, memo);
    lines.join("\n")
}
