//! `core::syntax::prompt_md`（`prompts/*.md`、§5.9.1 / §5.9.2）のテスト。
//!
//! `docs/history.md` の完了条件の 1（§5.9.1 の例が期待どおりにパースできる）・2（round-trip が
//! バイト一致）・3（壊れた入力でパニックしない）に対応する。

use novnow::core::syntax::prompt_md::{
    self, is_role_name, Line, Options, Prompt, PromptSectionKey, BUILTIN_ROLES, VARIABLES,
};
use novnow::core::text::lexer::FieldKey;

/// §5.9.1 の例（`prompts/writer.md`）。**仕様のコード例をそのまま**使う（完了条件 1）。
const SPEC_EXAMPLE: &str = r#"# writer
model-hint: balanced          # fast | balanced | careful（MCP サーバ選択のヒント）
max-chars: 12000              # ContextPacket の上限（§15.4）
temperature: 0.7

## system
あなたは novnow プロジェクトの本文執筆担当です。…

## task
次の節の本文だけを書いてください。
対象: {{scene_id}}
プロット: {{plot_section}}

## output
- 出力は本文のみ。見出し・解説・前置きを書かない。
- 役割語は {{role_language}} に従う。
"#;

/// §5.9.2 の例（`prompts/custom/weapon-advisor.md`）。
const SPEC_CUSTOM: &str = r#"# weapon-advisor
label: 武器考証                     # UI 表示名（省略時はファイル名）
model-hint: careful
max-chars: 8000
context: project outline target materials constraints    # 送るブロックを選ぶ（§15.4）

## system
あなたは近世ヨーロッパの武具に詳しい考証担当です。

## task
次の節の武器・防具の描写に、時代考証上の誤りがないか指摘してください。
対象: {{scene_id}}
本文: {{text_current}}

## output
- 指摘は箇条書き。各項目に「該当箇所の引用」「問題」「代案」を書く。
- 問題がなければ「問題なし」とだけ書く。
"#;

fn custom() -> Options {
    Options { custom: true }
}

// ============================================================ タグ

fn prompt_tag(parsed: &Prompt<'_>) -> String {
    let role = match &parsed.role {
        Some(found) => format!(
            "{}|@{}|ok{}|builtin{}",
            found.name, found.name_at, found.valid, found.builtin
        ),
        None => "-".to_owned(),
    };
    let sections: Vec<String> = parsed
        .sections
        .iter()
        .map(|found| {
            format!(
                "{:?}[{}|{}..{}|f{}|b{}|free{}]",
                found.key,
                found.key_text,
                found.span.start,
                found.span.end,
                indices(&found.fields),
                indices(&found.bullets),
                indices(&found.free)
            )
        })
        .collect();
    format!(
        "R[{}]\nf{}\n{}",
        role,
        indices(&parsed.fields),
        sections.join("\n")
    )
}

fn variables_tag(parsed: &Prompt<'_>) -> String {
    let parts: Vec<String> = parsed
        .variables
        .iter()
        .map(|found| {
            format!(
                "{}|l{}|@{}|ok{}",
                found.name, found.line, found.at, found.known
            )
        })
        .collect();
    parts.join("\n")
}

fn indices(found: &[usize]) -> String {
    if found.is_empty() {
        return "-".to_owned();
    }
    let parts: Vec<String> = found.iter().map(usize::to_string).collect();
    parts.join(",")
}

fn diags_tag(text: &str) -> String {
    diags_of(&prompt_md::parse(text))
}

fn diags_of(parsed: &Prompt<'_>) -> String {
    let parts: Vec<String> = parsed
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

// ============================================================ §5.9.1 の例（受け入れテスト）

#[test]
fn the_spec_example_reads_the_role_and_the_sections() {
    let parsed = prompt_md::parse(SPEC_EXAMPLE);
    assert_eq!(
        prompt_tag(&parsed),
        "R[writer|@2|oktrue|builtintrue]\nf1,2,3\n\
         System[system|5..8|f-|b-|free6]\n\
         Task[task|8..13|f-|b-|free9,10,11]\n\
         Output[output|13..17|f-|b14,15|free-]"
    );
    assert_eq!(parsed.known_sections().len(), 3);
}

#[test]
fn the_spec_example_reads_the_meta_fields() {
    let parsed = prompt_md::parse(SPEC_EXAMPLE);
    let keys: Vec<&str> = parsed
        .fields
        .iter()
        .map(|&line| parsed.field(line).unwrap().key_text)
        .collect();
    assert_eq!(keys, vec!["model-hint", "max-chars", "temperature"]);
    // §5.0.8 の `# ` 行コメントは ② なので、① では値の一部としてそのまま残る（B-36 と同じ）
    assert!(parsed
        .meta_field("model-hint")
        .unwrap()
        .value
        .starts_with("balanced          # fast"));
    assert!(parsed
        .meta_field("max-chars")
        .unwrap()
        .value
        .starts_with("12000"));
    assert_eq!(parsed.meta_field("temperature").unwrap().value, "0.7");
}

#[test]
fn the_spec_example_reads_the_meta_keys_from_the_table() {
    // §5.9.1 が定義する `model-hint` / `max-chars` / `temperature` は §5.0.6 の表に**正式追加**
    // された（第 2.6 版。`docs/history.md` A-15 の裁定＝すべて正式追加）ので W011 は出ない
    assert_eq!(diags_tag(SPEC_EXAMPLE), "");
    let parsed = prompt_md::parse(SPEC_EXAMPLE);
    for (key, alias) in [
        (FieldKey::ModelHint, "モデル指定"),
        (FieldKey::MaxChars, "最大文字数"),
        (FieldKey::Temperature, "温度"),
    ] {
        assert_eq!(parsed.meta_fields(key).len(), 1, "{key:?}");
        assert_eq!(
            parsed.meta_fields(key)[0].key.alias(),
            Some(alias),
            "{key:?}"
        );
    }
}

#[test]
fn the_spec_example_collects_only_known_variables() {
    let parsed = prompt_md::parse(SPEC_EXAMPLE);
    assert_eq!(
        variables_tag(&parsed),
        // [`Variable::at`] は**行内相対**（`{{` の直後）
        format!(
            "scene_id|l10|@{}|oktrue\nplot_section|l11|@{}|oktrue\nrole_language|l15|@{}|oktrue",
            "対象: ".len() + 2,
            "プロット: ".len() + 2,
            "- 役割語は ".len() + 2,
        )
    );
    assert!(parsed.unknown_variables().is_empty());
}

#[test]
fn the_spec_example_body_of_a_section_is_the_raw_text() {
    let parsed = prompt_md::parse(SPEC_EXAMPLE);
    assert_eq!(
        parsed.body_of(0),
        "あなたは novnow プロジェクトの本文執筆担当です。…\n"
    );
    assert_eq!(
        parsed.body_of(1),
        "次の節の本文だけを書いてください。\n対象: {{scene_id}}\nプロット: {{plot_section}}\n"
    );
    assert_eq!(
        parsed.body_of(2),
        "- 出力は本文のみ。見出し・解説・前置きを書かない。\n- 役割語は {{role_language}} に従う。\n"
    );
    assert_eq!(parsed.body_of(9), "");
}

#[test]
fn the_spec_example_round_trips_byte_for_byte() {
    assert_eq!(prompt_md::parse(SPEC_EXAMPLE).to_text(), SPEC_EXAMPLE);
}

#[test]
fn the_spec_example_does_not_check_the_role_name() {
    // 既定の `prompts/<role>.md` は §5.9.2 の名前規則を検査しない（`Options::custom` が false）
    let text = "# Writer\n## system\n";
    assert_eq!(diags_tag(text), "");
    let parsed = prompt_md::parse_with(text, &custom());
    assert_eq!(diags_of(&parsed), "E032@2..8(Writer)");
}

// ============================================================ §5.9.2 の例（カスタムロール）

#[test]
fn the_custom_role_example_has_a_valid_name() {
    let parsed = prompt_md::parse_with(SPEC_CUSTOM, &custom());
    let role = parsed.role.as_ref().unwrap();
    assert_eq!(role.name, "weapon-advisor");
    assert!(role.valid);
    assert!(!role.builtin);
    // `context` は §5.0.6 の表にある（prompts）ので警告にならない
    let context = parsed.meta_fields(FieldKey::Context);
    assert_eq!(context.len(), 1);
    assert_eq!(
        context[0].value,
        "project outline target materials constraints    # 送るブロックを選ぶ（§15.4）"
    );
}

#[test]
fn the_custom_role_example_reads_the_meta_keys_from_the_table() {
    // §5.9.2 の `label` も表に**正式追加**された（第 2.6 版。A-15）ので、例の診断は 0 件
    let parsed = prompt_md::parse_with(SPEC_CUSTOM, &custom());
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let found = parsed.meta_fields(FieldKey::Label);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].key_text, "label");
    // §5.0.8 の `# ` 行コメントは ② なので、① では値の一部としてそのまま残る（B-36）
    assert!(
        found[0].value.starts_with("武器考証"),
        "{:?}",
        found[0].value
    );
    // `context` は表に元からあるキー（§5.9.2）
    assert_eq!(parsed.meta_fields(FieldKey::Context).len(), 1);
}

#[test]
fn the_custom_role_example_uses_only_variables_of_the_closed_set() {
    // §5.9.2 の例は第 2.6 版で `{{content_section}}` → `{{text_current}}` に直った
    // （§15.5.3 の当時の閉じた 19 変数が正本。`docs/history.md` A-16 の裁定＝サンプルの記述ミス）
    let parsed = prompt_md::parse_with(SPEC_CUSTOM, &custom());
    assert!(
        parsed.unknown_variables().is_empty(),
        "{:?}",
        parsed.unknown_variables()
    );
    let names: Vec<&str> = parsed.variables.iter().map(|found| found.name).collect();
    assert_eq!(names, vec!["scene_id", "text_current"]);
    assert!(names.iter().all(|name| VARIABLES.contains(name)));
}

#[test]
fn the_custom_role_example_round_trips() {
    let parsed = prompt_md::parse_with(SPEC_CUSTOM, &custom());
    assert_eq!(parsed.to_text(), SPEC_CUSTOM);
}

// ============================================================ 変数（§15.5.3）

#[test]
fn every_variable_of_the_closed_set_is_known() {
    assert_eq!(VARIABLES.len(), 20);
    for name in VARIABLES {
        let text = format!("# writer\n## task\n前 {{{{{name}}}}} 後\n");
        let parsed = prompt_md::parse(&text);
        assert_eq!(parsed.variables.len(), 1, "{name}");
        assert!(parsed.variables[0].known, "{name}");
        assert!(parsed.unknown_variables().is_empty(), "{name}");
    }
}

#[test]
fn an_unknown_variable_reports_e031() {
    let text = "# writer\n## task\n対象: {{scene}}\n";
    let at = byte_offset(text, 2) + "対象: ".len() + 2;
    assert_eq!(
        diags_tag(text),
        format!("E031@{}..{}(scene)", at, at + "scene".len())
    );
    // 第 2.6 版までの §5.9.2 の例が書いていた `content_section` も §15.5.3 の集合に無い
    // （本文は `text_current`）。例は直したが**検査はここに残す**（A-16）
    let old = "# writer\n## task\n本文: {{content_section}}\n";
    let at = byte_offset(old, 2) + "本文: ".len() + 2;
    assert_eq!(
        diags_tag(old),
        format!(
            "E031@{}..{}(content_section)",
            at,
            at + "content_section".len()
        )
    );
}

#[test]
fn variables_are_read_in_fields_headings_and_bullets() {
    // 変数はどこに書いても拾う（LLM に渡る部分だから）
    let text = "# {{role}}\nmax-chars: {{n}}\n## {{sec}}\n- {{item}}\n";
    let parsed = prompt_md::parse(text);
    let names: Vec<&str> = parsed.variables.iter().map(|found| found.name).collect();
    assert_eq!(names, vec!["role", "n", "sec", "item"]);
    assert_eq!(parsed.unknown_variables().len(), 4);
}

#[test]
fn a_variable_in_a_line_comment_is_ignored() {
    // `// …` は LLM に渡らないので走査しない（§5.0.7）
    let text = "# writer\n## task\n本文 // {{unknown}}\n// {{another}}\n";
    assert_eq!(diags_tag(text), "");
    assert!(prompt_md::parse(text).variables.is_empty());
}

#[test]
fn a_variable_after_a_stage_two_hash_comment_is_still_read() {
    // §5.0.8 の `# ` は ② なので ① ではコメントではなく、中身の変数は走る
    let text = "# writer\n## task\n本文 # {{unknown}}\n";
    let parsed = prompt_md::parse(text);
    assert_eq!(parsed.variables.len(), 1);
    assert_eq!(parsed.variables[0].name, "unknown");
}

#[test]
fn an_unclosed_brace_is_not_a_variable() {
    // 閉じない `{{` は変数ではない（診断も出さない。そのまま雛形に残る）
    let text = "# writer\n## task\n{{scene_id\n{{}}\n";
    let parsed = prompt_md::parse(text);
    assert_eq!(parsed.variables.len(), 1);
    assert_eq!(parsed.variables[0].name, "");
    assert_eq!(parsed.to_text(), text);
}

#[test]
fn two_variables_on_one_line_are_both_read() {
    let text = "# writer\n## task\n{{scene_id}} と {{cast}}\n";
    let parsed = prompt_md::parse(text);
    let names: Vec<&str> = parsed.variables.iter().map(|found| found.name).collect();
    assert_eq!(names, vec!["scene_id", "cast"]);
    assert!(parsed.variables.iter().all(|found| found.known));
}

// ============================================================ 役割名（§5.9.2）

#[test]
fn role_names_follow_the_documented_shape() {
    let longest = "a".repeat(32);
    let too_long = "a".repeat(33);
    let valid = ["writer", "w", "weapon-advisor", "role2", longest.as_str()];
    let invalid = [
        "",
        "Writer",
        "2roles",
        "-role",
        "role name",
        "ロール",
        too_long.as_str(),
    ];
    for name in valid {
        assert!(is_role_name(name), "{name}");
    }
    for name in invalid {
        assert!(!is_role_name(name), "{name}");
    }
}

#[test]
fn a_custom_role_colliding_with_a_builtin_reports_e033() {
    for name in BUILTIN_ROLES {
        let text = format!("# {name}\n## system\n");
        let parsed = prompt_md::parse_with(&text, &custom());
        // §5.9.2「既定 7 役割と同名は E033」。名前の形は正しいので E032 は出ない
        assert_eq!(
            diags_of(&parsed),
            format!("E033@2..{}({name})", 2 + name.len())
        );
    }
    assert_eq!(BUILTIN_ROLES.len(), 7);
}

#[test]
fn a_builtin_prompt_may_use_a_builtin_name() {
    // `prompts/writer.md` は E033 にならない（上書きは §5.9.2 の表どおりこちら側で行う）
    let parsed = prompt_md::parse("# writer\n## system\n");
    assert!(parsed.role.as_ref().unwrap().builtin);
    assert!(parsed.diagnostics.is_empty());
}

#[test]
fn a_custom_role_with_an_invalid_name_reports_e032() {
    let parsed = prompt_md::parse_with("# 武器考証\n## system\n", &custom());
    assert_eq!(diags_of(&parsed).lines().count(), 1);
    assert!(diags_of(&parsed).starts_with("E032@"));
}

// ============================================================ セクション（§5.9.1）

#[test]
fn the_five_section_keys_are_recognized() {
    let text = "# writer\n## system\n## task\n## output\n## examples\n## refuse\n";
    let parsed = prompt_md::parse(text);
    let keys: Vec<PromptSectionKey> = parsed.sections.iter().map(|found| found.key).collect();
    assert_eq!(
        keys,
        vec![
            PromptSectionKey::System,
            PromptSectionKey::Task,
            PromptSectionKey::Output,
            PromptSectionKey::Examples,
            PromptSectionKey::Refuse
        ]
    );
}

#[test]
fn an_unknown_section_is_kept_as_other() {
    // §7 に対応コードが無いので診断しない（行も中身も保持される）
    let text = "# writer\n## guard\n本文\n## system\n";
    let parsed = prompt_md::parse(text);
    assert_eq!(parsed.sections[0].key, PromptSectionKey::Other);
    assert_eq!(parsed.sections[0].key_text, "guard");
    assert_eq!(parsed.sections[0].span, 1..3);
    assert_eq!(parsed.body_of(0), "本文");
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.known_sections(), vec![PromptSectionKey::System]);
}

#[test]
fn a_third_level_heading_does_not_close_a_section() {
    // §5.9.1 に H3 は無い
    let text = "# writer\n## system\n### 深い\n本文\n";
    let parsed = prompt_md::parse(text);
    assert_eq!(parsed.sections.len(), 1);
    assert_eq!(parsed.sections[0].span, 1..5);
    assert_eq!(parsed.body_of(0), "### 深い\n本文\n");
}

#[test]
fn fields_inside_a_section_belong_to_the_section() {
    // §5.0.5 は形式を問わないので、雛形の中に `note: …` を書けばフィールド行になる。
    // 中身は `body_of` でそのまま返る
    let text = "# writer\n## task\nnote: メモ\n## output\n";
    let parsed = prompt_md::parse(text);
    assert_eq!(parsed.sections[0].fields, vec![2]);
    assert!(parsed.fields.is_empty());
    // `body_of` はセクションの行を LF で連結する（末尾の LF は次の見出しの行なので含まない）
    assert_eq!(parsed.body_of(0), "note: メモ");
}

#[test]
fn lines_before_the_first_section_are_meta_fields() {
    let text = "# writer\nmodel-hint: fast\n自由記述\n## system\n";
    let parsed = prompt_md::parse(text);
    assert_eq!(parsed.fields, vec![1]);
    // §5.9.1 に「H1 直下の自由記述」は無いので木に載せない（行は保持される）
    assert_eq!(parsed.sections[0].span, 3..5);
    assert_eq!(parsed.to_text(), text);
}

#[test]
fn a_file_without_a_heading_has_no_role() {
    let parsed = prompt_md::parse("## system\n本文\n");
    assert!(parsed.role.is_none());
    assert_eq!(parsed.sections.len(), 1);
    assert_eq!(diags_of(&parsed), "");
    let parsed = prompt_md::parse_with("## system\n本文\n", &custom());
    assert!(parsed.role.is_none());
    assert!(parsed.diagnostics.is_empty());
}

#[test]
fn a_second_heading_is_not_a_second_role() {
    // 1 ファイル 1 役割（§5.9.1）。2 つ目の H1 は行として残るだけ
    let text = "# writer\n## system\n# plotter\n## task\n";
    let parsed = prompt_md::parse(text);
    assert_eq!(parsed.role.as_ref().unwrap().name, "writer");
    // `# plotter` はセクションを閉じるが、役割としては 1 つ目だけを読む
    assert_eq!(parsed.sections.len(), 2);
    assert_eq!(parsed.sections[0].span, 1..2);
    assert_eq!(parsed.sections[1].span, 3..5);
    assert!(matches!(parsed.lines[2], Line::Heading(_)));
}

// ============================================================ 未知キー（§5.0.6）

#[test]
fn keys_outside_the_table_still_warn_and_keep_their_values() {
    // §5.0.6 の表に無いキーは W011 ＋ 値を改変せず保持（縫い代 2）。② のキー（`flurb`）も
    // ① では未知のまま（`docs/plans.md` §4.2）
    let text = "# writer\nmodel-hint: careful\nfoo: bar\nflurb: 一行紹介\n";
    assert_eq!(
        diags_tag(text),
        [w011(text, 2, "foo"), w011(text, 3, "flurb")].join("\n")
    );
    let parsed = prompt_md::parse(text);
    // 表にあるキーは正準形で引ける（第 2.6 版）
    assert_eq!(parsed.meta_fields(FieldKey::ModelHint).len(), 1);
    // 表に無いキーは書かれたままのキーで引く
    assert_eq!(parsed.meta_field("foo").unwrap().key, FieldKey::Unknown);
    assert_eq!(parsed.meta_field("foo").unwrap().value, "bar");
    assert_eq!(parsed.meta_field("flurb").unwrap().value, "一行紹介");
    assert_eq!(parsed.to_text(), text);
}

// ============================================================ 壊れた入力（完了条件 3）

#[test]
fn broken_input_does_not_panic() {
    let cases = [
        "",
        "\n",
        "#",
        "# \n",
        "## \n",
        "{{",
        "{{}}",
        "# writer\n{{",
        "# writer\n## system\n{{scene_id",
        "# writer\n## system\n}}\n",
        "## task\n# writer\n",
        "# writer\n## system\n  継続行\n",
    ];
    for text in cases {
        let parsed = prompt_md::parse(text);
        assert_eq!(parsed.to_text(), text, "round-trip: {text:?}");
        let _ = prompt_tag(&parsed);
        let _ = variables_tag(&parsed);
        let _ = diags_of(&parsed);
        let _ = parsed.body_of(0);
        let custom_parsed = prompt_md::parse_with(text, &custom());
        assert_eq!(custom_parsed.to_text(), text);
    }
}

#[test]
fn an_empty_file_has_nothing() {
    let parsed = prompt_md::parse("");
    assert!(parsed.role.is_none());
    assert!(parsed.sections.is_empty());
    assert!(parsed.variables.is_empty());
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.lines.len(), 1);
}
