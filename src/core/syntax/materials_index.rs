//! `materials/index.json`（§5.8.2 スキーマ v1）のパーサ。
//!
//! §5.5.1「人間が直接書く情報は MD、**機械が採番・管理する情報は JSON**
//! （`materials/index.json` のみ）」。なのでこの形式だけは JSON そのものがストレージで、
//! 木は [`crate::core::model::material`] の型（`docs/history.md` B-30）。
//!
//! # round-trip（`docs/history.md` の完了条件 2）
//!
//! **原文をそのまま [`Index::raw`] に持つ**ことでバイト一致を満たす。serde_json の
//! 再シリアライズはキー順と空白を変えるので「読んで書き戻す」の一致には使えない
//! （`preserve_order` は `indexmap` を連れてくるので §3.2 の「新クレートは仕様更新が先」に触れる）。
//! 意味的な round-trip（パース → シリアライズ → パースで同じ木）はテストが担保する。
//!
//! # 診断（M2 で出すもの）
//!
//! | コード | 条件 | 位置 |
//! |---|---|---|
//! | **E024** | `path` が `materials/` 配下でない（§5.8.2） | 原文中の `path` の値のリテラル |
//! | **E008** | `id` が同一ファイル内で重複（§5.8.2「単調増加・再利用禁止」） | 2 件目の `id` のリテラル |
//! | **E025** | JSON ファイルがパースできない（§4.3-4・§5.8.2） | ファイル全体 |
//! | **W065** | 出典の無い**提案**（`status: proposed`、§16.7。M8、`docs/history.md` B-134） | その資料の `id` のリテラル |
//!
//! JSON のトークン位置は serde が持たないので、位置は**値の JSON リテラルを原文から探して**
//! 決める（[`literal_ranges`]）。同じ文字列が何度も出てくる場合は書いた順に当てはめるだけで、
//! いつも「その値が実際に書かれている場所」を指す。
//!
//! **E025**（JSON として読めない）のときも**パニックしない**（§4.3-4「パース失敗を起動失敗に
//! しない」）。診断を出したうえで [`Index::index_or_default`] の空索引で続行し、
//! **原文は 1 バイトも書き換えない**（MUST、§5.8.2）。E025 が出たときは
//! E024 / E008 は報告しない（パースできていないので検査のしようがない）。
//!
//! # M2 で出さない診断
//!
//! - `title` の長さ（1〜80 字）・`summary`（≤120 字）・`saved_at`（RFC 3339）・
//!   `status` / `source.kind` / `conditions[].kind` の未知の値: 対応コードが §7 に無いので
//!   診断しない（B-39 の流儀）。値は文字列のまま保持され、`*_kind()` が `None` を返す。
//! - `next_id` が既存 ID より小さい（再利用が起きる状態）: 対応コードが無いので
//!   [`MaterialIndex::is_next_id_valid`] が `false` を返すだけ（保存を直すのは M5）。
//! - **W025**（資料 MD の H1 と index の不一致、§5.8.3）・index に未登録の MD（**W064**）:
//!   2 つのファイルを比べるので資料 MD の側で出す（`core::diagnostics::document`。W064 は M8）。
//! - **E034**（カスタムロール 16 件超）と同じく、ディレクトリを見る検査は M5 以降。

use std::ops::Range;

use serde::Serialize;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::model::material::{Material, MaterialId, MaterialIndex, MaterialStatus};

/// `materials/index.json` 1 ファイル。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Index<'a> {
    /// **書かれたままの原文**。[`Index::to_text`] はこれをそのまま返す（バイト一致）。
    pub raw: &'a str,
    /// パースできた木。JSON として読めなければ `None`（[`Index::error`] を見る）。
    pub index: Option<MaterialIndex>,
    /// JSON として読めなかった理由（serde のメッセージ）。
    ///
    /// 同じ内容は **E025** の引数 `{0}` にも入る（§7）。`error` は呼び出し側が
    /// ログに書きやすいよう生の文字列でも持つ。
    pub error: Option<String>,
    /// 診断（§7）。
    pub diagnostics: Vec<Diagnostic>,
}

/// `materials/index.json` のテキストをパースする。
///
/// JSON として壊れていても**パニックしない**（`docs/history.md` の完了条件 3）:
/// **E025**（§4.3-4・§7）を出し、[`Index::error`] に理由を入れて木を `None` で返す。
/// 呼び出し側は [`Index::index_or_default`] で既定値のまま続行する。
pub fn parse(text: &str) -> Index<'_> {
    match serde_json::from_str::<MaterialIndex>(text) {
        Ok(index) => {
            let mut diagnostics = Vec::new();
            warn_items(text, &index, &mut diagnostics);
            Index {
                raw: text,
                index: Some(index),
                error: None,
                diagnostics,
            }
        }
        // §4.3-4「パース失敗を起動失敗にしない」＝ E025（§7）を出して既定値で続行する。
        // 位置はトークン情報が無いのでファイル全体（§7.1 の範囲の決め方と同じ流儀）
        Err(error) => Index {
            raw: text,
            index: None,
            error: Some(error.to_string()),
            diagnostics: vec![Diagnostic::new(
                DiagnosticCode::E025,
                Some(0..text.len()),
                vec![Arg::text(error.to_string())],
            )],
        },
    }
}

impl<'a> Index<'a> {
    /// 入力と**バイト単位で一致する**テキストに戻す（`docs/history.md` の完了条件 2）。
    ///
    /// JSON は再シリアライズするとキー順と空白が変わるので、**原文をそのまま返す**。
    pub fn to_text(&self) -> &'a str {
        self.raw
    }

    /// パースできた木、または JSON が壊れているときの**空索引の既定値**（§4.3-4・§5.8.2）。
    ///
    /// **E025** が出てもアプリはこれで止まらない（P4「作家の原稿を人質に取らない」）。
    /// 原文は [`Index::raw`] にそのまま残るので、壊れたファイルを開き直しても失われない。
    pub fn index_or_default(&self) -> MaterialIndex {
        self.index.clone().unwrap_or_default()
    }

    /// 資料を ID で引く（§5.8.2）。`M` と `S` は同一の連番空間なので接頭辞は問わない。
    pub fn find(&self, id: &str) -> Option<&Material> {
        self.index.as_ref()?.items.iter().find(|item| item.id == id)
    }

    /// 資料を [`MaterialId`] で引く（ゼロ詰めの桁の違いを吸収する）。
    pub fn find_by_id(&self, id: MaterialId) -> Option<&Material> {
        self.index.as_ref()?.items.iter().find(|item| {
            item.material_id()
                .is_some_and(|found| found.prefix == id.prefix && found.num == id.num)
        })
    }

    /// タイトルで引く（§6.6 の参照の照合対象）。完全一致だけ。
    pub fn find_by_title(&self, title: &str) -> Option<&Material> {
        self.index
            .as_ref()?
            .items
            .iter()
            .find(|item| item.title == title)
    }

    /// `status` が `approved` の資料だけ（§5.8.1 `materials/docs/`）。
    pub fn approved(&self) -> Vec<&Material> {
        let Some(index) = &self.index else {
            return Vec::new();
        };
        index
            .items
            .iter()
            .filter(|item| item.status_kind() == Some(MaterialStatus::Approved))
            .collect()
    }
}

/// 資料ごとの検査（**E024** / **E008**）を**書いた順**に出す。
fn warn_items(text: &str, index: &MaterialIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: Vec<&str> = Vec::new();
    for item in &index.items {
        // §5.8.2「`path` はプロジェクトルートからの相対パス。`materials/` 配下に限る（外は E024）」
        if !item.path.is_empty() && !item.is_path_inside_materials() {
            out.push(Diagnostic::new(
                DiagnosticCode::E024,
                literal_ranges(text, &item.path).into_iter().next(),
                Vec::new(),
            ));
        }
        // §5.8.2「`id` は単調増加・再利用禁止（MUST）」。§7 E008「同一ファイル内の ID 重複」
        let repeats = seen
            .iter()
            .filter(|&&other| other == item.id.as_str())
            .count();
        if repeats > 0 {
            out.push(Diagnostic::new(
                DiagnosticCode::E008,
                literal_ranges(text, &item.id).into_iter().nth(repeats),
                vec![Arg::text(item.id.as_str())],
            ));
        } else {
            seen.push(item.id.as_str());
        }
        // §16.7「出典のない提案は自動的に W065 を付け、採用ボタンに確認を挟む」。付けるのは**提案**だけ
        // （採用のときに確認を経たものには付けない。`docs/history.md` B-134）
        if item.status_kind() == Some(MaterialStatus::Proposed) && item.source.is_missing() {
            out.push(Diagnostic::new(
                DiagnosticCode::W065,
                literal_ranges(text, &item.id).into_iter().next(),
                vec![Arg::text(item.id.as_str())],
            ));
        }
    }
}

/// `value` の **JSON リテラル**（`"M0137"` のように引用符込み）が原文に出る場所を全部返す。
///
/// serde はトークン位置を持たないので、診断の範囲はこれで決める。同じ値が何度も出るときは
/// 書いた順に並ぶので、呼び出し側が「何件目か」で選ぶ。
fn literal_ranges(text: &str, value: &str) -> Vec<Range<usize>> {
    let Ok(needle) = serde_json::to_string(value) else {
        return Vec::new();
    };
    if needle.len() < 2 {
        // `"…"` の形にならない（空文字列でも `""` になるので普通は来ない）
        return Vec::new();
    }
    text.match_indices(&needle)
        .map(|(at, found)| at..at + found.len())
        .collect()
}
