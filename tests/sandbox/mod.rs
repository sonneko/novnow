//! ストレージのテストの共通部品（M5）。一時ディレクトリ・偽の環境・固定の時計・木の写し。
//!
//! テストはプロセスの環境変数を書き換えない（並列に走るため）。履歴ファイルの場所は
//! [`FakeEnv`] で渡す。一時ディレクトリは OS の一時ディレクトリの下に作り、`Drop` で消す。

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use novnow::adapters::storage_fs::{FsGate, FsStorage};
use novnow::ports::clock::Clock;
use novnow::ports::env::{Env, Os};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// テストごとの一時ディレクトリ（`Drop` で消す）。パスは解決済み（macOS の `/var` → `/private/var`）。
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let raw = std::env::temp_dir().join(format!("novnow-{label}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&raw);
        fs::create_dir_all(&raw).expect("一時ディレクトリを作れない");
        let path = fs::canonicalize(&raw).expect("canonicalize");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn join(&self, relative: &str) -> PathBuf {
        self.path.join(relative)
    }

    /// ファイルを書く（親ディレクトリも作る）。テストの準備用。
    pub fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(&path, text).expect("write");
        path
    }

    pub fn mkdir(&self, relative: &str) -> PathBuf {
        let path = self.join(relative);
        fs::create_dir_all(&path).expect("mkdir");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // `docs/verify-rule10.md` のファイル差分の手順で中身を見るときは残す
        if std::env::var_os("NOVNOW_TEST_KEEP").is_none() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// 表を引くだけの環境。
#[derive(Debug, Clone)]
pub struct FakeEnv {
    pub vars: BTreeMap<String, String>,
    pub os: Os,
    pub home: Option<PathBuf>,
}

impl FakeEnv {
    pub fn new(os: Os) -> Self {
        Self {
            vars: BTreeMap::new(),
            os,
            home: None,
        }
    }

    /// 履歴ファイルの場所を `NOVNOW_RECENT_FILE` で決めた Linux。
    pub fn with_history(path: &Path) -> Self {
        Self::new(Os::Linux).var("NOVNOW_RECENT_FILE", &path.to_string_lossy())
    }

    pub fn var(mut self, name: &str, value: &str) -> Self {
        self.vars.insert(name.to_owned(), value.to_owned());
        self
    }

    pub fn home(mut self, home: &Path) -> Self {
        self.home = Some(home.to_path_buf());
        self
    }
}

impl Env for FakeEnv {
    fn var(&self, name: &str) -> Option<String> {
        self.vars
            .get(name)
            .cloned()
            .filter(|value| !value.is_empty())
    }

    fn os(&self) -> Os {
        self.os
    }

    fn home_dir(&self) -> Option<PathBuf> {
        self.home.clone()
    }
}

/// 固定の時計。
#[derive(Debug, Clone, Copy)]
pub struct FixedClock {
    pub millis: i64,
    pub offset: i32,
}

impl Clock for FixedClock {
    fn now_millis(&self) -> i64 {
        self.millis
    }

    fn utc_offset_minutes(&self) -> i32 {
        self.offset
    }
}

/// 2026-10-13T03:04:05Z。
pub const NOW: FixedClock = FixedClock {
    millis: 1_791_860_645_000,
    offset: 0,
};

/// 最小のプロジェクト（§4.2 の必須パス）を `root` に作る。
pub fn minimal_project(dir: &TempDir, root: &str) -> PathBuf {
    dir.write(
        &format!("{root}/project.toml"),
        "schema_version = 2\ntitle = \"星屑の時計\"\n",
    );
    dir.write(
        &format!("{root}/characters.md"),
        "# 星野 アカリ\nalias: アカリ\n",
    );
    dir.write(
        &format!("{root}/words.md"),
        "# 星屑の時計\ndefinition: 時を止める懐中時計\n",
    );
    dir.write(
        &format!("{root}/materials/index.json"),
        "{\"schema_version\":1,\"next_id\":1,\"items\":[]}\n",
    );
    dir.write(
        &format!("{root}/plots/all.cnov"),
        "@01\nsummary: 出会い\n;\n",
    );
    dir.write(
        &format!("{root}/contents/01-01.cnov"),
        "@01-01-01\n「こんにちは」\n",
    );
    dir.join(root)
}

/// 関門と、`root` を開いたストレージ。
pub fn open(env: &FakeEnv, root: &Path) -> (Arc<FsGate>, FsStorage) {
    let gate = Arc::new(FsGate::new(env));
    let storage = FsStorage::open(gate.clone(), root).expect("open");
    (gate, storage)
}

/// `root` の下の全ファイルと中身（相対パス → バイト列）。シンボリックリンクは辿らない。
pub fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    collect(root, root, &mut out);
    out
}

fn collect(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = entry.file_type().expect("file_type");
        let relative = path
            .strip_prefix(root)
            .expect("prefix")
            .to_string_lossy()
            .replace('\\', "/");
        if kind.is_dir() {
            collect(root, &path, out);
        } else if kind.is_symlink() {
            out.insert(relative, b"<symlink>".to_vec());
        } else {
            out.insert(relative, fs::read(&path).expect("read"));
        }
    }
}

/// `root` の下の全ファイルの相対パス。
pub fn files(root: &Path) -> BTreeSet<String> {
    tree(root).into_keys().collect()
}
