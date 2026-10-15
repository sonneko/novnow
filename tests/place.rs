//! 診断の「どこ」を作家の言葉で（`core::features::place`、§14.2.5・§10.9.2、仕様 第 2.11 版）。
//!
//! 位置は原文の部分文字列から求める（byte を手で数えない）。表示（`ui::i18n::place`）も確かめる。

mod world;

use novnow::core::features::place::{self, Place};
use novnow::core::features::Project;
use novnow::core::model::document::Document;
use novnow::ui::i18n::{self, Locale};

use world::{world, world_with, ALL_PATH, CONTENTS_PATH, EPISODE_PATH};

/// `path` の文書で `needle` が最初に現れる位置の「どこ」。
fn at(project: &Project, path: &str, needle: &str) -> Option<Place> {
    let document = project.document(path).cloned().expect(path);
    let byte = document
        .text
        .find(needle)
        .unwrap_or_else(|| panic!("{path} に {needle:?} が無い"));
    place::at(&document, Some(byte), project)
}

fn whole(project: &Project, path: &str) -> Option<Place> {
    let document = project.document(path).cloned().expect(path);
    place::at(&document, None, project)
}

fn contents(id: &str) -> Option<Place> {
    Some(Place::Contents(id.to_owned()))
}

#[test]
fn prose_positions_are_section_ids() {
    // §5.2.3 の例（@01-04-02 図書室の攻防、`;` で 3 節）
    let project = world();
    assert_eq!(
        at(&project, CONTENTS_PATH, "呆れたような声"),
        contents("01-04-02-01")
    );
    assert_eq!(
        at(&project, CONTENTS_PATH, "旧校舎の地下遺跡"),
        contents("01-04-02-02")
    );
    assert_eq!(
        at(&project, CONTENTS_PATH, "付き合ってあげる"),
        contents("01-04-02-03")
    );
    // 見出しの行は場面、`;` の行はそれが閉じる節
    assert_eq!(
        at(&project, CONTENTS_PATH, "@01-04-02"),
        contents("01-04-02")
    );
    let document = project.document(CONTENTS_PATH).cloned().expect("doc");
    let first_break = document.text.find("\n;").expect(";") + 1;
    assert_eq!(
        place::at(&document, Some(first_break), &project),
        contents("01-04-02-01")
    );
    // 位置の無い診断はファイル名の話
    assert_eq!(whole(&project, CONTENTS_PATH), contents("01-04"));
}

#[test]
fn prose_before_the_first_heading_is_the_episode() {
    let project = world_with("contents/01-05.txt", "// メモ\n@01-05-01\n本文\n");
    assert_eq!(
        at(&project, "contents/01-05.txt", "メモ"),
        contents("01-05")
    );
    assert_eq!(
        at(&project, "contents/01-05.txt", "本文"),
        contents("01-05-01-01")
    );
}

#[test]
fn episode_plot_positions_are_section_ids_and_scene_fields_are_the_scene() {
    // §5.4.3 の例
    let project = world();
    let plot = |id: &str| Some(Place::EpisodePlot(id.to_owned()));
    assert_eq!(at(&project, EPISODE_PATH, "cast: "), plot("01-04-02"));
    assert_eq!(at(&project, EPISODE_PATH, "幼馴染の"), plot("01-04-02-01"));
    assert_eq!(
        at(&project, EPISODE_PATH, "地下遺跡の'星屑"),
        plot("01-04-02-02")
    );
    assert_eq!(
        at(&project, EPISODE_PATH, "呆れつつも"),
        plot("01-04-02-03")
    );
    assert_eq!(
        at(&project, EPISODE_PATH, "放課後に旧校舎"),
        plot("01-04-02-04")
    );
}

#[test]
fn outline_positions_are_block_ids() {
    // §5.3.3 の例（全体プロット）
    let project = world();
    assert_eq!(
        at(&project, ALL_PATH, "title: "),
        Some(Place::WholePlot(None))
    );
    assert_eq!(
        at(&project, ALL_PATH, "期末試験の失敗"),
        Some(Place::WholePlot(Some("02".to_owned())))
    );
    // 章プロット: ブロックの ID、ブロックより前はファイル名の章
    let chapter = world_with(
        "plots/01.txt",
        "// 第 1 章\n@01-04 図書室の密談\nsummary: 約束\n;\n",
    );
    assert_eq!(
        at(&chapter, "plots/01.txt", "約束"),
        Some(Place::ChapterPlot("01-04".to_owned()))
    );
    assert_eq!(
        at(&chapter, "plots/01.txt", "第 1 章"),
        Some(Place::ChapterPlot("01".to_owned()))
    );
}

#[test]
fn dictionary_positions_are_names() {
    let project = world_with(
        "characters.md",
        "# 恩田恵\nrole: ヒロイン\n\n## modes\n\n### A 日常\npersonality: 控えめ\n\n## relations\n'アルス: 友人\n",
    );
    assert_eq!(
        at(&project, "characters.md", "控えめ"),
        Some(Place::Character {
            name: "恩田恵".to_owned(),
            mode: Some("A 日常".to_owned()),
        })
    );
    assert_eq!(
        at(&project, "characters.md", "ヒロイン"),
        Some(Place::Character {
            name: "恩田恵".to_owned(),
            mode: None,
        })
    );
    // §5.6 の例の 2 語目
    let project = world();
    assert_eq!(
        at(&project, "words.md", "地下遺跡への入口"),
        Some(Place::Word("旧校舎".to_owned()))
    );
    let ideas = world_with("ideas.md", "# I0007 学院長の動機\nstatus: inbox\n本文\n");
    assert_eq!(
        at(&ideas, "ideas.md", "本文"),
        Some(Place::Idea {
            id: "I0007".to_owned(),
            title: "学院長の動機".to_owned(),
        })
    );
}

#[test]
fn materials_are_ids_and_titles() {
    let project = world();
    let want = Some(Place::Material {
        id: "M0137".to_owned(),
        title: "科学の甲子園 本戦の運営と実技課題".to_owned(),
    });
    assert_eq!(at(&project, "materials/index.json", "大会運営の手順"), want);
    // 資料の本体は索引の path で引く
    let with_doc = world_with(
        "materials/docs/M0137-kagaku-koshien.md",
        "# 科学の甲子園\n本文\n",
    );
    assert_eq!(
        at(&with_doc, "materials/docs/M0137-kagaku-koshien.md", "本文"),
        want
    );
    // 索引に無ければファイル名の ID だけ
    let orphan = world_with("materials/docs/M0200-x.md", "# x\n");
    assert_eq!(
        whole(&orphan, "materials/docs/M0200-x.md"),
        Some(Place::Material {
            id: "M0200".to_owned(),
            title: String::new(),
        })
    );
}

#[test]
fn prompts_settings_and_the_rest() {
    let project = world_with(
        "prompts/writer.md",
        "# writer\nmodel-hint: balanced\n\n## system\nあなたは本文担当です。\n\n## output\n- 本文だけ\n",
    );
    assert_eq!(
        at(&project, "prompts/writer.md", "本文だけ"),
        Some(Place::Prompt {
            role: "writer".to_owned(),
            section: Some("output".to_owned()),
        })
    );
    let settings = world_with("project.json", "{\"schema_version\": 1}\n");
    assert_eq!(whole(&settings, "project.json"), Some(Place::Settings));
    let mcp = world_with(
        "agents/mcp.json",
        "{\"schema_version\": 1, \"servers\": {}}\n",
    );
    assert_eq!(whole(&mcp, "agents/mcp.json"), None);
}

#[test]
fn places_read_naturally_in_both_languages() {
    let cases: Vec<(Place, &str, &str)> = vec![
        (
            Place::Contents("01-04-02-03".to_owned()),
            "本文 @01-04-02-03",
            "prose @01-04-02-03",
        ),
        (
            Place::EpisodePlot("01-04-02".to_owned()),
            "話プロット @01-04-02",
            "episode plot @01-04-02",
        ),
        (
            Place::ChapterPlot("01-04".to_owned()),
            "章プロット @01-04",
            "chapter plot @01-04",
        ),
        (
            Place::ChapterPlot(String::new()),
            "章プロット",
            "chapter plot",
        ),
        (
            Place::WholePlot(Some("01".to_owned())),
            "全体プロット @01",
            "overall plot @01",
        ),
        (Place::WholePlot(None), "全体プロット", "overall plot"),
        (
            Place::Character {
                name: "恩田恵".to_owned(),
                mode: Some("C 混乱".to_owned()),
            },
            "登場人物「恩田恵」のモード C 混乱",
            "character \"恩田恵\", mode C 混乱",
        ),
        (
            Place::Word("旧校舎".to_owned()),
            "用語「旧校舎」",
            "term \"旧校舎\"",
        ),
        (
            Place::Idea {
                id: "I0007".to_owned(),
                title: "学院長の動機".to_owned(),
            },
            "アイデア I0007「学院長の動機」",
            "idea I0007 \"学院長の動機\"",
        ),
        (
            Place::Material {
                id: "M0137".to_owned(),
                title: "旧校舎の構造メモ".to_owned(),
            },
            "資料 M0137「旧校舎の構造メモ」",
            "material M0137 \"旧校舎の構造メモ\"",
        ),
        (
            Place::Material {
                id: "M0200".to_owned(),
                title: String::new(),
            },
            "資料 M0200",
            "material M0200",
        ),
        (
            Place::Prompt {
                role: "writer".to_owned(),
                section: Some("output".to_owned()),
            },
            "プロンプト writerの output",
            "prompt writer, section output",
        ),
        (Place::Settings, "作品の設定", "project settings"),
    ];
    for (found, ja, en) in cases {
        assert_eq!(i18n::place(Locale::Ja, &found), ja);
        assert_eq!(i18n::place(Locale::En, &found), en);
    }
}

#[test]
fn a_document_without_structure_has_no_place() {
    let project = world_with("agents/tasks/T0001.md", "# T0001\n");
    let document: Document = project
        .document("agents/tasks/T0001.md")
        .cloned()
        .expect("doc");
    assert_eq!(place::at(&document, Some(0), &project), None);
}
