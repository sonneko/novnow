use novnow::core::{
    annotations, export,
    features::{local_llm, metadata, metrics, search, stock},
    features::{sections, Project},
    id::SceneId,
    model::document::Document,
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
    let p = project(&[("contents/01-01.cnov", text)]);
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
            "plots/01.cnov",
            "@01-00 あらすじ\nblurb: 章の表紙\n;\n@01-01 本編\n",
        ),
        ("contents/01-01.cnov", "@01-01-01\n本編。\n"),
        ("contents/00-01.cnov", "@00-01-01\nストック。\n"),
        ("contents/01-00.cnov", "@01-00-01\n本文ではない。\n"),
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
    assert!(metadata::asset_path("plots/map.cnov", true).is_none());
    assert_eq!(
        metadata::asset_path("./assets/icons/a.png", true).as_deref(),
        Some("assets/icons/a.png")
    );
}
#[test]
fn search_regex_case_deadline_replacement_and_snapshots() {
    let p = project(&[("contents/01-01.cnov", "@01-01-01\nApple apple\n")]);
    // 既定は大小文字を区別する（§9.7.4）
    let sensitive = search::Query {
        text: "apple".into(),
        ..Default::default()
    };
    assert_eq!(search::run(&p, &sensitive, || 0).unwrap().hits.len(), 1);
    let q = search::Query {
        case_sensitive: false,
        ..sensitive.clone()
    };
    let hits = search::run(&p, &q, || 0).unwrap();
    assert_eq!(hits.hits.len(), 2);
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
    // §9.4.3: 読点 0 個で `long_sentence_chars` を超える文の、優先度 1 の接続助詞の直後だけ。
    let text = "長い文章は長い文章が長い文章を長い文章に長い文章で長い文章と長い文章は長い文章が長い文章を";
    let options = metrics::CommaOptions::long(20);
    let positions = metrics::comma_candidates(text, options);
    assert_eq!(positions, metrics::comma_candidates(text, options));
    assert_eq!(positions.len(), 3);
    assert!(positions
        .iter()
        .all(|at| text[..*at].ends_with('が') || text[..*at].ends_with('と')));
    assert!(metrics::comma_candidates(&format!("「{text}」"), options).is_empty());
    // 短い文（閾値以下）は候補を出さない。
    assert!(
        metrics::comma_candidates("短いが、そうでもない", metrics::CommaOptions::long(120))
            .is_empty()
    );
    // 既に読点がある文は対象外。
    assert!(metrics::comma_candidates(
        "長い文章は長い、文章が長い文章を長い文章に長い文章で長い文章と長い文章は長い文章が",
        metrics::CommaOptions::long(10)
    )
    .is_empty());
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
        ("contents/00-01.cnov", "@00-01-01\n本文\n"),
        ("plots/00-01.cnov", "@00-01-01\nsummary: plot\n"),
        ("ideas.md", "# I0001\nrelates: @00-01-01-01\n"),
    ]);
    let plan = stock::assign(&p, 1, 2).unwrap();
    assert_eq!(plan.renames[0].1, "contents/02-01.cnov");
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
    let p = project(&[("contents/01-01.cnov", "@01-01-01\n原文\n")]);
    let mut bench = Workbench::new(p, Some("01-01-01-01"), 400);
    let before = bench.project().text("contents/01-01.cnov").to_owned();
    bench.toggle_human("2026-10-21T00:00:00Z", 0.0);
    bench.buffer_mut().push_str("提案");
    bench.edited(1.0);
    bench.commit();
    assert_eq!(bench.project().text("contents/01-01.cnov"), before);
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
        .any(|f| f.path == "contents/01-01.cnov" && f.text.contains("原文提案")));
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
        "project.toml",
        "schema_version = 2\n\n[agents]\ncustom_roles = [\"observer\"]\n",
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
            "plots/01-01.cnov",
            "@01-01-01 title\nillustration: plot memo\n",
        ),
        ("contents/01-01.cnov", "@01-01-01 title\n文\n! body memo\n"),
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
        ("contents/01-01.cnov", "@01-01-01\n原文\n"),
        ("contents/01-02.cnov", "@01-02-01\n原文\n"),
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
    for path in ["contents/01-01.cnov", "contents/01-02.cnov"] {
        assert!(bench.project().text(path).contains("原文"));
    }
    assert!(bench.undo_workspace(true, 3.0));
    for path in ["contents/01-01.cnov", "contents/01-02.cnov"] {
        assert!(bench.project().text(path).contains("変更"));
    }
}

#[test]
fn stale_search_cannot_replace_unrelated_current_text() {
    let mut p = project(&[("contents/01-01.cnov", "@01-01-01\n原文\n")]);
    let query = search::Query {
        text: "原文".into(),
        ..Default::default()
    };
    let results = search::run(&p, &query, || 0).unwrap();
    p.insert(Document::new("contents/01-01.cnov", "@01-01-01\n別文\n"));
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
        "contents/01-01.cnov",
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

#[test]
fn search_snapshot_slug_summary_and_rerun_diff_workflow() {
    let mut p = project(&[(
        "contents/01-01.cnov",
        "@01-01-01\n銀の指輪を外した。\n;\n金の指輪を見つめた。\n",
    )]);
    let q = search::Query {
        text: "指輪".into(),
        case_sensitive: true,
        ..Default::default()
    };
    let hits = search::run(&p, &q, || 0).unwrap();
    let created = search::snapshot_with(
        &p,
        &q,
        &hits,
        "2026-11-12T10:00:00+09:00",
        "指輪の登場箇所",
        "伏線チェック用の固定結果",
    )
    .unwrap();
    assert_eq!(created.id, "S0001");
    assert_eq!(created.path, "materials/docs/S0001-指輪の登場箇所.md");
    let index_json: serde_json::Value = serde_json::from_str(&created.index).unwrap();
    assert_eq!(
        index_json["items"][0]["summary"],
        "伏線チェック用の固定結果"
    );

    p.insert(Document::new(&created.path, &created.body));
    p.insert(Document::new("materials/index.json", &created.index));

    // 本文を編集して 1 節目のヒットを消し、3 節目に新しいヒットを足す
    p.insert(Document::new(
        "contents/01-01.cnov",
        "@01-01-01\n銀の腕輪を外した。\n;\n金の指輪を見つめた。\n;\n銅の指輪を拾った。\n",
    ));
    let new_hits = search::run(&p, &q, || 0).unwrap();
    let diff = search::rerun_diff(
        &p,
        p.text(&created.path),
        "S0001",
        "指輪の登場箇所",
        &q,
        &new_hits,
        "2026-11-12T12:30:00+09:00",
    );
    assert_eq!(
        diff.unchanged_count, 1,
        "created.body={:?}\ndiff={diff:?}",
        created.body
    );
    assert_eq!(diff.added.len(), 1);
    assert!(diff.added[0].contains("@01-01-01-03") && diff.added[0].contains("銅の**指輪**"));
    assert_eq!(diff.removed.len(), 1);
    assert!(diff.removed[0].contains("@01-01-01-01") && diff.removed[0].contains("銀の**指輪**"));

    // 再検索しただけでは保存済み本文は変わらない
    assert_eq!(p.text(&created.path), created.body);

    // 人間が更新適用を選んだときのみ本文と index.json の updated_at / searched_at が更新される
    let updated_body = diff.new_body.clone();
    let updated_index = search::apply_rerun(
        p.text("materials/index.json"),
        "S0001",
        "2026-11-12T12:30:00+09:00",
    )
    .unwrap();
    assert!(updated_body.contains("searched_at: 2026-11-12T12:30:00+09:00\n"));
    assert!(updated_body.contains("@01-01-01-03 … 銅の**指輪**"));
    assert!(!updated_body.contains("銀の**指輪**"));
    let updated_json: serde_json::Value = serde_json::from_str(&updated_index).unwrap();
    assert_eq!(
        updated_json["items"][0]["updated_at"],
        "2026-11-12T12:30:00+09:00"
    );
}

#[test]
fn stock_bidirectional_assignment_and_collision_errors() {
    use novnow::core::features::stock::StockError;

    let mut p = project(&[
        (
            "project.toml",
            "[project]\ntitle = \"t\"\n[structure]\nallow_chapter_zero = true\n",
        ),
        (
            "plots/00.cnov",
            "@00-01 秘蔵の幕間\nsummary: ストックの要約\nupdated: 2026-11-10\n",
        ),
        ("contents/00-01.cnov", "@00-01-01\n幕間の本文です。\n"),
        (
            "plots/00-01.cnov",
            "@00-01-01\n; @00-01-01-01\nsummary: 幕間節\n",
        ),
        ("contents/02-01.cnov", "@02-01-01\n既存の二章一話。\n"),
        ("ideas.md", "# I0001\nrelates: @00-01-01-01\n"),
    ]);

    let items = stock::list(&p);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].episode, 1);
    assert_eq!(items[0].summary, "ストックの要約");
    assert!(items[0].chars > 0);
    assert_eq!(items[0].updated_at, "2026-11-10");

    // 既存の 02-01 への配属は衝突エラーで中止される
    assert!(matches!(
        stock::move_episode(&p, 0, 1, 2, 1),
        Err(StockError::DestinationExists(_))
    ));

    // 空いている 02-05 へ章・話番号指定で配属する
    let assigned = stock::move_episode(&p, 0, 1, 2, 5).unwrap();
    assert_eq!(assigned.target_chapter, 2);
    assert_eq!(assigned.target_episode, 5);
    assert!(assigned
        .renames
        .contains(&("contents/00-01.cnov".into(), "contents/02-05.cnov".into())));
    for file in &assigned.edit.changes {
        let doc = p
            .document(&file.path)
            .cloned()
            .unwrap_or_else(|| Document::new(&file.path, ""));
        let mut text = doc.text.to_string();
        let mut edits = file.edits.clone();
        edits.sort_by(|a, b| b.range.start.cmp(&a.range.start));
        for e in edits {
            text.replace_range(e.range, &e.new_text);
        }
        p.insert(Document::new(&file.path, text));
    }
    for (from, to) in &assigned.renames {
        let body = p.text(from).to_owned();
        p.remove(from);
        p.insert(Document::new(to, body));
    }
    assert!(p.text("ideas.md").contains("@02-05-01-01"));

    // 正式な話 02-05 をストックへ戻す（00-01 へリネーム＋参照一括更新）
    let returned = stock::return_to_stock(&p, 2, 5).unwrap();
    assert_eq!(returned.target_chapter, 0);
    assert_eq!(returned.target_episode, 1);
    assert!(returned
        .renames
        .contains(&("contents/02-05.cnov".into(), "contents/00-01.cnov".into())));
}

#[test]
fn episode_section_swap_and_outline_three_tier_stock_last() {
    use novnow::core::features::plots;
    use novnow::core::id::SceneId;

    let p = project(&[
        (
            "project.toml",
            "[project]\ntitle = \"t\"\n[structure]\nallow_chapter_zero = true\n",
        ),
        ("plots/all.cnov", "@00\n;\n@01 第一章\n;\n@02 第二章\n"),
        ("plots/01.cnov", "@01-01 旅立ち\n"),
        (
            "plots/01-01.cnov",
            "@01-01-01 宿屋\nspace: 木造二階建ての宿屋\n朝の支度\nwhen: 第一日 朝\n;\n出発\nwhen: 第一日 昼\nspace: 宿屋の玄関前\n",
        ),
        (
            "contents/01-01.cnov",
            "@01-01-01\n朝の本文。\n;\n昼の本文。\n",
        ),
        ("contents/00-01.cnov", "@00-01-01\nストックの本文。\n"),
    ]);

    // 1. 構成ツリーは 3 階層（章 → 話 → 場面）で、ストック章 00 が最後尾に並ぶ
    let tree = plots::outline_tree(&p);
    let chapter_order: Vec<u16> = tree.iter().map(|c| c.chapter).collect();
    assert_eq!(chapter_order, vec![1, 2, 0], "ストック章 00 は常に最後尾");
    assert_eq!(tree[0].episode_count, 1);
    assert_eq!(tree[0].episodes[0].section_count, 2);
    assert_eq!(tree[0].episodes[0].scenes.len(), 1);
    assert_eq!(tree[0].episodes[0].scenes[0].title, "宿屋");

    // 2. episode_rows の when 抽出と space_lines
    let rows = plots::episode_rows(&p, 1, 1);
    assert_eq!(rows[1].when.as_deref(), Some("第一日 朝"));
    assert_eq!(rows[2].when.as_deref(), Some("第一日 昼"));
    let spaces = plots::space_lines(
        &p,
        SceneId {
            chapter: 1,
            episode: 1,
            scene: 1,
            section: 2,
        },
    );
    assert!(
        spaces.iter().any(|l| l.contains("宿屋の玄関前")),
        "節の space: が優先して取得される: {spaces:?}"
    );
    let scene_spaces = plots::space_lines(
        &p,
        SceneId {
            chapter: 1,
            episode: 1,
            scene: 1,
            section: 1,
        },
    );
    assert!(
        scene_spaces
            .iter()
            .any(|l| l.contains("木造二階建ての宿屋")),
        "節に space: が無ければ場面の space: を継承する: {scene_spaces:?}"
    );

    // 3. 節入替（本文連動あり）
    let s1 = SceneId {
        chapter: 1,
        episode: 1,
        scene: 1,
        section: 1,
    };
    let s2 = SceneId {
        chapter: 1,
        episode: 1,
        scene: 1,
        section: 2,
    };
    assert!(plots::has_prose_for_swap(&p, s1, s2));
    let edit = plots::swap_episode_sections(&p, s1, s2, true).unwrap();
    assert_eq!(edit.changes.len(), 2);
    let plot_change = edit
        .changes
        .iter()
        .find(|f| f.path == "plots/01-01.cnov")
        .unwrap();
    assert!(plot_change.edits[0].new_text.contains("出発"));
    assert!(plot_change.edits[1].new_text.contains("朝の支度"));
    let prose_change = edit
        .changes
        .iter()
        .find(|f| f.path == "contents/01-01.cnov")
        .unwrap();
    assert!(prose_change.edits[0].new_text.contains("昼の本文。"));
    assert!(prose_change.edits[1].new_text.contains("朝の本文。"));
}

#[test]
fn mode_specific_addresses_candidate_extraction_and_violations_work() {
    let chars = "# アルス\naddress: リリア => リリア\n## modes\n### formal\naddress: リリア => リリア様\n# リリア\n# 佐倉\n";
    // モード別の更新・新規モード挿入・空文字による削除
    let updated =
        metadata::set_address_with_mode(chars, "アルス", Some("formal"), "リリア", "王女殿下")
            .unwrap();
    assert!(updated.contains("address: リリア => リリア\n"));
    assert!(updated.contains("### formal\naddress: リリア => 王女殿下\n"));

    let inserted_mode =
        metadata::set_address_with_mode(&updated, "リリア", Some("battle"), "アルス", "貴様")
            .unwrap();
    assert!(inserted_mode.contains("## modes\n### battle\naddress: アルス => 貴様\n"));

    let cleared =
        metadata::set_address_with_mode(&inserted_mode, "リリア", Some("battle"), "アルス", "   ")
            .unwrap();
    assert!(!cleared.contains("address: アルス => 貴様"));

    // 本文からの呼称候補抽出（自動では characters.md を書き換えない）と W084 ペア違反
    let mut project = Project::default();
    project.insert(Document::new("characters.md", chars));
    project.insert(Document::new(
        "contents/01-01.cnov",
        "@01-01-01\n「リリア、佐倉先生を呼んでこい」@アルス:formal\n「アルスくん、待ってよ」@リリア\n",
    ));
    let violations = metadata::address_violations(&project);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].speaker, "アルス");
    assert_eq!(violations[0].mode.as_deref(), Some("formal"));
    assert_eq!(violations[0].target, "リリア");

    let cands = metadata::extract_address_candidates(&project);
    assert!(
        cands
            .iter()
            .any(|c| c.speaker == "アルス" && c.target == "佐倉" && c.surface == "佐倉先生"),
        "cands={cands:?}"
    );
    assert!(
        cands
            .iter()
            .any(|c| c.speaker == "リリア" && c.target == "アルス" && c.surface == "アルスくん"),
        "cands={cands:?}"
    );
    assert_eq!(project.text("characters.md"), chars);
}

#[test]
fn timeline_inheritance_flashback_senses_streaks_speakers_and_length_metrics_work() {
    let mut project = Project::default();
    project.insert(Document::new(
        "characters.md",
        "# アルス\nreading: あるす\n# リリア\nreading: りりあ\n",
    ));
    project.insert(Document::new(
        "plots/all.cnov",
        "@01 第一章\nsummary: 導入\nwhen: 405\n;\n@02 第二章\nsummary: 展開\n;\n",
    ));
    project.insert(Document::new(
        "plots/01.cnov",
        "@01-01 朝\nsummary: 登校\n;\n@01-02 昼\nsummary: 昼休み\nwhen: 410\n;\n@01-03 夕\nsummary: 巻き戻りテスト\nwhen: 408\n;\n@01-04 記憶\nsummary: 幼少期\nwhen: 100 回想\n;\n@01-05 未定\nsummary: 謎の幕間\nwhen: ?\n;\n",
    ));
    project.insert(Document::new(
        "plots/01-01.cnov",
        "@01-01-01 通学路\ncast: 'アルス 'リリア\npov: 'アルス\nsenses: 視覚\nlength: 20\n;\nsenses: 視覚\nlength: 10\n;\nsenses: 視覚\n;\nsenses: 視覚\n;\nsenses: 視覚\n;\nsenses: 聴覚\n;\n",
    ));
    project.insert(Document::new(
        "plots/02-01.cnov",
        "@02-01-01 別章\npov: 'リリア\n;\n",
    ));
    project.insert(Document::new(
        "contents/01-01.cnov",
        "@01-01-01\n「おはよう」@アルス\n朝の光が差す。長い文をここに書いて読点の無いまま終端まで伸ばす。\n;\n「ええ」@リリア\n;\n「誰の声だろう」\n;\n四節目の短い文。\n;\n五節目の短い文。\n;\n六節目の音が響く。\n",
    ));
    project.insert(Document::new(
        "contents/02-01.cnov",
        "@02-01-01\n「二章の台詞よ」@リリア\n二章の地の文。\n",
    ));

    // 1. 時系列: 章 01 (when: 405) を 01-01 が継承し、01-03 (408 < 410) に rewind_from が付き、01-04 (回想) には付かず、`?` は末尾
    let tl = metadata::timeline_view(&metadata::records(&project));
    let ep0101 = tl
        .iter()
        .find(|e| e.record.depth == 2 && e.record.id.episode == 1)
        .expect("01-01 inherited");
    assert!(ep0101.inherited);
    assert_eq!(ep0101.time.start, Some(405.0));

    let ep0103 = tl
        .iter()
        .find(|e| e.record.depth == 2 && e.record.id.episode == 3)
        .expect("01-03");
    assert_eq!(ep0103.rewind_from.map(|id| id.episode), Some(2));

    let ep0104 = tl
        .iter()
        .find(|e| e.record.depth == 2 && e.record.id.episode == 4)
        .expect("01-04 flashback");
    assert!(ep0104.time.flashback);
    assert!(ep0104.rewind_from.is_none());
    assert!(tl.last().unwrap().time.start.is_none());

    // 2. 五感: 全体では 7 節中 6 節宣言・1 節未宣言、視覚 5 連続区間が 1 件、章 02 切替では未宣言 1 節のみ
    let senses_all = metadata::senses_summary(&project, metadata::MetricsScope::All);
    assert_eq!(senses_all.total_sections, 7);
    assert_eq!(senses_all.declared_sections, 6);
    assert_eq!(senses_all.undeclared_sections, 1);
    assert_eq!(senses_all.counts[metadata::SenseKind::Sight as usize], 5);
    assert_eq!(senses_all.counts[metadata::SenseKind::Hearing as usize], 1);
    assert_eq!(senses_all.visual_streaks.len(), 1);
    assert_eq!(senses_all.visual_streaks[0].count, 5);

    let senses_ch2 = metadata::senses_summary(&project, metadata::MetricsScope::Chapter(2));
    assert_eq!(senses_ch2.total_sections, 1);
    assert_eq!(senses_ch2.declared_sections, 0);
    assert_eq!(senses_ch2.undeclared_sections, 1);

    // 3. 話者別: 登場節数・視点節数・話者不明・範囲切替
    let sp_all = metrics::speakers_in_scope(&project, metadata::MetricsScope::All);
    let ars = sp_all.rows.iter().find(|r| r.name == "アルス").unwrap();
    let lilia = sp_all.rows.iter().find(|r| r.name == "リリア").unwrap();
    let unknown = sp_all.rows.iter().find(|r| r.name.is_empty()).unwrap();
    assert_eq!(ars.lines, 1);
    assert_eq!(ars.sections, 6); // 場面 01-01-01 の cast: 'アルス を 6 節継承
    assert_eq!(ars.pov_sections, 6);
    assert_eq!(lilia.lines, 2); // 1 章 + 2 章
    assert_eq!(lilia.pov_sections, 1); // 2 章のみ
    assert_eq!(unknown.lines, 1); // 3 節目の話者不明台詞を除外しない

    let sp_ch2 = metrics::speakers_in_scope(&project, metadata::MetricsScope::Chapter(2));
    let ars_ch2 = sp_ch2.rows.iter().find(|r| r.name == "アルス").unwrap();
    assert_eq!(ars_ch2.lines, 0);
    assert_eq!(ars_ch2.sections, 0);

    // 4. 節長: 中央値・平均・最長・最短・目標比
    let len_rep = metrics::section_length_report(&project);
    assert_eq!(len_rep.summary.count, 7);
    assert!(len_rep.summary.max >= len_rep.summary.min && len_rep.summary.min > 0);
    assert_eq!(len_rep.rows[0].target, Some(20));
    assert!(len_rep.rows[0].target_ratio.is_some());

    // 5. 最長連続執筆日数: 過去に 5 日連続、直近は 2 日連続
    let mut stats = metrics::Stats::default();
    for d in [
        "2026-09-01",
        "2026-09-02",
        "2026-09-03",
        "2026-09-04",
        "2026-09-05",
        "2026-10-10",
        "2026-10-11",
    ] {
        stats.record(d, 21, 300, 600, true);
    }
    let recent = metrics::recent_dates("2026-10-11", 60);
    assert_eq!(stats.streak(&recent), 2);
    assert_eq!(stats.longest_streak(), 5);
}

#[test]
fn human_proposal_discard_preserves_history_and_external_change_marks_stale() {
    use novnow::app::workbench::Workbench;
    use novnow::core::agents::{apply, files};

    let mut project = Project::default();
    project.insert(Document::new(
        "contents/01-01.cnov",
        "@01-01-01\n元の本文です。\n",
    ));
    let mut bench = Workbench::new(project, None, 2000);

    // 提案モードで H0001 と H0002 を作成
    bench.toggle_human("2026-11-12T10:00:00Z", 1.0);
    *bench.buffer_mut() = "提案1の本文です。".to_owned();
    bench.toggle_human("2026-11-12T10:01:00Z", 2.0);

    bench.toggle_human("2026-11-12T10:02:00Z", 3.0);
    *bench.buffer_mut() = "提案2の本文です。".to_owned();
    bench.toggle_human("2026-11-12T10:03:00Z", 4.0);

    // 1. H0001 を破棄してもファイルは残っており、status: discarded と before/after が保持される
    let plan = apply::discard(bench.project(), "H0001", "2026-11-12T10:05:00Z").unwrap();
    for w in plan.writes {
        bench.replace_document(&w.path, w.text, 5.0);
    }
    let h1_doc = bench
        .project()
        .document("agents/proposals/H0001.md")
        .expect("discarded proposal file must remain");
    let h1 = files::parse_proposal(&h1_doc.text).unwrap();
    assert_eq!(h1.status, "discarded");
    assert_eq!(h1.before, "元の本文です。");
    assert_eq!(h1.after, "提案1の本文です。");

    // 2. 外部変更で本文が変わると H0002 は status: stale になり、本文は上書きされず W093 が出る
    bench.external_change(
        "contents/01-01.cnov",
        Some("@01-01-01\n外部エディタで書き換えた本文。\n".to_owned()),
    );
    assert!(bench
        .project()
        .text("contents/01-01.cnov")
        .contains("外部エディタで書き換えた本文。"));
    let h2_doc = bench
        .project()
        .document("agents/proposals/H0002.md")
        .expect("H0002 exists");
    let h2 = files::parse_proposal(&h2_doc.text).unwrap();
    assert_eq!(h2.status, "stale");
    assert!(
        apply::accept(bench.project(), "H0002", None, "2026-11-12T10:10:00Z").is_err(),
        "stale proposal must never overwrite manuscript"
    );
    let diags = novnow::core::diagnostics::document(h2_doc, bench.project());
    assert!(
        diags
            .iter()
            .any(|d| d.code == novnow::core::diagnostics::DiagnosticCode::W093),
        "W093 must be emitted for stale proposal: {diags:?}"
    );
}
