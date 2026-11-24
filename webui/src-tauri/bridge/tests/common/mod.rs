//! ネイティブの橋渡しのテストの共通部品（一時ディレクトリ・サンプル作品・木の写し・JSON の読み取り）。
//!
//! 一時ディレクトリは OS の一時ディレクトリの下に作り、`Drop` で消す（`REF:tests/sandbox/mod.rs`
//! と同じ流儀）。**ここ（テストの準備）だけ**がファイルを直接書く。橋渡しの本体は書かない。

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// テストごとの一時ディレクトリ（`Drop` で消す）。パスは解決済み（macOS の `/var` → `/private/var`）。
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let raw =
            std::env::temp_dir().join(format!("novnow-l6-{label}-{}-{n}", std::process::id()));
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
    pub fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(&path, bytes).expect("write");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // 中身を見たいときは残す（`REF:tests/sandbox/mod.rs` と同じ合図）
        if std::env::var_os("NOVNOW_TEST_KEEP").is_none() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// 付録 C のサンプル作品（ブラウザ版が最初に入れるものと同じ一式）を `name` に置く。
pub fn sample(dir: &TempDir, name: &str) -> PathBuf {
    for file in novnow::core::template::sample_project() {
        dir.write(&format!("{name}/{}", file.path), file.text);
    }
    dir.join(name)
}

/// パスの文字列（コマンドへ渡す形）。
pub fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
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

/// 2 つの木で中身が違うパス（片方にしか無いものを含む）。
pub fn differing(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    let mut paths: Vec<String> = before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// コマンドの戻り値（文字列の JSON）を読む。
pub fn parse(reply: &str) -> Value {
    serde_json::from_str(reply).unwrap_or_else(|error| panic!("JSON でない: {error}: {reply}"))
}

/// 成功の戻り値の `result`。
pub fn ok(reply: &str) -> Value {
    let found = parse(reply);
    assert_eq!(found["ok"], Value::Bool(true), "失敗している: {reply}");
    found["result"].clone()
}

/// 失敗の戻り値の `error`。
pub fn err(reply: &str) -> Value {
    let found = parse(reply);
    assert_eq!(found["ok"], Value::Bool(false), "成功している: {reply}");
    found["error"].clone()
}
