//! ネイティブのストレージ（`adapters::storage_fs`、§8.3・§13.1）の振る舞い。規則 10 の拒否は
//! `tests/storage_policy.rs`。ここは「書ける場所での」原子的書き込み・一覧・読み取り・追記・削除。

mod sandbox;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use novnow::adapters::block_on;
use novnow::adapters::storage_fs::{resolve, FsStorage, TMP_SUFFIX};
use novnow::core::policy::ProjectPath;
use novnow::ports::storage::{Storage, StorageError};

use sandbox::{minimal_project, open, FakeEnv, TempDir};

fn path(text: &str) -> ProjectPath {
    ProjectPath::new(text).expect("相対パス")
}

fn setup(label: &str) -> (TempDir, FsStorage) {
    let dir = TempDir::new(label);
    let root = minimal_project(&dir, "novel");
    let (_, storage) = open(
        &FakeEnv::with_history(&dir.join("state/recent.json")),
        &root,
    );
    (dir, storage)
}

#[test]
fn an_atomic_write_replaces_the_file_and_leaves_no_temporary_file() {
    let (_dir, storage) = setup("fs-atomic");
    let target = path("contents/01-01.cnov");
    let stamp = block_on(storage.write(&target, "@01-01-01\n新しい本文\n")).expect("write");
    assert_eq!(
        block_on(storage.read(&target)).expect("read"),
        "@01-01-01\n新しい本文\n"
    );
    assert_eq!(stamp.len, "@01-01-01\n新しい本文\n".len() as u64);
    assert!(stamp.modified_ms.is_some());
    let names: Vec<String> = std::fs::read_dir(storage.root().join("contents"))
        .expect("read_dir")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["01-01.cnov"], "一時ファイルが残った");
}

#[test]
fn missing_parent_directories_are_created_inside_the_project() {
    let (_dir, storage) = setup("fs-parents");
    let target = path("agents/proposals/2026/H0001.md");
    block_on(storage.write(&target, "# 提案\n")).expect("write");
    assert!(storage
        .root()
        .join("agents/proposals/2026/H0001.md")
        .is_file());
}

#[cfg(unix)]
#[test]
fn an_atomic_write_keeps_the_permissions_of_the_replaced_file() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, storage) = setup("fs-mode");
    let file = storage.root().join("characters.md");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    block_on(storage.write(&path("characters.md"), "# 新しい人\n")).expect("write");
    let mode = std::fs::metadata(&file).expect("meta").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn append_adds_to_the_end_and_creates_the_file() {
    let (_dir, storage) = setup("fs-append");
    let log = path("agents/log/2026-10-13.jsonl");
    let first = block_on(storage.append(&log, "{\"n\":1}\n")).expect("append");
    let second = block_on(storage.append(&log, "{\"n\":2}\n")).expect("append");
    assert!(second.len > first.len);
    assert_eq!(
        block_on(storage.read(&log)).expect("read"),
        "{\"n\":1}\n{\"n\":2}\n"
    );
}

#[test]
fn remove_reports_missing_files() {
    let (_dir, storage) = setup("fs-remove");
    let target = path("contents/01-01.cnov");
    block_on(storage.remove(&target)).expect("remove");
    assert_eq!(
        block_on(storage.remove(&target)),
        Err(StorageError::NotFound(target.clone()))
    );
    assert_eq!(
        block_on(storage.read(&target)),
        Err(StorageError::NotFound(target))
    );
}

#[test]
fn invalid_utf8_is_an_encoding_error() {
    let (dir, storage) = setup("fs-utf8");
    std::fs::write(dir.join("novel/words.md"), [0xff, 0xfe, b'a']).expect("write");
    assert_eq!(
        block_on(storage.read(&path("words.md"))),
        Err(StorageError::Encoding(path("words.md")))
    );
    // バイト列としては読める
    assert_eq!(
        storage.read_bytes(&path("words.md")).expect("bytes"),
        vec![0xff, 0xfe, b'a']
    );
}

#[test]
fn the_listing_skips_git_and_temporary_files_and_reports_directories() {
    let (dir, storage) = setup("fs-list");
    dir.write("novel/.git/HEAD", "ref: refs/heads/main\n");
    dir.write(
        &format!("novel/contents/.01-01.txt{TMP_SUFFIX}"),
        "書きかけ",
    );
    dir.write("novel/.novnow/ui.json", "{}");
    dir.mkdir("novel/materials/docs");
    let files: Vec<String> = block_on(storage.list())
        .expect("list")
        .into_iter()
        .map(|entry| entry.path.as_str().to_owned())
        .collect();
    assert_eq!(
        files,
        vec![
            ".novnow/ui.json",
            "characters.md",
            "contents/01-01.cnov",
            "materials/index.json",
            "plots/all.cnov",
            "project.toml",
            "words.md",
        ]
    );
    let dirs: Vec<String> = block_on(storage.dirs())
        .expect("dirs")
        .expect("fs has directories")
        .into_iter()
        .map(|found| found.as_str().to_owned())
        .collect();
    assert_eq!(
        dirs,
        vec![
            ".novnow",
            "contents",
            "materials",
            "materials/docs",
            "plots"
        ]
    );
}

#[cfg(unix)]
#[test]
fn the_listing_does_not_follow_symlinked_directories() {
    let (dir, storage) = setup("fs-list-link");
    dir.write("outside/secret.md", "外");
    std::os::unix::fs::symlink(dir.join("outside"), dir.join("novel/linked")).expect("ln");
    let files: Vec<String> = block_on(storage.list())
        .expect("list")
        .into_iter()
        .map(|entry| entry.path.as_str().to_owned())
        .collect();
    assert!(
        !files.iter().any(|name| name.starts_with("linked")),
        "{files:?}"
    );
}

#[test]
fn opening_requires_an_existing_directory() {
    let dir = TempDir::new("fs-open");
    let env = FakeEnv::with_history(&dir.join("state/recent.json"));
    let gate = std::sync::Arc::new(novnow::adapters::storage_fs::FsGate::new(&env));
    assert!(matches!(
        FsStorage::open(gate.clone(), &dir.join("missing")),
        Err(StorageError::Io { .. })
    ));
    dir.write("file.txt", "x");
    assert!(matches!(
        FsStorage::open(gate.clone(), &dir.join("file.txt")),
        Err(StorageError::Io { .. })
    ));
    // 作る口なら無いディレクトリでもよい（Zip の取り込み先・`novnow new`）
    let created = FsStorage::create(gate.clone(), &dir.join("new/novel")).expect("create");
    assert!(created.root().is_dir());
    assert_eq!(gate.project_root().as_deref(), Some(created.root()));
}

#[test]
fn resolve_canonicalizes_the_existing_part_and_keeps_the_rest() {
    let dir = TempDir::new("fs-resolve");
    dir.mkdir("a/b");
    let resolved = resolve(&dir.join("a/b/../b/new/file.txt")).expect("resolve");
    // 存在する `a/b/../b` は物理的に解決され、無い `new/file.txt` はそのまま
    assert_eq!(resolved, dir.join("a/b/new/file.txt"));
}

#[test]
fn block_on_waits_for_a_pending_future() {
    // 1 回目は Pending を返して自分を起こす Future（待ちが生じる Future でも動くこと）
    struct Twice(bool);
    impl Future for Twice {
        type Output = u32;
        fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<u32> {
            if self.0 {
                Poll::Ready(7)
            } else {
                self.0 = true;
                context.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
    assert_eq!(block_on(Twice(false)), 7);
}

#[test]
fn structural_batch_rolls_back_already_written_files_on_mid_batch_failure() {
    let (_dir, storage) = setup("fs-batch-rollback");
    let original = path("contents/01-01.cnov");
    storage
        .write_bytes(&original, b"original bytes\r\n")
        .unwrap();
    let bad = path("contents/bad.cnov");
    std::fs::create_dir(
        storage
            .root()
            .join(format!("contents/.bad.cnov{TMP_SUFFIX}")),
    )
    .unwrap();
    assert!(storage
        .write_batch(
            &[
                (original.clone(), "changed".into()),
                (bad.clone(), "new".into())
            ],
            std::slice::from_ref(&original)
        )
        .is_err());
    assert_eq!(
        storage.read_bytes(&original).unwrap(),
        b"original bytes\r\n"
    );
    assert!(!storage.absolute(&bad).exists());
}
#[test]
fn structural_batch_commits_writes_before_removing_old_paths() {
    let (_dir, storage) = setup("fs-batch-success");
    let old = path("contents/00-01.cnov");
    let new = path("contents/01-02.cnov");
    storage.write_bytes(&old, b"stock").unwrap();
    let stamps = storage
        .write_batch(
            &[(new.clone(), "assigned".into())],
            std::slice::from_ref(&old),
        )
        .unwrap();
    assert_eq!(stamps.len(), 1);
    assert_eq!(storage.read_bytes(&new).unwrap(), b"assigned");
    assert!(!storage.absolute(&old).exists());
}
