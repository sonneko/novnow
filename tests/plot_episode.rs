//! `core::syntax::plot_episode`（B 形式、§5.4）のテスト。
//!
//! `docs/history.md` の完了条件の 1（§5.4.3・§5.4.4・§5.4.5 の例が期待どおりにパースできる）・
//! 2（未知フィールドキーを含むファイルの round-trip がバイト一致）・3（壊れた入力でパニック
//! しない）に対応する。
//!
//! 比較は [`lines_tag`] / [`scenes_tag`] / [`sections_tag`] / [`diags_tag`] が作る短い文字列で
//! 行う（`tests/contents.rs`・`tests/plot_outline.rs` と同じ流儀）。タグの意味:
//!
//! - 行: `scene[…]` `break[…]` `field[…]` `cont(…)[…]` `point[…]` `comment[…]` `blank[…]`
//! - 場面 `S<n>|h<行>|id<展開済み>|d<階層>|<span>|f<場面レベルのフィールド行>`
//! - 節 `s<n>|n<節番号>|<span>|c<内容>|b<閉じた ;>|f<節フィールド>|p<point 行>`

use std::ops::Range;

use novnow::core::diagnostics::{Arg, Diagnostic};
use novnow::core::id::SceneId;
use novnow::core::syntax::contents::Comment;
use novnow::core::syntax::plot_episode::{self, CharRef, Field, Line, Options, Scene, Section};
use novnow::core::text::lexer::FieldKey;

// ============================================================ フィクスチャ

/// §5.4.3 の例（`plots/01-04.txt`）。**仕様のコード例をそのまま**使う（完了条件 1）。
///
/// `space:` の継続行は**字下げが値の一部**なので、行を並べて連結する
/// （`"…\n\` の折り返しは次の行の先頭空白を落としてしまう）。
fn spec_episode() -> String {
    let rows = [
        "@01-04-02 図書室の攻防",
        "cast: 'アルス 'リリア",
        "mode: 'アルス:A 'リリア:A",
        "pov: 'アルス",
        "when: 406 09:40",
        "space-mode: ascii",
        "space:",
        "  ┌────────┬────┐",
        "  │ 書架A  │ 窓 │     入口は左下。",
        "  │ 机     │    │     リリアは窓を背にして座る。",
        "  └────────┴────┘",
        "",
        "幼馴染の'リリア が、試験勉強をサボっている'アルス を咎める。",
        "senses: 視覚 聴覚",
        "check: リリアが「怒っているが見捨てない」ことが伝わること",
        ";",
        "'アルス は地下遺跡の'星屑の時計 の謎を解こうとしていると明かす。",
        "material: 'M0137",
        "senses: 視覚 嗅覚",
        ";",
        "mode: 'リリア:B+C          // 呆れモードと好奇心モードの併用（2 つまで）",
        "# ここの心変わりは何度か書き直すかもしれない",
        "'リリア は呆れつつも、図面の精巧さに好奇心を抑えられなくなる。",
        "check: 心変わりを台詞で説明しない",
        "length: 500",
        "illustration: 挿絵候補 / リリアが図面を覗き込む / 窓からの逆光",
        ";",
        "二人は放課後に旧校舎へ向かうことを決める。",
        "state: fixed",
        ";",
    ];
    let mut out = rows.join("\n");
    out.push('\n');
    out
}

/// §5.4.4 の例（`senses:`）。
const SPEC_SENSES: &str = "@01-04-02 図書室の攻防\nsenses: 視覚 聴覚\n;\nsenses: 視覚 嗅覚 触覚\n古い紙の匂いと、埃の舌触り。\n;\n";

/// §5.4.5 の例（`illustration:`）。**最初の節が空**なのがこの例のポイント。
const SPEC_ILLUSTRATION: &str = "@01-04-02 図書室の攻防\n;\nillustration: 見開き / リリアが階段を見上げる / 逆光 / 制服は夏服\nnote: 口絵候補\n;\n";

const PREAMBLE: &str = "note: 前の行\n;\n@01-04-02 攻防\ncast: a\n";
const BREAK_FIRST: &str = ";\n@01-04-02 攻防\n何か起こる。\n;";
const TWO_SCENES: &str =
    "@01-04-01 一\nmode: 'アルス:A\n始まる。\n;\n@01-04-02 二\n続く。\n;\n おわり";
const FOUR_LEVELS: &str = "@01-04-02-07 攻防\ncast: a\n;\n次\n";
const EMPTY: &str = "";
const NO_BREAK: &str = "@01-04-02 攻防\ncast: a\n本文。\n";
const TRAILING_BREAK: &str = "@01-04-02 攻防\n本文。\n;\n";
const BROKEN_ID: &str = "@xx 攻防\n本文。\n;\n";
const SHALLOW_ID: &str = "@01-04 攻防\n本文。\n";
const RESERVED: &str = "@01-04-00 攻防\n本文。\n";
const CHAPTER_ZERO: &str = "@00-01-02 攻防\n本文。\n";
const DUPLICATE: &str = "@01-04-02 一\n本文。\n;\n@01-4-2 二\nまた。\n";
const DUPLICATE_DEPTH: &str = "@01-04-02 一\n@01-04-02-01 二\n";
const COMMENTS: &str = "@01-04-02 攻防\n// 場面のメモ\ncast: a\n# 井戸\n本文。\n// 節のメモ\n;";
const ORPHAN_CONT: &str = "  続きだけ\n@01-04-02 攻防\n本文。\n";
const BLANK_TAIL: &str = "@01-04-02 攻防\n本文。\n\n   \n;\n";
const MODE_THREE: &str = "@01-04-02 攻防\nmode: 'アルス:A+B+C\n本文。\n";
const MODE_TWICE: &str = "@01-04-02 攻防\nmode: 'アルス:A 'アルス:B\n本文。\n";
const MODE_BOTH: &str =
    "@01-04-02 攻防\nmode: 'アルス:A+B+C 'リリア:A 'リリア:B 'アルス:D\n本文。\n";
const MODE_TWO: &str = "@01-04-02 攻防\nmode: 'リリア:B+C // 2 つまで\n本文。\n";
const MODE_NOT_ID: &str = "@01-04-02 攻防\nmode: 'アルス:A+戦闘+B\n本文。\n";
const MODE_PLAIN: &str = "@01-04-02 攻防\ncast: 'アルス 'リリア\n本文。\n";
const UNKNOWN_KEYS: &str =
    "@01-04-02 攻防\nwhen: 406 09:40\nsenses: 視覚\n本文。\nillustration: 見開き / 逆光\n;";
const SATURATE: &str = "@01-04-02-9998 攻防\n一\n;\n二\n;\n三\n";
const CONT_CHAIN: &str = "@01-04-02 攻防\nspace-mode: ascii\nspace:\n  ┌────┐\n  │ 机 │\n  └────┘\n\n  孤児\n// コメント\n  続き\n本文。\n";
const BROKEN: &str = "@\n@01-04-02\n; ; ;\n  \n:\nmode: '\n@01-04-02-03-04\n\\@ エスケープ\n";
// ============================================================ 描画ヘルパ

fn join(parts: impl Iterator<Item = String>) -> String {
    parts.collect::<Vec<String>>().join(" / ")
}

fn range_text(range: &Range<usize>) -> String {
    format!("{}..{}", range.start, range.end)
}

/// 見出しの ID。省略された階層は 0（`expand_low`）、壊れていれば `-`。
fn id_text(id: Option<SceneId>) -> String {
    match id {
        Some(found) => format!(
            "{}-{}-{}-{}",
            found.chapter, found.episode, found.scene, found.section
        ),
        None => "-".to_owned(),
    }
}

fn indices(found: &[usize]) -> String {
    if found.is_empty() {
        return "-".to_owned();
    }
    let parts: Vec<String> = found.iter().map(|index| index.to_string()).collect();
    parts.join(",")
}

/// コメント（`<本文>@<範囲>`）。無ければ空。`tests/contents.rs` と同じ流儀。
fn comment_tag(comment: Option<&Comment<'_>>) -> String {
    match comment {
        Some(found) => format!("{}@{}", found.text, range_text(&found.range)),
        None => String::new(),
    }
}

fn line_tag(line: &Line<'_>) -> String {
    match line {
        Line::Scene(found) => format!(
            "scene[{}|d{}|id{}|t{}|c{}]",
            found.id_text,
            found.depth,
            id_text(found.id),
            found.title,
            comment_tag(found.comment.as_ref())
        ),
        Line::Break(found) => format!(
            "break[@{}|c{}]",
            found.sigil_at,
            comment_tag(found.comment.as_ref())
        ),
        Line::Field(found) => {
            let mut out = format!("field[{}|{:?}|v{}", found.key_text, found.key, found.value);
            if !found.extra.is_empty() {
                out.push_str("|x");
                out.push_str(&found.extra.join("\u{23ce}"));
            }
            out.push(']');
            out
        }
        Line::Continuation(found) => format!("cont({})[{}]", found.indent, found.text),
        Line::Point(found) => format!("point[{found}]"),
        Line::Comment(found) => format!("comment[{}]", comment_tag(Some(&found.comment))),
        Line::Blank(found) => format!("blank[{found}]"),
    }
}

fn scene_tag(index: usize, scene: &Scene) -> String {
    format!(
        "S{}|h{}|id{}|d{}|{}|f{}",
        index,
        scene.header,
        id_text(scene.id),
        scene.depth,
        range_text(&scene.span),
        indices(&scene.fields)
    )
}

fn section_tag(at: usize, section: &Section) -> String {
    let closed = match section.break_line {
        Some(found) => found.to_string(),
        None => "-".to_owned(),
    };
    format!(
        "s{}|n{}|{}|c{}|b{}|f{}|p{}",
        at,
        section.number,
        range_text(&section.span),
        range_text(&section.content),
        closed,
        indices(&section.fields),
        indices(&section.points)
    )
}

fn ref_tag(found: &CharRef<'_>) -> String {
    let explicit = if found.explicit { "*" } else { "" };
    let modifier = found.modifier.unwrap_or("-");
    format!(
        "ref[{}{}|m{}@{}]",
        found.name,
        explicit,
        modifier,
        range_text(&found.range)
    )
}

fn diag_tag(found: &Diagnostic) -> String {
    let range = match &found.range {
        Some(found) => range_text(found),
        None => "-".to_owned(),
    };
    if found.args.is_empty() {
        return format!("{}@{}", found.code.as_str(), range);
    }
    let args: Vec<String> = found.args.iter().map(arg_tag).collect();
    format!("{}@{}({})", found.code.as_str(), range, args.join(","))
}

fn arg_tag(found: &Arg) -> String {
    match found {
        Arg::Text(value) => value.clone(),
        Arg::Number(value) => value.to_string(),
    }
}

// ============================================================ 入口ヘルパ

fn lines_tag(text: &str) -> String {
    join(plot_episode::parse(text).lines.iter().map(line_tag))
}

fn line_tag_of(text: &str, index: usize) -> String {
    line_tag(&plot_episode::parse(text).lines[index])
}

fn scenes_of(parsed: &plot_episode::EpisodePlot<'_>) -> String {
    let parts: Vec<String> = parsed
        .scenes
        .iter()
        .enumerate()
        .map(|(index, scene)| scene_tag(index, scene))
        .collect();
    parts.join(" / ")
}

fn scenes_tag(text: &str) -> String {
    scenes_of(&plot_episode::parse(text))
}

fn sections_of(parsed: &plot_episode::EpisodePlot<'_>, index: usize) -> String {
    let Some(scene) = parsed.scenes.get(index) else {
        return "-".to_owned();
    };
    if scene.sections.is_empty() {
        return "-".to_owned();
    }
    let parts: Vec<String> = scene
        .sections
        .iter()
        .enumerate()
        .map(|(at, section)| section_tag(at, section))
        .collect();
    parts.join(" / ")
}

fn sections_tag(text: &str, index: usize) -> String {
    sections_of(&plot_episode::parse(text), index)
}

fn preamble_tag(text: &str) -> String {
    let parsed = plot_episode::parse(text);
    format!(
        "P{}|f{}",
        range_text(&parsed.preamble.span),
        indices(&parsed.preamble.fields)
    )
}

fn diags_tag(text: &str) -> String {
    join(plot_episode::parse(text).diagnostics.iter().map(diag_tag))
}

fn diags_with(text: &str, options: &Options) -> String {
    let parsed = plot_episode::parse_with(text, options);
    join(parsed.diagnostics.iter().map(diag_tag))
}

/// `index` 行目のフィールドの値の中の参照トークン（§6.2）。
fn refs_tag_of(text: &str, index: usize) -> String {
    let parsed = plot_episode::parse(text);
    let Some(found) = parsed.field(index) else {
        return "-".to_owned();
    };
    join(found.refs().iter().map(ref_tag))
}

/// `index` 行目のフィールドの、継続行まで含めた値と行数（B-37）。
fn full_tag_of(text: &str, index: usize) -> String {
    let parsed = plot_episode::parse(text);
    let Some(found) = parsed.field(index) else {
        return "-".to_owned();
    };
    format!(
        "full[{}]|n{}",
        found.full_value().replace('\n', "\u{23ce}"),
        found.line_count()
    )
}

fn field_tag_of(text: &str, index: usize) -> String {
    match plot_episode::parse(text).field(index) {
        Some(found) => format!("{:?}={}", found.key, found.value),
        None => "-".to_owned(),
    }
}

/// 場面レベルのフィールドの値（出現順）。
fn scene_keys_of(text: &str, index: usize, key: FieldKey) -> String {
    let parsed = plot_episode::parse(text);
    values(parsed.scene_fields(index, key))
}

/// 節レベルのフィールドの値（出現順）。
fn section_keys_of(text: &str, index: usize, at: usize, key: FieldKey) -> String {
    let parsed = plot_episode::parse(text);
    values(parsed.section_fields(index, at, key))
}

fn values(found: Vec<&Field<'_>>) -> String {
    if found.is_empty() {
        return "-".to_owned();
    }
    join(found.iter().map(|field| field.value.to_owned()))
}

/// `lines[index]` の持ち主（`scene_of` / `section_of`）。
fn owner_tag_of(text: &str, index: usize) -> String {
    let parsed = plot_episode::parse(text);
    let scene = parsed.scenes.iter().position(|s| s.span.contains(&index));
    match (scene, parsed.section_of(index)) {
        (Some(found), Some(section)) => format!("S{found}|n{}", section.number),
        (Some(found), None) => format!("S{found}|n-"),
        (None, _) => "-".to_owned(),
    }
}

/// 0 章を拒否する設定（§5.1.2 `structure.allow_chapter_zero = false`）。
fn strict() -> Options {
    Options {
        allow_chapter_zero: false,
    }
}

/// 全フィクスチャ（round-trip と no-panic の確認用）。
fn fixtures() -> Vec<String> {
    let rows = [
        SPEC_SENSES,
        SPEC_ILLUSTRATION,
        PREAMBLE,
        BREAK_FIRST,
        TWO_SCENES,
        FOUR_LEVELS,
        EMPTY,
        NO_BREAK,
        TRAILING_BREAK,
        BROKEN_ID,
        SHALLOW_ID,
        RESERVED,
        CHAPTER_ZERO,
        DUPLICATE,
        DUPLICATE_DEPTH,
        COMMENTS,
        ORPHAN_CONT,
        BLANK_TAIL,
        MODE_THREE,
        MODE_TWICE,
        MODE_BOTH,
        MODE_TWO,
        MODE_NOT_ID,
        MODE_PLAIN,
        UNKNOWN_KEYS,
        SATURATE,
        CONT_CHAIN,
        BROKEN,
    ];
    let mut out: Vec<String> = rows.iter().map(|text| text.to_string()).collect();
    out.push(spec_episode());
    out
}

// ============================================================ §5.4.3 の例

#[test]
fn the_spec_example_has_one_scene_with_four_sections() {
    let text = spec_episode();
    assert_eq!(scenes_tag(&text), "S0|h0|id1-4-2-0|d3|0..31|f1,2,3,4,5,6");
    assert_eq!(
        sections_tag(&text, 0),
        "s0|n1|12..15|c12..15|b15|f13,14|p12 / s1|n2|16..19|c16..19|b19|f17,18|p16 / s2|n3|20..26|c20..26|b26|f20,23,24,25|p22 / s3|n4|27..29|c27..29|b29|f28|p27"
    );
}

/// §5.4.1 の EBNF: `section` は最初の `point_line` から始まるので、それより前のフィールドだけが
/// 場面レベル（§5.4.2 の「適用」列で `check` が節なのとも整合する）。
#[test]
fn scene_level_fields_stop_at_the_first_point_line() {
    let text = spec_episode();
    assert_eq!(scene_keys_of(&text, 0, FieldKey::Cast), "'アルス 'リリア");
    assert_eq!(
        scene_keys_of(&text, 0, FieldKey::Mode),
        "'アルス:A 'リリア:A"
    );
    assert_eq!(scene_keys_of(&text, 0, FieldKey::Pov), "'アルス");
    assert_eq!(scene_keys_of(&text, 0, FieldKey::Check), "-");
    assert_eq!(
        section_keys_of(&text, 0, 0, FieldKey::Check),
        "リリアが「怒っているが見捨てない」ことが伝わること"
    );
    assert_eq!(
        section_keys_of(&text, 0, 2, FieldKey::Check),
        "心変わりを台詞で説明しない"
    );
    assert_eq!(section_keys_of(&text, 0, 1, FieldKey::Material), "'M0137");
    assert_eq!(section_keys_of(&text, 0, 2, FieldKey::Length), "500");
    assert_eq!(section_keys_of(&text, 0, 3, FieldKey::State), "fixed");
}

#[test]
fn the_spec_example_reads_the_space_memo_as_a_continuation() {
    let text = spec_episode();
    assert_eq!(text.split('\n').count(), 31);
    assert_eq!(
        full_tag_of(&text, 6),
        "full[⏎┌────────┬────┐⏎│ 書架A  │ 窓 │     入口は左下。⏎│ 机     │    │     リリアは窓を背にして座る。⏎└────────┴────┘]|n5"
    );
    assert_eq!(line_tag_of(&text, 11), "blank[]");
    assert_eq!(line_tag_of(&text, 7), "cont(2)[┌────────┬────┐]");
}

/// ② のキー（`when` `space` `space-mode` `senses` `illustration`）は W011 になるが、
/// **値は改変せずに保持する**（§5.0.6、`docs/history.md` の「② の生命線」）。
#[test]
fn the_spec_example_reads_stage_two_fields_and_keeps_their_values() {
    let text = spec_episode();
    assert_eq!(diags_tag(&text), "");
    assert_eq!(scene_keys_of(&text, 0, FieldKey::When), "406 09:40");
    assert_eq!(scene_keys_of(&text, 0, FieldKey::SpaceMode), "ascii");
    assert_eq!(section_keys_of(&text, 0, 0, FieldKey::Senses), "視覚 聴覚");
    assert_eq!(section_keys_of(&text, 0, 1, FieldKey::Senses), "視覚 嗅覚");
    assert_eq!(
        section_keys_of(&text, 0, 2, FieldKey::Illustration),
        "挿絵候補 / リリアが図面を覗き込む / 窓からの逆光"
    );
}

/// §5.4.2「1 キャラに 2 つまでのモード」は許す。行末コメントは値に残る（B-36）。
#[test]
fn two_modes_on_one_character_are_allowed() {
    let text = spec_episode();
    assert_eq!(refs_tag_of(&text, 20), "ref[リリア|mB+C@0..14]");
    assert_eq!(
        section_keys_of(&text, 0, 2, FieldKey::Mode),
        "'リリア:B+C          // 呆れモードと好奇心モードの併用（2 つまで）"
    );
}

/// Additional specification 15: hash lines are comments.
#[test]
fn a_hash_line_is_a_comment() {
    let text = spec_episode();
    assert_eq!(
        line_tag_of(&text, 21),
        "comment[# ここの心変わりは何度か書き直すかもしれない@0..65]"
    );
    assert!(sections_tag(&text, 0).contains("|p12"));
}

#[test]
fn the_spec_example_round_trips_byte_for_byte() {
    let text = spec_episode();
    assert_eq!(plot_episode::parse(&text).to_text(), text);
}

// ============================================================ §5.4.4 / §5.4.5 の例

#[test]
fn senses_before_the_first_point_is_scene_level() {
    assert_eq!(scenes_tag(SPEC_SENSES), "S0|h0|id1-4-2-0|d3|0..7|f1");
    assert_eq!(scene_keys_of(SPEC_SENSES, 0, FieldKey::Senses), "視覚 聴覚");
    assert_eq!(
        section_keys_of(SPEC_SENSES, 0, 1, FieldKey::Senses),
        "視覚 嗅覚 触覚"
    );
    assert_eq!(diags_tag(SPEC_SENSES), "");
}

/// §5.4.5 の例は最初の節が**空**。B 形式では空の節が正当なので W012 は出さない
/// （§5.4 に対応する記述が無い）。1:1 対応（W030）のために節としては数える。
#[test]
fn the_illustration_example_has_an_empty_first_section() {
    assert_eq!(
        sections_tag(SPEC_ILLUSTRATION, 0),
        "s0|n1|1..1|c1..1|b1|f-|p- / s1|n2|2..4|c2..4|b4|f2,3|p-"
    );
    assert_eq!(diags_tag(SPEC_ILLUSTRATION), "");
    assert_eq!(
        section_keys_of(SPEC_ILLUSTRATION, 0, 1, FieldKey::Illustration),
        "見開き / リリアが階段を見上げる / 逆光 / 制服は夏服"
    );
    assert_eq!(
        section_keys_of(SPEC_ILLUSTRATION, 0, 1, FieldKey::Note),
        "口絵候補"
    );
    assert_eq!(scenes_tag(SPEC_ILLUSTRATION), "S0|h0|id1-4-2-0|d3|0..6|f-");
}

// ============================================================ 前文と場面

#[test]
fn fields_before_the_first_header_are_the_preamble() {
    assert_eq!(preamble_tag(PREAMBLE), "P0..2|f0");
    assert_eq!(
        lines_tag(PREAMBLE),
        "field[note|Note|v前の行] / break[@0|c] / scene[01-04-02|d3|id1-4-2-0|t攻防|c] / field[cast|Cast|va] / blank[]"
    );
    assert_eq!(scenes_tag(PREAMBLE), "S0|h2|id1-4-2-0|d3|2..5|f3");
    // 場面レベルのフィールドだけで終わる場面には節ができない（§5.4.1 `section` は 0 行でもよい）
    assert_eq!(sections_tag(PREAMBLE, 0), "-");
}

#[test]
fn a_section_break_before_the_first_header_does_nothing() {
    assert_eq!(preamble_tag(BREAK_FIRST), "P0..1|f-");
    assert_eq!(scenes_tag(BREAK_FIRST), "S0|h1|id1-4-2-0|d3|1..4|f-");
    assert_eq!(sections_tag(BREAK_FIRST, 0), "s0|n1|2..3|c2..3|b3|f-|p2");
    assert_eq!(diags_tag(BREAK_FIRST), "");
}

#[test]
fn each_scene_numbers_its_own_sections() {
    assert_eq!(
        scenes_tag(TWO_SCENES),
        "S0|h0|id1-4-1-0|d3|0..4|f1 / S1|h4|id1-4-2-0|d3|4..8|f-"
    );
    assert_eq!(sections_tag(TWO_SCENES, 0), "s0|n1|2..3|c2..3|b3|f-|p2");
    assert_eq!(
        sections_tag(TWO_SCENES, 1),
        "s0|n1|5..6|c5..6|b6|f-|p5 / s1|n2|7..8|c7..8|b-|f-|p7"
    );
    assert_eq!(scene_keys_of(TWO_SCENES, 0, FieldKey::Mode), "'アルス:A");
    assert_eq!(scene_keys_of(TWO_SCENES, 1, FieldKey::Mode), "-");
}

/// §5.2.2「見出しに `-D` が明示されていれば、その直後の節番号を D から開始する」。
#[test]
fn a_four_level_header_starts_numbering_from_d() {
    assert_eq!(scenes_tag(FOUR_LEVELS), "S0|h0|id1-4-2-7|d4|0..5|f1");
    assert_eq!(
        sections_tag(FOUR_LEVELS, 0),
        "s0|n7|2..2|c2..2|b2|f-|p- / s1|n8|3..5|c3..4|b-|f-|p3"
    );
}

#[test]
fn an_empty_file_has_no_scenes() {
    assert_eq!(lines_tag(EMPTY), "blank[]");
    assert_eq!(preamble_tag(EMPTY), "P0..1|f-");
    assert_eq!(scenes_tag(EMPTY), "");
    assert_eq!(diags_tag(EMPTY), "");
    assert_eq!(plot_episode::parse(EMPTY).to_text(), EMPTY);
}

#[test]
fn a_scene_without_a_break_still_gets_one_section() {
    assert_eq!(sections_tag(NO_BREAK, 0), "s0|n1|2..4|c2..3|b-|f-|p2");
    assert_eq!(scenes_tag(NO_BREAK), "S0|h0|id1-4-2-0|d3|0..4|f1");
}

/// `;` で終わるファイルに空の節を足さない（C 形式の B-35 と同じ規則）。
#[test]
fn a_file_ending_with_a_break_gets_no_empty_tail_section() {
    assert_eq!(sections_tag(TRAILING_BREAK, 0), "s0|n1|1..2|c1..2|b2|f-|p1");
    assert_eq!(diags_tag(TRAILING_BREAK), "");
}

/// §5.2.2 MUST「節の末尾の空行は節の内容に含めない」。空白だけの行も空行（B-34）。
#[test]
fn trailing_blank_lines_are_not_part_of_the_content() {
    assert_eq!(
        lines_tag(BLANK_TAIL),
        "scene[01-04-02|d3|id1-4-2-0|t攻防|c] / point[本文。] / blank[] / blank[   ] / break[@0|c] / blank[]"
    );
    assert_eq!(sections_tag(BLANK_TAIL, 0), "s0|n1|1..4|c1..2|b4|f-|p1");
}

/// コメント行も空行も `scene_field` の領域を終わらせない（§5.4.1 の EBNF）。
#[test]
fn comments_do_not_end_the_scene_field_region() {
    assert_eq!(
        lines_tag(COMMENTS),
        "scene[01-04-02|d3|id1-4-2-0|t攻防|c] / comment[ 場面のメモ@0..18] / field[cast|Cast|va] / comment[# 井戸@0..8] / point[本文。] / comment[ 節のメモ@0..15] / break[@0|c]"
    );
    assert_eq!(scenes_tag(COMMENTS), "S0|h0|id1-4-2-0|d3|0..7|f2");
    assert_eq!(sections_tag(COMMENTS, 0), "s0|n1|4..6|c4..6|b6|f-|p4");
}

#[test]
fn an_orphan_continuation_stays_a_line_of_its_own() {
    assert_eq!(preamble_tag(ORPHAN_CONT), "P0..1|f-");
    assert_eq!(line_tag_of(ORPHAN_CONT, 0), "cont(2)[続きだけ]");
    assert_eq!(sections_tag(ORPHAN_CONT, 0), "s0|n1|2..4|c2..3|b-|f-|p2");
}

#[test]
fn a_continuation_chain_breaks_on_a_blank_or_a_comment() {
    assert_eq!(full_tag_of(CONT_CHAIN, 2), "full[⏎┌────┐⏎│ 机 │⏎└────┘]|n4");
    assert_eq!(line_tag_of(CONT_CHAIN, 7), "cont(2)[孤児]");
    assert_eq!(line_tag_of(CONT_CHAIN, 8), "comment[ コメント@0..15]");
    assert_eq!(line_tag_of(CONT_CHAIN, 9), "cont(2)[続き]");
    assert_eq!(diags_tag(CONT_CHAIN), "");
    assert_eq!(
        sections_tag(CONT_CHAIN, 0),
        "s0|n1|10..12|c10..11|b-|f-|p10"
    );
}

/// §4.3-4「どのような入力でもパニックしない」。`u16` の節番号は飽和させる。
#[test]
fn section_numbering_saturates_instead_of_panicking() {
    assert_eq!(
        sections_tag(SATURATE, 0),
        "s0|n9998|1..2|c1..2|b2|f-|p1 / s1|n9999|3..4|c3..4|b4|f-|p3 / s2|n10000|5..7|c5..6|b-|f-|p5"
    );
}

// ============================================================ 見出しの診断

#[test]
fn an_unparsable_id_is_e002() {
    assert_eq!(diags_tag(BROKEN_ID), "E002@0..3");
    assert_eq!(scenes_tag(BROKEN_ID), "S0|h0|id-|d0|0..4|f-");
    assert_eq!(sections_tag(BROKEN_ID, 0), "s0|n1|1..2|c1..2|b2|f-|p1");
}

/// B 形式の見出しは 3〜4 階層（§5.4.1 `scene_header`）。2 階層は E020（C 形式と同じ。
/// 第 2.8 版で E002 から変更、`docs/history.md` A-10）。
#[test]
fn a_two_level_id_is_e020_because_b_needs_three_levels() {
    assert_eq!(diags_tag(SHALLOW_ID), "E020@0..6(3-4)");
    assert_eq!(scenes_tag(SHALLOW_ID), "S0|h0|id-|d2|0..3|f-");
}

#[test]
fn a_reserved_scene_number_is_e022() {
    assert_eq!(diags_tag(RESERVED), "E022@0..9(01-04-00)");
    assert_eq!(scenes_tag(RESERVED), "S0|h0|id1-4-0-0|d3|0..3|f-");
}

#[test]
fn chapter_zero_follows_the_project_setting() {
    assert_eq!(diags_tag(CHAPTER_ZERO), "");
    assert_eq!(diags_with(CHAPTER_ZERO, &strict()), "E022@0..9(00-01-02)");
}

#[test]
fn the_same_id_twice_is_e008() {
    assert_eq!(diags_tag(DUPLICATE), "E008@26..33(01-4-2)");
    assert_eq!(
        scenes_tag(DUPLICATE),
        "S0|h0|id1-4-2-0|d3|0..3|f- / S1|h3|id1-4-2-0|d3|3..6|f-"
    );
}

/// `@01-04-02` と `@01-04-02-01` は別の見出し（階層が違う）。
#[test]
fn the_same_id_at_a_different_depth_is_not_e008() {
    assert_eq!(diags_tag(DUPLICATE_DEPTH), "");
    assert_eq!(
        scenes_tag(DUPLICATE_DEPTH),
        "S0|h0|id1-4-2-0|d3|0..1|f- / S1|h1|id1-4-2-1|d4|1..3|f-"
    );
    assert_eq!(sections_tag(DUPLICATE_DEPTH, 0), "-");
    assert_eq!(sections_tag(DUPLICATE_DEPTH, 1), "-");
}

// ============================================================ E005（§5.4.2 の制約）

#[test]
fn three_modes_on_one_character_is_e005() {
    assert_eq!(diags_tag(MODE_THREE), "E005@23..39");
    assert_eq!(refs_tag_of(MODE_THREE, 1), "ref[アルス|mA+B+C@0..16]");
}

#[test]
fn the_same_character_twice_in_one_mode_is_e005() {
    // 位置は 2 つ目のトークン（1 つ目は正しい）
    assert_eq!(diags_tag(MODE_TWICE), "E005@36..48");
    assert_eq!(
        refs_tag_of(MODE_TWICE, 1),
        "ref[アルス|mA@0..12] / ref[アルス|mB@13..25]"
    );
}

/// §7 E005「先頭 2 つを残す」＝ 3 つ以上でもキャラはその節に居るので、続く重複も E005。
#[test]
fn e005_is_reported_once_per_offending_token() {
    assert_eq!(
        diags_tag(MODE_BOTH),
        "E005@23..39 / E005@53..65 / E005@66..78"
    );
    assert_eq!(sections_tag(MODE_BOTH, 0), "s0|n1|2..4|c2..3|b-|f-|p2");
}

#[test]
fn two_modes_and_a_trailing_comment_are_allowed() {
    assert_eq!(diags_tag(MODE_TWO), "");
    assert_eq!(
        scene_keys_of(MODE_TWO, 0, FieldKey::Mode),
        "'リリア:B+C // 2 つまで"
    );
    assert_eq!(section_keys_of(MODE_TWO, 0, 0, FieldKey::Mode), "-");
}

/// `mode_id` の形をしていない修飾子は E003 / E004（M3、`characters.md` の辞書が要る）。
#[test]
fn a_modifier_that_is_not_a_mode_id_is_left_to_m3() {
    assert_eq!(diags_tag(MODE_NOT_ID), "");
    assert_eq!(refs_tag_of(MODE_NOT_ID, 1), "ref[アルス|mA+戦闘+B@0..21]");
}

/// 修飾子が空（`'アルス:`）のときは `mode_id` でないので E005 にしない。存在しないモードの
/// 診断（E003 / E004）は `characters.md` の辞書が要るので M3（§6.4）。
#[test]
fn an_empty_modifier_is_not_e005() {
    // `let text = "…"` は `&str` なので `&text` にしない（clippy::needless_borrow）
    let text = "@01-04-02 攻防\nmode: 'アルス: 'リリア:A\n本文。\n";
    assert_eq!(diags_tag(text), "");
    assert_eq!(
        refs_tag_of(text, 1),
        "ref[アルス|m@0..11] / ref[リリア|mA@12..24]"
    );
    assert_eq!(scene_keys_of(text, 0, FieldKey::Mode), "'アルス: 'リリア:A");
}

/// E005 は `mode:` だけの規則。`cast:` に同じキャラを 2 回書いてもここでは何も出さない。
#[test]
fn cast_is_not_checked_for_e005() {
    assert_eq!(diags_tag(MODE_PLAIN), "");
    assert_eq!(
        refs_tag_of(MODE_PLAIN, 1),
        "ref[アルス|m-@0..10] / ref[リリア|m-@11..21]"
    );
}

// ============================================================ 未知キー（② の生命線）

#[test]
fn unknown_keys_are_warned_and_their_values_are_kept() {
    assert_eq!(diags_tag(UNKNOWN_KEYS), "");
    assert_eq!(scene_keys_of(UNKNOWN_KEYS, 0, FieldKey::When), "406 09:40");
    assert_eq!(
        section_keys_of(UNKNOWN_KEYS, 0, 0, FieldKey::Illustration),
        "見開き / 逆光"
    );
    assert_eq!(plot_episode::parse(UNKNOWN_KEYS).to_text(), UNKNOWN_KEYS);
}

// ============================================================ アクセサ

#[test]
fn scene_of_and_section_of_find_the_owner() {
    let text = spec_episode();
    assert_eq!(owner_tag_of(&text, 0), "S0|n-");
    assert_eq!(owner_tag_of(&text, 3), "S0|n-");
    assert_eq!(owner_tag_of(&text, 12), "S0|n1");
    assert_eq!(owner_tag_of(&text, 16), "S0|n2");
    assert_eq!(owner_tag_of(&text, 21), "S0|n3");
    assert_eq!(owner_tag_of(&text, 27), "S0|n4");
    assert_eq!(owner_tag_of(&text, 30), "S0|n-");
    assert_eq!(owner_tag_of(&text, 99), "-");
}

#[test]
fn field_returns_the_field_of_a_line() {
    let text = spec_episode();
    let parsed = plot_episode::parse(&text);
    assert_eq!(field_tag_of(&text, 2), "Mode='アルス:A 'リリア:A");
    assert_eq!(field_tag_of(&text, 0), "-");
    assert_eq!(field_tag_of(&text, 11), "-");
    assert!(parsed.field(0).is_none());
    assert!(parsed.field(99).is_none());
    assert_eq!(parsed.field(6).unwrap().key, FieldKey::Space);
}

#[test]
fn accessors_return_nothing_out_of_range() {
    let text = spec_episode();
    assert_eq!(scene_keys_of(&text, 9, FieldKey::Mode), "-");
    assert_eq!(section_keys_of(&text, 0, 9, FieldKey::Mode), "-");
    assert_eq!(sections_tag(&text, 9), "-");
    assert!(!scenes_of(&plot_episode::parse(&text)).contains("S1"));
}

// ============================================================ 完了条件 2・3

#[test]
fn every_fixture_round_trips_byte_for_byte() {
    for text in fixtures() {
        let parsed = plot_episode::parse(&text);
        assert_eq!(parsed.to_text(), text);
        assert_eq!(parsed.lines.len(), text.split('\n').count());
    }
}

/// §5.3 完了条件 3「壊れた入力でも部分木と診断を返す」。
#[test]
fn broken_input_yields_a_partial_tree_and_diagnostics() {
    assert_eq!(
        lines_tag(BROKEN),
        "scene[|d0|id-|t|c] / scene[01-04-02|d3|id1-4-2-0|t|c] / point[; ; ;] / blank[  ] / point[:] / field[mode|Mode|v'] / scene[01-04-02-03-04|d0|id-|t|c] / point[\\@ エスケープ] / blank[]"
    );
    assert_eq!(
        scenes_tag(BROKEN),
        "S0|h0|id-|d0|0..1|f- / S1|h1|id1-4-2-0|d3|1..6|f- / S2|h6|id-|d0|6..9|f-"
    );
    assert_eq!(sections_tag(BROKEN, 0), "-");
    assert_eq!(sections_tag(BROKEN, 1), "s0|n1|2..6|c2..6|b-|f5|p2,4");
    assert_eq!(sections_tag(BROKEN, 2), "s0|n1|7..9|c7..8|b-|f-|p7");
    assert_eq!(diags_tag(BROKEN), "E002@0..1 / E002@31..46");
    assert_eq!(preamble_tag(BROKEN), "P0..0|f-");
    assert_eq!(plot_episode::parse(BROKEN).to_text(), BROKEN);
}
