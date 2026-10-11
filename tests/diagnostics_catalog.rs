//! `core::diagnostics` の表を `SPECIFICATIONS.md` §7 と突き合わせるテスト。
//!
//! §7（36 件）・§7.1（14 件）・§7.2（27 件）の **77 件**を仕様書の Markdown から直接
//! 読み直し、`CATALOG` と一致するかを見る。`PLANS.md` §6.2 では `tests/diagnostics_catalog.rs`
//! の導入は M4 だが、§3 I5「診断は先に表を実装」に合わせて**前倒し**した。表を先に作る以上、
//! 仕様書との一致も先に固定しておく（後から 77 件を照合するのは手でできない）。
//!
//! 確認するのは次の 7 つ。
//!
//! 1. 3 つの表の行数と、`CATALOG` の行数・コードの集合が一致する。
//! 2. 重大度（`Sev` 列）がコードの 1 文字目と一致する。
//! 3. 引数の数が ja 文言のプレースホルダと一致し、ja と en のプレースホルダが同じ。
//! 4. `CATALOG` が重大度 → 番号の順で、重複が無い。
//! 5. variant の doc コメントが §7 の「条件」列を書き写している。
//! 6. §7.1 の全件と §7.2 の E032〜E036 が `Anchor::Status`（仕様書の明記）。
//! 7. §7.2「既定の有効 / 無効」と「実行コストの制御（MUST）」の表が実装と一致。
//!
//! 仕様書と実装はどちらも `include_str!` で埋め込む。`tests/layering.rs` と同じ理由で、
//! テスト内でも `std::fs` を使わない（フィクスチャは埋め込むか `tests/golden/**`）。

use std::collections::HashSet;

use novnow::core::diagnostics::{
    Anchor, Arg, Diagnostic, DiagnosticCode, Group, Scope, Severity, Toggle, Trigger, CATALOG,
    COST, STATUS_ANCHOR, TOGGLE_OFF, TOGGLE_ON,
};

/// 仕様書そのもの（正本）。
const SPEC: &str = include_str!("../SPECIFICATIONS.md");

/// 実装そのもの。variant の doc コメントが §7 の「条件」列と一致するかを見るのに使う。
const SOURCE: &str = include_str!("../src/core/diagnostics.rs");

/// §7 の表の 1 行。
#[derive(Debug)]
struct Row {
    /// どの表に載っているか。
    group: Group,
    /// コード（`E001`）。
    code: String,
    /// `Sev` 列（`E` / `W` / `I`）。
    sev: char,
    /// 「条件」列。
    cond: String,
    /// ja 文言（backquote を外した形）。
    ja: String,
    /// en 文言（backquote を外した形）。
    en: String,
}

#[test]
fn the_spec_has_77_codes_in_three_tables() {
    let rows = all_rows();
    let core = count_group(&rows, Group::Core);
    let storage = count_group(&rows, Group::Storage);
    let writing = count_group(&rows, Group::Writing);
    assert_eq!(core, 36);
    assert_eq!(storage, 14);
    assert_eq!(writing, 27);
    assert_eq!(rows.len(), 77);
    assert_eq!(CATALOG.len(), rows.len());
}

#[test]
fn every_spec_row_is_in_the_catalog() {
    for row in all_rows() {
        let code = lookup(&row.code);
        assert_eq!(code.severity(), severity_of(row.sev), "{}", row.code);
        assert_eq!(code.group(), row.group, "{}", row.code);
        let want = params(&row.ja);
        assert_eq!(usize::from(code.params()), want, "{}", row.code);
    }
}

#[test]
fn the_catalog_has_no_code_the_spec_does_not_list() {
    let rows = all_rows();
    for row in CATALOG {
        let text = row.0.as_str();
        let listed = rows.iter().any(|spec| spec.code == text);
        assert!(listed, "{text}");
    }
}

#[test]
fn the_catalog_is_sorted_and_unique() {
    let mut seen = HashSet::new();
    let mut previous: Option<DiagnosticCode> = None;
    for row in CATALOG {
        let code = row.0;
        assert!(seen.insert(code.as_str()), "{}", code.as_str());
        assert_eq!(code.params(), row.1, "{}", code.as_str());
        assert_eq!(DiagnosticCode::parse(code.as_str()), Some(code));
        if let Some(before) = previous {
            assert!(before < code, "{}", code.as_str());
        }
        previous = Some(code);
    }
    assert_eq!(seen.len(), CATALOG.len());
}

#[test]
fn code_strings_are_well_formed() {
    for row in CATALOG {
        let code = row.0;
        let text = code.as_str();
        assert!(is_code(text), "{text}");
        let head = text.chars().next().expect("head");
        assert_eq!(head, letter_of(code.severity()), "{text}");
        let number: u16 = text[1..].parse().expect("number");
        assert_eq!(code.number(), number, "{text}");
    }
}

#[test]
fn ja_and_en_use_the_same_placeholders() {
    // 文言は `ui/i18n/*` が持つ（§12.2）ので、core が知るべきは**引数の数**だけ。
    // ja と en で数がずれると同じ `args` ではどちらかの言語が壊れる
    for row in all_rows() {
        let ja = placeholders(&row.ja);
        let en = placeholders(&row.en);
        assert_eq!(ja, en, "{}", row.code);
    }
}

#[test]
fn e004_skips_the_first_placeholder() {
    // `PLANS.md` §9.1 A-7: E004 の文言は ja も en も `{0}` を使わない（`{1}`〜`{3}`）。
    // 引数の数は「最大の添字 + 1」なので 4 になる。番号を振り直すときは仕様書から直す
    let rows = all_rows();
    let row = rows.iter().find(|row| row.code == "E004").expect("E004");
    let ja = placeholders(&row.ja);
    assert_eq!(ja, vec![1, 2, 3]);
    assert_eq!(placeholders(&row.en), ja);
    let count = DiagnosticCode::E004.params();
    assert_eq!(count, 4);
}

#[test]
fn every_variant_documents_the_condition_from_the_spec() {
    // 条件は core に置ける唯一の情報（文言は §12.2 で core に置けない）なので、
    // 仕様書の「条件」列と食い違ったら困る
    for row in all_rows() {
        let variant = format!("\n    {},\n", row.code);
        let at = SOURCE.find(&variant).expect("variant");
        let head = &SOURCE[..at];
        let doc = head.lines().next_back().expect("doc");
        assert!(doc.trim_start().starts_with("///"), "{doc}");
        assert!(doc.contains(&row.cond), "{}: {doc}", row.code);
    }
}

#[test]
fn the_storage_table_has_no_document_position() {
    // §7.1「文書位置を持たない通知」＋ §7.2 の注「E032〜E036 はエラーだが文書位置を持たない」
    for row in all_rows() {
        let code = lookup(&row.code);
        if stated_as_status(&row) {
            assert_eq!(code.anchor(), Anchor::Status, "{}", row.code);
            assert!(STATUS_ANCHOR.contains(&code), "{}", row.code);
        }
    }
    // 表と `anchor()` が食い違っていないか（表が正）
    for code in STATUS_ANCHOR {
        let anchor = code.anchor();
        assert_eq!(anchor, Anchor::Status, "{}", code.as_str());
    }
}

#[test]
fn the_default_toggles_match_the_spec() {
    let on = codes_on_the_line("v1 の既定で **有効** なのは");
    let off = codes_on_the_line("は **既定で無効** とし");
    assert_eq!(on.len(), 9);
    assert_eq!(off.len(), 7);
    for text in &on {
        let code = lookup(text);
        assert_eq!(code.toggle(), Toggle::OnByDefault, "{text}");
        assert!(TOGGLE_ON.contains(&code), "{text}");
        assert!(code.enabled_by_default(), "{text}");
    }
    for text in &off {
        let code = lookup(text);
        assert_eq!(code.toggle(), Toggle::OffByDefault, "{text}");
        assert!(TOGGLE_OFF.contains(&code), "{text}");
        assert!(!code.enabled_by_default(), "{text}");
    }
    // 仕様が名指ししない W085・W086 は `PLANS.md` §9.2 B-26 の判断で有効側に入れている
    assert_eq!(TOGGLE_ON.len(), on.len() + 2);
    assert!(TOGGLE_ON.contains(&DiagnosticCode::W085));
    assert!(TOGGLE_ON.contains(&DiagnosticCode::W086));
    // §7 と §7.1 はトグルの対象外（仕様にトグルの記述が無い）
    for row in all_rows() {
        if row.group == Group::Writing {
            continue;
        }
        let code = lookup(&row.code);
        assert_eq!(code.toggle(), Toggle::Always, "{}", row.code);
        assert!(code.enabled_by_default(), "{}", row.code);
    }
    // 有効と無効は交わらない
    for code in TOGGLE_OFF {
        assert!(!TOGGLE_ON.contains(code), "{}", code.as_str());
    }
}

#[test]
fn the_cost_table_matches_the_spec() {
    // §7.2「実行コストの制御（MUST）」。範囲は MUST なので、`Scope::Section` の診断を
    // プロジェクト全体で計算してはいけない
    assert_eq!(COST.len(), 16);
    for row in COST {
        let text = row.0.as_str();
        let line = cost_line(text);
        assert!(line.contains(trigger_text(row.1)), "{text}: {line}");
        assert!(line.contains(scope_text(row.2)), "{text}: {line}");
    }
    // 逆方向: 表に載っているコードはすべて `COST` にある
    for text in cost_table_codes() {
        let code = lookup(&text);
        assert!(code.cost().is_some(), "{text}");
        assert_ne!(code.trigger(), Trigger::Unspecified, "{text}");
        assert_ne!(code.scope(), Scope::Unspecified, "{text}");
    }
}

#[test]
fn a_diagnostic_carries_only_the_code_and_the_arguments() {
    // §7.2「診断の実装規則（MUST）}: `core::diagnostics` はコードと引数だけを返す」
    let args = vec![Arg::number(140)];
    let made = Diagnostic::new(DiagnosticCode::W019, Some(0..12), args.clone());
    assert_eq!(made.code, DiagnosticCode::W019);
    assert_eq!(made.args, args);
    assert_eq!(made.range, Some(0..12));
    assert_eq!(made.severity(), Severity::Warning);
    assert_eq!(made.anchor(), Anchor::Document);
    assert!(made.is_underlined());
    // `要約が長すぎます（{0} 字）` なので引数は 1 つ
    let want = DiagnosticCode::W019.params();
    assert_eq!(want, 1);
    assert_eq!(args.len(), usize::from(want));
}

#[test]
fn a_notice_has_no_range_and_no_underline() {
    // I051（未保存の変更）は §7 の表に載っているが文書位置を持たない通知
    let notice = Diagnostic::simple(DiagnosticCode::I051, None);
    assert!(notice.args.is_empty());
    assert_eq!(notice.range, None);
    assert_eq!(notice.anchor(), Anchor::Status);
    assert!(!notice.is_underlined());
    // W012（空の節）は本文中に波線を引く
    let empty = Diagnostic::simple(DiagnosticCode::W012, Some(4..9));
    assert!(empty.args.is_empty());
    assert_eq!(empty.anchor(), Anchor::Document);
    assert!(empty.is_underlined());
    // I は下線なし（§7「I=情報（下線なし、レビュー一覧のみ）」）
    let info = Diagnostic::simple(DiagnosticCode::I050, None);
    assert_eq!(info.severity(), Severity::Info);
    assert!(!info.is_underlined());
}

// ---------------------------------------------------------------- 仕様書の読み直し

/// コード文字列から enum を引く。無ければテストを落とす（どのコードかはメッセージに出す）。
fn lookup(code: &str) -> DiagnosticCode {
    match DiagnosticCode::parse(code) {
        Some(found) => found,
        None => panic!("{code} が CATALOG に無い"),
    }
}

/// 3 つの表をまとめて読む。
fn all_rows() -> Vec<Row> {
    let mut rows = rows_of(Group::Core, "## 7. 診断カタログ", "### 7.1");
    rows.extend(rows_of(Group::Storage, "### 7.1", "### 7.2"));
    rows.extend(rows_of(Group::Writing, "### 7.2", "## 8. 編集モデルと保存"));
    rows
}

fn count_group(rows: &[Row], group: Group) -> usize {
    rows.iter().filter(|row| row.group == group).count()
}

/// マーカー 2 つの間の部分を返す。
fn section(start: &str, end: &str) -> &'static str {
    let from = SPEC.find(start).expect("start marker");
    let rest = &SPEC[from..];
    let to = rest.find(end).expect("end marker");
    &rest[..to]
}

/// 1 つの表を行に分解する。区切り行（`|---|`）と見出し行はコード形でないので落ちる。
fn rows_of(group: Group, start: &str, end: &str) -> Vec<Row> {
    let mut out = Vec::new();
    for line in section(start, end).lines() {
        if !line.starts_with('|') {
            continue;
        }
        let cells = split_cells(line);
        if cells.len() != 8 || !is_code(&cells[1]) {
            continue;
        }
        let sev = cells[2].chars().next().expect("sev");
        out.push(Row {
            group,
            code: cells[1].clone(),
            sev,
            cond: cells[3].clone(),
            ja: unquote(&cells[4]),
            en: unquote(&cells[5]),
        });
    }
    out
}

/// 表の行を `|` で割る（先頭と末尾に空のセルができるので 6 列の行は 8 要素）。
fn split_cells(line: &str) -> Vec<String> {
    line.split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// 文言のセルを囲む backquote を外す。
fn unquote(cell: &str) -> String {
    assert!(cell.starts_with('`'), "{cell}");
    assert!(cell.ends_with('`'), "{cell}");
    cell[1..cell.len() - 1].to_string()
}

/// コード形（`E001`）かどうか。
fn is_code(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 4 {
        return false;
    }
    let head = matches!(bytes[0], b'E' | b'W' | b'I');
    let digits = bytes[1..].iter().all(u8::is_ascii_digit);
    head && digits
}

/// `{0}` 形式のプレースホルダの添字（重複なし・昇順）。`{` が閉じていなければ無視する。
fn placeholders(message: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for part in message.split('{').skip(1) {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            continue;
        }
        if !part[digits.len()..].starts_with('}') {
            continue;
        }
        let value: usize = digits.parse().expect("digits");
        if !out.contains(&value) {
            out.push(value);
        }
    }
    out.sort_unstable();
    out
}

/// 引数の数 = 最大の添字 + 1。
fn params(message: &str) -> usize {
    placeholders(message).last().map_or(0, |value| value + 1)
}

/// マーカーを含む行から、コード形（`E001` 等）の語を全部拾う。
fn codes_on_the_line(marker: &str) -> Vec<String> {
    let found = SPEC.lines().find(|line| line.contains(marker));
    let line = found.expect("marker");
    let mut out = Vec::new();
    for word in line.split(|c: char| !c.is_ascii_alphanumeric()) {
        if is_code(word) {
            out.push(word.to_string());
        }
    }
    out
}

/// §7.2 の注「E032〜E036 はエラーだが文書位置を持たないため、§7.1 と同じく
/// ステータスバー通知として扱う」が名指しするコード。
const STATED_AS_STATUS: &[&str] = &["E032", "E033", "E034", "E035", "E036"];

/// §7.1 の全件と §7.2 の E032〜E036 は「文書位置を持たない」と仕様書が明記している。
fn stated_as_status(row: &Row) -> bool {
    if row.group == Group::Storage {
        return true;
    }
    STATED_AS_STATUS.contains(&row.code.as_str())
}

/// §7.2「実行コストの制御（MUST）」以降の部分。
fn cost_section() -> &'static str {
    let writing = section("### 7.2", "## 8. 編集モデルと保存");
    let marker = "**実行コストの制御（MUST）**";
    let at = writing.find(marker).expect("cost table");
    &writing[at..]
}

/// コードが載っている行を返す（`W081 / W087 / …` のように 1 行に複数コードが来る）。
fn cost_line(code: &str) -> String {
    for line in cost_section().lines() {
        if !line.starts_with('|') {
            continue;
        }
        let cells = split_cells(line);
        if cells.len() < 4 {
            continue;
        }
        let mut listed = cells[1].split(['・', '/']);
        if listed.any(|item| item.trim() == code) {
            return line.to_string();
        }
    }
    panic!("no cost row for {code}")
}

/// 実行コスト表に載っているコードを全部拾う。
fn cost_table_codes() -> Vec<String> {
    let mut out = Vec::new();
    for line in cost_section().lines() {
        if !line.starts_with('|') {
            continue;
        }
        let cells = split_cells(line);
        if cells.len() < 4 {
            continue;
        }
        for word in cells[1].split(['・', '/']) {
            if is_code(word.trim()) {
                out.push(word.trim().to_string());
            }
        }
    }
    out
}

/// 実行タイミングの列の文言（§7.2 の表そのまま）。
fn trigger_text(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Unspecified => "",
        Trigger::Editing => "編集中（アイドル 300ms）",
        Trigger::Idle => "明示操作またはアイドル 5 秒",
        Trigger::Startup => "起動時と明示操作",
        Trigger::Event => "イベント発生時",
    }
}

/// 範囲の列の文言（§7.2 の表そのまま）。
fn scope_text(scope: Scope) -> &'static str {
    match scope {
        Scope::Unspecified => "",
        Scope::Section => "編集中の節のみ",
        Scope::Scene => "編集中の場面",
        Scope::Project => "プロジェクト全体",
        Scope::Session => "セッション",
    }
}

fn severity_of(letter: char) -> Severity {
    match letter {
        'E' => Severity::Error,
        'W' => Severity::Warning,
        _ => Severity::Info,
    }
}

fn letter_of(severity: Severity) -> char {
    match severity {
        Severity::Error => 'E',
        Severity::Warning => 'W',
        Severity::Info => 'I',
    }
}
