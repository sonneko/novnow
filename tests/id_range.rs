//! `core::id`（SceneId / PartialSceneId / RangeExpr）のテスト。
//!
//! `PLANS.md` §5.2 完了条件「`tests/id_range.rs`: 仕様 §5.1 の全例（`@01 .. @02` の展開、
//! E016 等）が通る」に対応する。例は `SPECIFICATIONS.md` §5.1 / §5.1.2 からそのまま取っている。

use novnow::core::id::MAX_LEVEL;
use novnow::core::id::{Digits, IdError, PartialSceneId, RangeError, RangeExpr, SceneId};

/// テスト用の [`SceneId`] 構築。
fn id(chapter: u16, episode: u16, scene: u16, section: u16) -> SceneId {
    SceneId {
        chapter,
        episode,
        scene,
        section,
    }
}

/// 範囲式をパースする（失敗したらパニック）。
fn range(text: &str) -> RangeExpr {
    RangeExpr::parse(text).unwrap()
}

/// 範囲式の唯一の項の区間。
fn interval(text: &str) -> (SceneId, SceneId) {
    let parsed = range(text);
    let interval = parsed.intervals()[0];
    (interval.start, interval.end)
}

// ---------------------------------------------------------------- SceneId

#[test]
fn scene_id_parses_three_and_four_levels() {
    // §5.1「`@01-04-02-03` は節、`@01-04-02` は場面全体（`section = 0`）」
    let section = SceneId::parse("01-04-02-03").unwrap();
    assert_eq!(section, id(1, 4, 2, 3));

    let whole_scene = SceneId::parse("01-04-02").unwrap();
    assert_eq!(whole_scene, id(1, 4, 2, 0));
    assert!(whole_scene.is_whole_scene());
    assert!(!section.is_whole_scene());
}

#[test]
fn scene_id_accepts_one_to_four_digits_per_level() {
    // §5.1 の `num = /\d{1,4}/`
    let short = SceneId::parse("1-2-3").unwrap();
    assert_eq!(short, id(1, 2, 3, 0));

    let long = SceneId::parse("1234-0-9999-1").unwrap();
    assert_eq!(long, id(1234, 0, 9999, 1));
}

#[test]
fn scene_id_rejects_malformed_input() {
    assert_eq!(SceneId::parse(""), Err(IdError::Empty));
    assert_eq!(SceneId::parse("01-04"), Err(IdError::TooFewLevels));
    assert_eq!(SceneId::parse("01"), Err(IdError::TooFewLevels));
    assert_eq!(
        SceneId::parse("01-04-02-03-01"),
        Err(IdError::TooManyLevels)
    );
    assert_eq!(SceneId::parse("01-04-0x"), Err(IdError::NotANumber));
    assert_eq!(SceneId::parse("01-04- 2"), Err(IdError::NotANumber));
    assert_eq!(SceneId::parse("12345-01-02"), Err(IdError::TooManyDigits));
    assert_eq!(SceneId::parse("01--02"), Err(IdError::EmptyLevel));
    assert_eq!(SceneId::parse("01-04-"), Err(IdError::EmptyLevel));
    // シジル付きは `parse` では通さない（`parse_with_sigil` を使う）
    assert_eq!(SceneId::parse("@01-04-02"), Err(IdError::NotANumber));
}

#[test]
fn scene_id_parse_with_sigil() {
    let with_sigil = SceneId::parse_with_sigil("@01-04-02-03").unwrap();
    assert_eq!(with_sigil, id(1, 4, 2, 3));
    assert_eq!(
        SceneId::parse_with_sigil("01-04-02"),
        Err(IdError::MissingSigil)
    );
}

#[test]
fn scene_id_display_uses_digits_and_hides_zero_section() {
    // §5.1「`section == 0` のときは 3 階層表示、それ以外は 4 階層。ゼロ詰め」
    let digits = Digits::default();
    let default_digits = Digits {
        chapter: 2,
        episode: 2,
        scene: 2,
        section: 2,
    };
    assert_eq!(digits, default_digits);

    assert_eq!(id(1, 4, 2, 3).display(digits), "01-04-02-03");
    assert_eq!(id(1, 4, 2, 0).display(digits), "01-04-02");

    let wide = Digits {
        chapter: 3,
        episode: 2,
        scene: 2,
        section: 4,
    };
    assert_eq!(id(1, 4, 2, 3).display(wide), "001-04-02-0003");

    // 桁数は §5.1 の `num` の範囲（1〜4）にクランプする
    let narrow = Digits {
        chapter: 1,
        episode: 1,
        scene: 1,
        section: 1,
    };
    assert_eq!(id(12, 4, 2, 3).display(narrow), "12-4-2-3");
    let zero = Digits {
        chapter: 0,
        episode: 0,
        scene: 0,
        section: 0,
    };
    assert_eq!(id(1, 4, 2, 3).display(zero), "1-4-2-3");
    let huge = Digits {
        chapter: 9,
        episode: 9,
        scene: 9,
        section: 9,
    };
    assert_eq!(id(1, 4, 2, 3).display(huge), "0001-0004-0002-0003");
}

#[test]
fn scene_id_order_is_lexicographic() {
    // §5.1「比較は `(chapter, episode, scene, section)` の辞書式順序」
    assert!(id(1, 4, 2, 3) < id(1, 4, 3, 0));
    assert!(id(1, 4, 2, 3) < id(1, 4, 2, 4));
    assert!(id(2, 0, 0, 0) > id(1, MAX_LEVEL, MAX_LEVEL, MAX_LEVEL));

    let mut ids = vec![id(2, 1, 0, 0), id(1, 9, 9, 9), id(1, 2, 0, 0)];
    ids.sort();
    assert_eq!(ids, vec![id(1, 2, 0, 0), id(1, 9, 9, 9), id(2, 1, 0, 0)]);
}

#[test]
fn scene_id_new_rejects_values_over_four_digits() {
    assert_eq!(SceneId::new(1, 2, 3, 4), Some(id(1, 2, 3, 4)));
    assert_eq!(
        SceneId::new(MAX_LEVEL, MAX_LEVEL, MAX_LEVEL, MAX_LEVEL),
        Some(SceneId::MAX)
    );
    assert_eq!(SceneId::new(MAX_LEVEL + 1, 0, 0, 0), None);
    assert_eq!(SceneId::new(0, 0, 0, MAX_LEVEL + 1), None);
}

#[test]
fn scene_id_extremes() {
    assert_eq!(SceneId::MIN, id(0, 0, 0, 0));
    assert_eq!(SceneId::MAX, id(9999, 9999, 9999, 9999));
    assert!(SceneId::MIN <= SceneId::MAX);
}

// ------------------------------------------------------- PartialSceneId

#[test]
fn partial_id_keeps_depth() {
    let one = PartialSceneId::parse("01").unwrap();
    assert_eq!(one.depth(), 1);
    assert_eq!(one.chapter(), 1);
    assert_eq!(one.episode(), None);

    let two = PartialSceneId::parse_with_sigil("@01-04").unwrap();
    assert_eq!(two.depth(), 2);
    assert_eq!(two.episode(), Some(4));
    assert_eq!(two.scene(), None);

    let four = PartialSceneId::parse("01-04-02-03").unwrap();
    assert_eq!(four.depth(), 4);
    assert_eq!(four.section(), Some(3));

    assert_eq!(PartialSceneId::parse(""), Err(IdError::Empty));
    assert_eq!(
        PartialSceneId::parse("01-04-02-03-05"),
        Err(IdError::TooManyLevels)
    );
}

#[test]
fn partial_id_expands_low_and_high() {
    // §5.1「左辺の省略階層は最小値（0）、右辺の省略階層は最大値（9999）に展開」
    let one = PartialSceneId::parse("01").unwrap();
    assert_eq!(one.expand_low(), id(1, 0, 0, 0));
    assert_eq!(one.expand_high(), id(1, 9999, 9999, 9999));

    let two = PartialSceneId::parse("01-04").unwrap();
    assert_eq!(two.expand_low(), id(1, 4, 0, 0));
    assert_eq!(two.expand_high(), id(1, 4, 9999, 9999));

    // 4 階層を書き切ったときはどちらも同じ
    let four = PartialSceneId::parse("01-04-02-03").unwrap();
    assert_eq!(four.expand_low(), four.expand_high());
    assert_eq!(four.expand_low(), id(1, 4, 2, 3));
}

#[test]
fn partial_id_display_shows_only_written_levels() {
    let digits = Digits::default();
    assert_eq!(
        PartialSceneId::parse("1-4").unwrap().display(digits),
        "01-04"
    );
    assert_eq!(
        PartialSceneId::parse("1-4-2-3").unwrap().display(digits),
        "01-04-02-03"
    );
    let wide = Digits {
        chapter: 3,
        episode: 1,
        scene: 2,
        section: 2,
    };
    assert_eq!(PartialSceneId::parse("1-4").unwrap().display(wide), "001-4");
}

#[test]
fn scene_id_converts_to_partial() {
    let scene = id(1, 4, 2, 3);
    assert_eq!(scene.partial().expand_low(), scene);
    assert_eq!(scene.partial().depth(), 4);

    let whole = scene.scene_partial();
    assert_eq!(whole.depth(), 3);
    assert_eq!(whole.expand_low(), id(1, 4, 2, 0));
    assert_eq!(whole.expand_high(), id(1, 4, 2, 9999));
}

// ------------------------------------------------------------- RangeExpr

#[test]
fn range_expands_omitted_levels() {
    // §5.1 の例: `@01 .. @02` ⇒ `01-00-00-00 .. 02-9999-9999-9999`
    let (start, end) = interval("@01 .. @02");
    assert_eq!(start, id(1, 0, 0, 0));
    assert_eq!(end, id(2, 9999, 9999, 9999));
}

#[test]
fn range_is_closed_interval() {
    // §5.1「`@01-04 .. @02-01` は**閉区間**」
    let (start, end) = interval("@01-04 .. @02-01");
    assert_eq!(start, id(1, 4, 0, 0));
    assert_eq!(end, id(2, 1, 9999, 9999));

    let parsed = range("@01-04 .. @02-01");
    assert!(parsed.contains(start));
    assert!(parsed.contains(end));
    assert!(parsed.contains(id(1, 5, 0, 0)));
    assert!(parsed.contains(id(2, 1, 3, 4)));
    assert!(!parsed.contains(id(1, 3, 9, 9)));
    assert!(!parsed.contains(id(2, 2, 0, 0)));
}

#[test]
fn range_supports_open_ends() {
    // §5.1「`@03-01 ..` は上端開放、`.. @01-03` は下端開放」
    let upper = range("@03-01 ..");
    let upper_interval = upper.intervals()[0];
    assert_eq!(upper_interval.start, id(3, 1, 0, 0));
    assert_eq!(upper_interval.end, SceneId::MAX);
    assert!(upper.terms[0].is_upper_open());
    assert!(!upper.terms[0].is_lower_open());
    assert!(upper.contains(id(9999, 1, 1, 1)));

    let lower = range(".. @01-03");
    let lower_interval = lower.intervals()[0];
    assert_eq!(lower_interval.start, SceneId::MIN);
    assert_eq!(lower_interval.end, id(1, 3, 9999, 9999));
    assert!(lower.terms[0].is_lower_open());
    assert!(lower.contains(id(0, 0, 0, 0)));
    assert!(!lower.contains(id(1, 4, 0, 0)));
}

#[test]
fn range_single_term_covers_its_subtree() {
    // 区間演算子なしの `@01-04` は「1 章 4 話の下位すべて」
    let (start, end) = interval("@01-04");
    assert_eq!(start, id(1, 4, 0, 0));
    assert_eq!(end, id(1, 4, 9999, 9999));

    let parsed = range("@01-04");
    assert!(parsed.contains(id(1, 4, 2, 3)));
    assert!(!parsed.contains(id(1, 5, 0, 0)));
    assert!(!parsed.contains(id(2, 4, 0, 0)));
}

#[test]
fn range_is_union_of_comma_separated_terms() {
    // §5.1「カンマ区切りで複数区間の和集合」
    let parsed = range("@01, @03-02 .. @03-04");
    assert_eq!(parsed.len(), 2);
    assert!(parsed.contains(id(1, 9, 9, 9)));
    assert!(parsed.contains(id(3, 3, 0, 0)));
    assert!(!parsed.contains(id(2, 1, 0, 0)));
    assert!(!parsed.contains(id(3, 5, 0, 0)));
}

#[test]
fn range_reversed_bounds_are_e016() {
    // §5.1「区間の左端 > 右端 のときは E016」
    let reversed = range("@02 .. @01");
    let error = reversed.check().unwrap_err();
    assert_eq!(
        error,
        RangeError::Reversed {
            term: 0,
            start: id(2, 0, 0, 0),
            end: id(1, 9999, 9999, 9999),
        }
    );

    // 場面単位でも逆転する
    assert!(range("@01-04 .. @01-03").check().is_err());
    // 2 番目の項だけが逆
    let second = range("@01, @05 .. @04").check().unwrap_err();
    assert_eq!(term_of(second), 1);

    // 正常な式は通る
    assert!(range("@01 .. @02").check().is_ok());
    assert!(range("@01 .. @01").check().is_ok());
    assert!(range("@01 .. @02, @05 ..").check().is_ok());
}

/// `Reversed` の項番号を取り出す（それ以外の誤りならパニック）。
fn term_of(error: RangeError) -> usize {
    match error {
        RangeError::Reversed { term, .. } => term,
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn range_accepts_whitespace_variants() {
    // §5.1 の ebnf は `..` の前後とカンマの後の ws を許す
    let expected = (id(1, 0, 0, 0), id(2, 9999, 9999, 9999));
    for text in [
        "@01..@02",
        "@01 ..@02",
        "@01.. @02",
        " @01 .. @02 ",
        "\t@01 .. @02\n",
    ] {
        assert_eq!(interval(text), expected, "input: {text}");
    }
    let union = ["@01 .. @02,@05", "@01 .. @02 ,@05", "@01 .. @02, @05"];
    for text in union {
        assert_eq!(range(text).len(), 2, "input: {text}");
    }
}

#[test]
fn range_rejects_malformed_input() {
    assert_eq!(RangeExpr::parse(""), Err(RangeError::Empty));
    assert_eq!(RangeExpr::parse("   "), Err(RangeError::Empty));
    assert_eq!(RangeExpr::parse("@01,,"), Err(RangeError::Empty));
    assert_eq!(RangeExpr::parse("@01, ,@02"), Err(RangeError::Empty));
    // `@` が無い
    assert_eq!(RangeExpr::parse("01 .. 02"), Err(RangeError::MissingSigil));
    assert_eq!(RangeExpr::parse(".. 02"), Err(RangeError::MissingSigil));
    // ID 部分が壊れている
    assert_eq!(
        RangeExpr::parse("@01 .. @0x"),
        Err(RangeError::Id(IdError::NotANumber))
    );
    let doubled = RangeExpr::parse("@01 .. @02 .. @03");
    assert_eq!(doubled, Err(RangeError::Id(IdError::NotANumber)));
    assert_eq!(
        RangeExpr::parse("@12345"),
        Err(RangeError::Id(IdError::TooManyDigits))
    );
}

#[test]
fn range_term_may_have_one_to_four_levels() {
    // `partial_id` は 1 階層から書ける
    let (start, end) = interval("@01-02");
    assert_eq!(start, id(1, 2, 0, 0));
    assert_eq!(end, id(1, 2, 9999, 9999));

    let (start, end) = interval("@01-04-02-03");
    assert_eq!(start, id(1, 4, 2, 3));
    assert_eq!(end, id(1, 4, 2, 3));
}

#[test]
fn range_intersects_other_range() {
    // §3.3「範囲の交差判定」
    let first = range("@01 .. @02");
    assert!(first.intersects(&range("@02 .. @03")));
    assert!(first.intersects(&range("@01-05")));
    assert!(!first.intersects(&range("@03 .. @04")));

    // 和集合のどれか 1 項が交われば交わる
    let union = range("@01, @05 .. @06");
    assert!(union.intersects(&range("@05-01 .. @05-02")));
    assert!(!union.intersects(&range("@03 .. @04")));
}

// ------------------------------------------------- 予約番号 00（§5.1.2）

#[test]
fn reserved_zero_is_accepted() {
    // §5.1.2「番号としては正当な値であり、パースを拒否しない（MUST）」
    let stock = SceneId::parse_with_sigil("@00-01-02-03").unwrap();
    assert_eq!(stock, id(0, 1, 2, 3));

    // `@01-00` は正当な PartialSceneId（1 章のあらすじ＝②の機能だが番号は受理する）
    let blurb = PartialSceneId::parse_with_sigil("@01-00").unwrap();
    assert_eq!(blurb.depth(), 2);
    assert_eq!(blurb.episode(), Some(0));
    assert_eq!(blurb.expand_low(), id(1, 0, 0, 0));

    let split_blurb = SceneId::parse_with_sigil("@01-00-01-01").unwrap();
    assert_eq!(split_blurb, id(1, 0, 1, 1));

    // 範囲式でも通常の番号と同じ
    let parsed = range("@00 .. @01");
    assert!(parsed.contains(id(0, 5, 0, 0)));
    assert!(parsed.contains(id(1, 0, 0, 0)));
    assert!(!parsed.contains(id(2, 0, 0, 0)));

    // ソート順は数値順なのでストック章（00）が先頭、章あらすじ（B=00）が章の先頭
    let mut ids = vec![id(1, 1, 0, 0), id(0, 1, 0, 0), id(1, 0, 0, 0)];
    ids.sort();
    assert_eq!(ids, vec![id(0, 1, 0, 0), id(1, 0, 0, 0), id(1, 1, 0, 0)]);
}
