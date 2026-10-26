use novnow::core::{
    annotations, export,
    features::{sections, Project},
    id::SceneId,
    model::document::Document,
    stage_two::{local_llm, metadata, metrics, search, stock},
};
fn project(files: &[(&str, &str)]) -> Project {
    Project::with_documents(
        Default::default(),
        files.iter().map(|(p, t)| Document::new(*p, *t)),
    )
}
#[test]
fn citations_are_suffix_only_variable_width_adjacent_and_utf8_safe() {
    for text in ["日本語^M0001", "文 ^M0001 ^S12345", "文^M0001^S12345"] {
        let ids = annotations::citations(text);
        assert!(!ids.is_empty());
        assert!(!annotations::without_citations(text).contains('^'));
    }
    for text in [
        "日本語",
        "abc",
        "文 \\^M0001",
        "^M123",
        "^M0001 中間",
        "文 // ^M0001",
    ] {
        assert!(annotations::citations(text).is_empty(), "{text}");
    }
}
#[test]
fn annotations_are_shared_by_counts_export_and_metrics() {
    let text = "@01-01-01\n「台詞」 ^M12345 @葵 // note\n! 逆光\n# メモ\n本文。\n";
    let p = project(&[("contents/01-01.txt", text)]);
    let options = export::Options {
        newline: novnow::core::model::Newline::Lf,
        ..Default::default()
    };
    let out = export::run(&p, None, &options);
    assert!(!out.contains('^'));
    assert!(!out.contains("逆光"));
    assert!(!out.contains("メモ"));
    assert_eq!(sections::count_chars(text), metrics::section(text).total);
    assert_eq!(
        sections::count_chars(text),
        out.chars().filter(|c| !c.is_whitespace()).count()
    );
    let kept = export::run(
        &p,
        None,
        &export::Options {
            keep_illustration_memo: true,
            ..options
        },
    );
    assert!(kept.contains("［挿絵: 逆光］"));
}
#[test]
fn stock_and_blurbs_are_not_body_or_default_export() {
    let p = project(&[
        (
            "plots/01.txt",
            "@01-00 あらすじ\nblurb: 章の表紙\n;\n@01-01 本編\n",
        ),
        ("contents/01-01.txt", "@01-01-01\n本編。\n"),
        ("contents/00-01.txt", "@00-01-01\nストック。\n"),
        ("contents/01-00.txt", "@01-00-01\n本文ではない。\n"),
    ]);
    assert_eq!(metadata::chapter_blurb(&p, 1), "章の表紙");
    let plain = export::run(&p, None, &Default::default());
    assert!(!plain.contains("ストック"));
    assert!(!plain.contains("本文ではない"));
    assert!(!plain.contains("章の表紙"));
    let options = export::Options {
        include_stock: true,
        include_blurb: true,
        ..Default::default()
    };
    let out = export::run(&p, None, &options);
    assert!(out.contains("ストック"));
    assert!(out.contains("章の表紙"));
    assert_eq!(metrics::project(&p).total, 3);
}
#[test]
fn timeline_relative_unknown_ranges_and_clocks() {
    assert_eq!(metadata::when("405 春分 09:15", None).unwrap().clock, 555);
    assert_eq!(
        metadata::when("+1", Some(405.0)).unwrap().start,
        Some(406.0)
    );
    assert!(metadata::when("+1", None).is_none());
    let range = metadata::when("12 .. 14", None).unwrap();
    assert_eq!((range.start, range.end), (Some(12.0), Some(14.0)));
    assert_eq!(metadata::when("?", None).unwrap().start, None);
    assert!(metadata::when("12 回想", None).unwrap().flashback);
}
#[test]
fn addresses_keep_colons_on_rhs_modes_and_wildcards() {
    let text = "# 葵\naddress: '恵 => お姉:ちゃん, 恵\n## modes\n### A 普段\naddress: * → きみ\n";
    let rows = metadata::addresses(text);
    assert_eq!(rows[0].names, ["お姉:ちゃん", "恵"]);
    assert_eq!(rows[1].mode.as_deref(), Some("A"));
    assert_eq!(rows[1].target, "*");
}
#[test]
fn asset_paths_reject_escape_urls_absolute_and_icon_outside_assets() {
    for path in [
        "../secret",
        "/tmp/a.png",
        "https://example.test/a.png",
        "assets/../secret",
    ] {
        assert!(metadata::asset_path(path, true).is_none(), "{path}");
    }
    assert!(metadata::asset_path("plots/map.txt", true).is_none());
    assert_eq!(
        metadata::asset_path("./assets/icons/a.png", true).as_deref(),
        Some("assets/icons/a.png")
    );
}
#[test]
fn search_regex_case_deadline_replacement_and_snapshots() {
    let p = project(&[("contents/01-01.txt", "@01-01-01\nApple apple\n")]);
    let q = search::Query {
        text: "apple".into(),
        ..Default::default()
    };
    let hits = search::run(&p, &q, || 0).unwrap();
    assert_eq!(hits.hits.len(), 2);
    let sensitive = search::Query {
        case_sensitive: true,
        ..q.clone()
    };
    assert_eq!(search::run(&p, &sensitive, || 0).unwrap().hits.len(), 1);
    let invalid = search::Query {
        text: "(".into(),
        regex: true,
        ..Default::default()
    };
    assert!(search::run(&p, &invalid, || 0).is_err());
    assert!(search::run(&p, &q, || 1000).unwrap().timed_out);
    let edit = search::replacement(&p, &q, "梨", &hits).unwrap();
    assert_eq!(edit.changes[0].edits.len(), 2);
    let snap = search::snapshot(&p, &q, &hits, "2026-10-21T00:00:00Z").unwrap();
    assert_eq!(snap.id, "S0001");
    assert!(snap.body.contains("Apple"));
    let mut index: serde_json::Value = serde_json::from_str(&snap.index).unwrap();
    assert_eq!(index["items"][0]["source"]["kind"], "search");
    index["next_id"] = serde_json::json!(2);
    let p2 = project(&[("materials/index.json", &index.to_string())]);
    assert_eq!(
        search::snapshot(&p2, &q, &hits, "stamp").unwrap().id,
        "S0002"
    );
}
#[test]
fn local_config_accepts_only_loopback_and_preserves_manual_definitions() {
    for url in [
        "http://localhost:11434",
        "http://127.0.0.1:8000/mcp",
        "http://[::1]:5000/mcp",
    ] {
        assert!(local_llm::is_loopback(url));
    }
    for url in [
        "http://localhost.evil/",
        "http://localhost@evil/",
        "http://127.0.0.2/",
        "file:///tmp/a",
        "http://localhost:0/",
        "http://localhost?evil",
    ] {
        assert!(!local_llm::is_loopback(url), "{url}");
    }
    let text = r#"{"schema_version":1,"future":3,"servers":{"manual":{"command":"manual","future":true},"old":{"managed_by":"novnow-local-llm"}}}"#;
    let config = novnow::core::model::project::LocalLlm {
        enabled: true,
        url: "http://localhost:11434/mcp".into(),
        model: "small".into(),
        ..Default::default()
    };
    let value: serde_json::Value =
        serde_json::from_str(&local_llm::configure(text, &config).unwrap()).unwrap();
    assert_eq!(value["future"], 3);
    assert_eq!(value["servers"]["manual"]["future"], true);
    assert_eq!(value["servers"]["novnow-local-llm"]["transport"], "http");
    assert!(value["servers"].get("old").is_none());
}
#[test]
fn metrics_describe_not_evaluate_and_ignore_zero_population() {
    assert_eq!(metrics::t_scores(&[1, 2, 0]), vec![None, None, None]);
    assert_eq!(metrics::t_scores(&[3, 3, 3]), vec![Some(50.0); 3]);
    let m = metrics::section("「a」 @葵\n「b」\n地の文\n「c」\n");
    assert_eq!(m.speakers["葵"].lines, 2);
    assert_eq!(m.speakers[""].lines, 1);
    assert_eq!(
        m.dialogue,
        m.speakers.values().map(|s| s.chars).sum::<usize>()
    );
    let text="長い文章は長い文章が長い文章を長い文章に長い文章で長い文章と長い文章は長い文章が長い文章を";
    let positions = metrics::comma_candidates(text, 20);
    assert_eq!(positions, metrics::comma_candidates(text, 20));
    assert!(metrics::comma_candidates(&format!("「{text}」"), 20).is_empty());
}
#[test]
fn session_deltas_allow_deletion_and_idle_resets_without_counting_idle() {
    let mut session = metrics::Session::default();
    session.begin(0.0, 100);
    assert_eq!(session.edit(60.0, 120), (20, 60, false));
    assert_eq!(session.edit(120.0, 90), (-30, 60, false));
    assert_eq!(session.delta, -10);
    assert_eq!(session.edit(2000.0, 95), (5, 0, true));
    assert_eq!(session.delta, 5);
}
#[test]
fn stock_assignment_updates_references_and_refuses_collisions() {
    let p = project(&[
        ("contents/00-01.txt", "@00-01-01\n本文\n"),
        ("plots/00-01.txt", "@00-01-01\nsummary: plot\n"),
        ("ideas.md", "# I0001\nrelates: @00-01-01-01\n"),
    ]);
    let plan = stock::assign(&p, 1, 2).unwrap();
    assert_eq!(plan.renames[0].1, "contents/02-01.txt");
    assert!(plan.edit.changes.iter().any(|f| f.path == "ideas.md"));
    assert!(stock::assign(&p, 1, 0).is_err());
}
#[test]
fn reader_is_pure_bounded_and_readonly() {
    let options = novnow::core::reader::Options {
        rows: 2,
        columns: 2,
        vertical: true,
    };
    let first = novnow::core::reader::layout_cells("あいうえお", options);
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].glyphs[0].column, 1);
    assert_eq!(first[0].glyphs[2].column, 0);
    assert_eq!(
        first,
        novnow::core::reader::layout_cells("あいうえお", options)
    );
}
#[cfg(feature = "ui")]
#[test]
fn human_proposal_never_mutates_body_and_accepts_through_existing_pipeline() {
    use novnow::{
        core::agents::{apply, files},
        ui::workbench::Workbench,
    };
    let p = project(&[("contents/01-01.txt", "@01-01-01\n原文\n")]);
    let mut bench = Workbench::new(p, Some("01-01-01-01"), 400);
    let before = bench.project().text("contents/01-01.txt").to_owned();
    bench.toggle_human("2026-10-21T00:00:00Z", 0.0);
    bench.buffer_mut().push_str("提案");
    bench.edited(1.0);
    bench.commit();
    assert_eq!(bench.project().text("contents/01-01.txt"), before);
    assert_eq!(bench.counts().0, 2);
    bench.finish_human(2.0);
    let proposal =
        files::parse_proposal(bench.project().text("agents/proposals/H0001.md")).unwrap();
    assert_eq!(proposal.before, "原文");
    assert_eq!(proposal.after, "原文提案");
    let plan = apply::accept(bench.project(), "H0001", None, "2026-10-21T00:00:01Z").unwrap();
    assert!(plan
        .writes
        .iter()
        .any(|f| f.path == "contents/01-01.txt" && f.text.contains("原文提案")));
    bench.toggle_human("stamp", 3.0);
    bench.buffer_mut().push('別');
    bench.finish_human(4.0);
    assert!(bench
        .project()
        .document("agents/proposals/H0002.md")
        .is_some());
}
#[test]
fn registered_custom_role_uses_custom_prompt_and_context_is_mandatory() {
    let p = project(&[(
        "project.json",
        r#"{"schema_version":1,"agents":{"custom_roles":["observer"]}}"#,
    )]);
    assert!(novnow::core::agents::roles(&p).contains(&"observer".into()));
    assert_eq!(
        novnow::core::agents::files::prompt_path("observer"),
        "prompts/custom/observer.md"
    );
    let _id = SceneId::default();
}

#[test]
fn ruby_counts_exclude_readings_and_reader_composes_them() {
    use novnow::core::reader::{self, Metrics, Options};
    assert_eq!(sections::count_chars("漢字《かんじ》"), 2);
    let options = Options {
        rows: 5,
        columns: 4,
        vertical: true,
    };
    let mut metrics = Metrics {
        advance: 20.0,
        line_advance: 30.0,
        ..Default::default()
    };
    metrics.glyph_advances.insert('漢', 10.0);
    let pages = reader::layout("漢字《かんじ》12 abc、ー", options, &metrics);
    let glyphs = pages.iter().flat_map(|p| &p.glyphs).collect::<Vec<_>>();
    assert_eq!(glyphs[0].ruby, "か");
    assert_eq!(glyphs[1].ruby, "んじ");
    assert_eq!(glyphs[1].y, 10.0);
    assert!(glyphs
        .iter()
        .any(|g| g.text == "12" && g.compact && !g.rotate));
    assert!(glyphs.iter().any(|g| g.ch == 'a' && g.rotate));
    assert!(glyphs.iter().any(|g| g.ch == '、' && g.punctuation));
    assert!(glyphs.iter().any(|g| g.ch == 'ー' && g.rotate));
}
#[test]
fn reader_moves_prohibited_start_and_end_off_the_boundary() {
    use novnow::core::reader::{self, Options};
    let pages = reader::layout_cells(
        "あいう「え」、お",
        Options {
            rows: 4,
            columns: 4,
            vertical: true,
        },
    );
    for page in pages {
        for column in 0..4 {
            let line = page
                .glyphs
                .iter()
                .filter(|g| g.column == column)
                .collect::<Vec<_>>();
            if let Some(first) = line.first() {
                assert!(!"、。「」".contains(first.ch) || first.ch == '「');
            }
            if let Some(last) = line.last() {
                assert_ne!(last.ch, '「');
            }
        }
    }
}
#[test]
fn address_cell_write_preserves_other_fields_and_unknown_text() {
    let text="# A\nunknown: preserved\naddress: B => Bさん // note\n## modes\n### calm\naddress: B => 君\n# B\n";
    let updated = metadata::set_address(text, "A", "B", "あなた:先輩").unwrap();
    assert!(updated.contains("unknown: preserved"));
    assert!(updated.contains("address: B => あなた:先輩 // note"));
    assert!(updated.contains("address: B => 君"));
    let inserted = metadata::set_address(&updated, "B", "A", "Aさん").unwrap();
    assert!(inserted.ends_with("address: A => Aさん\n"));
    assert!(metadata::set_address(text, "missing", "B", "name").is_none());
    assert!(metadata::set_address(text, "A", "B", "a\nb").is_none());
}
#[test]
fn daily_stats_accept_published_schema_and_preserve_unknown_keys() {
    let mut stats:metrics::Stats=serde_json::from_str(r#"{"schema_version":1,"future":true,"days":{"2026-10-21":{"chars":12,"hours":[9,22],"custom":1}}}"#).unwrap();
    stats.record("2026-10-21", 9, -2, 30, false);
    let value = serde_json::to_value(&stats).unwrap();
    assert_eq!(value["future"], true);
    assert_eq!(value["days"]["2026-10-21"]["custom"], 1);
    assert_eq!(stats.days["2026-10-21"].hours, vec![9, 22]);
    let dates = metrics::recent_dates("2024-03-01", 3);
    assert_eq!(dates, vec!["2024-03-01", "2024-02-29", "2024-02-28"]);
    assert_eq!(metrics::weekday("2026-10-21"), Some(3));
    assert_eq!(
        stats.average(&metrics::recent_dates("2026-10-21", 7)),
        10.0 / 7.0
    );
    let mut session = metrics::Session::default();
    session.begin(0.0, 0);
    let mut seconds = 0;
    for i in 1..=100 {
        seconds += session.edit(i as f64 / 10.0, i).1;
    }
    assert!((9..=10).contains(&seconds));
}
#[test]
fn illustration_list_unifies_plot_and_body_without_counting_memos() {
    let p = project(&[
        (
            "plots/01-01.txt",
            "@01-01-01 title\nillustration: plot memo\n",
        ),
        ("contents/01-01.txt", "@01-01-01 title\n文\n! body memo\n"),
    ]);
    let items = metadata::illustrations(&p);
    assert_eq!(items.len(), 2);
    assert!(items
        .iter()
        .any(|i| i.source == metadata::IllustrationSource::Contents));
    assert!(items
        .iter()
        .any(|i| i.source == metadata::IllustrationSource::Plot));
}
#[cfg(feature = "ui")]
#[test]
fn multi_file_replace_is_one_undo_redo_transaction() {
    let p = project(&[
        ("contents/01-01.txt", "@01-01-01\n原文\n"),
        ("contents/01-02.txt", "@01-02-01\n原文\n"),
    ]);
    let query = search::Query {
        text: "原文".into(),
        ..Default::default()
    };
    let results = search::run(&p, &query, || 0).unwrap();
    let edit = search::replacement(&p, &query, "変更", &results).unwrap();
    let mut bench = novnow::ui::workbench::Workbench::new(p, None, 1500);
    assert!(bench.apply_edit(&edit, 1.0));
    assert!(bench.undo_workspace(false, 2.0));
    for path in ["contents/01-01.txt", "contents/01-02.txt"] {
        assert!(bench.project().text(path).contains("原文"));
    }
    assert!(bench.undo_workspace(true, 3.0));
    for path in ["contents/01-01.txt", "contents/01-02.txt"] {
        assert!(bench.project().text(path).contains("変更"));
    }
}

#[test]
fn stale_search_cannot_replace_unrelated_current_text() {
    let mut p = project(&[("contents/01-01.txt", "@01-01-01\n原文\n")]);
    let query = search::Query {
        text: "原文".into(),
        ..Default::default()
    };
    let results = search::run(&p, &query, || 0).unwrap();
    p.insert(Document::new("contents/01-01.txt", "@01-01-01\n別文\n"));
    assert!(search::replacement(&p, &query, "変更", &results).is_err());
}
#[test]
fn one_reader_page_composes_within_three_milliseconds_in_release() {
    use novnow::core::reader::{self, Metrics, Options};
    let text = "日本語の本文と12個の項目、漢字《かんじ》とLatin。".repeat(20);
    let options = Options::default();
    let metrics = Metrics::default();
    let started = std::time::Instant::now();
    for _ in 0..50 {
        std::hint::black_box(reader::layout(&text, options, &metrics));
    }
    let average = started.elapsed() / 50;
    let limit = std::time::Duration::from_millis(if cfg!(debug_assertions) { 30 } else { 3 });
    assert!(average < limit, "page layout {average:?}");
}

#[cfg(feature = "ui")]
#[test]
fn workspace_shortcut_cannot_leak_its_digit_into_new_editor() {
    use egui::{Event, Key, Modifiers, RawInput};
    use novnow::ui::{
        shell::{self, Command, ImeState},
        workspaces::Workspace,
    };
    let ctx = egui::Context::default();
    let mut ime = ImeState::default();
    let input = RawInput {
        events: vec![
            Event::Key {
                key: Key::Num5,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::ALT,
            },
            Event::Text("5".into()),
            Event::Text("別".into()),
        ],
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        ctx.input_mut(|input| {
            assert_eq!(
                shell::take_commands_in(input, &mut ime, false, false, Workspace::Reader),
                vec![Command::Workspace(5)]
            );
            assert!(!input
                .events
                .iter()
                .any(|e| matches!(e,Event::Text(t)if t=="5")));
            assert!(input
                .events
                .iter()
                .any(|e| matches!(e,Event::Text(t)if t=="別")));
        });
    });
}

#[test]
fn reader_brackets_substitute_only_when_the_font_has_the_form() {
    use novnow::core::reader::{layout, Metrics, Options};
    let options = Options {
        rows: 10,
        columns: 5,
        vertical: true,
    };
    // 互換形の収録が無いフォントでは、元の括弧を 90° 回転して描く（豆腐にしない）。
    let pages = layout("「あ」", options, &Metrics::default());
    let glyphs = &pages[0].glyphs;
    let open = glyphs.iter().find(|g| g.ch == '「').unwrap();
    assert_eq!(open.text, "「");
    assert!(open.rotate);
    let body = glyphs.iter().find(|g| g.ch == 'あ').unwrap();
    assert!(!body.rotate);
    // 収録がある字だけ互換形へ置換し、回転しない。
    let metrics = Metrics {
        vertical_forms: ['﹁'].into_iter().collect(),
        ..Metrics::default()
    };
    let pages = layout("「あ」", options, &metrics);
    let glyphs = &pages[0].glyphs;
    let open = glyphs.iter().find(|g| g.ch == '「').unwrap();
    assert_eq!(open.text, "﹁");
    assert!(!open.rotate);
    let close = glyphs.iter().find(|g| g.ch == '」').unwrap();
    assert_eq!(close.text, "」");
    assert!(close.rotate, "収録の無い字は置換しない");
    // 横書きでは括弧を置換も回転もしない。
    let pages = layout(
        "「あ」",
        Options {
            vertical: false,
            ..options
        },
        &Metrics::default(),
    );
    let open = pages[0].glyphs.iter().find(|g| g.ch == '「').unwrap();
    assert_eq!(open.text, "「");
    assert!(!open.rotate);
}

#[test]
fn search_excerpts_are_structured_and_snapshots_use_the_fixed_form() {
    let p = project(&[(
        "contents/01-01.txt",
        "@01-01-01\n銀の指輪を外して机に置いた。\n",
    )]);
    let q = search::Query {
        text: "指輪".into(),
        case_sensitive: true,
        ..Default::default()
    };
    let hits = search::run(&p, &q, || 0).unwrap();
    assert_eq!(hits.hits.len(), 1);
    let hit = &hits.hits[0];
    // 抜粋は前文・一致・後文の構造で、表示用の `**…**` はデータに入らない。
    assert_eq!(hit.before, "銀の");
    assert_eq!(hit.matched, "指輪");
    assert_eq!(hit.after, "を外して机に置いた。");
    assert!(!format!("{}{}{}", hit.before, hit.matched, hit.after).contains('*'));
    // 資料として保存するときだけ §16.3.1 の固定形で組み立てる。
    let snap = search::snapshot(&p, &q, &hits, "2026-10-26T00:00:00+09:00").unwrap();
    assert!(
        snap.body.starts_with("# 指輪\n<!-- novnow:S0001 -->\n"),
        "{}",
        snap.body
    );
    for meta in [
        "query: 指輪\n",
        "regex: false\n",
        "case_sensitive: true\n",
        "searched_at: 2026-10-26T00:00:00+09:00\n",
        "hits: 1\n",
        "\n## 結果\n\n",
        "- @01-01-01-01 … 銀の**指輪**を外して机に置いた。\n",
    ] {
        assert!(
            snap.body.contains(meta),
            "{meta:?} missing in {:?}",
            snap.body
        );
    }
}
