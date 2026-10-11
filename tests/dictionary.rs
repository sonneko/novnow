//! `core::resolve::dictionary`（§6.1 辞書 / §6.2 `'` の最短一致）のテスト。
//!
//! `PLANS.md` §5.4 の作業項目 1 と完了条件 2（§6.2.2 の `'リリアナ` → `リリア` + `ナ` と
//! **W029**）・完了条件 4（5,000 エントリの辞書構築が 10ms 以内、§18.1）に対応する。
//!
//! 入力には**仕様書のコード例をそのまま**使う（§5.5.7 の恩田恵、§5.6 の星屑の時計、
//! §5.8.2 の index.json。`PLANS.md` §6.3「仕様が正本であることを機械的に担保する」）。

use std::ops::Range;
use std::time::Instant;

use novnow::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use novnow::core::resolve::dictionary::{
    self, Dictionary, Entry, EntryKind, Origin, SourceFile, Target, DELIMITERS, MAX_KEY_BYTES,
};
use novnow::core::syntax::contents::Speaker;
use novnow::core::syntax::{characters_md, materials_index, words_md};

// ============================================================ 小道具

fn origin(file: SourceFile) -> Origin {
    Origin { file, range: None }
}

fn character(name: &str, index: usize) -> Entry {
    Entry::new(
        EntryKind::Character,
        name,
        Target::Character {
            index,
            name: name.to_string(),
        },
        origin(SourceFile::Characters),
    )
}

fn word(term: &str, index: usize) -> Entry {
    Entry::new(
        EntryKind::Word,
        term,
        Target::Word {
            index,
            term: term.to_string(),
        },
        origin(SourceFile::Words),
    )
}

fn material(id: &str) -> Entry {
    Entry::new(
        EntryKind::Material,
        id,
        Target::Material { id: id.to_string() },
        origin(SourceFile::MaterialsIndex),
    )
}

fn material_title(title: &str, id: &str) -> Entry {
    Entry::new(
        EntryKind::MaterialTitle,
        title,
        Target::Material { id: id.to_string() },
        origin(SourceFile::MaterialsIndex),
    )
}

fn dictionary(entries: Vec<Entry>) -> Dictionary {
    Dictionary::build(entries).dictionary
}

/// 1 行を走査して `(コード, 行内相対の範囲)` を返す。
fn found(line: &str, entries: Vec<Entry>) -> Vec<(DiagnosticCode, Range<usize>)> {
    let built = dictionary(entries);
    dictionary::scan_line(line, 0, &built)
        .diagnostics
        .into_iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.range.unwrap_or(0..0)))
        .collect()
}

/// 1 行を走査してトークンだけ返す。
fn tokens(line: &str, entries: Vec<Entry>) -> Vec<dictionary::RefToken> {
    let built = dictionary(entries);
    dictionary::scan_line(line, 0, &built).tokens
}

/// 解決されたエントリのキー（行内相対の範囲と合わせて確認する）。
fn resolved_keys(line: &str, entries: Vec<Entry>) -> Vec<(String, Range<usize>)> {
    let built = dictionary(entries);
    let scan = dictionary::scan_line(line, 0, &built);
    scan.tokens
        .iter()
        .map(|token| {
            let key = token
                .resolved
                .and_then(|id| built.entry(id))
                .map(|entry| entry.key.clone())
                .unwrap_or_default();
            (key, token.range.clone())
        })
        .collect()
}

// ============================================================ §6.1.3 正規化

#[test]
fn normalization_unifies_case_width_long_vowel_and_spaces() {
    // 1. NFKC（全角英数字 → 半角）、2. 小文字化、3. 長音統一、4. 空白除去
    assert_eq!(dictionary::normalize("ＡＢＣ"), "abc");
    assert_eq!(dictionary::normalize("Lily"), "lily");
    // 長音 4 文字（`ー` `－` `—` `〜`）はすべて `ー` になる
    assert_eq!(
        dictionary::normalize("スカーレット－"),
        dictionary::normalize("スカーレット〜")
    );
    assert_eq!(
        dictionary::normalize("スカーレット—"),
        dictionary::normalize("スカーレットー")
    );
    // 空白（半角・全角・タブ）は落ちるので、資料タイトルは空白の有無を問わない
    assert_eq!(
        dictionary::normalize("科学の甲子園 本戦の運営と実技課題"),
        dictionary::normalize("科学の甲子園本戦の運営と実技課題")
    );
    assert_eq!(dictionary::normalize(" 星屑の\t時計　"), "星屑の時計");
    // ASCII のハイフンは長音にしない（`M-01` のような ID を壊さない）
    assert_eq!(dictionary::normalize("M-01"), "m-01");
}

#[test]
fn normalization_never_rewrites_the_source_text() {
    // §6.1.3 MUST「元テキストは一切書き換えない」
    let source = String::from("『'リリアナ』は ＡＢＣ－です。");
    let normalized = dictionary::normalize(&source);
    assert_ne!(normalized, source);
    assert_eq!(source, "『'リリアナ』は ＡＢＣ－です。");
}

#[test]
fn offset_map_maps_normalized_positions_back_to_the_source() {
    let (normalized, map) = dictionary::normalize_scanning("AB ｃｄ");
    assert_eq!(normalized, "abcd");
    // 正規化後の 0..2 は原文の `AB`、2..4 は空白を飛ばした `ｃｄ`
    assert_eq!(map.original_start(0), 0);
    assert_eq!(map.original_end(2), 2);
    // 空白（1 byte）を飛ばすので `ｃ`（3 byte）の原文位置は 3
    assert_eq!(map.original_start(2), 3);
    assert_eq!(map.original_end(4), 9);
    // 何も消費していなければ走査を始めた位置
    assert_eq!(map.original_end(0), 0);
}

// ============================================================ §6.2.2 最短一致

#[test]
fn the_shortest_entry_wins_and_the_longer_one_warns_w029() {
    // 完了条件 2「§6.2.2 の『`'リリアナ` が `リリア` + `ナ` になる』が仕様どおり再現され、
    // W029 が出る」
    let line = "'リリアナは振り返った";
    let scan = found(line, vec![character("リリア", 0), character("リリアナ", 1)]);
    assert_eq!(
        scan,
        vec![(
            DiagnosticCode::W029,
            // `'リリアナ` の**名前部**（`'` を除く）に波線
            1..1 + "リリア".len()
        )]
    );
    // 短い方が勝つ（§6.2.2「辞書に両方があるときは `リリア`（短い方）が選ばれる」）
    assert_eq!(
        resolved_keys(line, vec![character("リリア", 0), character("リリアナ", 1)]),
        vec![("リリア".to_string(), 0..1 + "リリア".len())]
    );
}

#[test]
fn w029_carries_the_longer_key_and_proposes_the_explicit_form() {
    let line = "'リリアナ";
    let built = dictionary(vec![character("リリア", 0), character("リリアナ", 1)]);
    let scan = dictionary::scan_line(line, 0, &built);
    let diagnostic = scan
        .diagnostics
        .iter()
        .find(|item| item.code == DiagnosticCode::W029)
        .expect("W029");
    // §7 W029「より長い項目 {0} にも一致します」
    assert_eq!(diagnostic.args, vec![Arg::text("リリアナ")]);
    // クイックフィックスは明示形 `'{リリアナ}` への書き換え（§6.2.2）
    let token = &scan.tokens[0];
    assert_eq!(token.explicit_form(&built).as_deref(), Some("'{リリアナ}"));
}

#[test]
fn a_longer_key_absent_from_the_dictionary_leaves_the_rest_as_body_text() {
    // §6.2.2 の地文（`リリア` + `ナ`）。W029 は**より長い項目があるときだけ**
    let line = "'リリアナは振り返った";
    assert_eq!(
        found(line, vec![character("リリア", 0)]),
        Vec::<(DiagnosticCode, Range<usize>)>::new()
    );
    assert_eq!(
        resolved_keys(line, vec![character("リリア", 0)]),
        vec![("リリア".to_string(), 0..1 + "リリア".len())]
    );
}

#[test]
fn an_unknown_reference_stops_at_the_delimiter_set_and_is_e001() {
    // §6.2.1 (c)「未解決: 区切り文字までを範囲とし、診断 E001 を付ける」
    let line = "'しらぬ名を呼んだ。";
    let scan = found(line, vec![character("リリア", 0)]);
    assert_eq!(scan.len(), 1);
    assert_eq!(scan[0].0, DiagnosticCode::E001);
    // `'しらぬ名を呼んだ` まで（`。` は区切り）
    assert_eq!(scan[0].1, 0.."しらぬ名を呼んだ".len() + 1);
    let built = dictionary(vec![character("リリア", 0)]);
    let diagnostic = &dictionary::scan_line(line, 0, &built).diagnostics[0];
    // §7 E001「辞書にない参照です: {0}」
    assert_eq!(diagnostic.args, vec![Arg::text("しらぬ名を呼んだ")]);
}

#[test]
fn every_delimiter_in_the_fixed_list_ends_an_unknown_reference() {
    // §6.2.1「未解決時の区切り文字集合（固定）」
    let built = dictionary(vec![character("リリア", 0)]);
    for delimiter in DELIMITERS {
        let line = format!("'しらぬ{delimiter}続き");
        let scan = dictionary::scan_line(&line, 0, &built);
        // 区切りに `'` 自身が含まれるので、2 トークン目ができる場合がある
        assert!(!scan.tokens.is_empty(), "{delimiter:?}");
        let token = &scan.tokens[0];
        assert_eq!(
            token.key(&line),
            "しらぬ",
            "区切り {delimiter:?} の前で止まる"
        );
    }
    // 空白（半角・全角・タブ）も区切り
    for line in ["'しらぬ 続き", "'しらぬ　続き", "'しらぬ\t続き"] {
        let scan = dictionary::scan_line(line, 0, &built);
        assert_eq!(scan.tokens[0].key(line), "しらぬ", "{line}");
    }
}

#[test]
fn a_bare_sigil_is_not_a_reference() {
    // `'` の直後が区切り文字なら参照ではない（E001 の山にしない）
    let built = dictionary(vec![character("リリア", 0)]);
    for line in ["''", "' 「リリア」", "don't stop"] {
        let scan = dictionary::scan_line(line, 0, &built);
        assert!(
            scan.diagnostics.is_empty(),
            "{line}: {:?}",
            scan.diagnostics
        );
    }
    // 英単語の中の `'`（所有格・短縮形）も参照ではない
    let scan = dictionary::scan_line("it's リリア's", 0, &built);
    assert!(scan.diagnostics.is_empty(), "{scan:?}");
}

#[test]
fn a_token_carries_both_the_whole_range_and_the_key_range() {
    // `range` は `'` と修飾子を含み、`key_range` は名前だけ（§6.2.3 / §6.2.4）
    let line = "'リリア:B と '{リリアナ}";
    let found = tokens(line, vec![character("リリア", 0), character("リリアナ", 1)]);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].range, 0.."'リリア:B".len());
    assert_eq!(found[0].key_range, 1.."'リリア".len());
    assert_eq!(found[0].key(line), "リリア");
    assert!(!found[0].explicit);
    // 明示形は波括弧を除いた内側が名前
    assert_eq!(found[1].range, "'リリア:B と ".len()..line.len());
    assert_eq!(found[1].key_range, "'リリア:B と '{".len()..line.len() - 1);
    assert_eq!(found[1].key(line), "リリアナ");
    assert!(found[1].explicit);
    // W029 は最短一致のときだけ（明示形は作家が長さを選んだので出さない）
    assert_eq!(found[1].longer, None);
}

// ============================================================ §6.1.1 優先順位と衝突

#[test]
fn the_priority_order_is_character_word_material_title() {
    // §6.1.1「衝突規則（固定）: `Character > Address > Word > Material > MaterialTitle`」
    let entries = vec![
        material_title("星屑の時計", "M0001"),
        material("M0001"),
        word("星屑の時計", 0),
        character("星屑の時計", 0),
    ];
    let built = dictionary(entries);
    let id = built.lookup("星屑の時計", None).expect("解決できる");
    assert_eq!(
        built.entry(id).map(|entry| entry.kind),
        Some(EntryKind::Character)
    );
    // 登録順を逆にしても結果は同じ（内部で優先順位順に並べ直す）
    let reversed = dictionary(vec![
        character("星屑の時計", 0),
        word("星屑の時計", 0),
        material("M0001"),
        material_title("星屑の時計", "M0001"),
    ]);
    let id = reversed.lookup("星屑の時計", None).expect("解決できる");
    assert_eq!(
        reversed.entry(id).map(|entry| entry.kind),
        Some(EntryKind::Character)
    );
}

#[test]
fn a_key_in_two_dictionaries_warns_w027_on_the_loser() {
    let built = Dictionary::build(vec![word("あおい", 0), character("あおい", 0)]);
    // §6.1.1「衝突したキーには起動時に W027 を出す」
    let codes: Vec<DiagnosticCode> = built
        .diagnostics
        .iter()
        .map(|item| item.diagnostic.code)
        .collect();
    assert_eq!(codes, vec![DiagnosticCode::W027]);
    // §7 W027「同じ名前の項目が複数の辞書にあります: {0}」
    assert_eq!(
        built.diagnostics[0].diagnostic.args,
        vec![Arg::text("あおい")]
    );
    // 勝者は Character（優先順位）
    let id = built.dictionary.lookup("あおい", None).expect("解決できる");
    assert_eq!(
        built.dictionary.entry(id).map(|entry| entry.kind),
        Some(EntryKind::Character)
    );
    // §6.1.1「参照ペインに『別種別の同名項目あり』を併記する」
    assert_eq!(built.dictionary.conflicts(id).len(), 1);
}

#[test]
fn a_repeated_key_inside_one_dictionary_is_not_a_conflict() {
    // 同じ種別の重複は W027 にしない（定義側の重複は E008 で M2 のパーサが出す）
    let built = Dictionary::build(vec![character("あおい", 0), character("あおい", 1)]);
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    // 先に書いたほうが勝つ（トライの終端は上書きしない）
    let id = built.dictionary.lookup("あおい", None).expect("解決できる");
    assert_eq!(
        built
            .dictionary
            .entry(id)
            .and_then(|entry| match &entry.target {
                Target::Character { index, .. } => Some(*index),
                _ => None,
            }),
        Some(0)
    );
    assert!(built.dictionary.conflicts(id).is_empty());
}

// ============================================================ §6.2.1 E028

#[test]
fn a_key_longer_than_96_bytes_is_rejected_with_e028() {
    // §6.2.1「`MAX_KEY_BYTES = 96`（固定）。これを超える辞書キーは登録時に E028 で拒否する」
    let long = "あ".repeat(MAX_KEY_BYTES / 3 + 1);
    assert!(long.len() > MAX_KEY_BYTES);
    let built = Dictionary::build(vec![character(&long, 0), character("リリア", 1)]);
    let codes: Vec<DiagnosticCode> = built
        .diagnostics
        .iter()
        .map(|item| item.diagnostic.code)
        .collect();
    assert_eq!(codes, vec![DiagnosticCode::E028]);
    assert_eq!(built.dictionary.len(), 1, "長いキーは登録されない");
    assert!(built.dictionary.lookup(&long, None).is_none());
    // 96 byte ちょうどは通る
    let edge = "あ".repeat(MAX_KEY_BYTES / 3);
    assert!(edge.len() <= MAX_KEY_BYTES);
    let built = Dictionary::build(vec![character(&edge, 0)]);
    assert!(built.diagnostics.is_empty());
    assert!(built.dictionary.lookup(&edge, None).is_some());
}

#[test]
fn a_long_title_is_measured_after_normalization() {
    // 空白は落ちてから数える（B-57）。空白込みで 96 byte を超えても実体が短ければ通る
    let mut title = String::new();
    for index in 0..20 {
        title.push_str("科学の甲子園");
        if index + 1 < 20 {
            title.push(' ');
        }
    }
    assert!(title.len() > MAX_KEY_BYTES, "書かれた形は 96 byte 超");
    assert!(
        dictionary::normalize(&title).len() > MAX_KEY_BYTES,
        "正規化後も超えるので E028"
    );
    let built = Dictionary::build(vec![material_title(&title, "M0001")]);
    assert_eq!(
        built
            .diagnostics
            .iter()
            .map(|item| item.diagnostic.code)
            .collect::<Vec<_>>(),
        vec![DiagnosticCode::E028]
    );
}

// ============================================================ §6.2.3 明示形

#[test]
fn the_explicit_form_takes_everything_inside_the_braces() {
    // §6.2.3「`'{葵あおい}` のように波括弧で囲むと、その内側全体がキーになる」
    let built = dictionary(vec![character("リリア", 0), character("リリアナ", 1)]);
    let line = "'{リリアナ}が最短一致を上書きする";
    let scan = dictionary::scan_line(line, 0, &built);
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    let token = &scan.tokens[0];
    assert!(token.explicit);
    assert_eq!(token.key(line), "リリアナ");
    assert_eq!(
        token
            .resolved
            .and_then(|id| built.entry(id))
            .map(|e| e.key.clone()),
        Some("リリアナ".to_string())
    );
    // 範囲は `'{リリアナ}` 全体
    assert_eq!(token.range, 0.."'{リリアナ}".len());
}

#[test]
fn an_unclosed_explicit_form_is_e002_to_the_end_of_the_line() {
    // §6.2.3「閉じ括弧が無い場合は E002（行末までを未解決範囲とする）」
    let built = dictionary(vec![character("リリアナ", 0)]);
    let line = "'{リリアナ が閉じていない";
    let scan = dictionary::scan_line(line, 0, &built);
    assert_eq!(scan.diagnostics.len(), 1);
    assert_eq!(scan.diagnostics[0].code, DiagnosticCode::E002);
    assert_eq!(scan.diagnostics[0].range, Some(0..line.len()));
    // E001 は重ねない
    assert!(scan.tokens[0].unclosed);
}

#[test]
fn an_explicit_form_without_an_entry_is_e001() {
    let built = dictionary(vec![character("リリア", 0)]);
    let line = "'{しらぬ}";
    let scan = dictionary::scan_line(line, 0, &built);
    assert_eq!(scan.diagnostics.len(), 1);
    assert_eq!(scan.diagnostics[0].code, DiagnosticCode::E001);
    assert_eq!(scan.diagnostics[0].args, vec![Arg::text("しらぬ")]);
}

// ============================================================ §6.2.4 修飾子

#[test]
fn a_modifier_on_a_character_is_a_mode_expression() {
    let built = dictionary(vec![character("リリア", 0)]);
    for (line, expected) in [
        ("'リリア:B", vec!["B"]),
        ("'リリア:B+D", vec!["B", "D"]),
        ("'{リリア}:B", vec!["B"]),
    ] {
        let scan = dictionary::scan_line(line, 0, &built);
        assert!(
            scan.diagnostics.is_empty(),
            "{line}: {:?}",
            scan.diagnostics
        );
        let token = &scan.tokens[0];
        let modifier = token.modifier.as_ref().unwrap_or_else(|| panic!("{line}"));
        let modes: Vec<&str> = modifier
            .modes()
            .unwrap_or_default()
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(modes, expected, "{line}");
        // 範囲は修飾子まで含む
        assert_eq!(token.range, 0..line.len(), "{line}");
    }
}

#[test]
fn a_modifier_on_a_word_is_e026() {
    // §6.1.1 の表「`Word` …（不可。付ければ **E026**）」
    let built = dictionary(vec![word("星屑の時計", 0)]);
    let line = "'星屑の時計:B";
    let scan = dictionary::scan_line(line, 0, &built);
    assert_eq!(scan.diagnostics.len(), 1);
    assert_eq!(scan.diagnostics[0].code, DiagnosticCode::E026);
    assert_eq!(
        scan.diagnostics[0].range,
        Some("'星屑の時計".len()..line.len())
    );
}

#[test]
fn a_modifier_on_a_material_is_its_title_even_with_spaces() {
    // §6.6.1「`'M0137:科学の甲子園 本戦の運営と実技課題` タイトル明示。index と照合される」
    let built = dictionary(vec![material("M0137")]);
    let line = "'M0137:科学の甲子園 本戦の運営と実技課題";
    let scan = dictionary::scan_line(line, 0, &built);
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    let modifier = scan.tokens[0].modifier.clone().expect("修飾子");
    assert!(modifier.is_title());
    assert_eq!(modifier.text, "科学の甲子園 本戦の運営と実技課題");
    // §6.6.3 の E006 は `modifier.title` の範囲に引くので、値だけの範囲を持つ
    assert_eq!(
        &line[modifier.text_range.clone()],
        "科学の甲子園 本戦の運営と実技課題"
    );
}

#[test]
fn a_colon_inside_a_material_title_is_part_of_the_title() {
    // §6.2.4 の `:` の文脈例外表「資料タイトル修飾子の中の `:` はタイトルの一部」
    let built = dictionary(vec![material("M0137")]);
    let line = "'M0137:取材: 甲子園本戦";
    let scan = dictionary::scan_line(line, 0, &built);
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    let modifier = scan.tokens[0].modifier.clone().expect("修飾子");
    assert_eq!(modifier.text, "取材: 甲子園本戦");
}

#[test]
fn three_modes_in_one_expression_is_e005() {
    // §6.4.4「1 キャラ 1 節につき最大 2 モード（`A+B`）。3 つ以上は E005」
    let built = dictionary(vec![character("リリア", 0)]);
    let line = "'リリア:A+B+C";
    let scan = dictionary::scan_line(line, 0, &built);
    assert_eq!(scan.diagnostics.len(), 1);
    assert_eq!(scan.diagnostics[0].code, DiagnosticCode::E005);
}

#[test]
fn a_malformed_mode_expression_is_e002() {
    let built = dictionary(vec![character("リリア", 0)]);
    for line in ["'リリア:1B", "'リリア:B+"] {
        let scan = dictionary::scan_line(line, 0, &built);
        assert_eq!(scan.diagnostics.len(), 1, "{line}");
        assert_eq!(scan.diagnostics[0].code, DiagnosticCode::E002, "{line}");
    }
}

#[test]
fn a_mode_expression_ends_at_the_delimiter_set() {
    // モード式は §6.2.1 の区切り文字（空白を含む）で終わる（B-62）。
    // 資料タイトルと違い、行末まで飲み込むと `'リリア:B と '{リリアナ}` の 2 つ目が消える
    let built = dictionary(vec![character("リリア", 0), character("リリアナ", 1)]);
    let line = "'リリア:B と '{リリアナ}";
    let scan = dictionary::scan_line(line, 0, &built);
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    assert_eq!(scan.tokens.len(), 2);
    let modes: Vec<&str> = scan.tokens[0]
        .modifier
        .as_ref()
        .and_then(|found| found.modes())
        .unwrap_or_default()
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(modes, vec!["B"]);
    assert_eq!(scan.tokens[0].key(line), "リリア");
    assert_eq!(scan.tokens[1].key(line), "リリアナ");
}

#[test]
fn a_colon_after_an_unknown_reference_is_punctuation_not_a_modifier() {
    // 未解決の参照に `:` が続く場合は修飾子として読まない（E001 だけ）
    let built = dictionary(vec![character("リリア", 0)]);
    let line = "'しらぬ: なんとか";
    let scan = dictionary::scan_line(line, 0, &built);
    assert_eq!(scan.diagnostics.len(), 1);
    assert_eq!(scan.diagnostics[0].code, DiagnosticCode::E001);
    assert!(scan.tokens[0].modifier.is_none());
}

#[test]
fn a_fullwidth_colon_never_starts_a_modifier() {
    // §6.2.4 の EBNF は半角 `:`。本文の `：` は句読点（§6.2.1 の区切り文字集合にもある）
    let built = dictionary(vec![character("リリア", 0)]);
    let line = "'リリア：待って";
    let scan = dictionary::scan_line(line, 0, &built);
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    let token = &scan.tokens[0];
    assert!(token.modifier.is_none());
    assert_eq!(token.range, 0.."'リリア".len());
}

#[test]
fn a_colon_with_nothing_after_it_is_not_a_modifier() {
    let built = dictionary(vec![character("リリア", 0)]);
    for line in ["'リリア:", "'リリア: "] {
        let scan = dictionary::scan_line(line, 0, &built);
        assert!(
            scan.diagnostics.is_empty(),
            "{line}: {:?}",
            scan.diagnostics
        );
        assert!(scan.tokens[0].modifier.is_none(), "{line}");
    }
}

// ============================================================ 走査の範囲

#[test]
fn scan_line_finds_every_reference_in_a_line_in_order() {
    let built = dictionary(vec![
        character("リリア", 0),
        word("星屑の時計", 0),
        material("M0137"),
    ]);
    let line = "『'リリア』が'M0137を見て「'星屑の時計」と言った";
    let scan = dictionary::scan_line(line, 0, &built);
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    let keys: Vec<&str> = scan.tokens.iter().map(|token| token.key(line)).collect();
    assert_eq!(keys, vec!["リリア", "M0137", "星屑の時計"]);
    // §6.3 のトークン種別
    let kinds: Vec<&str> = scan
        .tokens
        .iter()
        .map(|token| token.token_kind(&built))
        .collect();
    assert_eq!(kinds, vec!["ref.character", "ref.material", "ref.word"]);
}

#[test]
fn scan_line_shifts_ranges_by_the_line_start() {
    let built = dictionary(vec![character("リリア", 0)]);
    let line = "'しらぬ";
    let scan = dictionary::scan_line(line, 100, &built);
    // 診断はファイル単位、トークンは行内相対
    assert_eq!(scan.diagnostics[0].range, Some(100..100 + line.len()));
    assert_eq!(scan.tokens[0].range, 0..line.len());
}

// ============================================================ §6.2.5 話者注釈

/// 行末話者注釈の**形だけ**を作る（M2 の `contents::parse` が読むものと同一の型）。
fn speaker(name: &'static str) -> Speaker<'static> {
    Speaker {
        range: 0..name.len() + 1,
        sigil_at: 0,
        name,
        modes: Vec::new(),
    }
}

#[test]
fn a_speaker_annotation_resolves_to_a_character_only() {
    // §6.2.5「行末（`@名前` が行の最後）→ 話者注釈。辞書の `Character` を最短一致」
    let built = dictionary(vec![character("リリア", 0), word("リリア", 0)]);
    let id = dictionary::resolve_speaker(&speaker("リリア"), &built).expect("解決できる");
    assert_eq!(
        built.entry(id).map(|entry| entry.kind),
        Some(EntryKind::Character),
        "用語ではなくキャラに解決する"
    );
    // §6.1.1「`Address` は**話者注釈 `@` の解決対象外**（MUST）」— ① では Address が
    // 1 件も無いので、Word だけの場合も E012 のまま
    let words_only = dictionary(vec![word("あおいちゃん", 0)]);
    assert!(dictionary::resolve_speaker(&speaker("あおいちゃん"), &words_only).is_none());
}

#[test]
fn an_unknown_speaker_is_e012() {
    let built = dictionary(vec![character("リリア", 0)]);
    let diagnostic = dictionary::speaker_diagnostic(&speaker("しらぬ"), &built).expect("E012");
    assert_eq!(diagnostic.code, DiagnosticCode::E012);
    // §7 E012「登場人物 {0} は characters.md にいません」
    assert_eq!(diagnostic.args, vec![Arg::text("しらぬ")]);
    // 解決できる話者には診断が出ない
    assert!(dictionary::speaker_diagnostic(&speaker("リリア"), &built).is_none());
}

// ============================================================ §6.1.2 Aho-Corasick

#[test]
fn word_occurrences_are_found_without_the_reference_sigil() {
    // §6.1.2「用語の本文中出現検出（参照ペインの『この節に出てくる用語』）は
    // Aho-Corasick を併用する」
    let built = dictionary(vec![
        word("星屑の時計", 0),
        word("魔導具", 1),
        character("リリア", 0),
    ]);
    let text = "『'星屑の時計』は魔導具だ。星屑時計は別名でも出る。";
    let found = built.occurrences(text);
    let ranges: Vec<(String, Range<usize>)> = found
        .iter()
        .map(|item| {
            (
                built
                    .entry(item.entry)
                    .map(|entry| entry.key.clone())
                    .unwrap_or_default(),
                item.range.clone(),
            )
        })
        .collect();
    assert_eq!(
        ranges,
        vec![
            (
                "星屑の時計".to_string(),
                text.find("星屑の時計").unwrap()
                    ..text.find("星屑の時計").unwrap() + "星屑の時計".len()
            ),
            (
                "魔導具".to_string(),
                text.find("魔導具").unwrap()..text.find("魔導具").unwrap() + "魔導具".len()
            ),
        ]
    );
    // 別名は正規化で空白が落ちるので `星屑時計` にも当たる（別エントリとして登録していれば）
    let built = dictionary(vec![word("星屑時計", 0)]);
    assert_eq!(built.occurrences("星屑の時計はここ").len(), 0);
    assert_eq!(built.occurrences("星屑時計はここ").len(), 1);
}

#[test]
fn occurrences_of_a_dictionary_without_words_are_empty() {
    let built = dictionary(vec![character("リリア", 0)]);
    assert!(built.occurrences("リリアと魔導具").is_empty());
}

// ============================================================ §6.1.1 供給元（3 ファイル）

/// §5.5.7 の例（恩田恵）＋ relations の解決に必要な 2 人。
const CHARACTERS_MD: &str = "# 恩田恵
reading: おんだめぐみ
alias: めぐみ, 恵
role: メインヒロイン
default-mode: A

## tone
「*で…」「*です…」「…あの…*」

## modes

### A 日常
range: @01 ..
personality: おとなしく控えめ。'葵あおい の後ろに隠れていることが多い。

### C 混乱・言語化障害
range: @01 .. @02

## relations
'葵あおい: 魂のパートナー。
'瀬戸晴香: 技術の師弟関係。

# 葵あおい
alias: あおい

# 瀬戸晴香
";

/// `## relations` に解決できない参照がある例（§5.5.6）。
const RELATIONS_MD: &str = "# 恩田恵

## relations
'葵あおい: 魂のパートナー。
'しらぬ人: 誰？
";

/// §5.6 の例（星屑の時計）。
const WORDS_MD: &str = "# 星屑の時計
reading: ほしくずのとけい
category: 魔導具
alias: 星屑時計
definition: 旧校舎の地下遺跡に眠るとされる幻の魔導具。
range: @01 ..
";

/// §5.8.2 の例（`items` は 1 件に絞ったもの）。
const INDEX_JSON: &str = r#"{
  "schema_version": 1,
  "next_id": 138,
  "items": [
    {
      "id": "M0137",
      "title": "科学の甲子園 本戦の運営と実技課題",
      "path": "materials/docs/M0137-kagaku-koshien.md",
      "status": "approved"
    }
  ]
}
"#;

#[test]
fn from_sources_reads_all_three_files() {
    let characters = characters_md::parse(CHARACTERS_MD);
    let words = words_md::parse(WORDS_MD);
    let index = materials_index::parse(INDEX_JSON);
    let built = Dictionary::from_sources(&characters, &words, &index);
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);

    let dictionary = &built.dictionary;
    // H1 と alias の両方が登録される（§5.5.3「すべて辞書に登録される」）
    for key in ["恩田恵", "めぐみ", "恵", "葵あおい", "あおい", "瀬戸晴香"] {
        let id = dictionary
            .lookup(key, None)
            .unwrap_or_else(|| panic!("{key}"));
        assert_eq!(
            dictionary.entry(id).map(|entry| entry.kind),
            Some(EntryKind::Character),
            "{key}"
        );
    }
    // 用語（H1 と alias）
    for key in ["星屑の時計", "星屑時計"] {
        let id = dictionary
            .lookup(key, None)
            .unwrap_or_else(|| panic!("{key}"));
        assert_eq!(
            dictionary.entry(id).map(|entry| entry.kind),
            Some(EntryKind::Word),
            "{key}"
        );
    }
    // 資料の ID とタイトル
    let id = dictionary.lookup("M0137", None).expect("ID");
    assert_eq!(
        dictionary.entry(id).map(|entry| entry.kind),
        Some(EntryKind::Material)
    );
    let id = dictionary
        .lookup("科学の甲子園 本戦の運営と実技課題", None)
        .expect("タイトル");
    assert_eq!(
        dictionary.entry(id).map(|entry| entry.kind),
        Some(EntryKind::MaterialTitle)
    );
    assert_eq!(
        dictionary.entry(id).map(|entry| entry.target.clone()),
        Some(Target::Material {
            id: "M0137".to_string()
        }),
        "タイトルは ID に正規化する（§6.6.1）"
    );
    // 別名で引いても表示名が返る
    assert_eq!(
        dictionary
            .lookup("めぐみ", None)
            .and_then(|id| match &dictionary.entry(id)?.target {
                Target::Character { name, .. } => Some(name.clone()),
                _ => None,
            }),
        Some("恩田恵".to_string())
    );
}

#[test]
fn from_sources_points_diagnostics_at_the_right_file_and_offset() {
    let characters = characters_md::parse(CHARACTERS_MD);
    let words = words_md::parse(WORDS_MD);
    let index = materials_index::parse(INDEX_JSON);
    let built = Dictionary::from_sources(&characters, &words, &index);
    let dictionary = &built.dictionary;

    // エントリの出所はファイル単位の byte 範囲（§7.2）
    let id = dictionary.lookup("めぐみ", None).expect("alias");
    let range = dictionary
        .entry(id)
        .and_then(|entry| entry.origin.range.clone())
        .expect("範囲");
    assert_eq!(
        CHARACTERS_MD.get(range.clone()),
        Some("めぐみ"),
        "alias の位置: {range:?}"
    );
    assert_eq!(
        dictionary.entry(id).map(|entry| entry.origin.file),
        Some(SourceFile::Characters)
    );
    // 資料の ID は index.json の中の位置
    let id = dictionary.lookup("M0137", None).expect("id");
    let range = dictionary
        .entry(id)
        .and_then(|entry| entry.origin.range.clone())
        .expect("範囲");
    assert_eq!(INDEX_JSON.get(range.clone()), Some("M0137"), "{range:?}");
    assert_eq!(
        dictionary.entry(id).map(|entry| entry.origin.file),
        Some(SourceFile::MaterialsIndex)
    );
}

#[test]
fn from_sources_warns_w027_across_files() {
    // `characters.md` のキャラ名と `words.md` の用語が同じ（§6.1.1 の衝突）
    let characters = characters_md::parse("# 星屑の時計\n");
    let words = words_md::parse(WORDS_MD);
    let index = materials_index::parse(INDEX_JSON);
    let built = Dictionary::from_sources(&characters, &words, &index);
    let codes: Vec<(SourceFile, DiagnosticCode)> = built
        .diagnostics
        .iter()
        .map(|item| (item.file, item.diagnostic.code))
        .collect();
    assert_eq!(codes, vec![(SourceFile::Words, DiagnosticCode::W027)]);
    // 勝者はキャラ（`Character > Word`）
    let id = built
        .dictionary
        .lookup("星屑の時計", None)
        .expect("解決できる");
    assert_eq!(
        built.dictionary.entry(id).map(|entry| entry.kind),
        Some(EntryKind::Character)
    );
}

#[test]
fn from_sources_survives_a_broken_index_json() {
    // §4.3-4「パース失敗を起動失敗にしない」。E025 は M2 が出す（ここは繰り返さない）
    let characters = characters_md::parse(CHARACTERS_MD);
    let words = words_md::parse(WORDS_MD);
    let index = materials_index::parse("{ 壊れている");
    let built = Dictionary::from_sources(&characters, &words, &index);
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert!(built.dictionary.lookup("M0137", None).is_none());
    assert!(built.dictionary.lookup("恩田恵", None).is_some());
}

#[test]
fn relations_that_do_not_resolve_are_e013() {
    // §5.5.6 `'参照: 説明` → 実体が無ければ E013
    let characters = characters_md::parse(RELATIONS_MD);
    let words = words_md::parse("");
    let index = materials_index::parse("{}");
    let built = Dictionary::from_sources(&characters, &words, &index);
    // 葵あおい が辞書にいないので両方 E013
    let found = dictionary::relation_diagnostics(&characters, &built.dictionary);
    assert_eq!(found.len(), 2);
    assert!(found
        .iter()
        .all(|item| item.diagnostic.code == DiagnosticCode::E013));
    assert_eq!(
        found[1].diagnostic.args,
        vec![Arg::text("しらぬ人")],
        "§7 E013 の引数は参照先の名前"
    );
    // 位置は characters.md のファイル範囲
    let range = found[1].diagnostic.range.clone().expect("範囲");
    // 位置は `'` を含む参照トークン（M2 の `Relation::range` と同じ）
    assert_eq!(&RELATIONS_MD[range], "'しらぬ人");

    // 実体があれば診断は出ない
    let characters = characters_md::parse(CHARACTERS_MD);
    let built = Dictionary::from_sources(
        &characters,
        &words_md::parse(WORDS_MD),
        &materials_index::parse(INDEX_JSON),
    );
    assert!(dictionary::relation_diagnostics(&characters, &built.dictionary).is_empty());
}

// ============================================================ §18.1 性能

#[test]
fn building_5000_entries_stays_cheap() {
    // 完了条件 4「5,000 エントリの辞書構築が 10ms 以内（§18.1）」。
    // **実測**（2 コアの作業環境、エントリ作り＝§6.1.3 の正規化を含む）:
    // release 5.3ms / debug 20.3ms → 目標の 10ms 以内（release）。
    // CI のマシンは 2 コアで揺れ、`cargo test` は debug なので、ここでは**桁の退行**
    // （O(n²)）を捕まえるための緩い上限で見る（`tests/incremental.rs` と同じ流儀）。
    let started = Instant::now();
    // エントリ作り（§6.1.3 の正規化を含む）から測る。`from_sources` が実際にやる仕事なので
    let entries: Vec<Entry> = (0..5_000)
        .map(|index| character(&format!("なまえ{index:04}のキャラ"), index))
        .collect();
    let built = Dictionary::build(entries);
    let elapsed = started.elapsed();
    assert_eq!(built.dictionary.len(), 5_000);
    assert!(elapsed.as_millis() < 500, "build: {elapsed:?}");
    // スキャンも 1 行あたり線形であること（§6.7 の 300ms アイドル再計算の前提）
    let dictionary = &built.dictionary;
    let line = "'なまえ4999のキャラが登場する";
    let started = Instant::now();
    for _ in 0..1_000 {
        let scan = dictionary::scan_line(line, 0, dictionary);
        assert!(scan.tokens[0].is_resolved());
    }
    let elapsed = started.elapsed();
    assert!(elapsed.as_millis() < 500, "scan 1000: {elapsed:?}");
}

#[test]
fn an_empty_dictionary_resolves_nothing_and_panics_on_nothing() {
    let built = Dictionary::empty();
    assert!(built.is_empty());
    assert_eq!(built.len(), 0);
    for line in ["'リリア", "'{リリア}", "@リリア", "普通の文", ""] {
        let scan = dictionary::scan_line(line, 0, &built);
        assert!(
            scan.diagnostics
                .iter()
                .all(|item| item.code == DiagnosticCode::E001),
            "{line}"
        );
    }
    assert!(built.lookup("リリア", None).is_none());
    assert!(built.occurrences("リリア").is_empty());
}

/// `Diagnostic` の引数をそのまま比べられるようにするための確認（型の見た目テスト）。
#[test]
fn diagnostics_are_plain_data() {
    let diagnostic = Diagnostic::new(DiagnosticCode::E001, Some(0..3), vec![Arg::text("x")]);
    assert_eq!(diagnostic.range, Some(0..3));
    assert_eq!(diagnostic.args.len(), 1);
}
