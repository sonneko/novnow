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

criterion_group!(
    benches,
    bench_single_line_relex,
    bench_role_language_10_patterns,
    bench_dictionary_trie_5000,
    bench_reference_pane_rebuild,
    bench_export_100k_chars,
);
criterion_main!(benches);
