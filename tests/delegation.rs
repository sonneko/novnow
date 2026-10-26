//! 委譲レベル（§15.1.1）と実行の前後（§15.3・§15.7・§15.9）・MCP のツール（§15.8.3）・並列の順番
//! （§15.7.5 U-8）のテスト（`docs/history.md` のの完了条件「`tests/delegation.rs` が通る」）。
//!
//! I/O を持たない core の関数だけで確かめる（UI と adapters はそれぞれのテストで）。

mod world;

use novnow::core::agents::files::{self, Board, LogEvent};
use novnow::core::agents::runner::{self, Blocker, Draft, Limit, Reply, Session};
use novnow::core::agents::server;
use novnow::core::agents::{apply, Notice, Plan};
use novnow::core::diagnostics::DiagnosticCode;
use novnow::core::features::{sections, Project};
use novnow::core::id::SceneId;
use novnow::core::model::delegation::Delegation;
use novnow::core::model::document::Document;
use novnow::core::template::DEFAULT_PROMPTS;
use serde_json::{json, Value};

const NOW: &str = "2026-10-14T09:00:00+09:00";
const LATER: &str = "2026-10-14T09:05:00+09:00";

const MCP: &str = r#"{
  "schema_version": 1,
  "servers": {
    "llm-main": {
      "transport": "stdio",
      "command": "my-llm-mcp",
      "env": { "API_KEY": "${NOVNOW_LLM_KEY}" },
      "capabilities": ["sampling"],
      "roles": ["*"]
    }
  }
}"#;

fn id(chapter: u16, episode: u16, scene: u16, section: u16) -> SceneId {
    SceneId::new(chapter, episode, scene, section).unwrap()
}

fn env(name: &str) -> Option<String> {
    (name == "NOVNOW_LLM_KEY").then(|| "secret".to_owned())
}

fn no_env(_: &str) -> Option<String> {
    None
}

/// 仕様書の例 + 既定のプロンプト + サーバ 1 つ。委譲レベルは `roles` で与える。
fn project(roles: &[(&str, &str)], integrator: &str) -> Project {
    let mut project = world::world();
    for (path, text) in DEFAULT_PROMPTS {
        project.insert(Document::new(*path, *text));
    }
    project.insert(Document::new("agents/mcp.json", MCP));
    // 仕様書の例の全体プロットは第 1 章を確定済みにしている。節 4（話プロットの state: fixed）だけを確定済みにする
    project.insert(Document::new(
        world::ALL_PATH,
        world::ALL.replacen("state: fixed", "state: draft", 1),
    ));
    let mut settings = project.settings().clone();
    for (role, level) in roles {
        settings
            .agents
            .roles
            .insert((*role).to_owned(), (*level).to_owned());
    }
    settings.agents.integrator = integrator.to_owned();
    project.set_settings(settings);
    project
}

/// 計画の書き込みをプロジェクトに入れる（UI と `novnow mcp` がすることと同じ）。
fn run(project: &mut Project, plan: &Plan) {
    for change in &plan.writes {
        project.insert(Document::new(change.path.clone(), change.text.clone()));
    }
}

fn create(project: &mut Project, role: &str, scope: &str) -> String {
    let plan = runner::new_tasks(
        project,
        &Draft {
            role,
            scope,
            intent: "テスト",
            comment: "",
            examples: "",
            initiator: "human",
            status: "draft",
        },
        NOW,
    );
    run(project, &plan);
    let path = &plan.writes.last().expect("a task").path;
    path.trim_start_matches("agents/tasks/")
        .trim_end_matches(".md")
        .to_owned()
}

fn start(project: &mut Project, task: &str) -> runner::Start {
    let start = runner::prepare(project, task, NOW, "星屑", &env).expect("prepare");
    run(project, &runner::started(&start));
    start
}

fn reply(text: &str) -> Reply {
    Reply {
        text: text.to_owned(),
        model: "model-x".to_owned(),
        truncated: false,
        duration_ms: 1200,
        input_chars: 5000,
    }
}

/// 終わらせて書き込む（セッションは使い捨て）。
fn finish(project: &mut Project, task: &str, text: &str) -> Plan {
    let plan = runner::finish(project, task, &reply(text), LATER, &mut Session::default());
    run(project, &plan);
    plan
}

fn events(plan: &Plan) -> Vec<&str> {
    plan.log.iter().map(|event| event.event.as_str()).collect()
}

fn section_text(project: &Project, scene: SceneId) -> String {
    apply::current_text(project, novnow::core::agents::Kind::SectionText, scene)
}

#[test]
fn new_tasks_expand_a_scene_into_section_tasks_and_refuse_duplicates_with_e035() {
    let mut project = project(&[], "human");
    let plan = runner::new_tasks(
        &project,
        &Draft {
            role: "writer",
            scope: "@01-04-02",
            intent: "初稿",
            comment: "台詞で説明しない",
            examples: "",
            initiator: "human",
            status: "draft",
        },
        NOW,
    );
    // 本文に 3 節・話プロットに 4 節 → 4 件
    assert_eq!(plan.writes.len(), 4);
    run(&mut project, &plan);
    let board = Board::of(&project);
    assert_eq!(board.tasks.len(), 4);
    assert_eq!(board.tasks[0].id, "T0001");
    assert_eq!(board.tasks[3].scope, "@01-04-02-04");
    assert_eq!(board.tasks[0].status, "draft");
    assert_eq!(board.tasks[0].human_comment, "台詞で説明しない");
    // 同じ役割・同じ節は作らない（E035）。別の役割なら作れる
    let again = runner::new_tasks(
        &project,
        &Draft {
            role: "writer",
            scope: "@01-04-02-02",
            intent: "もう一度",
            comment: "",
            examples: "",
            initiator: "human",
            status: "draft",
        },
        NOW,
    );
    assert!(again.writes.is_empty());
    assert!(
        matches!(&again.notices[..], [Notice::Duplicate { role, scene }] if role == "writer" && scene == "01-04-02-02")
    );
    assert_eq!(create(&mut project, "stylist", "@01-04-02-02"), "T0005");
}

#[test]
fn prepare_checks_the_switches_the_prompt_the_server_the_environment_and_fixed_sections() {
    // 無効
    let mut disabled = project(&[], "human");
    let task = create(&mut disabled, "writer", "@01-04-02-02");
    let mut settings = disabled.settings().clone();
    settings.agents.enabled = false;
    disabled.set_settings(settings);
    assert_eq!(
        runner::prepare(&disabled, &task, NOW, "", &env).unwrap_err(),
        Blocker::Disabled
    );
    // off
    let mut off = project(&[("writer", "off")], "human");
    let task = create(&mut off, "writer", "@01-04-02-02");
    assert_eq!(
        runner::prepare(&off, &task, NOW, "", &env).unwrap_err(),
        Blocker::Off
    );
    // integrator は agents.integrator = "human" なら off
    let mut human = project(&[], "human");
    let task = create(&mut human, "integrator", "@01-04-02-02");
    assert_eq!(
        runner::prepare(&human, &task, NOW, "", &env).unwrap_err(),
        Blocker::Off
    );
    // 環境変数が無い（E030）
    let mut project = project(&[], "human");
    let task = create(&mut project, "writer", "@01-04-02-02");
    assert_eq!(
        runner::prepare(&project, &task, NOW, "", &no_env).unwrap_err(),
        Blocker::Env(vec!["NOVNOW_LLM_KEY".to_owned()])
    );
    // 壊れたプロンプト（E029）
    let mut broken = self::project(&[], "human");
    broken.insert(Document::new(
        "prompts/writer.md",
        "# writer\n## output\n- x\n",
    ));
    let task = create(&mut broken, "writer", "@01-04-02-02");
    match runner::prepare(&broken, &task, NOW, "", &env).unwrap_err() {
        Blocker::Prompt(found) => assert_eq!(found[0].code, DiagnosticCode::E029),
        other => panic!("{other:?}"),
    }
    // サーバが無い
    let mut serverless = self::project(&[], "human");
    serverless.insert(Document::new(
        "agents/mcp.json",
        novnow::core::template::MCP_CONFIG,
    ));
    let task = create(&mut serverless, "writer", "@01-04-02-02");
    assert_eq!(
        runner::prepare(&serverless, &task, NOW, "", &env).unwrap_err(),
        Blocker::NoServer
    );
    // propose の writer は確定済みの節を対象にできない（A7）。stylist（note）はできる
    let mut fixed = self::project(&[], "human");
    let task = create(&mut fixed, "writer", "@01-04-02-04");
    assert_eq!(
        runner::prepare(&fixed, &task, NOW, "", &env).unwrap_err(),
        Blocker::Fixed
    );
    let task = create(&mut fixed, "stylist", "@01-04-02-04");
    assert!(runner::prepare(&fixed, &task, NOW, "", &env).is_ok());
    // 上限を超えたパケットは送れない
    let mut tight = self::project(&[], "human");
    let mut settings = tight.settings().clone();
    settings.agents.max_chars_per_request = 100;
    tight.set_settings(settings);
    let task = create(&mut tight, "writer", "@01-04-02-02");
    assert_eq!(
        runner::prepare(&tight, &task, NOW, "", &env).unwrap_err(),
        Blocker::Over
    );
}

#[test]
fn a_prepared_task_runs_with_the_level_the_hash_and_the_server_recorded() {
    let mut project = project(&[], "human");
    let task = create(&mut project, "writer", "@01-04-02-02");
    let start = start(&mut project, &task);
    assert_eq!(start.task.status, "running");
    assert_eq!(start.task.delegation, Delegation::Propose);
    assert_eq!(start.task.server, "llm-main");
    assert!(start.task.context_hash.starts_with("sha256:"));
    assert_eq!(start.env, [("API_KEY".to_owned(), "secret".to_owned())]);
    assert!(start.input_chars > 1000);
    let params = runner::sampling_params(&start.request, &start.task);
    assert_eq!(params["messages"][0]["role"], "user");
    assert!(params["systemPrompt"]
        .as_str()
        .unwrap()
        .contains("本文担当"));
    assert_eq!(params["metadata"]["novnow"]["task"], json!(task));
    assert_eq!(params["temperature"], json!(0.7));
    // 走っているタスクは二度は走らない
    assert_eq!(
        runner::prepare(&project, &task, NOW, "", &env).unwrap_err(),
        Blocker::Unknown
    );
    // 応答の読み方（sampling の content は 1 つ、tools の content は配列）
    let sampled = json!({ "role": "assistant", "content": { "type": "text", "text": "本文" }, "model": "m1" });
    assert_eq!(
        runner::reply_of(&sampled),
        Some(("本文".to_owned(), "m1".to_owned()))
    );
    let tool = json!({ "content": [{ "type": "text", "text": "a" }, { "type": "image" }, { "type": "text", "text": "b" }] });
    assert_eq!(
        runner::reply_of(&tool),
        Some(("a\nb".to_owned(), String::new()))
    );
    assert_eq!(runner::reply_of(&json!({ "content": [] })), None);
}

#[test]
fn propose_leaves_a_proposal_for_review_and_logs_the_run_without_bodies() {
    let mut project = project(&[], "human");
    let task = create(&mut project, "writer", "@01-04-02-02");
    start(&mut project, &task);
    let mut session = Session::default();
    let plan = runner::finish(
        &project,
        &task,
        &reply("新しい本文。\n\n## question\n窓は？"),
        LATER,
        &mut session,
    );
    run(&mut project, &plan);
    let board = Board::of(&project);
    let done = board.task(&task).unwrap();
    assert_eq!(done.status, "review");
    assert_eq!(done.result, "agents/results/T0001.md");
    let proposal = board.proposal_of(&task).unwrap();
    assert_eq!(proposal.status, "open");
    assert_eq!(proposal.kind, "section-text");
    assert_eq!(proposal.target, "contents/01-04.cnov @01-04-02-02");
    assert_eq!(proposal.after, "新しい本文。");
    assert!(proposal
        .before
        .starts_with("「旧校舎の地下遺跡に眠っているっていう"));
    // 本文は変わらない（A2）
    assert!(section_text(&project, id(1, 4, 2, 2)).starts_with("「旧校舎"));
    assert_eq!(events(&plan), ["run"]);
    let line = plan.log[0].line();
    assert!(
        !line.contains("新しい本文") && !line.contains("secret"),
        "{line}"
    );
    assert!(plan.notices.contains(&Notice::Questions {
        task: task.clone(),
        count: 1
    }));
    let result = files::parse_result(project.text("agents/results/T0001.md")).unwrap();
    assert_eq!(result.output, "新しい本文。\n\n## question\n窓は？");
    assert_eq!(result.proposal, "agents/proposals/P0001.md");
    assert_eq!(session.calls, 1);
}

#[test]
fn apply_auto_applies_with_a_record_and_the_audit_log() {
    let mut project = project(&[("writer", "apply")], "human");
    let task = create(&mut project, "writer", "@01-04-02-02");
    start(&mut project, &task);
    let plan = runner::finish(
        &project,
        &task,
        &reply("書き換えた本文。"),
        LATER,
        &mut Session::default(),
    );
    run(&mut project, &plan);
    assert_eq!(section_text(&project, id(1, 4, 2, 2)), "書き換えた本文。");
    let board = Board::of(&project);
    assert_eq!(board.task(&task).unwrap().status, "applied");
    let proposal = board.proposal_of(&task).unwrap();
    assert_eq!(proposal.status, "applied");
    assert!(proposal.before.starts_with("「旧校舎"));
    assert_eq!(events(&plan), ["auto_applied", "run"]);
    assert!(plan.notices.contains(&Notice::AutoApplied {
        task: task.clone(),
        target: "01-04-02-02".to_owned()
    }));
    // 差し戻し（コメント必須）で本文が before に戻る
    let revert = apply::revert(&project, &proposal.id, "語尾が違う", LATER).unwrap();
    run(&mut project, &revert);
    assert!(section_text(&project, id(1, 4, 2, 2)).starts_with("「旧校舎"));
    let board = Board::of(&project);
    let proposal = board.proposal_of(&task).unwrap();
    assert_eq!(proposal.status, "rejected");
    assert_eq!(proposal.comment, "語尾が違う");
    assert_eq!(board.task(&task).unwrap().status, "rejected");
    assert_eq!(revert.log[0].event, "reject");
    assert_eq!(revert.log[0].detail.as_deref(), Some("revert"));
}

#[test]
fn auto_applying_a_fixed_section_is_logged_and_notified() {
    let mut project = project(&[("writer", "apply")], "human");
    let task = create(&mut project, "writer", "@01-04-02-03");
    // 節 3 を確定済みにする（話プロットの state: fixed を節 3 に移す）
    let plot = world::EPISODE.replace(
        "illustration: 挿絵候補 / リリアが図面を覗き込む / 窓からの逆光\n",
        "illustration: 挿絵候補 / リリアが図面を覗き込む / 窓からの逆光\nstate: fixed\n",
    );
    project.insert(Document::new(world::EPISODE_PATH, plot));
    start(&mut project, &task);
    let plan = runner::finish(
        &project,
        &task,
        &reply("確定済みを書き換える本文。"),
        LATER,
        &mut Session::default(),
    );
    assert_eq!(events(&plan), ["auto_applied", "fixed_overwrite", "run"]);
    assert!(plan.notices.contains(&Notice::FixedOverwrite {
        task: task.clone(),
        target: "01-04-02-03".to_owned()
    }));
}

#[test]
fn an_auto_apply_that_would_add_an_error_stays_a_proposal() {
    let mut project = project(&[("writer", "apply")], "human");
    let task = create(&mut project, "writer", "@01-04-02-02");
    start(&mut project, &task);
    let plan = runner::finish(
        &project,
        &task,
        &reply("'存在しない人物 が来た。"),
        LATER,
        &mut Session::default(),
    );
    run(&mut project, &plan);
    assert!(section_text(&project, id(1, 4, 2, 2)).starts_with("「旧校舎"));
    let board = Board::of(&project);
    assert_eq!(board.task(&task).unwrap().status, "review");
    assert_eq!(board.proposal_of(&task).unwrap().status, "open");
    assert!(
        matches!(&plan.notices[..], [Notice::GateRefused { codes, .. }] if codes.contains("E001"))
    );
    assert_eq!(events(&plan), ["run"]);
}

#[test]
fn accepting_applies_supersedes_the_others_and_partial_accepts_are_marked() {
    let mut project = project(&[], "human");
    let first = create(&mut project, "writer", "@01-04-02-02");
    start(&mut project, &first);
    finish(&mut project, &first, "案その一。");
    let second = create(&mut project, "writer", "@01-04-02-02");
    start(&mut project, &second);
    finish(&mut project, &second, "案その二。\n二行目。");
    let board = Board::of(&project);
    let one = board.proposal_of(&first).unwrap().id.clone();
    let two = board.proposal_of(&second).unwrap().id.clone();
    let plan = apply::accept(&project, &two, Some("案その二。"), LATER).unwrap();
    run(&mut project, &plan);
    assert_eq!(section_text(&project, id(1, 4, 2, 2)), "案その二。");
    let board = Board::of(&project);
    assert_eq!(board.proposal(&two).unwrap().status, "accepted");
    assert!(board.proposal(&two).unwrap().partial);
    assert_eq!(board.proposal(&one).unwrap().status, "superseded");
    assert_eq!(board.task(&second).unwrap().status, "accepted");
    assert_eq!(events(&plan), ["accept"]);
    // 決着した提案はもう採用・却下できない
    assert_eq!(
        apply::accept(&project, &one, None, LATER).unwrap_err(),
        apply::Refusal::Settled
    );
    assert_eq!(
        apply::reject(&project, &two, "x", LATER).unwrap_err(),
        apply::Refusal::Settled
    );
}

#[test]
fn rejecting_records_the_comment_for_the_next_history() {
    let mut project = project(&[], "human");
    let task = create(&mut project, "writer", "@01-04-02-02");
    start(&mut project, &task);
    finish(&mut project, &task, "案。");
    let proposal = Board::of(&project).proposal_of(&task).unwrap().id.clone();
    let plan = apply::reject(&project, &proposal, "心変わりを台詞で説明している", LATER).unwrap();
    run(&mut project, &plan);
    assert_eq!(Board::of(&project).task(&task).unwrap().status, "rejected");
    let next = create(&mut project, "writer", "@01-04-02-02");
    let packet = runner::packet_for(
        &project,
        Board::of(&project).task(&next).unwrap(),
        NOW,
        "星屑",
    )
    .unwrap();
    let history = packet
        .block(novnow::core::resolve::context_rules::PacketBlock::History)
        .unwrap()
        .body();
    assert_eq!(
        history,
        "- T0001 writer rejected: 心変わりを台詞で説明している"
    );
}

#[test]
fn ideas_and_materials_are_applied_to_their_own_files() {
    let mut project = project(&[("ideator", "apply"), ("researcher", "apply")], "human");
    let task = create(&mut project, "ideator", "@01-04-02-02");
    start(&mut project, &task);
    finish(&mut project, &task, "- 時計塔の鐘\n- 地下の湖\n");
    let ideas = project.text("ideas.md");
    assert!(
        ideas.contains("# I0001 時計塔の鐘\nstatus: inbox"),
        "{ideas}"
    );
    assert!(ideas.contains("# I0002 地下の湖"), "{ideas}");
    let task = create(&mut project, "researcher", "@01-04-02-02");
    start(&mut project, &task);
    finish(&mut project, &task, "旧校舎の窓は上下げ窓が主流。");
    let index = project.text("materials/index.json");
    assert!(
        index.contains("\"added_by\": \"agent:researcher\""),
        "{index}"
    );
    assert!(index.contains("\"status\": \"proposed\""), "{index}");
    assert!(project.document("materials/inbox/M0138.md").is_some());
}

#[test]
fn notes_are_never_applied_even_with_apply() {
    let mut project = project(&[("stylist", "apply")], "human");
    let task = create(&mut project, "stylist", "@01-04-02-02");
    start(&mut project, &task);
    let plan = runner::finish(
        &project,
        &task,
        &reply("- 語尾が単調"),
        LATER,
        &mut Session::default(),
    );
    run(&mut project, &plan);
    let board = Board::of(&project);
    assert_eq!(board.task(&task).unwrap().status, "review");
    assert_eq!(board.proposal_of(&task).unwrap().kind, "note");
    assert!(section_text(&project, id(1, 4, 2, 2)).starts_with("「旧校舎"));
}

#[test]
fn messages_wait_for_approval_unless_both_roles_are_autonomous() {
    let mut project = project(&[], "human");
    let task = create(&mut project, "writer", "@01-04-02-02");
    start(&mut project, &task);
    finish(
        &mut project,
        &task,
        "本文。\n\n## message\nto: researcher\nkind: request\n窓の資料が欲しい",
    );
    let board = Board::of(&project);
    assert_eq!(board.threads.len(), 1);
    assert_eq!(board.threads[0].scope, "@01-04-02-02");
    assert_eq!(board.threads[0].messages[0].approved_by, "pending");
    assert_eq!(board.threads[0].messages[0].from, "writer");
    let mut autonomous = self::project(
        &[("writer", "autonomous"), ("researcher", "autonomous")],
        "human",
    );
    let task = create(&mut autonomous, "writer", "@01-04-02-02");
    start(&mut autonomous, &task);
    finish(
        &mut autonomous,
        &task,
        "本文。\n\n## message\nto: researcher\n窓の資料",
    );
    assert_eq!(
        Board::of(&autonomous).threads[0].messages[0].approved_by,
        "auto"
    );
}

#[test]
fn an_autonomous_integrator_chains_tasks_until_the_hop_limit_with_w095() {
    let mut project = project(&[("integrator", "autonomous")], "agent");
    let mut settings = project.settings().clone();
    settings.agents.limits.max_chain_hops = 2;
    project.set_settings(settings);
    let mut session = Session::default();
    let writer = create(&mut project, "writer", "@01-04-02-02");
    start(&mut project, &writer);
    // writer が終わると integrator のタスクが起きる（起動: agent）
    let plan = runner::finish(&project, &writer, &reply("本文。"), LATER, &mut session);
    run(&mut project, &plan);
    assert_eq!(plan.follow.len(), 1);
    let integrator = plan.follow[0].clone();
    let board = Board::of(&project);
    let created = board.task(&integrator).unwrap();
    assert_eq!(created.role, "integrator");
    assert_eq!(created.initiator, "agent");
    assert_eq!(created.status, "ready");
    assert!(events(&plan).contains(&"chain_hop"));
    assert_eq!(session.auto_started, 1);
    // integrator の ## task が次のタスクを起こす（hop 2）
    start(&mut project, &integrator);
    let plan = runner::finish(
        &project,
        &integrator,
        &reply("停滞はない。\n\n## task\nrole: stylist\nscope: @01-04-02-02\nintent: 文体を見る"),
        LATER,
        &mut session,
    );
    run(&mut project, &plan);
    assert_eq!(plan.follow.len(), 1);
    let stylist = plan.follow[0].clone();
    assert_eq!(Board::of(&project).task(&stylist).unwrap().role, "stylist");
    // stylist が終わると integrator を起こしたい（hop 3）が、上限 2 で止まる
    start(&mut project, &stylist);
    let plan = runner::finish(&project, &stylist, &reply("- 指摘"), LATER, &mut session);
    assert!(plan.follow.is_empty());
    assert!(plan.notices.contains(&Notice::Limit {
        limit: "max_chain_hops".to_owned()
    }));
    let stop: Vec<&LogEvent> = plan
        .log
        .iter()
        .filter(|event| event.event == "limit_stop")
        .collect();
    assert_eq!(stop.len(), 1);
    assert_eq!(stop[0].detail.as_deref(), Some("max_chain_hops"));
}

#[test]
fn without_an_autonomous_integrator_nothing_chains() {
    let mut project = project(&[("writer", "autonomous")], "human");
    let task = create(&mut project, "writer", "@01-04-02-02");
    start(&mut project, &task);
    let plan = runner::finish(
        &project,
        &task,
        &reply("本文。"),
        LATER,
        &mut Session::default(),
    );
    assert!(plan.follow.is_empty());
    assert!(!events(&plan).contains(&"chain_hop"));
}

#[test]
fn session_limits_stop_automatic_starts_and_zero_means_unlimited() {
    let limits = novnow::core::model::project::AgentLimits {
        max_tasks_per_session: 2,
        max_chain_hops: 3,
        max_input_chars_per_session: 1000,
    };
    let mut session = Session::default();
    assert_eq!(runner::limit(&limits, &session, 1), None);
    assert_eq!(runner::limit(&limits, &session, 4), Some(Limit::Hops));
    session.auto_started = 2;
    assert_eq!(runner::limit(&limits, &session, 1), Some(Limit::Tasks));
    session.auto_started = 0;
    session.input_chars = 1000;
    assert_eq!(runner::limit(&limits, &session, 1), Some(Limit::InputChars));
    let unlimited = novnow::core::model::project::AgentLimits {
        max_tasks_per_session: 0,
        max_chain_hops: 0,
        max_input_chars_per_session: 0,
    };
    session.auto_started = 1_000;
    assert_eq!(runner::limit(&unlimited, &session, 99), None);
    assert_eq!(Limit::Tasks.key(), "max_tasks_per_session");
}

#[test]
fn failures_return_the_task_to_ready_and_cancel_marks_it_canceled() {
    let mut project = project(&[], "human");
    let task = create(&mut project, "writer", "@01-04-02-02");
    start(&mut project, &task);
    let plan = runner::fail(&project, &task, "E063 llm-main", 60_000, LATER);
    run(&mut project, &plan);
    assert_eq!(Board::of(&project).task(&task).unwrap().status, "ready");
    assert_eq!(plan.log[0].error.as_deref(), Some("E063 llm-main"));
    assert!(Board::of(&project).proposals.is_empty());
    let plan = runner::cancel(&project, &task, false, LATER);
    run(&mut project, &plan);
    assert_eq!(Board::of(&project).task(&task).unwrap().status, "canceled");
    assert!(plan.log.is_empty());
    assert!(runner::cancel(&project, &task, false, LATER)
        .writes
        .is_empty());
}

#[test]
fn delegation_changes_are_written_to_project_toml_and_logged() {
    let project = project(&[], "human");
    let settings = project.settings().agents.clone();
    let toml_text = "schema_version = 2\ntitle = \"星屑\"\n";
    let (text, event) =
        runner::set_level(toml_text, &settings, "writer", Delegation::Apply, NOW).unwrap();
    let found = novnow::core::syntax::project_toml::settings(&text);
    assert_eq!(
        found.agents.roles.get("writer").map(String::as_str),
        Some("apply")
    );
    assert_eq!(found.title, "星屑");
    assert!(
        text.contains("title = \"星屑\""),
        "書かなかったキーはそのまま（無損失編集）: {text}"
    );
    assert_eq!(event.event, "delegation_changed");
    assert_eq!(event.detail.as_deref(), Some("writer: propose -> apply"));
    let (text, _) =
        runner::set_level(&text, &settings, "integrator", Delegation::Autonomous, NOW).unwrap();
    let found = novnow::core::syntax::project_toml::settings(&text);
    assert_eq!(found.agents.integrator, "agent");
    assert_eq!(
        found.agents.roles.get("integrator").map(String::as_str),
        Some("autonomous")
    );
    let (text, _) =
        runner::set_level(&text, &settings, "integrator", Delegation::Off, NOW).unwrap();
    let found = novnow::core::syntax::project_toml::settings(&text);
    assert_eq!(found.agents.integrator, "human");
    // 壊れた TOML は上書きしない（§4.5）
    assert!(runner::set_level("title = ", &settings, "writer", Delegation::Apply, NOW).is_none());
}

#[test]
fn stop_all_lowers_every_writing_level_to_propose() {
    let project = project(
        &[
            ("writer", "apply"),
            ("integrator", "autonomous"),
            ("stylist", "off"),
        ],
        "agent",
    );
    let mut settings = project.settings().agents.clone();
    settings.delegation = "apply".to_owned();
    let toml_text = "[agents]\ndelegation = \"apply\"\nintegrator = \"agent\"\n\n[agents.roles]\nwriter = \"apply\"\nintegrator = \"autonomous\"\nstylist = \"off\"\n";
    let (text, events) = runner::stop_all(toml_text, &settings, NOW).unwrap();
    let found = novnow::core::syntax::project_toml::settings(&text);
    assert_eq!(found.agents.delegation, "propose");
    assert_eq!(
        found.agents.roles.get("writer").map(String::as_str),
        Some("propose")
    );
    assert_eq!(
        found.agents.roles.get("integrator").map(String::as_str),
        Some("propose")
    );
    assert_eq!(
        found.agents.roles.get("stylist").map(String::as_str),
        Some("off")
    );
    assert_eq!(found.agents.integrator, "agent");
    // writer・integrator と、既定（apply）に従っていた役割が propose に下がる
    let roles: Vec<&str> = events
        .iter()
        .filter_map(|event| event.role.as_deref())
        .collect();
    assert!(
        roles.contains(&"writer") && roles.contains(&"integrator") && roles.contains(&"plotter")
    );
    assert!(!roles.contains(&"stylist"));
}

#[test]
fn five_parallel_tasks_run_two_at_a_time_without_id_collisions_and_same_sections_wait() {
    let mut project = project(&[], "human");
    let mut ids = Vec::new();
    for (role, scope) in [
        ("writer", "@01-04-02-01"),
        ("stylist", "@01-04-02-01"),
        ("writer", "@01-04-02-02"),
        ("writer", "@01-04-02-03"),
        ("logician", "@01-04-02-02"),
    ] {
        ids.push(create(&mut project, role, scope));
    }
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 5);
    let slot = |task: &str| {
        let board = Board::of(&project);
        let found = board.task(task).unwrap();
        (
            task.to_owned(),
            "llm-main".to_owned(),
            files::task_scene(found),
        )
    };
    let mut queue: Vec<_> = ids.iter().map(|task| slot(task)).collect();
    let mut running: Vec<(String, String, Option<SceneId>)> = Vec::new();
    // 1 件目が走る。2 件目は同じ節（R1）なので飛ばして 3 件目
    let first = runner::pick(&queue, &running, 2).unwrap();
    running.push(queue.remove(first));
    let second = runner::pick(&queue, &running, 2).unwrap();
    assert_eq!(queue[second].0, ids[2]);
    running.push(queue.remove(second));
    // max_parallel = 2 で満杯
    assert_eq!(runner::pick(&queue, &running, 2), None);
    // サーバごとに最大 2（max_parallel が大きくても）
    assert_eq!(runner::pick(&queue, &running, 8), None);
    // 1 件目が終わると、待っていた同じ節の 2 件目が走れる
    running.remove(0);
    assert_eq!(queue[runner::pick(&queue, &running, 2).unwrap()].0, ids[1]);
    // 並列でも提案の ID は衝突しない（結果の処理は 1 件ずつ、採番は最大値 + 1）
    let mut session = Session::default();
    for task in &ids {
        start(&mut project, task);
    }
    for task in &ids {
        let plan = runner::finish(&project, task, &reply("案。"), LATER, &mut session);
        run(&mut project, &plan);
    }
    let board = Board::of(&project);
    let mut proposals: Vec<&str> = board
        .proposals
        .iter()
        .map(|found| found.id.as_str())
        .collect();
    proposals.dedup();
    assert_eq!(proposals, ["P0001", "P0002", "P0003", "P0004", "P0005"]);
    assert_eq!(runner::max_parallel(&project.settings().agents), 2);
}

// ============================================================ MCP のツール（§15.8.3）

fn call(project: &Project, name: &str, args: Value) -> server::Outcome {
    server::call(project, name, &args, NOW, "星屑")
}

#[test]
fn the_server_lists_the_fourteen_tools() {
    let tools = server::tools();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 14);
    assert_eq!(names[0], "novnow.project.info");
    assert_eq!(names[13], "novnow.mail.send");
}

#[test]
fn read_tools_answer_from_the_project_documents() {
    let project = project(&[], "human");
    let info = call(&project, "novnow.project.info", json!({}))
        .result
        .unwrap();
    assert_eq!(info["sections"], 3);
    assert_eq!(info["title"], "星屑");
    let section = call(
        &project,
        "novnow.section.get",
        json!({ "scene": "@01-04-02-02", "neighbors": 1 }),
    )
    .result
    .unwrap();
    assert!(section["text"].as_str().unwrap().starts_with("「旧校舎"));
    assert_eq!(section["before"][0]["scene"], "01-04-02-01");
    assert_eq!(section["after"][0]["scene"], "01-04-02-03");
    let cast = call(
        &project,
        "novnow.cast.get",
        json!({ "scene": "01-04-02-02" }),
    )
    .result
    .unwrap();
    assert!(cast["cast"].as_str().unwrap().contains("- name: アルス"));
    let packet = call(
        &project,
        "novnow.context.build",
        json!({ "role": "writer", "scene": "@01-04-02-02" }),
    )
    .result
    .unwrap();
    assert!(packet["packet"]
        .as_str()
        .unwrap()
        .starts_with("# novnow context\nscene: 01-04-02-02\nrole: writer"));
    let words = call(
        &project,
        "novnow.lexicon.search",
        json!({ "query": "時計" }),
    )
    .result
    .unwrap();
    assert_eq!(words["words"][0]["term"], "星屑の時計");
    let plot = call(
        &project,
        "novnow.plot.get",
        json!({ "scene": "@01-04-02-02", "level": "episode" }),
    )
    .result
    .unwrap();
    assert!(plot["text"]
        .as_str()
        .unwrap()
        .starts_with("@01-04-02 図書室の攻防"));
    let tree = call(
        &project,
        "novnow.structure.tree",
        json!({ "level": "scene" }),
    )
    .result
    .unwrap();
    assert_eq!(tree["nodes"][0]["id"], "01-04-02");
    let diagnostics = call(
        &project,
        "novnow.diagnostics.run",
        json!({ "scope": "contents/" }),
    )
    .result
    .unwrap();
    assert!(diagnostics["diagnostics"].is_array());
    assert!(matches!(
        call(&project, "novnow.section.get", json!({ "scene": "@01-04" })).result,
        Err(server::ToolError::Argument("scene"))
    ));
    assert!(matches!(
        call(&project, "novnow.nope", json!({})).result,
        Err(server::ToolError::Unknown(_))
    ));
}

#[test]
fn direct_writes_need_a_role_with_apply_or_autonomous_otherwise_e036() {
    let project = project(&[("writer", "apply")], "human");
    for args in [
        json!({ "scene": "@01-04-02-02", "text": "x" }),
        json!({ "role": "stylist", "scene": "@01-04-02-02", "text": "x" }),
        json!({ "role": "nobody", "scene": "@01-04-02-02", "text": "x" }),
    ] {
        match call(&project, "novnow.section.write", args).result {
            Err(server::ToolError::Diagnostic(found)) => {
                assert_eq!(found.code, DiagnosticCode::E036)
            }
            other => panic!("{other:?}"),
        }
    }
    let mut project = project;
    let outcome = call(
        &project,
        "novnow.section.write",
        json!({ "role": "writer", "scene": "@01-04-02-02", "text": "直接書いた本文。", "note": "外部から" }),
    );
    assert_eq!(outcome.result.as_ref().unwrap()["status"], "applied");
    assert_eq!(events(&outcome.plan), ["direct_write"]);
    run(&mut project, &outcome.plan);
    assert_eq!(section_text(&project, id(1, 4, 2, 2)), "直接書いた本文。");
    let board = Board::of(&project);
    let proposal = &board.proposals[0];
    assert_eq!(proposal.status, "applied");
    assert_eq!(proposal.origin, "mcp:writer");
    assert_eq!(proposal.note, "外部から");
    assert!(proposal.before.starts_with("「旧校舎"));
    // E 系が増える書き込みはせず、提案として残す
    let outcome = call(
        &project,
        "novnow.section.write",
        json!({ "role": "writer", "scene": "@01-04-02-02", "text": "'存在しない人物" }),
    );
    assert!(matches!(
        outcome.result,
        Err(server::ToolError::Gate { .. })
    ));
    assert_eq!(outcome.plan.writes.len(), 1);
    assert!(outcome.plan.writes[0].path.starts_with("agents/proposals/"));
}

#[test]
fn propose_mail_and_material_tools_only_add_records() {
    let mut project = project(&[], "human");
    let outcome = call(
        &project,
        "novnow.section.propose",
        json!({ "scene": "@01-04-02-01", "text": "案", "note": "n" }),
    );
    assert_eq!(outcome.result.as_ref().unwrap()["status"], "open");
    run(&mut project, &outcome.plan);
    assert!(section_text(&project, id(1, 4, 2, 1)).starts_with("「やっぱり"));
    assert_eq!(Board::of(&project).proposals[0].origin, "mcp");
    let outcome = call(
        &project,
        "novnow.mail.send",
        json!({ "to": "writer", "kind": "request", "scope": "@01-04-02", "body": "続きを" }),
    );
    run(&mut project, &outcome.plan);
    let thread = &Board::of(&project).threads[0];
    assert_eq!(thread.messages[0].from, "mcp");
    assert_eq!(thread.messages[0].approved_by, "pending");
    assert!(matches!(
        call(
            &project,
            "novnow.mail.send",
            json!({ "to": "writer", "kind": "shout", "body": "x" })
        )
        .result,
        Err(server::ToolError::Argument("kind"))
    ));
    let outcome = call(
        &project,
        "novnow.materials.propose",
        json!({ "title": "窓", "source": "https://example.org", "markdown": "上下げ窓", "role": "researcher" }),
    );
    assert_eq!(outcome.result.as_ref().unwrap()["status"], "proposed");
    run(&mut project, &outcome.plan);
    assert!(project
        .text("materials/index.json")
        .contains("\"added_by\": \"agent:researcher\""));
}

#[test]
fn a_dry_run_writes_only_the_packet() {
    let mut project = project(&[], "human");
    let task = create(&mut project, "writer", "@01-04-02-02");
    let (path, text) = runner::dry_run(&project, &task, NOW, "星屑").unwrap();
    assert_eq!(path, "agents/results/T0001.dryrun.md");
    assert!(text.starts_with("# novnow context\n"));
    // 外したブロックはドライランでも外れる
    let mut found = Board::of(&project).task(&task).unwrap().clone();
    found.exclude = "lexicon".to_owned();
    project.insert(Document::new(
        files::task_path(&task),
        files::write_task(&found),
    ));
    let (_, text) = runner::dry_run(&project, &task, NOW, "星屑").unwrap();
    assert!(text.contains("## 6 lexicon\n\n## 7 materials\n"));
    // 結果の番号も数えるので、次のタスクは T0002
    project.insert(Document::new(path, text));
    assert_eq!(
        files::next_task_id(files::paths(&project).iter().map(String::as_str)),
        "T0002"
    );
    let _ = sections::all(&project);
}
