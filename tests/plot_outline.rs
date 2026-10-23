//! `core::syntax::plot_outline`（A 形式、§5.3）のテスト。
//!
//! `docs/history.md` の完了条件の 1（§5.3.3・§5.3.4 の例が期待どおりにパースできる）・
//! 2（未知フィールドキーを含むファイルの round-trip がバイト一致）・3（壊れた入力でパニックしない）
//! に対応する。
//!
//! 比較は [`lines_tag`] / [`blocks_tag`] / [`diags_tag`] が作る短い文字列で行う
//! （`tests/contents.rs` と同じ流儀）。

use std::ops::Range;

use novnow::core::diagnostics::{Arg, Diagnostic};
use novnow::core::id::SceneId;
use novnow::core::syntax::contents::Comment;
use novnow::core::syntax::plot_outline::{self, Block, CharRef, Foreshadow, Line, Options};
use novnow::core::text::lexer::FieldKey;

/// §5.3.3 の例（`plots/all.txt`）。**仕様のコード例をそのまま**使う（§5.3 完了条件 1）。
const SPEC_ALL: &str = "// 全体プロット: 章を 1 ブロック 1 章で書く。\n\
title: 星屑の時計\n\
summary: 古代錬金術を追う少年と幼馴染が、学院の地下遺跡の魔導具を巡って対立と和解を繰り返す。\n\
\n\
@01 失われた刻印\n\
summary: 'アルス が地下遺跡に惹かれ、'リリア を巻き込んで探索を始める導入章。\n\
goal: 二人の関係性と'星屑の時計 の存在を読者に提示する。\n\
turn: 'リリア が渋々ながら同行を決める。\n\
cast: 'アルス:A 'リリア:A\n\
seed: F01 地下遺跡の封印紋様 -> @03-02\n\
state: fixed\n\
;\n\
\n\
@02 補習と地図\n\
summary: 期末試験の失敗を代償に、二人は旧校舎の鍵を手に入れる。\n\
goal: 代償を払わせることで探索の重みを出す。\n\
cast: 'アルス:B 'リリア:B\n\
payoff: F02 -> @02-05\n\
state: draft\n\
;\n";

/// §5.3.4 の例（`plots/01.txt` — 章プロット）。
const SPEC_CHAPTER: &str = "@01-04 図書室の密談\n\
summary: 試験前の図書室で'アルス が'リリア に設計図を見せ、放課後の探索を約束させる。\n\
goal: 読者に探索の動機とリスクを同時に渡す。\n\
cast: 'アルス:A 'リリア:A\n\
material: 'M0137 旧校舎の構造メモ\n\
length: 5200\n\
state: review\n\
;\n";

/// §5.3.6 の例（`space:` の継続行）。
///
/// **字下げが値の一部**なので、行を並べて連結する（`"…\n\` の折り返しは次の行の先頭空白を
/// 落としてしまうので字下げが消える）。
fn space_memo() -> String {
    let rows = [
        "space-mode: ascii",
        "space:",
        "  ┌─────────┬─────┐",
        "  │  書架A   │ 窓  │",
        "  ├─────────┤     │",
        "  │  机（アルス）  │     │",
        "  │  机（リリア）  │     │",
        "  └─────────┴─────┘",
        "  入口は左下。リリアは窓を背にして座る。",
    ];
    let mut out = rows.join("\n");
    out.push('\n');
    out
}

/// 0 章を拒否する設定（§5.1.2 `structure.allow_chapter_zero = false`）。
fn strict() -> Options {
    Options {
        depth: None,
        allow_chapter_zero: false,
    }
}

/// `plots/all.txt` の設定（見出しは章 = 1 階層、§5.3 の表）。
fn chapters() -> Options {
    Options {
        depth: Some(1),
        allow_chapter_zero: true,
    }
}

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

/// コメント（`<本文>@<範囲>`）。無ければ空。C 形式の `tests/contents.rs` と同じ流儀だが、
/// A 形式では行末コメントの**位置も**検証する（`find_comment` の byte 計算）。
fn comment_tag(comment: Option<&Comment<'_>>) -> String {
    match comment {
        Some(found) => format!("{}@{}", found.text, range_text(&found.range)),
        None => String::new(),
    }
}

fn line_tag(line: &Line<'_>) -> String {
    match line {
        Line::Header(found) => format!(
            "header[{}|d{}|id{}|t{}|c{}]",
            found.id_text,
            found.depth,
            id_text(found.id),
            found.title,
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
        Line::Note(found) => format!("note[{found}]"),
        Line::Comment(found) => format!("comment[{}]", comment_tag(Some(&found.comment))),
        Line::Blank(found) => format!("blank[{found}]"),
        Line::Break(found) => format!(
            "break[@{}|c{}]",
            found.sigil_at,
            comment_tag(found.comment.as_ref())
        ),
    }
}

fn block_tag(index: usize, block: &Block) -> String {
    let fields = indices(&block.fields);
    let notes = indices(&block.notes);
    let end = match block.end {
        Some(found) => found.to_string(),
        None => "-".to_owned(),
    };
    format!(
        "B{}|h{}|id{}|d{}|{}|e{}|f{}|n{}",
        index,
        block.header,
        id_text(block.id),
        block.depth,
        range_text(&block.span),
        end,
        fields,
        notes
    )
}

fn indices(found: &[usize]) -> String {
    if found.is_empty() {
        return "-".to_owned();
    }
    let parts: Vec<String> = found.iter().map(|index| index.to_string()).collect();
    parts.join(",")
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

fn foreshadow_tag(found: &Foreshadow<'_>) -> String {
    format!("fs[{}|n{}|t{}]", found.tag, found.note, found.target)
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
    join(plot_outline::parse(text).lines.iter().map(line_tag))
}

fn line_tag_of(text: &str, index: usize) -> String {
    line_tag(&plot_outline::parse(text).lines[index])
}

fn blocks_of(parsed: &plot_outline::Outline<'_>) -> String {
    let parts: Vec<String> = parsed
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| block_tag(index, block))
        .collect();
    parts.join(" / ")
}

fn blocks_tag(text: &str) -> String {
    blocks_of(&plot_outline::parse(text))
}

fn preamble_tag(text: &str) -> String {
    let parsed = plot_outline::parse(text);
    format!(
        "P{}|f{}",
        range_text(&parsed.preamble.span),
        indices(&parsed.preamble.fields)
    )
}

fn diags_tag(text: &str) -> String {
    join(plot_outline::parse(text).diagnostics.iter().map(diag_tag))
}

fn diags_with(text: &str, options: &Options) -> String {
    let parsed = plot_outline::parse_with(text, options);
    join(parsed.diagnostics.iter().map(diag_tag))
}

/// `index` 行目のフィールドの値の中の参照トークン（§6.2）。
fn refs_tag_of(text: &str, index: usize) -> String {
    let parsed = plot_outline::parse(text);
    let Some(found) = parsed.field(index) else {
        return "-".to_owned();
    };
    join(found.refs().iter().map(ref_tag))
}

/// `index` 行目のフィールドの値を `seed` / `payoff` の形（§5.3.2）として読む。
fn foreshadow_tag_of(text: &str, index: usize) -> String {
    let parsed = plot_outline::parse(text);
    let Some(found) = parsed.field(index) else {
        return "-".to_owned();
    };
    match found.foreshadow() {
        Some(value) => foreshadow_tag(&value),
        None => "-".to_owned(),
    }
}

/// `index` 行目のフィールドの、継続行まで含めた値と行数（B-37）。
fn full_tag_of(text: &str, index: usize) -> String {
    let parsed = plot_outline::parse(text);
    let Some(found) = parsed.field(index) else {
        return "-".to_owned();
    };
    format!(
        "full[{}]|n{}",
        found.full_value().replace('\n', "\u{23ce}"),
        found.line_count()
    )
}

/// `index` 番目のブロックの `key` のフィールドのキー文字列（出現順）。
fn keys_tag_of(text: &str, index: usize, key: FieldKey) -> String {
    let parsed = plot_outline::parse(text);
    let found = parsed.fields_of(index, key);
    if found.is_empty() {
        return "-".to_owned();
    }
    join(found.iter().map(|field| field.value.to_owned()))
}

// ============================================================ §5.3.3 / §5.3.4 の例

#[test]
fn the_spec_example_for_all_txt_has_two_blocks() {
    // §5.3.3。前文（`// …` と `title:` と `summary:`）＋ 2 ブロック
    assert_eq!(preamble_tag(SPEC_ALL), "P0..4|f1,2");
    assert_eq!(
        blocks_tag(SPEC_ALL),
        "B0|h4|id1-0-0-0|d1|4..12|e11|f5,6,7,8,9,10|n- / \
        B1|h13|id2-0-0-0|d1|13..20|e19|f14,15,16,17,18|n-"
    );
    // 見出し行とブロック終わりの `;`
    assert_eq!(
        line_tag_of(SPEC_ALL, 4),
        "header[01|d1|id1-0-0-0|t失われた刻印|c]"
    );
    assert_eq!(line_tag_of(SPEC_ALL, 11), "break[@0|c]");
}

#[test]
fn the_spec_example_for_all_txt_reads_the_title_key() {
    // §5.3.3 の例の `title:` は §5.0.6 の表に**正式追加**された（第 2.6 版。`docs/history.md`
    // A-12）ので、例の診断は 0 件。値は改変せずに保持する（§5.0.6 MUST）
    assert_eq!(diags_tag(SPEC_ALL), "");
    assert_eq!(line_tag_of(SPEC_ALL, 1), "field[title|Title|v星屑の時計]");
    let parsed = plot_outline::parse(SPEC_ALL);
    let titles: Vec<&str> = parsed
        .preamble
        .fields
        .iter()
        .filter_map(|&index| parsed.field(index))
        .filter(|field| field.key == FieldKey::Title)
        .map(|field| field.value)
        .collect();
    assert_eq!(titles, vec!["星屑の時計"]);
    // 別名（§5.0.6「固定・双方向 1:1」）でも同じキーになる
    assert_eq!(diags_tag("題: 星屑の時計\n"), "");
    assert_eq!(
        line_tag_of("題: 星屑の時計\n", 0),
        "field[題|Title|v星屑の時計]"
    );
}

#[test]
fn the_spec_example_for_all_txt_reads_the_field_shapes() {
    // `seed: F01 地下遺跡の封印紋様 -> @03-02` / `payoff: F02 -> @02-05`
    assert_eq!(
        foreshadow_tag_of(SPEC_ALL, 9),
        "fs[F01|n地下遺跡の封印紋様|t@03-02]"
    );
    assert_eq!(foreshadow_tag_of(SPEC_ALL, 17), "fs[F02|n|t@02-05]");
    // `cast: 'アルス:A 'リリア:A`（位置は値の先頭からの byte）
    assert_eq!(
        refs_tag_of(SPEC_ALL, 8),
        "ref[アルス|mA@0..12] / ref[リリア|mA@13..25]"
    );
    // `state: fixed` は ① の表にあるキーなので W011 にならない
    assert_eq!(keys_tag_of(SPEC_ALL, 0, FieldKey::State), "fixed");
    assert_eq!(keys_tag_of(SPEC_ALL, 1, FieldKey::State), "draft");
}

#[test]
fn the_spec_example_for_a_chapter_plot_parses() {
    // §5.3.4。見出しは 2 階層（`@A-B`）、診断は無い
    assert_eq!(
        blocks_tag(SPEC_CHAPTER),
        "B0|h0|id1-4-0-0|d2|0..8|e7|f1,2,3,4,5,6|n-"
    );
    assert_eq!(diags_tag(SPEC_CHAPTER), "");
    assert_eq!(preamble_tag(SPEC_CHAPTER), "P0..0|f-");
    assert_eq!(
        line_tag_of(SPEC_CHAPTER, 0),
        "header[01-04|d2|id1-4-0-0|t図書室の密談|c]"
    );
    // `material: 'M0137 旧校舎の構造メモ` — 補足は参照ではない
    assert_eq!(refs_tag_of(SPEC_CHAPTER, 4), "ref[M0137|m-@0..6]");
    // `length: 5200` は整数として読める
    let parsed = plot_outline::parse(SPEC_CHAPTER);
    assert_eq!(parsed.field(5).and_then(|found| found.number()), Some(5200));
}

#[test]
fn the_spec_examples_round_trip() {
    // §5.3 完了条件 2
    for text in [SPEC_ALL, SPEC_CHAPTER] {
        let parsed = plot_outline::parse(text);
        assert_eq!(&parsed.to_text(), text, "round-trip");
    }
    // §5.3.6 の例（字下げが値の一部）
    let memo = space_memo();
    assert_eq!(&plot_outline::parse(&memo).to_text(), &memo);
}

// ============================================================ §5.3.1 行の種別

#[test]
fn a_heading_keeps_the_id_the_title_and_the_comment() {
    assert_eq!(
        line_tag_of("@01-04 図書室の密談 // メモ", 0),
        "header[01-04|d2|id1-4-0-0|t図書室の密談|c メモ@26..35]"
    );
    // タイトルは無くてもよい
    assert_eq!(line_tag_of("@01", 0), "header[01|d1|id1-0-0-0|t|c]");
    // 行頭の空白 1 文字は許す（§5.3.1 `ws?`）
    assert_eq!(line_tag_of(" @01 x", 0), "header[01|d1|id1-0-0-0|tx|c]");
    // 2 文字以上は継続行のために予約されているので見出しにならない（§5.0.5 手順 1）
    assert_eq!(line_tag_of("  @01 x", 0), "cont(2)[@01 x]");
}

#[test]
fn a_block_end_may_carry_a_line_comment() {
    // §5.3.1 の `block_end` に行末コメントは無いが、§5.0.2 の共通規則で受ける
    // （`docs/history.md` A-11）
    assert_eq!(
        lines_tag("@01 x\nsummary: s\n; // メモ\n"),
        "header[01|d1|id1-0-0-0|tx|c] / field[summary|Summary|vs] / \
        break[@0|c メモ@2..11] / blank[]"
    );
}

#[test]
fn a_hash_comment_is_not_a_note() {
    // Additional specification 15.
    assert_eq!(
        lines_tag("# メモ\n@01 x\nsummary: s\n"),
        "comment[# メモ@0..8] / header[01|d1|id1-0-0-0|tx|c] / field[summary|Summary|vs] / blank[]"
    );
}

#[test]
fn a_whitespace_only_line_is_blank() {
    // B-34（C 形式と同じ）
    assert_eq!(
        lines_tag(" \n\u{3000}\n"),
        "blank[ ] / blank[\u{3000}] / blank[]"
    );
}

#[test]
fn prose_that_is_not_a_field_is_a_note() {
    assert_eq!(
        lines_tag("@01 x\nsummary: s\n自由記述の行\n  字下げの続き\n;\n"),
        "header[01|d1|id1-0-0-0|tx|c] / field[summary|Summary|vs] / note[自由記述の行] / \
        cont(2)[字下げの続き] / break[@0|c] / blank[]"
    );
    // 自由記述はブロックの `notes` に入る。継続行はどのフィールドにも属さないので入らない
    assert_eq!(
        blocks_tag("@01 x\nsummary: s\n自由記述の行\n  字下げの続き\n;\n"),
        "B0|h0|id1-0-0-0|d1|0..5|e4|f1|n2"
    );
}

// ============================================================ フィールドと継続行

#[test]
fn a_japanese_alias_is_a_known_key() {
    // §5.0.6「日本語別名は表にある語のみ受理し、保存時に正準形へ正規化しない」
    assert_eq!(diags_tag("@01 x\n要約：あらすじ\n;\n"), "");
    assert_eq!(
        line_tag_of("@01 x\n要約：あらすじ\n;\n", 1),
        "field[要約|Summary|vあらすじ]"
    );
}

#[test]
fn an_unknown_key_is_w011_and_keeps_its_value() {
    // §5.0.6「未知のキーは W011 とし、値は改変せず保持する」
    assert_eq!(
        diags_tag("@01 x\nsummary: s\nfoo: bar\n;\n"),
        "W011@17..20(foo)"
    );
    assert_eq!(
        line_tag_of("@01 x\nsummary: s\nfoo: bar\n;\n", 2),
        "field[foo|Unknown|vbar]"
    );
}

#[test]
fn stage_two_keys_survive_as_unknown_fields() {
    // ② のキーは ① の表に無いので W011 になり、**値はそのまま残る**（作業項目 2「② の生命線」）
    let text = "@01 x\nsummary: s\nwhen: 405.0 春分 09:15\nspace-mode: ascii\n\
               space:\n  ┌──┐\n  │書架│\nblurb: あらすじ\n;\n";
    assert_eq!(diags_tag(text), "");
    // `space:` の値は空で、図は継続行として付く
    assert_eq!(full_tag_of(text, 4), "full[⏎┌──┐⏎│書架│]|n3");
    assert_eq!(&plot_outline::parse(text).to_text(), text);
}

#[test]
fn a_continuation_line_joins_the_field_value() {
    // §5.0.5「値は 1 行。複数行が必要な場合は継続行」
    assert_eq!(
        full_tag_of("summary: 一行目\n  二行目\n", 0),
        "full[一行目⏎二行目]|n2"
    );
    // 1 タブのインデントも継続行として結合され、先頭 1 タブが落とされる（B-21）
    assert_eq!(
        full_tag_of("summary: 一行目\n\t二行目\n\t\t三行目\n", 0),
        "full[一行目⏎二行目⏎\t三行目]|n3"
    );
    // 2 文字を超える字下げは**形として残る**（§5.3.6 `space-mode: ascii` は整形なし MUST、B-37）
    assert_eq!(
        full_tag_of(&space_memo(), 1),
        "full[⏎\
        ┌─────────┬─────┐⏎\
        │  書架A   │ 窓  │⏎\
        ├─────────┤     │⏎\
        │  机（アルス）  │     │⏎\
        │  机（リリア）  │     │⏎\
        └─────────┴─────┘⏎\
        入口は左下。リリアは窓を背にして座る。]|n8"
    );
}

#[test]
fn a_continuation_needs_a_field_before_it() {
    // 直前がフィールド行でなければ、継続行はどのフィールドにも属さない（B-37）
    assert_eq!(
        lines_tag("  字下げ\n@01 x\nsummary: s\n"),
        "cont(2)[字下げ] / header[01|d1|id1-0-0-0|tx|c] / field[summary|Summary|vs] / blank[]"
    );
    // 空行も継続を中断する（空白だけの行は空行、B-34）
    assert_eq!(full_tag_of("summary: a\n  b\n\n  c\n", 0), "full[a⏎b]|n2");
    assert_eq!(
        lines_tag("summary: a\n  b\n\n  c\n"),
        "field[summary|Summary|va|xb] / cont(2)[b] / blank[] / cont(2)[c] / blank[]"
    );
}

// ============================================================ ブロックの範囲

#[test]
fn fields_before_the_first_heading_are_the_preamble() {
    // §5.3.1 `preamble`。§5.3.7 は前文の `blurb:` を作品全体のあらすじとする
    let text = "title: 星屑の時計\nsummary: 全体の要約\n// メモ\n\n@01 x\nsummary: s\n";
    assert_eq!(preamble_tag(text), "P0..4|f0,1");
    assert_eq!(blocks_tag(text), "B0|h4|id1-0-0-0|d1|4..7|e-|f5|n-");
}

#[test]
fn a_block_end_closes_the_block() {
    // `;` はブロックに含まれ、`end` に行番号が入る
    assert_eq!(
        blocks_tag("@01 x\nsummary: s\n;\n"),
        "B0|h0|id1-0-0-0|d1|0..3|e2|f1|n-"
    );
    // `;` の後ろに本文が無ければ、そこで新しいブロックは開かない
    assert_eq!(
        blocks_tag("@01 x\nsummary: s\n;\n;\n"),
        "B0|h0|id1-0-0-0|d1|0..3|e2|f1|n-"
    );
}

#[test]
fn a_block_runs_to_the_next_heading_or_to_the_end() {
    // `;` が無ければ次の見出しまで。最後のブロックはファイル末尾まで
    assert_eq!(
        blocks_tag("@01 x\nsummary: s\n@02 y\nsummary: t\n"),
        "B0|h0|id1-0-0-0|d1|0..2|e-|f1|n- / B1|h2|id2-0-0-0|d1|2..5|e-|f3|n-"
    );
}

#[test]
fn fields_after_a_block_end_belong_to_no_block() {
    // §5.3.1 の EBNF にその場所は無い。行としては保持するので round-trip は保たれる（B-38）
    let text = "@01 x\nsummary: s\n;\nsummary: 迷子\n\n@02 y\nsummary: t\n";
    assert_eq!(preamble_tag(text), "P0..0|f-");
    assert_eq!(
        blocks_tag(text),
        "B0|h0|id1-0-0-0|d1|0..3|e2|f1|n- / B1|h5|id2-0-0-0|d1|5..8|e-|f6|n-"
    );
    assert_eq!(diags_tag(text), "");
    assert_eq!(plot_outline::parse(text).block_of(3), None);
}

#[test]
fn block_of_finds_the_owner_of_a_line() {
    let text = "@01 x\nsummary: s\n;\n";
    let parsed = plot_outline::parse(text);
    assert_eq!(parsed.block_of(0).map(|block| block.header), Some(0));
    assert_eq!(parsed.block_of(1).map(|block| block.header), Some(0));
    assert_eq!(parsed.block_of(2).map(|block| block.header), Some(0));
    assert_eq!(parsed.block_of(3), None);
}

#[test]
fn fields_of_returns_every_occurrence() {
    // `turn` は 0..n（§5.3.2）。順番のまま全部返る
    let text = "@01 x\nsummary: s\nturn: 一つ目\nturn: 二つ目\n;\n";
    assert_eq!(keys_tag_of(text, 0, FieldKey::Turn), "一つ目 / 二つ目");
    assert_eq!(keys_tag_of(text, 0, FieldKey::Goal), "-");
    assert_eq!(keys_tag_of(text, 9, FieldKey::Turn), "-");
}

// ============================================================ 診断

#[test]
fn a_block_without_a_summary_is_e018() {
    // §5.3.2 MUST「`summary` が無いブロックは E018」。位置は見出し行
    assert_eq!(diags_tag("@01 x\ngoal: y\n;\n"), "E018@0..5");
    // 前文はブロックではないので `summary` を要求されない（`title` は表にあるキー、第 2.6 版）
    assert_eq!(diags_tag("title: 星屑の時計\n"), "");
    assert_eq!(diags_tag("foo: bar\n"), "W011@0..3(foo)");
    // `;` が無くても同じ
    assert_eq!(diags_tag("@01 x\n"), "E018@0..5");
}

#[test]
fn a_summary_over_120_chars_is_w019() {
    // §5.3.2「`summary` が 120 字を超えると W019」。字数は**文字数**（byte ではない）
    let exactly = format!("@01 x\nsummary: {}\n;\n", "あ".repeat(120));
    assert_eq!(diags_tag(&exactly), "");
    let over = format!("@01 x\nsummary: {}\n;\n", "あ".repeat(121));
    assert_eq!(diags_tag(&over), "W019@15..378(121)");
    // 継続行まで含めた字数で数える（改行も 1 字、B-38）。範囲は 1 行目の値
    let joined = format!(
        "@01 x\nsummary: {}\n  {}\n;\n",
        "あ".repeat(60),
        "あ".repeat(61)
    );
    assert_eq!(diags_tag(&joined), "W019@15..195(122)");
}

#[test]
fn a_heading_of_the_wrong_depth_is_e020() {
    // §5.3.2「ファイル内の見出し階層は揃っていること（`all.txt` に `@01-04` が現れたら E020）」
    assert_eq!(
        diags_with("@01 x\nsummary: s\n@01-04 y\nsummary: t\n;\n", &chapters()),
        "E020@17..23(1)"
    );
    // 設定が無いときは**最初の見出し**から階層を決める
    assert_eq!(
        diags_tag("@01 x\nsummary: s\n@02-03 y\nsummary: t\n"),
        "E020@17..23(1)"
    );
    // 揃っていれば診断は無い
    assert_eq!(diags_with("@01 x\nsummary: s\n;\n", &chapters()), "");
}

#[test]
fn a_duplicated_heading_id_is_e008() {
    // §5.3.2「ブロックの見出し ID は同一ファイル内で一意」
    assert_eq!(
        diags_tag("@01 x\nsummary: s\n;\n@01 y\nsummary: t\n;\n"),
        "E008@19..22(01)"
    );
    // ゼロ詰めの違いは同じ ID（`@01` と `@1`）
    assert_eq!(
        diags_tag("@01 x\nsummary: s\n;\n@1 y\nsummary: t\n;\n"),
        "E008@19..21(1)"
    );
}

#[test]
fn a_malformed_heading_id_is_e002() {
    // §6.2.5「行頭（空白のみ先行）→ 構造見出し。`scene_id` としてパース → E002」
    assert_eq!(diags_tag("@abc x\nsummary: s\n;\n"), "E002@0..4");
    // ID が壊れていてもブロックは作る（部分木、完了条件 3）
    assert_eq!(
        lines_tag("@abc x\nsummary: s\n;\n"),
        "header[abc|d0|id-|tx|c] / field[summary|Summary|vs] / break[@0|c] / blank[]"
    );
    assert_eq!(
        blocks_tag("@abc x\nsummary: s\n;\n"),
        "B0|h0|id-|d0|0..3|e2|f1|n-"
    );
}

#[test]
fn chapter_zero_is_rejected_only_when_strict() {
    // §5.1.2「`A = 00` は `structure.allow_chapter_zero = false` にすると **E022** で拒否」
    // （第 2.8 版で E020 から分離。`docs/history.md` A-9）
    assert_eq!(diags_tag("@00 x\nsummary: s\n;\n"), "");
    assert_eq!(
        diags_with("@00 x\nsummary: s\n;\n", &strict()),
        "E022@0..3(00)"
    );
    // `B = 00`（章あらすじ、§5.1.2）は正当
    assert_eq!(
        diags_with("@01-00 あらすじ\nsummary: s\n;\n", &strict()),
        ""
    );
}

#[test]
fn diagnostics_are_ordered_by_line_then_by_block() {
    // 行の診断が行順、そのあとにブロック順の E018（C 形式の W012 と同じ流儀）
    assert_eq!(
        diags_tag("@abc x\nfoo: 1\n@01 y\n;\n@01 z\n;\n"),
        "E002@0..4 / W011@7..10(foo) / E008@22..25(01) / E018@0..6 / E018@14..19 / \
        E018@22..27"
    );
}

// ============================================================ 値の形

#[test]
fn cast_and_mode_values_are_read_as_refs() {
    // §5.3.2 `cast` = `'名前[:モード[+モード]]` の空白区切り
    assert_eq!(
        refs_tag_of("cast: 'アルス:A 'リリア:B+C", 0),
        "ref[アルス|mA@0..12] / ref[リリア|mB+C@13..27]"
    );
    // `pov` = `'名前`（修飾子なし）
    assert_eq!(refs_tag_of("pov: 'アルス", 0), "ref[アルス|m-@0..10]");
    // 修飾子が `mode_expr` の形なら各モード ID（§6.2.4）。判定は M3
    let parsed = plot_outline::parse("cast: 'アルス:A 'リリア:B+C");
    let field = parsed.field(0).expect("field line");
    let refs = field.refs();
    assert_eq!(refs[0].modes(), Some(vec!["A"]));
    assert_eq!(refs[1].modes(), Some(vec!["B", "C"]));
}

#[test]
fn a_full_width_space_also_separates_refs() {
    // 区切りの空白は 1 byte とは限らない（§5.0.5 は U+3000 も空白とする）
    assert_eq!(
        refs_tag_of("cast: 'アルス:A\u{3000}\u{3000}'リリア:B", 0),
        "ref[アルス|mA@0..12] / ref[リリア|mB@18..30]"
    );
}

#[test]
fn a_material_title_stays_a_modifier() {
    // §6.2.4「直前の参照が `Material` のとき → 資料タイトル」。M2 は形だけを読む
    assert_eq!(
        refs_tag_of("material: 'M0137:旧校舎の構造メモ", 0),
        "ref[M0137|m旧校舎の構造メモ@0..31]"
    );
    let parsed = plot_outline::parse("material: 'M0137:旧校舎の構造メモ");
    let field = parsed.field(0).expect("field line");
    // `旧校舎の構造メモ` は `mode_id` の形ではないのでモード式ではない
    assert_eq!(field.refs()[0].modes(), None);
}

#[test]
fn an_explicit_ref_form_is_recognized() {
    // §6.2.3 明示形 `'{...}`
    assert_eq!(
        refs_tag_of("cast: '{葵あおい}:A", 0),
        "ref[葵あおい*|mA@0..17]"
    );
    // 閉じ括弧が無ければ明示形にしない（診断は M3、§6.2.3 の E002）
    assert_eq!(
        refs_tag_of("cast: '{葵あおい", 0),
        "ref[{葵あおい|m-@0..14]"
    );
}

#[test]
fn shapes_that_are_not_refs_stay_out() {
    // `'` がトークンの先頭に無いもの、名前が空のものは参照ではない
    assert_eq!(
        refs_tag_of("cast: ' 'x 'アルス:", 0),
        "ref[x|m-@2..4] / ref[アルス|m@5..16]"
    );
    // 補足のように `'` の無い語は参照ではない（§5.3.2 `material: 'M0137 [補足]`）
    assert_eq!(
        refs_tag_of("material: 'M0137 補足", 0),
        "ref[M0137|m-@0..6]"
    );
}

#[test]
fn seed_and_payoff_split_at_the_arrow() {
    // §5.3.2 `seed: F<id> 説明 -> @範囲` / `payoff: F<id> -> @範囲`
    assert_eq!(
        foreshadow_tag_of("seed: F01 地下遺跡の封印紋様 -> @03-02", 0),
        "fs[F01|n地下遺跡の封印紋様|t@03-02]"
    );
    assert_eq!(
        foreshadow_tag_of("payoff: F02 -> @02-05", 0),
        "fs[F02|n|t@02-05]"
    );
    // `->` が無ければ `target` は空（形の検査は伏線を追跡する M3）
    assert_eq!(
        foreshadow_tag_of("seed: F03 説明だけ", 0),
        "fs[F03|n説明だけ|t]"
    );
    // 値が空なら形も無い
    assert_eq!(foreshadow_tag_of("seed:", 0), "-");
}

#[test]
fn a_length_that_is_not_a_number_is_kept_as_text() {
    // §7 に対応コードが無いので診断は出さず `None`（B-39）
    let parsed = plot_outline::parse("length: 5200\n");
    assert_eq!(parsed.field(0).and_then(|found| found.number()), Some(5200));
    let parsed = plot_outline::parse("length: 五二〇〇\n");
    assert_eq!(parsed.field(0).and_then(|found| found.number()), None);
    assert_eq!(diags_tag("length: 五二〇〇\n"), "");
}

// ============================================================ round-trip と壊れた入力

#[test]
fn round_trip_is_byte_exact() {
    // §5.3 完了条件 2。未知キー・継続行・空行・末尾改行の有無もすべて戻る
    let cases = [
        "",
        "\n",
        ";",
        "@01",
        "@01 x\nsummary: s\n;\n",
        "@01 x\nsummary: s\n;\n;\n",
        "foo: bar\n  継続行\n",
        "  字下げだけの行\n",
        "@abc x\n'\nsummary:\n;\n",
        SPEC_ALL,
        SPEC_CHAPTER,
    ];
    for text in cases {
        let parsed = plot_outline::parse(text);
        assert_eq!(&parsed.to_text(), text, "{text}");
    }
    // 継続行の字下げが残る例も（§5.3.6）
    let memo = space_memo();
    assert_eq!(&plot_outline::parse(&memo).to_text(), &memo);
}

#[test]
fn an_empty_file_has_one_blank_line_and_no_blocks() {
    assert_eq!(lines_tag(""), "blank[]");
    assert_eq!(blocks_tag(""), "");
    assert_eq!(preamble_tag(""), "P0..1|f-");
    assert_eq!(diags_tag(""), "");
}

#[test]
fn broken_input_does_not_panic() {
    // §5.3 完了条件 3。部分木と診断を返す
    let text = "@\n@01-\nsummary:\n'\n  \n;\n;\n";
    let parsed = plot_outline::parse(text);
    assert_eq!(parsed.lines.len(), 8);
    assert_eq!(diags_tag(text), "E002@0..1 / E002@2..6 / E018@0..1");
    // ID が壊れていてもブロックはできる。`summary:` は値が空でも `summary` として数える
    assert_eq!(
        blocks_tag(text),
        "B0|h0|id-|d0|0..1|e-|f-|n- / B1|h1|id-|d0|1..6|e5|f2|n3"
    );
    // 空白だけの行は継続行ではなく空行（B-34）
    assert_eq!(
        lines_tag(text),
        "header[|d0|id-|t|c] / header[01-|d0|id-|t|c] / field[summary|Summary|v] / \
        note['] / blank[  ] / break[@0|c] / break[@0|c] / blank[]"
    );
}
