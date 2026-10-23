//! `core::syntax::ideas_md`（`ideas.md`、§5.7）のテスト。
//!
//! `docs/history.md` の完了条件の 1（§5.7 の例が期待どおりにパースできる）・2（round-trip が
//! バイト一致）・3（壊れた入力でパニックしない）に対応する。

use novnow::core::syntax::ideas_md::{self, Idea, Line};
use novnow::core::text::lexer::FieldKey;

/// §5.7 の例。**仕様のコード例をそのまま**使う（§5.3 完了条件 1）。
const SPEC_EXAMPLE: &str = r#"# I0007 学院長の動機
status: inbox            # inbox | keep | promoted | dropped
created: 2026-10-09T12:04:11+09:00
tags: 学院長, 動機
relates: @02, 'アルス
promoted-to: plots/02.txt#@02-03      # 昇格時のみ
本文は自由記述。改行もそのまま保持する。
"#;

// ============================================================ タグ

fn idea_tag(found: &Idea<'_>) -> String {
    format!(
        "I[{}|n{}|ok{}|{}|{}..{}|f{}|free{}]",
        found.id,
        match found.number {
            Some(found) => found.to_string(),
            None => "-".to_owned(),
        },
        found.valid_id,
        found.title,
        found.span.start,
        found.span.end,
        indices(&found.fields),
        indices(&found.free)
    )
}

fn indices(found: &[usize]) -> String {
    if found.is_empty() {
        return "-".to_owned();
    }
    let parts: Vec<String> = found.iter().map(usize::to_string).collect();
    parts.join(",")
}

fn ideas_tag(text: &str) -> String {
    let parts: Vec<String> = ideas_md::parse(text).ideas.iter().map(idea_tag).collect();
    parts.join("\n")
}

fn diags_tag(text: &str) -> String {
    let parts: Vec<String> = ideas_md::parse(text)
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
    let parsed = ideas_md::parse(text);
    let parts: Vec<String> = parsed.ideas[index]
        .fields
        .iter()
        .map(|&line| parsed.field(line).unwrap().key_text.to_owned())
        .collect();
    parts.join(",")
}

/// `text` の `line` 行目の先頭の byte 位置。
fn byte_offset(text: &str, line: usize) -> usize {
    text.split('\n')
        .take(line)
        .map(|found| found.len() + 1)
        .sum()
}

/// `line` 行目の未知キーの W011 の期待文字列。
fn w011(text: &str, line: usize, key: &str) -> String {
    let at = byte_offset(text, line);
    format!("W011@{}..{}({})", at, at + key.len(), key)
}

// ============================================================ §5.7 の例（受け入れテスト）

#[test]
fn the_spec_example_reads_the_idea() {
    assert_eq!(
        ideas_tag(SPEC_EXAMPLE),
        "I[I0007|n7|oktrue|学院長の動機|0..8|f1,2,3,4,5|free6]"
    );
}

#[test]
fn the_spec_example_reads_the_keys_of_the_table() {
    assert_eq!(
        keys_tag(SPEC_EXAMPLE, 0),
        "status,created,tags,relates,promoted-to"
    );
    // §5.7 の 5 キーは**すべて** §5.0.6 の表にある（第 2.6 版。A-15 の裁定＝正式追加）
    let parsed = ideas_md::parse(SPEC_EXAMPLE);
    let keys: Vec<FieldKey> = parsed.ideas[0]
        .fields
        .iter()
        .map(|&line| parsed.field(line).unwrap().key)
        .collect();
    assert_eq!(
        keys,
        vec![
            FieldKey::Status,
            FieldKey::Created,
            FieldKey::Tags,
            FieldKey::Relates,
            FieldKey::PromotedTo,
        ]
    );
    let tags = parsed.idea_fields(0, FieldKey::Tags);
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].value, "学院長, 動機");
}

#[test]
fn keys_outside_the_table_still_warn_and_keep_their_values() {
    // §5.0.6 の表に無いキーは W011 ＋ 値を改変せず保持（縫い代 2）。② のキー（`flurb`）も
    // ① では未知のまま（`docs/plans.md` §4.2）
    let text = "# I0001 下書き\nstatus: inbox\nflurb: 一行紹介\nfoo: bar\n";
    assert_eq!(
        diags_tag(text),
        [w011(text, 2, "flurb"), w011(text, 3, "foo")].join("\n")
    );
    let parsed = ideas_md::parse(text);
    assert_eq!(parsed.status(0).unwrap().key, FieldKey::Status);
    assert_eq!(parsed.unknown_field(0, "flurb").unwrap().value, "一行紹介");
    assert_eq!(parsed.to_text(), text);
}

#[test]
fn the_spec_example_reads_the_meta_keys_from_the_table() {
    // §5.7 が定義する `status` `created` `relates` `promoted-to` は §5.0.6 の表に**正式追加**
    // された（第 2.6 版。`docs/history.md` A-15 の裁定＝すべて正式追加）ので W011 は出ない
    assert_eq!(diags_tag(SPEC_EXAMPLE), "");
}

#[test]
fn the_spec_example_keeps_the_values_of_the_meta_keys() {
    let parsed = ideas_md::parse(SPEC_EXAMPLE);
    // §5.0.8 の `# ` 行コメントは ② なので、① では値の一部としてそのまま残る（B-36 と同じ）
    assert_eq!(
        parsed.status(0).unwrap().value,
        "inbox            # inbox | keep | promoted | dropped"
    );
    assert_eq!(
        parsed.unknown_field(0, "created").unwrap().value,
        "2026-10-09T12:04:11+09:00"
    );
    assert_eq!(
        parsed.unknown_field(0, "relates").unwrap().value,
        "@02, 'アルス"
    );
    assert_eq!(
        parsed.promoted_to(0).unwrap().value,
        "plots/02.txt#@02-03      # 昇格時のみ"
    );
}

#[test]
fn the_spec_example_keeps_the_body_as_free_text() {
    let parsed = ideas_md::parse(SPEC_EXAMPLE);
    assert_eq!(parsed.body(0), "本文は自由記述。改行もそのまま保持する。");
}

#[test]
fn the_spec_example_round_trips_byte_for_byte() {
    assert_eq!(ideas_md::parse(SPEC_EXAMPLE).to_text(), SPEC_EXAMPLE);
}

// ============================================================ ID（§5.7）

#[test]
fn a_duplicate_id_reports_e008() {
    // §5.7「ID は ideas.md 内で最大値 +1 を採番（MUST、再利用禁止）」
    let text = "# I0007 一つ\n# I0008 二つ\n# I0007 三つ\n";
    let at = byte_offset(text, 2) + 2;
    assert_eq!(diags_tag(text), format!("E008@{}..{}(I0007)", at, at + 5));
}

#[test]
fn every_reuse_is_reported() {
    let text = "# I0001 a\n# I0001 b\n# I0001 c\n";
    assert_eq!(diags_tag(text).lines().count(), 2);
}

#[test]
fn the_same_number_with_a_different_spelling_is_not_a_duplicate() {
    // E008 は**書かれた ID**の重複。`I0007` と `I007` は別の文字列（どちらも §5.7 の形では
    // ないので `valid_id` は片方だけ）
    let text = "# I0007 a\n# I007 b\n";
    assert_eq!(diags_tag(text), "");
    let parsed = ideas_md::parse(text);
    assert!(parsed.ideas[0].valid_id);
    assert!(!parsed.ideas[1].valid_id);
    assert_eq!(parsed.ideas[1].number, None);
}

#[test]
fn an_invalid_id_is_flagged_without_a_diagnostic() {
    // §7 に「アイデア ID の形」に対応するコードが無いので診断しない（B-39 の流儀）
    let text = "# 7 動機\n# X0007 動機\n# I007 動機\n# I0007x 動機\n# I10000 動機\n";
    assert_eq!(diags_tag(text), "");
    let parsed = ideas_md::parse(text);
    let flags: Vec<bool> = parsed.ideas.iter().map(|idea| idea.valid_id).collect();
    assert_eq!(flags, vec![false, false, false, false, true]);
    // 5 桁は「4 桁以上」なので受理する（採番が続けば 9999 を超える）
    assert_eq!(parsed.ideas[4].id, "I10000");
    assert_eq!(parsed.ideas[4].number, Some(10000));
    // ゼロ詰めが違うだけの `I0007` と `I00007` は**別の ID 文字列**（E008 は文字列で見る）
    let parsed = ideas_md::parse("# I0007 a\n# I00007 b\n");
    assert_eq!(diags_tag("# I0007 a\n# I00007 b\n"), "");
    assert_eq!(parsed.ideas[1].number, Some(7));
}

#[test]
fn an_idea_without_a_title_keeps_an_empty_one() {
    let parsed = ideas_md::parse("# I0007\n");
    assert_eq!(parsed.ideas[0].title, "");
    assert_eq!(parsed.ideas[0].id, "I0007");
    assert!(parsed.ideas[0].valid_id);
}

#[test]
fn the_title_keeps_inner_spaces() {
    // 見出しは最初の空白で 1 度だけ切る（§5.7「I + 4 桁 ID + 空白 + タイトル」）
    let parsed = ideas_md::parse("# I0007 学院長 の 動機\n");
    assert_eq!(parsed.ideas[0].title, "学院長 の 動機");
}

// ============================================================ MD 方言

#[test]
fn a_second_level_heading_does_not_close_the_idea() {
    // §5.7 に H2 は無いので、`## ` は行として残るだけ
    let text = "# I0007 タイトル\n## notes\nnote: メモ\n";
    let parsed = ideas_md::parse(text);
    assert_eq!(parsed.ideas.len(), 1);
    assert_eq!(keys_tag(text, 0), "note");
    assert!(matches!(parsed.lines[1], Line::Heading(_)));
}

#[test]
fn a_continuation_line_joins_the_field_value() {
    let text = "# I0007 タイトル\nstatus: inbox\n  keep にする予定\n";
    let parsed = ideas_md::parse(text);
    assert_eq!(
        parsed.status(0).unwrap().full_value(),
        "inbox\nkeep にする予定"
    );
    assert_eq!(parsed.ideas[0].free.len(), 0);
}

#[test]
fn lines_before_the_first_h1_are_the_preamble() {
    let text = "// アイデア箱\ntags: 全体\n# I0007 タイトル\n";
    let parsed = ideas_md::parse(text);
    assert_eq!(parsed.preamble.span, 0..2);
    assert_eq!(parsed.preamble.fields, vec![1]);
    assert_eq!(parsed.ideas[0].span, 2..4);
}

#[test]
fn several_ideas_are_read_in_order() {
    let text = "# I0001 一つ\nstatus: inbox\n# I0002 二つ\nstatus: keep\n本文\n";
    assert_eq!(
        ideas_tag(text),
        "I[I0001|n1|oktrue|一つ|0..2|f1|free-]\nI[I0002|n2|oktrue|二つ|2..6|f3|free4]"
    );
    let parsed = ideas_md::parse(text);
    assert_eq!(parsed.body(1), "本文");
}

// ============================================================ 壊れた入力（完了条件 3）

#[test]
fn broken_input_does_not_panic() {
    let cases = [
        "",
        "\n",
        "#",
        "# \n",
        "# I\n",
        "# I0007\n",
        "status: inbox\n",
        "# I0007 a\n# I0007\n",
        "## notes\n# I0007 a\n",
        "# I0007 a\n  \n    x\n",
    ];
    for text in cases {
        let parsed = ideas_md::parse(text);
        assert_eq!(parsed.to_text(), text, "round-trip: {text:?}");
        let _ = ideas_tag(text);
        let _ = diags_tag(text);
        let _ = parsed.body(0);
        let _ = parsed.status(0);
    }
}

#[test]
fn an_empty_file_has_no_ideas() {
    let parsed = ideas_md::parse("");
    assert!(parsed.ideas.is_empty());
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.body(0), "");
    assert!(parsed.status(0).is_none());
}
