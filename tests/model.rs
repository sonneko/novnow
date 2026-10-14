//! `core::model`（§3.3 の 10 型）のテスト。
//!
//! `PLANS.md` §9.2 B-29（型ごとに 1 ファイル、依存は `syntax → model` の 1 本）と
//! B-30（`Serialize` + `Deserialize`、未知キー温存、`deny_unknown_fields` を使わない）に対応する。
//! 型の中身は仕様書の表（§4.5 / §5.5 / §5.6 / §5.7 / §5.8.2 / §5.10.1 / §6.5.2 / §15.1.1）と
//! 突き合わせる。

use novnow::core::id::SceneId;
use novnow::core::model::{
    AgentLimits, Agents, Anchor, Character, Condition, ConditionKind, Delegation, Document,
    DocumentFormat, Export, Idea, IdeaStatus, Initiator, Lexicon, LocalLlm, Material, MaterialId,
    MaterialIndex, MaterialStatus, Mode, Newline, Project, Relation, RolePattern, SchemaState,
    Source, SourceKind, Structure, Task, TaskStatus, Word, Writing,
};

// ============================================================ 委譲レベル（§15.1.1）

#[test]
fn delegation_follows_the_table_in_15_1_1() {
    // レベル | エージェントの書き込み | 連鎖・次タスク
    assert!(!Delegation::Off.can_write());
    assert!(!Delegation::Off.can_chain());
    assert!(!Delegation::Propose.can_write());
    assert!(!Delegation::Propose.can_chain());
    assert!(Delegation::Apply.can_write());
    assert!(!Delegation::Apply.can_chain());
    assert!(Delegation::Autonomous.can_write());
    assert!(Delegation::Autonomous.can_chain());
    // 既定は propose（§4.5.1）
    assert_eq!(Delegation::default(), Delegation::Propose);
    assert!(!Delegation::Off.is_automatic());
    assert!(Delegation::Propose.is_automatic());
}

#[test]
fn delegation_reads_and_writes_the_documented_words() {
    for level in [
        Delegation::Off,
        Delegation::Propose,
        Delegation::Apply,
        Delegation::Autonomous,
    ] {
        assert_eq!(Delegation::parse(level.as_str()), Some(level));
        let text = serde_json::to_string(&level).unwrap();
        assert_eq!(text, format!("\"{}\"", level.as_str()));
        assert_eq!(serde_json::from_str::<Delegation>(&text).unwrap(), level);
    }
    assert_eq!(Delegation::parse(""), None);
    assert_eq!(Delegation::parse("Propose"), None);
}

// ============================================================ 役割語パターン（§6.5.2）

#[test]
fn anchors_follow_the_fixed_table_in_6_5_2() {
    // 形 | 例 | 照合
    let cases = [
        ("*だよ！", Anchor::SentenceEnd), // 先頭ワイルドカード・末尾リテラル → 文末一致
        ("ちょっ…*", Anchor::SentenceStart), // 先頭リテラル・末尾ワイルドカード → 文頭一致
        ("*ヨシ*", Anchor::Anywhere),     // 両端ワイルドカード → 位置自由
        ("です。", Anchor::SentenceEnd),  // ワイルドカード無し → 文末一致（既定）
        ("ほう…", Anchor::SentenceEnd),   // `…` はワイルドカードではない（§6.5.1）
        ("=ほう…", Anchor::Exact),        // `=` 前置 → 完全一致
        ("*で…", Anchor::SentenceEnd),
        ("〜だよ", Anchor::SentenceEnd), // `〜` もワイルドカード（§6.5.1）
        ("?", Anchor::SentenceEnd),      // `?` は任意の 1 文字でワイルドカードではない
    ];
    for (source, expected) in cases {
        let pattern = RolePattern::new(source);
        assert_eq!(pattern.anchor(), expected, "{source}");
    }
}

#[test]
fn an_exact_pattern_keeps_its_prefix_in_the_source() {
    let pattern = RolePattern::new("=ほう…");
    assert!(pattern.is_exact());
    assert_eq!(pattern.body(), "ほう…");
    assert_eq!(pattern.source, "=ほう…");
    // `=` が無ければ body は source と同じ
    assert_eq!(RolePattern::new("ほう…").body(), "ほう…");
}

#[test]
fn an_empty_pattern_is_a_sentence_end_match() {
    assert_eq!(RolePattern::new("").anchor(), Anchor::SentenceEnd);
    assert_eq!(RolePattern::new("=").anchor(), Anchor::Exact);
}

// ============================================================ モードとキャラ（§5.5）

#[test]
fn a_mode_without_a_range_covers_everything() {
    // §5.5.4「`range` 省略時は全範囲（`@00 ..`）」
    let mode = Mode {
        id: "A".to_owned(),
        ..Mode::default()
    };
    let expr = mode.range_expr().unwrap();
    assert!(expr.contains(SceneId::MIN));
    assert!(expr.contains(SceneId::MAX));
}

#[test]
fn a_mode_range_is_read_as_a_range_expression() {
    let mode = Mode {
        id: "C".to_owned(),
        range: Some("@01 .. @02".to_owned()),
        ..Mode::default()
    };
    let expr = mode.range_expr().unwrap();
    assert_eq!(expr.len(), 1);
    assert!(expr.contains(SceneId {
        chapter: 1,
        episode: 1,
        scene: 1,
        section: 1
    }));
    assert!(!expr.contains(SceneId {
        chapter: 3,
        episode: 0,
        scene: 0,
        section: 0
    }));
}

#[test]
fn a_broken_mode_range_does_not_panic() {
    let mode = Mode {
        id: "A".to_owned(),
        range: Some("01 ..".to_owned()),
        ..Mode::default()
    };
    assert!(mode.range_expr().is_none());
}

#[test]
fn a_character_keeps_unknown_keys() {
    // §5.5.6 の `address:` は ② なので ① では未知キーとして残る（縫い代 1・2）
    let text = "{\"name\":\"恩田恵\",\"alias\":[\"めぐみ\"],\"address\":\"unknown-form\"}";
    let found: Character = serde_json::from_str(text).unwrap();
    assert_eq!(found.name, "恩田恵");
    assert_eq!(found.alias, vec!["めぐみ".to_owned()]);
    assert_eq!(found.unknown["address"], serde_json::json!("unknown-form"));
    // 書き戻しても消えない
    let again: Character = serde_json::from_str(&serde_json::to_string(&found).unwrap()).unwrap();
    assert_eq!(again, found);
}

#[test]
fn a_character_reads_the_fields_of_5_5_3() {
    let text = "{\"name\":\"恵\",\"reading\":\"めぐみ\",\"role\":\"メインヒロイン\",\
                \"default-mode\":\"A\",\"color\":\"#8fb8ff\",\"tags\":[\"a\"],\
                \"icon\":\"assets/icons/megumi.png\"}";
    let found: Character = serde_json::from_str(text).unwrap();
    // `default-mode` はハイフンを含むので個別に rename する（B-30: `rename_all` は付けない）
    assert_eq!(found.default_mode.as_deref(), Some("A"));
    assert_eq!(found.color.as_deref(), Some("#8fb8ff"));
    assert_eq!(found.icon.as_deref(), Some("assets/icons/megumi.png"));
    assert!(found.unknown.is_empty());
    assert!(serde_json::to_string(&found)
        .unwrap()
        .contains("default-mode"));
}

#[test]
fn relations_hold_a_target_and_a_note() {
    let found: Relation =
        serde_json::from_str("{\"target\":\"葵あおい\",\"note\":\"魂のパートナー。\"}").unwrap();
    assert_eq!(found.target, "葵あおい");
    assert_eq!(found.note, "魂のパートナー。");
}

#[test]
fn a_word_reads_the_fields_of_5_6() {
    let text = "{\"term\":\"星屑の時計\",\"reading\":\"ほしくずのとけい\",\"category\":\"魔導具\",\
                \"alias\":[\"星屑時計\"],\"definition\":\"幻の魔導具。\",\"range\":\"@01 ..\"}";
    let found: Word = serde_json::from_str(text).unwrap();
    assert_eq!(found.term, "星屑の時計");
    assert_eq!(found.definition, "幻の魔導具。");
    assert!(found.range_expr().is_some());
    assert!(found.unknown.is_empty());
}

// ============================================================ アイデア（§5.7）

#[test]
fn an_idea_reads_its_number_and_status() {
    let text = "{\"id\":\"I0007\",\"title\":\"学院長の動機\",\"status\":\"inbox\",\
                \"tags\":[\"学院長\"],\"relates\":\"@02, 'アルス\",\
                \"promoted-to\":\"plots/02.txt#@02-03\"}";
    let found: Idea = serde_json::from_str(text).unwrap();
    assert_eq!(found.number(), Some(7));
    assert_eq!(found.status_kind(), Some(IdeaStatus::Inbox));
    assert!(!found.is_hidden());
    // `promoted-to` もハイフンを含むので個別に rename する
    assert_eq!(found.promoted_to, "plots/02.txt#@02-03");
}

#[test]
fn a_dropped_idea_is_hidden_by_default() {
    // §5.7「`status: dropped` のものは既定で非表示（フィルタで表示可）」
    let dropped = Idea {
        status: "dropped".to_owned(),
        ..Idea::default()
    };
    assert!(dropped.is_hidden());
    assert_eq!(dropped.status_kind(), Some(IdeaStatus::Dropped));
    assert!(IdeaStatus::Dropped.is_hidden_by_default());
    assert!(!IdeaStatus::Keep.is_hidden_by_default());
    // 未知の値は「隠す」側に倒さない（§4.3-4）
    let archived = Idea {
        status: "archived".to_owned(),
        ..Idea::default()
    };
    assert_eq!(archived.status_kind(), None);
    assert!(!archived.is_hidden());
}

#[test]
fn an_idea_id_needs_four_digits() {
    let mut found = Idea::default();
    for (id, expected) in [
        ("I0007", Some(7)),
        ("I10000", Some(10000)),
        ("I007", None),
        ("7", None),
        ("I000x", None),
        ("", None),
    ] {
        found.id = id.to_owned();
        assert_eq!(found.number(), expected, "{id}");
    }
}

// ============================================================ 資料（§5.8.2）

#[test]
fn a_material_id_reads_both_prefixes() {
    // §5.8.2「`M` と `S` は同一の連番空間を共有する（MUST）」＋縫い代 7
    assert_eq!(MaterialId::parse("M0137").unwrap().to_string(), "M0137");
    assert_eq!(MaterialId::parse("S0138").unwrap().to_string(), "S0138");
    assert_eq!(
        MaterialId::parse("M0137").unwrap(),
        MaterialId {
            prefix: 'M',
            num: 137
        }
    );
    assert!(MaterialId::parse("M137").is_none());
}

#[test]
fn a_material_checks_its_path() {
    for (path, expected) in [
        ("materials/docs/M0001-a.md", true),
        ("materials/inbox/S0042-search.md", true),
        ("contents/01-01.txt", false),
        ("../secret.md", false),
        ("materials/../secret.md", false),
        ("/etc/passwd", false),
    ] {
        let found = Material {
            path: path.to_owned(),
            ..Material::default()
        };
        assert_eq!(found.is_path_inside_materials(), expected, "{path}");
    }
}

#[test]
fn a_material_reads_added_by() {
    let by_human = Material {
        added_by: "human".to_owned(),
        ..Material::default()
    };
    assert!(by_human.is_added_by_human());
    assert_eq!(by_human.agent_role(), None);
    let by_agent = Material {
        added_by: "agent:researcher".to_owned(),
        ..Material::default()
    };
    assert!(!by_agent.is_added_by_human());
    assert_eq!(by_agent.agent_role(), Some("researcher"));
}

#[test]
fn source_and_condition_kinds_follow_the_spec() {
    let source: Source =
        serde_json::from_str("{\"kind\":\"url\",\"url\":\"https://example.org\"}").unwrap();
    assert_eq!(source.kind_value(), Some(SourceKind::Url));
    let source: Source = serde_json::from_str("{\"kind\":\"search\",\"query\":\"指輪\"}").unwrap();
    assert_eq!(source.kind_value(), Some(SourceKind::Search));
    assert_eq!(source.query.as_deref(), Some("指輪"));
    let condition: Condition =
        serde_json::from_str("{\"kind\":\"scope\",\"value\":\"@02-03 .. @02-08\"}").unwrap();
    assert_eq!(condition.kind_value(), Some(ConditionKind::Scope));
    let condition: Condition =
        serde_json::from_str("{\"kind\":\"tag\",\"value\":\"大会\"}").unwrap();
    assert_eq!(condition.kind_value(), Some(ConditionKind::Tag));
    let condition: Condition = serde_json::from_str("{\"kind\":\"\",\"value\":\"\"}").unwrap();
    assert_eq!(condition.kind_value(), None);
    assert_eq!(
        MaterialStatus::parse("proposed"),
        Some(MaterialStatus::Proposed)
    );
    assert_eq!(MaterialStatus::parse("archived"), None);
}

#[test]
fn the_index_hands_out_ids_without_reusing_them() {
    let mut index = MaterialIndex {
        next_id: 138,
        items: vec![Material {
            id: "M0137".to_owned(),
            ..Material::default()
        }],
        ..MaterialIndex::default()
    };
    assert!(index.is_next_id_valid());
    assert_eq!(index.max_number(), Some(137));
    assert_eq!(index.issue('M').to_string(), "M0138");
    // §5.8.2「`M0137` の次に検索結果を作れば `S0138` になる」
    assert_eq!(index.issue('S').to_string(), "S0139");
    assert_eq!(index.next_id, 140);
    assert_eq!(index.duplicate_ids().len(), 0);
}

#[test]
fn the_index_reports_a_stale_next_id() {
    let mut index = MaterialIndex {
        next_id: 1,
        items: vec![Material {
            id: "M0137".to_owned(),
            ..Material::default()
        }],
        ..MaterialIndex::default()
    };
    assert!(!index.is_next_id_valid());
    // 採番は `next_id` ではなく既存の最大値から続ける（再利用禁止、MUST）
    assert_eq!(index.issue('M').to_string(), "M0138");
}

// ============================================================ タスク（§5.10.1）

#[test]
fn a_task_reads_the_keys_of_5_10_1() {
    let text = "{\"id\":\"T0042\",\"role\":\"writer\",\"scope\":\"@01-04-02-03\",\
                \"intent\":\"初稿生成\",\"status\":\"ready\",\"initiator\":\"human\",\
                \"delegation\":\"propose\",\"prompt\":\"prompts/writer.md\",\
                \"context-hash\":\"sha256:1a2b\",\"result\":\"agents/results/T0042.md\",\
                \"server\":\"llm-main\",\"human-comment\":\"リリアの心変わりを台詞で説明しないこと\"}";
    let found: Task = serde_json::from_str(text).unwrap();
    assert_eq!(found.id, "T0042");
    assert_eq!(found.status_kind(), Some(TaskStatus::Ready));
    assert_eq!(found.initiator_kind(), Some(Initiator::Human));
    assert_eq!(found.delegation, Delegation::Propose);
    assert_eq!(found.context_hash, "sha256:1a2b");
    assert_eq!(
        found.human_comment,
        "リリアの心変わりを台詞で説明しないこと"
    );
    assert_eq!(
        found.scope_id(),
        Some(SceneId {
            chapter: 1,
            episode: 4,
            scene: 2,
            section: 3
        })
    );
    assert!(found.unknown.is_empty());
}

#[test]
fn task_statuses_follow_5_10_1() {
    for (text, expected) in [
        ("draft", TaskStatus::Draft),
        ("ready", TaskStatus::Ready),
        ("running", TaskStatus::Running),
        ("review", TaskStatus::Review),
        ("accepted", TaskStatus::Accepted),
        ("rejected", TaskStatus::Rejected),
        ("canceled", TaskStatus::Canceled),
        ("applied", TaskStatus::Applied),
    ] {
        assert_eq!(TaskStatus::parse(text), Some(expected), "{text}");
    }
    assert_eq!(TaskStatus::parse("done"), None);
    assert!(TaskStatus::Applied.is_final());
    assert!(!TaskStatus::Running.is_final());
    assert_eq!(Initiator::parse("agent"), Some(Initiator::Agent));
    assert_eq!(Initiator::parse("bot"), None);
}

#[test]
fn a_task_with_missing_keys_uses_defaults() {
    let found: Task = serde_json::from_str("{}").unwrap();
    assert_eq!(found.status_kind(), None);
    assert_eq!(found.delegation, Delegation::Propose);
    assert!(found.scope_id().is_none());
}

// ============================================================ 文書（§4.2）

#[test]
fn document_formats_follow_the_project_layout() {
    for (path, expected) in [
        ("contents/01-04.txt", DocumentFormat::Contents),
        ("contents/00-01.txt", DocumentFormat::Contents),
        ("plots/all.txt", DocumentFormat::PlotOutline),
        ("plots/01.txt", DocumentFormat::PlotOutline),
        ("plots/00.txt", DocumentFormat::PlotOutline),
        ("plots/01-04.txt", DocumentFormat::PlotEpisode),
        ("characters.md", DocumentFormat::Characters),
        ("words.md", DocumentFormat::Words),
        ("ideas.md", DocumentFormat::Ideas),
        ("materials/index.json", DocumentFormat::MaterialsIndex),
        ("materials/docs/M0137-a.md", DocumentFormat::MaterialDoc),
        (
            "materials/inbox/S0042-search.md",
            DocumentFormat::MaterialDoc,
        ),
        ("prompts/writer.md", DocumentFormat::Prompt),
        (
            "prompts/custom/weapon-advisor.md",
            DocumentFormat::CustomPrompt,
        ),
        ("prompts/_shared/style.md", DocumentFormat::SharedPrompt),
        ("agents/tasks/T0042.md", DocumentFormat::Task),
        ("agents/results/T0042.md", DocumentFormat::TaskResult),
        ("agents/proposals/P0003.md", DocumentFormat::Proposal),
        ("agents/threads/TH0007.md", DocumentFormat::Thread),
        ("agents/log/2026-10-09.jsonl", DocumentFormat::AgentLog),
        ("agents/mcp.json", DocumentFormat::McpConfig),
        ("project.json", DocumentFormat::ProjectFile),
        (".novnow/ui.json", DocumentFormat::UiState),
        ("exports/out.txt", DocumentFormat::Unknown),
        ("assets/icons/megumi.png", DocumentFormat::Unknown),
    ] {
        assert_eq!(DocumentFormat::of_path(path), expected, "{path}");
    }
}

#[test]
fn a_document_format_knows_its_lexical_format() {
    assert_eq!(
        DocumentFormat::Contents.text_format(),
        Some(novnow::core::text::lexer::TextFormat::Contents)
    );
    assert_eq!(
        DocumentFormat::PlotEpisode.text_format(),
        Some(novnow::core::text::lexer::TextFormat::PlotEpisode)
    );
    assert_eq!(
        DocumentFormat::Characters.text_format(),
        Some(novnow::core::text::lexer::TextFormat::Markdown)
    );
    assert_eq!(DocumentFormat::MaterialsIndex.text_format(), None);
    assert_eq!(DocumentFormat::Unknown.text_format(), None);
}

#[test]
fn a_document_is_built_from_a_path_and_a_text() {
    let found = Document::new("plots/01-04.txt", "@01-04-01 x\n");
    assert_eq!(found.format, DocumentFormat::PlotEpisode);
    assert_eq!(found.text, "@01-04-01 x\n");
}

// ============================================================ project.json（§4.5）

#[test]
fn the_defaults_match_the_schema_of_4_5() {
    let found = Project::default();
    assert_eq!(found.schema_version, 1);
    assert_eq!(found.title, "");
    assert_eq!(found.content_encoding, "utf-8");
    assert_eq!(found.newline_value(), Newline::Lf);
    assert_eq!(found.structure.scene_id_format, "A-B-C-D");
    assert_eq!(found.structure.chapter_digits, 2);
    assert!(found.structure.allow_chapter_zero);
    assert_eq!(found.writing.section_target_chars, 400);
    assert_eq!(found.writing.section_warn_chars, 1200);
    assert_eq!(found.writing.episode_target_chars, 6000);
    assert_eq!(found.writing.manuscript_page_chars, 400);
    assert_eq!(found.writing.max_consecutive_speeches, 3);
    assert_eq!(found.writing.dialogue_ratio_target, 0.3);
    assert_eq!(found.writing.long_sentence_chars, 120);
    assert_eq!(found.writing.particle_run_threshold, 4);
    assert!(found.lexicon.word_classes.is_empty());
    assert_eq!(found.export.paragraph_break, "blank-line");
    assert!(found.export.keep_ruby);
    assert!(!found.export.include_blurb);
    assert_eq!(found.export.newline, Newline::Crlf);
    assert!(found.agents.enabled);
    assert_eq!(found.agents.delegation_level(), Delegation::Propose);
    assert_eq!(found.agents.max_chars_per_request, 12000);
    assert!(found.agents.integrator_is_human());
    assert_eq!(found.agents.max_parallel, 2);
    assert!(found.agents.custom_roles.is_empty());
    assert_eq!(found.agents.limits.max_tasks_per_session, 100);
    assert_eq!(found.agents.limits.max_chain_hops, 10);
    assert_eq!(found.agents.limits.max_input_chars_per_session, 2_000_000);
    assert!(!found.agents.local_llm.enabled);
    assert_eq!(found.agents.local_llm.url, "http://localhost:11434");
}

#[test]
fn the_spec_example_of_4_5_parses_into_the_defaults() {
    // §4.5 の例は「既定値が全部書かれた形」なので、パースしても既定と同じになる
    // （`reader` だけは §4.5.1「v1 本体は読まない。未知キーとして温存する」）
    let found: Project = serde_json::from_str(SPEC_PROJECT).unwrap();
    assert_eq!(found.schema_state(), SchemaState::Current);
    assert_eq!(found.title, "星屑の時計");
    assert!(found.unknown.contains_key("reader"));
    assert_eq!(found.unknown["reader"]["columns"], serde_json::json!(18));
    assert_eq!(found.agents.delegation_level(), Delegation::Propose);
    assert_eq!(found.agents.max_parallel, 2);
    // 書き戻しても `reader` は消えない（§4.5 MUST「未知キーは保存時に温存」）
    let text = serde_json::to_string(&found).unwrap();
    assert!(text.contains("reader"));
    let again: Project = serde_json::from_str(&text).unwrap();
    assert_eq!(again, found);
}

#[test]
fn schema_version_follows_4_3_2() {
    let mut found = Project::default();
    assert_eq!(found.schema_state(), SchemaState::Current);
    found.schema_version = 2;
    assert_eq!(found.schema_state(), SchemaState::TooNew { found: 2 });
    // 省略時は 1 として扱う（§4.3-2、第 2.17 版）
    let omitted: Project = serde_json::from_str("{}").unwrap();
    assert_eq!(omitted.schema_version, 1);
    assert_eq!(omitted.schema_state(), SchemaState::Current);
}

#[test]
fn a_broken_delegation_falls_back_to_the_default() {
    // §4.3-4「パース失敗を起動失敗にしない」。未知の値でもファイルは読める
    let found: Project =
        serde_json::from_str("{\"agents\":{\"delegation\":\"sometimes\"}}").unwrap();
    assert_eq!(found.agents.delegation, "sometimes");
    assert_eq!(found.agents.delegation_level(), Delegation::Propose);
}

#[test]
fn a_role_overrides_the_project_default() {
    // §15.1.1「設定は `agents.delegation`（全体既定）と `agents.roles.<role>`（上書き）」
    let found: Project = serde_json::from_str(
        "{\"agents\":{\"delegation\":\"propose\",\"roles\":{\"writer\":\"off\",\"ideator\":\"apply\"}}}",
    )
    .unwrap();
    assert_eq!(found.agents.delegation_of("writer"), Delegation::Off);
    assert_eq!(found.agents.delegation_of("ideator"), Delegation::Apply);
    assert_eq!(found.agents.delegation_of("plotter"), Delegation::Propose);
    // README の例「全く任せない」
    let found: Project = serde_json::from_str("{\"agents\":{\"delegation\":\"off\"}}").unwrap();
    assert_eq!(found.agents.delegation_of("writer"), Delegation::Off);
}

#[test]
fn custom_roles_have_a_limit() {
    // §5.9.2「1 プロジェクトあたり 16 ロール。超過は E034」
    let mut found = Agents::default();
    assert!(!found.exceeds_custom_roles());
    found.custom_roles = (0..16).map(|index| format!("role{index}")).collect();
    assert!(!found.exceeds_custom_roles());
    found.custom_roles.push("one-more".to_owned());
    assert!(found.exceeds_custom_roles());
}

#[test]
fn an_integrator_can_be_an_agent() {
    // §4.5.1「`agents.integrator`: `"human"` | `"agent"`」
    let found: Project = serde_json::from_str("{\"agents\":{\"integrator\":\"agent\"}}").unwrap();
    assert!(!found.agents.integrator_is_human());
    assert_eq!(found.agents.integrator, "agent");
}

#[test]
fn missing_keys_fall_back_to_the_documented_defaults() {
    let found: Project = serde_json::from_str("{\"title\":\"x\"}").unwrap();
    assert_eq!(found.title, "x");
    assert_eq!(found.writing, Writing::default());
    assert_eq!(found.structure, Structure::default());
    assert_eq!(found.export, Export::default());
    assert_eq!(found.lexicon, Lexicon::default());
    assert_eq!(found.agents.limits, AgentLimits::default());
    assert_eq!(found.agents.local_llm, LocalLlm::default());
}

#[test]
fn newline_reads_and_writes_the_two_words() {
    assert_eq!(Newline::parse("lf"), Some(Newline::Lf));
    assert_eq!(Newline::parse("crlf"), Some(Newline::Crlf));
    assert_eq!(Newline::parse("cr"), None);
    assert_eq!(Newline::Lf.text(), "\n");
    assert_eq!(Newline::Crlf.text(), "\r\n");
    assert_eq!(Newline::default(), Newline::Lf);
}

/// §4.5 の例（`project.json` スキーマ v1）。**仕様のコード例をそのまま**使う。
const SPEC_PROJECT: &str = r#"{
  "schema_version": 1,
  "title": "星屑の時計",
  "author": "",
  "content_encoding": "utf-8",
  "newline": "lf",
  "structure": {
    "scene_id_format": "A-B-C-D",
    "chapter_digits": 2,
    "episode_digits": 2,
    "scene_digits": 2,
    "section_digits": 2,
    "allow_chapter_zero": true
  },
  "writing": {
    "section_target_chars": 400,
    "section_warn_chars": 1200,
    "episode_target_chars": 6000,
    "manuscript_page_chars": 400,
    "max_consecutive_speeches": 3,
    "dialogue_ratio_target": 0.3,
    "long_sentence_chars": 120,
    "particle_run_threshold": 4
  },
  "lexicon": {
    "word_classes": {
      "体言": "[\\p{Han}\\p{Katakana}\\p{Hiragana}A-Za-z0-9ー]{1,24}"
    }
  },
  "export": {
    "paragraph_break": "blank-line",
    "indent_first_line": false,
    "keep_ruby": true,
    "insert_headings": false,
    "include_blurb": false,
    "newline": "crlf"
  },
  "reader": { "columns": 18, "chars_per_column": 42, "ruby": true },
  "agents": {
    "enabled": true,
    "delegation": "propose",
    "roles": {},
    "limits": { "max_tasks_per_session": 100, "max_chain_hops": 10, "max_input_chars_per_session": 2000000 },
    "max_chars_per_request": 12000,
    "integrator": "human",
    "max_parallel": 2,
    "custom_roles": [],
    "local_llm": { "enabled": false, "url": "http://localhost:11434", "model": "" }
  }
}
"#;
