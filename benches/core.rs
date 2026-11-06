//! `core` 層の性能目標ベンチマーク（`docs/specifications/workflows.md` §18.1 / §18.4）。
//!
//! CI の `bench` ジョブでは `cargo bench --locked --bench core -- --quick` で全ベンチマークが
//! パニックせず完走することを検証する。

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use novnow::core::export::{self, Options as ExportOptions};
use novnow::core::features::{hover, Project};
use novnow::core::id::SceneId;
use novnow::core::model::document::Document;
use novnow::core::model::project::{Lexicon, Project as Settings};
use novnow::core::model::role_pattern::RolePattern;
use novnow::core::resolve::dictionary::{Dictionary, Entry, EntryKind, Origin, SourceFile, Target};
use novnow::core::resolve::role_language::MatcherSet;
use novnow::core::syntax::contents;
use novnow::core::template;
use novnow::core::text::incremental::LineIndex;
use novnow::core::text::lexer::{self, TextFormat};

/// §18.1: 1 行編集時の再字句解析 + トークン分類（目標: 1ms 以内）。
fn bench_single_line_relex(c: &mut Criterion) {
    let mut lines = Vec::with_capacity(200);
    lines.push("@01-04-01 旧校舎の廊下".to_owned());
    for index in 0..199 {
        lines.push(format!(
            "廊下の先で'恵:先輩が立ち止まった（{index}行目）。「まだ帰らないの？」 恵"
        ));
    }
    let text = lines.join("\n");
    let mut index = LineIndex::new(&text, TextFormat::Contents);
    let edited = "廊下の先で'恵:先輩と'湊が立ち止まった。「もう帰ろうよ！」 恵";

    c.bench_function("single_line_relex", |b| {
        b.iter(|| {
            let line = black_box(edited);
            let kind = lexer::classify(line, TextFormat::Contents);
            let parsed = contents::parse(line);
            let renumber = index.relex_text(100, line);
            black_box((kind, parsed, renumber));
        });
    });
}

/// §18.1: 役割語パターン照合（1 行、10 パターン、目標: 20µs 以内）。
fn bench_role_language_10_patterns(c: &mut Criterion) {
    let raw_patterns = [
        "*だよ！",
        "*だね",
        "えへへ、*",
        "*かな？",
        "…あの…*",
        "*じゃない？",
        "*でしょう",
        "もう、*",
        "*(体言)！",
        "=ほう…",
    ];
    let patterns: Vec<RolePattern> = raw_patterns.into_iter().map(RolePattern::new).collect();
    let set = MatcherSet::new(&patterns, &Lexicon::default());
    let utterance = "えへへ、もう行く時間だよ！";

    c.bench_function("role_language_10_patterns", |b| {
        b.iter(|| {
            let spans = set.underlines(black_box(utterance));
            black_box(spans);
        });
    });
}

/// §18.1: 辞書トライ再構築（5,000 エントリ、目標: 10ms 以内）。
fn bench_dictionary_trie_5000(c: &mut Criterion) {
    let entries: Vec<Entry> = (0..5_000)
        .map(|index| {
            let key = format!("登場人物_{index:04}");
            Entry::new(
                EntryKind::Character,
                key.clone(),
                Target::Character { index, name: key },
                Origin {
                    file: SourceFile::Characters,
                    range: Some(index..index + 1),
                },
            )
        })
        .collect();

    c.bench_function("dictionary_trie_5000", |b| {
        b.iter(|| {
            let dict = Dictionary::build(black_box(entries.clone()));
            black_box(dict);
        });
    });
}

/// §18.1: 参照ペイン（カーソル移動時のプロット + ホバー再解決、目標: 16ms 以内）。
fn bench_reference_pane_rebuild(c: &mut Criterion) {
    let project = Project::with_documents(
        Settings::default(),
        template::sample_project()
            .into_iter()
            .map(|file| Document::new(file.path, file.text)),
    );
    let scene = SceneId {
        chapter: 1,
        episode: 4,
        scene: 1,
        section: 1,
    };
    let doc = project
        .document("contents/01-04.cnov")
        .expect("sample contents/01-04.cnov")
        .clone();
    let ref_pos = doc.text.find("'恵").unwrap_or(0) + 1;

    c.bench_function("reference_pane_rebuild", |b| {
        b.iter(|| {
            let plot = project.episode_plot(scene.chapter, scene.episode);
            let card = hover::at(black_box(&doc), black_box(ref_pos), black_box(&project));
            black_box((plot, card));
        });
    });
}

/// §18.1: エクスポート（10 万字、目標: 100ms 以内）。
fn bench_export_100k_chars(c: &mut Criterion) {
    let mut project = Project::with_documents(
        Settings::default(),
        template::sample_project()
            .into_iter()
            .map(|file| Document::new(file.path, file.text)),
    );
    // 10 万字相当の本文を生成する
    let mut body = String::with_capacity(320_000);
    body.push_str("@01-04-01 旧校舎の階段\n");
    let paragraph = "放課後の廊下に西日が差し込み、'恵:先輩は'旧校舎の階段を見上げた。「ここから先は立ち入り禁止だよ！」 恵\n";
    while body.chars().count() < 100_000 {
        body.push_str(paragraph);
        body.push_str(";\n");
    }
    project.insert(Document::new("contents/01-04.cnov", body));
    let options = ExportOptions::default();

    c.bench_function("export_100k_chars", |b| {
        b.iter(|| {
            let out = export::run(black_box(&project), None, black_box(&options));
            black_box(out);
        });
    });
}

// ============================================================ B13（§18.1 の単価目標）

/// B13 のベンチで使う合成プロジェクト（本文 12 場面 × 4 行／ファイル、話プロットも同数）。
fn synthetic_project(files: usize, writing: bool) -> Project {
    let mut docs = vec![
        Document::new("characters.md", "# アルス\nreading: あるす\n"),
        Document::new(
            "words.md",
            "# 星屑の時計\nreading: ほしづくのとけい\ntype: 語\n",
        ),
        Document::new("materials/index.json", "{}\n"),
    ];
    for index in 1..=files {
        let chapter = (index / 10) as u16 + 1;
        let episode = (index % 10) as u16 + 1;
        docs.push(Document::new(
            format!("contents/{chapter:02}-{episode:02}.cnov"),
            synthetic_contents(chapter, episode),
        ));
        docs.push(Document::new(
            format!("plots/{chapter:02}-{episode:02}.cnov"),
            synthetic_episode(chapter, episode),
        ));
    }
    let mut settings = Settings::default();
    if writing {
        settings
            .features
            .diagnostics
            .insert("writing".to_owned(), "warn".to_owned());
    }
    Project::with_documents(settings, docs)
}

fn synthetic_contents(chapter: u16, episode: u16) -> String {
    let mut out = String::new();
    for scene in 1..=12 {
        out.push_str(&format!(
            "@0{chapter}-{episode:02}-{scene:02} 場面{scene}\n"
        ));
        for _ in 0..2 {
            out.push_str("「やっぱり'アルス、ここにいたのね」@リリア:A\n\n");
            out.push_str("'リリアは溜め息をついて、'アルス の顔を覗き込んだ。\n");
        }
        out.push_str(";\n");
    }
    out
}

fn synthetic_episode(chapter: u16, episode: u16) -> String {
    let mut out = String::new();
    for scene in 1..=12 {
        out.push_str(&format!(
            "@0{chapter}-{episode:02}-{scene:02} 場面{scene}\n"
        ));
        out.push_str("cast: 'アルス 'リリア\nwhen: 406 09:40\nspace-mode: ascii\n");
        out.push_str("'アルス は遺跡の謎を解こうとしている。\nsenses: 視覚\n;\n");
    }
    out
}

/// §18.1: 診断コードの属性 1 回の参照（目標: 5ns 未満。コード数に依存しないこと）。
fn bench_diagnostic_attribute_lookup(c: &mut Criterion) {
    let codes: Vec<novnow::core::diagnostics::DiagnosticCode> = novnow::core::diagnostics::CATALOG
        .iter()
        .map(|row| row.0)
        .collect();
    c.bench_function("diagnostic_attribute_lookup", |b| {
        b.iter(|| {
            let mut sum = 0u32;
            for code in &codes {
                let attributes = black_box(*code).attributes();
                sum += u32::from(attributes.params);
                sum += u32::from(attributes.trigger as u8);
                sum += u32::from(attributes.anchor as u8);
                sum += u32::from(attributes.scope as u8);
            }
            black_box(sum);
        });
    });
}

/// §18.1: 診断 1,000 件の重複除去（目標: 件数に線形であること。`document()` 経由で測る）。
fn bench_diagnostics_dedup_1000(c: &mut Criterion) {
    let mut text = String::new();
    for scene in 1..=250u16 {
        text.push_str(&format!("@01-01-{scene:03} 場面\n"));
        for index in 0..4 {
            text.push_str(&format!("'未登録{scene}_{index} が来た。\n"));
        }
        text.push_str(";\n");
    }
    let project = synthetic_project(1, false);
    let path = project
        .documents()
        .find(|doc| doc.path.starts_with("contents/"))
        .map(|doc| doc.path.clone())
        .unwrap_or_default();
    project.documents().count();
    let mut docs: Vec<Document> = project.documents().cloned().collect();
    docs.retain(|doc| doc.path != path);
    docs.push(Document::new(path.clone(), text));
    let project = Project::with_documents(Settings::default(), docs);
    let doc = project.document(&path).cloned().expect("contents document");
    c.bench_function("diagnostics_dedup_1000", |b| {
        b.iter(|| {
            let found = novnow::core::diagnostics::document(black_box(&doc), black_box(&project));
            black_box(found.len());
        });
    });
}

/// §18.1: ファイル別索引からの辞書診断の取得（2,000 件。目標: 1 回 1µs 未満）。
fn bench_dictionary_diagnostics_index(c: &mut Criterion) {
    let mut words = String::new();
    for index in 0..2_000 {
        words.push_str(&format!("# キー{index}\nreading: きー{index}\n\n"));
    }
    let project = Project::with_documents(
        Settings::default(),
        vec![
            Document::new("characters.md", "# アルス\nreading: あるす\n"),
            Document::new("words.md", words),
            Document::new("materials/index.json", "{}\n"),
        ],
    );
    c.bench_function("dictionary_diagnostics_index", |b| {
        b.iter(|| {
            let found = project.dictionary_diagnostics(SourceFile::Words);
            black_box(found.len());
        });
    });
}

/// §18.1: 2 回目の節の並びの取得（100 ファイル・2,300 節。目標: 0.2ms 未満）。
fn bench_sections_all_cached(c: &mut Criterion) {
    let project = synthetic_project(100, false);
    let _ = novnow::core::features::sections::all(&project);
    c.bench_function("sections_all_cached", |b| {
        b.iter(|| {
            let list = novnow::core::features::sections::all(black_box(&project));
            black_box(list.len());
        });
    });
}

/// §18.1: 書き戻しの後の `Workbench` の再構築（100 ファイル。目標: 3ms 未満）。
fn bench_workbench_rebuild_100_files(c: &mut Criterion) {
    let project = synthetic_project(100, false);
    let _ = project.sections();
    let _ = project.section_chars();
    let mut bench = novnow::app::workbench::Workbench::new(project, None, 0);
    c.bench_function("workbench_rebuild_100_files", |b| {
        b.iter(|| {
            bench.rebuild();
            black_box(bench.sections().len());
        });
    });
}

/// §18.1: 2 回目のプロジェクト全体メタデータの取得（100 ファイル。目標: 20µs 未満）。
fn bench_metadata_records_cached(c: &mut Criterion) {
    let project = synthetic_project(100, false);
    let _ = novnow::core::features::metadata::records(&project);
    c.bench_function("metadata_records_cached", |b| {
        b.iter(|| {
            let rows = novnow::core::features::metadata::records(black_box(&project));
            black_box(rows.len());
        });
    });
}

/// §18.1: W096 / W097 有効時の本文 1 ファイルの診断（100 ファイル。目標: ファイル数に依存しない）。
fn bench_project_diagnostics_100_files(c: &mut Criterion) {
    let project = synthetic_project(100, true);
    let path = project
        .documents()
        .find(|doc| doc.path.starts_with("contents/"))
        .map(|doc| doc.path.clone())
        .unwrap_or_default();
    let doc = project.document(&path).cloned().expect("contents document");
    let _ = novnow::core::diagnostics::document(&doc, &project);
    c.bench_function("project_diagnostics_100_files", |b| {
        b.iter(|| {
            let found = novnow::core::diagnostics::document(black_box(&doc), black_box(&project));
            black_box(found.len());
        });
    });
}

criterion_group!(
    benches,
    bench_single_line_relex,
    bench_role_language_10_patterns,
    bench_dictionary_trie_5000,
    bench_reference_pane_rebuild,
    bench_export_100k_chars,
    bench_diagnostic_attribute_lookup,
    bench_diagnostics_dedup_1000,
    bench_dictionary_diagnostics_index,
    bench_sections_all_cached,
    bench_workbench_rebuild_100_files,
    bench_metadata_records_cached,
    bench_project_diagnostics_100_files,
);
criterion_main!(benches);
