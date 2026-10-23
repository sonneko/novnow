//! 配役のワークスペースの補助（`core::features::cast`、M8、`docs/history.md` B-133）。

mod novel;
mod world;

use novnow::core::features::cast;
use novnow::core::id::SceneId;

#[test]
fn characters_are_listed_in_reading_order_with_their_lines() {
    let project = novel::novel();
    let rows = cast::characters(&project);
    let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, vec!["アルス", "リリア"]);
    assert_eq!(rows[0].reading, "あるす");
    assert_eq!(rows[0].lines.start, 0);
    let text = project.text(cast::PATH);
    let first: Vec<&str> = text.split('\n').collect();
    assert_eq!(
        first[rows[0].lines.end - 1],
        "'リリア: 幼馴染。",
        "末尾の空行を含まない"
    );
}

#[test]
fn the_timeline_has_a_column_per_chapter_and_a_band_per_mode() {
    let project = novel::novel();
    let timeline = cast::timeline(&project, 0);
    assert_eq!(timeline.chapters, vec![1, 2]);
    let ids: Vec<&str> = timeline.bands.iter().map(|band| band.id.as_str()).collect();
    assert_eq!(ids, vec!["A", "B"]);
    assert_eq!(timeline.bands[0].covered, vec![true, true]);
    assert_eq!(timeline.bands[1].covered, vec![false, true], "B は @02 ..");
    assert_eq!(timeline.bands[1].range, "@02 ..");
    assert_eq!(timeline.bands[1].label, "補習");
}

#[test]
fn shifting_a_band_rewrites_the_range_by_whole_chapters() {
    assert_eq!(cast::shift_range("@01 ..", 1).as_deref(), Some("@02 .."));
    assert_eq!(
        cast::shift_range("@01-03 .. @02", 1).as_deref(),
        Some("@02-03 .. @03"),
        "下の階層と桁数は保つ"
    );
    assert_eq!(
        cast::shift_range(".. @03, @05", -1).as_deref(),
        Some(".. @02, @04")
    );
    assert_eq!(cast::shift_range("@01", -1), None, "章 0 にはならない");
    let shifted = cast::shift_band(world::CHARACTERS, 0, 1, -1).expect("shift");
    assert!(shifted.contains("### B 補習\nrange: @01 ..\n"), "{shifted}");
    assert_eq!(
        shifted.len(),
        world::CHARACTERS.len(),
        "range の値だけが変わる"
    );
    // range が無い（全範囲）モードはずらせない
    let whole = world::CHARACTERS.replace("### A 探究\nrange: @01 ..\n", "### A 探究\n");
    assert_eq!(cast::shift_band(&whole, 0, 0, 1), None);
}

#[test]
fn the_tester_shows_which_patterns_match_and_their_regex() {
    let project = novel::novel();
    // リリア（H1 の 2 人目）: キャラ共通「*のね」・C 専用「*わよ」
    let result = cast::test_utterance(&project, 1, "そうなのね");
    let sources: Vec<(&Option<String>, &str, bool)> = result
        .checks
        .iter()
        .map(|check| (&check.mode, check.source.as_str(), check.matched))
        .collect();
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0], (&None, "*のね", true));
    assert_eq!(sources[1], (&Some("C".to_owned()), "*わよ", false));
    assert!(!result.checks[0].regex.is_empty());
    assert!(!result.underlines.is_empty(), "一致部分に下線");
    let empty = cast::test_utterance(&project, 1, "");
    assert!(empty.checks.iter().all(|check| !check.matched));
    assert!(empty.underlines.is_empty());
}

#[test]
fn unmatched_dialogue_lists_lines_whose_speaker_patterns_all_fail() {
    let project = novel::novel();
    let found = cast::unmatched_dialogue(&project);
    let texts: Vec<(&str, &str)> = found
        .iter()
        .map(|item| (item.speaker.as_str(), item.text.as_str()))
        .collect();
    assert!(texts.contains(&("リリア", "明日も来るの？")), "{texts:?}");
    assert!(
        !texts
            .iter()
            .any(|(_, text)| text.contains("ここにいたのね")),
        "「*のね」に一致する台詞は出さない"
    );
    let item = found
        .iter()
        .find(|item| item.text == "明日も来るの？")
        .expect("item");
    assert_eq!(
        item.scene,
        SceneId {
            chapter: 1,
            episode: 1,
            scene: 2,
            section: 1
        }
    );
    assert_eq!(item.path, "contents/01-01.txt");
    let text = project.text("contents/01-01.txt");
    assert!(text[item.byte..].starts_with("明日も来るの？"));
}
