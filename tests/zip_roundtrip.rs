//! Zip のインポート / エクスポート（§13.5 第 2.9 版、`docs/history.md` の完了条件 3）。
//!
//! 「Zip エクスポート → 別環境でインポート → 内容一致」を、デスクトップ（`FsStorage`）から
//! 書き出した Zip を (1) 別のディレクトリ（デスクトップ）と (2) Web の取り込み経路
//! （`Imported::into_texts` → IndexedDB に書くテキスト）の両方で読み直して確かめる。
//! 規則（何を入れて何を取り込むか）は `core::policy::archive` のテストも兼ねる。

mod sandbox;

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::Arc;

use novnow::adapters::storage_fs::{FsGate, FsStorage};
use novnow::adapters::zip::{self as archive_io, ArchiveError};
use novnow::core::policy::archive::{is_exported, plan_import, ArchiveEntry, SkipReason};
use novnow::core::policy::{PathError, ProjectPath};

use sandbox::{minimal_project, tree, FakeEnv, TempDir};

fn path(text: &str) -> ProjectPath {
    ProjectPath::new(text).expect("相対パス")
}

/// 一通りのファイルを持つプロジェクト（ローカル状態・ドットファイル・バイナリを含む）。
fn rich_project(dir: &TempDir) -> std::path::PathBuf {
    let root = minimal_project(dir, "novel");
    dir.write("novel/plots/01.txt", "@01-01\nsummary: 出会い\n;\n");
    dir.write("novel/plots/01-01.txt", "@01-01-01\n出会いの場面\n");
    dir.write("novel/ideas.md", "# I0001 学院長の動機\nstatus: inbox\n");
    dir.write("novel/materials/docs/.gitkeep", "");
    dir.write("novel/agents/log/.gitkeep", "");
    dir.write("novel/.gitignore", ".novnow/\nexports/\n");
    dir.write("novel/exports/old-export.txt", "前回の書き出し\n");
    dir.write("novel/.novnow/ui.json", "{\"schema_version\":1}\n");
    dir.write("novel/.novnow/cache/index.bin", "cache");
    dir.write("novel/.DS_Store", "junk");
    dir.write("novel/.git/HEAD", "ref: refs/heads/main\n");
    dir.write("novel/contents/.01-01.txt.novnow-tmp", "書きかけ");
    let icon = root.join("assets/icons/akari.png");
    std::fs::create_dir_all(icon.parent().expect("parent")).expect("mkdir");
    std::fs::write(&icon, [0x89, b'P', b'N', b'G', 0xff, 0xfe, 0x00]).expect("png");
    root
}

/// Zip に入るべきもの（`.novnow/`・`.git/`・`.DS_Store`・一時ファイルと、書き出し先の Zip 自身を除く）。
fn expected(root: &std::path::Path, zip_name: &str) -> BTreeMap<String, Vec<u8>> {
    tree(root)
        .into_iter()
        .filter(|(name, _)| {
            !name.starts_with(".novnow/")
                && !name.starts_with(".git/")
                && name != ".DS_Store"
                && !name.ends_with(".novnow-tmp")
                && name != zip_name
        })
        .collect()
}

#[test]
fn export_then_import_on_another_desktop_gives_the_same_files() {
    let dir = TempDir::new("zip-desktop");
    let root = rich_project(&dir);
    let gate = Arc::new(FsGate::new(&FakeEnv::with_history(
        &dir.join("state/recent.json"),
    )));
    let storage = FsStorage::open(gate.clone(), &root).expect("open");
    let zip_path = archive_io::export_project(&storage, "星屑の時計").expect("export");
    assert_eq!(zip_path.as_str(), "exports/星屑の時計.zip");
    let want = expected(&root, zip_path.as_str());
    assert!(want.contains_key(".gitignore") && want.contains_key("materials/docs/.gitkeep"));
    assert!(
        want.contains_key("exports/old-export.txt"),
        "exports/ は含める（§13.5）"
    );

    // 別の環境（別のディレクトリ）に取り込む
    let bytes = storage.read_bytes(&zip_path).expect("zip");
    let imported = archive_io::import(&bytes).expect("import");
    assert_eq!(imported.skipped, vec![]);
    assert_eq!(imported.stripped_root, None);
    let target = dir.join("elsewhere/novel");
    let other = FsStorage::create(gate, &target).expect("create");
    let report = archive_io::import_project(&other, &imported).expect("write");
    assert_eq!(report.kept, vec![]);
    assert_eq!(
        tree(&target),
        want,
        "取り込んだ内容が書き出した内容と一致しない"
    );
}

#[test]
fn export_then_import_on_the_web_gives_the_same_texts() {
    let dir = TempDir::new("zip-web");
    let root = rich_project(&dir);
    let gate = Arc::new(FsGate::new(&FakeEnv::with_history(
        &dir.join("state/recent.json"),
    )));
    let storage = FsStorage::open(gate, &root).expect("open");
    let zip_path = archive_io::export_project(&storage, "novel").expect("export");
    let bytes = storage.read_bytes(&zip_path).expect("zip");
    // Web はテキストのみ（§13.3.2）: バイナリは取り込まずに一覧で示す
    let (texts, skipped) = archive_io::import(&bytes).expect("import").into_texts();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].name, "assets/icons/akari.png");
    assert_eq!(skipped[0].reason, SkipReason::NotText);
    let got: BTreeMap<String, Vec<u8>> = texts
        .into_iter()
        .map(|(found, text)| (found.as_str().to_owned(), text.into_bytes()))
        .collect();
    let mut want = expected(&root, zip_path.as_str());
    want.remove("assets/icons/akari.png");
    assert_eq!(got, want);
}

#[test]
fn importing_into_a_non_empty_directory_keeps_existing_files() {
    let dir = TempDir::new("zip-keep");
    let root = minimal_project(&dir, "novel");
    let gate = Arc::new(FsGate::new(&FakeEnv::with_history(
        &dir.join("state/recent.json"),
    )));
    let storage = FsStorage::open(gate.clone(), &root).expect("open");
    let zip_path = archive_io::export_project(&storage, "novel").expect("export");
    let bytes = storage.read_bytes(&zip_path).expect("zip");
    dir.write("target/project.json", "{\"title\":\"既存\"}\n");
    let target = FsStorage::create(gate, &dir.join("target")).expect("create");
    let report = archive_io::import_project(&target, &archive_io::import(&bytes).expect("import"))
        .expect("write");
    assert_eq!(report.kept, vec![path("project.json")]);
    assert_eq!(
        std::fs::read_to_string(dir.join("target/project.json")).expect("read"),
        "{\"title\":\"既存\"}\n",
        "既存のファイルは上書きしない（§13.5 第 2.9 版）"
    );
}

#[test]
fn a_single_root_directory_is_stripped() {
    // GitHub の「Download ZIP」や v0 の Web 版（`my-novel/`）のような Zip
    let entries = [
        ArchiveEntry::dir("my-novel/"),
        ArchiveEntry::file("my-novel/project.json"),
        ArchiveEntry::file("my-novel/contents/01-01.txt"),
        ArchiveEntry::file("__MACOSX/my-novel/._project.json"),
        ArchiveEntry::file("my-novel/.DS_Store"),
    ];
    let plan = plan_import(&entries);
    assert_eq!(plan.stripped_root.as_deref(), Some("my-novel"));
    let paths: Vec<&str> = plan
        .entries
        .iter()
        .map(|found| found.path.as_str())
        .collect();
    assert_eq!(paths, vec!["project.json", "contents/01-01.txt"]);
    let reasons: Vec<SkipReason> = plan.skipped.iter().map(|found| found.reason).collect();
    assert_eq!(reasons, vec![SkipReason::MacMetadata, SkipReason::Hidden]);
}

#[test]
fn a_layout_directory_is_not_mistaken_for_a_root() {
    // 全部が contents/ の下でも、それはルートではなく中身（project.json が無い）
    let entries = [
        ArchiveEntry::file("contents/01-01.txt"),
        ArchiveEntry::file("contents/01-02.txt"),
    ];
    let plan = plan_import(&entries);
    assert_eq!(plan.stripped_root, None);
    assert_eq!(plan.entries[0].path.as_str(), "contents/01-01.txt");
    // ルート直下にファイルがあれば剥がさない
    let entries = [
        ArchiveEntry::file("project.json"),
        ArchiveEntry::file("contents/01-01.txt"),
    ];
    assert_eq!(plan_import(&entries).stripped_root, None);
}

#[test]
fn unsafe_names_hidden_files_and_duplicates_are_listed_not_imported() {
    let entries = [
        ArchiveEntry::file("../evil.txt"),
        ArchiveEntry::file("/etc/passwd"),
        ArchiveEntry::file("C:\\Windows\\evil.txt"),
        ArchiveEntry::file("contents\\01-01.txt"),
        ArchiveEntry::file("./contents/01-02.txt"),
        ArchiveEntry::file("contents/01-01.txt"),
        ArchiveEntry::file(".novnow/ui.json"),
        ArchiveEntry::file(".git/config"),
        ArchiveEntry::file(".gitignore"),
        ArchiveEntry::file("agents/tasks/.gitkeep"),
    ];
    let plan = plan_import(&entries);
    let imported: Vec<&str> = plan
        .entries
        .iter()
        .map(|found| found.path.as_str())
        .collect();
    assert_eq!(
        imported,
        vec![
            "contents/01-01.txt",
            "contents/01-02.txt",
            ".gitignore",
            "agents/tasks/.gitkeep"
        ]
    );
    let skipped: Vec<(&str, SkipReason)> = plan
        .skipped
        .iter()
        .map(|found| (found.name.as_str(), found.reason))
        .collect();
    assert_eq!(
        skipped,
        vec![
            ("../evil.txt", SkipReason::Unsafe(PathError::ParentDir)),
            ("/etc/passwd", SkipReason::Unsafe(PathError::Absolute)),
            (
                "C:\\Windows\\evil.txt",
                SkipReason::Unsafe(PathError::Colon)
            ),
            ("contents/01-01.txt", SkipReason::Duplicate),
            (".novnow/ui.json", SkipReason::Hidden),
            (".git/config", SkipReason::Hidden),
        ]
    );
}

#[test]
fn a_root_named_dot_dot_is_not_stripped_into_safety() {
    let entries = [
        ArchiveEntry::file("../a.txt"),
        ArchiveEntry::file("../b.txt"),
    ];
    let plan = plan_import(&entries);
    assert_eq!(plan.stripped_root, None);
    assert!(plan.entries.is_empty());
    assert_eq!(plan.skipped.len(), 2);
}

#[test]
fn what_goes_into_an_export() {
    for (name, want) in [
        ("contents/01-01.txt", true),
        ("exports/old.txt", true),
        (".gitignore", true),
        ("materials/docs/.gitkeep", true),
        (".novnow/ui.json", false),
        (".git/HEAD", false),
        (".DS_Store", false),
        ("contents/.01-01.txt.novnow-tmp", false),
    ] {
        assert_eq!(is_exported(&path(name)), want, "{name}");
    }
}

#[test]
fn export_file_names_are_safe() {
    assert_eq!(archive_io::file_name("星屑の時計"), "星屑の時計.zip");
    assert_eq!(archive_io::file_name("a/b\\c:d"), "a_b_c_d.zip");
    assert_eq!(archive_io::file_name("  "), "project.zip");
    assert_eq!(archive_io::file_name(".hidden"), "_.hidden.zip");
    assert_eq!(archive_io::export_path("a/b").as_str(), "exports/a_b.zip");
}

#[test]
fn broken_bytes_are_not_a_zip() {
    assert!(matches!(
        archive_io::import(b"not a zip"),
        Err(ArchiveError::Broken(_))
    ));
}

#[test]
fn the_export_is_deflated() {
    let text = "あ".repeat(10_000);
    let bytes = archive_io::export(&[(path("contents/01-01.txt"), text.clone().into_bytes())])
        .expect("export");
    assert!(bytes.len() < text.len() / 10, "{} bytes", bytes.len());
    let mut zip = ::zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
    let file = zip.by_index(0).expect("entry");
    assert_eq!(file.compression(), ::zip::CompressionMethod::Deflated);
    assert_eq!(file.name(), "contents/01-01.txt");
}

#[test]
fn export_skips_local_state_even_when_asked() {
    let bytes = archive_io::export(&[
        (path(".novnow/ui.json"), b"{}".to_vec()),
        (path("project.json"), b"{}".to_vec()),
    ])
    .expect("export");
    let imported = archive_io::import(&bytes).expect("import");
    let names: Vec<&str> = imported
        .files
        .iter()
        .map(|(found, _)| found.as_str())
        .collect();
    assert_eq!(names, vec!["project.json"]);
}

#[test]
fn raw_zip_with_a_nested_root_and_crlf_round_trips_bytes_exactly() {
    // 改行や BOM を含むバイト列もそのまま運ぶ（正規化は読み込み側の仕事、§5.0.4）
    let mut writer = ::zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = ::zip::write::SimpleFileOptions::default();
    writer
        .start_file("novel-main/project.json", options)
        .expect("start");
    writer
        .write_all("\u{feff}{}\r\n".as_bytes())
        .expect("write");
    let bytes = writer.finish().expect("finish").into_inner();
    let imported = archive_io::import(&bytes).expect("import");
    assert_eq!(imported.stripped_root.as_deref(), Some("novel-main"));
    assert_eq!(
        imported.files,
        vec![(path("project.json"), "\u{feff}{}\r\n".as_bytes().to_vec())]
    );
}
