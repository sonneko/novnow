//! `novnow new` の雛形と既定プロンプト（`core::template`、§14.5・§15.5、`docs/history.md` の作業項目 6）。
//!
//! - 雛形は §14.5 の表のとおりのファイルを持ち、§4.3 の判定を通る。
//! - **雛形に診断が 1 件も出ない**（作家が最初に見る診断がサンプル由来にならないように。
//!   `novnow check` を雛形に掛けると 0 件、の機械化）。
//! - 既定プロンプトは 7 役割 ＋ `_shared/` の 2 本で、§5.9.1 の形（H1 = 役割名・`system` / `task` /
//!   `output`）を守り、§15.5.3 の閉じた変数だけを使い、§15.5.2 の共通の約束と付録 A の要素を含む。

use std::collections::BTreeSet;

use novnow::adapters::project_load::is_document;
use novnow::core::diagnostics;
use novnow::core::features::Project;
use novnow::core::model::document::Document;
use novnow::core::model::project::{validate, Listing};
use novnow::core::policy::ProjectPath;
use novnow::core::syntax::prompt_md::{self, PromptSectionKey, BUILTIN_ROLES, VARIABLES};
use novnow::core::template::{self, DEFAULT_PROMPTS};

const SPEC: &str = include_str!("../docs/specifications/platform.md");

/// 雛形の文書（`Document` になるものだけ）を集約に入れる。
fn template_project() -> (Vec<Document>, Project) {
    // project_load::load と同じ選び方（`.gitkeep` などは文書にしない）
    let documents: Vec<Document> = template::new_project("untitled")
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(file.path).expect("path")))
        .map(|file| Document::new(file.path, file.text))
        .collect();
    let project = Project::with_documents(Default::default(), documents.clone());
    (documents, project)
}

#[test]
fn the_template_has_exactly_the_files_of_section_14_5() {
    let paths: Vec<&str> = template::new_project("x")
        .iter()
        .map(|file| file.path)
        .collect();
    let mut want = vec![
        "project.json",
        "characters.md",
        "words.md",
        "ideas.md",
        "contents/01-01.txt",
        "plots/all.txt",
        "plots/01.txt",
        "plots/01-01.txt",
        "materials/index.json",
        "materials/docs/.gitkeep",
    ];
    want.extend(DEFAULT_PROMPTS.iter().map(|(path, _)| *path));
    want.extend([
        "agents/mcp.json",
        "agents/tasks/.gitkeep",
        "agents/results/.gitkeep",
        "agents/threads/.gitkeep",
        "agents/proposals/.gitkeep",
        "agents/log/.gitkeep",
        ".gitignore",
    ]);
    assert_eq!(paths, want);
    // §1.2（② の除外）: prompts/custom/ と assets/ は作らない（`docs/history.md` 作業項目 6）
    assert!(paths
        .iter()
        .all(|path| !path.starts_with("prompts/custom/") && !path.starts_with("assets/")));
    // 名前の重複が無い
    let unique: BTreeSet<&str> = paths.iter().copied().collect();
    assert_eq!(unique.len(), paths.len());
}

#[test]
fn the_template_passes_the_project_validation() {
    let files = template::new_project("x");
    let mut listing = Listing::default();
    for file in &files {
        listing.files.insert(file.path.to_owned());
        let mut dir = file.path;
        while let Some((parent, _)) = dir.rsplit_once('/') {
            listing.dirs.insert(parent.to_owned());
            dir = parent;
        }
    }
    listing.project_json = Some(template::project_json("x"));
    assert_eq!(validate(&listing), Ok(()));
}

#[test]
fn the_template_has_no_diagnostics_at_all() {
    // `novnow check` を雛形に掛けたときの 0 件（E も W も I も）
    let (documents, project) = template_project();
    assert_eq!(documents.len(), 19);
    for document in &documents {
        let found = diagnostics::document(document, &project);
        assert!(
            found
                .iter()
                .all(|d| d.severity() == novnow::core::diagnostics::Severity::Info),
            "{} に診断が出た: {found:?}",
            document.path
        );
    }
}

#[test]
fn the_sample_uses_the_dictionary_and_modes() {
    // §14.5: サンプル 1 名（モード 2 つ・役割語 2 本）・1 語・節のサンプル（mode: つき）
    let (_, project) = template_project();
    let characters = project.text("characters.md");
    assert_eq!(characters.matches("\n### ").count(), 2, "{characters}");
    assert!(characters.contains("「*だよ」「*かな」"));
    assert_eq!(project.text("words.md").matches("\n# ").count(), 1);
    assert!(project.text("contents/01-01.txt").contains("@01-01-01"));
    assert!(project
        .text("plots/01-01.txt")
        .contains("mode: '星野ひかり:A"));
    assert!(project.text("plots/all.txt").contains("summary: "));
    // ideas.md はコメントだけ（見出しはアイデア 1 件になる。§5.7）
    let ideas = project.text("ideas.md");
    assert!(ideas.lines().all(|line| line.starts_with("//")), "{ideas}");
}

#[test]
fn json_files_and_gitignore_are_as_section_14_5() {
    let value: serde_json::Value =
        serde_json::from_str(&template::project_json("星屑の \"時計\"")).expect("json");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["title"], "星屑の \"時計\"");
    assert_eq!(value.as_object().expect("object").len(), 2);
    let index: serde_json::Value =
        serde_json::from_str(template::MATERIALS_INDEX).expect("index.json");
    assert_eq!(
        index,
        serde_json::json!({"schema_version": 1, "next_id": 1, "items": []})
    );
    let mcp: serde_json::Value = serde_json::from_str(template::MCP_CONFIG).expect("mcp.json");
    assert_eq!(mcp, serde_json::json!({"schema_version": 1, "servers": {}}));
    assert_eq!(template::GITIGNORE, ".novnow/\nexports/\n");
}

#[test]
fn every_template_file_is_lf_utf8_with_one_trailing_newline() {
    for file in template::new_project("x") {
        if file.text.is_empty() {
            continue; // .gitkeep
        }
        assert!(!file.text.contains('\r'), "{}", file.path);
        assert!(!file.text.starts_with('\u{feff}'), "{}", file.path);
        assert!(file.text.ends_with('\n'), "{}", file.path);
        assert!(!file.text.ends_with("\n\n"), "{}", file.path);
        assert!(
            file.text.lines().all(|line| line == line.trim_end()),
            "行末の空白: {}",
            file.path
        );
    }
}

#[test]
fn the_template_does_not_hardcode_urls() {
    // §14.2.1: URL は定数だけを参照する（MUST）。雛形の中に書くと二重管理になる
    for file in template::new_project("x") {
        assert!(!file.text.contains("http"), "{}", file.path);
    }
}

#[test]
fn the_default_prompts_are_the_seven_roles_and_the_shared_files() {
    let paths: Vec<&str> = DEFAULT_PROMPTS.iter().map(|(path, _)| *path).collect();
    let mut want = vec![
        "prompts/_shared/style.md".to_owned(),
        "prompts/_shared/output-contract.md".to_owned(),
    ];
    want.extend(
        BUILTIN_ROLES
            .iter()
            .map(|role| format!("prompts/{role}.md")),
    );
    assert_eq!(paths, want);
    assert_eq!(BUILTIN_ROLES.len(), 7);
    for (path, text) in DEFAULT_PROMPTS {
        assert_eq!(template::default_prompt(path), Some(*text));
    }
    assert_eq!(template::default_prompt("prompts/custom/x.md"), None);
}

#[test]
fn every_role_prompt_has_the_shape_of_section_5_9_1() {
    for role in BUILTIN_ROLES {
        let path = format!("prompts/{role}.md");
        let text = template::default_prompt(&path).expect("prompt");
        let prompt = prompt_md::parse(text);
        assert_eq!(prompt.diagnostics, Vec::new(), "{path}");
        assert_eq!(prompt.role.as_ref().map(|found| found.name), Some(*role));
        let sections = prompt.known_sections();
        for key in [
            PromptSectionKey::System,
            PromptSectionKey::Task,
            PromptSectionKey::Output,
        ] {
            assert!(sections.contains(&key), "{path} に {key:?} が無い");
        }
        // 未知のセクションを作らない（§5.9.1 の 5 種だけ）
        assert!(
            prompt
                .sections
                .iter()
                .all(|found| found.key != PromptSectionKey::Other),
            "{path}"
        );
        for key in ["model-hint", "max-chars", "temperature"] {
            assert!(prompt.meta_field(key).is_some(), "{path} に {key} が無い");
        }
        // §15.5.3 の閉じた集合だけ（E031 が出ないことは diagnostics で見たが、念のため名前でも）
        for variable in &prompt.variables {
            assert!(
                VARIABLES.contains(&variable.name),
                "{path}: {}",
                variable.name
            );
        }
    }
}

#[test]
fn every_role_prompt_carries_the_common_promises_of_section_15_5_2() {
    // 1 役割の境界 / 2 出力契約 / 3 不確実性 / 4 改変禁止域 / 5 言語 / 6 長さ / 7 記法 / 8 委譲レベル
    let promises: [(&str, &[&str]); 8] = [
        ("役割の境界", &["他の役割の仕事はしません"]),
        ("出力契約", &["前置き"]),
        ("不確実性", &["## question"]),
        ("改変禁止域", &["state: fixed"]),
        ("言語", &["日本語"]),
        ("長さ", &["±20%"]),
        ("記法", &["@ ' ;"]),
        ("委譲レベル", &["{{delegation}}"]),
    ];
    for role in BUILTIN_ROLES {
        let path = format!("prompts/{role}.md");
        let text = template::default_prompt(&path).expect("prompt");
        for (promise, needles) in promises {
            assert!(
                needles.iter().any(|needle| text.contains(needle)),
                "{path} に §15.5.2 の「{promise}」が無い"
            );
        }
    }
}

#[test]
fn the_prompts_carry_the_must_have_elements_of_appendix_a() {
    // 付録 A と §15.5.1 の「外してはならない要素」（代表的な語で確かめる）
    let musts: &[(&str, &[&str])] = &[
        (
            "prompts/_shared/style.md",
            &["人称", "時制", "地の文と台詞の比率", "句読点"],
        ),
        (
            "prompts/_shared/output-contract.md",
            &[
                "本文だけを出す",
                "見出し",
                "アノテーションの記法を勝手に使わない",
                "質問として",
                "±20%",
                "変更箇所を最小",
                "不確実である",
                "固有名詞を作らない",
            ],
        ),
        (
            "prompts/writer.md",
            &[
                "personality",
                "役割語",
                "発明しない",
                "直前の節",
                "check",
                "{{target_chars}}",
            ],
        ),
        (
            "prompts/plotter.md",
            &[
                "1 節 = 1 出来事",
                "goal",
                "seed",
                "payoff",
                "本文や台詞を書かない",
            ],
        ),
        (
            "prompts/ideator.md",
            &["1 行 1 案", "最大 20 案", "重複させない", "2 割"],
        ),
        (
            "prompts/researcher.md",
            &[
                "出どころ",
                "事実・推測・一般論",
                "120 字",
                "conditions",
                "加工しない",
            ],
        ),
        (
            "prompts/stylist.md",
            &[
                "書き換えずに、指摘します",
                "位置・該当する文字列・理由・改善案",
                "役割語の不一致を最優先",
                "矯正しない",
            ],
        ),
        (
            "prompts/logician.md",
            &[
                "seed と payoff",
                "時系列",
                "良し悪しを評価しない",
                "確実な矛盾 / 疑わしい点 / 情報不足で判断不能",
            ],
        ),
        (
            "prompts/integrator.md",
            &[
                "次にすべき 1 手",
                "人間がすべき 1 手",
                "起動する指示を出さない",
                "autonomous",
                "停滞の理由",
            ],
        ),
    ];
    for (path, needles) in musts {
        let text = template::default_prompt(path).expect("prompt");
        for needle in *needles {
            assert!(text.contains(needle), "{path} に「{needle}」が無い");
        }
    }
}

#[test]
fn the_spec_lists_the_same_template() {
    // §14.5 の表の左の列（パス）が雛形と食い違っていないか（仕様書を読み直す）
    let start = SPEC.find("### 14.5 `novnow new` の生成物").expect("§14.5");
    let block = &SPEC[start..];
    let block = &block[block.find("```").expect("open") + 3..];
    let block = &block[..block.find("```").expect("close")];
    for name in [
        "project.json",
        "characters.md",
        "words.md",
        "ideas.md",
        "contents/01-01.txt",
        "plots/all.txt",
        "plots/01.txt",
        "plots/01-01.txt",
        "materials/index.json",
        "materials/docs/.gitkeep",
        "prompts/*.md",
        "agents/mcp.json",
        ".gitignore",
    ] {
        assert!(block.contains(name), "§14.5 に {name} が無い");
    }
}

#[test]
fn sample_project_covers_appendix_c_with_no_diagnostics() {
    use novnow::core::export::{self, State};
    use novnow::core::features::{cast, plots, reference, sections};

    let files = template::sample_project();
    let mut listing = Listing::default();
    for file in &files {
        listing.files.insert(file.path.to_owned());
        let mut dir = file.path;
        while let Some((parent, _)) = dir.rsplit_once('/') {
            listing.dirs.insert(parent.to_owned());
            dir = parent;
        }
    }
    listing.project_json = files
        .iter()
        .find(|file| file.path == "project.json")
        .map(|file| file.text.clone());
    assert_eq!(validate(&listing), Ok(()));

    let documents: Vec<Document> = files
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(file.path).expect("path")))
        .map(|file| Document::new(file.path, file.text))
        .collect();
    let project = Project::with_documents(Default::default(), documents.clone());

    // 1. 診断が 0 件（① の範囲のみ・未知キー無し）
    for document in &documents {
        let found = diagnostics::document(document, &project);
        assert!(
            found
                .iter()
                .all(|d| d.code == novnow::core::diagnostics::DiagnosticCode::I054),
            "{} に診断が出た: {found:?}",
            document.path
        );
    }

    // 2. モードの併用（'リリア:B+C）と range 変種（リリアの ### B が 2 回）
    assert!(project
        .text("plots/01-04.txt")
        .contains("mode: 'リリア:B+C"));
    assert_eq!(
        project
            .text("characters.md")
            .lines()
            .filter(|line| line.starts_with("### B 呆れ"))
            .count(),
        2
    );

    // 3. 役割語の一致例と不一致例（一致した台詞は未一致一覧に出ず、不一致の台詞が出る）
    let unmatched = cast::unmatched_dialogue(&project);
    assert!(
        unmatched
            .iter()
            .any(|item| item.speaker == "リリア" && item.text.contains("眉唾な噂話")),
        "unmatched: {unmatched:?}"
    );
    for matched in [
        "ここにいたのね",
        "言ってたでしょ",
        "これを見てくれ",
        "設計図だ。",
        "行くつもりなの？",
        "来るだろ？",
        "付き合ってあげるわよ。",
    ] {
        assert!(
            !unmatched.iter().any(|item| item.text.contains(matched)),
            "一致する台詞が未一致に入った ({matched}): {unmatched:?}"
        );
    }

    // 4. 資料 ID 参照（明示参照）と条件一致による資料候補
    use novnow::core::id::SceneId;
    let all_sections = sections::all(&project);
    let doc = project.document("contents/01-04.txt").expect("01-04.txt");
    let sec2 = all_sections
        .iter()
        .find(|sec| sec.id == SceneId::parse("01-04-02-02").unwrap())
        .expect("01-04-02-02");
    let pane2 = reference::at(doc, sec2.lines.clone(), sec2.id, &project);
    assert!(
        pane2
            .materials
            .iter()
            .any(|mat| mat.id == "M0001" && !mat.candidate),
        "01-04-02-02 に M0001 の明示参照がある: {:?}",
        pane2.materials
    );
    let sec4 = all_sections
        .iter()
        .find(|sec| sec.id == SceneId::parse("01-04-02-04").unwrap())
        .expect("01-04-02-04");
    let pane4 = reference::at(doc, sec4.lines.clone(), sec4.id, &project);
    assert!(
        pane4
            .materials
            .iter()
            .any(|mat| mat.id == "M0001" && mat.candidate),
        "01-04-02-04 に M0001 の条件一致候補がある: {:?}",
        pane4.materials
    );

    // 5. 未回収の伏線 1 件（F01）
    let foreshadows = plots::foreshadows(&project);
    assert_eq!(foreshadows.len(), 1);
    assert_eq!(foreshadows[0].tag, "F01");
    assert!(foreshadows[0].open());

    // 6. state: fixed の節 1 つ
    assert_eq!(export::effective_state(&project, sec4.id), State::Fixed);

    // 7. アイデア 1 件（I0001）
    assert!(project.text("ideas.md").contains("# I0001 学院長の動機"));
}
