//! アノテーション除去エクスポート（`core::export`、§17.1・§17.2、M9）の振る舞い。
//!
//! 不変条件（§17.1 の 1〜4）は `tests/export_invariants.rs`。ここは 1 段ずつの変換・範囲・状態の
//! 絞り込み・既定のファイル名を、仕様書の例（`tests/world`）と「実プロジェクト」（`tests/novel`）で見る。

mod novel;
mod world;

use novnow::core::export::{
    self, effective_state, file_name, parse_range, range_label, select, Break, Format, Options,
    State, SEPARATOR,
};
use novnow::core::features::Project;
use novnow::core::id::{Digits, SceneId};
use novnow::core::model::project::Newline;

use novel::{novel, novel_with, EXPECTED};
use world::{world, world_with, CONTENTS_PATH};

fn id(chapter: u16, episode: u16, scene: u16, section: u16) -> SceneId {
    SceneId {
        chapter,
        episode,
        scene,
        section,
    }
}

/// `project.toml` の既定（§4.5 の例: `blank-line`・字下げなし・ルビを残す・見出しなし・CRLF）。
fn defaults() -> Options {
    Options::from_settings(&Default::default())
}

fn lf() -> Options {
    Options {
        newline: Newline::Lf,
        ..defaults()
    }
}

fn whole(project: &Project, options: &Options) -> String {
    export::run(project, None, options)
}

fn ranged(project: &Project, range: &str, options: &Options) -> String {
    let range = parse_range(range).expect("範囲");
    export::run(project, Some(&range), options)
}

// ============================================================ 設定

#[test]
fn the_settings_of_section_4_5_become_the_options() {
    let options = defaults();
    assert_eq!(options.paragraph_break, Break::BlankLine);
    assert!(!options.indent_first_line);
    assert!(options.keep_ruby);
    assert!(!options.insert_headings);
    assert_eq!(options.newline, Newline::Crlf);
    assert_eq!(options.format, Format::Text);
    assert_eq!(options.state, None);
    // 表に無い値は既定の blank-line
    let mut settings = novnow::core::model::project::Export::default();
    for (value, want) in [
        ("none", Break::None),
        ("separator", Break::Separator),
        ("blank-line", Break::BlankLine),
        ("dots", Break::BlankLine),
    ] {
        settings.paragraph_break = value.to_owned();
        assert_eq!(
            Options::from_settings(&settings).paragraph_break,
            want,
            "{value}"
        );
    }
    assert_eq!(Format::of_name("out/a.md"), Format::Markdown);
    assert_eq!(Format::of_name("A.MD"), Format::Markdown);
    assert_eq!(Format::of_name("a.txt"), Format::Text);
    assert_eq!(State::parse(" fixed "), Some(State::Fixed));
    assert_eq!(State::parse("FIXED"), None);
    assert!(State::Draft < State::Review && State::Review < State::Fixed);
}

// ============================================================ §17.1 の各段

#[test]
fn the_spec_example_loses_every_annotation() {
    // §5.2.3 の例。`'『` の `'` は名前を持たないシジルなので消える（B-124）
    let want = "「やっぱりアルス、ここにいたのね」\r\n\
                \r\n\
                呆れたような声とともに、リリアが隣の椅子を引き寄せた。\r\n\
                \r\n\
                「旧校舎の地下遺跡に眠っているっていう『星屑の時計』の設計図だ」\r\n\
                \r\n\
                リリアは溜め息をついたが、図面の精巧な書き込みに目を奪われた。\r\n\
                \r\n\
                「……付き合ってあげるわよ」\r\n";
    assert_eq!(whole(&world(), &defaults()), want);
}

#[test]
fn the_real_project_exports_as_expected() {
    assert_eq!(whole(&novel(), &defaults()), EXPECTED);
    // 純粋関数: 何度やっても同じ
    assert_eq!(whole(&novel(), &defaults()), EXPECTED);
}

#[test]
fn section_breaks_follow_paragraph_break() {
    let project = world();
    let none = whole(
        &project,
        &Options {
            paragraph_break: Break::None,
            ..lf()
        },
    );
    assert!(
        none.contains("引き寄せた。\n「旧校舎の"),
        "none は何も入れない: {none}"
    );
    let separator = whole(
        &project,
        &Options {
            paragraph_break: Break::Separator,
            ..lf()
        },
    );
    assert!(
        separator.contains(&format!("引き寄せた。\n{SEPARATOR}\n「旧校舎の")),
        "{separator}"
    );
    assert_eq!(SEPARATOR, "　　＊");
    let blank = whole(&project, &lf());
    assert!(blank.contains("引き寄せた。\n\n「旧校舎の"), "{blank}");
}

#[test]
fn scene_episode_and_chapter_boundaries_are_breaks_too() {
    let separator = whole(
        &novel(),
        &Options {
            paragraph_break: Break::Separator,
            ..lf()
        },
    );
    // 場面の境目（`@01-01-02` の見出し）・話の境目・章の境目にも 1 回ずつ
    for (before, after) in [
        ("未登録の紋様 が描かれていた。", "夕焼けの中を"),
        ("「明日も来るの？」", "旧校舎の扉は"),
        ("が見える。", "補習の教室で"),
    ] {
        let at = separator.find(before).expect(before);
        let rest = &separator[at..];
        let next = rest.find(after).expect(after);
        assert_eq!(
            rest[..next].matches(SEPARATOR).count(),
            1,
            "{before} → {after}: {separator}"
        );
    }
    // 空の節（`;` が 2 つ続く）は区切りが 2 回になる（`;` 2 行をそのまま変換したのと同じ）
    assert!(
        separator.contains(&format!(
            "錆びついていた。\n{SEPARATOR}\n{SEPARATOR}\n扉の向こう"
        )),
        "{separator}"
    );
}

#[test]
fn comments_are_removed_and_comment_only_lines_vanish() {
    let text = whole(&novel(), &lf());
    for gone in [
        "メモ",
        "ここで節を切る",
        "後で書き直す",
        "仮の一文",
        "行コメントだけ",
        "見出しより前",
    ] {
        assert!(!text.contains(gone), "{gone}: {text}");
    }
    // 行内コメントの前後はつながる
    assert!(text.contains("リリアは溜め息をついた。窓の外は夕焼けだ。\n"));
    // URL の `://` はコメントではない（§5.0.2）
    assert!(text.contains("https://example.jp/hoshi"));
}

#[test]
fn speakers_references_and_materials_become_plain_text() {
    let text = whole(&novel(), &lf());
    // 話者注釈（直前の空白ごと）
    assert!(text.contains("「やっぱりアルス、ここにいたのね」\n"));
    assert!(text.contains("「明日も来るの？」\n"));
    assert!(!text.contains("@リリア") && !text.contains("@アルス"));
    // 参照（明示形・最短一致・未解決）
    assert!(text.contains("アルス は｜古文書"));
    assert!(text.contains("未登録の紋様 が"));
    // 資料参照は修飾子ごと消える（M0137 は索引にある、S0042 は無い）
    assert!(!text.contains("M0137") && !text.contains("S0042") && !text.contains("甲子園"));
    assert!(text.contains("「これに載ってたんだ」\n"));
    assert!(text.contains("彼が見せたのは古い写しだった。\n"));
    // 未解決の参照のモード式・資料 ID の形の未解決参照
    let project = novel_with(
        "contents/02-01.cnov",
        "@02-01-01 補習\n'未登録の人:B が来た。'S0042:検索結果 を見た。\n",
    );
    let text = ranged(&project, "@02", &lf());
    assert_eq!(text, "未登録の人 が来た。 を見た。\n");
}

#[test]
fn escapes_are_released() {
    let text = whole(&novel(), &lf());
    assert!(text.contains("本文の@は残る。宛先は hoshi@example.jp、"));
    assert!(text.contains("扉の向こうは暗かった。\\ 落書きの「「」が見える。"));
    // `\'` は参照にならず、`'` が残る。§5.0.3 に無い `\x` はそのまま
    let project = novel_with(
        "contents/02-01.cnov",
        "@02-01-01 補習\nIt\\'s \\'アルス のこと。\\x と \\; と \\/\\/ と \\「。\n",
    );
    let text = ranged(&project, "@02", &lf());
    assert_eq!(text, "It's 'アルス のこと。\\x と ; と // と 「。\n");
}

#[test]
fn rubies_follow_keep_ruby() {
    let keep = whole(&novel(), &lf());
    assert!(keep.contains("｜古文書《こもんじょ》"));
    assert!(keep.contains("魔導具《まどうぐ》"));
    let base = whole(
        &novel(),
        &Options {
            keep_ruby: false,
            ..lf()
        },
    );
    assert!(base.contains("アルス は古文書を広げている。"), "{base}");
    assert!(base.contains("魔導具の図面には、"), "{base}");
    assert!(!base.contains('《') && !base.contains('｜'), "{base}");
}

#[test]
fn the_first_line_indent_skips_quotes_and_indented_lines() {
    let text = whole(
        &novel(),
        &Options {
            indent_first_line: true,
            ..lf()
        },
    );
    let lines: Vec<&str> = text.lines().collect();
    // 既に字下げしてある行は二重にしない・「 で始まる行には入れない
    assert!(lines[0].starts_with("　放課後") && !lines[0].starts_with("　　"));
    assert_eq!(lines[1], "「やっぱりアルス、ここにいたのね」");
    assert_eq!(lines[3], "　リリアは溜め息をついた。窓の外は夕焼けだ。");
    assert!(lines.contains(&"　『星屑の時計』の設計図を、リリアは覗き込んだ。"));
    // 空行は空のまま
    assert_eq!(lines[2], "");
}

#[test]
fn headings_are_inserted_at_chapters_and_episodes() {
    let options = Options {
        insert_headings: true,
        ..lf()
    };
    let text = whole(&novel(), &options);
    assert!(
        text.starts_with("失われた刻印\n\n図書室の密談\n\n　放課後の"),
        "{text}"
    );
    // 話の頭（タイトルあり）には区切りの代わりに見出し、章の頭も同じ。タイトルの無い話（@02-01）は入れない
    assert!(
        text.contains("「明日も来るの？」\n\n旧校舎へ\n\n旧校舎の扉は"),
        "{text}"
    );
    assert!(
        text.contains("が見える。\n\n補習と地図\n\n補習の教室で"),
        "{text}"
    );
    let markdown = whole(
        &novel(),
        &Options {
            format: Format::Markdown,
            ..options.clone()
        },
    );
    assert!(
        markdown.starts_with("# 失われた刻印\n\n## 図書室の密談\n\n"),
        "{markdown}"
    );
    assert!(markdown.contains("\n\n## 旧校舎へ\n\n"));
    assert!(markdown.contains("\n\n# 補習と地図\n\n"));
    // 見出しを入れなければ Markdown でも見出しは無い
    let plain = whole(
        &novel(),
        &Options {
            format: Format::Markdown,
            ..lf()
        },
    );
    assert!(!plain.contains('#'));
}

#[test]
fn the_end_is_one_newline_and_crlf_is_the_default() {
    let crlf = whole(&novel(), &defaults());
    assert!(crlf.ends_with("いた。\r\n") && !crlf.ends_with("\r\n\r\n"));
    assert!(!crlf.replace("\r\n", "").contains('\n'));
    let unix = whole(&novel(), &lf());
    assert!(!unix.contains('\r') && unix.ends_with("いた。\n") && !unix.starts_with('\n'));
    assert_eq!(unix.replace('\n', "\r\n"), crlf);
    // 末尾に空行・空白だけの行があっても 1 個
    let project = world_with(CONTENTS_PATH, "@01-04-02 図書室\n本文。\n\n\u{3000}\n\n");
    assert_eq!(whole(&project, &lf()), "本文。\n");
    // 何も無ければ空文字列
    let project = world_with(CONTENTS_PATH, "@01-04-02 図書室\n// まだ書いていない\n");
    assert_eq!(whole(&project, &lf()), "");
}

#[test]
fn consecutive_blank_lines_collapse_to_one() {
    let project = world_with(
        CONTENTS_PATH,
        "@01-04-02 図書室\n\n\n一段落目。\n\n\n\n// メモ\n\n二段落目。\n;\n\n\n三段落目。\n",
    );
    assert_eq!(
        whole(&project, &lf()),
        "一段落目。\n\n二段落目。\n\n三段落目。\n"
    );
}

// ============================================================ 範囲（§17.2）

#[test]
fn ranges_pick_chapters_episodes_scenes_and_sections() {
    let project = novel();
    let ids = |range: &str| -> Vec<SceneId> {
        let range = parse_range(range).expect(range);
        select(&project, Some(&range), None)
            .into_iter()
            .map(|section| section.id)
            .collect()
    };
    assert_eq!(select(&project, None, None).len(), 8);
    assert_eq!(ids("@02"), vec![id(2, 1, 1, 1)]);
    assert_eq!(
        ids("@01-02"),
        vec![id(1, 2, 1, 1), id(1, 2, 1, 2), id(1, 2, 1, 3)]
    );
    assert_eq!(
        ids("@01-01-01"),
        vec![id(1, 1, 1, 1), id(1, 1, 1, 2), id(1, 1, 1, 3)]
    );
    assert_eq!(ids("@01-01-01-02"), vec![id(1, 1, 1, 2)]);
    // 範囲式・`@` の省略
    assert_eq!(
        ids("@01-01-02 .. @01-02-01-01, @02"),
        vec![id(1, 1, 2, 1), id(1, 2, 1, 1), id(2, 1, 1, 1)]
    );
    assert_eq!(
        ids("01-01-02 .. 01-02-01-01, 02"),
        ids("@01-01-02 .. @01-02-01-01, @02")
    );
    assert_eq!(ids("01-01-01-02"), vec![id(1, 1, 1, 2)]);
    assert!(ids("@03").is_empty());
    // 1 つの節だけなら区切りも見出しも無い
    assert_eq!(
        ranged(&project, "@01-01-01-02", &lf()),
        "「これに載ってたんだ」\n彼が見せたのは古い写しだった。\n本文の@は残る。宛先は hoshi@example.jp、資料は https://example.jp/hoshi にある。\n"
    );
    // 離れた節のあいだは区切り 1 回
    assert_eq!(
        ranged(&project, "@01-02-01-01, @02", &lf()),
        "旧校舎の扉は、錆びついていた。\n\n補習の教室で、アルス は居眠りをしていた。\n"
    );
}

#[test]
fn bad_ranges_are_rejected() {
    for bad in ["", "@", "@01-x", "@03 .. @01", "01,,02", "@01 @02"] {
        assert!(parse_range(bad).is_err(), "{bad:?}");
    }
}

// ============================================================ 状態（§17.2 `--state`、B-122）

#[test]
fn the_effective_state_comes_from_the_nearest_plot() {
    let project = novel();
    let state = |chapter, episode, scene, section| {
        effective_state(&project, id(chapter, episode, scene, section))
    };
    assert_eq!(state(1, 1, 1, 1), State::Fixed, "話プロットの節");
    assert_eq!(state(1, 1, 1, 2), State::Review, "章 @01（全体プロット）");
    assert_eq!(state(1, 1, 1, 3), State::Draft, "話プロットの節");
    assert_eq!(state(1, 1, 2, 1), State::Review, "章 @01");
    assert_eq!(state(1, 2, 1, 2), State::Fixed, "話 @01-02（章プロット）");
    assert_eq!(state(2, 1, 1, 1), State::Draft, "どこにも無い");
    // 場面レベルの state（最初の要点より前）も節の既定になる。読めない値は書かれていないのと同じ
    let project = novel_with(
        "plots/01-01.cnov",
        "@01-01-01 放課後\nstate: fixed\n二人が話す。\n;\n資料。\nstate: done\n;\n設計図。\nstate: draft\n",
    );
    let state = |section| effective_state(&project, id(1, 1, 1, section));
    assert_eq!(state(1), State::Fixed);
    assert_eq!(state(2), State::Fixed, "done は読めないので場面の fixed");
    assert_eq!(state(3), State::Draft);
}

#[test]
fn the_state_filter_keeps_sections_at_or_above() {
    let project = novel();
    let count = |state| select(&project, None, Some(state)).len();
    assert_eq!(count(State::Draft), 8);
    assert_eq!(count(State::Review), 6);
    assert_eq!(count(State::Fixed), 4);
    let fixed = export::run(
        &project,
        None,
        &Options {
            state: Some(State::Fixed),
            ..lf()
        },
    );
    assert_eq!(
        fixed,
        "\u{3000}放課後の図書室は静かだった。アルス は｜古文書《こもんじょ》を広げている。\n\
         「やっぱりアルス、ここにいたのね」\n\
         \n\
         リリアは溜め息をついた。窓の外は夕焼けだ。\n\
         \n\
         旧校舎の扉は、錆びついていた。\n\
         \n\
         扉の向こうは暗かった。\\ 落書きの「「」が見える。\n"
    );
    // 範囲と状態の両方
    let range = parse_range("@01-01").expect("範囲");
    let both: Vec<SceneId> = select(&project, Some(&range), Some(State::Review))
        .into_iter()
        .map(|section| section.id)
        .collect();
    assert_eq!(both, vec![id(1, 1, 1, 1), id(1, 1, 1, 2), id(1, 1, 2, 1)]);
}

// ============================================================ 既定の書き出し先（§17.2）

#[test]
fn the_default_file_name_is_title_range_and_time() {
    let digits = Digits::default();
    assert_eq!(range_label(None, digits), "all");
    let label = |text: &str| range_label(Some(&parse_range(text).expect(text)), digits);
    assert_eq!(label("@01-04"), "01-04");
    assert_eq!(label("01 .. 03, 05"), "01~03_05");
    assert_eq!(label(".. @01-03"), "~01-03");
    assert_eq!(label("@03-01 .."), "03-01~");
    assert_eq!(
        file_name("星屑の時計", "01-04", "20261013-2140", Format::Text),
        "星屑の時計-01-04-20261013-2140.txt"
    );
    assert_eq!(
        file_name(
            " a/b\\c:d*e?f\"g<h>i|j ",
            "all",
            "20261013-2140",
            Format::Markdown
        ),
        "a_b_c_d_e_f_g_h_i_j-all-20261013-2140.md"
    );
    assert_eq!(
        file_name("", "all", "20261013-2140", Format::Text),
        "project-all-20261013-2140.txt"
    );
    assert_eq!(
        file_name(".hidden", "all", "20261013-2140", Format::Text),
        "_.hidden-all-20261013-2140.txt"
    );
}

#[test]
fn the_compact_stamp_is_yyyymmdd_hhmm() {
    use novnow::adapters::clock_system::compact_stamp;
    // 2026-10-13T12:40:59Z
    let millis = 1_791_895_259_000;
    assert_eq!(compact_stamp(millis, 0), "20261013-1240");
    assert_eq!(compact_stamp(millis, 540), "20261013-2140");
    assert_eq!(compact_stamp(millis, 12 * 60), "20261014-0040");
}
