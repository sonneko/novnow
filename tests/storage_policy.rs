//! 規則 10（指示 10）— プロジェクト外への書き込み禁止（§13.1、`PLANS.md` §5.6 完了条件 1、CI 必須）。
//!
//! **全書き込み経路**（本文・設定・エクスポート・提案・ログ・Zip・履歴）にプロジェクト外のパスを
//! 与えて、**すべて拒否され、何も書かれない**ことを確かめる。違反はデバッグビルドでは `panic!`、
//! リリースビルドでは E060（§13.1.2）なので、[`rejected`] は両方を受け付ける。
//!
//! **特例**（§13.1.1、ユーザー裁定 2026-10-13・第 2.10 版）: 保存ダイアログで指定された Zip
//! （`ChosenPath`）だけは `FsGate::write_chosen_export` からプロジェクトの外にも書ける。
//! 「その 1 個だけ・`.zip` だけ・その口だけ・一時ファイルも親ディレクトリも作らない」を
//! `the_save_dialog_exception_*` が確かめる。
//!
//! 前半は core の判定（`WritePolicy` / `ProjectPath`）、後半はネイティブの関門（`FsGate`）を
//! 通る実際の書き込み。シンボリックリンクを使う検査は Unix だけ（Windows はリンクの作成に権限が要る）。

mod sandbox;

use std::fmt::Debug;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};

use novnow::adapters::block_on;
use novnow::adapters::recent_file::RecentFile;
use novnow::adapters::storage_fs::{FsGate, FsStorage};
use novnow::adapters::zip;
use novnow::core::diagnostics::{Anchor, Arg, DiagnosticCode};
use novnow::core::policy::{
    Allowed, PathError, PolicyViolation, ProjectPath, Violation, WritePolicy,
};
use novnow::ports::dialog::ChosenPath;
use novnow::ports::storage::{Storage, StorageError};

use sandbox::{minimal_project, open, tree, FakeEnv, TempDir, NOW};

// ============================================================ 共通

/// 違反の panic（デバッグビルド）の出力を黙らせる（他の panic はそのまま出す）。
fn quiet_policy_panics() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let default = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let payload = info.payload();
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("");
            if !message.contains("E060 write policy violation") {
                default(info);
            }
        }));
    });
}

/// 書き込みが規則 10 で拒否されたことを確かめる（§13.1.2: デバッグは panic、リリースは E060）。
fn rejected<T: Debug>(attempt: impl FnOnce() -> Result<T, StorageError>) {
    quiet_policy_panics();
    match panic::catch_unwind(AssertUnwindSafe(attempt)) {
        Err(payload) => {
            if !cfg!(debug_assertions) {
                panic!("リリースビルドで panic した");
            }
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_default();
            assert!(message.contains("E060 write policy violation"), "{message}");
        }
        Ok(Err(StorageError::Policy(violation))) => {
            if cfg!(debug_assertions) {
                panic!("デバッグビルドなのに panic しない: {violation}");
            }
            assert_eq!(violation.diagnostic().code, DiagnosticCode::E060);
        }
        Ok(other) => panic!("拒否されなかった: {other:?}"),
    }
}

/// プロジェクトとプロジェクト外の 2 つを持つ砂場。
struct World {
    dir: TempDir,
    root: PathBuf,
    outside: PathBuf,
    gate: Arc<FsGate>,
    storage: FsStorage,
}

impl World {
    fn new(label: &str) -> Self {
        let dir = TempDir::new(label);
        let root = minimal_project(&dir, "project");
        dir.write("outside/victim.txt", "触ってはいけない\n");
        let outside = dir.join("outside");
        let env = FakeEnv::with_history(&dir.join("state/recent.json"));
        let (gate, storage) = open(&env, &root);
        Self {
            dir,
            root,
            outside,
            gate,
            storage,
        }
    }

    /// プロジェクト外が書き込みの前後で変わっていないこと。
    fn outside_untouched(&self, before: &std::collections::BTreeMap<String, Vec<u8>>) {
        assert_eq!(&tree(&self.outside), before, "プロジェクト外が変わった");
        assert!(!self.dir.join("evil.txt").exists());
        assert!(!self.dir.join("state").exists() || self.dir.join("state/recent.json").exists());
    }
}

fn path(text: &str) -> ProjectPath {
    ProjectPath::new(text).expect("相対パス")
}

// ============================================================ core の判定

#[test]
fn the_policy_allows_only_the_project_and_the_history_file() {
    let mut policy = WritePolicy::new(Some(PathBuf::from(
        "/home/u/.local/state/novnow/recent.json",
    )));
    policy.open(Path::new("/novels/hoshi")).expect("open");
    let check = |target: &str| policy.check(Path::new(target));
    assert_eq!(
        check("/novels/hoshi/contents/01-01.txt"),
        Ok(Allowed::Project)
    );
    assert_eq!(check("/novels/hoshi"), Ok(Allowed::Project));
    assert_eq!(
        check("/novels/hoshi/./plots/../project.json"),
        Ok(Allowed::Project)
    );
    assert_eq!(
        check("/home/u/.local/state/novnow/recent.json"),
        Ok(Allowed::History)
    );
    let reason = |target: &str| policy.check(Path::new(target)).expect_err(target).reason;
    assert_eq!(reason("/novels/other/x.txt"), Violation::Outside);
    // 前方一致ではなく要素で比べる（`/novels/hoshi-evil` は別のディレクトリ）
    assert_eq!(reason("/novels/hoshi-evil/x.txt"), Violation::Outside);
    assert_eq!(reason("/novels/hoshi/../other/x.txt"), Violation::Outside);
    assert_eq!(reason("/../x"), Violation::Escape);
    assert_eq!(reason("relative/x.txt"), Violation::Relative);
    assert_eq!(reason("/tmp/x.txt"), Violation::Outside);
    // 履歴の隣や祖先はファイルとして書けない
    assert_eq!(
        reason("/home/u/.local/state/novnow/other.json"),
        Violation::Outside
    );
    assert_eq!(reason("/home/u/.local/state/novnow"), Violation::Outside);
}

#[test]
fn the_save_dialog_exception_allows_only_a_zip_file_outside() {
    // §13.1.1 の特例（第 2.10 版）: 判定は check_chosen_export だけ。check は特例を知らない
    let history = "/home/u/.local/state/novnow/recent.json";
    let mut policy = WritePolicy::new(Some(PathBuf::from(history)));
    let chosen = |target: &str| policy.check_chosen_export(Path::new(target));
    // プロジェクトを開いていなくても、ダイアログで指定された外の Zip は書ける
    assert_eq!(
        chosen("/home/u/Desktop/novel.zip"),
        Ok(Allowed::ChosenExport)
    );
    policy.open(Path::new("/novels/hoshi")).expect("open");
    let chosen = |target: &str| policy.check_chosen_export(Path::new(target));
    assert_eq!(
        chosen("/home/u/Desktop/novel.zip"),
        Ok(Allowed::ChosenExport)
    );
    assert_eq!(chosen("/media/usb/NOVEL.ZIP"), Ok(Allowed::ChosenExport));
    assert_eq!(chosen("/tmp/novel.zip"), Ok(Allowed::ChosenExport));
    // プロジェクトの中を指定したら、ふつうのプロジェクト内の書き込み
    assert_eq!(
        chosen("/novels/hoshi/exports/novel.zip"),
        Ok(Allowed::Project)
    );
    let reason = |target: &str| {
        policy
            .check_chosen_export(Path::new(target))
            .expect_err(target)
            .reason
    };
    // `.zip` でないもの・履歴ファイルは特例にならない
    assert_eq!(reason("/home/u/Desktop/novel.txt"), Violation::NotAnExport);
    assert_eq!(reason("/home/u/Desktop/novel"), Violation::NotAnExport);
    assert_eq!(reason("/home/u/Desktop/.zip"), Violation::NotAnExport);
    assert_eq!(reason("/home/u/.bashrc"), Violation::NotAnExport);
    assert_eq!(reason("/novels/hoshi/project.json"), Violation::NotAnExport);
    assert_eq!(reason(history), Violation::NotAnExport);
    assert_eq!(reason("relative/novel.zip"), Violation::Relative);
    assert_eq!(reason("/../novel.zip"), Violation::Escape);
    // 一般の判定は特例を知らない（ほかの書き込みの口は外の Zip を書けない）
    assert_eq!(
        policy
            .check(Path::new("/home/u/Desktop/novel.zip"))
            .expect_err("check")
            .reason,
        Violation::Outside
    );
}

#[test]
fn history_ancestors_are_allowed_only_as_directories() {
    let policy = WritePolicy::new(Some(PathBuf::from(
        "/home/u/.local/state/novnow/recent.json",
    )));
    assert_eq!(
        policy.check_dir(Path::new("/home/u/.local/state/novnow")),
        Ok(Allowed::HistoryDir)
    );
    assert_eq!(
        policy.check_dir(Path::new("/home/u/.local")),
        Ok(Allowed::HistoryDir)
    );
    // 履歴そのものはディレクトリとしては作れない・プロジェクトを開いていなければ他は不可
    assert_eq!(
        policy
            .check_dir(Path::new("/home/u/.local/state/novnow/recent.json"))
            .expect_err("dir")
            .reason,
        Violation::NoProject
    );
    assert_eq!(
        policy
            .check(Path::new("/home/u/x.txt"))
            .expect_err("file")
            .reason,
        Violation::NoProject
    );
}

#[test]
fn closing_the_project_leaves_only_the_history_file() {
    let mut policy = WritePolicy::new(None);
    policy.open(Path::new("/novels/hoshi")).expect("open");
    policy.close();
    assert_eq!(
        policy
            .check(Path::new("/novels/hoshi/a.txt"))
            .expect_err("closed")
            .reason,
        Violation::NoProject
    );
    assert!(policy.open(Path::new("relative")).is_err());
}

#[test]
fn a_violation_is_reported_as_e060_with_the_path() {
    let violation = PolicyViolation {
        target: PathBuf::from("/tmp/x.txt"),
        reason: Violation::Outside,
    };
    let found = violation.diagnostic();
    assert_eq!(found.code, DiagnosticCode::E060);
    assert_eq!(found.args, vec![Arg::text("/tmp/x.txt")]);
    assert_eq!(found.range, None);
    assert_eq!(found.anchor(), Anchor::Status);
}

#[test]
fn project_paths_cannot_name_anything_outside_the_root() {
    let bad = [
        ("", PathError::Empty),
        ("/etc/passwd", PathError::Absolute),
        ("a\\b.txt", PathError::Backslash),
        ("../outside.txt", PathError::ParentDir),
        ("contents/../../x", PathError::ParentDir),
        ("./a.txt", PathError::CurrentDir),
        ("a//b.txt", PathError::EmptySegment),
        ("contents/", PathError::EmptySegment),
        ("C:/Windows/x", PathError::Colon),
        ("a.txt:stream", PathError::Colon),
        ("a\nb.txt", PathError::Control),
    ];
    for (text, error) in bad {
        assert_eq!(ProjectPath::new(text), Err(error), "{text:?}");
    }
    for good in [
        "contents/01-01.txt",
        ".novnow/ui.json",
        "exports/星屑の時計.zip",
        "materials/docs/M0001-旧校舎.md",
        "..hidden",
    ] {
        assert_eq!(ProjectPath::new(good).expect(good).as_str(), good);
    }
    let found = path("agents/log/2026-10-13.jsonl");
    assert_eq!(found.file_name(), "2026-10-13.jsonl");
    assert_eq!(found.parent(), Some("agents/log"));
    assert!(found.is_under("agents"));
    assert!(!found.is_under("agent"));
    assert!(path(".novnow/ui.json").is_local_state());
}

// ============================================================ 関門を通る書き込み（成功側）

#[test]
fn every_write_path_works_inside_the_project() {
    let world = World::new("policy-inside");
    let storage = &world.storage;
    // 本文・設定・提案・エクスポート（テキスト）
    for (target, text) in [
        ("contents/01-04.txt", "@01-04-01\n本文\n"),
        ("project.json", "{\"schema_version\":1}\n"),
        (".novnow/ui.json", "{\"schema_version\":1}\n"),
        ("agents/proposals/H0001.md", "# 提案\n"),
        ("exports/out.txt", "書き出し\n"),
    ] {
        block_on(storage.write(&path(target), text)).expect(target);
        assert_eq!(
            std::fs::read_to_string(world.root.join(target)).expect(target),
            text
        );
    }
    // ログ（追記）
    let log = path("agents/log/2026-10-13.jsonl");
    block_on(storage.append(&log, "{\"a\":1}\n")).expect("append");
    block_on(storage.append(&log, "{\"a\":2}\n")).expect("append");
    assert_eq!(
        block_on(storage.read(&log)).expect("read"),
        "{\"a\":1}\n{\"a\":2}\n"
    );
    // Zip（エクスポート → 取り込み）
    let zip_path = zip::export_project(storage, "星屑の時計").expect("export");
    assert_eq!(zip_path.as_str(), "exports/星屑の時計.zip");
    assert!(world.root.join("exports/星屑の時計.zip").is_file());
    // 削除
    block_on(storage.remove(&path("exports/out.txt"))).expect("remove");
    assert!(!world.root.join("exports/out.txt").exists());
    // 原子的保存の一時ファイルは残らない
    let leftovers: Vec<String> = tree(&world.root)
        .into_keys()
        .filter(|name| name.ends_with(".novnow-tmp"))
        .collect();
    assert_eq!(leftovers, Vec::<String>::new());
    // 履歴
    let recent = RecentFile::new(world.gate.clone());
    recent.push(&world.root, &NOW).expect("history");
    assert!(world.dir.join("state/recent.json").is_file());
}

// ============================================================ 関門を通る書き込み（拒否側）

#[test]
fn absolute_paths_outside_the_project_are_rejected_on_every_gate_method() {
    let world = World::new("policy-absolute");
    let before = tree(&world.outside);
    let gate = &world.gate;
    let new = world.outside.join("new.txt");
    let victim = world.outside.join("victim.txt");
    rejected(|| gate.write_atomic(&new, b"x"));
    rejected(|| gate.write_atomic(&victim, b"x"));
    rejected(|| gate.append(&victim, b"x"));
    rejected(|| gate.remove_file(&victim));
    rejected(|| gate.create_dir_all(&world.outside.join("dir")));
    // OS の一時ディレクトリも禁止（§13.1.1）
    rejected(|| gate.write_atomic(&std::env::temp_dir().join("novnow-evil.txt"), b"x"));
    // 相対パスはカレントディレクトリ基準で解決され、プロジェクトの外
    rejected(|| gate.write_atomic(Path::new("evil.txt"), b"x"));
    world.outside_untouched(&before);
    assert!(!std::env::temp_dir().join("novnow-evil.txt").exists());
    assert!(
        !Path::new("evil.txt").exists(),
        "カレントディレクトリに書いた"
    );
}

#[test]
fn dot_dot_escapes_are_rejected() {
    let world = World::new("policy-dotdot");
    let before = tree(&world.outside);
    let gate = &world.gate;
    let escape = world.root.join("../outside/new.txt");
    rejected(|| gate.write_atomic(&escape, b"x"));
    rejected(|| gate.append(&escape, b"x"));
    rejected(|| gate.create_dir_all(&world.root.join("contents/../../outside/d")));
    rejected(|| gate.remove_file(&world.root.join("../outside/victim.txt")));
    // まだ無いディレクトリを `..` で抜ける形も（無い要素は字句で正規化される）
    rejected(|| gate.write_atomic(&world.root.join("nope/../../evil.txt"), b"x"));
    world.outside_untouched(&before);
}

#[test]
fn the_history_file_is_written_only_through_the_history_door() {
    let world = World::new("policy-history");
    let history = world.gate.history_file().expect("history");
    // 一般の口からは書けない（原子的保存の一時ファイルが外にできるのも防ぐ）
    rejected(|| world.gate.write_atomic(&history, b"{}"));
    rejected(|| world.gate.append(&history, b"{}"));
    assert!(!history.exists());
    assert!(!history.parent().expect("parent").exists());
    // 履歴の口からは書ける（親ディレクトリも作る）
    world.gate.write_history(b"{}").expect("history");
    assert_eq!(std::fs::read(&history).expect("read"), b"{}");
    // 一時ファイルを作らずに直接書く（§13.2.2 第 2.9 版）
    let siblings: Vec<_> = std::fs::read_dir(history.parent().expect("parent"))
        .expect("read_dir")
        .flatten()
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(siblings.len(), 1);
}

#[test]
fn a_closed_project_rejects_every_write() {
    let world = World::new("policy-closed");
    world.gate.close_project();
    let storage = &world.storage;
    rejected(|| block_on(storage.write(&path("contents/01-01.txt"), "x")));
    rejected(|| block_on(storage.append(&path("agents/log/a.jsonl"), "x")));
    rejected(|| storage.write_bytes(&path("exports/a.zip"), b"x"));
    assert_eq!(
        std::fs::read_to_string(world.root.join("contents/01-01.txt")).expect("read"),
        "@01-01-01\n「こんにちは」\n"
    );
}

#[test]
fn a_storage_for_a_previous_project_can_no_longer_write() {
    let world = World::new("policy-stale");
    let other = minimal_project(&world.dir, "other");
    let fresh = FsStorage::open(world.gate.clone(), &other).expect("open");
    // 関門は「今開いているプロジェクト」だけを許す（§13.1.1）
    rejected(|| block_on(world.storage.write(&path("contents/01-01.txt"), "x")));
    block_on(fresh.write(&path("contents/01-01.txt"), "ok\n")).expect("fresh");
}

#[test]
fn the_save_dialog_exception_writes_exactly_the_chosen_zip() {
    let world = World::new("policy-chosen-outside");
    let before = tree(&world.outside);
    let target = world.outside.join("星屑の時計.zip");
    let written =
        zip::export_to_chosen(&world.storage, ChosenPath::from_save_dialog(target.clone()))
            .expect("export");
    assert_eq!(written, std::fs::canonicalize(&target).expect("written"));
    // プロジェクトの外で増えたのは指定した Zip 1 個だけ（一時ファイルも残らない）
    let mut expected = before.clone();
    expected.insert(
        "星屑の時計.zip".to_owned(),
        std::fs::read(&target).expect("zip"),
    );
    assert_eq!(tree(&world.outside), expected);
    assert!(!world.dir.join("evil.txt").exists());
    // 中身はプロジェクトの全ファイル（取り込み直せる）
    let imported = zip::import(&std::fs::read(&target).expect("zip")).expect("import");
    let names: Vec<&str> = imported
        .files
        .iter()
        .map(|(path, _)| path.as_str())
        .collect();
    assert!(names.contains(&"project.json"), "{names:?}");
    assert!(names.contains(&"contents/01-01.txt"), "{names:?}");
    // ダイアログで上書きを確かめた既存の Zip は上書きする（直接・一時ファイルなし）
    std::fs::write(&target, b"old").expect("plant");
    zip::export_to_chosen(&world.storage, ChosenPath::from_save_dialog(target.clone()))
        .expect("overwrite");
    assert_ne!(std::fs::read(&target).expect("zip"), b"old");
    assert_eq!(tree(&world.outside).len(), before.len() + 1);
}

#[test]
fn the_save_dialog_exception_is_only_for_zip_files_and_only_through_its_door() {
    let world = World::new("policy-chosen-limits");
    let before = tree(&world.outside);
    let gate = &world.gate;
    let chosen = |name: &str| ChosenPath::from_save_dialog(world.outside.join(name));
    // 事前判定は panic しない（ダイアログの結果なので E060 を出すだけ。§13.1.2）
    let text = gate
        .check_chosen_export(&chosen("novel.txt"))
        .expect_err("txt");
    assert_eq!(text.reason, Violation::NotAnExport);
    assert_eq!(text.diagnostic().code, DiagnosticCode::E060);
    match zip::export_to_chosen(&world.storage, chosen("victim.txt")) {
        Err(zip::ArchiveError::Storage(StorageError::Policy(violation))) => {
            assert_eq!(violation.reason, Violation::NotAnExport);
        }
        other => panic!("拒否されなかった: {other:?}"),
    }
    // 事前判定を飛ばして特例の口に渡したら、他の口と同じく panic / E060
    rejected(|| gate.write_chosen_export(chosen("novel.txt"), b"x"));
    rejected(|| gate.write_chosen_export(chosen("victim.txt"), b"x"));
    let history = gate.history_file().expect("history");
    rejected(|| gate.write_chosen_export(ChosenPath::from_save_dialog(history.clone()), b"x"));
    assert!(!history.exists());
    // ほかの口は特例を使えない（外の `.zip` でも拒否）
    let zip_outside = world.outside.join("novel.zip");
    rejected(|| gate.write_atomic(&zip_outside, b"x"));
    rejected(|| gate.append(&zip_outside, b"x"));
    assert!(gate.check(&zip_outside).is_err());
    // 親ディレクトリは作らない（ダイアログで選べるのは既存のディレクトリだけ）
    let missing = world.outside.join("missing/novel.zip");
    match gate.write_chosen_export(ChosenPath::from_save_dialog(missing), b"x") {
        Err(StorageError::Io { kind, .. }) => assert_eq!(kind, std::io::ErrorKind::NotFound),
        other => panic!("親ディレクトリが無いのに書けた: {other:?}"),
    }
    world.outside_untouched(&before);
    // 特例で 1 回書いても、その場所に許可は残らない（関門はパスを覚えない）
    gate.write_chosen_export(chosen("novel.zip"), b"zip")
        .expect("chosen");
    rejected(|| gate.write_atomic(&zip_outside, b"again"));
    assert_eq!(std::fs::read(&zip_outside).expect("zip"), b"zip");
}

#[test]
fn the_save_dialog_exception_inside_the_project_is_an_ordinary_atomic_write() {
    let world = World::new("policy-chosen-inside");
    let target = world.root.join("exports/hoshi.zip");
    let written = zip::export_to_chosen(&world.storage, ChosenPath::from_save_dialog(target))
        .expect("export");
    assert_eq!(
        written,
        std::fs::canonicalize(world.root.join("exports/hoshi.zip")).expect("written")
    );
    // 書き出し先の Zip 自身は中身に入らない（2 回目でも入れ子にならない）
    let again = zip::export_to_chosen(
        &world.storage,
        ChosenPath::from_save_dialog(world.root.join("exports/hoshi.zip")),
    )
    .expect("again");
    let imported = zip::import(&std::fs::read(again).expect("zip")).expect("import");
    assert!(imported
        .files
        .iter()
        .all(|(path, _)| path.as_str() != "exports/hoshi.zip"));
    let leftovers: Vec<String> = tree(&world.root)
        .into_keys()
        .filter(|name| name.ends_with(".novnow-tmp"))
        .collect();
    assert_eq!(leftovers, Vec::<String>::new());
}

#[test]
fn zip_import_never_writes_outside_the_new_root() {
    let world = World::new("policy-zip-import");
    let before = tree(&world.outside);
    // 悪意のある名前（zip slip）を含む Zip を手で作る
    let bytes = raw_zip(&[
        ("../outside/evil.txt", "x"),
        ("/tmp/novnow-evil-abs.txt", "x"),
        ("C:\\evil.txt", "x"),
        ("project.json", "{\"schema_version\":1}"),
        ("contents/01-01.txt", "@01-01-01\n"),
    ]);
    let imported = zip::import(&bytes).expect("import");
    let target = world.dir.join("imported");
    let storage = FsStorage::create(world.gate.clone(), &target).expect("create");
    let report = zip::import_project(&storage, &imported).expect("write");
    assert_eq!(report.written.len(), 2);
    assert_eq!(imported.skipped.len(), 3);
    assert_eq!(
        sandbox::files(&target),
        ["contents/01-01.txt", "project.json"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    world.outside_untouched(&before);
    assert!(!Path::new("/tmp/novnow-evil-abs.txt").exists());
}

#[cfg(unix)]
mod symlinks {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn a_symlinked_directory_pointing_outside_is_rejected() {
        // 本文・設定・提案・ログ・エクスポートのディレクトリをプロジェクト外へのリンクにする
        let world = World::new("policy-symlink-dir");
        let before = tree(&world.outside);
        std::fs::remove_dir_all(world.root.join("contents")).expect("rm");
        symlink(&world.outside, world.root.join("contents")).expect("symlink");
        for dir in [".novnow", "agents", "exports"] {
            symlink(&world.outside, world.root.join(dir)).expect("symlink");
        }
        let storage = &world.storage;
        rejected(|| block_on(storage.write(&path("contents/01-01.txt"), "x")));
        rejected(|| block_on(storage.write(&path("contents/victim.txt"), "x")));
        rejected(|| block_on(storage.write(&path(".novnow/ui.json"), "{}")));
        rejected(|| block_on(storage.write(&path("agents/proposals/H0001.md"), "x")));
        rejected(|| block_on(storage.append(&path("agents/log/2026-10-13.jsonl"), "x")));
        rejected(|| block_on(storage.write(&path("exports/out.txt"), "x")));
        rejected(|| storage.write_bytes(&path("exports/a.zip"), b"x"));
        rejected(|| zip::export_project(storage, "novel").map_err(storage_error));
        rejected(|| block_on(storage.remove(&path("contents/victim.txt"))));
        world.outside_untouched(&before);
    }

    #[test]
    fn a_symlinked_file_pointing_outside_is_rejected_and_left_alone() {
        let world = World::new("policy-symlink-file");
        let before = tree(&world.outside);
        std::fs::remove_file(world.root.join("project.json")).expect("rm");
        symlink(
            world.outside.join("victim.txt"),
            world.root.join("project.json"),
        )
        .expect("ln");
        let storage = &world.storage;
        rejected(|| block_on(storage.write(&path("project.json"), "{}")));
        rejected(|| block_on(storage.append(&path("project.json"), "{}")));
        world.outside_untouched(&before);
        // 消すのはリンク自身（プロジェクト内のエントリ）なので許され、行き先は残る
        block_on(storage.remove(&path("project.json"))).expect("remove link");
        assert!(world.outside.join("victim.txt").is_file());
        world.outside_untouched(&before);
    }

    #[test]
    fn a_dangling_symlink_is_not_followed() {
        let world = World::new("policy-dangling");
        let before = tree(&world.outside);
        symlink(world.outside.join("missing-dir"), world.root.join("agents")).expect("ln");
        rejected(|| block_on(world.storage.append(&path("agents/log/a.jsonl"), "x")));
        assert!(!world.outside.join("missing-dir").exists());
        world.outside_untouched(&before);
    }

    #[test]
    fn a_planted_temporary_file_link_does_not_redirect_the_atomic_write() {
        // `.<name>.novnow-tmp` をプロジェクト外へのリンクにしておいても、リンクごと消してから作る
        let world = World::new("policy-tmp-link");
        let before = tree(&world.outside);
        symlink(
            world.outside.join("victim.txt"),
            world.root.join("contents/.01-01.txt.novnow-tmp"),
        )
        .expect("ln");
        block_on(
            world
                .storage
                .write(&path("contents/01-01.txt"), "新しい本文\n"),
        )
        .expect("write");
        assert_eq!(
            std::fs::read_to_string(world.root.join("contents/01-01.txt")).expect("read"),
            "新しい本文\n"
        );
        world.outside_untouched(&before);
    }

    #[test]
    fn the_save_dialog_exception_judges_the_link_target() {
        // 指定された `.zip` が外の別のファイルへのリンクなら、行き先（`.zip` でない）で判定する
        let world = World::new("policy-chosen-link");
        let before = tree(&world.outside);
        let link = world.dir.join("desk/novel.zip");
        std::fs::create_dir_all(link.parent().expect("parent")).expect("mkdir");
        symlink(world.outside.join("victim.txt"), &link).expect("ln");
        let violation = world
            .gate
            .check_chosen_export(&ChosenPath::from_save_dialog(link.clone()))
            .expect_err("link");
        assert_eq!(violation.reason, Violation::NotAnExport);
        rejected(|| {
            world
                .gate
                .write_chosen_export(ChosenPath::from_save_dialog(link.clone()), b"x")
        });
        // 履歴ファイルへのリンクも同じ
        let history = world.gate.history_file().expect("history");
        world.gate.write_history(b"{}").expect("history");
        let to_history = world.dir.join("desk/history.zip");
        symlink(&history, &to_history).expect("ln");
        rejected(|| {
            world
                .gate
                .write_chosen_export(ChosenPath::from_save_dialog(to_history.clone()), b"x")
        });
        assert_eq!(std::fs::read(&history).expect("read"), b"{}");
        world.outside_untouched(&before);
    }

    #[test]
    fn a_history_file_replaced_by_a_link_is_rejected() {
        let world = World::new("policy-history-link");
        let before = tree(&world.outside);
        let history = world.gate.history_file().expect("history");
        std::fs::create_dir_all(history.parent().expect("parent")).expect("mkdir");
        symlink(world.outside.join("victim.txt"), &history).expect("ln");
        rejected(|| world.gate.write_history(b"{}"));
        world.outside_untouched(&before);
    }

    fn storage_error(error: zip::ArchiveError) -> StorageError {
        match error {
            zip::ArchiveError::Storage(inner) => inner,
            other => panic!("{other:?}"),
        }
    }
}

// ============================================================ 規則 10 の総合（§19.10 の機械化）

#[test]
fn after_a_full_session_the_only_files_outside_the_projects_are_the_history_and_the_chosen_zip() {
    // §19.10「実機で一通り操作した後、プロジェクト外に作られたファイルが履歴 1 個と、保存ダイアログで
    // 指定した Zip だけ」（第 2.10 版）をストレージ層でなぞる（手動の手順は docs/verify-rule10.md）。
    // HOME を砂場に向けた Linux として、履歴は §13.2.1 の既定の場所（~/.local/state/novnow/recent.json）
    // に書かせる。
    let dir = TempDir::new("policy-session");
    let root = minimal_project(&dir, "novels/hoshi");
    let home = dir.mkdir("home");
    let desktop = dir.mkdir("home/Desktop");
    let env = FakeEnv::new(novnow::ports::env::Os::Linux).home(&home);
    let before = tree(dir.path());
    let gate = Arc::new(FsGate::new(&env));
    let storage = FsStorage::open(gate.clone(), &root).expect("open");
    let recent = RecentFile::new(gate.clone());
    recent.push(storage.root(), &NOW).expect("history");
    // 読み込み → 編集 → 保存 → ログ → 提案 → エクスポート → 削除
    let loaded = block_on(novnow::adapters::project_load::load(&storage)).expect("load");
    assert_eq!(loaded.validation, Ok(()));
    block_on(storage.write(&path("contents/01-01.txt"), "@01-01-01\n「やあ」\n")).expect("w");
    block_on(storage.write(&path(".novnow/ui.json"), "{\"schema_version\":1}\n")).expect("w");
    block_on(storage.append(&path("agents/log/2026-10-13.jsonl"), "{}\n")).expect("a");
    block_on(storage.write(&path("agents/proposals/H0001.md"), "# 提案\n")).expect("w");
    let zip_path = zip::export_project(&storage, "hoshi").expect("export");
    zip::export_to_chosen(
        &storage,
        ChosenPath::from_save_dialog(desktop.join("hoshi.zip")),
    )
    .expect("export to the chosen path");
    block_on(storage.remove(&path(".novnow/ui.json"))).expect("rm");
    // Zip を別のディレクトリに取り込む（新しいプロジェクトルート）
    let bytes = storage.read_bytes(&zip_path).expect("zip");
    let imported_root = dir.join("novels/imported");
    let imported = FsStorage::create(gate.clone(), &imported_root).expect("create");
    zip::import_project(&imported, &zip::import(&bytes).expect("import")).expect("write");
    recent.push(imported.root(), &NOW).expect("history");
    // プロジェクトの外で増えたファイル
    let after = tree(dir.path());
    let added: Vec<&String> = after
        .keys()
        .filter(|name| !before.contains_key(*name))
        .filter(|name| !name.starts_with("novels/hoshi/") && !name.starts_with("novels/imported/"))
        .collect();
    assert_eq!(
        added,
        vec![
            "home/.local/state/novnow/recent.json",
            "home/Desktop/hoshi.zip"
        ]
    );
    // 履歴の中身は path と opened_at だけ（§19.10）
    let text = std::fs::read_to_string(home.join(".local/state/novnow/recent.json")).expect("read");
    let value: serde_json::Value = serde_json::from_str(&text).expect("json");
    let keys: Vec<&String> = value.as_object().expect("object").keys().collect();
    assert_eq!(keys, vec!["items", "schema_version"]);
    for item in value["items"].as_array().expect("items") {
        let keys: Vec<&String> = item.as_object().expect("item").keys().collect();
        assert_eq!(keys, vec!["opened_at", "path"]);
    }
}

// ============================================================ 補助

/// 名前を自由に付けた Zip を作る（`zip::export` は安全な名前しか書かないので、攻撃用は手で作る）。
fn raw_zip(entries: &[(&str, &str)]) -> Vec<u8> {
    use std::io::Write;
    let mut writer = ::zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = ::zip::write::SimpleFileOptions::default();
    for (name, text) in entries {
        writer.start_file(*name, options).expect("start");
        writer.write_all(text.as_bytes()).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}
