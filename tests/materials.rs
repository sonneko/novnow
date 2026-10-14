//! `core::resolve::materials`（§6.6 資料 ID の解決とインレイヒント）のテスト。
//!
//! `PLANS.md` §5.4 の作業項目 4 に対応する。入力には **§5.8.2 の `index.json` の例をそのまま**
//! 使い（`PLANS.md` §6.3「仕様が正本」）、§6.6.1 の 3 経路の書き方・§6.6.2 のヒント・
//! §6.6.3 の E006 / E007 とクイックフィックス 3 種・§6.6.4 の W031 を固定する。
//!
//! 位置は範囲の数値ではなく**その範囲で原文を切った文字列**で固定する（CJK は 1 字 3 byte）。

use novnow::core::diagnostics::DiagnosticCode;
use novnow::core::model::material::MaterialStatus;
use novnow::core::resolve::dictionary::{self, Dictionary};
use novnow::core::resolve::materials::{self, Hint, Route, Scan};
use novnow::core::syntax::characters_md;
use novnow::core::syntax::materials_index::{self, Index};
use novnow::core::syntax::words_md;

/// §5.8.2 の例（M0137）に、`status` の 3 値・`S` 接頭辞（§5.8.2・追加仕様 11）・
/// クイックフィックス 3 用の「よく似た別タイトル」・長いタイトルを足したもの。
const INDEX_JSON: &str = r#"{
  "schema_version": 1,
  "next_id": 143,
  "items": [
    {
      "id": "M0137",
      "title": "科学の甲子園 本戦の運営と実技課題",
      "path": "materials/docs/M0137-kagaku-koshien.md",
      "source": { "kind": "url", "url": "https://example.org/report/2026", "accessed_at": "2026-10-01T09:12:00+09:00" },
      "category": "取材",
      "conditions": [
        { "kind": "scope", "value": "@02-03 .. @02-08" },
        { "kind": "ref",   "value": "'科学の甲子園" },
        { "kind": "text",  "value": "大会運営の手順を描写するとき" }
      ],
      "saved_at": "2026-10-01T09:20:33+09:00",
      "updated_at": "2026-10-02T22:10:00+09:00",
      "tags": ["大会", "高校", "運営"],
      "status": "approved",
      "added_by": "human",
      "summary": "本戦の 1 日のタイムテーブルと実技課題の採点基準。",
      "checksum": "sha256:9f2c..."
    },
    {
      "id": "M0138",
      "title": "旧校舎の間取り",
      "path": "materials/inbox/M0138-kyukousha.md",
      "source": { "kind": "custom", "text": "聞き取り" },
      "category": "画像",
      "conditions": [],
      "saved_at": "2026-10-02T09:00:00+09:00",
      "status": "proposed",
      "added_by": "agent:researcher"
    },
    {
      "id": "M0139",
      "title": "使わなかった年表",
      "path": "materials/docs/M0139-nenpyou.md",
      "source": { "kind": "custom", "text": "自作" },
      "category": "文献",
      "conditions": [],
      "saved_at": "2026-10-03T09:00:00+09:00",
      "status": "rejected",
      "added_by": "human"
    },
    {
      "id": "S0140",
      "title": "上下げ窓の調査メモ",
      "path": "materials/docs/S0140-uage-mado.md",
      "source": { "kind": "search", "query": "上下げ窓 1930年代" },
      "category": "検索結果",
      "conditions": [],
      "saved_at": "2026-10-04T09:00:00+09:00",
      "status": "approved",
      "added_by": "agent:researcher"
    },
    {
      "id": "M0141",
      "title": "科学の甲子園 地方大会の運営",
      "path": "materials/docs/M0141-kagaku-chihou.md",
      "source": { "kind": "custom", "text": "自作" },
      "category": "取材",
      "conditions": [],
      "saved_at": "2026-10-05T09:00:00+09:00",
      "status": "approved",
      "added_by": "human"
    },
    {
      "id": "M0142",
      "title": "二十四字をこえるとても長いタイトルはヒントで切られる",
      "path": "materials/docs/M0142-nagai.md",
      "source": { "kind": "custom", "text": "自作" },
      "category": "文献",
      "conditions": [],
      "saved_at": "2026-10-06T09:00:00+09:00",
      "status": "approved",
      "added_by": "human"
    }
  ]
}
"#;

/// JSON として壊れている `index.json`（§5.8.2 → **E025**、空索引の既定値で続行）。
const BROKEN_JSON: &str = "{\n  \"schema_version\": 1,\n";

// ============================================================ 小道具

/// `index.json` と、それだけから作った辞書。
struct World {
    index: Index<'static>,
    dictionary: Dictionary,
}

impl World {
    fn new(text: &'static str) -> Self {
        let index = materials_index::parse(text);
        let characters = characters_md::parse("");
        let words = words_md::parse("");
        let dictionary = Dictionary::from_sources(&characters, &words, &index).dictionary;
        Self { index, dictionary }
    }

    /// §5.8.2 の例の索引。
    fn spec() -> Self {
        Self::new(INDEX_JSON)
    }

    fn scan(&self, line: &str, in_body: bool) -> Scan<'_> {
        materials::scan_line(line, 0, &self.dictionary, &self.index, in_body)
    }
}

/// 診断を `CODE@該当箇所` の形にする（行頭を 0 としているので原文をそのまま切れる）。
fn diags_tag(line: &str, found: &Scan<'_>) -> String {
    found
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let range = diagnostic.range.clone().unwrap_or(0..0);
            format!("{}@{}", diagnostic.code.as_str(), &line[range])
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

fn codes(found: &Scan<'_>) -> Vec<DiagnosticCode> {
    found
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn args(found: &Scan<'_>) -> Vec<String> {
    found
        .diagnostics
        .iter()
        .flat_map(|diagnostic| {
            diagnostic.args.iter().map(|arg| match arg {
                novnow::core::diagnostics::Arg::Text(text) => text.clone(),
                novnow::core::diagnostics::Arg::Number(value) => value.to_string(),
            })
        })
        .collect()
}

fn hint_of<'a>(found: &'a Scan<'_>) -> &'a Hint {
    found.resolutions[0].hint.as_ref().expect("ヒントが無い")
}

// ============================================================ §6.6.1 の 3 経路

#[test]
fn the_index_example_resolves_by_id() {
    let world = World::spec();
    // §5.3.4 の例の `material:` 行（ID のみ）
    let line = "material: 'M0137 旧校舎の構造メモ";
    let found = world.scan(line, false);
    assert!(found.diagnostics.is_empty(), "{}", diags_tag(line, &found));
    assert_eq!(found.resolutions.len(), 1);
    let resolved = &found.resolutions[0];
    assert_eq!(resolved.route, Some(Route::Id));
    assert_eq!(resolved.material.expect("実体").id, "M0137");
    assert!(resolved.is_resolved());
    // §6.6.1「タイトルはインレイヒント（ファイルには書かれない）として表示」
    assert_eq!(hint_of(&found).title, "科学の甲子園 本戦の運営と実技課題");
    assert!(!hint_of(&found).truncated);
    assert_eq!(resolved.normalize_to_id, None);
}

#[test]
fn an_explicit_title_that_matches_is_not_a_diagnostic() {
    let world = World::spec();
    // §6.6.1 の 2 行目の書き方
    let line = "'M0137:科学の甲子園 本戦の運営と実技課題";
    let found = world.scan(line, false);
    assert!(found.diagnostics.is_empty(), "{}", diags_tag(line, &found));
    assert_eq!(found.resolutions[0].route, Some(Route::IdWithTitle));
    assert_eq!(found.resolutions[0].material.expect("実体").id, "M0137");
}

#[test]
fn a_title_reference_resolves_and_offers_the_id() {
    let world = World::spec();
    // §6.6.1 の 3 行目の書き方（空白を含むタイトルが 1 つの参照になる、B-59）
    let line = "'科学の甲子園 本戦の運営と実技課題";
    let found = world.scan(line, false);
    assert!(found.diagnostics.is_empty(), "{}", diags_tag(line, &found));
    let resolved = &found.resolutions[0];
    assert_eq!(resolved.route, Some(Route::Title));
    assert_eq!(resolved.material.expect("実体").id, "M0137");
    // §6.6.1「ID へ正規化するクイックフィックスを提供」（診断は出ない）
    assert_eq!(resolved.normalize_to_id.as_deref(), Some("'M0137"));
    assert_eq!(hint_of(&found).title, "科学の甲子園 本戦の運営と実技課題");
}

#[test]
fn an_s_prefixed_id_resolves_the_same_way() {
    let world = World::spec();
    // §5.8.2「`M` と `S` は同一の連番空間を共有する（MUST）」
    let line = "'S0140";
    let found = world.scan(line, false);
    assert!(found.diagnostics.is_empty(), "{}", diags_tag(line, &found));
    assert_eq!(found.resolutions[0].route, Some(Route::Id));
    assert_eq!(
        found.resolutions[0].material.expect("実体").title,
        "上下げ窓の調査メモ"
    );
}

// ============================================================ §6.6.3 E006 とクイックフィックス

#[test]
fn a_differing_explicit_title_is_e006() {
    let world = World::spec();
    let line = "'M0137:科学の甲子園";
    let found = world.scan(line, false);
    // 波線は `modifier.title` の範囲（§6.6.3）＝ `:` の後ろだけ
    assert_eq!(diags_tag(line, &found), "E006@科学の甲子園");
    // §7 E006「資料 {0} のタイトルが一致しません」
    assert_eq!(args(&found), vec!["M0137".to_string()]);
    let fixes = found.resolutions[0]
        .fixes
        .clone()
        .expect("クイックフィックス");
    // 1（既定）: index のタイトルで置換
    assert_eq!(fixes.replace_title, "科学の甲子園 本戦の運営と実技課題");
    // 2: index 側をファイルの記述に合わせる
    assert_eq!(fixes.rewrite_index, "科学の甲子園");
    // 3: 同名タイトルを持つ別 ID は無い
    assert_eq!(fixes.other_id, None);
    // 実体は引けているのでインレイヒントは出る
    assert!(found.resolutions[0].is_resolved());
}

#[test]
fn the_third_quick_fix_points_at_the_other_id() {
    let world = World::spec();
    // 書かれたタイトルは M0141 のもの（§6.6.3 のクイックフィックス 3）
    let line = "'M0137:科学の甲子園 地方大会の運営";
    let found = world.scan(line, false);
    assert_eq!(codes(&found), vec![DiagnosticCode::E006]);
    let fixes = found.resolutions[0]
        .fixes
        .clone()
        .expect("クイックフィックス");
    assert_eq!(
        fixes.other_id.map(|id| id.to_string()).as_deref(),
        Some("M0141")
    );
}

#[test]
fn e006_compares_after_normalization() {
    // §6.6.3「正規化比較（§6.1.3）」＝長音統一 → NFKC → 小文字化 → 空白除去
    assert!(materials::same_title(
        "科学の甲子園　本戦の運営と実技課題",
        "科学の甲子園 本戦の運営と実技課題"
    ));
    assert!(materials::same_title("ソ－ラ－", "ソーラー"));
    assert!(materials::same_title("ＫＡＧＡＫＵ", "kagaku"));
    assert!(materials::same_title("ｽﾀｰ", "スター"));
    assert!(!materials::same_title("科学の甲子園", "科学の甲子園 本戦"));
}

#[test]
fn a_normalized_title_is_not_e006() {
    let world = World::spec();
    // 全角空白で書いても一致（§6.1.3 の空白除去）
    let line = "'M0137:科学の甲子園　本戦の運営と実技課題";
    let found = world.scan(line, false);
    assert!(found.diagnostics.is_empty(), "{}", diags_tag(line, &found));
}

// ============================================================ §6.6.3 E007 と補完候補

#[test]
fn an_unknown_id_is_e007_with_five_suggestions() {
    let world = World::spec();
    let line = "'M0136";
    let found = world.scan(line, false);
    // **E001 と重ねない**（資料 ID の形をした未解決トークンは E007 だけ。B-71）
    assert_eq!(diags_tag(line, &found), "E007@'M0136");
    assert_eq!(args(&found), vec!["M0136".to_string()]);
    let resolved = &found.resolutions[0];
    assert!(!resolved.is_resolved());
    assert_eq!(resolved.route, None);
    assert_eq!(resolved.id.as_deref(), Some("M0136"));
    // §6.6.3「補完候補として近い ID / タイトルを 5 件提示」
    assert_eq!(resolved.suggestions.len(), materials::SUGGESTION_LIMIT);
    let ids = resolved
        .suggestions
        .iter()
        .map(|material| material.id.as_str())
        .collect::<Vec<_>>();
    // 一番近いのは 1 文字違いの M0137
    assert_eq!(ids[0], "M0137");
    assert_eq!(ids, vec!["M0137", "M0138", "M0139", "M0141", "M0142"]);
}

#[test]
fn a_reference_that_is_not_an_id_stays_e001() {
    let world = World::spec();
    // 資料 ID の形でない未解決参照は辞書の E001 のまま（§6.2.1）
    let line = "'しらぬ資料";
    let found = world.scan(line, false);
    assert_eq!(codes(&found), vec![DiagnosticCode::E001]);
    assert!(found.resolutions.is_empty());
}

#[test]
fn suggestions_also_look_at_titles() {
    let world = World::spec();
    let found = materials::suggestions(&world.index, "旧校舎", 3);
    let ids = found
        .iter()
        .map(|material| material.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids[0], "M0138");
    assert_eq!(found.len(), 3);
}

#[test]
fn distance_is_the_levenshtein_distance() {
    assert_eq!(materials::distance("M0137", "M0137"), 0);
    assert_eq!(materials::distance("M0137", "M0138"), 1);
    assert_eq!(materials::distance("M0137", "S0140"), 3);
    assert_eq!(materials::distance("", "abc"), 3);
    assert_eq!(materials::distance("kitten", "sitting"), 3);
}

#[test]
fn a_broken_index_reports_e007_and_keeps_going() {
    let world = World::new(BROKEN_JSON);
    // §5.8.2「E025 が出たときは空の索引の既定値で続行し、原文は 1 バイトも書き換えない」
    assert_eq!(
        world
            .index
            .diagnostics
            .iter()
            .map(|found| found.code)
            .collect::<Vec<_>>(),
        vec![DiagnosticCode::E025]
    );
    let line = "'M0137";
    let found = world.scan(line, false);
    assert_eq!(codes(&found), vec![DiagnosticCode::E007]);
    assert!(found.resolutions[0].suggestions.is_empty());
    assert_eq!(world.index.to_text(), BROKEN_JSON);
}

// ============================================================ §6.6.2 インレイヒント

#[test]
fn the_hint_sits_right_after_the_token() {
    let world = World::spec();
    let line = "material: 'M0137";
    let found = world.scan(line, false);
    // §6.6.2「表示位置: トークン直後」
    assert_eq!(hint_of(&found).at, line.len());
    // 行頭がファイルの途中なら、そのぶんだけずれる（B-56「位置の単位を混ぜない」）
    let shifted = materials::scan_line(line, 100, &world.dictionary, &world.index, false);
    assert_eq!(hint_of(&shifted).at, 100 + line.len());
    // E006 の波線も同じぶんだけずれる（B-56「位置の単位を混ぜない」）
    let wrong = "'M0137:科学の甲子園";
    let shifted = materials::scan_line(wrong, 100, &world.dictionary, &world.index, false);
    let range = shifted.diagnostics[0].range.clone().expect("range");
    assert_eq!(range.start, 100 + "'M0137:".len());
}

#[test]
fn dictionary_diagnostics_are_shifted_to_the_file_too() {
    // 辞書側の診断（E001 / W029 など）も資料側と同じくファイル単位で返す（B-56）。
    // M3 の時点では `line_start` を足し忘れていて、行頭が 0 でない行で波線がずれていた
    let world = World::spec();
    let line = "'存在しない名前 と 'M9999";
    let shifted = materials::scan_line(line, 100, &world.dictionary, &world.index, false);
    let mut found: Vec<(String, usize)> = shifted
        .diagnostics
        .iter()
        .map(|d| {
            (
                d.code.as_str().to_string(),
                d.range.clone().expect("range").start,
            )
        })
        .collect();
    found.sort();
    let unknown = line.find("'M9999").expect("M9999");
    assert_eq!(
        found,
        vec![
            ("E001".to_string(), 100),
            ("E007".to_string(), 100 + unknown)
        ]
    );
}

#[test]
fn the_hint_carries_the_status_for_the_ui_to_word() {
    let world = World::spec();
    // §6.6.2 の `⟨タイトル・提案中⟩` / `⟨タイトル・却下⟩` は **UI が組む**（§12.2）。
    // core は status をそのまま返す
    for (line, status) in [
        ("'M0137", Some(MaterialStatus::Approved)),
        ("'M0138", Some(MaterialStatus::Proposed)),
        ("'M0139", Some(MaterialStatus::Rejected)),
    ] {
        let found = world.scan(line, false);
        assert_eq!(hint_of(&found).status, status, "{line}");
        assert_eq!(found.resolutions[0].route, Some(Route::Id), "{line}");
    }
}

#[test]
fn a_long_title_is_cut_at_twenty_four_characters() {
    let world = World::spec();
    let line = "'M0142";
    let found = world.scan(line, false);
    let hint = hint_of(&found);
    // §6.6.2「24 字を超える場合は 23 字 + `…`」。**字**は Unicode の文字数（byte ではない）
    let title = &found.resolutions[0].material.expect("実体").title;
    assert!(title.chars().count() > materials::HINT_MAX_CHARS);
    assert!(hint.truncated);
    assert_eq!(hint.title.chars().count(), materials::HINT_MAX_CHARS);
    assert_eq!(hint.title.chars().last(), Some(materials::HINT_ELLIPSIS));
    let expected: String = title.chars().take(materials::HINT_KEEP_CHARS).collect();
    assert_eq!(hint.title, expected + "…");
}

#[test]
fn hint_title_keeps_twenty_four_characters_as_they_are() {
    let exactly = "あ".repeat(materials::HINT_MAX_CHARS);
    assert_eq!(materials::hint_title(&exactly), (exactly.clone(), false));
    let one_more = exactly.clone() + "い";
    let (cut, truncated) = materials::hint_title(&one_more);
    assert!(truncated);
    assert_eq!(cut.chars().count(), materials::HINT_MAX_CHARS);
    assert!(cut.ends_with(materials::HINT_ELLIPSIS));
    assert!(cut.starts_with("あ"));
    // 空タイトルでもパニックしない
    assert_eq!(materials::hint_title(""), (String::new(), false));
}

// ============================================================ §6.6.4 W031

#[test]
fn a_material_reference_in_the_body_is_w031() {
    let world = World::spec();
    let line = "'M0137 を見ながら書いた。";
    // 本文（C 形式）→ §6.6.4「資料参照はプロットに書くことを推奨します」
    assert_eq!(diags_tag(line, &world.scan(line, true)), "W031@'M0137");
    // プロットなら出ない
    assert!(world.scan(line, false).diagnostics.is_empty());
}

#[test]
fn w031_is_added_even_when_the_id_does_not_exist() {
    let world = World::spec();
    let line = "'M0136";
    assert_eq!(
        diags_tag(line, &world.scan(line, true)),
        "W031@'M0136 / E007@'M0136"
    );
}

#[test]
fn w031_is_not_added_for_other_references() {
    let world = World::spec();
    // 資料でない参照（未解決でも）には W031 を出さない。
    // 赤波線は §6.2.6 により先頭 1 形態素まで（`'しらぬ` の `資料` は巻き込まない）
    let line = "'しらぬ資料";
    assert_eq!(diags_tag(line, &world.scan(line, true)), "E001@'しらぬ");
}

#[test]
fn e006_and_w031_can_come_out_together() {
    let world = World::spec();
    let line = "'M0137:科学の甲子園";
    assert_eq!(
        diags_tag(line, &world.scan(line, true)),
        "W031@'M0137:科学の甲子園 / E006@科学の甲子園"
    );
}

// ============================================================ 入口の小道具

#[test]
fn route_of_reads_the_dictionary_entry() {
    let world = World::spec();
    let routes = [
        ("'M0137", Some(Route::Id)),
        ("'M0137:科学の甲子園", Some(Route::IdWithTitle)),
        ("'科学の甲子園 本戦の運営と実技課題", Some(Route::Title)),
        ("'しらぬ資料", None),
    ];
    for (line, expected) in routes {
        let scan = dictionary::scan_line(line, 0, &world.dictionary);
        assert_eq!(scan.tokens.len(), 1, "{line}");
        assert_eq!(
            materials::route_of(&scan.tokens[0], &world.dictionary),
            expected,
            "{line}"
        );
        assert_eq!(
            materials::material_id_of(&scan.tokens[0], &world.dictionary),
            expected.map(|_| "M0137"),
            "{line}"
        );
    }
}

#[test]
fn unknown_material_id_only_accepts_the_id_shape() {
    let world = World::spec();
    let accepted = ["'M0136", "'S9999", "'M00001"];
    let rejected = ["'しらぬ資料", "'M13", "'0137", "'M013A"];
    for line in accepted.into_iter().chain(rejected) {
        let scan = dictionary::scan_line(line, 0, &world.dictionary);
        let found = materials::unknown_material_id(line, &scan.tokens[0]);
        let should = accepted.contains(&line);
        assert_eq!(found.is_some(), should, "{line} → {found:?}");
    }
}

#[test]
fn a_line_without_references_is_empty() {
    let world = World::spec();
    let line = "summary: 資料は無い行。";
    let found = world.scan(line, true);
    assert!(found.diagnostics.is_empty());
    assert!(found.resolutions.is_empty());
}
