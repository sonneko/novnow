//! `diagnostics::document`（§3.4 → `textDocument/publishDiagnostics`）。

mod world;

use novnow::core::diagnostics::{self, Arg, Diagnostic, DiagnosticCode};
use novnow::core::features::Project;

use world::*;

/// `CODE@原文の切り出し(引数)` を並べた 1 行（範囲を手で数えない）。
fn tags(project: &Project, path: &str) -> String {
    let document = doc(project, path);
    let found = diagnostics::document(&document, project);
    found
        .iter()
        .map(|item| tag(&document.text, item))
        .collect::<Vec<_>>()
        .join(" / ")
}

fn tag(text: &str, item: &Diagnostic) -> String {
    let range = item
        .range
        .as_ref()
        .map(|range| slice(text, range))
        .unwrap_or_default();
    let args: Vec<String> = item
        .args
        .iter()
        .map(|arg| match arg {
            Arg::Text(value) => value.clone(),
            Arg::Number(value) => value.to_string(),
        })
        .collect();
    if args.is_empty() {
        format!("{}@{range}", item.code.as_str())
    } else {
        format!("{}@{range}({})", item.code.as_str(), args.join(","))
    }
}

// ============================================================ 仕様書の例

#[test]
fn the_spec_examples_only_warn_about_what_the_spec_itself_leaves_open() {
    let project = world();
    // 定義側の 3 ファイルは診断 0 件
    assert_eq!(tags(&project, "characters.md"), "");
    assert_eq!(tags(&project, "words.md"), "");
    assert_eq!(tags(&project, "materials/index.json"), "I054@(1)");
    // 本文（§5.2.3）も 0 件（`'『星屑の時計』` は `'` の直後が区切り文字なので参照ではない）
    assert_eq!(tags(&project, CONTENTS_PATH), "");
    assert_eq!(tags(&project, ALL_PATH), "");
    // 話プロット（§5.4.3）は ② のキー（`when` など）の W011 と、本文（§5.2.3、3 節）との
    // 節数の食い違い W030 だけ。どちらも仕様書の例そのものの性質
    let episode = tags(&project, EPISODE_PATH);
    assert!(episode.starts_with("W030@@01-04-02(4,3)"), "{episode}");
    let rest: Vec<&str> = episode.split(" / ").skip(1).collect();
    assert!(
        rest.iter().all(|item| item.starts_with("W011@")),
        "{episode}"
    );
}

// ============================================================ 層ごとの診断が 1 か所に集まる

#[test]
fn syntax_and_reference_diagnostics_come_together_sorted() {
    let text =
        "@01-04-01 x\n「'だれか、来て」@ゲスト\n'M9999 と 'リリア:Z\n@01-04 階層が足りない\n";
    let project = world_with(CONTENTS_PATH, text);
    // E → W → I、同じ重大度なら位置の順（§9.5.2 の 9）
    assert_eq!(
        tags(&project, CONTENTS_PATH),
        "E001@'だれ(だれ) / E012@@ゲスト(ゲスト) / E007@'M9999(M9999) / \
         E003@'リリア:Z(リリア,Z) / E020@@01-04(3-4) / W031@'M9999"
    );
}

#[test]
fn speaker_modes_are_checked_against_the_scene_of_the_line() {
    // §6.4.4: アルスの B は `range: @02 ..` なので 1 章では E004、未定義の Z は E003。
    // 波線は話者注釈全体（M3 の `modes::resolve` の段 1 の流儀）
    let text = "@01-04-01 x\n「行くぞ」@アルス:B\n「待って」@リリア:Z\n";
    let project = world_with(CONTENTS_PATH, text);
    assert_eq!(
        tags(&project, CONTENTS_PATH),
        "E004@@アルス:B(アルス,B,01-04-01-01,@02 ..) / E003@@リリア:Z(リリア,Z)"
    );
    // 本文中の参照に書いたモードも同じ検査（8 段の解決を通らないので M4 が見る、B-79）
    let body = world_with(CONTENTS_PATH, "@01-04-01 x\n'アルス:B が来た。\n");
    assert_eq!(
        tags(&body, CONTENTS_PATH),
        "E004@'アルス:B(アルス,B,01-04-01-01,@02 ..)"
    );
}

#[test]
fn mode_fields_in_the_episode_plot_are_checked_there() {
    let plot = "@01-04-01 x\nmode: 'リリア:Z\n;\n本文の予定\n";
    let project = world_with(EPISODE_PATH, plot);
    let found = tags(&project, EPISODE_PATH);
    assert!(found.contains("E003@'リリア:Z(リリア,Z)"), "{found}");
    // 同じ割り当ては複数の節から解決されるが、波線は 1 本（重複を落とす）
    assert_eq!(found.matches("E003").count(), 1, "{found}");
}

#[test]
fn e005_on_a_mode_field_is_reported_once() {
    // B-67: B 形式の `mode:` は plot_episode が、1 トークンの 3 つ以上は辞書が E005 を出すが、
    // 範囲はどちらもトークン全体なので 1 本にまとまる
    let plot = "@01-04-01 x\nmode: 'リリア:A+B+C\n;\n";
    let project = world_with(EPISODE_PATH, plot);
    let found = tags(&project, EPISODE_PATH);
    assert_eq!(found.matches("E005").count(), 1, "{found}");
}

#[test]
fn w006_needs_a_speaker_without_any_mode() {
    // リリアの既定モードを外し、話プロットにも全体プロットにも割り当てが無い話を作る
    let characters = CHARACTERS.replacen(
        "# リリア\nreading: りりあ\ndefault-mode: A\n",
        "# リリア\nreading: りりあ\n",
        1,
    );
    let mut project = world_with("characters.md", &characters);
    project.insert(novnow::core::model::document::Document::new(
        "contents/03-01.cnov",
        "@03-01-01 x\n「来たわよ」@リリア\n",
    ));
    project.insert(novnow::core::model::document::Document::new(
        "plots/03-01.cnov",
        "@03-01-01 x\nsummary: s\n",
    ));
    let found = tags(&project, "plots/03-01.cnov");
    // 話プロットに節が無い（`;` が無い）ので波線は場面全体。節数の W030 も出る（0 節 / 1 節）
    assert_eq!(
        found,
        "W006@@03-01-01 x\nsummary: s\n / W030@@03-01-01(0,1)"
    );
    // 本文には W006 を載せない（話プロットの節に載る。§6.4.3 の段 8）
    assert_eq!(tags(&project, "contents/03-01.cnov"), "");
}

#[test]
fn a_word_used_before_its_range_is_w022() {
    // §5.6 の例の「星屑の時計」は `range: @01 ..`。0 章（ストック）で使うと先出し
    let text = "@00-01-01 x\n'星屑の時計 の噂。\n";
    let project = world_with("contents/00-01.cnov", text);
    assert_eq!(
        tags(&project, "contents/00-01.cnov"),
        "W022@'星屑の時計(星屑の時計)"
    );
    // 1 章なら範囲内
    let inside = world_with(CONTENTS_PATH, "@01-04-01 x\n'星屑の時計 の噂。\n");
    assert_eq!(tags(&inside, CONTENTS_PATH), "");
}

#[test]
fn a_material_file_whose_heading_differs_from_the_index_is_w025() {
    let path = "materials/docs/M0137-kagaku-koshien.md";
    let differs = world_with(path, "# 科学の甲子園\n<!-- novnow:M0137 -->\n");
    assert_eq!(tags(&differs, path), "W025@科学の甲子園");
    // 正規化（§6.1.3）して同じなら出さない（全角英数・空白の違いは同じとみなす）
    let same = world_with(path, "# 科学の甲子園　本戦の運営と実技課題\n");
    assert_eq!(tags(&same, path), "");
}

#[test]
fn relations_report_e013_without_an_extra_e001() {
    let characters = CHARACTERS.replacen("'リリア: 幼馴染。", "'ゼノ: 宿敵。", 1);
    let project = world_with("characters.md", &characters);
    assert_eq!(tags(&project, "characters.md"), "E013@'ゼノ(ゼノ)");
}

#[test]
fn dictionary_build_diagnostics_land_on_the_losing_file() {
    // 用語とキャラに同じ名前 → W027 は負けた側（用語、§6.1.1 の優先順位）の定義に出る
    let words = format!("{WORDS}\n# リリア\ndefinition: 同名の花。\n");
    let project = world_with("words.md", &words);
    assert_eq!(tags(&project, "words.md"), "W027@リリア(リリア)");
    assert_eq!(tags(&project, "characters.md"), "");
}

#[test]
fn reserved_numbers_and_shallow_headers_use_the_new_codes() {
    // 第 2.8 版（A-9 / A-10）
    let project = world_with(CONTENTS_PATH, "@01-04-00 x\n本文\n@01-04 y\n");
    assert_eq!(
        tags(&project, CONTENTS_PATH),
        "E022@@01-04-00(01-04-00) / E020@@01-04(3-4)"
    );
}

#[test]
fn custom_prompts_check_the_role_name() {
    // §5.9.2: カスタムロールの名前は既定 7 役割と衝突してはいけない（E033）
    let project = world_with(
        "prompts/custom/writer.md",
        "# writer\n本文: {{text_current}}\n",
    );
    let found = tags(&project, "prompts/custom/writer.md");
    assert!(found.starts_with("E033@writer"), "{found}");
    // 既定役割のプロンプト（`prompts/writer.md`）では名前を検査しない（必須のセクションが無いので
    // E029 だけが出る。第 2.16 版、`docs/history.md` B-139）
    let builtin = world_with("prompts/writer.md", "# writer\n本文: {{text_current}}\n");
    assert_eq!(
        tags(&builtin, "prompts/writer.md"),
        "E029@# writer(system, task)"
    );
    let whole = world_with(
        "prompts/writer.md",
        "# writer\n\n## system\nあなたは本文担当です。\n\n## task\n本文: {{text_current}}\n",
    );
    assert_eq!(tags(&whole, "prompts/writer.md"), "");
}

#[test]
fn unknown_files_have_no_diagnostics() {
    let project = world_with("exports/out.txt", "'だれか");
    assert_eq!(tags(&project, "exports/out.txt"), "");
}

#[test]
fn features_diagnostics_overrides_follow_4_5_2() {
    use novnow::core::diagnostics::{DiagnosticCode, Severity};
    use novnow::core::model::document::Document;

    // 1 節目が空 → W012（structure グループ）
    let prose = "@01-04-01 場面\n;\n本文。\n";
    let w012 = |project: &Project| {
        diagnostics::document(&doc(project, CONTENTS_PATH), project)
            .into_iter()
            .find(|found| found.code == DiagnosticCode::W012)
    };
    let base = world_with(CONTENTS_PATH, prose);
    let found = w012(&base).expect("既定では出る");
    assert_eq!(found.severity(), Severity::Warning);
    assert_eq!(found.severity_override, None);

    let with = |toml: &str| {
        let mut project = world_with(CONTENTS_PATH, prose);
        project.insert(Document::new("project.toml", toml.to_owned()));
        project
    };
    // 個別コードの off は件数にも含めない
    let off = with("[features.diagnostics]\nW012 = \"off\"\n");
    assert!(w012(&off).is_none());
    // 個別コードの error は重大度を上げる
    let error = with("[features.diagnostics]\nW012 = \"error\"\n");
    assert_eq!(w012(&error).expect("出る").severity(), Severity::Error);
    // グループ（structure）の off
    let group = with("[features.diagnostics]\nstructure = \"off\"\n");
    assert!(w012(&group).is_none());
    // 関係ないグループ（writing）は効かない
    let other = with("[features.diagnostics]\nwriting = \"off\"\n");
    assert!(w012(&other).is_some());
    // default は最後に効く
    let default = with("[features.diagnostics]\ndefault = \"error\"\n");
    assert_eq!(w012(&default).expect("出る").severity(), Severity::Error);
    // 個別コードがグループより先（§4.5.2 の順）
    let code_first = with("[features.diagnostics]\nW012 = \"warn\"\nstructure = \"off\"\n");
    let found = w012(&code_first).expect("個別コードが勝つ");
    assert_eq!(found.severity(), Severity::Warning);
    // 値でない文字列は書かれていないものとして次へ進む
    let invalid = with("[features.diagnostics]\nW012 = \"loud\"\ndefault = \"error\"\n");
    assert_eq!(w012(&invalid).expect("出る").severity(), Severity::Error);
}

/// §7.2.2（W096）用: 本文を差し替え、任意で `[features.diagnostics]` を足す。
fn project_with_dialogue(texts: &[&str], settings: Option<&str>) -> Project {
    use novnow::core::model::document::Document;
    let mut project = world_with(CONTENTS_PATH, &texts.join(""));
    if let Some(extra) = settings {
        project.insert(Document::new(
            "project.toml",
            format!("schema_version = 2\n\n[features.diagnostics]\n{extra}"),
        ));
    }
    project
}

fn w096_args(found: &[Diagnostic]) -> Vec<String> {
    found
        .iter()
        .filter(|d| d.code == DiagnosticCode::W096)
        .map(|d| match &d.args[0] {
            Arg::Text(mark) => mark.clone(),
            Arg::Number(n) => n.to_string(),
        })
        .collect()
}

#[test]
fn w096_flags_the_minority_spacing_per_mark() {
    // ！ の後は空白ありが 5 件・なしが 1 件（合計 6、差 4）→ なしの 1 件だけ
    let project = project_with_dialogue(
        &["@01-01-01-01\n\
             「行くぞ！　待て」\n\
             「止まれ！　聞け」\n\
             「来い！　早く」\n\
             「逃げろ！　今だ」\n\
             「見ろ！　あれを」\n\
             「進め！続け」\n"],
        Some("W096 = \"warn\"\n"),
    );
    let document = doc(&project, CONTENTS_PATH);
    let found = diagnostics::document(&document, &project);
    assert_eq!(w096_args(&found), vec!["！".to_string()]);
    // 範囲は記号そのもの
    let w096 = found
        .iter()
        .find(|d| d.code == DiagnosticCode::W096)
        .unwrap();
    let range = w096.range.clone().unwrap();
    assert_eq!(&document.text[range], "！");

    // 記号ごとに別集計: 。 は空白なしが多数派なら、空白ありが少数派になる
    let project = project_with_dialogue(
        &["@01-01-01-01\n\
             「行くぞ！　待て」\n\
             「止まれ！　聞け」\n\
             「来い！　早く」\n\
             「逃げろ！　今だ」\n\
             「見ろ！　あれを」\n\
             「進め！続け」\n\
             「分かった。行く」\n\
             「そうだ。始める」\n\
             「違う。止まれ」\n\
             「いいよ。来て」\n\
             「駄目だ。帰る」\n\
             「本当だ。　信じて」\n"],
        Some("W096 = \"warn\"\n"),
    );
    let document = doc(&project, CONTENTS_PATH);
    let found = diagnostics::document(&document, &project);
    let mut marks = w096_args(&found);
    marks.sort();
    assert_eq!(marks, vec!["。".to_string(), "！".to_string()]);
    // 空白あり側の範囲には空白も含まれる（クイックフィックスで取るため）
    let spaced = found
        .iter()
        .find(|d| d.code == DiagnosticCode::W096 && d.args[0] == Arg::Text("。".into()))
        .unwrap();
    assert_eq!(&document.text[spaced.range.clone().unwrap()], "。　");
}

#[test]
fn w096_stays_silent_without_a_clear_majority_or_enablement() {
    let lines = &["@01-01-01-01\n\
         「行くぞ！　待て」\n\
         「止まれ！　聞け」\n\
         「来い！早く」\n\
         「逃げろ！今だ」\n"];
    // 件数不足（合計 4）かつ同数 → 出ない
    let project = project_with_dialogue(lines, Some("W096 = \"warn\"\n"));
    let document = doc(&project, CONTENTS_PATH);
    assert!(w096_args(&diagnostics::document(&document, &project)).is_empty());

    // [features.diagnostics] で有効化しなければ出ない（組み込み既定は無効）
    let project = project_with_dialogue(
        &["@01-01-01-01\n\
             「行くぞ！　待て」\n\
             「止まれ！　聞け」\n\
             「来い！　早く」\n\
             「逃げろ！　今だ」\n\
             「見ろ！　あれを」\n\
             「進め！続け」\n"],
        None,
    );
    let document = doc(&project, CONTENTS_PATH);
    assert!(w096_args(&diagnostics::document(&document, &project)).is_empty());
    // 明示操作（文体タブ）では計算できる
    let explicit = novnow::core::features::diagnose::document(&document, &project, true);
    assert_eq!(w096_args(&explicit).len(), 1);

    // 日本語主体でない本文では出ない（文字種で決める）
    let project = project_with_dialogue(
        &["@01-01-01-01\n\
             「Go now！　Wait here please」\n\
             「Stop！　Listen to me now」\n\
             「Come！　Hurry up please」\n\
             「Run！　This is the moment」\n\
             「Look！　Over there it is」\n\
             「Move！Keep going forward」\n"],
        Some("W096 = \"warn\"\n"),
    );
    let document = doc(&project, CONTENTS_PATH);
    assert!(w096_args(&diagnostics::document(&document, &project)).is_empty());
}
