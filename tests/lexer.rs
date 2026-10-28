//! `core::text::lexer` のテスト（§5.0 の共通字句規則）。
//!
//! `docs/history.md` の完了条件「`tests/lexer.rs`: §5.0.2 のシジル表と §5.0.3 のエスケープ表の
//! **全行にテストがある**」に対応する。① で実装しない行（`'` / `'{ }` / `:` / `+` / `「」` /
//! `^` / `..`）は「字句器が行の種別を変えないこと」と、表に行が残って ② の受け皿になっている
//! ことを確認する。
//!
//! 比較は [`tag`] が作る短い文字列で行う。`assert_eq!` の引数に構造体リテラルを直接書くと
//! rustfmt が必ず折り返すので、期待値を 1 行に収めるためでもある。

use std::borrow::Cow;
use std::collections::HashSet;

use novnow::core::id::RangeExpr;
use novnow::core::text::lexer::{
    self, FieldKey, Formats, LineKind, Newline, Sigil, Stage, TextFormat, FIELD_KEYS, SIGIL_TABLE,
};

/// C 形式（`contents/AA-BB.cnov`、§5.2）。
const C: TextFormat = TextFormat::Contents;
/// A 形式（`plots/all.cnov` / `plots/AA.cnov`、§5.3）。
const A: TextFormat = TextFormat::PlotOutline;
/// B 形式（`plots/AA-BB.cnov`、§5.4）。
const B: TextFormat = TextFormat::PlotEpisode;
/// `*.md`（§5.5–§5.7）。
const MD: TextFormat = TextFormat::Markdown;

/// §5.0.3 のエスケープ表（有効な 7 行）。`(書いた形, エスケープされる文字, 解除後)`。
const ESCAPES: &[(&str, char, &str)] = &[
    ("\\@", '@', "@"),
    ("\\'", '\'', "'"),
    ("\\;", ';', ";"),
    ("\\/", '/', "/"),
    ("\\「", '「', "「"),
    ("\\」", '」', "」"),
    ("\\\\", '\\', "\\"),
];

/// §5.0.5 の不変条件（MUST）に使う、URL scheme と本文中に書かれがちな接頭辞。
///
/// §5.0.6 のキー表と**交わってはいけない**。交わると `http://example.com` のような行が
/// フィールド行になってしまう（手順 5 が「表にあるキーは空白を要求しない」ため）。
const SCHEMES: &[&str] = &[
    "about",
    "arxiv",
    "bitcoin",
    "blob",
    "chrome",
    "data",
    "doi",
    "ed2k",
    "feed",
    "file",
    "ftp",
    "ftps",
    "gemini",
    "geo",
    "git",
    "gopher",
    "http",
    "https",
    "isbn",
    "javascript",
    "jdbc",
    "ldap",
    "magnet",
    "mail",
    "mailto",
    "matrix",
    "news",
    "nfs",
    "nntp",
    "pmid",
    "rtsp",
    "s3",
    "sftp",
    "sip",
    "smb",
    "sms",
    "ssh",
    "svn",
    "tel",
    "urn",
    "view-source",
    "webcal",
    "ws",
    "wss",
];

/// ① の範囲で 1 行を字句解析し、比較しやすい形にする。
fn tag(line: &str, format: TextFormat) -> String {
    tag_at(line, format, Stage::One)
}

/// `stage` の範囲で 1 行を字句解析し、比較しやすい形にする。
fn tag_at(line: &str, format: TextFormat, stage: Stage) -> String {
    match lexer::classify_at_stage(line, format, stage) {
        LineKind::Structure(rest) => format!("structure[{rest}]"),
        LineKind::SectionBreak => "section-break".to_owned(),
        LineKind::Comment { sigil, text } => format!("comment({sigil:?})[{text}]"),
        LineKind::Field(field) => {
            let key = format!("{:?}", field.key);
            format!("field({key}, {})[{}]", field.key_text, field.value)
        }
        LineKind::Continuation { indent, text } => format!("continuation({indent})[{text}]"),
        LineKind::IllustrationMemo(text) => format!("illustration[{text}]"),
        LineKind::Text(text) => format!("text[{text}]"),
    }
}

/// [`SIGIL_TABLE`] から指定のシジルの「出現場所」を引く。
fn sigil_formats(sigil: Sigil) -> Formats {
    SIGIL_TABLE
        .iter()
        .find(|spec| spec.sigil == sigil)
        .map(|spec| spec.formats)
        .expect("sigil is not in SIGIL_TABLE")
}

// ============================================================ §5.0.2 `@` StructureSigil

#[test]
fn at_line_head_is_structure_in_every_format() {
    for format in [C, A, B, MD] {
        let got = tag("@01-04-02 見出し", format);
        assert_eq!(got, "structure[01-04-02 見出し]");
    }
}

#[test]
fn structure_sigil_does_not_need_a_title() {
    let got = tag("@01-04-02-03", C);
    assert_eq!(got, "structure[01-04-02-03]");
}

#[test]
fn at_any_other_position_does_not_change_the_line_kind() {
    // 行末の話者注釈（§5.2.3）と本文中の裸の `@`（W002）は M2 のパーサが扱う
    let got = tag("「こんにちは」@恵", C);
    assert_eq!(got, "text[「こんにちは」@恵]");
}

#[test]
fn an_escaped_at_is_not_a_sigil() {
    // §5.0.3 の `\@`（行頭 `@` の回避）
    let got = tag("\\@01-04", C);
    assert_eq!(got, "text[\\@01-04]");
}

// ============================================================ §5.0.2 `'` `'{ }` `:` `+`

#[test]
fn ref_sigils_do_not_change_the_line_kind() {
    // 参照シジル・明示形・修飾子・モード併用は行の種別を変えない（M2 / M3 の仕事）
    for line in ["'恵", "'{恵}", "'恵:B", "'恵:B+D", "本文'恵", "+D"] {
        let got = tag(line, C);
        assert_eq!(got, format!("text[{line}]"));
    }
}

// ============================================================ §5.0.2 `;` SectionBreak

#[test]
fn section_break_is_a_whole_line_in_every_txt_form() {
    // §5.2.1 / §5.3.1 / §5.4.1（`docs/history.md` A-11）
    for format in [C, B, A] {
        let got = tag(";", format);
        assert_eq!(got, "section-break");
    }
}

#[test]
fn section_break_tolerates_trailing_space() {
    // §5.0.4 で保存時に消えるが、編集中の行末空白でも節区切りとして扱う
    let got = tag(";   ", C);
    assert_eq!(got, "section-break");
}

#[test]
fn section_break_needs_the_whole_line() {
    // §5.0.2「行全体が `;` のときのみ」。`;` の後ろに本文があれば節区切りではない
    for line in ["; 注", "a;", "；", "\\;", ";;"] {
        let got = tag(line, C);
        assert_eq!(got, format!("text[{line}]"));
    }
}

#[test]
fn section_break_allows_a_trailing_line_comment() {
    // §5.2.1 `section_break = ws? , ";" , [ comment ] , eol`（`docs/history.md` B-31）
    for line in ["; // メモ", ";  // メモ", ";//メモ", "\t;\t// メモ"] {
        let got = tag(line, C);
        assert_eq!(got, "section-break");
    }
    // `://` は行内コメントにしない（§5.0.2）ので、これは本文のまま
    let url = tag("; https://example.com", C);
    assert_eq!(url, "text[; https://example.com]");
}

#[test]
fn a_line_head_sigil_tolerates_one_space_of_indent() {
    // §5.2.1 の各行は `ws?` で始まる。§5.0.5 のフィールド行と同じ 0〜1 文字
    let scene = tag(" @01-04 見出し", C);
    assert_eq!(scene, "structure[01-04 見出し]");
    let wide = tag("　@01-04", C);
    assert_eq!(wide, "structure[01-04]");
    let comment = tag(" // メモ", C);
    assert_eq!(comment, "comment(LineComment)[ メモ]");
    // 2 文字以上は継続行（§5.0.5、B-18）。シジルより優先する
    let deep = tag("  @01-04 見出し", C);
    assert_eq!(deep, "continuation(2)[@01-04 見出し]");
    let deep_break = tag("　　;", C);
    assert_eq!(deep_break, "continuation(2)[;]");
}

#[test]
fn section_break_is_rejected_in_markdown() {
    // §5.0.2 の `;` の出現場所は `.txt` 3 形式。`.md` では通常テキストのまま
    let got = tag(";", MD);
    assert_eq!(got, "text[;]");
}

// ============================================================ §5.0.2 `//` LineComment

#[test]
fn line_comment_at_line_head_in_every_format() {
    for format in [C, A, B, MD] {
        let got = tag("// メモ", format);
        assert_eq!(got, "comment(LineComment)[ メモ]");
    }
}

#[test]
fn line_comment_runs_to_the_end_of_the_line() {
    let line = "本文 // 注";
    let got = lexer::find_line_comment(line);
    assert_eq!(got, Some(7));
    assert_eq!(lexer::strip_line_comment(line), "本文 ");
    let head = lexer::find_line_comment("// 注");
    assert_eq!(head, Some(0));
}

#[test]
fn a_scheme_separator_is_not_a_line_comment() {
    // §5.0.2「`://` は行内コメントにしない（MUST）」
    let url = "https://example.com";
    assert_eq!(lexer::find_line_comment(url), None);
    assert_eq!(lexer::strip_line_comment(url), url);
    let mixed = "source: https://example.com // 注";
    let got = lexer::find_line_comment(mixed);
    assert_eq!(got, Some(28));
    let stripped = lexer::strip_line_comment(mixed);
    assert_eq!(stripped, "source: https://example.com ");
}

#[test]
fn only_the_slash_pair_right_after_a_colon_is_protected() {
    let line = "a://b//c";
    let got = lexer::find_line_comment(line);
    assert_eq!(got, Some(5));
    // 行頭の `//` はこれまでどおりコメント
    assert_eq!(lexer::find_line_comment("// 注"), Some(0));
}

#[test]
fn an_escaped_slash_does_not_start_a_line_comment() {
    // §5.0.3 の `\/`。**無効な** `\x` も 2 文字を消費するので `\/x//` の `//` は残る
    let got = lexer::find_line_comment("本文 \\/\\/ 注");
    assert_eq!(got, None);
    let head = tag("\\// メモ", C);
    assert_eq!(head, "text[\\// メモ]");
}

// ============================================================ §5.0.2 `/* */` InlineComment

#[test]
fn inline_comment_must_close_on_the_same_line() {
    let line = "本文 /* 注 */ 続き";
    let got = lexer::find_inline_comment(line);
    assert_eq!(got, Some(7..16));
    let unclosed = lexer::find_inline_comment("本文 /* 注");
    assert_eq!(unclosed, None);
}

#[test]
fn inline_comment_does_not_change_the_line_kind() {
    // 行内コメントの除去は形式ごとのパーサ（M2）の仕事
    let got = tag("本文 /* 注 */ 続き", C);
    assert_eq!(got, "text[本文 /* 注 */ 続き]");
}

#[test]
fn an_escaped_slash_does_not_open_an_inline_comment() {
    let got = lexer::find_inline_comment("\\/* 注 */");
    assert_eq!(got, None);
}

// ============================================================ §5.0.2 `# ` / `! `（②）

#[test]
fn hash_comments_are_active_but_illustrations_remain_reserved() {
    // Additional specification 15 is active independently of 23.
    let hash = tag("# メモ", A);
    assert_eq!(hash, "comment(HashComment)[メモ]");
    let memo = tag("! 挿絵メモ", C);
    assert_eq!(memo, "illustration[挿絵メモ]");
}

#[test]
fn stage_two_accepts_them_without_rewriting_the_lexer() {
    // 縫い代 4: 表の `stage` を切り替えるだけで ② が有効になる
    let hash = tag_at("# メモ", A, Stage::Two);
    assert_eq!(hash, "comment(HashComment)[メモ]");
    let memo = tag_at("! 挿絵", C, Stage::Two);
    assert_eq!(memo, "illustration[挿絵]");
}

#[test]
fn hash_comment_needs_a_space_even_in_stage_two() {
    // §5.0.2 MUST: `#1 位` や `!?` を誤認しない
    for line in ["#1 位", "#メモ", "#", "#x"] {
        let got = tag_at(line, A, Stage::Two);
        assert_eq!(got, format!("text[{line}]"));
    }
    // 全角空白でもよい（§5.0.2「半角または全角の空白が必要」）
    let full_width = tag_at("#　メモ", A, Stage::Two);
    assert_eq!(full_width, "comment(HashComment)[メモ]");
    // 空白だけの行は空のコメント
    let empty = tag_at("#　", A, Stage::Two);
    assert_eq!(empty, "comment(HashComment)[]");
}

#[test]
fn illustration_memo_is_contents_only() {
    // §5.0.2 の出現場所は C 形式だけ
    for format in [A, B, MD] {
        let got = tag_at("! 挿絵", format, Stage::Two);
        assert_eq!(got, "text[! 挿絵]");
    }
}

#[test]
fn hash_comment_is_cnov_only() {
    // §5.0.8: `.md` では `#` は見出しなのでコメントにしない
    let got = tag_at("# メモ", MD, Stage::Two);
    assert_eq!(got, "text[# メモ]");
}

// ============================================================ §5.0.2 `「」` `^` `..`

#[test]
fn dialogue_brackets_are_plain_text_to_the_lexer() {
    // `「」` のトークン分割は M2 / M3（§6.2）。`『』` も入れ子の引用として同様（§5.0.2）
    let got = tag("「『こんにちは』」", C);
    assert_eq!(got, "text[「『こんにちは』」]");
}

#[test]
fn citation_sigil_is_plain_text_in_stage_one() {
    // `^` 引用注釈は ②（`docs/plans.md` §4.2 の 10、§5.2.4）
    let got = tag("本文^M0137", C);
    assert_eq!(got, "text[本文^M0137]");
}

#[test]
fn the_range_operator_belongs_to_core_id() {
    // §5.0.2 の `..` は範囲式の区切り。字句器は行の種別を変えず、展開は `core::id`（M1）が持つ
    let got = tag("range: @01 .. @02", A);
    assert_eq!(got, "field(Range, range)[@01 .. @02]");
    let expr = RangeExpr::parse("@01 .. @02");
    assert!(expr.is_ok());
}

// ============================================================ §5.0.2 / §5.0.5 フィールド行

#[test]
fn field_line_in_a_b_and_markdown_but_not_in_c() {
    // §5.0.2 の `key:` の出現場所は「A/B 形式, `*.md`」
    for format in [A, B, MD] {
        let got = tag("summary: 一行", format);
        assert_eq!(got, "field(Summary, summary)[一行]");
    }
    let prose = tag("summary: 一行", C);
    assert_eq!(prose, "text[summary: 一行]");
}

#[test]
fn a_full_width_colon_does_not_need_a_space() {
    // §5.0.5 手順 4（ユーザー裁定 2026-10-11）: 日本語の自然な書き方を受ける
    let tight = tag("要約：あらすじ", A);
    assert_eq!(tight, "field(Summary, 要約)[あらすじ]");
    let spaced = tag("要約： あらすじ", A);
    assert_eq!(spaced, "field(Summary, 要約)[あらすじ]");
    // 全角コロンのときは表に無い ascii キーでもフィールド行（W011 行き）
    let unknown = tag("foo：bar", A);
    assert_eq!(unknown, "field(Unknown, foo)[bar]");
}

#[test]
fn a_half_width_colon_needs_a_space_for_keys_outside_the_table() {
    // §5.0.5 手順 5。表に無い ascii キーは空白が無ければフィールド行にしないので、
    // URL 判定の**除外リストを持たない**（MUST）
    let url = tag("http://example.com", A);
    assert_eq!(url, "text[http://example.com]");
    let unknown = tag("foo:bar", A);
    assert_eq!(unknown, "text[foo:bar]");
    // 空白があれば未知のキーでもフィールド行（W011）
    let spaced = tag("foo: bar", A);
    assert_eq!(spaced, "field(Unknown, foo)[bar]");
}

#[test]
fn a_key_in_the_table_does_not_need_a_space_after_a_half_width_colon() {
    // §5.0.5 手順 5: 表にあるキーは scheme と衝突しない（下の不変条件）ので空白を要求しない
    let tight = tag("summary:一行", A);
    assert_eq!(tight, "field(Summary, summary)[一行]");
    let alias = tag("要約:一行", A);
    assert_eq!(alias, "field(Summary, 要約)[一行]");
    let mode = tag("mode:B", B);
    assert_eq!(mode, "field(Mode, mode)[B]");
}

#[test]
fn a_url_in_a_field_value_is_kept_whole() {
    // 値の中身は検査しない（§5.0.5）。`://` は行内コメントにもならない（§5.0.2）
    let tight = tag("source:https://example.com/x?a=1", MD);
    assert_eq!(tight, "field(Source, source)[https://example.com/x?a=1]");
    let spaced = tag("source: https://example.com", MD);
    assert_eq!(spaced, "field(Source, source)[https://example.com]");
}

#[test]
fn urls_and_identifiers_stay_in_the_prose() {
    // §5.0.5「URL 判定は除外リストを持たない（MUST）」の確認
    let prose = [
        "http://example.com",
        "https://example.com/x?a=1#b",
        "ftp://ftp.example.com/pub",
        "mailto:foo@example.com",
        "doi:10.1000/xyz",
        "isbn:978-4-06-519794-1",
        "tel:090-1234-5678",
        "file:///tmp/x.txt",
        "git@github.com:sonneko/novnow.git",
        "C:\\Users\\writer\\draft.txt",
        "10:30",
        "3:4",
        "注：これは本文",
    ];
    for line in prose {
        let got = tag(line, A);
        assert_eq!(got, format!("text[{line}]"));
    }
}

#[test]
fn no_field_key_collides_with_a_url_scheme() {
    // §5.0.5 の不変条件（MUST）。表に scheme と同名のキーを足すと URL がフィールド化するので、
    // ここで固定する（キーを足すときは §5.0.5 の手順 5 を読むこと）
    for scheme in SCHEMES {
        let key = lexer::lookup_key(scheme);
        assert_eq!(key, FieldKey::Unknown, "{scheme}");
    }
}

#[test]
fn field_value_may_be_empty() {
    // §5.0.5 手順 6。§5.0.4 で行末空白は消えるので、`key:` だけの行も
    // 空の値のフィールド行として受理する（`docs/history.md` A-6 の裁定 (1)）
    let bare = tag("summary:", A);
    assert_eq!(bare, "field(Summary, summary)[]");
    let spaced = tag("summary: ", A);
    assert_eq!(spaced, "field(Summary, summary)[]");
    let full_width = tag("要約：", A);
    assert_eq!(full_width, "field(Summary, 要約)[]");
}

#[test]
fn field_value_is_trimmed_but_the_key_is_not_normalized() {
    // §5.0.6「保存時に正準形へ正規化しない」（作家の表記を保つ）
    let got = tag("要約:   一行  ", A);
    assert_eq!(got, "field(Summary, 要約)[一行]");
}

#[test]
fn field_may_have_one_space_of_indent() {
    // §5.0.5 の ebnf は `ws? field_key ...`（0〜1 文字）
    let got = tag(" summary: x", A);
    assert_eq!(got, "field(Summary, summary)[x]");
}

#[test]
fn field_line_exposes_indent_key_and_value() {
    let line = " summary: 一行";
    match lexer::classify(line, A) {
        LineKind::Field(field) => {
            assert_eq!(field.indent, 1);
            assert_eq!(field.key, FieldKey::Summary);
            assert_eq!(field.key_text, "summary");
            assert_eq!(field.value, "一行");
        }
        other => panic!("not a field line: {other:?}"),
    }
}

/// `value_at`（値の 1 byte 目の**行内**位置）だけを取り出す。
fn value_at(line: &str) -> usize {
    match lexer::classify(line, A) {
        LineKind::Field(field) => field.value_at,
        other => panic!("not a field line: {other:?}"),
    }
}

#[test]
fn field_line_exposes_the_byte_position_of_the_value() {
    // 診断の範囲を値の中の一部に作るときに使う（A 形式の W019 は値の範囲、
    // B 形式の E005 は `mode:` の値の中のトークン、§5.4.2）
    assert_eq!(value_at("summary: あらすじ"), 9);
    // 全角コロンは 3 byte。位置は byte で数える（§3.4）
    assert_eq!(value_at(" 要約：あらすじ"), 10);
    // 値の前の空白はすべて飛ぶ（§5.0.5 手順 6）
    assert_eq!(value_at("summary:   x"), 11);
    // 値が空のときは行末（範囲が空になる位置）
    assert_eq!(value_at("space:  "), 8);
    assert_eq!(value_at("summary:"), 8);
    // 行末コメントは**値の一部**。フィールド行では `//` で切らない（`docs/history.md` B-36）
    assert_eq!(value_at("mode: 'アルス:A // 戦闘"), 6);
}

#[test]
fn unknown_ascii_key_is_kept_as_unknown() {
    // §5.0.6「未知のキーは W011 とし、値は改変せず保持する」
    let got = tag("foo: bar", A);
    assert_eq!(got, "field(Unknown, foo)[bar]");
    let long = tag("a-b-c: x", B);
    assert_eq!(long, "field(Unknown, a-b-c)[x]");
}

#[test]
fn a_japanese_word_outside_the_alias_table_is_not_a_key() {
    // §5.0.5 の ebnf は `field_key = ascii_key | ja_alias`。表に無い日本語は散文
    let got = tag("注：これは本文", A);
    assert_eq!(got, "text[注：これは本文]");
}

#[test]
fn ascii_key_follows_the_regular_expression() {
    // §5.0.5 `ascii_key = /[a-z][a-z0-9-]{0,23}/`
    assert!(lexer::is_ascii_key("a"));
    assert!(lexer::is_ascii_key("default-mode"));
    assert!(lexer::is_ascii_key("k1"));
    assert!(!lexer::is_ascii_key("A"));
    assert!(!lexer::is_ascii_key("1abc"));
    assert!(!lexer::is_ascii_key("a_b"));
    assert!(!lexer::is_ascii_key(""));
    let max = "a".repeat(24);
    assert!(lexer::is_ascii_key(&max));
    let too_long = "a".repeat(25);
    assert!(!lexer::is_ascii_key(&too_long));
}

// ============================================================ §5.0.5 継続行

#[test]
fn the_spec_example_splits_into_a_field_and_a_continuation() {
    let first = tag("summary: 一行目", A);
    assert_eq!(first, "field(Summary, summary)[一行目]");
    let second = tag("  二行目（継続行）", A);
    assert_eq!(second, "continuation(2)[二行目（継続行）]");
}

#[test]
fn two_or_more_spaces_of_indent_beat_the_field_line() {
    // 2 スペース以上は継続行のために予約されている（`docs/history.md` B-18）
    let got = tag("  summary: x", A);
    assert_eq!(got, "continuation(2)[summary: x]");
    let deeper = tag("     x", A);
    assert_eq!(deeper, "continuation(5)[x]");
}

#[test]
fn one_space_of_indent_is_not_a_continuation() {
    let got = tag(" 二行目", A);
    assert_eq!(got, "text[ 二行目]");
}

#[test]
fn indent_counts_chars_and_a_single_tab_is_a_continuation() {
    // §5.0.5 の継続行は「2 文字以上の空白、または 1 文字以上のタブ」（`docs/history.md` B-20・B-21）
    assert_eq!(lexer::leading_indent("  x"), 2);
    assert_eq!(lexer::leading_indent("\tx"), 1);
    assert_eq!(lexer::leading_indent("　x"), 1);
    assert_eq!(lexer::leading_indent(" \t　x"), 3);
    assert_eq!(lexer::leading_indent("x"), 0);
    assert_eq!(lexer::after_indent(" \t　x"), "x");
    assert!(lexer::is_continuation_indent("  x"));
    assert!(lexer::is_continuation_indent("\tx"));
    assert!(!lexer::is_continuation_indent(" x"));
    assert!(!lexer::is_continuation_indent("　x"));

    let tab = tag("\t二行目", A);
    assert_eq!(tab, "continuation(1)[二行目]");
    let tab_field = tag("\tsummary: x", A);
    assert_eq!(tab_field, "continuation(1)[summary: x]");
    let tabs = tag("\t\t二行目", A);
    assert_eq!(tabs, "continuation(2)[二行目]");
    let wide = tag("　　二行目", A);
    assert_eq!(wide, "continuation(2)[二行目]");
}

// ============================================================ §5.0.6 別名表

#[test]
fn every_key_in_the_table_resolves_both_ways() {
    // §5.0.6「固定・双方向 1:1」
    assert_eq!(FIELD_KEYS.len(), 46);
    for (field, key, alias, _) in FIELD_KEYS {
        assert_eq!(lexer::lookup_key(key), *field, "ascii: {key}");
        assert_eq!(lexer::lookup_key(alias), *field, "alias: {alias}");
        assert_eq!(field.ascii_key(), Some(*key), "ascii: {key}");
        assert_eq!(field.alias(), Some(*alias), "alias: {alias}");
    }
    assert_eq!(FieldKey::Unknown.ascii_key(), None);
    assert_eq!(FieldKey::Unknown.alias(), None);
}

#[test]
fn the_alias_table_is_one_to_one_in_both_directions() {
    let keys: HashSet<&str> = FIELD_KEYS.iter().map(|row| row.1).collect();
    let aliases: HashSet<&str> = FIELD_KEYS.iter().map(|row| row.2).collect();
    assert_eq!(keys.len(), FIELD_KEYS.len());
    assert_eq!(aliases.len(), FIELD_KEYS.len());
    // 別名が正準形と衝突しないことも 1:1 の一部
    assert!(keys.is_disjoint(&aliases));
}

#[test]
fn the_table_records_the_formats_of_each_key() {
    // §5.0.6 の「使用形式」列
    assert_eq!(FieldKey::Summary.formats(), Some(Formats::AB));
    assert_eq!(FieldKey::Turn.formats(), Some(Formats::PLOT_OUTLINE));
    assert_eq!(FieldKey::Mode.formats(), Some(Formats::PLOT_EPISODE));
    assert_eq!(FieldKey::Note.formats(), Some(Formats::FIELD_FORMATS));
    assert_eq!(FieldKey::Tone.formats(), Some(Formats::MARKDOWN));
    assert_eq!(FieldKey::Unknown.formats(), None);
    // 第 2.6 版で追加した 10 キーの「使用形式」列
    assert_eq!(FieldKey::Title.formats(), Some(Formats::PLOT_OUTLINE));
    for key in [
        FieldKey::Color,
        FieldKey::Status,
        FieldKey::Created,
        FieldKey::Relates,
        FieldKey::PromotedTo,
        FieldKey::ModelHint,
        FieldKey::MaxChars,
        FieldKey::Temperature,
        FieldKey::Label,
    ] {
        assert_eq!(key.formats(), Some(Formats::MARKDOWN), "{key:?}");
    }
}

#[test]
fn stage_two_keys_are_in_the_table() {
    // ② のキー（`docs/plans.md` §4.2 ）も §5.0.6 の表にあるので Unknown にならない
    let stage_two = [
        "when",
        "senses",
        "space",
        "space-mode",
        "illustration",
        "blurb",
        "address",
        "icon",
        "日時",
        "五感",
        "空間",
        "挿絵",
        "あらすじ",
        "呼称",
        "アイコン",
    ];
    for key in stage_two {
        assert_ne!(lexer::lookup_key(key), FieldKey::Unknown, "{key}");
    }
}

#[test]
fn every_alias_in_the_spec_table_is_present() {
    // §5.0.6 の表（① の 38 件）を 1 行ずつ確認する
    let spec = [
        (FieldKey::Summary, "要約"),
        (FieldKey::Goal, "目的"),
        (FieldKey::Turn, "転換"),
        (FieldKey::State, "状態"),
        (FieldKey::Cast, "配役"),
        (FieldKey::Mode, "モード"),
        (FieldKey::Pov, "視点"),
        (FieldKey::Seed, "伏線"),
        (FieldKey::Payoff, "回収"),
        (FieldKey::Material, "資料"),
        (FieldKey::Length, "分量"),
        (FieldKey::Check, "確認"),
        (FieldKey::Note, "メモ"),
        (FieldKey::Alias, "別名"),
        (FieldKey::Range, "範囲"),
        (FieldKey::Reading, "読み"),
        (FieldKey::Definition, "定義"),
        (FieldKey::Category, "分類"),
        (FieldKey::Tone, "口調"),
        (FieldKey::Personality, "性格"),
        (FieldKey::Emotion, "感情"),
        (FieldKey::Role, "役割"),
        (FieldKey::Relation, "関係"),
        (FieldKey::DefaultMode, "既定モード"),
        (FieldKey::Tags, "タグ"),
        (FieldKey::Source, "出典"),
        (FieldKey::Condition, "条件"),
        (FieldKey::Context, "文脈"),
        // 第 2.6 版で追加した 10 キー（A-12・A-15）
        (FieldKey::Title, "題"),
        (FieldKey::Color, "色"),
        (FieldKey::Status, "状況"),
        (FieldKey::Created, "作成日時"),
        (FieldKey::Relates, "関連"),
        (FieldKey::PromotedTo, "昇格先"),
        (FieldKey::ModelHint, "モデル指定"),
        (FieldKey::MaxChars, "最大文字数"),
        (FieldKey::Temperature, "温度"),
        (FieldKey::Label, "表示名"),
    ];
    for (field, alias) in spec {
        assert_eq!(lexer::lookup_key(alias), field, "{alias}");
    }
}

// ============================================================ §5.0.2 シジル表そのもの

#[test]
fn the_sigil_table_is_the_fixed_priority_order() {
    // §5.0.2「行頭シジルの優先順位（固定）: `@` → `;` → `# ` → `! ` → `//`」
    let order: Vec<Sigil> = SIGIL_TABLE.iter().map(|spec| spec.sigil).collect();
    let want = [
        Sigil::Structure,
        Sigil::SectionBreak,
        Sigil::HashComment,
        Sigil::IllustrationMemo,
        Sigil::LineComment,
    ];
    assert_eq!(order, want);
}

#[test]
fn the_table_records_the_place_and_shape_of_each_sigil() {
    // §5.0.2 の「出現場所」列
    assert_eq!(sigil_formats(Sigil::Structure), Formats::ALL);
    assert_eq!(sigil_formats(Sigil::LineComment), Formats::ALL);
    assert_eq!(sigil_formats(Sigil::SectionBreak), Formats::CNOV);
    assert_eq!(sigil_formats(Sigil::HashComment), Formats::CNOV);
    assert_eq!(sigil_formats(Sigil::IllustrationMemo), Formats::CONTENTS);

    // 直後の空白を要求するのは `# ` と `! ` だけ（§5.0.2 MUST）
    for spec in SIGIL_TABLE {
        let want_space = matches!(spec.sigil, Sigil::HashComment | Sigil::IllustrationMemo);
        assert_eq!(spec.requires_space, want_space, "{:?}", spec.sigil);
        // 行全体を要求するのは `;` だけ
        assert_eq!(spec.whole_line, spec.sigil == Sigil::SectionBreak);
    }
}

#[test]
fn formats_are_a_bitmask_of_the_four_text_forms() {
    assert!(Formats::ALL.contains(C));
    assert!(Formats::ALL.contains(A));
    assert!(Formats::ALL.contains(B));
    assert!(Formats::ALL.contains(MD));
    assert!(!Formats::NONE.contains(C));
    assert!(Formats::CNOV.contains(C));
    assert!(!Formats::CNOV.contains(MD));
    assert!(Formats::FIELD_FORMATS.contains(MD));
    assert!(!Formats::FIELD_FORMATS.contains(C));
    let union = Formats::CONTENTS.union(Formats::MARKDOWN);
    assert!(union.contains(C));
    assert!(union.contains(MD));
    assert!(!union.contains(A));
}

#[test]
fn text_format_helpers_agree_with_the_table() {
    assert!(C.is_cnov());
    assert!(A.is_cnov());
    assert!(B.is_cnov());
    assert!(!MD.is_cnov());
    assert!(!C.allows_field());
    assert!(A.allows_field());
    // `;` は `.cnov` 3 形式（§5.2.1 / §5.3.1 / §5.4.1。`docs/history.md` A-11）
    assert!(C.allows_section_break());
    assert!(B.allows_section_break());
    assert!(A.allows_section_break());
    assert!(!MD.allows_section_break());
}

// ============================================================ §5.0.3 エスケープ表

#[test]
fn every_valid_escape_is_recognized_and_removed() {
    for (written, target, plain) in ESCAPES {
        let found = lexer::find_escapes(written);
        assert_eq!(found.len(), 1, "{written}");
        assert_eq!(found[0].start, 0, "{written}");
        assert_eq!(found[0].len, written.len(), "{written}");
        assert_eq!(found[0].target, Some(*target), "{written}");
        assert!(found[0].valid, "{written}");
        let unescaped = lexer::unescape(written);
        assert_eq!(unescaped.as_ref(), *plain, "{written}");
    }
}

#[test]
fn an_invalid_escape_stays_two_chars_without_a_diagnostic() {
    // §5.0.3「それ以外の `\x` は `\x` の 2 文字としてそのまま扱う（診断なし）」
    for written in ["\\n", "\\x", "\\ ", "\\あ", "\\*"] {
        let found = lexer::find_escapes(written);
        assert_eq!(found.len(), 1, "{written}");
        assert!(!found[0].valid, "{written}");
        assert!(found[0].target.is_some(), "{written}");
        assert_eq!(found[0].len, written.len(), "{written}");
        let unescaped = lexer::unescape(written);
        assert_eq!(unescaped.as_ref(), written, "{written}");
    }
}

#[test]
fn a_lone_backslash_at_the_end_of_a_line_is_not_an_escape() {
    let found = lexer::find_escapes("本文\\");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].target, None);
    assert_eq!(found[0].len, 1);
    assert!(!found[0].valid);
    let unescaped = lexer::unescape("本文\\");
    assert_eq!(unescaped.as_ref(), "本文\\");
}

#[test]
fn an_escaped_backslash_consumes_both_chars() {
    // `\\@` は「エスケープされた `\`」+「リテラルの `@`」で、構造見出しにはならない
    let found = lexer::find_escapes("\\\\@");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].target, Some('\\'));
    assert!(found[0].valid);
    let unescaped = lexer::unescape("\\\\@");
    assert_eq!(unescaped.as_ref(), "\\@");
    let got = tag("\\\\@01-04", C);
    assert_eq!(got, "text[\\\\@01-04]");
}

#[test]
fn escapes_are_found_at_any_position() {
    let found = lexer::find_escapes("a\\@b\\;c");
    let starts: Vec<usize> = found.iter().map(|e| e.start).collect();
    assert_eq!(starts, [1, 4]);
    assert!(found.iter().all(|e| e.valid));
    assert_eq!(lexer::ESCAPE_TARGETS.len(), 10);
}

#[test]
fn unescape_borrows_when_there_is_nothing_to_remove() {
    assert!(matches!(lexer::unescape("本文"), Cow::Borrowed(_)));
    // 無効なエスケープは解除しないので借用のまま
    assert!(matches!(lexer::unescape("\\n"), Cow::Borrowed(_)));
    let unescaped = lexer::unescape("a\\@b");
    assert_eq!(unescaped.as_ref(), "a@b");
}

#[test]
fn every_escape_target_is_listed_in_the_spec() {
    // §5.0.3, including literal markers used by migration.
    let want = ['@', '\'', ';', '/', '「', '」', '\\', '#', '!', '^'];
    assert_eq!(lexer::ESCAPE_TARGETS, want.as_slice());
}

// ============================================================ §5.0.4 改行・符号化・正規化

#[test]
fn bom_is_stripped_on_load_and_never_written() {
    let text = "\u{feff}あ\nい";
    assert_eq!(lexer::strip_bom(text), "あ\nい");
    assert_eq!(lexer::BOM, "\u{feff}");
    let loaded = lexer::normalize_on_load(text);
    assert_eq!(loaded.as_ref(), "あ\nい");
    let saved = lexer::normalize_for_save(text, Newline::Lf);
    assert_eq!(saved, "あ\nい\n");
}

#[test]
fn crlf_and_cr_become_lf_on_load() {
    let crlf = lexer::normalize_on_load("あ\r\nい\r\n");
    assert_eq!(crlf.as_ref(), "あ\nい\n");
    let cr = lexer::normalize_on_load("あ\rい\r");
    assert_eq!(cr.as_ref(), "あ\nい\n");
    let mixed = lexer::normalize_on_load("あ\r\nい\rう\n");
    assert_eq!(mixed.as_ref(), "あ\nい\nう\n");
    // 変換が要らないときは確保せずに借用を返す
    assert!(matches!(lexer::normalize_on_load("あ\n"), Cow::Borrowed(_)));
}

#[test]
fn trailing_spaces_are_removed_and_indent_is_kept_on_save() {
    let text = "summary: 一行  \n  二行目\t\n　全角　\n";
    let saved = lexer::normalize_for_save(text, Newline::Lf);
    assert_eq!(saved, "summary: 一行\n  二行目\n　全角\n");
}

#[test]
fn the_file_ends_with_exactly_one_newline() {
    // §5.0.4「ファイル末尾は必ず改行 1 個で終える」（MUST）
    for text in ["あ", "あ\n", "あ\n\n\n", "あ\r\n", "あ "] {
        let saved = lexer::normalize_for_save(text, Newline::Lf);
        assert_eq!(saved, "あ\n", "input: {text:?}");
    }
    // 途中の空行は残る
    let middle = lexer::normalize_for_save("あ\n\nい", Newline::Lf);
    assert_eq!(middle, "あ\n\nい\n");
}

#[test]
fn an_empty_document_stays_empty() {
    // 改行 1 個を強制すると「空の 1 行」が増えて往復で文書が変わってしまう
    assert_eq!(lexer::normalize_for_save("", Newline::Lf), "");
    assert_eq!(lexer::normalize_for_save("\n", Newline::Lf), "");
    assert_eq!(lexer::normalize_for_save("\u{feff}", Newline::Lf), "");
}

#[test]
fn the_newline_setting_decides_the_output_form() {
    // §4.5 の `project.toml.newline`
    let crlf = lexer::normalize_for_save("あ\nい", Newline::Crlf);
    assert_eq!(crlf, "あ\r\nい\r\n");
    let cr = lexer::normalize_for_save("あ\nい", Newline::Cr);
    assert_eq!(cr, "あ\rい\r");
    // 読み込み直すと LF に戻る（往復で壊れない）
    let back = lexer::normalize_on_load(&crlf);
    assert_eq!(back.as_ref(), "あ\nい\n");
}

#[test]
fn newline_is_serialized_the_way_project_toml_writes_it() {
    assert_eq!(Newline::default(), Newline::Lf);
    let json = serde_json::to_string(&Newline::Crlf).unwrap();
    assert_eq!(json, "\"crlf\"");
    let back: Newline = serde_json::from_str("\"cr\"").unwrap();
    assert_eq!(back, Newline::Cr);
    assert_eq!(Newline::Lf.as_str(), "\n");
}

#[test]
fn no_unicode_normalization_happens() {
    // §5.0.4「Unicode 正規化はしない」。NFKC 相当の照合は §6.1.3（M3）だけ
    let saved = lexer::normalize_for_save("ｶﾞ が", Newline::Lf);
    assert_eq!(saved, "ｶﾞ が\n");
    assert_eq!(saved.chars().count(), 5);
}

// ============================================================ §5.0.1 行指向

#[test]
fn every_line_is_lexed_on_its_own() {
    // §5.0.1「複数行にまたがるトークンは存在しない。1 行は常に単独で字句解析できる」
    let doc = "@01-04 見出し\nsummary: 一行\n  継続\n本文'恵\n// メモ\n;";
    let whole: Vec<String> = doc.split('\n').map(|l| tag(l, A)).collect();
    let alone: Vec<String> = doc.lines().map(|l| tag(l, A)).collect();
    assert_eq!(whole, alone);
    assert_eq!(whole.len(), 6);
    assert_eq!(whole[0], "structure[01-04 見出し]");
    assert_eq!(whole[5], "section-break");
}
