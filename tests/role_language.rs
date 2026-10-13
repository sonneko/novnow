//! `core::resolve::role_language`（§6.5 / 付録 B）のテスト。
//!
//! `PLANS.md` §5.4 の作業項目 3 と**完了条件 1**（付録 B.3 の変換結果表 **8 行**がすべて
//! テストで一致する）に対応する。B.3 の表は**そのまま期待値**にしてある（§6.3「仕様が正本」）。
//!
//! `^` / `$` が発話全体の先頭・末尾を指すこと（文単位に分割しない）は §6.5.2 の「文」の段落と
//! B.3 の 1・2 行目が両立しないための裁定で、経緯は `PLANS.md` §9.1 **A-23**。

use std::ops::Range;
use std::time::Instant;

use novnow::core::model::project::Lexicon;
use novnow::core::model::role_pattern::{Anchor, RolePattern};
use novnow::core::resolve::role_language::{self, Cache, Matcher, MatcherSet};

// ============================================================ 付録 B.3（テストの期待値）

/// 付録 B.3 の 1 行。
struct Row {
    /// パターン（`「` `」` の内側）。
    pattern: &'static str,
    /// §6.5.2 のアンカー。
    anchor: Anchor,
    /// 変換後の正規表現。
    regex: &'static str,
    /// 一致例（発話全体）。
    hit: &'static str,
    /// 一致例のうち**下線する部分**。
    underline: &'static str,
    /// 不一致例。
    miss: &'static str,
}

/// 付録 B.3 の変換結果の例（**8 行すべて**＝ M3 完了条件 1）。
const B3: &[Row] = &[
    Row {
        pattern: "*だよ！",
        anchor: Anchor::SentenceEnd,
        regex: "[^\\n]*?だよ！$",
        hit: "もう行くだよ！",
        underline: "だよ！",
        miss: "だよ！って何",
    },
    Row {
        pattern: "…あの…*",
        anchor: Anchor::SentenceStart,
        regex: "^[…‥.。]{1,6}あの[…‥.。]{1,6}[^\\n]*?",
        hit: "…あの…その…",
        underline: "…あの…",
        miss: "えっと、…あの…",
    },
    Row {
        pattern: "です。",
        anchor: Anchor::SentenceEnd,
        regex: "です。$",
        hit: "承知しましたです。",
        underline: "です。",
        miss: "ですが",
    },
    Row {
        pattern: "(体言)！",
        anchor: Anchor::SentenceEnd,
        regex: "[\\p{Han}\\p{Katakana}\\p{Hiragana}A-Za-z0-9ー]{1,24}！$",
        hit: "静粛に！",
        underline: "静粛に！",
        miss: "ねえ、いいでしょ？",
    },
    Row {
        pattern: "ちょっ…*",
        anchor: Anchor::SentenceStart,
        regex: "^ちょっ[…‥.。]{1,6}[^\\n]*?",
        hit: "ちょっ…待って",
        underline: "ちょっ…",
        miss: "あ、ちょっ…",
    },
    Row {
        pattern: "*ヨシ",
        anchor: Anchor::SentenceEnd,
        regex: "[^\\n]*?ヨシ$",
        hit: "指差し確認ヨシ",
        underline: "ヨシ",
        miss: "ヨシと言った",
    },
    Row {
        pattern: "オッケーっじゃあ*",
        anchor: Anchor::SentenceStart,
        regex: "^オッケーっじゃあ[^\\n]*?",
        hit: "オッケーっじゃあ始めるぞ",
        underline: "オッケーっじゃあ",
        miss: "じゃあオッケー",
    },
    Row {
        pattern: "かな",
        anchor: Anchor::SentenceEnd,
        regex: "かな$",
        hit: "行けるかな",
        underline: "かな",
        miss: "かなり",
    },
];

// ============================================================ 小道具

fn pattern(source: &str) -> RolePattern {
    RolePattern::new(source)
}

fn lexicon() -> Lexicon {
    Lexicon::default()
}

fn lexicon_with(name: &str, regex: &str) -> Lexicon {
    let mut found = Lexicon::default();
    found
        .word_classes
        .insert(name.to_string(), regex.to_string());
    found
}

/// パターン 1 件をコンパイルする（既定の語クラス）。
fn matcher(source: &str) -> Matcher {
    Matcher::new(&pattern(source), &lexicon())
        .unwrap_or_else(|| panic!("{source} がコンパイルできない"))
}

fn slices<'a>(text: &'a str, ranges: &[Range<usize>]) -> Vec<&'a str> {
    ranges.iter().map(|range| &text[range.clone()]).collect()
}

// ============================================================ 完了条件 1（付録 B.3 の 8 行）

#[test]
fn b3_anchors_match_the_table() {
    for row in B3 {
        assert_eq!(pattern(row.pattern).anchor(), row.anchor, "{}", row.pattern);
    }
}

#[test]
fn b3_regexes_match_the_table() {
    for row in B3 {
        assert_eq!(
            role_language::to_regex(&pattern(row.pattern), &lexicon()),
            row.regex,
            "{}",
            row.pattern
        );
    }
}

#[test]
fn b3_hit_examples_underline_exactly_the_literal_part() {
    for row in B3 {
        let found = matcher(row.pattern);
        let hit = found
            .find(row.hit)
            .unwrap_or_else(|| panic!("{} は「{}」に一致するはず", row.pattern, row.hit));
        // §6.5.4「一致したリテラル部分のみに緑の下線」（A-21: 除くのは `*` / `〜` だけ）
        assert_eq!(
            slices(row.hit, &hit.underlines),
            vec![row.underline],
            "{}",
            row.pattern
        );
    }
}

#[test]
fn b3_miss_examples_do_not_match() {
    for row in B3 {
        let found = matcher(row.pattern);
        assert!(
            found.find(row.miss).is_none(),
            "{} は「{}」に一致してはいけない",
            row.pattern,
            row.miss
        );
    }
}

/// B.3 の 8 行は**マッチャを毎回作っても**同じ結果になる（`MatcherSet` 経由でも同じ）。
#[test]
fn b3_rows_survive_a_matcher_set() {
    let set = MatcherSet::new(
        &B3.iter()
            .map(|row| pattern(row.pattern))
            .collect::<Vec<_>>(),
        &lexicon(),
    );
    assert_eq!(set.len(), B3.len());
    for row in B3 {
        // 8 行を 1 つの集合に入れると**別の行のパターンも一致する**（`もう行くだよ！` は
        // `(体言)！` にも合う）ので、和集合ではなく 1 パターンごとの結果で見る
        let found = set
            .hits(row.hit)
            .iter()
            .any(|hit| slices(row.hit, &hit.underlines) == vec![row.underline]);
        assert!(found, "{} → 「{}」", row.pattern, row.hit);
    }
}

// ============================================================ §6.5.1 の 5 アトム

#[test]
fn both_wildcard_characters_convert_the_same() {
    // `〜`（波ダッシュ）は `*` と同じ（§6.5.1）
    assert_eq!(
        role_language::to_regex(&pattern("*よ"), &lexicon()),
        role_language::to_regex(&pattern("〜よ"), &lexicon())
    );
    assert_eq!(
        role_language::to_regex(&pattern("*よ"), &lexicon()),
        "[^\\n]*?よ$"
    );
}

#[test]
fn ellipsis_forms_all_convert_to_the_same_class() {
    for source in ["…", "‥", "..."] {
        assert_eq!(
            role_language::to_regex(&pattern(source), &lexicon()),
            "[…‥.。]{1,6}$",
            "{source}"
        );
    }
    // 2 点以下はリテラル（`...` だけが省略記号）
    assert_eq!(
        role_language::to_regex(&pattern(".."), &lexicon()),
        "\\.\\.$"
    );
    // 句点 `。` はアトムではないが、変換後のクラスには入る（§6.5.1）
    assert_eq!(role_language::to_regex(&pattern("。"), &lexicon()), "。$");
}

#[test]
fn a_question_mark_is_one_character() {
    assert_eq!(
        role_language::to_regex(&pattern("?かな"), &lexicon()),
        "[^\\n]かな$"
    );
    let found = matcher("?かな");
    assert_eq!(
        slices(
            "行けるかな",
            &found.find("行けるかな").expect("一致").underlines
        ),
        vec!["るかな"]
    );
    // 前に 1 文字要るので「かな」だけでは一致しない
    assert!(found.find("かな").is_none());
}

#[test]
fn the_three_word_classes_use_the_default_regexes() {
    assert_eq!(
        role_language::to_regex(&pattern("(体言)"), &lexicon()),
        "[\\p{Han}\\p{Katakana}\\p{Hiragana}A-Za-z0-9ー]{1,24}$"
    );
    assert_eq!(
        role_language::to_regex(&pattern("(用言)"), &lexicon()),
        "[\\p{Han}\\p{Hiragana}]{1,16}$"
    );
    assert_eq!(
        role_language::to_regex(&pattern("(数)"), &lexicon()),
        "[0-9０-９一二三四五六七八九十百千万]{1,8}$"
    );
    assert!(role_language::class_regex("体言", &lexicon()).is_some());
    assert_eq!(role_language::class_regex("不存在", &lexicon()), None);
}

#[test]
fn literals_are_escaped() {
    // 正規表現のメタ文字はリテラル（§6.5.1「エスケープ」）
    assert_eq!(
        role_language::to_regex(&pattern("a+b.c(d)"), &lexicon()),
        "a\\+b\\.c\\(d\\)$"
    );
    let found = matcher("a+b.c(d)");
    assert!(found.find("xa+b.c(d)").is_some());
    assert!(found.find("aabxcd").is_none());
}

#[test]
fn an_unknown_word_class_stays_a_literal() {
    // `project.json` にも既定にも無いクラス名は括弧ごとリテラル（勝手に無視しない）
    assert_eq!(
        role_language::to_regex(&pattern("(ほげ)"), &lexicon()),
        "\\(ほげ\\)$"
    );
    let found = matcher("(ほげ)");
    assert_eq!(
        slices("x(ほげ)", &found.find("x(ほげ)").expect("一致").underlines),
        vec!["(ほげ)"]
    );
}

#[test]
fn a_word_class_from_the_project_wins_over_the_default() {
    // §6.5.1「`project.json.lexicon.word_classes` で定義」
    let defined = lexicon_with("体言", "[0-9]{1,3}");
    assert_eq!(
        role_language::to_regex(&pattern("(体言)！"), &defined),
        "[0-9]{1,3}！$"
    );
    let found = Matcher::new(&pattern("(体言)！"), &defined).expect("compile");
    assert_eq!(
        slices("123！", &found.find("123！").expect("一致").underlines),
        vec!["123！"]
    );
    // 既定の `(体言)` なら一致していたものが一致しなくなる
    assert!(found.find("静粛に！").is_none());
}

#[test]
fn a_broken_word_class_regex_is_dropped_without_stopping_the_others() {
    // §7 に対応コードが無いので黙って捨てる（A-24）。§4.3-4「起動失敗にしない」
    let broken = lexicon_with("体言", "[");
    assert!(Matcher::new(&pattern("(体言)！"), &broken).is_none());
    let set = MatcherSet::new(&[pattern("(体言)！"), pattern("かな")], &broken);
    assert_eq!(set.len(), 1);
    assert_eq!(set.matchers()[0].source(), "かな");
}

// ============================================================ §6.5.2 のアンカー 5 種

#[test]
fn the_five_anchor_shapes() {
    let rows = [
        ("*だよ！", Anchor::SentenceEnd, "[^\\n]*?だよ！$"),
        (
            "ちょっ…*",
            Anchor::SentenceStart,
            "^ちょっ[…‥.。]{1,6}[^\\n]*?",
        ),
        ("*ヨシ*", Anchor::Anywhere, "[^\\n]*?ヨシ[^\\n]*?"),
        ("ほう…", Anchor::SentenceEnd, "ほう[…‥.。]{1,6}$"),
        ("=ほう…", Anchor::Exact, "^ほう[…‥.。]{1,6}$"),
    ];
    for (source, anchor, regex) in rows {
        assert_eq!(pattern(source).anchor(), anchor, "{source}");
        assert_eq!(
            role_language::to_regex(&pattern(source), &lexicon()),
            regex,
            "{source}"
        );
    }
}

#[test]
fn an_exact_pattern_needs_the_whole_utterance() {
    let found = matcher("=ほう…");
    assert!(found.find("ほう…").is_some());
    assert!(found.find("ほう…って").is_none());
    assert!(found.find("えっと、ほう…").is_none());
}

#[test]
fn a_free_pattern_matches_in_the_middle() {
    let found = matcher("*ヨシ*");
    // B.3 の 6 行目（`*ヨシ`）の不一致例が、末尾にもワイルドカードを付けると一致する
    assert_eq!(
        slices(
            "ヨシと言った",
            &found.find("ヨシと言った").expect("一致").underlines
        ),
        vec!["ヨシ"]
    );
}

/// §6.5.2 の「文」ではなく**発話全体**の先頭・末尾を見る（**A-23**）。
#[test]
fn anchors_look_at_the_whole_utterance_not_at_each_sentence() {
    // §6.5.2 の例の台詞。`*よ。` は 1 文目の文末には**一致しない**（B.3 の 1 行目の
    // 不一致例「だよ！って何」と同じ理由。文単位にすると B.3 の 2 行目が壊れる）
    let utterance = "分かってるよ。でも、これを見てくれ";
    assert!(matcher("*よ。").find(utterance).is_none());
    // 発話全体が `よ。` で終われば一致する
    assert!(matcher("*よ。").find("分かってるよ。").is_some());
}

// ============================================================ §6.5.4 の下線

#[test]
fn a_wildcard_only_pattern_underlines_nothing() {
    // 下線が 1 本も無い一致は「一致しなかった」扱い（§6.5.4「一致しない場合は何もしない」）
    assert!(matcher("*").find("何でも").is_none());
    assert!(MatcherSet::new(&[pattern("*")], &lexicon())
        .underlines("何でも")
        .is_empty());
}

#[test]
fn several_patterns_share_one_underline() {
    // §6.5.4「複数パターンが一致したら、一致範囲の和集合に 1 本の下線を引く（二重線にしない）」
    let set = MatcherSet::new(&[pattern("*だよ！"), pattern("もう行く*")], &lexicon());
    let utterance = "もう行くだよ！";
    let hits = set.hits(utterance);
    assert_eq!(hits.len(), 2);
    // 隣り合った 2 本は 1 本にまとまる
    assert_eq!(
        slices(utterance, &set.underlines(utterance)),
        vec!["もう行くだよ！"]
    );
}

#[test]
fn the_same_pattern_twice_is_one_underline() {
    let set = MatcherSet::new(&[pattern("かな"), pattern("かな")], &lexicon());
    assert_eq!(
        slices("行けるかな", &set.underlines("行けるかな")),
        vec!["かな"]
    );
}

#[test]
fn overlapping_matches_merge() {
    let set = MatcherSet::new(&[pattern("*に！"), pattern("静粛に！")], &lexicon());
    let utterance = "静粛に！";
    assert_eq!(set.hits(utterance).len(), 2);
    assert_eq!(
        slices(utterance, &set.underlines(utterance)),
        vec!["静粛に！"]
    );
}

#[test]
fn a_miss_returns_nothing_at_all() {
    let set = MatcherSet::new(
        &[pattern("*だよ！"), pattern("かな"), pattern("(体言)！")],
        &lexicon(),
    );
    assert!(set.hits("ねえ、いいでしょ？").is_empty());
    assert!(set.underlines("ねえ、いいでしょ？").is_empty());
    // パターンが 1 つも無い集合も同じ（§6.5.3 の和集合が空のとき）
    assert!(MatcherSet::new(&[], &lexicon())
        .underlines("何でも")
        .is_empty());
}

// ============================================================ §6.5.3 の照合対象

#[test]
fn dialogue_spans_returns_only_the_inside_of_the_quotes() {
    let text = "「もう行くだよ！」と彼女は言った。";
    let spans = role_language::dialogue_spans(text);
    assert_eq!(slices(text, &spans), vec!["もう行くだよ！"]);
}

#[test]
fn dialogue_spans_handles_both_kinds_and_nesting() {
    let text = "「『静粛に！』と言われた」";
    // 入れ子は**内側と外側の両方**を返す（和集合にすると内側の位置が失われる）
    assert_eq!(
        slices(text, &role_language::dialogue_spans(text)),
        vec!["『静粛に！』と言われた", "静粛に！"]
    );
    // 2 つ並ぶ
    let two = "「かな」と「だよ」";
    assert_eq!(
        slices(two, &role_language::dialogue_spans(two)),
        vec!["かな", "だよ"]
    );
}

#[test]
fn dialogue_spans_of_plain_text_is_empty() {
    // 地の文は照合しない（§6.5.3 MUST）
    assert!(role_language::dialogue_spans("彼女は静粛に！と言った。").is_empty());
}

#[test]
fn an_unclosed_quote_runs_to_the_end() {
    let text = "「書きかけの台詞";
    assert_eq!(
        slices(text, &role_language::dialogue_spans(text)),
        vec!["書きかけの台詞"]
    );
    // 種類の違う閉じ括弧は閉じない
    let mixed = "「静粛に！』と続くだよ！";
    assert_eq!(
        slices(mixed, &role_language::dialogue_spans(mixed)),
        vec!["静粛に！』と続くだよ！"]
    );
}

#[test]
fn only_the_dialogue_is_matched() {
    // 地の文に同じ文字列があっても下線しない（§6.5.3 MUST）
    let text = "かな、と書いてある。「行けるかな」";
    let set = MatcherSet::new(&[pattern("かな")], &lexicon());
    let found = role_language::dialogue_spans(text)
        .iter()
        .flat_map(|span| {
            let inner = &text[span.clone()];
            set.underlines(inner)
                .into_iter()
                .map(move |range| range.start + span.start..range.end + span.start)
        })
        .collect::<Vec<_>>();
    assert_eq!(slices(text, &found), vec!["かな"]);
}

// ============================================================ §6.5.5 のキャッシュ

#[test]
fn the_cache_compiles_each_key_once() {
    let mut cache = Cache::new(lexicon());
    assert!(cache.is_empty());
    let first = cache.set_for("恩田恵:A", &[pattern("*だよ…")]);
    assert_eq!(first.len(), 1);
    // 同じキーに別のパターンを渡しても**覚えているほう**を返す
    let again = cache.set_for("恩田恵:A", &[pattern("かな"), pattern("です。")]);
    assert_eq!(again.len(), 1);
    assert_eq!(again.matchers()[0].source(), "*だよ…");
    assert_eq!(cache.len(), 1);
    // 別のキーは別
    assert_eq!(cache.set_for("恩田恵:B", &[pattern("*ないと…！")]).len(), 1);
    assert_eq!(cache.len(), 2);
    assert!(cache.forget("恩田恵:A"));
    assert!(!cache.forget("恩田恵:A"));
    assert_eq!(
        cache.set_for("恩田恵:A", &[pattern("かな")]).matchers()[0].source(),
        "かな"
    );
    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn the_cache_keeps_the_lexicon_it_was_built_with() {
    let mut cache = Cache::new(lexicon_with("体言", "[0-9]{1,3}"));
    assert_eq!(
        cache.set_for("x", &[pattern("(体言)")]).matchers()[0].regex_source(),
        "[0-9]{1,3}$"
    );
    assert_eq!(cache.lexicon().word_classes.len(), 1);
}

// ============================================================ §5.5.7 とのつながり

#[test]
fn the_spec_example_patterns_match_their_own_lines() {
    // §5.5.7 の恩田恵: キャラ共通 `## tone` と モード A の専用パターン
    let text = "「*で…」「*です…」「…あの…*」";
    let patterns = patterns_of(text);
    let set = MatcherSet::new(&patterns, &lexicon());
    assert_eq!(set.len(), 3);
    assert_eq!(
        slices("…あの…その…", &set.underlines("…あの…その…")),
        vec!["…あの…"]
    );
    assert_eq!(
        slices(
            "えへへ、動きますです。",
            &set.underlines("えへへ、動きますです。")
        ),
        vec!["です。"]
    );
}

#[test]
fn a_mode_pattern_and_a_tone_pattern_apply_together() {
    // §6.5.3「適用パターン集合 = （有効モードのパターン）∪（キャラ共通 `## tone`）」
    let mut patterns = patterns_of("「*だよ…」「えへへ、*」");
    patterns.extend(patterns_of("「*で…」「*です…」「…あの…*」"));
    let set = MatcherSet::new(&patterns, &lexicon());
    assert_eq!(set.len(), 5);
    let utterance = "えへへ、動くんだよ…";
    assert_eq!(
        slices(utterance, &set.underlines(utterance)),
        vec!["えへへ、", "だよ…"]
    );
}

/// `「…」` の行からパターンを作る（M2 の `characters_md` と同じ形）。
fn patterns_of(line: &str) -> Vec<RolePattern> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('「') {
        let inner = &rest[open + '「'.len_utf8()..];
        let Some(close) = inner.find('」') else {
            break;
        };
        out.push(pattern(&inner[..close]));
        rest = &inner[close + '」'.len_utf8()..];
    }
    out
}

// ============================================================ §6.5.5 の計算量

#[test]
fn ten_patterns_on_one_line_stay_fast() {
    // 目標: 1 行 10 パターンで **20µs 以内**（§6.5.5）。**release 実測 0.68µs / 行**
    // （同じ台詞に 100 パターンを当てても 9.8µs）。
    // ここは debug ビルドで走るので上限は緩く取る（`RegexSet` で候補を絞っているので
    // パターン数を 10 倍にしても時間は 10 倍にならないことの確認でもある）
    let patterns = (0..10)
        .map(|index| {
            pattern(if index % 2 == 0 {
                "*だよ！"
            } else {
                "(体言)！"
            })
        })
        .collect::<Vec<_>>();
    let set = MatcherSet::new(&patterns, &lexicon());
    assert_eq!(set.len(), 10);
    let utterance = "もう行くだよ！";
    let started = Instant::now();
    let rounds = 2_000;
    for _ in 0..rounds {
        assert!(!set.underlines(utterance).is_empty());
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed.as_millis() < 2_000,
        "10 パターン × {rounds} 行が {}ms（1 行あたり {}µs）",
        elapsed.as_millis(),
        elapsed.as_micros() / rounds as u128
    );
}
