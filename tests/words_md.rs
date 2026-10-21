//! `core::syntax::words_md`（`words.md`、§5.6）のテスト。
//!
//! `PLANS.md` §5.3 完了条件の 1（§5.6 の例が期待どおりにパースできる）・2（round-trip が
//! バイト一致）・3（壊れた入力でパニックしない）に対応する。

use novnow::core::syntax::words_md::{self, Line, Word};
use novnow::core::text::lexer::FieldKey;

/// §5.6 の例。**仕様のコード例をそのまま**使う（§5.3 完了条件 1）。
const SPEC_EXAMPLE: &str = r#"# 星屑の時計
reading: ほしくずのとけい
category: 魔導具
alias: 星屑時計
definition: 旧校舎の地下遺跡に眠るとされる幻の魔導具。古代錬金術の存在証明とされる。
range: @01 ..
note: 第 3 章で贋作であることが判明する。

# 旧校舎
reading: きゅうこうしゃ
category: 舞台
definition: 地下遺跡への入口がある立ち入り制限区域。
"#;

// ============================================================ タグ

fn word_tag(found: &Word<'_>) -> String {
    format!(
        "W[{}|@{}|{}..{}|f{}|free{}|r{}]",
        found.term,
        found.term_at,
        found.span.start,
        found.span.end,
        indices(&found.fields),
        indices(&found.free),
        match &found.range {
            Some(found) => found.text.to_owned(),
            None => "-".to_owned(),
        }
    )
}

fn indices(found: &[usize]) -> String {
    if found.is_empty() {
        return "-".to_owned();
    }
    let parts: Vec<String> = found.iter().map(usize::to_string).collect();
    parts.join(",")
}

fn words_tag(text: &str) -> String {
    let parts: Vec<String> = words_md::parse(text).words.iter().map(word_tag).collect();
    parts.join("\n")
}

fn diags_tag(text: &str) -> String {
    let parts: Vec<String> = words_md::parse(text)
        .diagnostics
        .iter()
        .map(|found| {
            let range = match &found.range {
                Some(found) => format!("{}..{}", found.start, found.end),
                None => "-".to_owned(),
            };
            let args: Vec<String> = found
                .args
                .iter()
                .map(|arg| match arg {
                    novnow::core::diagnostics::Arg::Text(value) => value.clone(),
                    novnow::core::diagnostics::Arg::Number(value) => value.to_string(),
                })
                .collect();
            if args.is_empty() {
                format!("{}@{}", found.code.as_str(), range)
            } else {
                format!("{}@{}({})", found.code.as_str(), range, args.join(","))
            }
        })
        .collect();
    parts.join("\n")
}

fn keys_tag(text: &str, index: usize) -> String {
    let parsed = words_md::parse(text);
    let found = &parsed.words[index];
    let parts: Vec<String> = found
        .fields
        .iter()
        .map(|&line| {
            let field = parsed.field(line).unwrap();
            field.key_text.to_owned()
        })
        .collect();
    parts.join(",")
}

fn line_tag_of(text: &str, index: usize) -> String {
    match &words_md::parse(text).lines[index] {
        Line::Heading(found) => format!("h{}[{}]", found.level, found.text),
        Line::Field(found) => format!("field[{}|v{}]", found.key_text, found.value),
        Line::Bullet(found) => format!("bullet[{}]", found.text),
        Line::Text(found) => format!("text[{found}]"),
        Line::Blank(found) => format!("blank[{found}]"),
        Line::Continuation(found) => format!("cont[{}]", found.text),
        Line::Comment(found) => format!("comment[{}]", found.comment.text),
        Line::Pattern(found) => format!("pattern[{}]", found.patterns.len()),
    }
}

/// `text` の `line` 行目の先頭の byte 位置。
fn byte_offset(text: &str, line: usize) -> usize {
    text.split('\n')
        .take(line)
        .map(|found| found.len() + 1)
        .sum()
}

// ============================================================ §5.6 の例（受け入れテスト）

#[test]
fn the_spec_example_reads_two_words() {
    assert_eq!(
        words_tag(SPEC_EXAMPLE),
        "W[星屑の時計|@2|0..8|f1,2,3,4,5,6|free-|r@01 ..]\nW[旧校舎|@2|8..13|f9,10,11|free-|r-]"
            .to_owned()
    );
}

#[test]
fn the_spec_example_reads_the_fields() {
    assert_eq!(
        keys_tag(SPEC_EXAMPLE, 0),
        "reading,category,alias,definition,range,note"
    );
    assert_eq!(keys_tag(SPEC_EXAMPLE, 1), "reading,category,definition");
    let parsed = words_md::parse(SPEC_EXAMPLE);
    let definition = parsed.definition(0).unwrap();
    assert_eq!(
        definition.value,
        "旧校舎の地下遺跡に眠るとされる幻の魔導具。古代錬金術の存在証明とされる。"
    );
    let alias = parsed.word_fields(0, FieldKey::Alias);
    assert_eq!(alias.len(), 1);
    assert_eq!(alias[0].value, "星屑時計");
}

#[test]
fn the_spec_example_has_no_diagnostics() {
    // §5.6 の例のキーはすべて §5.0.6 の表にある（reading / category / alias / definition /
    // range / note）ので警告は出ない
    assert_eq!(diags_tag(SPEC_EXAMPLE), "");
}

#[test]
fn the_spec_example_round_trips_byte_for_byte() {
    assert_eq!(words_md::parse(SPEC_EXAMPLE).to_text(), SPEC_EXAMPLE);
}

#[test]
fn the_spec_example_reads_the_range() {
    let parsed = words_md::parse(SPEC_EXAMPLE);
    let expr = parsed.range(0).unwrap();
    assert_eq!(expr.len(), 1);
    // 2 番目の用語は `range` を書かないので全範囲
    assert_eq!(parsed.range(1).unwrap().len(), 1);
    assert!(parsed.words[1].range.is_none());
}

// ============================================================ E021（definition 必須）

#[test]
fn a_missing_definition_reports_e021() {
    let text = "# 旧校舎\nreading: きゅうこうしゃ\n";
    let at = byte_offset(text, 0);
    // 位置は H1 の行全体（A 形式の E018 と同じ流儀）
    assert_eq!(
        diags_tag(text),
        format!("E021@{}..{}", at, at + "# 旧校舎".len())
    );
}

#[test]
fn an_empty_definition_counts_as_present() {
    // §5.6 は「`definition` 必須（欠落は E021）」。空の値は欠落ではない
    // （§7 の E021 の修正提案が「空の definition を挿入」なので、空は正規の状態）
    assert_eq!(diags_tag("# 旧校舎\ndefinition:\n"), "");
}

#[test]
fn a_definition_on_a_continuation_line_counts() {
    let text = "# 旧校舎\ndefinition: 一行目\n  二行目\n";
    assert_eq!(diags_tag(text), "");
    let parsed = words_md::parse(text);
    assert_eq!(parsed.definition(0).unwrap().full_value(), "一行目\n二行目");
}

#[test]
fn every_word_is_checked() {
    let text = "# 一つ\ndefinition: x\n# 二つ\n# 三つ\ndefinition: y\n";
    let at = byte_offset(text, 2);
    assert_eq!(
        diags_tag(text),
        format!("E021@{}..{}", at, at + "# 二つ".len())
    );
}

// ============================================================ range（§5.6 / §5.1）

#[test]
fn a_reversed_range_reports_e016() {
    let text = "# 用語\ndefinition: x\nrange: @03 .. @01\n";
    let parsed = words_md::parse(text);
    let found = parsed.words[0].range.as_ref().unwrap();
    assert!(!found.valid);
    let at = byte_offset(text, 2) + found.at;
    assert_eq!(
        diags_tag(text),
        format!("E016@{}..{}", at, at + "@03 .. @01".len())
    );
}

#[test]
fn an_unparsable_range_reports_e002() {
    let text = "# 用語\ndefinition: x\nrange: 01 ..\n";
    let parsed = words_md::parse(text);
    let found = parsed.words[0].range.as_ref().unwrap();
    assert!(!found.valid);
    assert!(found.expr().is_none());
    assert!(diags_tag(text).starts_with("E002@"));
}

#[test]
fn a_range_may_list_several_intervals() {
    let text = "# 用語\ndefinition: x\nrange: @01 .. @02, @05\n";
    let parsed = words_md::parse(text);
    assert_eq!(parsed.range(0).unwrap().len(), 2);
    assert_eq!(diags_tag(text), "");
}

// ============================================================ 未知キーと MD 方言

#[test]
fn unknown_keys_warn_and_keep_their_values() {
    // §5.0.6 の表に無いキーは W011 ＋ 値を改変せずに保持（縫い代 2）。② のキー（`icoz`）も
    // ① では未知のまま（PLANS.md §1.2）。`color` は第 2.6 版で表に入ったので W011 にならない
    let text = "# 用語\ndefinition: x\nicoz: star\ncolor: #ff0000\n";
    let at = byte_offset(text, 2);
    assert_eq!(diags_tag(text), format!("W011@{}..{}(icoz)", at, at + 4));
    let parsed = words_md::parse(text);
    let found = parsed.unknown_field(0, "icoz");
    assert_eq!(found.map(|field| field.value), Some("star"));
    // 表にあるキーは `unknown_field`（書かれたままのキーで引く）からも引ける
    assert_eq!(
        parsed.unknown_field(0, "color").map(|field| field.key),
        Some(FieldKey::Color)
    );
    assert_eq!(parsed.to_text(), text);
}

#[test]
fn a_second_level_heading_does_not_close_the_word() {
    // §5.6 に H2 は無いので、`## ` は行として残るだけでフィールドは同じ用語に属する
    let text = "# 用語\n## notes\nnote: メモ\ndefinition: x\n";
    assert_eq!(line_tag_of(text, 1), "h2[notes]");
    assert_eq!(keys_tag(text, 0), "note,definition");
    assert_eq!(words_md::parse(text).words.len(), 1);
}

#[test]
fn bullets_and_patterns_are_kept_as_lines_only() {
    // §5.6 の木に箇条書きとパターン行は無い（行は保持されるので round-trip は保たれる）
    let text = "# 用語\ndefinition: x\n- 項目\n「*だよ」\n";
    assert_eq!(line_tag_of(text, 2), "bullet[項目]");
    assert_eq!(line_tag_of(text, 3), "pattern[1]");
    assert_eq!(words_tag(text), "W[用語|@2|0..5|f1|free-|r-]");
    assert_eq!(words_md::parse(text).to_text(), text);
}

#[test]
fn free_text_goes_to_the_word() {
    let text = "# 用語\ndefinition: x\n補足は自由記述。\n";
    assert_eq!(line_tag_of(text, 2), "text[補足は自由記述。]");
    assert_eq!(words_md::parse(text).words[0].free, vec![2]);
}

#[test]
fn lines_before_the_first_h1_are_the_preamble() {
    let text = "// 用語集\nnote: 全体メモ\n# 用語\ndefinition: x\n";
    let parsed = words_md::parse(text);
    assert_eq!(parsed.preamble.span, 0..2);
    assert_eq!(parsed.preamble.fields, vec![1]);
    assert_eq!(parsed.words[0].span, 2..5);
}

#[test]
fn duplicate_terms_are_not_reported() {
    // 用語名の重複に対応するコードは §7 に無い（辞書は M3）
    let text = "# 用語\ndefinition: x\n# 用語\ndefinition: y\n";
    assert_eq!(diags_tag(text), "");
    assert_eq!(words_md::parse(text).words.len(), 2);
}

// ============================================================ 壊れた入力（完了条件 3）

#[test]
fn broken_input_does_not_panic() {
    let cases = [
        "",
        "\n",
        "#",
        "# \n",
        "# 用語\n",
        "definition: x\n",
        "range: @\n",
        "range:\n",
        "# 用語\ndefinition: x\nrange: @01 ..\nrange: @02 ..\n",
        "## notes\n# 用語\n",
    ];
    for text in cases {
        let parsed = words_md::parse(text);
        assert_eq!(parsed.to_text(), text, "round-trip: {text:?}");
        let _ = words_tag(text);
        let _ = diags_tag(text);
    }
}

#[test]
fn a_word_without_a_heading_is_not_created() {
    let parsed = words_md::parse("definition: x\nrange: @01 ..\n");
    assert!(parsed.words.is_empty());
    // H1 が無いので前文は全行（末尾改行のあとの空行を含む）
    assert_eq!(parsed.preamble.span, 0..parsed.lines.len());
    assert_eq!(parsed.preamble.span, 0..3);
    assert_eq!(parsed.preamble.fields, vec![0, 1]);
}

#[test]
fn an_empty_file_has_no_words() {
    let parsed = words_md::parse("");
    assert!(parsed.words.is_empty());
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.lines.len(), 1);
}
