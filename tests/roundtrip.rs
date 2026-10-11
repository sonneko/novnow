//! 未知キーを含むファイルの読み書き（`PLANS.md` §6.2 の M2 の行、本書 §1.5-1,2）。
//!
//! §1.5 は ① が ② のために必ず守る MUST なので、形式ごとの unit テストとは別に
//! **1 ファイルに集めて**固定する。
//!
//! - **§1.5-2（未知フィールド行の温存）**: A / B 形式の未知 `key:` は W011 を出しつつ
//!   **値を保持**し、書き戻したバイト列は入力と一致する（§5.3 完了条件 2）。② で
//!   `when:` `senses:` `space:` `blurb:` `illustration:` を足しても ① のファイルは壊れない。
//! - **§1.5-1（未知キーの温存）**: `project.json` / `materials/index.json` の未知キーを
//!   読み書きで失わない。`materials/index.json` は JSON の再シリアライズがキー順と空白を
//!   変えてしまうので**原文をそのまま返す**（`core::syntax::materials_index` の doc）。
//!   `.novnow/ui.json`（§4.4.1）は §3.3 の `core/model` の 10 型に含まれないので M2 の範囲外。
//!
//! 日本語別名（§5.0.6）も「保存時に正準形へ正規化しない」ので、ここで作者が書いた表記が
//! そのまま戻ることを一緒に見る。

use novnow::core::model::{MaterialIndex, Project};
use novnow::core::syntax::{
    characters_md, contents, ideas_md, materials_index, plot_episode, plot_outline, prompt_md,
    words_md,
};
use novnow::core::text::lexer::FieldKey;

// ============================================================ ② のキーをたっぷり含むフィクスチャ

/// A 形式。② のキー（`blurb` `when` `space-mode` `space` `illustration`）と未知キー（`foo`）、
/// 日本語別名（`要約`）、継続行、行コメント、`# ` 行（① ではコメントにならない、B-36）。
const A_FORM: &str = "\
// 全体プロット: ② のキーを ① で書いても壊れないことの確認
title: 星屑の時計
blurb: 作品全体のあらすじ

@01 失われた刻印
要約: 日本語別名で書いた summary
when: 406 09:40
space-mode: ascii
space:
  ┌────┬──┐
  │ 書架A │ 窓 │     字下げは値の一部
  └────┴──┘
illustration: 挿絵候補 / リリアが図面を覗き込む
foo: 未知のキー
  字下げた継続行
state: fixed
;

@02 補習と地図
summary: 期末試験の失敗
;
";

/// B 形式。② のキー（`when` `senses` `space` `illustration`）と行末コメント、`# ` 行。
const B_FORM: &str = "\
@01-04-02 図書室の攻防
cast: 'アルス 'リリア
mode: 'アルス:A 'リリア:A
pov: 'アルス
when: 406 09:40
space-mode: ascii
space:
  ┌────────┬────┐
  └────────┴────┘

幼馴染の'リリア が、試験勉強をサボっている'アルス を咎める。
senses: 視覚 聴覚
check: リリアが「怒っているが見捨てない」ことが伝わること
;
mode: 'リリア:B+C          // 呆れモードと好奇心モードの併用
# ここの心変わりは何度か書き直すかもしれない
'リリア は呆れつつも、図面の精巧さに好奇心を抑えられなくなる。
illustration: 挿絵候補 / リリアが図面を覗き込む / 窓からの逆光
length: 500
;
";

/// C 形式。話者注釈・ルビ・エスケープした `@`・字下げ・行コメント・行末コメント。
const C_FORM: &str = "\
@01-04-02 図書室の攻防
「やっぱり'アルス、ここにいたのね」@リリア:A
\\@ エスケープした at はただの文字
呆れたような声とともに、'リリアが隣の椅子を引き寄せた。『星屑の時計《ほしくずのとけい》』
   字下げた行もそのまま
;
// 行コメントだけの行
「……付き合ってあげるわよ」@リリア:B    // ここでモードが切り替わる
";

/// `characters.md`。② のキー（`color` `address` `icon`）、日本語のセクション名（§5.5.2）、
/// 4 種に無い行（`#恩田恵`・`#### 深い見出し`・閉じない鉤括弧・`-5`）。
const CHARACTERS_MD: &str = "\
# 恩田恵
reading: おんだめぐみ
alias: めぐみ, 恵
color: #8fb8ff
icon: assets/icons/megumi.png

## 性格
属性: A高校サイエンス部員（高1→高2）。
  字下げた継続行

## tone
「*で…」「*です…」「…あの…*」

## modes
### A 日常
range: @01 ..
personality: おとなしく控えめ。
address: '葵あおい => あおいちゃん
「*だよ…」「えへへ、*」
### A 後半
range: @03 ..
#### 深すぎる見出しは自由記述
#恩田恵
「閉じない鉤括弧
-5

## relations
'葵あおい: 魂のパートナー。
'瀬戸晴香：技術の師弟関係。

## notes
口癖サンプル:
あおいちゃんが行くなら私も……。
";

/// `words.md`。未知キーと H2 と箇条書き。
const WORDS_MD: &str = "\
# 星屑の時計
reading: ほしくずのとけい
category: 魔導具
alias: 星屑時計
definition: 旧校舎の地下遺跡に眠るとされる幻の魔導具。
range: @01 ..
color: #ff0000
note: 第 3 章で贋作であることが判明する。

## notes
- 箇条書きの行

# 旧校舎
reading: きゅうこうしゃ
definition:
  複数行にした定義の 1 行目
  2 行目
";

/// `ideas.md`。§5.7 が定義するが §5.0.6 の表に無い 4 キー。
const IDEAS_MD: &str = "\
# I0007 学院長の動機
status: inbox            # inbox | keep | promoted | dropped
created: 2026-10-09T12:04:11+09:00
tags: 学院長, 動機
relates: @02, 'アルス
promoted-to: plots/02.txt#@02-03      # 昇格時のみ
本文は自由記述。改行もそのまま保持する。

# I0008 タイトルだけ
";

/// `prompts/*.md`。§5.9.1 が定義するが §5.0.6 の表に無いキーと未知の変数。
const PROMPT_MD: &str = "\
# weapon-advisor
label: 武器考証                     # UI 表示名（省略時はファイル名）
model-hint: careful
max-chars: 8000
temperature: 0.5
context: project outline target materials constraints

## system
あなたは近世ヨーロッパの武具に詳しい考証担当です。

## task
対象: {{scene_id}}
本文: {{content_section}}

## output
- 指摘は箇条書き。
- 問題がなければ「問題なし」とだけ書く。
";

/// `materials/index.json`。3 階層すべてに未知キー。
const MATERIALS_JSON: &str = r#"{
  "schema_version": 1,
  "next_id": 138,
  "future_index_key": true,
  "items": [
    {
      "id": "M0137",
      "title": "科学の甲子園 本戦の運営と実技課題",
      "path": "materials/docs/M0137-kagaku-koshien.md",
      "future_item_key": { "nested": [1, 2] },
      "source": { "kind": "url", "url": "https://example.org/r", "future_source_key": null },
      "conditions": [
        { "kind": "scope", "value": "@02-03 .. @02-08", "future_condition_key": "x" }
      ],
      "saved_at": "2026-10-01T09:20:33+09:00",
      "status": "approved"
    }
  ]
}
"#;

/// `project.json`（§4.5）。トップレベルと入れ子に未知キー。
const PROJECT_JSON: &str = r#"{
  "schema_version": 1,
  "title": "星屑の時計",
  "future_top_key": { "a": 1 },
  "structure": {
    "chapter_digits": 2,
    "future_structure_key": "x"
  }
}
"#;

// ============================================================ §1.5-2 バイト一致

#[test]
fn the_a_form_round_trips_byte_for_byte_with_unknown_keys() {
    let parsed = plot_outline::parse(A_FORM);
    assert_eq!(parsed.to_text(), A_FORM);
    // 未知キーは W011 になるが**値は消えない**（§5.0.6 MUST）
    assert!(parsed
        .diagnostics
        .iter()
        .any(|found| found.code.as_str() == "W011"));
    assert!(parsed
        .blocks
        .iter()
        .flat_map(|block| block.fields.iter())
        .any(|&index| parsed
            .field(index)
            .is_some_and(|field| field.key_text == "foo")));
}

#[test]
fn the_b_form_round_trips_byte_for_byte_with_unknown_keys() {
    let parsed = plot_episode::parse(B_FORM);
    assert_eq!(parsed.to_text(), B_FORM);
    let keys: Vec<&str> = parsed
        .scenes
        .iter()
        .flat_map(|scene| scene.fields.iter())
        .filter_map(|&index| parsed.field(index))
        .map(|field| field.key_text)
        .collect();
    // ② のキーが**書かれたまま**で残る（`when` `space-mode` `space`）
    assert!(keys.contains(&"when"));
    assert!(keys.contains(&"space-mode"));
    assert!(keys.contains(&"space"));
}

#[test]
fn the_c_form_round_trips_byte_for_byte() {
    assert_eq!(contents::parse(C_FORM).to_text(), C_FORM);
}

#[test]
fn characters_md_round_trips_byte_for_byte_with_unknown_keys() {
    let parsed = characters_md::parse(CHARACTERS_MD);
    assert_eq!(parsed.to_text(), CHARACTERS_MD);
    // `address` / `icon` は ② のキーなので ① では W011 ＋ 値を保持（PLANS.md §1.2）。
    // `color` は §5.0.6 の表に正式追加された（第 2.6 版・A-15）ので W011 にならない
    let warned: Vec<String> = parsed
        .diagnostics
        .iter()
        .filter(|found| found.code.as_str() == "W011")
        .filter_map(|found| match found.args.first() {
            Some(novnow::core::diagnostics::Arg::Text(value)) => Some(value.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(warned, vec!["icon".to_owned(), "address".to_owned()]);
    assert_eq!(
        parsed.character_fields(0, FieldKey::Color).len(),
        1,
        "{warned:?}"
    );
}

#[test]
fn words_md_round_trips_byte_for_byte_with_unknown_keys() {
    let parsed = words_md::parse(WORDS_MD);
    assert_eq!(parsed.to_text(), WORDS_MD);
    assert_eq!(parsed.words.len(), 2);
    // `definition:` の継続行も値の一部として戻る。1 行目が `definition:` だけなので値は空で、
    // 先頭に LF が入る。継続行の寄与はインデントから **2 文字だけ**落とした残り（B-37）
    assert_eq!(
        parsed.definition(1).unwrap().full_value(),
        "\n複数行にした定義の 1 行目\n2 行目"
    );
}

#[test]
fn ideas_md_round_trips_byte_for_byte_with_unknown_keys() {
    let parsed = ideas_md::parse(IDEAS_MD);
    assert_eq!(parsed.to_text(), IDEAS_MD);
    // `status` `created` `relates` `promoted-to` は §5.0.6 の表に正式追加された
    // （第 2.6 版・A-15）ので W011 は出ない。値は改変せず残る
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.status(0).unwrap().key, FieldKey::Status);
    assert_eq!(parsed.promoted_to(0).unwrap().key, FieldKey::PromotedTo);
    assert_eq!(
        parsed.unknown_field(0, "relates").unwrap().value,
        "@02, 'アルス"
    );
}

#[test]
fn prompt_md_round_trips_byte_for_byte_with_unknown_keys() {
    let parsed = prompt_md::parse(PROMPT_MD);
    assert_eq!(parsed.to_text(), PROMPT_MD);
    assert_eq!(parsed.meta_field("model-hint").unwrap().value, "careful");
}

// ============================================================ 形が崩れたファイル

#[test]
fn files_without_a_trailing_newline_round_trip() {
    for text in [
        "@01-04-02 図書室の攻防\n「こんにちは」@リリア:A",
        "@01 章\nsummary: s\n;",
        "# 恵\nreading: めぐみ",
    ] {
        assert_eq!(contents::parse(text).to_text(), text, "{text:?}");
        assert_eq!(plot_outline::parse(text).to_text(), text, "{text:?}");
        assert_eq!(characters_md::parse(text).to_text(), text, "{text:?}");
        assert_eq!(words_md::parse(text).to_text(), text, "{text:?}");
        assert_eq!(ideas_md::parse(text).to_text(), text, "{text:?}");
        assert_eq!(prompt_md::parse(text).to_text(), text, "{text:?}");
        assert_eq!(plot_episode::parse(text).to_text(), text, "{text:?}");
    }
}

#[test]
fn trailing_spaces_and_crlf_are_kept_as_written() {
    // §17.1「保存時に末尾空白を落とす」は**エクスポート**の規則で、パースの書き戻しは
    // 書かれたバイトをそのまま返す（§5.3 完了条件 2）。B-34（空白だけの行も Blank）
    let text = "@01-04-02 x   \n本文\t \r\n   \n;\n";
    assert_eq!(contents::parse(text).to_text(), text);
    assert_eq!(plot_episode::parse(text).to_text(), text);
}

#[test]
fn an_empty_file_round_trips_for_every_form() {
    for text in ["", "\n", "\n\n\n"] {
        assert_eq!(contents::parse(text).to_text(), text, "{text:?}");
        assert_eq!(plot_outline::parse(text).to_text(), text, "{text:?}");
        assert_eq!(plot_episode::parse(text).to_text(), text, "{text:?}");
        assert_eq!(characters_md::parse(text).to_text(), text, "{text:?}");
        assert_eq!(words_md::parse(text).to_text(), text, "{text:?}");
        assert_eq!(ideas_md::parse(text).to_text(), text, "{text:?}");
        assert_eq!(prompt_md::parse(text).to_text(), text, "{text:?}");
        assert_eq!(materials_index::parse(text).to_text(), text, "{text:?}");
    }
}

// ============================================================ §1.5-1 JSON の未知キー

#[test]
fn materials_index_returns_the_original_bytes() {
    // JSON は再シリアライズするとキー順と空白が変わるので**原文をそのまま返す**
    let parsed = materials_index::parse(MATERIALS_JSON);
    assert!(parsed.error.is_none(), "{:?}", parsed.error);
    assert_eq!(parsed.to_text(), MATERIALS_JSON);
}

#[test]
fn materials_index_keeps_unknown_keys_at_every_level() {
    let index = materials_index::parse(MATERIALS_JSON).index.unwrap();
    assert!(index.unknown.contains_key("future_index_key"));
    let item = &index.items[0];
    assert!(item.unknown.contains_key("future_item_key"));
    assert!(item.source.unknown.contains_key("future_source_key"));
    assert!(item.conditions[0]
        .unknown
        .contains_key("future_condition_key"));
    // 書いて戻しても未知キーは消えない（読み → 書き → 読み）
    let written = serde_json::to_string(&index).unwrap();
    let again: MaterialIndex = serde_json::from_str(&written).unwrap();
    assert!(again.unknown.contains_key("future_index_key"));
    assert!(again.items[0].unknown.contains_key("future_item_key"));
    assert!(again.items[0]
        .source
        .unknown
        .contains_key("future_source_key"));
}

#[test]
fn project_json_keeps_unknown_keys_at_every_level() {
    let project: Project = serde_json::from_str(PROJECT_JSON).unwrap();
    assert_eq!(project.schema_version, 1);
    assert!(project.unknown.contains_key("future_top_key"));
    assert!(project
        .structure
        .unknown
        .contains_key("future_structure_key"));
    // §4.5.1 の既定値は書かれていないキーに入る
    assert_eq!(project.structure.episode_digits, 2);
    let written = serde_json::to_string(&project).unwrap();
    let again: Project = serde_json::from_str(&written).unwrap();
    assert!(again.unknown.contains_key("future_top_key"));
    assert!(again.structure.unknown.contains_key("future_structure_key"));
}

#[test]
fn broken_json_is_kept_as_text_and_does_not_panic() {
    // §4.3-4「壊れていても開ける」。`materials/index.json` は診断コードが無いので
    // `Index::error` にメッセージを持つ（`PLANS.md` §9.1 A-17）
    for text in ["{", "{\"items\": [", "not json at all", "[1,2,3]"] {
        let parsed = materials_index::parse(text);
        assert_eq!(parsed.to_text(), text, "{text:?}");
        assert!(parsed.error.is_some() || parsed.index.is_some(), "{text:?}");
    }
}
