//! 層構造の機械検査（`docs/history.md` の作業項目 3 / §6.2、`docs/specifications/overview.md` §3.2・§12.2）。
//!
//! `src/core/**` を `syn` の AST として訪走し、次の 2 点を検査する。
//!
//! 1. 禁止依存（§3.2）— UI / I/O / 非同期 / 時刻・乱数、および許可リストに無いクレート
//! 2. ユーザー向け文言（§12.2）— 日本語の文字列リテラル
//!
//! 加えて **`Cargo.toml` の feature gate**（`docs/history.md` B-54・B-100）を見る: GUI のクレート
//! （egui / eframe / rfd / fontdb）が `optional = true` で `ui` feature の後ろにあり、既定のビルド
//! （＝ CI の fmt / clippy / test / wasm）がそれらをコンパイルしないこと。`ui` 層のうち文言カタログ
//! `ui::i18n` だけは CLI も使うので常にビルドし、GUI のクレートを使わないこと（M6）。
//!
//! §12.2 の日本語リテラル検査は `src/core/**` に加えて、M6 から **`src/cli/**`・`src/main.rs`・
//! `src/ui/i18n/**`** も対象にする（CLI のメッセージも同じカタログを使う、§12.2）。
//!
//! テキスト走査ではなく AST を使うので、コメント・文字列リテラル・char リテラルを字句として
//! 正しく区別できる。`'"'` のような char リテラルや `r#"..."#` の埋め込みデータによって
//! 検査が抜けたり誤検出したりしない。
//!
//! 訪走するノードは `ItemUse` / `ItemExternCrate` / `Path`（式・型・パターン・マクロ呼び出し・
//! 属性のすべて）/ `LitStr` / `TokenStream`（マクロ本体）。`visit_expr_path` ではなく
//! `visit_path` を実装するのが要点で、これにより型位置や属性位置も検査できる。
//!
//! `#[cfg(test)] mod tests { .. }` の内側は日本語リテラル検査の対象外にする（パーサの単体
//! テストには日本語の入力例が必須で、テストデータはユーザー向け文言ではないため）。禁止依存は
//! テスト内でも禁止のまま（フィクスチャは `include_str!` か `tests/golden/**` を使う）。
//! この免除の判定は **`test` という Ident の完全一致**で行い、`all(...)` / `any(...)` の中は
//! 再帰して探す。`contains("test")` のような部分文字列判定にすると `#[cfg(feature = "latest")]`
//! が免除されてしまう。
//!
//! `i18n: format-vocabulary` という doc 行を付けた `const` / `static` の中も日本語リテラル検査の
//! 対象外にする。§5.0.6 のフィールドキー別名表は「UI 言語が `en` でも受理する」ファイル形式の
//! 語彙であってユーザー向け文言ではなく、ロケール表（`ui/i18n/`）にも置けないため（§12.2 の
//! 「文言」の解釈は `docs/history.md` A-5・B-19）。マーカーは **doc 行の完全一致**で判定し、
//! 免除が及ぶのはその item の中だけ（同じファイルの関数やモジュールには効かない）。
//!
//! §3.2 の「読み書きは不可」は `std::fs` 等の禁止モジュールに加えて、**`io::stdin` /
//! `io::stdout` / `io::stderr`**（`use std::io;` 経由の 2 セグメント形を含む）でも検査する。
//! `std::io::Read` / `Write` / `Error` を型として使うのは純粋なので許可する。
//!
//! 検査器そのものの回帰は末尾の `checker_*` テストが担保する（`docs/history.md` のの完了条件
//! 「`src/core/` に意図的に `use egui;` を書き足すと CI が落ちる」の機械化）。

use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, ItemConst, ItemExternCrate, ItemMod, ItemStatic, ItemUse, Lit, LitStr, Meta,
    UseTree,
};

/// このクレートの名前（`Cargo.toml` の `[lib] name`）。
const CRATE_NAME: &str = "novnow";

/// §3.2 で core に許可されたクレートと、パス先頭に来る特殊な識別子。
///
/// `use` 文と `extern crate` の先頭セグメントはこれ以外を取り得ないので、一覧に無い
/// クレートを core が使い始めたらここで落ちる（§3.2 の MUST「許可クレートを増やすときは
/// 先に仕様書 §3.2 の表を更新する」の裏返し）。
const ALLOWED_ROOTS: &[&str] = &[
    "std",
    "core",
    "alloc",
    "crate",
    "self",
    "super",
    CRATE_NAME,
    // --- ここから下が §3.2 の許可クレート ---
    "serde",
    "serde_json",
    "regex",
    "aho_corasick",
    "ropey",
    "anyhow",
    "thiserror",
    "unicode_normalization",
    // §6.2.6（S-09 / F-23）の簡易形態素解析。依存ゼロ・純 Rust（モデル同梱）で I/O を持たない。
    "tiniestsegmenter",
    // `project.toml`（§4.5）の型付き読み取りと無損失編集（Issue #29 B10 で許可表へ追加）。
    "toml",
    "toml_edit",
];

/// core が参照してはならないクレート（§3.2）。
const FORBIDDEN_CRATES: &[&str] = &[
    "egui",
    "eframe",
    "rfd",
    "web_sys",
    "js_sys",
    "wasm_bindgen",
    "wasm_bindgen_futures",
    "tokio",
    "fontdb",
    "zip",
    "directories",
    "web_time",
    "rand",
    "getrandom",
    "console_error_panic_hook",
];

/// core が参照してはならない std のモジュール（§3.2 / `docs/developers.md` §3）。
///
/// `std::path` は型として持つことが §3.2 で明示的に許されているので対象外。`std::io` も
/// 型（`io::Result` 等）としての利用があり得るので対象外にしている（I/O 呼び出し自体は
/// `std::fs` 等を伴うので (2) で捕まる）。
const FORBIDDEN_STD_MODULES: &[&str] = &["fs", "net", "process", "thread", "env"];

/// core が参照してはならない層（§3.1「呼び出しのみ（逆方向なし）」）。
const FORBIDDEN_LAYERS: &[&str] = &["ui", "adapters"];

/// パスが現れた位置。位置によって検査の厳しさを変える。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pos {
    /// `use` 文 / `extern crate`。先頭は必ずクレート名か `crate` / `self` / `super`。
    Import,
    /// 属性。`#[wasm_bindgen]` のように 1 セグメントでも意味を持つ。
    Attr,
    /// 式・型・パターン・マクロ呼び出し。1 セグメントはローカル名（`let zip = ..` の
    /// `zip` など）であってクレート参照ではないので検査しない。
    Expr,
}

/// 検査結果を溜める visitor。
#[derive(Default)]
struct Checker {
    /// エラーメッセージに載せる検査対象（ファイル名、または自己テストのラベル）。
    label: String,
    /// §3.2 の禁止依存違反。
    deps: Vec<String>,
    /// §12.2 の日本語リテラル違反。
    text: Vec<String>,
    /// `#[cfg(test)]` モジュールの内側かどうか。テストデータの日本語はユーザー向け文言では
    /// ないので、§12.2 の検査からは外す（パーサの単体テストには日本語の入力例が必須）。
    in_test_cfg: bool,
    /// [`VOCABULARY_MARKER`] の doc 行を付けた `const` / `static` の内側かどうか。
    /// §5.0.6 の別名表のような**ファイル形式の語彙**はユーザー向け文言ではないので外す。
    in_vocabulary: bool,
}

impl Checker {
    fn report_dep(&mut self, path: &str, ctx: &str) {
        let msg = format!("'{}' in {} ({})", path, self.label, ctx);
        self.deps.push(msg);
    }

    fn report_text(&mut self, shown: &str) {
        if self.in_test_cfg || self.in_vocabulary {
            return;
        }
        if has_japanese(shown) {
            let msg = format!("{} in {}", shown, self.label);
            self.text.push(msg);
        }
    }

    /// パスのセグメント列を規則に照らして検査する。
    fn check_segments(&mut self, segs: &[String], pos: Pos, ctx: &str) {
        if segs.is_empty() {
            return;
        }
        if pos == Pos::Expr && segs.len() < 2 {
            return;
        }

        let first = segs[0].as_str();
        let path = segs.join("::");

        // (1) 禁止クレート
        if FORBIDDEN_CRATES.contains(&first) {
            self.report_dep(&path, ctx);
            return;
        }

        // (2) 禁止 std モジュール
        let std_like = first == "std" || first == "core" || first == "alloc";
        if std_like && segs.len() >= 2 && FORBIDDEN_STD_MODULES.contains(&segs[1].as_str()) {
            self.report_dep(&path, ctx);
            return;
        }

        // (3) 逆方向依存（core → ui / adapters）。`crate::ui` だけでなく `novnow::ui` や
        //     `super::super::ui` のような書き方も捕まえる。
        let intra = matches!(first, "crate" | "self" | "super") || first == CRATE_NAME;
        if intra
            && segs[1..]
                .iter()
                .any(|s| FORBIDDEN_LAYERS.contains(&s.as_str()))
        {
            self.report_dep(&path, ctx);
            return;
        }

        // (4) グローバルな現在時刻・乱数（§3.2）
        let now = segs
            .windows(2)
            .any(|w| w[1] == "now" && (w[0] == "SystemTime" || w[0] == "Instant"));
        if now || segs.iter().any(|s| s == "thread_rng") {
            self.report_dep(&path, ctx);
            return;
        }

        // (4b) 標準入出力（§3.2「`PathBuf` を型として持つのは可、読み書きは不可」）。
        //      `std::io::stdout` だけでなく、`use std::io;` 経由の `io::stdout` も捕まえる。
        //      `std::io::Read` / `Write` を**型として**使うのは純粋なので許可する
        //      （末尾が stdin / stdout / stderr のときだけ違反にする）。
        let stdio = segs
            .windows(2)
            .any(|w| w[0] == "io" && matches!(w[1].as_str(), "stdin" | "stdout" | "stderr"));
        if stdio {
            self.report_dep(&path, ctx);
            return;
        }

        // (5) §3.2 の許可リストに無いクレートの import
        if pos == Pos::Import && !ALLOWED_ROOTS.contains(&first) {
            self.report_dep(&path, ctx);
        }
    }

    /// `use` 木を末端まで展開して検査する。
    fn check_use_tree(&mut self, tree: &UseTree, prefix: &mut Vec<String>) {
        match tree {
            UseTree::Path(p) => {
                prefix.push(p.ident.to_string());
                self.check_use_tree(&p.tree, prefix);
                prefix.pop();
            }
            UseTree::Name(n) => {
                prefix.push(n.ident.to_string());
                self.check_segments(prefix, Pos::Import, "use");
                prefix.pop();
            }
            UseTree::Rename(r) => {
                prefix.push(r.ident.to_string());
                self.check_segments(prefix, Pos::Import, "use as");
                prefix.pop();
            }
            UseTree::Glob(_) => {
                self.check_segments(prefix, Pos::Import, "use glob");
            }
            UseTree::Group(g) => {
                for item in &g.items {
                    self.check_use_tree(item, prefix);
                }
            }
        }
    }

    /// マクロ本体（`panic!` のメッセージ、`macro_rules!` の定義、`derive` の中身など）を
    /// トークン列として走査する。syn はマクロ本体をパースしないので、AST 訪走だけでは
    /// 中の文字列リテラルもパスも見えない。
    fn scan_tokens(&mut self, stream: &TokenStream) {
        let toks: Vec<TokenTree> = stream.clone().into_iter().collect();
        let mut i = 0;
        while i < toks.len() {
            match &toks[i] {
                TokenTree::Group(g) => {
                    let inner = g.stream();
                    self.scan_tokens(&inner);
                    i += 1;
                }
                TokenTree::Literal(l) => {
                    let raw = l.to_string();
                    if is_string_literal(&raw) {
                        self.report_text(&raw);
                    }
                    i += 1;
                }
                TokenTree::Ident(_) => {
                    let mut segs = vec![toks[i].to_string()];
                    let mut j = i + 1;
                    while j + 2 < toks.len() && is_colon(&toks[j]) && is_colon(&toks[j + 1]) {
                        match &toks[j + 2] {
                            TokenTree::Ident(id) => segs.push(id.to_string()),
                            _ => break,
                        }
                        j += 3;
                    }
                    self.check_segments(&segs, Pos::Expr, "macro");
                    i = j;
                }
                TokenTree::Punct(_) => i += 1,
            }
        }
    }
}

impl<'ast> Visit<'ast> for Checker {
    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        let mut prefix = Vec::new();
        self.check_use_tree(&node.tree, &mut prefix);
        visit::visit_item_use(self, node);
    }

    fn visit_item_extern_crate(&mut self, node: &'ast ItemExternCrate) {
        let segs = [node.ident.to_string()];
        self.check_segments(&segs, Pos::Import, "extern crate");
        visit::visit_item_extern_crate(self, node);
    }

    /// `#[cfg(test)] mod tests { .. }` の内側では日本語リテラルを許容する。
    /// 禁止依存（§3.2）はテスト内でも禁止のまま（フィクスチャは `include_str!` を使う）。
    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let outer = self.in_test_cfg;
        if !outer && has_cfg_test(&node.attrs) {
            self.in_test_cfg = true;
        }
        visit::visit_item_mod(self, node);
        self.in_test_cfg = outer;
    }

    /// [`VOCABULARY_MARKER`] の doc 行が付いた `const` の中だけ日本語リテラルを許容する。
    fn visit_item_const(&mut self, node: &'ast ItemConst) {
        let outer = self.in_vocabulary;
        if !outer && has_vocabulary_marker(&node.attrs) {
            self.in_vocabulary = true;
        }
        visit::visit_item_const(self, node);
        self.in_vocabulary = outer;
    }

    /// `const` と同じ免除を `static` にも適用する。
    fn visit_item_static(&mut self, node: &'ast ItemStatic) {
        let outer = self.in_vocabulary;
        if !outer && has_vocabulary_marker(&node.attrs) {
            self.in_vocabulary = true;
        }
        visit::visit_item_static(self, node);
        self.in_vocabulary = outer;
    }

    /// 式・型・パターン・マクロ呼び出しのパスをまとめて検査する。
    fn visit_path(&mut self, node: &'ast syn::Path) {
        let segs: Vec<String> = node.segments.iter().map(|s| s.ident.to_string()).collect();
        self.check_segments(&segs, Pos::Expr, "path");
        visit::visit_path(self, node);
    }

    fn visit_attribute(&mut self, node: &'ast Attribute) {
        let path = meta_path(&node.meta);
        // ドキュメントコメント（`///` と `//!`）はコメント扱い。§12.2 が禁じるのは
        // ユーザー向け文言であってコメントではないので、中身を検査せずに打ち切る。
        // （syn は doc コメントを `#[doc = "..."]` 属性として持つため、打ち切らないと
        //   日本語の doc コメントがリテラル違反として誤検出される）
        if path.is_ident("doc") {
            return;
        }
        // 1 セグメントの属性（`#[wasm_bindgen]` 等）は visit_path では拾えない
        // （式位置の 1 セグメントはローカル名として除外するため）のでここで検査する。
        // 2 セグメント以上（`#[tokio::main]`）は visit_path が拾うので二重報告を避ける。
        let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        if segs.len() == 1 {
            self.check_segments(&segs, Pos::Attr, "attribute");
        }
        visit::visit_attribute(self, node);
    }

    fn visit_lit_str(&mut self, node: &'ast LitStr) {
        let shown = format!("\"{}\"", node.value());
        self.report_text(&shown);
        visit::visit_lit_str(self, node);
    }

    fn visit_token_stream(&mut self, node: &'ast TokenStream) {
        self.scan_tokens(node);
    }
}

/// 属性が指すパスを返す（`Meta` の 3 形すべてがパスを持つ）。
fn meta_path(meta: &Meta) -> &syn::Path {
    match meta {
        Meta::Path(p) => p,
        Meta::List(l) => &l.path,
        Meta::NameValue(nv) => &nv.path,
    }
}

/// `#[cfg(test)]`（または `#[cfg(all(test))]` 等）が付いているか。
///
/// `tokens.to_string().contains("test")` のような**部分文字列判定はしない**。
/// `#[cfg(feature = "latest")]` が `test` を含んでしまい、§12.2 の日本語リテラル検査を
/// 意図せず免除してしまうため。`test` という **Ident の完全一致**を `all(...)` / `any(...)`
/// の括弧の中まで再帰して探す（`"test"` という文字列リテラルは feature 名であって
/// test 設定ではないので、Ident だけを見るのが正しい）。
fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !meta_path(&a.meta).is_ident("cfg") {
            return false;
        }
        match &a.meta {
            Meta::List(l) => mentions_test_ident(&l.tokens),
            _ => false,
        }
    })
}

/// トークン列を再帰して `test` という Ident があるかを探す。
fn mentions_test_ident(tokens: &TokenStream) -> bool {
    tokens.clone().into_iter().any(|tt| match tt {
        TokenTree::Ident(ident) => ident == "test",
        TokenTree::Group(group) => mentions_test_ident(&group.stream()),
        _ => false,
    })
}

/// 字句データ（ファイル形式の語彙）であることを宣言する doc 行のマーカー。
///
/// §5.0.6 の別名表のように、**日本語だがユーザー向け文言ではない**データを core に置くための
/// 唯一の抜け道。`ui/i18n/` はロケールで引く表なので、UI 言語が `en` でも受理する別名は
/// 置けない（§12）。
const VOCABULARY_MARKER: &str = "i18n: format-vocabulary";

/// doc コメントに [`VOCABULARY_MARKER`] の行があるか。
///
/// **行の完全一致**だけをマーカーと認める（説明文の中に同じ語が紛れ込んでも免除しない）。
/// 普通の `//` コメントは syn が捨てて AST に残らないので、`///`（doc 属性）で書く必要がある。
fn has_vocabulary_marker(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !meta_path(&a.meta).is_ident("doc") {
            return false;
        }
        match &a.meta {
            Meta::NameValue(nv) => match &nv.value {
                Expr::Lit(lit) => match &lit.lit {
                    Lit::Str(s) => s.value().trim() == VOCABULARY_MARKER,
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        }
    })
}

/// 文字列リテラル（`"..."`, `r"..."`, `r#"..."#`, `b"..."`, `br"..."`）かどうか。
/// char リテラル（`'「'`）は字句解析で正当に必要なので対象外。
fn is_string_literal(text: &str) -> bool {
    let plain = text.starts_with('"');
    let raw_or_byte = text.starts_with('r') || text.starts_with('b');
    plain || (raw_or_byte && text.contains('"'))
}

fn is_colon(tt: &TokenTree) -> bool {
    matches!(tt, TokenTree::Punct(p) if p.as_char() == ':')
}

/// 検査対象の Unicode ブロック（仮名・漢字）。
///
/// 約物（`「」`、U+3000-303F）と全角形（U+FF00-FFEF）は**意図的に対象外**。前者は字句解析の
/// 区切り定数として core が正当に持ち得るため、後者は §6.1.3 の NFKC 照合のテストデータとして
/// 正当に持ち得るため。ユーザー向け文言は必ず仮名・漢字を含むので実務上の検出漏れは無い。
const JAPANESE_RANGES: &[(u32, u32)] = &[
    (0x3040, 0x309F), // ひらがな
    (0x30A0, 0x30FF), // カタカナ
    (0x31F0, 0x31FF), // カタカナ音声拡張
    (0x3400, 0x4DBF), // CJK 拡張 A
    (0x4E00, 0x9FFF), // CJK 漢字
];

fn has_japanese(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        JAPANESE_RANGES
            .iter()
            .any(|&(lo, hi)| (lo..=hi).contains(&u))
    })
}

/// ソース 1 件を検査し、(禁止依存違反, 日本語リテラル違反) を返す。
fn scan(label: &str, source: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let parsed = syn::parse_file(source);
    let file = parsed.map_err(|e| format!("{}: {}", label, e))?;
    let mut checker = Checker {
        label: label.to_string(),
        ..Default::default()
    };
    checker.visit_file(&file);
    Ok((checker.deps, checker.text))
}

fn scan_or_panic(label: &str, source: &str) -> (Vec<String>, Vec<String>) {
    scan(label, source).unwrap_or_else(|e| panic!("{}", e))
}

fn collect_rs(dir: &Path, files: &mut Vec<PathBuf>) {
    if dir.is_dir() {
        for entry in fs::read_dir(dir).expect("read_dir failed") {
            let entry = entry.expect("entry failed");
            let path = entry.path();
            if path.is_dir() {
                collect_rs(&path, files);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
}

/// `src/core/**` の全 `.rs`。
///
/// ディレクトリを改名・移動されたときに検査が空振りして緑になるのを防ぐため、
/// 0 件なら失敗させる。
fn core_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs(Path::new("src/core"), &mut files);
    files.sort();
    assert!(!files.is_empty(), "No .rs files found in src/core!");
    files
}

fn bullets(items: &[String]) -> String {
    let mut out = String::new();
    for item in items {
        out.push_str("  - ");
        out.push_str(item);
        out.push('\n');
    }
    out
}

fn read_core(file: &Path) -> String {
    let path = file.display().to_string();
    let result = fs::read_to_string(file);
    result.unwrap_or_else(|e| panic!("read failed {}: {}", path, e))
}

// ---------------------------------------------------------------------------
// 本体の検査（`docs/history.md` の CI 常設テスト）
// ---------------------------------------------------------------------------

/// §3.2: `src/core/**` に禁止依存（UI / I/O / 非同期 / 時刻・乱数 / 許可外クレート）が無いこと。
#[test]
fn core_has_no_forbidden_dependency() {
    let mut found = Vec::new();
    for file in core_files() {
        let source = read_core(&file);
        let label = file.display().to_string();
        let (deps, _) = scan_or_panic(&label, &source);
        found.extend(deps);
    }
    assert!(
        found.is_empty(),
        "`docs/specifications/overview.md` §3.2 の層構造違反が {} 件（src/core）:\n{}",
        found.len(),
        bullets(&found)
    );
}

/// §12.2: `src/core/**` にユーザー向け文言（日本語の文字列リテラル）が無いこと。
#[test]
fn core_has_no_user_facing_text() {
    let mut found = Vec::new();
    for file in core_files() {
        let source = read_core(&file);
        let label = file.display().to_string();
        let (_, text) = scan_or_panic(&label, &source);
        found.extend(text);
    }
    assert!(
        found.is_empty(),
        "`docs/specifications/ui.md` §12.2 違反（core は文言を持たない）が {} 件:\n{}",
        found.len(),
        bullets(&found)
    );
}

// ---------------------------------------------------------------------------
// 検査器の自己テスト
//
// 「違反を書いたら本当に落ちるか」をテストで担保する。これが無いと、検査器の改修で
// 検出漏れが混入しても CI は緑のままになる（`docs/history.md` の完了条件 2 の機械化）。
// ---------------------------------------------------------------------------

/// 検出されるべき違反（§3.2）。
const MUST_CATCH_DEPS: &[&str] = &[
    // --- import 位置 ---
    "use egui::Context;",
    "use egui;",
    "use eframe::NativeOptions;",
    "use rfd::FileDialog;",
    "use web_sys::console;",
    "use js_sys::Date;",
    "use wasm_bindgen::prelude::wasm_bindgen;",
    "use wasm_bindgen_futures::spawn_local;",
    "use tokio::sync::Mutex;",
    "use fontdb::Database;",
    "use zip::ZipWriter;",
    "use zip::ZipArchive;",
    "use directories::ProjectDirs;",
    "use web_time::Instant;",
    "use rand::rngs::StdRng;",
    "use std::fs;",
    "use std::fs::File;",
    "use std::{fs, io};",
    "use std::net::TcpStream;",
    "use std::process::Command;",
    "use std::thread;",
    "use std::env;",
    // --- 標準入出力（§3.2「読み書きは不可」。io の型そのものは可） ---
    "use std::io::stdout;",
    "pub fn a() { let _ = std::io::stdout(); }",
    "use std::io;\npub fn b() { let _ = io::stderr(); }",
    // --- 逆方向依存（core → ui / adapters） ---
    "use crate::ui::theme;",
    "use crate::adapters::storage_fs;",
    "use novnow::ui::shell;",
    "use novnow::adapters::mcp_stdio;",
    "use super::super::ui::theme;",
    // --- 許可リストに無いクレート（§3.2 の MUST） ---
    "use reqwest::Client;",
    "extern crate egui;",
    // --- 型位置（visit_expr_path だけを実装すると抜ける） ---
    "pub fn f(ctx: &egui::Context) { let _ = ctx; }",
    "pub struct S { pub file: std::fs::File }",
    "pub type T = tokio::sync::Mutex<u8>;",
    // --- 式位置 ---
    "pub fn g() { let _ = web_sys::console::log_1(); }",
    "pub fn h() { let _ = std::time::SystemTime::now(); }",
    "pub fn i() { let _ = std::time::Instant::now(); }",
    "pub fn j() { let _ = Instant::now(); }",
    "pub fn k() { let _ = rand::thread_rng(); }",
    "pub fn l() { let _ = std::process::Command::new(\"ls\"); }",
    // --- 属性位置 ---
    "#[wasm_bindgen] pub fn m() {}",
    "#[tokio::main] pub async fn n() {}",
    // --- テスト内でも禁止依存は禁止（§12.2 の免除は文言だけ） ---
    "#[cfg(test)] mod t { use std::fs; }",
    "#[cfg(test)] mod t { fn a() { let _ = egui::vec2(1.0, 2.0); } }",
    // --- マクロ呼び出しパス ---
    "pub fn o() { let _ = tokio::join!(async {}, async {}); }",
    "pub fn p() { let _ = egui::vec2(1.0, 2.0); }",
];

/// 検出されてはいけない正当な core コード（誤検出の回帰防止）。
const MUST_PASS_DEPS: &[&str] = &[
    // --- §3.2 の許可クレート ---
    "use std::collections::HashMap;",
    "use std::path::{Path, PathBuf};",
    "use std::time::Duration;",
    "use serde::{Deserialize, Serialize};",
    "use serde_json::Value;",
    "use regex::Regex;",
    "use aho_corasick::AhoCorasick;",
    "use ropey::Rope;",
    "use anyhow::Result;",
    "use thiserror::Error;",
    "use unicode_normalization::UnicodeNormalization;",
    "use crate::core::id::SceneId;",
    "use crate::ports::storage::Storage;",
    "use super::lexer::Token;",
    "use self::Thing;",
    // --- コメント内の禁止識別子は対象外 ---
    "// egui も eframe も std::fs も使わない\npub fn a() {}",
    "/* tokio::spawn と web_sys は禁止 */ pub fn b() {}",
    // --- 禁止クレート名を含むだけの文字列データは対象外（golden フィクスチャ想定） ---
    "pub const FIXTURE: &str = r#\"{\"crate\": \"egui\"}\"#;",
    // --- char リテラルは字句解析で必要（§5.0 のシジル、「」） ---
    "pub fn is_quote(c: char) -> bool { matches!(c, '\"' | '「' | '」') }",
    "pub const QUOTE: char = '「';",
    // --- ローカル名が禁止クレート名と衝突しても、式位置の 1 セグメントは対象外 ---
    "pub fn locals() -> u32 { let toml = 1; let ui = 2; toml + ui }",
    "pub fn more_locals() -> u32 { let adapters = 1; let zip = 2; adapters + zip }",
    "pub fn zip_local(v: Vec<u8>) -> usize { let zip = v.iter().zip(v.iter()); zip.count() }",
    // --- std::iter::zip は std::iter であって zip クレートではない ---
    "pub fn zipped() -> usize { std::iter::zip(0..3, 3..6).count() }",
    // --- std::io は型として使うぶんには純粋（stdin / stdout / stderr だけ禁止） ---
    "use std::io::{Read, Write};",
    "pub fn write_all<W: std::io::Write>(w: &mut W) -> std::io::Result<()> { w.flush() }",
    "pub type IoError = std::io::Error;",
    // --- 時刻・乱数でも now() でなければ可（Duration 等は純粋な型） ---
    "pub fn span() -> std::time::Duration { std::time::Duration::from_secs(1) }",
    // --- derive の中身は許可クレート ---
    "#[derive(Debug, Clone, serde::Serialize)] pub struct M { pub a: u8 }",
    "#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct N { pub a: u8 }",
];

/// 検出されるべき日本語リテラル（§12.2）。
const MUST_CATCH_TEXT: &[&str] = &[
    "pub const MSG: &str = \"未保存\";",
    "pub fn a() -> &'static str { \"保存しました\" }",
    "pub fn b() { panic!(\"ファイルが見つかりません\"); }",
    "pub fn c() { assert!(true, \"検査に失敗しました\"); }",
    "pub fn d() { let _ = format!(\"{} 件の診断\", 3); }",
    "pub fn e() { println!(\"完了\"); }",
    "macro_rules! fail { () => { panic!(\"致命的なエラー\") }; }",
    "pub fn f() -> Result<u8, String> { Err(\"権限がありません\".to_owned()) }",
    "pub struct G; impl G { pub fn h(&self) -> String { String::from(\"開けません\") } }",
    // --- 免除は #[cfg(test)] の内側だけ（外に出たら違反） ---
    "pub fn x() { let _ = \"日本語\"; }\n#[cfg(test)] mod t {}",
    // --- "latest" のように test を部分文字列として含むだけの cfg は免除しない ---
    "#[cfg(feature = \"latest\")] mod t { pub const M: &str = \"日本語\"; }",
    "#[cfg(any(feature = \"greatest\"))] mod t { pub const M: &str = \"日本語\"; }",
    // --- 字句データの免除はマーカー付き const / static の中だけ（関数には効かない） ---
    "pub const KEYS: &[(&str, &str)] = &[(\"summary\", \"要約\")];",
    "/// i18n: format-vocabulary\npub const KEYS: &[&str] = &[\"要約\"];\npub fn msg() -> &'static str { \"保存しました\" }",
    // --- マーカーは doc 行の完全一致（説明文の中の語や、普通の `//` コメントでは免除しない） ---
    "/// §5.0.6 は i18n: format-vocabulary に触れている\npub const KEYS: &[&str] = &[\"要約\"];",
    "// i18n: format-vocabulary\npub const KEYS: &[&str] = &[\"要約\"];",
];

/// 検出されてはいけない日本語（コメント・doc コメント・char リテラル）。
const MUST_PASS_TEXT: &[&str] = &[
    "// 日本語の行コメントは可\npub fn a() {}",
    "/* 日本語のブロックコメントも可 */ pub fn b() {}",
    "/// 日本語の doc コメントも可\npub fn c() {}",
    "//! 日本語のモジュール doc も可\npub fn d() {}",
    "#[doc = \"日本語の属性 doc\"] pub fn e() {}",
    "pub fn f(ch: char) -> bool { ch == '「' || ch == 'あ' }",
    "pub fn g() -> String { String::from(\"ascii only\") }",
    "pub const H: &str = r#\"{\"title\": \"ascii\"}\"#;",
    // --- #[cfg(test)] の内側はテストデータなので日本語リテラルを許容 ---
    "#[cfg(test)] mod t { fn a() { let _ = \"日本語のテストデータ\"; } }",
    // --- all(...) / any(...) の括弧の中に test があっても同じ（再帰して探す） ---
    "#[cfg(all(test, feature = \"x\"))] mod t { pub const M: &str = \"日本語\"; }",
    "#[cfg(any(not(feature = \"x\"), test))] mod t { fn a() { let _ = \"日本語\"; } }",
    // --- §5.0.6 の別名表のような字句データは `i18n: format-vocabulary` の doc 行で免除 ---
    "/// i18n: format-vocabulary\npub const KEYS: &[(&str, &str)] = &[(\"summary\", \"要約\")];",
    "/// 説明\n/// i18n: format-vocabulary\npub static KEYS: [&str; 1] = [\"要約\"];",
];

#[test]
fn checker_catches_forbidden_dependencies() {
    let mut missed = Vec::new();
    for source in MUST_CATCH_DEPS {
        let (deps, _) = scan_or_panic("<selftest>", source);
        if deps.is_empty() {
            missed.push((*source).to_string());
        }
    }
    assert!(
        missed.is_empty(),
        "検査器が違反を検出できない（{} 件）:\n{}",
        missed.len(),
        bullets(&missed)
    );
}

#[test]
fn checker_allows_pure_core_code() {
    let mut flagged = Vec::new();
    for source in MUST_PASS_DEPS {
        let (deps, _) = scan_or_panic("<selftest>", source);
        for d in deps {
            flagged.push(format!("{} <- {}", source, d));
        }
    }
    assert!(
        flagged.is_empty(),
        "検査器が正当な core コードを誤検出（{} 件）:\n{}",
        flagged.len(),
        bullets(&flagged)
    );
}

#[test]
fn checker_catches_japanese_literals() {
    let mut missed = Vec::new();
    for source in MUST_CATCH_TEXT {
        let (_, text) = scan_or_panic("<selftest>", source);
        if text.is_empty() {
            missed.push((*source).to_string());
        }
    }
    assert!(
        missed.is_empty(),
        "検査器が日本語リテラルを検出できない（{} 件）:\n{}",
        missed.len(),
        bullets(&missed)
    );
}

#[test]
fn checker_allows_japanese_comments() {
    let mut flagged = Vec::new();
    for source in MUST_PASS_TEXT {
        let (_, text) = scan_or_panic("<selftest>", source);
        for t in text {
            flagged.push(format!("{} <- {}", source, t));
        }
    }
    assert!(
        flagged.is_empty(),
        "検査器がコメント等を誤検出（{} 件）:\n{}",
        flagged.len(),
        bullets(&flagged)
    );
}

// ============================================================ Cargo の feature gate（`docs/history.md` B-54）

/// `Cargo.toml`（依存と feature の正本）。
const CARGO_TOML: &str = include_str!("../Cargo.toml");

/// `src/lib.rs`（4 層の宣言の正本）。
const LIB_RS: &str = include_str!("../src/lib.rs");

/// `src/adapters/mod.rs`（アダプタの宣言の正本）。
const ADAPTERS_MOD_RS: &str = include_str!("../src/adapters/mod.rs");

/// GUI（§3.3 の `ui` 層の egui 部分）に要るクレート。`docs/designs/overview.md` §3.1 の表で層が ui のもの
/// （`toml` は M6 から CLI も使う文言カタログのために通常の依存になった。B-100）。
const UI_CRATES: &[&str] = &["eframe", "egui", "rfd", "fontdb", "image"];

/// `src/ui/mod.rs`（`ui` 層の宣言の正本）。
const UI_MOD_RS: &str = include_str!("../src/ui/mod.rs");

#[test]
fn the_ui_crates_are_optional_and_behind_the_ui_feature() {
    let cargo: toml::Value = toml::from_str(CARGO_TOML).expect("Cargo.toml がパースできない");
    let deps = cargo["dependencies"]
        .as_table()
        .expect("Cargo.toml に [dependencies] が無い");
    for name in UI_CRATES {
        let dep = deps[*name]
            .as_table()
            .unwrap_or_else(|| panic!("{name} はインラインテーブル（`optional = true`）にする"));
        assert_eq!(
            dep["optional"].as_bool(),
            Some(true),
            "{name} は optional = true にする（`docs/history.md` B-54）"
        );
    }
    let features = cargo["features"]
        .as_table()
        .expect("Cargo.toml に [features] が無い");
    let ui: Vec<&str> = features["ui"]
        .as_array()
        .expect("[features] に ui が無い")
        .iter()
        .filter_map(|found| found.as_str())
        .collect();
    for name in UI_CRATES {
        let want = format!("dep:{name}");
        assert!(ui.contains(&want.as_str()), "ui feature に {want} が無い");
    }
    // 既定で ui をビルドしない（egui / eframe を CI の 4 ジョブから外すのが目的そのもの）
    let default: Vec<&str> = features["default"]
        .as_array()
        .map(|found| found.iter().filter_map(|item| item.as_str()).collect())
        .unwrap_or_default();
    assert!(
        !default.contains(&"ui"),
        "default に ui を入れると CI が egui / eframe をビルドしてしまう（{default:?}）"
    );
}

#[test]
fn the_gui_is_compiled_only_with_the_ui_feature() {
    // GUI のモジュールと `pub mod dialog_rfd;` が cfg で守られているか。守られていないと、
    // M7 で `src/ui/**` が egui を使い始めた瞬間に**既定のビルド（CI の 4 ジョブ）が落ちる**。
    // 文言カタログ `ui::i18n` だけは CLI も使うので守らない（B-100）
    assert!(
        LIB_RS.contains("\npub mod ui;")
            && !LIB_RS.contains("#[cfg(feature = \"ui\")]\npub mod ui;"),
        "src/lib.rs の `pub mod ui;` は cfg なしで宣言する（中の GUI だけを src/ui/mod.rs で守る）"
    );
    let file = syn::parse_file(UI_MOD_RS).expect("src/ui/mod.rs がパースできない");
    let mut names = Vec::new();
    for item in &file.items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        let name = module.ident.to_string();
        let gated = module.attrs.iter().any(|attr| {
            attr.path().is_ident("cfg")
                && match attr.parse_args::<Meta>() {
                    Ok(Meta::NameValue(pair)) => {
                        pair.path.is_ident("feature")
                            && matches!(&pair.value, Expr::Lit(syn::ExprLit { lit: Lit::Str(text), .. }) if text.value() == "ui")
                    }
                    _ => false,
                }
        });
        if name == "i18n" {
            assert!(
                module.attrs.iter().all(|attr| !attr.path().is_ident("cfg")),
                "ui::i18n は常にビルドする（CLI が使う）"
            );
        } else {
            assert!(
                gated,
                "src/ui/mod.rs の `pub mod {name};` に `#[cfg(feature = \"ui\")]` が付いていない"
            );
        }
        names.push(name);
    }
    assert!(
        names.contains(&"i18n".to_owned()),
        "ui::i18n が無い: {names:?}"
    );
    assert!(
        names.len() > 1,
        "GUI のモジュールが見つからない（検査が空振り）: {names:?}"
    );
    assert!(
        ADAPTERS_MOD_RS.contains("#[cfg(feature = \"ui\")]\npub mod dialog_rfd;"),
        "src/adapters/mod.rs の `pub mod dialog_rfd;` に `#[cfg(feature = \"ui\")]` が付いていない"
    );
}

/// パスと `use` の先頭のクレート名を集める（GUI のクレートを使っていないかの検査用）。
#[derive(Default)]
struct CrateRoots {
    roots: Vec<String>,
}

impl<'ast> Visit<'ast> for CrateRoots {
    fn visit_path(&mut self, node: &'ast syn::Path) {
        if let Some(first) = node.segments.first() {
            self.roots.push(first.ident.to_string());
        }
        visit::visit_path(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        let mut tree = &node.tree;
        loop {
            match tree {
                UseTree::Path(path) if path.ident == "crate" || path.ident == "self" => {
                    tree = &path.tree;
                }
                UseTree::Path(path) => {
                    self.roots.push(path.ident.to_string());
                    break;
                }
                UseTree::Name(name) => {
                    self.roots.push(name.ident.to_string());
                    break;
                }
                _ => break,
            }
        }
        visit::visit_item_use(self, node);
    }
}

/// §9.5.1 MUST: 参照ペインはカードにしない。`egui::Frame::group`（と、それを使う `Ui::group`）は
/// `src/ui/` 全体で 0 件（`docs/history.md` の完了条件「grep で検査、CI に追加」。CI の test ジョブで走る）。
#[test]
fn the_gui_never_uses_frame_group() {
    let mut files = Vec::new();
    collect_rs(Path::new("src/ui"), &mut files);
    assert!(files.len() > 10, "src/ui の .rs が見つからない: {files:?}");
    let mut found = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).expect("read");
        for (number, line) in source.lines().enumerate() {
            // コメント（`//`・`///`・`//!`）の中の言及は数えない
            let code = match line.find("//") {
                Some(at) => &line[..at],
                None => line,
            };
            if code.contains("Frame::group") || code.contains(".group(") {
                found.push(format!(
                    "{}:{}: {}",
                    file.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "Frame::group / Ui::group を使っている:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_message_catalog_does_not_use_the_gui_crates() {
    // `ui::i18n` は `ui` feature なしでビルドするので、egui などを使うと既定のビルドが落ちる
    let mut files = Vec::new();
    collect_rs(Path::new("src/ui/i18n"), &mut files);
    assert!(!files.is_empty(), "src/ui/i18n に .rs が無い");
    for file in files {
        let source = read_core(&file);
        let parsed = syn::parse_file(&source).expect("parse");
        let mut roots = CrateRoots::default();
        roots.visit_file(&parsed);
        let used: Vec<&String> = roots
            .roots
            .iter()
            .filter(|root| UI_CRATES.contains(&root.as_str()))
            .collect();
        assert!(
            used.is_empty(),
            "{} が GUI のクレートを使っている: {used:?}",
            file.display()
        );
    }
}

/// 段階 ③ §19.1: app 層（外部インターフェースの機能核）は GUI のクレートを使わない。
///
/// `src/app/**` は `ui` feature なし（＝ CI の 4 ジョブの既定ビルド）でコンパイルされる。
/// egui / eframe / rfd / fontdb / image のどれかに依存した瞬間に既定のビルドが落ちるので、
/// ソースを読んで固定する（GUI 付きビルドでなくても検査できる）。文言は `ui::i18n` 経由
/// （§19.1.8）なので、`gui_has_no_user_facing_text` が `src/app/**` も一緒に検査する。
#[test]
fn the_app_layer_does_not_use_the_gui_crates() {
    let mut files = Vec::new();
    collect_rs(Path::new("src/app"), &mut files);
    assert!(files.len() > 5, "src/app の .rs が少なすぎる: {files:?}");
    for file in &files {
        let source = read_core(file);
        let parsed = syn::parse_file(&source).expect("parse");
        let mut roots = CrateRoots::default();
        roots.visit_file(&parsed);
        let used: Vec<&String> = roots
            .roots
            .iter()
            .filter(|root| UI_CRATES.contains(&root.as_str()))
            .collect();
        assert!(
            used.is_empty(),
            "{} が GUI のクレートを使っている: {used:?}（§19.1: app 層は GUI なしで動く）",
            file.display()
        );
    }
}

/// 段階 ③ H1-AC-1: `src/app/` へ移した状態機械は旧パスから再エクスポートで解決する。
///
/// 文書（`REF:docs/designs/**`）と既存テストは `ui::workbench` などの旧パスを参照し続ける
/// ため、移設は「移動 + 旧パスからの `pub use`」で行う（`REF:docs/plans.md#stage-3`）。
#[test]
fn the_moved_state_machines_are_reexported_from_their_old_paths() {
    let app_mod = include_str!("../src/app/mod.rs");
    for name in [
        "agents",
        "assist",
        "command",
        "commands",
        "host",
        "palette",
        "save_machine",
        "settings",
        "workbench",
        "workspace",
    ] {
        assert!(
            app_mod.contains(&format!("pub mod {name};")),
            "src/app/mod.rs に `pub mod {name};` が無い"
        );
    }
    assert!(
        UI_MOD_RS.contains(
            "pub use crate::app::{agents, assist, palette, save_machine, settings, workbench};"
        ),
        "src/ui/mod.rs が app 層の 6 モジュールを再エクスポートしていない（旧パスが壊れる）"
    );
    let shell = include_str!("../src/ui/shell.rs");
    assert!(
        shell.contains("pub use crate::app::command::Command;"),
        "src/ui/shell.rs が Command を再エクスポートしていない"
    );
    let spaces = include_str!("../src/ui/workspaces/mod.rs");
    assert!(
        spaces.contains("pub use crate::app::workspace::{Switcher, Workspace};"),
        "src/ui/workspaces/mod.rs が Switcher / Workspace を再エクスポートしていない"
    );
    let gui = include_str!("../src/ui/app.rs");
    assert!(
        gui.contains(
            "pub use crate::app::host::{save_now, ExportFailure, Host, HostEvent, SaveOutcome};"
        ),
        "src/ui/app.rs が Host 一式を再エクスポートしていない"
    );
    // app 層が GUI 層を逆に参照していないこと（依存の向き ui → app、`overview.md` §3.3）。
    let mut files = Vec::new();
    collect_rs(Path::new("src/app"), &mut files);
    let mut found = Vec::new();
    for file in &files {
        let source = read_core(file);
        for (number, line) in source.lines().enumerate() {
            let code = match line.find("//") {
                Some(at) => &line[..at],
                None => line,
            };
            if code.contains("crate::ui::") && !code.contains("crate::ui::i18n") {
                found.push(format!(
                    "{}:{}: {}",
                    file.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "app 層が ui 層（i18n 以外）を参照している（依存の向きが逆）:\n{}",
        found.join("\n")
    );
}

/// §12.2: CLI（`src/cli/**`・`src/main.rs`）と文言カタログ（`src/ui/i18n/**`）にも
/// ユーザー向け文言（日本語の文字列リテラル）を書かない（CLI も同じカタログを使う）。
#[test]
fn cli_has_no_user_facing_text() {
    let mut files = vec![PathBuf::from("src/main.rs")];
    collect_rs(Path::new("src/cli"), &mut files);
    collect_rs(Path::new("src/ui/i18n"), &mut files);
    assert!(files.len() > 5, "src/cli の .rs が少なすぎる: {files:?}");
    let mut found = Vec::new();
    for file in &files {
        let source = read_core(file);
        let (_, text) = scan_or_panic(&file.display().to_string(), &source);
        found.extend(text);
    }
    assert!(
        found.is_empty(),
        "`docs/specifications/ui.md` §12.2 違反（CLI の文言はカタログに置く）が {} 件:\n{}",
        found.len(),
        bullets(&found)
    );
}

#[test]
fn gui_has_no_user_facing_text() {
    // §19.1「`ui/` 内に i18n を経由しない日本語リテラルが存在しない（例外は i18n カタログのみ）」。
    // GUI のコードは `ui` feature の中だけでビルドされるが、ここはソースを読むので既定のビルドでも検査できる
    let mut files = Vec::new();
    collect_rs(Path::new("src/ui"), &mut files);
    // 段階 ③ H1 で `src/app/**` へ移した状態機械も同じ規則（§19.1.8: 文言はカタログ経由）
    collect_rs(Path::new("src/app"), &mut files);
    assert!(files.len() > 10, "src/ui の .rs が少なすぎる: {files:?}");
    let mut found = Vec::new();
    for file in &files {
        let source = read_core(file);
        let (_, text) = scan_or_panic(&file.display().to_string(), &source);
        found.extend(text);
    }
    assert!(
        found.is_empty(),
        "`docs/specifications/ui.md` §12.2 / §19.1 違反（GUI の文言はカタログに置く）が {} 件:\n{}",
        found.len(),
        bullets(&found)
    );
}

// ---------------------------------------------------------------------------
// §13.1.2: 書き込みの system call は書き込みの関門（`src/adapters/storage_fs.rs`）だけ
//
// 仕様は「`std::fs::write` / `File::create` の直接使用が `adapters/` 以外に無いこと」。第 2.9 版で
// **関門の 1 ファイル**に絞った（`docs/history.md` B-91）。関門の中の書き込みはすべて
// `WritePolicy` の判定を通るので、ここで「関門の外に書き込み API が無い」ことを固定すれば、
// 規則 10（プロジェクト外への書き込み禁止）を迂回する経路がソースに現れない。
// ---------------------------------------------------------------------------

/// 書き込みの関門。書き込み API を直接使ってよい唯一のファイル（§13.1.2 第 2.9 版）。
const WRITE_GATE: &str = "src/adapters/storage_fs.rs";

/// `fs::<これ>` はファイルシステムへの書き込み。
const FS_WRITE_FNS: &[&str] = &[
    "write",
    "rename",
    "copy",
    "create_dir",
    "create_dir_all",
    "remove_file",
    "remove_dir",
    "remove_dir_all",
    "hard_link",
    "soft_link",
    "symlink",
    "symlink_file",
    "symlink_dir",
    "set_permissions",
];

/// `File::<これ>` は書き込み用に開く（`File::open` は読み取りなので許可）。
const FILE_WRITE_FNS: &[&str] = &["create", "create_new", "options"];

/// パス（`::` でつないだ識別子の並び）が書き込み API を指すか。
fn write_api(segments: &[String]) -> Option<String> {
    let joined = segments.join("::");
    if segments.iter().any(|segment| segment == "OpenOptions") {
        return Some(joined);
    }
    for pair in segments.windows(2) {
        let (head, tail) = (pair[0].as_str(), pair[1].as_str());
        let hit = (head == "fs" && (FS_WRITE_FNS.contains(&tail) || tail == "*"))
            || (head == "File" && FILE_WRITE_FNS.contains(&tail))
            || (head == "tokio" && tail == "fs");
        if hit {
            return Some(joined);
        }
    }
    None
}

/// 書き込み API を探す visitor（`use` 文・式・型・属性・マクロ本体）。
#[derive(Default)]
struct WriteApiChecker {
    label: String,
    found: Vec<String>,
}

impl WriteApiChecker {
    fn check(&mut self, segments: &[String]) {
        if let Some(path) = write_api(segments) {
            let message = format!("'{}' in {}", path, self.label);
            self.found.push(message);
        }
    }

    /// `use` の木をパスに展開する（`use std::fs::{self, write};` → `std::fs` と `std::fs::write`）。
    fn walk_use(&mut self, prefix: &mut Vec<String>, tree: &UseTree) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.walk_use(prefix, &path.tree);
                prefix.pop();
            }
            UseTree::Name(name) => {
                let mut full = prefix.clone();
                if name.ident != "self" {
                    full.push(name.ident.to_string());
                }
                self.check(&full);
            }
            UseTree::Rename(rename) => {
                let mut full = prefix.clone();
                full.push(rename.ident.to_string());
                self.check(&full);
            }
            UseTree::Glob(_) => {
                let mut full = prefix.clone();
                full.push("*".to_string());
                self.check(&full);
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    self.walk_use(prefix, item);
                }
            }
        }
    }

    /// マクロ本体のトークン列から `a::b::c` の並びを拾う。
    fn scan_tokens(&mut self, tokens: TokenStream) {
        let mut run: Vec<String> = Vec::new();
        let mut colons = 0;
        for token in tokens {
            match token {
                TokenTree::Ident(ident) => {
                    if colons != 2 {
                        self.flush(&mut run);
                    }
                    run.push(ident.to_string());
                    colons = 0;
                }
                TokenTree::Punct(punct) if punct.as_char() == ':' => colons += 1,
                TokenTree::Group(group) => {
                    self.flush(&mut run);
                    colons = 0;
                    self.scan_tokens(group.stream());
                }
                _ => {
                    self.flush(&mut run);
                    colons = 0;
                }
            }
        }
        self.flush(&mut run);
    }

    fn flush(&mut self, run: &mut Vec<String>) {
        if !run.is_empty() {
            let taken = std::mem::take(run);
            self.check(&taken);
        }
    }
}

impl<'ast> Visit<'ast> for WriteApiChecker {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        let segments: Vec<String> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        self.check(&segments);
        visit::visit_path(self, path);
    }

    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        let mut prefix = Vec::new();
        self.walk_use(&mut prefix, &item.tree);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.scan_tokens(mac.tokens.clone());
        visit::visit_macro(self, mac);
    }
}

fn scan_write_apis(label: &str, source: &str) -> Vec<String> {
    let file = syn::parse_file(source).unwrap_or_else(|e| panic!("{label}: {e}"));
    let mut checker = WriteApiChecker {
        label: label.to_string(),
        ..Default::default()
    };
    checker.visit_file(&file);
    checker.found
}

/// §13.1.2: 書き込み API を直接使うのは書き込みの関門だけ。
#[test]
fn only_the_write_gate_touches_the_file_system_for_writing() {
    let mut files = Vec::new();
    collect_rs(Path::new("src"), &mut files);
    files.sort();
    assert!(
        files.len() > 50,
        "src/ の .rs が少なすぎる（{}）",
        files.len()
    );
    let gate = Path::new(WRITE_GATE);
    let mut found = Vec::new();
    for file in &files {
        if file == gate {
            continue;
        }
        let source = read_core(file);
        found.extend(scan_write_apis(&file.display().to_string(), &source));
    }
    assert!(
        found.is_empty(),
        "`docs/specifications/platform.md` §13.1.2 違反（書き込み API は {WRITE_GATE} だけ）が {} 件:\n{}",
        found.len(),
        bullets(&found)
    );
    // 関門の場所が変わったのに定数が古いまま（＝検査が空振り）にならないように
    let gate_source = read_core(gate);
    assert!(
        !scan_write_apis(WRITE_GATE, &gate_source).is_empty(),
        "{WRITE_GATE} に書き込み API が無い（関門が移動した？）"
    );
}

/// 検出されるべき書き込み（§13.1.2）。
const MUST_CATCH_WRITES: &[&str] = &[
    "fn f() { std::fs::write(\"x\", \"y\").ok(); }",
    "use std::fs; fn f() { fs::write(\"x\", \"y\").ok(); }",
    "use std::fs::write; fn f() { write(\"x\", \"y\").ok(); }",
    "use std::fs::{self, write as w};",
    "use std::fs::*;",
    "use std::fs::File; fn f() { File::create(\"x\").ok(); }",
    "fn f() { std::fs::File::options().write(true); }",
    "use std::fs::OpenOptions;",
    "fn f() { let _ = std::fs::OpenOptions::new(); }",
    "fn f() { std::fs::rename(\"a\", \"b\").ok(); }",
    "fn f() { std::fs::remove_file(\"a\").ok(); }",
    "fn f() { std::fs::create_dir_all(\"a\").ok(); }",
    "fn f() { std::fs::copy(\"a\", \"b\").ok(); }",
    "fn f() { std::os::unix::fs::symlink(\"a\", \"b\").ok(); }",
    "fn f() { let _ = tokio::fs::write(\"a\", \"b\"); }",
    "fn f() { println!(\"{:?}\", std::fs::write(\"a\", \"b\")); }",
];

/// 許されるもの（読み取り・書き込み先の決まった Writer）。
const MUST_PASS_WRITES: &[&str] = &[
    "fn f() { let _ = std::fs::read_to_string(\"x\"); }",
    "use std::fs; fn f() { let _ = fs::read(\"x\"); let _ = fs::metadata(\"x\"); }",
    "use std::fs::File; fn f() { File::open(\"x\").ok(); }",
    "use std::io::Write; fn f(out: &mut Vec<u8>) { out.write_all(b\"x\").ok(); writeln!(out).ok(); }",
    "fn f() { let write = 1; let _ = write; }",
    "fn f() { let _ = std::fs::canonicalize(\".\"); let _ = std::fs::read_dir(\".\"); }",
];

#[test]
fn write_checker_catches_every_write_api() {
    for (i, source) in MUST_CATCH_WRITES.iter().enumerate() {
        let found = scan_write_apis(&format!("must_catch_write[{i}]"), source);
        assert!(!found.is_empty(), "検出漏れ: {source}");
    }
}

#[test]
fn write_checker_allows_reads() {
    for (i, source) in MUST_PASS_WRITES.iter().enumerate() {
        let found = scan_write_apis(&format!("must_pass_write[{i}]"), source);
        assert!(found.is_empty(), "誤検出: {source}: {found:?}");
    }
}

/// §19.7「`characters.json` を参照するコードが存在しない」。
#[test]
fn no_code_mentions_characters_json() {
    let mut files = Vec::new();
    collect_rs(Path::new("src"), &mut files);
    files.sort();
    assert!(!files.is_empty(), "No .rs files found in src!");
    let offenders: Vec<String> = files
        .iter()
        .filter(|file| read_core(file).contains("characters.json"))
        .map(|file| file.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "characters.json を参照している:\n{}",
        bullets(&offenders)
    );
}

/// §19.2「Web 版が LocalStorage を使っていない」。
#[test]
fn no_code_touches_local_storage() {
    let mut files = Vec::new();
    collect_rs(Path::new("src"), &mut files);
    files.sort();
    let offenders: Vec<String> = files
        .iter()
        .filter(|file| {
            let source = read_core(file);
            source.contains("local_storage") || source.contains("localStorage")
        })
        .map(|file| file.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "LocalStorage を参照している:\n{}",
        bullets(&offenders)
    );
}

// ---------------------------------------------------------------------------
// §18.3: `unsafe` は Windows OS FFI の 2 箇所のみ許可
// ---------------------------------------------------------------------------

#[derive(Default)]
struct UnsafeChecker {
    label: String,
    unsafe_blocks: Vec<String>,
    other_unsafe: Vec<String>,
}

impl<'ast> Visit<'ast> for UnsafeChecker {
    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        let body = quote_tokens(&node.block);
        self.unsafe_blocks.push(format!("{}: {body}", self.label));
        visit::visit_expr_unsafe(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if node.sig.unsafety.is_some() {
            self.other_unsafe
                .push(format!("{}: unsafe fn {}", self.label, node.sig.ident));
        }
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if node.sig.unsafety.is_some() {
            self.other_unsafe
                .push(format!("{}: unsafe fn {}", self.label, node.sig.ident));
        }
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        if node.sig.unsafety.is_some() {
            self.other_unsafe
                .push(format!("{}: unsafe fn {}", self.label, node.sig.ident));
        }
        visit::visit_trait_item_fn(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if node.unsafety.is_some() {
            self.other_unsafe
                .push(format!("{}: unsafe impl", self.label));
        }
        visit::visit_item_impl(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        if node.unsafety.is_some() {
            self.other_unsafe
                .push(format!("{}: unsafe trait {}", self.label, node.ident));
        }
        visit::visit_item_trait(self, node);
    }
}

#[derive(Default)]
struct CallCollector {
    calls: Vec<String>,
}

impl<'ast> Visit<'ast> for CallCollector {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            let name = path
                .path
                .segments
                .iter()
                .map(|seg| seg.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            self.calls.push(name);
        } else {
            self.calls.push("<expr>".to_string());
        }
        visit::visit_expr_call(self, node);
    }
}

fn quote_tokens(block: &syn::Block) -> String {
    let mut collector = CallCollector::default();
    collector.visit_block(block);
    collector.calls.join(", ")
}

fn scan_unsafe(label: &str, source: &str) -> UnsafeChecker {
    let file = syn::parse_file(source).unwrap_or_else(|e| panic!("{label}: {e}"));
    let mut checker = UnsafeChecker {
        label: label.to_string(),
        ..Default::default()
    };
    checker.visit_file(&file);
    checker
}

/// §18.3: `unsafe` は Windows のコンソール接続 `AttachConsole`（`src/main.rs`）と
/// OS の既定ロケール取得 `GetUserDefaultLocaleName`（`src/adapters/env_system.rs`）の
/// 2 箇所を除き使わない（MUST）。`core/` は `unsafe` 0 件。
#[test]
fn unsafe_is_restricted_to_the_two_windows_ffi_sites() {
    let mut files = Vec::new();
    collect_rs(Path::new("src"), &mut files);
    files.sort();
    let allowed = [
        (Path::new("src/main.rs"), "AttachConsole"),
        (
            Path::new("src/adapters/env_system.rs"),
            "GetUserDefaultLocaleName",
        ),
    ];
    let mut unexpected = Vec::new();
    for file in &files {
        let label = file.display().to_string();
        let source = read_core(file);
        let checked = scan_unsafe(&label, &source);
        unexpected.extend(checked.other_unsafe);
        if let Some((_, expected_symbol)) = allowed.iter().find(|(path, _)| *path == file.as_path())
        {
            assert_eq!(
                checked.unsafe_blocks.len(),
                1,
                "{label} の unsafe ブロックは 1 箇所のみのはず: {:?}",
                checked.unsafe_blocks
            );
            assert!(
                checked.unsafe_blocks[0].contains(expected_symbol),
                "{label} の unsafe ブロックが {expected_symbol} ではない: {}",
                checked.unsafe_blocks[0]
            );
        } else {
            unexpected.extend(checked.unsafe_blocks);
        }
    }
    assert!(
        unexpected.is_empty(),
        "`docs/specifications/workflows.md` §18.3 違反（許可された 2 箇所外の unsafe）:\n{}",
        bullets(&unexpected)
    );
}

#[test]
fn unsafe_checker_catches_unsafe_constructs() {
    for sample in [
        "fn f() { unsafe { let _ = 1; } }",
        "unsafe fn f() {}",
        "struct S; unsafe impl Send for S {}",
        "unsafe trait T {}",
    ] {
        let checked = scan_unsafe("sample", sample);
        assert!(
            !checked.unsafe_blocks.is_empty() || !checked.other_unsafe.is_empty(),
            "検出漏れ: {sample}"
        );
    }
}

/// §19.13-2: `core::features` の 11 機能（§3.4 の表）がすべて `ui` から直接呼ばれている。
#[test]
fn ui_calls_all_eleven_lsp_feature_modules() {
    let expected = [
        ("semantic_tokens", "full"),
        ("completion", "at"),
        ("hover", "at"),
        ("inlay_hint", "range"),
        ("diagnostics", "document"),
        ("code_action", "at"),
        ("definition", "at"),
        ("references", "at"),
        ("document_symbol", "all"),
        ("formatting", "document"),
        ("rename", "prepare"),
        ("rename", "apply"),
        ("folding", "ranges"),
    ];
    let mut ui_files = Vec::new();
    collect_rs(Path::new("src/ui"), &mut ui_files);
    ui_files.sort();
    let mut all_calls = Vec::new();
    for file in &ui_files {
        let source = read_core(file);
        let parsed = syn::parse_file(&source).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let mut collector = CallCollector::default();
        collector.visit_file(&parsed);
        all_calls.extend(collector.calls);
    }
    let mut missing = Vec::new();
    for (module, func) in expected {
        let suffix = format!("{module}::{func}");
        let called = all_calls
            .iter()
            .any(|call| call == &suffix || call.ends_with(&format!("::{suffix}")));
        if !called {
            missing.push(suffix);
        }
    }
    assert!(
        missing.is_empty(),
        "`docs/specifications/interfaces.md` §19.1.2 違反（ui から呼ばれていない core::features 関数）:\n{}",
        bullets(&missing)
    );
}
