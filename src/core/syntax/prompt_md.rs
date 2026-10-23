//! `prompts/*.md`（§5.9.1）と `prompts/custom/*.md`（§5.9.2）のパーサ。
//!
//! ```md
//! # writer
//! model-hint: balanced
//! max-chars: 12000
//! temperature: 0.7
//!
//! ## system
//! あなたは novnow プロジェクトの本文執筆担当です。…
//!
//! ## task
//! 次の節の本文だけを書いてください。
//! 対象: {{scene_id}}
//!
//! ## output
//! - 出力は本文のみ。見出し・解説・前置きを書かない。
//! - 役割語は {{role_language}} に従う。
//! ```
//!
//! §5.6 と同じ MD 方言（行の分類は [`super::characters_md`]）を使うが、木は違う:
//! H1 = **役割名**（1 ファイル 1 役割）、`## ` = §5.9.1 の 5 種のセクション
//! （`system` / `task` / `output` / `examples` / `refuse`）、セクションの中身は
//! **LLM にそのまま渡す雛形**なので [`Prompt::body_of`] で繋いで返す。
//!
//! # 変数（§15.5.3）
//!
//! `{{variable}}` は**閉じた集合**で、未知の変数は **E031**（保存は可能、実行は不可）。
//! 変数はセクションの中身だけでなくフィールドの値や見出しにも書けるので、
//! **行コメント（`// …`）と空行を除く全行**を走査して [`Prompt::variables`] に集める。
//! `# ` の行コメントは ②（§5.0.8）なので、① では変数の走査対象に含まれる。
//!
//! # カスタムロール（§5.9.2）
//!
//! `prompts/custom/*.md` かどうかは**ファイルの内容からは分からない**（パスの話）ので、
//! [`Options::custom`] で呼び出し側が指定する。検査は次の 2 つだけ:
//!
//! - **E032**: 役割名が `/^[a-z][a-z0-9-]{0,31}$/` に合わない。
//! - **E033**: 役割名が既定 7 役割（[`BUILTIN_ROLES`]）と衝突する。
//!
//! # 診断（M2 で出すもの）
//!
//! | コード | 条件 | 位置 |
//! |---|---|---|
//! | **E032** | カスタムロール名が不正（§5.9.2） | H1 の名前 |
//! | **E033** | カスタムロール名が既定役割と衝突（§5.9.2） | H1 の名前 |
//! | **W011** | §5.0.6 の表に無いフィールドキー（`model-hint` `max-chars` `temperature` `label` は表に無い。`docs/history.md` A-15） | キー |
//! | **E031** | 未知の変数（§15.5.3） | 変数名 |
//!
//! 並びは**ロールの E032 / E033** → **行順の W011** → **行順の E031**。
//!
//! # M2 で出さない診断
//!
//! - **E034**（カスタムロールが 16 件を超えた、§5.9.2）: `prompts/custom/` の**件数**が
//!   要るので 1 ファイルを見るパーサでは出せない（M10）。
//! - **I056**（未登録のカスタムロール、§5.9.2）: `project.json` の `agents.custom_roles` と
//!   突き合わせる検査（M10）。
//! - 5 種に無い `## ` セクション: §7 に対応コードが無いので診断しない（[`PromptSectionKey::Other`]
//!   として保持し、[`Prompt::body_of`] で中身も読める）。
//! - §5.0.8 の `# ` 行コメントは ② なので、① では値や本文の一部としてそのまま保持される
//!   （B 形式の B-36 と同じ）。

use std::ops::Range;

use serde::Serialize;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::text::lexer::FieldKey;

use super::characters_md;
use super::contents;
use super::plot_outline::{self, Field};

// MD 方言の行は `characters_md` のものが正（§5.6「同一の MD 方言」）。このモジュールだけで
// 木を辿れるよう、同じ名前で公開し直す（`docs/history.md` B-40 の流儀）。
pub use super::characters_md::{Bullet, Heading, Line, Pattern, PatternLine};

/// §5.9.1 のセクション（固定 5 種）。
pub const SECTION_SYSTEM: &str = "system";
/// §5.9.1 のセクション。
pub const SECTION_TASK: &str = "task";
/// §5.9.1 のセクション。
pub const SECTION_OUTPUT: &str = "output";
/// §5.9.1 のセクション。
pub const SECTION_EXAMPLES: &str = "examples";
/// §5.9.1 のセクション。
pub const SECTION_REFUSE: &str = "refuse";

/// 変数の開き（§15.5.3 `{{variable}}`）。
pub const VAR_OPEN: &str = "{{";
/// 変数の閉じ（§15.5.3）。
pub const VAR_CLOSE: &str = "}}";

/// §5.9.2 の役割名の形 `/^[a-z][a-z0-9-]{0,31}$/` の最大長。
pub const ROLE_NAME_MAX: usize = 32;

/// 既定の 7 役割（§15.2「6 + 統合」）。
///
/// `docs/plans.md` §4.4 の縫い代 10「ロール一覧を配列で持つ（enum の直書きにしない）」の実体。
/// カスタムロール（§5.9.2）はこれと**同名であってはならない**（E033）。
pub const BUILTIN_ROLES: &[&str] = &[
    "writer",
    "plotter",
    "ideator",
    "researcher",
    "stylist",
    "logician",
    "integrator",
];

/// §15.5.3 の変数（**閉じた集合・固定**）。ここに無い変数は **E031**。
pub const VARIABLES: &[&str] = &[
    "scene_id",
    "project_title",
    "project_style",
    "outline_all",
    "outline_chapter",
    "plot_scene",
    "plot_section",
    "check",
    "text_current",
    "text_before",
    "text_after",
    "cast",
    "role_language",
    "lexicon",
    "materials",
    "target_chars",
    "history",
    "delegation",
    "user_comment",
    "examples",
];

/// 解析の設定。既定は `prompts/<role>.md`（既定役割）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Options {
    /// `prompts/custom/*.md`（§5.9.2）か。
    ///
    /// `true` のときだけ役割名を検査する（**E032** / **E033**）。既定の
    /// `prompts/<role>.md` は §5.9.1 の例どおり役割名がファイル名で、
    /// 名前そのものの検査は §5.9.2 の規則だから。
    pub custom: bool,
}

/// プロンプト 1 ファイル。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Prompt<'a> {
    /// 全行（書かれたまま）。[`Prompt::to_text`] はこれを LF で連結する。
    pub lines: Vec<Line<'a>>,
    /// 役割（H1）。H1 が無ければ `None`。2 つ目以降の H1 は木に載せない（行は残る）。
    pub role: Option<Role<'a>>,
    /// 最初の `## ` より前のフィールド行の index（出現順）。§5.9.1 の `model-hint` 等。
    pub fields: Vec<usize>,
    /// セクション（`## ` の順）。
    pub sections: Vec<PromptSection<'a>>,
    /// `{{variable}}`（行順）。未知の変数には **E031** が出ている。
    pub variables: Vec<Variable<'a>>,
    /// 診断（§7）。位置は `text` の中の byte range。
    pub diagnostics: Vec<Diagnostic>,
}

/// プロンプトのテキストをパースする（`prompts/<role>.md`。設定は [`Options::default`]）。
///
/// `text` は **LF 改行**であること（§5.0.4）。
pub fn parse(text: &str) -> Prompt<'_> {
    parse_with(text, &Options::default())
}

/// カスタムロールかどうかを指定してパースする（§5.9.2）。
pub fn parse_with<'a>(text: &'a str, options: &Options) -> Prompt<'a> {
    let mut lines: Vec<Line<'a>> = text.split('\n').map(characters_md::parse_line).collect();
    characters_md::attach_continuations(&mut lines);
    let mut diagnostics = Vec::new();
    let offsets = characters_md::line_offsets(&lines);
    let role = collect_role(&lines, options, &offsets, &mut diagnostics);
    let (fields, sections) = collect(&lines, &mut diagnostics, &offsets);
    let variables = collect_variables(&lines, &offsets, &mut diagnostics);
    Prompt {
        lines,
        role,
        fields,
        sections,
        variables,
        diagnostics,
    }
}

impl<'a> Prompt<'a> {
    /// 入力と**バイト単位で一致する**テキストに戻す（`docs/history.md` の完了条件 2）。
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (index, line) in self.lines.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(line.raw());
        }
        out
    }

    /// `key` のセクション（§5.9.1 の 5 種）。複数あれば最初。
    pub fn section(&self, key: PromptSectionKey) -> Option<&PromptSection<'a>> {
        self.sections.iter().find(|found| found.key == key)
    }

    /// `lines[index]` を含むセクション。H1 より前の行は `None`。
    pub fn section_of(&self, index: usize) -> Option<&PromptSection<'a>> {
        self.sections
            .iter()
            .find(|found| found.span.contains(&index))
    }

    /// `lines[index]` が見出し行ならその中身。
    pub fn heading(&self, index: usize) -> Option<&Heading<'a>> {
        match self.lines.get(index) {
            Some(Line::Heading(found)) => Some(found),
            _ => None,
        }
    }

    /// `lines[index]` がフィールド行ならその中身。
    pub fn field(&self, index: usize) -> Option<&Field<'a>> {
        match self.lines.get(index) {
            Some(Line::Field(found)) => Some(found),
            _ => None,
        }
    }

    /// セクションの中身（見出し行を除いた行を LF で連結したもの）。
    ///
    /// §15.5.3 の変数を展開するのは M10（`aicontext/packet.rs`）で、ここは**書かれたまま**を
    /// 返す（正規化もしない。§5.9.1「以後の更新は上書きしない」＝ユーザーの編集が正）。
    pub fn body_of(&self, index: usize) -> String {
        let Some(found) = self.sections.get(index) else {
            return String::new();
        };
        let parts: Vec<&str> = (found.span.start + 1..found.span.end)
            .map(|line| self.lines[line].raw())
            .collect();
        parts.join("\n")
    }

    /// §5.9.1 の 5 種のセクションが**書いた順に**全部あるか。
    ///
    /// 欠落や未知のセクションに対応する診断コードは §7 に無いので、これは検査ではなく
    /// UI（§15.5 のプロンプト編集）が「どれを出すか」を決めるための情報。
    pub fn known_sections(&self) -> Vec<PromptSectionKey> {
        self.sections
            .iter()
            .map(|found| found.key)
            .filter(|key| *key != PromptSectionKey::Other)
            .collect()
    }

    /// 表に無いキー（[`FieldKey::Unknown`]）を**書かれたままのキー**で引く。
    ///
    /// §5.9.1 の `model-hint` / `max-chars` / `temperature` と §5.9.2 の `label` は
    /// §5.0.6 の表に無いので **W011** になるが、値はそのまま保持される（縫い代 2）。
    pub fn meta_field(&self, key_text: &str) -> Option<&Field<'a>> {
        characters_md::unknown_field(&self.lines, &self.fields, key_text)
    }

    /// `key` のフィールド（H1 より前のもの。出現順）。§5.9.2 の `context` は表にあるキー。
    pub fn meta_fields(&self, key: FieldKey) -> Vec<&Field<'a>> {
        pick(&self.lines, &self.fields, key)
    }

    /// 未知の変数（**E031** が出ているもの）。
    pub fn unknown_variables(&self) -> Vec<&Variable<'a>> {
        self.variables.iter().filter(|found| !found.known).collect()
    }
}

/// 役割（§5.9.1 の H1 / §5.9.2 のカスタムロール名）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Role<'a> {
    /// H1 の行 index。
    pub heading: usize,
    /// 役割名（見出しの 1 トークン目。§5.9.2「ロール名 = ファイル名（拡張子なし）」）。
    pub name: &'a str,
    /// `name` の行内 byte 位置。
    pub name_at: usize,
    /// §5.9.2 の形（`/^[a-z][a-z0-9-]{0,31}$/`）に合うか。
    ///
    /// [`Options::custom`] が `false` のときは検査しないが、形は常に読んでおく。
    pub valid: bool,
    /// 既定 7 役割（[`BUILTIN_ROLES`]）のどれか。
    pub builtin: bool,
}

/// `## ` セクション（§5.9.1 の 5 種）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PromptSection<'a> {
    /// H2 の行 index。
    pub heading: usize,
    /// 引き当てたセクション。5 種に無ければ [`PromptSectionKey::Other`]。
    pub key: PromptSectionKey,
    /// **書かれたままの**セクション名。
    pub key_text: &'a str,
    /// 行範囲。H2 を含み、次の H2 を含まない。
    pub span: Range<usize>,
    /// セクション内のフィールド行の index（出現順）。
    ///
    /// 雛形の中に `note: …` のような行を書けばフィールド行になる（§5.0.5 は形式を問わない）。
    /// 中身は [`Prompt::body_of`] でそのまま返るので、これは位置の情報。
    pub fields: Vec<usize>,
    /// セクション内の箇条書き行の index（出現順）。§5.9.1 の `## output` は箇条書き。
    pub bullets: Vec<usize>,
    /// セクション内の自由記述の index（出現順）。
    pub free: Vec<usize>,
}

/// §5.9.1 のセクション（固定 5 種）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PromptSectionKey {
    /// `system`。
    System,
    /// `task`。
    Task,
    /// `output`。
    Output,
    /// `examples`。
    Examples,
    /// `refuse`。
    Refuse,
    /// 5 種のどれでもない。**行はそのまま保持される**ので情報は失われない。
    Other,
}

/// `{{variable}}` の 1 件（§15.5.3）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Variable<'a> {
    /// 行の index。
    pub line: usize,
    /// 変数名の行内 byte 位置（`{{` の直後）。
    pub at: usize,
    /// 変数名の行内 byte 範囲（`{{` と `}}` を除く）。
    pub range: Range<usize>,
    /// 変数名（`{{` と `}}` を除く）。
    pub name: &'a str,
    /// §15.5.3 の閉じた集合（[`VARIABLES`]）にあるか。`false` なら **E031**。
    pub known: bool,
}

// ============================================================ 木と診断

/// H1 から役割を読み、**E032** / **E033** を出す（§5.9.2）。
fn collect_role<'a>(
    lines: &[Line<'a>],
    options: &Options,
    offsets: &[usize],
    out: &mut Vec<Diagnostic>,
) -> Option<Role<'a>> {
    let (index, line) = lines
        .iter()
        .enumerate()
        .find(|(_, line)| matches!(line, Line::Heading(found) if found.level == 1))?;
    let Line::Heading(found) = line else {
        return None;
    };
    let (name, _) = found.split_label();
    let role = Role {
        heading: index,
        name,
        name_at: found.text_at,
        valid: is_role_name(name),
        builtin: BUILTIN_ROLES.contains(&name),
    };
    if options.custom {
        let start = offsets[index] + role.name_at;
        let at = start..start + name.len();
        // §5.9.2「ロール名は `/^[a-z][a-z0-9-]{0,31}$/`。違反は E032」
        if !role.valid {
            out.push(Diagnostic::new(
                DiagnosticCode::E032,
                Some(at.clone()),
                vec![Arg::text(name)],
            ));
        }
        // §5.9.2「既定 7 役割と同名は E033（上書きは `prompts/<role>.md` 側で行う）」
        if role.builtin {
            out.push(Diagnostic::new(
                DiagnosticCode::E033,
                Some(at),
                vec![Arg::text(name)],
            ));
        }
    }
    Some(role)
}

/// §5.9.2 の役割名（`/^[a-z][a-z0-9-]{0,31}$/`）か。
pub fn is_role_name(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() || text.len() > ROLE_NAME_MAX {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// §5.9.1 のセクション名を引き当てる。
pub(crate) fn section_key_of(text: &str) -> PromptSectionKey {
    match text {
        SECTION_SYSTEM => PromptSectionKey::System,
        SECTION_TASK => PromptSectionKey::Task,
        SECTION_OUTPUT => PromptSectionKey::Output,
        SECTION_EXAMPLES => PromptSectionKey::Examples,
        SECTION_REFUSE => PromptSectionKey::Refuse,
        _ => PromptSectionKey::Other,
    }
}

/// 全行を走査して `{{variable}}` を集め、未知の変数に **E031** を出す（§15.5.3）。
///
/// 行コメント（`// …`）と空行は走査しない（LLM に渡らない部分だから）。
/// `# ` の行コメントは ②（§5.0.8）なので ① では走査対象に残る。
fn collect_variables<'a>(
    lines: &[Line<'a>],
    offsets: &[usize],
    out: &mut Vec<Diagnostic>,
) -> Vec<Variable<'a>> {
    let mut variables = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(text) = scannable(line) else {
            continue;
        };
        let base = offsets[index];
        for found in scan_variables(text, index) {
            if !found.known {
                // [`Variable::at`] は行内相対なので、行頭の位置を足して絶対位置にする
                let start = base + found.at;
                out.push(Diagnostic::new(
                    DiagnosticCode::E031,
                    Some(start..start + found.name.len()),
                    vec![Arg::text(found.name)],
                ));
            }
            variables.push(found);
        }
    }
    variables
}

/// 走査対象のテキスト（行コメントより前）。空行と行コメントは `None`。
fn scannable<'a>(line: &Line<'a>) -> Option<&'a str> {
    if matches!(line, Line::Comment(_) | Line::Blank(_)) {
        return None;
    }
    let raw = line.raw();
    match contents::find_comment(raw) {
        Some(found) => Some(&raw[..found.range.start]),
        None => Some(raw),
    }
}

/// `text` の中の `{{variable}}` を全部拾う。位置は**行内相対**（[`Variable::at`]）。
fn scan_variables<'a>(text: &'a str, line: usize) -> Vec<Variable<'a>> {
    let mut out = Vec::new();
    let mut rest = text;
    let mut consumed = 0;
    while let Some(open) = rest.find(VAR_OPEN) {
        let after = &rest[open + VAR_OPEN.len()..];
        let Some(close) = after.find(VAR_CLOSE) else {
            // 閉じない `{{` は変数ではない（そのまま雛形に残る。診断は出さない）
            break;
        };
        let name = &after[..close];
        let at = consumed + open + VAR_OPEN.len();
        out.push(Variable {
            line,
            at,
            range: at..at + name.len(),
            name,
            known: VARIABLES.contains(&name),
        });
        let step = open + VAR_OPEN.len() + close + VAR_CLOSE.len();
        rest = &rest[step..];
        consumed += step;
    }
    out
}

/// 解析中のセクション。
struct OpenSection<'a> {
    /// H2 の行 index。
    heading: usize,
    /// セクション。
    key: PromptSectionKey,
    /// 書かれたままのセクション名。
    key_text: &'a str,
    /// フィールド行の index。
    fields: Vec<usize>,
    /// 箇条書き行の index。
    bullets: Vec<usize>,
    /// 自由記述の index。
    free: Vec<usize>,
}

/// 行の並びからセクションを作り、行順の診断（W011）を出す。
///
/// H1 より前のフィールド行は [`Prompt::fields`] に入る（§5.9.1 の `model-hint` 等）。
/// H3 は §5.9.1 に無いのでセクションを閉じない（行は残る）。
fn collect<'a>(
    lines: &[Line<'a>],
    out: &mut Vec<Diagnostic>,
    offsets: &[usize],
) -> (Vec<usize>, Vec<PromptSection<'a>>) {
    let mut fields = Vec::new();
    let mut open: Vec<OpenSection<'a>> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if let Line::Field(found) = line {
            // §5.0.6「未知のキーは W011 とし、値は改変せず保持する」（行順）
            plot_outline::warn_unknown_key(found, offsets[index], out);
        }
        match line {
            Line::Heading(found) if found.level == 2 => open.push(OpenSection {
                heading: index,
                key: section_key_of(found.text),
                key_text: found.text,
                fields: Vec::new(),
                bullets: Vec::new(),
                free: Vec::new(),
            }),
            Line::Heading(_) => {}
            Line::Field(_) => match open.last_mut() {
                Some(found) => found.fields.push(index),
                None => fields.push(index),
            },
            Line::Bullet(_) => {
                if let Some(found) = open.last_mut() {
                    found.bullets.push(index);
                }
            }
            Line::Text(_) => {
                if let Some(found) = open.last_mut() {
                    found.free.push(index);
                }
            }
            Line::Pattern(_) | Line::Continuation(_) | Line::Comment(_) | Line::Blank(_) => {}
        }
    }
    let mut sections = Vec::with_capacity(open.len());
    for found in open {
        // セクションの終わりは次の `## ` / `# `（無ければファイルの終わり）
        let end = sections_end(lines, found.heading).unwrap_or(lines.len());
        sections.push(PromptSection {
            heading: found.heading,
            key: found.key,
            key_text: found.key_text,
            span: found.heading..end,
            fields: found.fields,
            bullets: found.bullets,
            free: found.free,
        });
    }
    (fields, sections)
}

/// `from` の次の H2 / H1 の行 index（無ければ `None`）。
fn sections_end(lines: &[Line<'_>], from: usize) -> Option<usize> {
    (from + 1..lines.len()).find(|&index| match &lines[index] {
        Line::Heading(found) => found.level <= 2,
        _ => false,
    })
}

/// `indices` の中から `key` のフィールド行を集める（A / B 形式の `pick` と同じ）。
fn pick<'s, 'a>(lines: &'s [Line<'a>], indices: &[usize], key: FieldKey) -> Vec<&'s Field<'a>> {
    indices
        .iter()
        .filter_map(|&index| match lines.get(index) {
            Some(Line::Field(found)) if found.key == key => Some(found),
            _ => None,
        })
        .collect()
}
