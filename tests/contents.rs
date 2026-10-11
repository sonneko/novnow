//! `core::syntax::contents`（C 形式、§5.2）のテスト。
//!
//! `PLANS.md` §5.3 完了条件の 1（仕様書中のコード例 §5.2.3 が期待どおりにパースできる）・
//! 2（round-trip でバイト列が一致する）・3（壊れた入力でパニックせず診断を返す）に対応する。
//!
//! 比較は [`lines_tag`] / [`sections_tag`] / [`diags_tag`] が作る短い文字列で行う
//! （`tests/lexer.rs` と同じ流儀。`assert_eq!` の引数に構造体リテラルを直接書くと rustfmt が
//! 必ず折り返すので、期待値を 1 行に収めるためでもある）。

use novnow::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use novnow::core::id::SceneId;
use novnow::core::syntax::contents::{
    self, BreakLine, Comment, CommentLine, Contents, Line, Options, Ruby, SceneLine, Section,
    Speaker, TextLine,
};

/// §5.2.3 の例。**仕様のコード例をそのまま**使う（§5.3 完了条件 1）。
const SPEC_EXAMPLE: &str = "@01-04-02 図書室の攻防\n\
「やっぱり'アルス、ここにいたのね」@リリア:A\n\
\n\
呆れたような声とともに、'リリアが隣の椅子を引き寄せた。\n\
;\n\
「旧校舎の地下遺跡に眠っているっていう'『星屑の時計』の設計図だ」@アルス\n\
\n\
'リリアは溜め息をついたが、図面の精巧な書き込みに目を奪われた。\n\
;\n\
「……付き合ってあげるわよ」@リリア:B    // ここでモードが切り替わる\n";

/// §5.2.3 の例の最終行（話者注釈＋行末コメント＋その前後の空白）。
const ANNOTATED: &str = "「……付き合ってあげるわよ」@リリア:B    // ここでモードが切り替わる";

/// 章 0 を拒否する設定（§5.1.2 `structure.allow_chapter_zero = false`）。
fn strict() -> Options {
    Options {
        allow_chapter_zero: false,
    }
}

// ============================================================ 描画ヘルパ

fn id_text(id: Option<SceneId>) -> String {
    match id {
        Some(found) => format!(
            "{}-{}-{}-{}",
            found.chapter, found.episode, found.scene, found.section
        ),
        None => "-".to_owned(),
    }
}

fn range_text(range: &std::ops::Range<usize>) -> String {
    format!("{}..{}", range.start, range.end)
}

fn comment_text(comment: Option<&Comment<'_>>) -> String {
    match comment {
        Some(found) => found.text.to_owned(),
        None => String::new(),
    }
}

fn scene_tag(line: &SceneLine<'_>) -> String {
    format!(
        "scene[{}|d{}|id{}|t{}|c{}]",
        line.id_text,
        line.depth,
        id_text(line.id),
        line.title,
        comment_text(line.comment.as_ref())
    )
}

fn break_tag(line: &BreakLine<'_>) -> String {
    format!(
        "break[@{}|c{}]",
        line.sigil_at,
        comment_text(line.comment.as_ref())
    )
}

fn comment_tag(line: &CommentLine<'_>) -> String {
    format!(
        "comment[{}@{}]",
        line.comment.text,
        range_text(&line.comment.range)
    )
}

fn speaker_tag(found: &Speaker<'_>) -> String {
    let modes = found.modes.join("+");
    format!("sp={}:{}@{}", found.name, modes, range_text(&found.range))
}

fn ruby_tag(found: &Ruby<'_>) -> String {
    let explicit = if found.explicit { "*" } else { "" };
    format!(
        "ru={}={}@{}{}",
        found.base,
        found.reading,
        range_text(&found.range),
        explicit
    )
}

fn text_tag(line: &TextLine<'_>) -> String {
    let mut out = format!("text[{}", line.body);
    if let Some(found) = &line.speaker {
        out.push_str(&format!("|{}", speaker_tag(found)));
    }
    if !line.rubies.is_empty() {
        let parts: Vec<String> = line.rubies.iter().map(ruby_tag).collect();
        out.push_str(&format!("|{}", parts.join(",")));
    }
    if let Some(found) = &line.inline_comment {
        out.push_str(&format!("|ic{}", range_text(found)));
    }
    if let Some(found) = &line.comment {
        out.push_str(&format!("|c{}@{}", found.text, range_text(&found.range)));
    }
    out.push(']');
    out
}

fn line_tag(line: &Line<'_>) -> String {
    match line {
        Line::Scene(found) => scene_tag(found),
        Line::Break(found) => break_tag(found),
        Line::Comment(found) => comment_tag(found),
        Line::Blank(found) => format!("blank[{found}]"),
        Line::Text(found) => text_tag(found),
    }
}

fn section_tag(found: &Section) -> String {
    let header = match found.scene_header {
        Some(index) => index.to_string(),
        None => "-".to_owned(),
    };
    format!(
        "D{}|s{}|h{}|{}|c{}",
        found.number,
        id_text(found.scene),
        header,
        range_text(&found.span),
        range_text(&found.content)
    )
}

fn arg_tag(found: &Arg) -> String {
    match found {
        Arg::Text(value) => value.clone(),
        Arg::Number(value) => value.to_string(),
    }
}

fn diag_tag(found: &Diagnostic) -> String {
    let range = match &found.range {
        Some(value) => range_text(value),
        None => "-".to_owned(),
    };
    if found.args.is_empty() {
        return format!("{}@{}", found.code.as_str(), range);
    }
    let args: Vec<String> = found.args.iter().map(arg_tag).collect();
    format!("{}@{}({})", found.code.as_str(), range, args.join(","))
}

fn join(parts: Vec<String>) -> String {
    parts.join(" / ")
}

/// 見出し 1 行の `-D` 明示（§5.2.2）。見出しでなければパニックする（テストの誤り）。
fn section_start_of(text: &str) -> Option<u16> {
    let parsed = contents::parse(text);
    let Line::Scene(found) = &parsed.lines[0] else {
        panic!("見出しのはず: {text}");
    };
    found.section_start()
}

/// `text` の全行のタグ。
fn lines_tag(text: &str) -> String {
    let parsed: Contents<'_> = contents::parse(text);
    join(parsed.lines.iter().map(line_tag).collect())
}

/// `text` の 1 行目のタグ（1 行だけの入力で使う）。
fn line_tag_of(text: &str) -> String {
    let parsed = contents::parse(text);
    assert_eq!(parsed.lines.len(), 1, "1 行だけ渡すこと: {text}");
    line_tag(&parsed.lines[0])
}

/// `text` の節のタグ。
fn sections_tag(text: &str) -> String {
    let parsed = contents::parse(text);
    join(parsed.sections.iter().map(section_tag).collect())
}

/// `text` の診断のタグ。
fn diags_tag(text: &str) -> String {
    let parsed = contents::parse(text);
    join(parsed.diagnostics.iter().map(diag_tag).collect())
}

/// 設定を渡したときの診断のタグ。
fn diags_with(text: &str, options: &Options) -> String {
    let parsed = contents::parse_with(text, options);
    join(parsed.diagnostics.iter().map(diag_tag).collect())
}

/// `lines[index]` が属する節の番号（無ければ `-`）。
fn owner_tag(text: &str, index: usize) -> String {
    let parsed = contents::parse(text);
    match parsed.section_of(index) {
        Some(found) => found.number.to_string(),
        None => "-".to_owned(),
    }
}

/// 診断コードだけの列（順を確認するのに使う）。
fn codes(found: &[Diagnostic]) -> Vec<DiagnosticCode> {
    found.iter().map(|diag| diag.code).collect()
}

// ============================================================ §5.2.1 行の種別

#[test]
fn a_scene_header_carries_id_title_and_comment() {
    assert_eq!(
        line_tag_of("@01-04-02 図書室の攻防"),
        "scene[01-04-02|d3|id1-4-2-0|t図書室の攻防|c]"
    );
    assert_eq!(
        line_tag_of("@01-04-02 図書室の攻防 // メモ"),
        "scene[01-04-02|d3|id1-4-2-0|t図書室の攻防|c メモ]"
    );
    // タイトルは前後の空白を落とす（§5.2.1 `[ ws , title_text ]`）
    assert_eq!(
        line_tag_of("@01-04-02   図書室   "),
        "scene[01-04-02|d3|id1-4-2-0|t図書室|c]"
    );
    // タイトルは無くてもよい
    assert_eq!(line_tag_of("@01-04-02"), "scene[01-04-02|d3|id1-4-2-0|t|c]");
    // 行頭の空白 1 文字は許す（§5.2.1 `ws?`）
    assert_eq!(
        line_tag_of(" @01-04-02 x"),
        "scene[01-04-02|d3|id1-4-2-0|tx|c]"
    );
}

#[test]
fn a_four_level_header_fixes_the_first_section_number() {
    // §5.2.2「見出しに `D` が明示されている場合、その見出し直後の節番号を `D` から開始する」
    assert_eq!(
        line_tag_of("@01-04-02-03 差し込み"),
        "scene[01-04-02-03|d4|id1-4-2-3|t差し込み|c]"
    );
    assert_eq!(section_start_of("@01-04-02-03 x"), Some(3));
    assert_eq!(section_start_of("@01-04-02 x"), None);
    assert_eq!(section_start_of("@01-04 x"), None);
    assert_eq!(section_start_of("@01-04-02-00 x"), Some(0));
}

#[test]
fn a_section_break_takes_a_trailing_comment() {
    assert_eq!(line_tag_of(";"), "break[@0|c]");
    assert_eq!(line_tag_of(" ;"), "break[@1|c]");
    assert_eq!(line_tag_of("; // メモ"), "break[@0|c メモ]");
    // `;` の後ろに本文があれば節区切りではない（§5.0.2）
    assert_eq!(line_tag_of("; 注"), "text[; 注]");
}

#[test]
fn a_comment_line_keeps_the_text_after_the_sigil() {
    assert_eq!(line_tag_of("// メモ"), "comment[ メモ@0..9]");
    assert_eq!(line_tag_of(" //メモ"), "comment[メモ@1..9]");
    // `://` は行コメントにしない（§5.0.2）
    assert_eq!(
        line_tag_of("see https://example.com"),
        "text[see https://example.com]"
    );
}

#[test]
fn a_whitespace_only_line_is_blank() {
    assert_eq!(line_tag_of(""), "blank[]");
    assert_eq!(line_tag_of("   "), "blank[   ]");
    assert_eq!(line_tag_of("　"), "blank[　]");
    // 2 文字以上のインデントでも、空白だけなら継続行ではなく空行（§5.2.1 `blank`）
    assert_eq!(line_tag_of("\t\t"), "blank[\t\t]");
}

#[test]
fn indented_prose_is_body_text_not_a_continuation() {
    // 一字下げ・段落字下げは正当な本文（`PLANS.md` §9.2 B-31）
    assert_eq!(line_tag_of("　そして、"), "text[　そして、]");
    assert_eq!(line_tag_of("    続きの行"), "text[    続きの行]");
}

#[test]
fn unknown_syntax_stays_in_the_body() {
    // ① で解析しない構文は本文のまま（`PLANS.md` §5.3 作業項目 1・本書 §1.2 の 10/23/15）
    assert_eq!(
        line_tag_of("世界樹の樹皮は、剥がすと七日で再生するという。^M0137"),
        "text[世界樹の樹皮は、剥がすと七日で再生するという。^M0137]"
    );
    assert_eq!(
        line_tag_of("! 見開き。リリアが時計塔の階段を見上げている。"),
        "text[! 見開き。リリアが時計塔の階段を見上げている。]"
    );
    assert_eq!(
        line_tag_of("# これはコメントではない"),
        "text[# これはコメントではない]"
    );
    // C 形式にフィールド行は無い（§5.0.5 の表）
    assert_eq!(line_tag_of("summary: 一行"), "text[summary: 一行]");
    // 行内コメントは位置だけを持ち、文字列は本文に残る（除去は §17.1、M9）
    assert_eq!(
        line_tag_of("本文 /* メモ */ 続き"),
        "text[本文 /* メモ */ 続き|ic7..19]"
    );
}

// ============================================================ §5.2.1 話者注釈

#[test]
fn a_speaker_annotation_is_taken_from_the_line_end() {
    assert_eq!(
        line_tag_of("「やっぱり'アルス、ここにいたのね」@リリア:A"),
        "text[「やっぱり'アルス、ここにいたのね」|sp=リリア:A@52..64]"
    );
    assert_eq!(line_tag_of("本文@リリア"), "text[本文|sp=リリア:@6..16]");
    assert_eq!(
        line_tag_of("本文@リリア:B+D"),
        "text[本文|sp=リリア:B+D@6..20]"
    );
}

#[test]
fn the_spaces_before_and_after_an_annotation_join_it() {
    // §17.1-5「話者注釈を**直前の空白ごと**除去」。後ろの空白も注釈に含める
    assert_eq!(
        line_tag_of(ANNOTATED),
        "text[「……付き合ってあげるわよ」|sp=リリア:B@42..58|c ここでモードが切り替わる@58..97]"
    );
}

#[test]
fn only_the_last_at_sign_can_be_an_annotation() {
    // §6.2.5「行末の `@名前`」。それより前の `@` は W002
    assert_eq!(
        line_tag_of("本文 @あ @リリア:B"),
        "text[本文 @あ|sp=リリア:B@11..24]"
    );
    assert_eq!(diags_tag("本文 @あ @リリア:B"), "W002@7..8");
}

#[test]
fn a_bare_at_sign_is_w002() {
    assert_eq!(diags_tag("本文中の @ は裸"), "W002@13..14");
    // エスケープすれば診断は出ない（§5.0.3、quickfix `\@`）
    assert_eq!(diags_tag("本文中の \\@ は素性"), "");
    // 行内コメントと行末コメントの中は本文ではない
    assert_eq!(diags_tag("本文 /* @x */ 続き"), "");
    assert_eq!(diags_tag("本文 // @x"), "");
}

#[test]
fn shapes_that_are_not_annotations_stay_in_the_body() {
    // `@` の後ろに空白 → キャラ名ではない
    assert_eq!(line_tag_of("本文 @ リリア"), "text[本文 @ リリア]");
    assert_eq!(diags_tag("本文 @ リリア"), "W002@7..8");
    // 名前に空白を含む
    assert_eq!(line_tag_of("本文@リリア A"), "text[本文@リリア A]");
    // `:` の後ろが `mode_id` ではない（§5.5 `/[A-Za-z][A-Za-z0-9_]{0,15}/`）
    assert_eq!(line_tag_of("本文@リリア:担任"), "text[本文@リリア:担任]");
    assert_eq!(line_tag_of("本文@リリア:1"), "text[本文@リリア:1]");
    // `mode_expr` は最大 2 つ（§5.1）
    assert_eq!(line_tag_of("本文@リリア:A+B+C"), "text[本文@リリア:A+B+C]");
    assert_eq!(diags_tag("本文@リリア:A+B+C"), "W002@6..7");
    // 16 字までが `mode_id`
    assert_eq!(
        line_tag_of("x@y:Abcdefghijklmnop"),
        "text[x|sp=y:Abcdefghijklmnop@1..20]"
    );
    assert_eq!(
        line_tag_of("x@y:Abcdefghijklmnopq"),
        "text[x@y:Abcdefghijklmnopq]"
    );
}

#[test]
fn an_annotation_survives_a_broken_name() {
    // 名前が空（`@:A`）は話者注釈にしない
    assert_eq!(line_tag_of("本文@:A"), "text[本文@:A]");
    assert_eq!(diags_tag("本文@:A"), "W002@6..7");
}

// ============================================================ §5.2.1 ルビ

#[test]
fn both_ruby_forms_are_found() {
    // 形 1: `｜base《reading》`（§5.2.1）
    assert_eq!(
        line_tag_of("｜アルス《あるす》は振り返った"),
        "text[｜アルス《あるす》は振り返った|ru=アルス=あるす@0..27*]"
    );
    // 形 2: `kanji_run《reading》`
    assert_eq!(
        line_tag_of("漢字《かんじ》を読む"),
        "text[漢字《かんじ》を読む|ru=漢字=かんじ@0..21]"
    );
    assert_eq!(
        line_tag_of("人々《ひとびと》"),
        "text[人々《ひとびと》|ru=人々=ひとびと@0..24]"
    );
}

#[test]
fn a_ruby_needs_a_base_and_a_reading() {
    // `《` の直前が漢字でなければルビではない
    assert_eq!(line_tag_of("かな《かな》"), "text[かな《かな》]");
    // 読みが空
    assert_eq!(line_tag_of("漢字《》"), "text[漢字《》]");
    // `》` が行内に無ければルビではない（§5.0.1 の 1 行完結）
    assert_eq!(line_tag_of("漢字《かんじ"), "text[漢字《かんじ]");
    // `｜` の直後が `《` なら親文字が無い
    assert_eq!(line_tag_of("｜《あるす》"), "text[｜《あるす》]");
    // エスケープした `《` はルビを開かない（§5.0.3）
    assert_eq!(line_tag_of("\\《かんじ》"), "text[\\《かんじ》]");
}

#[test]
fn several_rubies_on_one_line_are_kept_in_order() {
    assert_eq!(
        line_tag_of("地下《ちか》の旧校舎《こうしゃ》"),
        "text[地下《ちか》の旧校舎《こうしゃ》|ru=地下=ちか@0..18,ru=旧校舎=こうしゃ@21..48]"
    );
    // 話者注釈が本文から切り離されるので、ルビは本文側だけを探す
    assert_eq!(
        line_tag_of("本文 漢字《かんじ》@リリア"),
        "text[本文 漢字《かんじ》|sp=リリア:@28..38|ru=漢字=かんじ@7..28]"
    );
    // 話者注釈にならなかった `@` の後ろのルビは本文のままなので拾われる
    assert_eq!(
        line_tag_of("本文@リリア 漢字《かんじ》"),
        "text[本文@リリア 漢字《かんじ》|ru=漢字=かんじ@17..38]"
    );
}

// ============================================================ §5.2.2 節の確定

#[test]
fn the_spec_example_has_three_sections() {
    assert_eq!(
        sections_tag(SPEC_EXAMPLE),
        "D1|s1-4-2-0|h0|1..4|c1..4 / D2|s1-4-2-0|h0|5..8|c5..8 / D3|s1-4-2-0|h0|9..11|c9..10"
    );
}

#[test]
fn the_spec_example_round_trips_and_has_no_diagnostics() {
    assert_eq!(contents::parse(SPEC_EXAMPLE).to_text(), SPEC_EXAMPLE);
    assert_eq!(diags_tag(SPEC_EXAMPLE), "");
}

#[test]
fn the_spec_example_parses_line_by_line() {
    assert_eq!(
        lines_tag(SPEC_EXAMPLE),
        "scene[01-04-02|d3|id1-4-2-0|t図書室の攻防|c] / \
        text[「やっぱり'アルス、ここにいたのね」|sp=リリア:A@52..64] / blank[] / \
        text[呆れたような声とともに、'リリアが隣の椅子を引き寄せた。] / \
        break[@0|c] / \
        text[「旧校舎の地下遺跡に眠っているっていう'『星屑の時計』の設計図だ」|sp=アルス:@97..107] / \
        blank[] / \
        text['リリアは溜め息をついたが、図面の精巧な書き込みに目を奪われた。] / \
        break[@0|c] / \
        text[「……付き合ってあげるわよ」|sp=リリア:B@42..58|c ここでモードが切り替わる@58..97] / \
        blank[]"
    );
}

#[test]
fn section_numbers_increase_with_each_break() {
    let text = "@01-04-02 x\nあ\n;\nい\n;\nう\n";
    assert_eq!(
        sections_tag(text),
        "D1|s1-4-2-0|h0|1..2|c1..2 / D2|s1-4-2-0|h0|3..4|c3..4 / D3|s1-4-2-0|h0|5..7|c5..6"
    );
}

#[test]
fn an_explicit_d_starts_the_numbering_there() {
    let text = "@01-04-02-03 x\nあ\n;\nい\n";
    assert_eq!(
        sections_tag(text),
        "D3|s1-4-2-3|h0|1..2|c1..2 / D4|s1-4-2-3|h0|3..5|c3..4"
    );
}

#[test]
fn a_new_header_restarts_the_numbering() {
    let text = "@01-04-01 x\nあ\n;\nい\n@01-04-02 y\nう\n";
    assert_eq!(
        sections_tag(text),
        "D1|s1-4-1-0|h0|1..2|c1..2 / D2|s1-4-1-0|h0|3..4|c3..4 / D1|s1-4-2-0|h4|5..7|c5..6"
    );
}

#[test]
fn trailing_blank_lines_are_not_section_content() {
    // §5.2.2「節の末尾の空行は節の内容に含めない」MUST
    let text = "@01-04-01 x\nあ\n\n\n;\n";
    assert_eq!(sections_tag(text), "D1|s1-4-1-0|h0|1..4|c1..2");
    // 途中の空行は内容に残る
    let middle = "@01-04-01 x\nあ\n\nい\n";
    assert_eq!(sections_tag(middle), "D1|s1-4-1-0|h0|1..5|c1..4");
}

#[test]
fn consecutive_breaks_make_an_empty_section() {
    // §5.2.2「`;` が連続した場合、空の節が生成される（診断 W012）」
    let text = "@01-04-01 x\nあ\n;\n;\nい\n";
    assert_eq!(
        sections_tag(text),
        "D1|s1-4-1-0|h0|1..2|c1..2 / D2|s1-4-1-0|h0|3..3|c3..3 / D3|s1-4-1-0|h0|4..6|c4..5"
    );
    // W012 は節を閉じた側の行（2 つ目の `;`）を指す
    assert_eq!(diags_tag(text), "W012@18..19");
}

#[test]
fn a_header_followed_by_a_break_is_an_empty_section() {
    let text = "@01-04-01 x\n;\nい\n";
    assert_eq!(
        sections_tag(text),
        "D1|s1-4-1-0|h0|1..1|c1..1 / D2|s1-4-1-0|h0|2..4|c2..3"
    );
    assert_eq!(diags_tag(text), "W012@12..13");
}

#[test]
fn a_section_of_only_blank_lines_is_empty() {
    let text = "@01-04-01 x\n\n\n;\nい\n";
    assert_eq!(
        sections_tag(text),
        "D1|s1-4-1-0|h0|1..3|c1..1 / D2|s1-4-1-0|h0|4..6|c4..5"
    );
    assert_eq!(diags_tag(text), "W012@14..15");
}

#[test]
fn a_trailing_break_does_not_open_a_section() {
    // `;` で終わるファイルに空の節を足さない（末尾改行の有無で結果を変えない）
    let without = "@01-04-01 x\nあ\n;";
    let with = "@01-04-01 x\nあ\n;\n";
    assert_eq!(sections_tag(without), "D1|s1-4-1-0|h0|1..2|c1..2");
    assert_eq!(sections_tag(with), "D1|s1-4-1-0|h0|1..2|c1..2");
    assert_eq!(diags_tag(with), "");
    // 見出しだけのファイルにも節は出来ない（末尾の空行は本文ではない）
    assert_eq!(sections_tag("@01-04-01 x\n"), "");
    assert_eq!(diags_tag("@01-04-01 x\n"), "");
}

#[test]
fn lines_before_the_first_header_belong_to_no_section() {
    // 節は場面見出しから始まる（§5.2.2、`PLANS.md` §9.2 B-32）
    let text = "前置き\n;\n@01-04-01 x\nあ\n";
    assert_eq!(sections_tag(text), "D1|s1-4-1-0|h2|3..5|c3..4");
    assert_eq!(owner_tag(text, 0), "-");
    assert_eq!(owner_tag(text, 1), "-");
    assert_eq!(owner_tag(text, 2), "-");
    assert_eq!(owner_tag(text, 3), "1");
    assert_eq!(diags_tag(text), "");
}

#[test]
fn a_comment_line_inside_a_section_is_content() {
    let text = "@01-04-01 x\n// メモ\nあ\n";
    assert_eq!(sections_tag(text), "D1|s1-4-1-0|h0|1..4|c1..3");
}

// ============================================================ 診断

#[test]
fn a_malformed_scene_id_is_e002() {
    // §6.2.5「行頭（空白のみ先行）→ 構造見出し。`scene_id` としてパース → **E002**」
    assert_eq!(diags_tag("@01-04 x"), "E002@0..6");
    assert_eq!(diags_tag("@01-04-02-03-05 x"), "E002@0..15");
    assert_eq!(diags_tag("@01--02 x"), "E002@0..7");
    assert_eq!(diags_tag("@01-04-xx x"), "E002@0..9");
    assert_eq!(diags_tag("@012345-01-02 x"), "E002@0..13");
    assert_eq!(diags_tag("@ x"), "E002@0..1");
    assert_eq!(diags_tag("@"), "E002@0..1");
}

#[test]
fn a_broken_header_still_makes_sections() {
    // §4.3-4「部分木を返す」。ID が壊れていても節は続く（`scene` だけ `None`）
    let text = "@01-04 x\nあ\n;\nい\n";
    assert_eq!(
        sections_tag(text),
        "D1|s-|h0|1..2|c1..2 / D2|s-|h0|3..5|c3..4"
    );
    assert_eq!(diags_tag(text), "E002@0..6");
}

#[test]
fn a_reserved_scene_zero_is_e020() {
    // §5.1.2「`C = 00` は予約（未使用）。v1 では書いてはならない（**E020**）」
    assert_eq!(diags_tag("@01-04-00 x"), "E020@0..9(3)");
    // `A = 00` は既定では正当（§5.1.2「パースを拒否しない」MUST）
    assert_eq!(diags_tag("@00-01-02 x"), "");
    assert_eq!(diags_with("@00-01-02 x", &strict()), "E020@0..9(3)");
    // `B = 00`（章あらすじ）は C 形式でも番号としては正当
    assert_eq!(diags_tag("@01-00-02 x"), "");
    assert_eq!(diags_with("@01-00-02 x", &strict()), "");
    // `D = 00` は「場面全体」で、見出しに書けば節番号 0 から始まる
    assert_eq!(diags_tag("@01-04-02-00 x"), "");
}

#[test]
fn diagnostics_are_ordered_by_line() {
    let text = "@01-04 x\n本文 @ 裸\n;\n;\n";
    let parsed = contents::parse(text);
    assert_eq!(
        codes(&parsed.diagnostics),
        vec![
            DiagnosticCode::E002,
            DiagnosticCode::W002,
            DiagnosticCode::W012,
            DiagnosticCode::W012
        ]
    );
}

// ============================================================ round-trip

#[test]
fn round_trip_is_byte_exact() {
    // §5.3 完了条件 2。末尾改行の有無・空白だけの行・未知の構文もすべて戻る
    let cases = [
        "",
        "\n",
        "\n\n\n",
        "@01-04-02",
        "@01-04-02 x\n",
        SPEC_EXAMPLE,
        "   \n\t\n　\n",
        ";",
        ";\n;\n;\n",
        "// メモ\n",
        "本文 /* メモ */ 続き @リリア:A // 注\n",
        "\\@ \\; \\/ \\「 \\」 \\\\ \\'",
        "　そして、\n\n@01-04-02-03 差し込み\nあ\n",
        "https://example.com/a:b // メモ",
        "^M0137 ! メモ # コメント 'リリア",
        "@01-04 x\n@01-04-00 y\n@00-01-02 z\n",
    ];
    for text in cases {
        assert_eq!(contents::parse(text).to_text(), text, "{text}");
    }
}

#[test]
fn a_line_decomposes_into_body_speaker_and_comment() {
    // `raw` = `body` ＋ 話者注釈 ＋ 行末コメント（隙間なく、§5.2.1 の並ぶ順）
    let text = "「行くわよ」@リリア:B    // メモ";
    let parsed = contents::parse(text);
    let Line::Text(found) = &parsed.lines[0] else {
        panic!("本文のはず");
    };
    let speaker = found.speaker.as_ref().expect("話者注釈がある");
    let comment = found.comment.as_ref().expect("行末コメントがある");
    let mut rebuilt = found.body.to_owned();
    rebuilt.push_str(&found.raw[speaker.range.clone()]);
    rebuilt.push_str(&found.raw[comment.range.clone()]);
    assert_eq!(rebuilt, found.raw);
    assert_eq!(found.body, "「行くわよ」");
}

#[test]
fn broken_input_does_not_panic() {
    // §5.3 完了条件 3。閉じない `'{`・不正な SceneId・途中切れ
    let cases = [
        "'{リリア",
        "'{リリア:mode",
        "@",
        "@-",
        "@--",
        "@01-04-02-",
        "@99999999-99999999-99999999 x",
        "《》《",
        "｜《》",
        "@リリア:@リリア:@リリア",
        "/* 閉じない",
        "//",
        ";@;",
        "\u{0}",
        "　《かんじ》",
    ];
    for text in cases {
        let parsed = contents::parse(text);
        assert_eq!(parsed.to_text(), text, "{text}");
        assert!(!parsed.lines.is_empty(), "{text}");
    }
}

#[test]
fn an_empty_document_has_one_blank_line_and_no_sections() {
    let parsed = contents::parse("");
    assert_eq!(parsed.lines.len(), 1);
    assert_eq!(parsed.lines[0], Line::Blank(""));
    assert!(parsed.sections.is_empty());
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.to_text(), "");
}
