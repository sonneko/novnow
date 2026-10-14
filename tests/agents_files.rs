//! `agents/` のファイル形式（§5.10）・`agents/mcp.json`（§5.11）・`prompts/`（§15.5）・LLM の出力の
//! 読み方と行差分（§15.5.2・§15.7.3）のテスト。

use std::collections::BTreeMap;

use novnow::core::agents::files::{self, LogEvent, Message, Proposal, RunResult, Thread, PENDING};
use novnow::core::agents::output::{self, DiffLine, Violation};
use novnow::core::agents::{cap_output, mcp_config, prompts, Kind, OUTPUT_LIMIT_CHARS};
use novnow::core::diagnostics::{self, DiagnosticCode};
use novnow::core::features::Project;
use novnow::core::model::delegation::Delegation;
use novnow::core::model::document::Document;
use novnow::core::model::project::Project as Settings;
use novnow::core::model::task::Task;
use novnow::core::resolve::context_rules::PacketBlock;
use novnow::core::template::DEFAULT_PROMPTS;
use serde_json::Value;

#[test]
fn a_task_round_trips_in_the_spec_order_and_keeps_unknown_keys() {
    let mut unknown = BTreeMap::new();
    unknown.insert("owner".to_owned(), Value::String("me".to_owned()));
    let task = Task {
        id: "T0042".to_owned(),
        role: "writer".to_owned(),
        scope: "@01-04-02-03".to_owned(),
        intent: "初稿生成".to_owned(),
        status: "ready".to_owned(),
        initiator: "human".to_owned(),
        delegation: Delegation::Apply,
        prompt: "prompts/writer.md".to_owned(),
        created_at: "2026-10-09T12:00:00+09:00".to_owned(),
        updated_at: "2026-10-09T12:03:11+09:00".to_owned(),
        context_hash: "sha256:1a2b".to_owned(),
        result: "agents/results/T0042.md".to_owned(),
        server: "llm-main".to_owned(),
        human_comment: "リリアの心変わりを\n台詞で説明しないこと".to_owned(),
        exclude: "materials, history".to_owned(),
        unknown,
    };
    let text = files::write_task(&task);
    assert_eq!(
        text,
        "# T0042\nrole: writer\nscope: @01-04-02-03\nintent: 初稿生成\nstatus: ready\ninitiator: human\ndelegation: apply\nprompt: prompts/writer.md\ncreated_at: 2026-10-09T12:00:00+09:00\nupdated_at: 2026-10-09T12:03:11+09:00\ncontext-hash: sha256:1a2b\nresult: agents/results/T0042.md\nserver: llm-main\nhuman-comment: リリアの心変わりを 台詞で説明しないこと\nexclude: materials, history\nowner: me\n"
    );
    let parsed = files::parse_task("agents/tasks/T0042.md", &text).unwrap();
    assert_eq!(
        parsed.human_comment,
        "リリアの心変わりを 台詞で説明しないこと"
    );
    assert_eq!(
        Task {
            human_comment: parsed.human_comment.clone(),
            ..task
        },
        parsed
    );
    assert_eq!(
        files::excluded_blocks(&parsed),
        vec![PacketBlock::Materials, PacketBlock::History]
    );
    assert_eq!(
        files::exclude_value(&[
            PacketBlock::History,
            PacketBlock::Cast,
            PacketBlock::History
        ]),
        "cast, history"
    );
    // project / constraints は外せない
    let mut mandatory = parsed.clone();
    mandatory.exclude = "project, constraints, lexicon".to_owned();
    assert_eq!(
        files::excluded_blocks(&mandatory),
        vec![PacketBlock::Lexicon]
    );
    assert_eq!(files::task_scene(&parsed).unwrap().section, 3);
    mandatory.scope = "@01-04-02".to_owned();
    assert_eq!(files::task_scene(&mandatory), None);
}

#[test]
fn a_result_keeps_the_output_as_written_even_with_headings_inside() {
    let result = RunResult {
        task: "T0042".to_owned(),
        finished_at: "2026-10-09T12:03:40+09:00".to_owned(),
        model: "model-x".to_owned(),
        input_chars: 7412,
        output_chars: 612,
        duration_ms: 4210,
        truncated: true,
        output: "本文\n\n## proposal\nこれは出力の一部\n\n## question\n誰？".to_owned(),
        proposal: "agents/proposals/P0003.md".to_owned(),
    };
    let text = files::write_result(&result);
    assert!(text.starts_with("# T0042 result\nfinished_at: 2026-10-09T12:03:40+09:00\nmodel: model-x\ninput_chars: 7412\noutput_chars: 612\nduration_ms: 4210\ntruncated: true\n\n## output\n本文\n"), "{text}");
    assert!(
        text.ends_with("\n\n## proposal\nagents/proposals/P0003.md\n"),
        "{text}"
    );
    assert_eq!(files::parse_result(&text).unwrap(), result);
    let none = RunResult {
        proposal: String::new(),
        truncated: false,
        ..result
    };
    assert_eq!(
        files::parse_result(&files::write_result(&none)).unwrap(),
        none
    );
}

#[test]
fn a_proposal_round_trips_with_empty_before_and_markdown_after() {
    let proposal = Proposal {
        id: "P0003".to_owned(),
        kind: "section-text".to_owned(),
        target: "contents/01-04.txt @01-04-02-03".to_owned(),
        origin: "T0042".to_owned(),
        status: "open".to_owned(),
        created_at: "2026-10-09T12:03:40+09:00".to_owned(),
        updated_at: String::new(),
        comment: String::new(),
        note: String::new(),
        partial: false,
        before: String::new(),
        after: "一行目\n\n## before\n三行目".to_owned(),
    };
    let text = files::write_proposal(&proposal);
    assert_eq!(
        text,
        "# P0003\nkind: section-text\ntarget: contents/01-04.txt @01-04-02-03\norigin: T0042\nstatus: open\ncreated_at: 2026-10-09T12:03:40+09:00\n\n## before\n\n\n## after\n一行目\n\n## before\n三行目\n"
    );
    assert_eq!(files::parse_proposal(&text).unwrap(), proposal);
    let (path, scene) = proposal.target_parts();
    assert_eq!(path, "contents/01-04.txt");
    assert_eq!(scene.unwrap().section, 3);
    let settled = Proposal {
        status: "rejected".to_owned(),
        updated_at: "2026-10-10T00:00:00+09:00".to_owned(),
        comment: "台詞で説明している".to_owned(),
        partial: true,
        before: "元の\n本文".to_owned(),
        ..proposal
    };
    assert_eq!(
        files::parse_proposal(&files::write_proposal(&settled)).unwrap(),
        settled
    );
}

#[test]
fn a_thread_round_trips_with_multi_line_bodies() {
    let thread = Thread {
        id: "TH0007".to_owned(),
        title: "旧校舎の間取り".to_owned(),
        scope: "@01-04-02".to_owned(),
        status: "open".to_owned(),
        messages: vec![
            Message {
                number: 1,
                from: "writer".to_owned(),
                to: "researcher".to_owned(),
                kind: "request".to_owned(),
                created_at: "2026-10-09T12:00:00+09:00".to_owned(),
                approved_by: PENDING.to_owned(),
                refs: String::new(),
                body: "旧校舎 2 階の窓の形式が分かる資料が欲しい。\n\n時代設定は 1930 年代相当。"
                    .to_owned(),
            },
            Message {
                number: 2,
                from: "researcher".to_owned(),
                to: "writer".to_owned(),
                kind: "deliver".to_owned(),
                created_at: "2026-10-09T13:00:00+09:00".to_owned(),
                approved_by: "human".to_owned(),
                refs: "'M0141".to_owned(),
                body: "上下げ窓が主流。".to_owned(),
            },
        ],
    };
    let text = files::write_thread(&thread);
    assert!(text.starts_with("# TH0007 旧校舎の間取り\nscope: @01-04-02\nstatus: open\n\n## msg 001\nfrom: writer\nto: researcher\nkind: request\ncreated_at: 2026-10-09T12:00:00+09:00\napproved_by: pending\nbody:\n  旧校舎 2 階の窓の形式が分かる資料が欲しい。\n\n  時代設定は 1930 年代相当。\n"), "{text}");
    assert_eq!(files::parse_thread(&text).unwrap(), thread);
    assert!(!thread.messages[0].delivered());
    assert!(thread.messages[1].delivered());
}

#[test]
fn ids_are_the_maximum_plus_one_and_results_count_for_tasks() {
    let paths = [
        "agents/tasks/T0003.md",
        "agents/results/T0009.dryrun.md",
        "agents/results/T0007.md",
        "agents/proposals/P0012.md",
        "agents/threads/TH0002.md",
        "agents/tasks/notes.md",
        "agents/tasks/T12x.md",
    ];
    assert_eq!(files::next_task_id(paths.iter().copied()), "T0010");
    assert_eq!(files::next_proposal_id(paths.iter().copied()), "P0013");
    assert_eq!(files::next_thread_id(paths.iter().copied()), "TH0003");
    assert_eq!(files::next_task_id(std::iter::empty()), "T0001");
    assert_eq!(
        files::dryrun_path("T0001"),
        "agents/results/T0001.dryrun.md"
    );
}

#[test]
fn log_events_are_one_json_line_in_the_spec_key_order_without_bodies() {
    let mut event = LogEvent::new("2026-10-09T12:03:40+09:00", "run");
    event.task = Some("T0042".to_owned());
    event.role = Some("writer".to_owned());
    event.input_chars = Some(7412);
    let line = event.line();
    assert_eq!(
        line,
        "{\"ts\":\"2026-10-09T12:03:40+09:00\",\"event\":\"run\",\"task\":\"T0042\",\"role\":\"writer\",\"server\":null,\"input_chars\":7412,\"output_chars\":null,\"duration_ms\":null,\"result_path\":null,\"error\":null}\n"
    );
    assert_eq!(event.path(), "agents/log/2026-10-09.jsonl");
    event.detail = Some("writer: propose -> apply".to_owned());
    assert!(event
        .line()
        .ends_with(",\"detail\":\"writer: propose -> apply\"}\n"));
    assert_eq!(files::EVENTS.len(), 9);
}

const MCP: &str = r#"{
  "schema_version": 1,
  "servers": {
    "zeta": {
      "transport": "stdio",
      "command": "zeta-mcp",
      "env": { "API_KEY": "${NOVNOW_LLM_KEY}", "MODE": "${A}" },
      "capabilities": ["sampling"],
      "roles": ["writer", "plotter"],
      "timeout_ms": 1000
    },
    "alpha": {
      "command": "alpha-mcp",
      "args": ["--x"],
      "env": { "TOKEN": "sk-literal" },
      "capabilities": ["tools"],
      "allow_tools": ["generate", "search"],
      "roles": ["*"]
    },
    "web": { "transport": "http", "command": "", "roles": ["researcher"] }
  }
}"#;

#[test]
fn mcp_servers_keep_the_definition_order_and_roles_pick_the_first_match() {
    let config = mcp_config::parse(MCP).unwrap();
    let names: Vec<&str> = config
        .servers
        .iter()
        .map(|found| found.name.as_str())
        .collect();
    assert_eq!(names, ["zeta", "alpha", "web"]);
    assert_eq!(config.server_for("writer").unwrap().name, "zeta");
    assert_eq!(config.server_for("stylist").unwrap().name, "alpha");
    // http は v1 では使わない（stdio だけ）
    assert_eq!(config.server_for("researcher").unwrap().name, "alpha");
    let zeta = config.server("zeta").unwrap();
    assert_eq!(zeta.env_refs(), ["NOVNOW_LLM_KEY", "A"]);
    assert!(zeta.literal_env().is_empty());
    assert!(zeta.samples());
    assert_eq!(zeta.timeout_ms, Some(1000));
    let alpha = config.server("alpha").unwrap();
    assert_eq!(alpha.literal_env(), ["TOKEN"]);
    assert!(!alpha.samples());
    let resolved = zeta
        .resolve_env(|name| match name {
            "NOVNOW_LLM_KEY" => Some("k".to_owned()),
            "A" => Some("1".to_owned()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        resolved,
        [
            ("API_KEY".to_owned(), "k".to_owned()),
            ("MODE".to_owned(), "1".to_owned())
        ]
    );
    let missing = zeta.resolve_env(|_| None).unwrap_err();
    assert_eq!(missing, ["NOVNOW_LLM_KEY", "A"]);
    // 参照だけの値でないものは W033（`${A}-${B}` も「参照のみ」ではない）
    assert!(mcp_config::is_reference("${NOVNOW_LLM_KEY}"));
    assert!(!mcp_config::is_reference("${A}-${B}"));
    assert!(!mcp_config::is_reference("sk-123"));
    assert_eq!(mcp_config::unresolved(&missing).code, DiagnosticCode::E030);
}

#[test]
fn literal_secrets_in_mcp_json_are_w033_at_the_key() {
    let found = mcp_config::diagnostics(MCP);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, DiagnosticCode::W033);
    let range = found[0].range.clone().unwrap();
    assert_eq!(&MCP[range], "\"TOKEN\"");
    // 文書の診断にも出る
    let project =
        Project::with_documents(Settings::default(), [Document::new("agents/mcp.json", MCP)]);
    let document = project.document("agents/mcp.json").unwrap();
    assert!(diagnostics::document(document, &project)
        .iter()
        .any(|found| found.code == DiagnosticCode::W033));
    assert!(mcp_config::diagnostics(novnow::core::template::MCP_CONFIG).is_empty());
}

#[test]
fn missing_prompts_are_listed_and_only_an_absent_directory_is_i050() {
    let empty = Project::with_documents(Settings::default(), []);
    let (missing, absent) = prompts::missing(&empty);
    assert_eq!(missing.len(), DEFAULT_PROMPTS.len());
    assert_eq!(DEFAULT_PROMPTS.len(), 9);
    assert!(absent);
    let partial = Project::with_documents(
        Settings::default(),
        [Document::new(
            "prompts/writer.md",
            "# writer\n## system\nx\n## task\ny\n",
        )],
    );
    let (missing, absent) = prompts::missing(&partial);
    assert_eq!(missing.len(), 8);
    assert!(!missing.iter().any(|(path, _)| *path == "prompts/writer.md"));
    assert!(!absent);
    assert!(prompts::edited(&partial, "prompts/writer.md"));
    assert!(!prompts::edited(&partial, "prompts/plotter.md"));
    assert_eq!(prompts::reset_targets("all").len(), 9);
    assert_eq!(
        prompts::reset_targets("writer"),
        [(
            "prompts/writer.md",
            novnow::core::template::default_prompt("prompts/writer.md").unwrap()
        )]
    );
    assert_eq!(
        prompts::reset_targets("style")[0].0,
        "prompts/_shared/style.md"
    );
}

#[test]
fn a_prompt_without_system_or_task_is_e029_and_blocks_only_that_role() {
    let broken = "# writer\nmodel-hint: fast\n\n## output\n- 本文だけ\n";
    let found = prompts::diagnostics(broken);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, DiagnosticCode::E029);
    assert_eq!(prompts::missing_sections(broken), ["system", "task"]);
    for (path, text) in DEFAULT_PROMPTS {
        if !path.contains("_shared") {
            assert!(prompts::diagnostics(text).is_empty(), "{path}");
        }
    }
    let project = Project::with_documents(
        Settings::default(),
        [
            Document::new("prompts/writer.md", broken),
            Document::new(
                "prompts/plotter.md",
                novnow::core::template::default_prompt("prompts/plotter.md").unwrap(),
            ),
        ],
    );
    assert_eq!(
        prompts::blockers(&project, "writer")[0].code,
        DiagnosticCode::E029
    );
    assert!(prompts::blockers(&project, "plotter").is_empty());
    let document = project.document("prompts/writer.md").unwrap();
    assert!(diagnostics::document(document, &project)
        .iter()
        .any(|found| found.code == DiagnosticCode::E029));
}

#[test]
fn rendering_expands_variables_drops_comments_and_records_unresolved_ones() {
    let text = "# writer\nmax-chars: 9000\ntemperature: 0.4\nmodel-hint: careful\n\n## system\n// 人間向けのメモ\n題は {{project_title}}。\n\n## task\n節 {{scene_id}}。{{check}}\n\n## refuse\n断る。\n";
    let rendered = prompts::render(text, &|name| match name {
        "project_title" => Some("星屑".to_owned()),
        "scene_id" => Some("01-04-02-03".to_owned()),
        _ => None,
    });
    assert_eq!(rendered.system, "題は 星屑。\n\n断る。");
    assert_eq!(rendered.task, "節 01-04-02-03。");
    assert_eq!(rendered.unresolved, ["check"]);
    assert_eq!(rendered.max_chars, Some(9000));
    assert_eq!(rendered.temperature, Some(0.4));
    assert_eq!(rendered.model_hint, "careful");
    assert_eq!(
        prompts::shared_body("# style\n// メモ\n\n- 三人称\n- 過去形\n"),
        "- 三人称\n- 過去形"
    );
}

#[test]
fn the_output_is_split_into_the_body_questions_messages_and_tasks() {
    let parsed = output::parse(
        "\n本文の一行目。\n本文の二行目。\n\n## question\n窓はいくつ？\n\n## message\nto: researcher\nkind: request\nrefs: 'M0141\n窓の資料が欲しい。\n\n## task\nrole: writer\nscope: @01-04-02-04\nintent: 続きを書く\n\n## message\nto: \n空の宛先は捨てる\n",
    );
    assert_eq!(parsed.main, "本文の一行目。\n本文の二行目。");
    assert_eq!(parsed.questions, ["窓はいくつ？"]);
    assert_eq!(parsed.messages.len(), 1);
    assert_eq!(parsed.messages[0].to, "researcher");
    assert_eq!(parsed.messages[0].kind, "request");
    assert_eq!(parsed.messages[0].refs, "'M0141");
    assert_eq!(parsed.messages[0].body, "窓の資料が欲しい。");
    assert_eq!(parsed.tasks.len(), 1);
    assert_eq!(parsed.tasks[0].scope, "@01-04-02-04");
    assert_eq!(parsed.tasks[0].intent, "続きを書く");
}

#[test]
fn contract_violations_are_reported_but_never_fixed() {
    let check = |kind, text: &str, target| output::violations(kind, &output::parse(text), target);
    assert_eq!(
        check(Kind::SectionText, "以下が本文です：\n本文", None),
        [Violation::Preamble]
    );
    assert_eq!(
        check(Kind::SectionText, "## 見出し\n本文", None),
        [Violation::Heading]
    );
    assert_eq!(
        check(Kind::SectionText, "```\n本文\n```", None),
        [Violation::CodeFence]
    );
    assert_eq!(
        check(Kind::SectionText, "@01-04\n本文", None),
        [Violation::Annotation]
    );
    assert_eq!(
        check(Kind::SectionText, &"あ".repeat(100), Some(500)),
        [Violation::Length]
    );
    assert!(check(Kind::SectionText, &"あ".repeat(450), Some(500)).is_empty());
    assert_eq!(
        check(Kind::SectionText, "## question\n誰？", None),
        [Violation::Empty]
    );
    assert!(check(Kind::Note, "## 指摘\n- 一つ目", None).is_empty());
    assert!(check(Kind::Material, "# 資料\n本文", None).is_empty());
}

#[test]
fn the_line_diff_and_partial_accept_pick_only_the_chosen_changes() {
    let before = "一\n二\n三";
    let after = "一\n弐\n三\n四";
    let lines = output::diff(before, after);
    assert_eq!(
        lines,
        [
            DiffLine::Same("一".to_owned()),
            DiffLine::Removed("二".to_owned()),
            DiffLine::Added("弐".to_owned()),
            DiffLine::Same("三".to_owned()),
            DiffLine::Added("四".to_owned()),
        ]
    );
    let all: Vec<bool> = lines.iter().map(DiffLine::changed).collect();
    assert_eq!(output::merge(&lines, &all), after);
    assert_eq!(output::merge(&lines, &[false; 5]), before);
    // 「弐」に置き換えるが「四」は足さない
    assert_eq!(
        output::merge(&lines, &[false, true, true, false, false]),
        "一\n弐\n三"
    );
    assert_eq!(output::diff("", "a"), [DiffLine::Added("a".to_owned())]);
}

#[test]
fn outputs_are_capped_at_twenty_thousand_chars() {
    let long = "字".repeat(OUTPUT_LIMIT_CHARS + 5);
    let (kept, truncated) = cap_output(&long);
    assert!(truncated);
    assert_eq!(kept.chars().count(), OUTPUT_LIMIT_CHARS);
    let (kept, truncated) = cap_output("短い");
    assert!(!truncated);
    assert_eq!(kept, "短い");
}
