//! プロジェクトの読み込み（§4.3・§14.2）と外部変更（§8.5）。
//!
//! - `core::model::project::validate`: v0 の目印 → 必須パス → `schema_version` → 文字コード（第 2.9 版）。
//! - `adapters::project_load`: ディスク上のプロジェクトを読み、M4 の診断がメモリ上と同じ結果になる
//!   （M5 の完了時に動くもの「`novnow check` が実プロジェクトで動く」の土台）。
//! - `core::policy::external_change`: ポーリングの候補と反応（Clean は再読込・Dirty は衝突・辞書は常に再読込）。

mod sandbox;
mod world;

use std::collections::BTreeSet;
use std::path::PathBuf;

use novnow::adapters::block_on;
use novnow::adapters::project_load::{is_document, listing, load, poll};
use novnow::core::diagnostics::{self, DiagnosticCode, Severity};
use novnow::core::model::project::{schema_of, validate, Listing, ProjectError, SchemaState};
use novnow::core::policy::external_change::{
    is_watched, notice, react, Buffer, ChangeKind, Reaction, Snapshot,
};
use novnow::core::policy::{FileEntry, ProjectPath, Stamp};
use novnow::ports::storage::Storage;

use sandbox::{open, FakeEnv, TempDir};

fn path(text: &str) -> ProjectPath {
    ProjectPath::new(text).expect("相対パス")
}

fn complete() -> Listing {
    Listing {
        files: [
            "project.json",
            "characters.md",
            "words.md",
            "materials/index.json",
            "plots/all.txt",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        dirs: ["contents", "plots", "materials"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        project_json: Some("{\"schema_version\":1}".to_owned()),
        ..Listing::default()
    }
}

// ============================================================ §4.3 の判定

#[test]
fn a_complete_project_is_valid() {
    assert_eq!(validate(&complete()), Ok(()));
}

#[test]
fn every_missing_path_is_listed_in_the_order_of_the_table() {
    // §14.2-2「最初の 1 件で止めず、不足を全部集める」
    let found = validate(&Listing::default());
    let want: Vec<PathBuf> = [
        "project.json",
        "characters.md",
        "words.md",
        "materials/index.json",
        "plots/all.txt",
        "contents/",
        "plots/",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    assert_eq!(found, Err(ProjectError::Incomplete { missing: want }));
}

#[test]
fn the_v0_marker_comes_before_the_missing_list() {
    // §14.2.4 の「v0 形式です（characters.json を検出）」を出すため（§4.3 第 2.9 版）
    let mut v0 = complete();
    v0.files.remove("characters.md");
    v0.files.remove("words.md");
    v0.files.insert("characters.json".to_owned());
    v0.files.insert("words.json".to_owned());
    assert_eq!(
        validate(&v0),
        Err(ProjectError::LegacyV0 {
            detected: "characters.json".to_owned()
        })
    );
    // characters.md もあれば目印にしない（v1 として判定を続ける）
    let mut both = complete();
    both.files.insert("characters.json".to_owned());
    assert_eq!(validate(&both), Ok(()));
}

#[test]
fn the_schema_version_is_checked_after_the_paths() {
    let with = |json: &str| Listing {
        project_json: Some(json.to_owned()),
        ..complete()
    };
    let legacy = Err(ProjectError::LegacyV0 {
        detected: "project.json".to_owned(),
    });
    assert_eq!(validate(&with("{}")), legacy);
    assert_eq!(validate(&with("{\"schema_version\":0}")), legacy);
    assert_eq!(
        validate(&with("{\"schema_version\":2}")),
        Err(ProjectError::TooNew { found: 2 })
    );
    // 版が読めないときは飛ばす（E025 を出して既定値で開く、§4.3 第 2.9 版）
    assert_eq!(validate(&with("{")), Ok(()));
    assert_eq!(validate(&with("{\"schema_version\":\"1\"}")), Ok(()));
    assert_eq!(validate(&with("[1]")), Ok(()));
    // 必須パスの欠落が先
    let mut missing = with("{\"schema_version\":2}");
    missing.files.remove("words.md");
    assert!(matches!(
        validate(&missing),
        Err(ProjectError::Incomplete { .. })
    ));
}

#[test]
fn schema_of_reads_only_the_version() {
    assert_eq!(
        schema_of("{\"schema_version\":1}"),
        Some(SchemaState::Current)
    );
    assert_eq!(
        schema_of("\u{feff}{\"schema_version\":1}"),
        Some(SchemaState::Current)
    );
    assert_eq!(schema_of("{\"title\":\"x\"}"), Some(SchemaState::LegacyV0));
    assert_eq!(
        schema_of("{\"schema_version\":5000000000}"),
        Some(SchemaState::TooNew { found: u32::MAX })
    );
    assert_eq!(schema_of("{\"schema_version\":-1}"), None);
    assert_eq!(schema_of("{\"schema_version\":1.5}"), None);
}

#[test]
fn invalid_utf8_is_reported_last() {
    let listing = Listing {
        invalid_utf8: vec!["contents/01-01.txt".to_owned()],
        ..complete()
    };
    assert_eq!(
        validate(&listing),
        Err(ProjectError::Encoding {
            files: vec![PathBuf::from("contents/01-01.txt")]
        })
    );
}

#[test]
fn storages_without_directories_skip_the_directory_check() {
    let listing = Listing {
        dirs: BTreeSet::new(),
        dirs_implicit: true,
        ..complete()
    };
    assert_eq!(validate(&listing), Ok(()));
}

// ============================================================ ディスクから読む

/// `tests/world` の仕様書の例をディスクに書く。
fn world_on_disk(dir: &TempDir) -> PathBuf {
    dir.write("novel/project.json", "{\"schema_version\":1}\n");
    dir.write("novel/characters.md", world::CHARACTERS);
    dir.write("novel/words.md", world::WORDS);
    dir.write("novel/materials/index.json", world::INDEX);
    dir.write(&format!("novel/{}", world::CONTENTS_PATH), world::CONTENTS);
    dir.write(&format!("novel/{}", world::EPISODE_PATH), world::EPISODE);
    dir.write(&format!("novel/{}", world::ALL_PATH), world::ALL);
    dir.join("novel")
}

#[test]
fn a_project_on_disk_gives_the_same_diagnostics_as_in_memory() {
    let dir = TempDir::new("load-world");
    let root = world_on_disk(&dir);
    let (_, storage) = open(
        &FakeEnv::with_history(&dir.join("state/recent.json")),
        &root,
    );
    let loaded = block_on(load(&storage)).expect("load");
    assert_eq!(loaded.validation, Ok(()));
    let memory = world::world();
    for path in [
        world::CONTENTS_PATH,
        world::EPISODE_PATH,
        world::ALL_PATH,
        "characters.md",
        "words.md",
    ] {
        let from_disk = loaded.project.document(path).expect(path);
        let in_memory = memory.document(path).expect(path);
        assert_eq!(from_disk.text, in_memory.text, "{path}");
        assert_eq!(
            diagnostics::document(from_disk, &loaded.project),
            diagnostics::document(in_memory, &memory),
            "{path}"
        );
    }
    // `novnow check` の材料: 全文書の診断が集められる
    let errors: usize = loaded
        .project
        .documents()
        .map(|document| {
            diagnostics::document(document, &loaded.project)
                .iter()
                .filter(|found| found.severity() == Severity::Error)
                .count()
        })
        .sum();
    let memory_errors: usize = memory
        .documents()
        .map(|document| {
            diagnostics::document(document, &memory)
                .iter()
                .filter(|found| found.severity() == Severity::Error)
                .count()
        })
        .sum();
    assert_eq!(errors, memory_errors);
}

#[test]
fn loading_normalizes_bom_and_line_endings() {
    let dir = TempDir::new("load-crlf");
    let root = world_on_disk(&dir);
    std::fs::write(
        root.join("plots/01.txt"),
        "\u{feff}@01-04\r\nsummary: 出会い\r\n;\r\n",
    )
    .expect("w");
    let (_, storage) = open(
        &FakeEnv::with_history(&dir.join("state/recent.json")),
        &root,
    );
    let loaded = block_on(load(&storage)).expect("load");
    assert_eq!(
        loaded.project.text("plots/01.txt"),
        "@01-04\nsummary: 出会い\n;\n"
    );
}

#[test]
fn invalid_utf8_files_are_listed_and_the_rest_still_loads() {
    let dir = TempDir::new("load-utf8");
    let root = world_on_disk(&dir);
    std::fs::write(root.join("contents/01-05.txt"), [0x40, 0xff, 0xfe]).expect("w");
    let (_, storage) = open(
        &FakeEnv::with_history(&dir.join("state/recent.json")),
        &root,
    );
    let loaded = block_on(load(&storage)).expect("load");
    assert_eq!(
        loaded.validation,
        Err(ProjectError::Encoding {
            files: vec![PathBuf::from("contents/01-05.txt")]
        })
    );
    assert!(loaded.project.document(world::CONTENTS_PATH).is_some());
    assert!(loaded.project.document("contents/01-05.txt").is_none());
}

#[test]
fn settings_come_from_project_json_on_disk() {
    let dir = TempDir::new("load-settings");
    let root = world_on_disk(&dir);
    std::fs::write(
        root.join("project.json"),
        "{\"schema_version\":1,\"title\":\"星屑の時計\",\"lexicon\":{\"word_classes\":{\"体言\":\"[\"}}}",
    )
    .expect("w");
    let (_, storage) = open(
        &FakeEnv::with_history(&dir.join("state/recent.json")),
        &root,
    );
    let loaded = block_on(load(&storage)).expect("load");
    assert_eq!(loaded.project.settings().title, "星屑の時計");
    let document = loaded
        .project
        .document("project.json")
        .expect("project.json");
    let found = diagnostics::document(document, &loaded.project);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, DiagnosticCode::E027);
}

#[test]
fn an_empty_contents_directory_still_counts() {
    // §4.2「contents/ 必須（空でも可）」— ファイルが無くてもディレクトリがあれば足りる
    let dir = TempDir::new("load-empty-contents");
    let root = sandbox::minimal_project(&dir, "novel");
    std::fs::remove_file(root.join("contents/01-01.txt")).expect("rm");
    let (_, storage) = open(
        &FakeEnv::with_history(&dir.join("state/recent.json")),
        &root,
    );
    let found = block_on(listing(&storage)).expect("listing");
    assert!(found.dirs.contains("contents"));
    assert_eq!(validate(&found), Ok(()));
    assert!(found.project_json.is_some());
}

#[test]
fn a_v0_directory_is_detected() {
    let dir = TempDir::new("load-v0");
    dir.write("v0/project.json", "{\"scene_id_format\":\"A-B-C-D\"}\n");
    dir.write("v0/characters.json", "[]\n");
    dir.write("v0/words.json", "[]\n");
    dir.write("v0/materials/index.json", "[]\n");
    dir.write("v0/plots/all.txt", "@00-00-00\n");
    dir.mkdir("v0/contents");
    let (_, storage) = open(
        &FakeEnv::with_history(&dir.join("state/recent.json")),
        &dir.join("v0"),
    );
    let loaded = block_on(load(&storage)).expect("load");
    assert_eq!(
        loaded.validation,
        Err(ProjectError::LegacyV0 {
            detected: "characters.json".to_owned()
        })
    );
}

#[test]
fn which_files_become_documents() {
    for (name, want) in [
        ("contents/01-01.txt", true),
        ("plots/all.txt", true),
        ("project.json", true),
        ("agents/log/2026-10-13.jsonl", false),
        (".novnow/ui.json", false),
        ("exports/out.txt", false),
        ("notes.txt", false),
        ("assets/icons/a.png", false),
    ] {
        assert_eq!(is_document(&path(name)), want, "{name}");
    }
}

// ============================================================ 外部変更（§8.5）

fn entry(name: &str, modified: i64, len: u64) -> FileEntry {
    FileEntry {
        path: path(name),
        stamp: Stamp {
            modified_ms: Some(modified),
            len,
        },
    }
}

#[test]
fn the_snapshot_diff_lists_created_modified_and_removed_files() {
    let before = Snapshot::from_entries([
        entry("contents/01-01.txt", 1, 10),
        entry("contents/01-02.txt", 1, 10),
        entry("words.md", 1, 10),
    ]);
    let now = Snapshot::from_entries([
        entry("contents/01-01.txt", 2, 10),
        entry("words.md", 1, 10),
        entry("plots/01-01.txt", 1, 5),
        entry(".novnow/ui.json", 9, 9),
        entry("exports/out.txt", 9, 9),
    ]);
    let found: Vec<(String, ChangeKind)> = before
        .diff(&now)
        .into_iter()
        .map(|change| (change.path.as_str().to_owned(), change.kind))
        .collect();
    assert_eq!(
        found,
        vec![
            ("contents/01-01.txt".to_owned(), ChangeKind::Modified),
            ("contents/01-02.txt".to_owned(), ChangeKind::Removed),
            ("plots/01-01.txt".to_owned(), ChangeKind::Created),
        ]
    );
}

#[test]
fn what_is_watched() {
    assert!(is_watched(&path("contents/01-01.txt")));
    assert!(is_watched(&path("materials/index.json")));
    assert!(!is_watched(&path(".novnow/ui.json")));
    assert!(!is_watched(&path("exports/a.zip")));
    assert!(!is_watched(&path("contents/.01-01.txt.novnow-tmp")));
}

#[test]
fn the_reaction_follows_8_5() {
    let body = path("contents/01-01.txt");
    let words = path("words.md");
    // 中身が同じ（自分の保存・touch・改行コードだけの違い）は無視
    assert_eq!(
        react(&body, Buffer::Dirty, Some("a\nb\n"), Some("a\r\nb\r\n")),
        Reaction::Ignore
    );
    // Clean なら再読込、Dirty なら衝突
    assert_eq!(
        react(&body, Buffer::Clean, Some("a"), Some("b")),
        Reaction::Reload
    );
    assert_eq!(
        react(&body, Buffer::Dirty, Some("a"), Some("b")),
        Reaction::Conflict
    );
    // 辞書の 3 ファイルは常に再読込（§8.5）
    assert_eq!(
        react(&words, Buffer::Dirty, Some("a"), Some("b")),
        Reaction::Reload
    );
    // 消えた・現れた
    assert_eq!(
        react(&body, Buffer::Clean, Some("a"), None),
        Reaction::Reload
    );
    assert_eq!(
        react(&body, Buffer::Dirty, Some("a"), None),
        Reaction::Conflict
    );
    assert_eq!(
        react(&body, Buffer::Clean, None, Some("new")),
        Reaction::Reload
    );
    // 通知は I052（文書位置を持たない）
    let found = notice(&body);
    assert_eq!(found.code, DiagnosticCode::I052);
    assert_eq!(found.range, None);
}

#[test]
fn polling_a_real_directory_ignores_our_own_saves() {
    let dir = TempDir::new("load-poll");
    let root = world_on_disk(&dir);
    let (_, storage) = open(
        &FakeEnv::with_history(&dir.join("state/recent.json")),
        &root,
    );
    let mut snapshot = Snapshot::from_entries(block_on(storage.list()).expect("list"));
    // 自分の保存: 返ったスタンプを覚えれば候補にならない
    let target = path(world::CONTENTS_PATH);
    let stamp = block_on(storage.write(&target, "@01-04-01\n自分で書いた\n")).expect("write");
    snapshot.acknowledge(target.clone(), stamp);
    let now = Snapshot::from_entries(block_on(storage.list()).expect("list"));
    assert_eq!(snapshot.diff(&now), vec![]);
    // 外部の変更（別のエディタ）: 長さが変わるので候補になり、中身を比べて再読込
    std::fs::write(
        root.join(world::CONTENTS_PATH),
        "@01-04-01\n外で書き換えた長い本文\n",
    )
    .expect("w");
    let later = Snapshot::from_entries(block_on(storage.list()).expect("list"));
    let changes = now.diff(&later);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, ChangeKind::Modified);
    let disk = block_on(storage.read(&changes[0].path)).expect("read");
    assert_eq!(
        react(
            &changes[0].path,
            Buffer::Clean,
            Some("@01-04-01\n自分で書いた\n"),
            Some(&disk)
        ),
        Reaction::Reload
    );
}

#[test]
fn one_poll_reports_changes_once_and_remembers_the_new_state() {
    let dir = TempDir::new("load-poll-once");
    let root = world_on_disk(&dir);
    let (_, storage) = open(
        &FakeEnv::with_history(&dir.join("state/recent.json")),
        &root,
    );
    let mut snapshot = Snapshot::default();
    // 初回は全部が「現れた」
    let first = block_on(poll(&storage, &mut snapshot)).expect("poll");
    assert_eq!(first.len(), 7);
    assert!(first
        .iter()
        .all(|change| change.kind == ChangeKind::Created));
    // 何も変えなければ候補なし
    assert_eq!(
        block_on(poll(&storage, &mut snapshot)).expect("poll"),
        vec![]
    );
    // 外部で足す・消す → 1 回だけ報告される
    std::fs::write(root.join("plots/01.txt"), "@01-04\n").expect("w");
    std::fs::remove_file(root.join("words.md")).expect("rm");
    let changes: Vec<(String, ChangeKind)> = block_on(poll(&storage, &mut snapshot))
        .expect("poll")
        .into_iter()
        .map(|change| (change.path.as_str().to_owned(), change.kind))
        .collect();
    assert_eq!(
        changes,
        vec![
            ("plots/01.txt".to_owned(), ChangeKind::Created),
            ("words.md".to_owned(), ChangeKind::Removed),
        ]
    );
    assert_eq!(
        block_on(poll(&storage, &mut snapshot)).expect("poll"),
        vec![]
    );
}
