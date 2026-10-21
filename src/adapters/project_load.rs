//! プロジェクトの読み込み（§4.3・§14.2、`PLANS.md` §5.6・§9.2 B-94）。どのストレージでも同じ手順。
//!
//! 1. 一覧を取る（`Storage::list` / `Storage::dirs`）。
//! 2. `.md` / `.txt` / `.json` を読み、UTF-8 として不正なものを集める（§4.3-3）。
//! 3. `core::model::project::validate` で判定する（v0 の目印 → 必須パス → `schema_version` →
//!    文字コード）。判定は core、材料集めはここ（core は I/O を持たない、§3.2）。
//! 4. 読めた文書を §5.0.4 の読み込み時の正規化（BOM・改行）にかけて `features::Project` に渡す。
//!    設定は `project.json` の文書から集約が自分で読む（B-89）。
//!
//! 判定に失敗しても文書は読んで返す（起動を止めるかどうかは呼び出し側が決める。§14.2 の CLI は
//! 止め、§13.5 の Zip の取り込みは「不備のまま取り込む / 取り消す」を選ばせる）。
//!
//! 外部変更の監視（§8.5）の 1 回分も、どのストレージでも同じ手順なのでここに置く（[`poll`]）。
//! 1,000ms ごとに呼ぶタイマは UI（M7）が持つ（ネイティブはスレッド、Web は `setInterval`）。

use std::collections::BTreeSet;

use crate::core::features::Project;
use crate::core::model::document::{Document, DocumentFormat};
use crate::core::model::project::{validate, Listing, ProjectError, PROJECT_FILE};
use crate::core::policy::external_change::{Change, Snapshot};
use crate::core::policy::ProjectPath;
use crate::core::text::lexer;
use crate::ports::storage::{Storage, StorageError};

/// 読み込んだプロジェクト。
#[derive(Debug)]
pub struct Loaded {
    /// §4.3 の判定の材料。
    pub listing: Listing,
    /// §4.3 の判定の結果。
    pub validation: Result<(), ProjectError>,
    /// 文書を入れた集約（§3.4）。
    pub project: Project,
}

/// 文字コードを調べるファイルか（§4.3-3「すべての `.md` / `.txt` / `.json`」）。
///
/// `.novnow/`（ローカル状態。壊れていても既定値で動く、§4.4）・`exports/`（出力）・
/// ドットで始まる名前は対象外。
pub fn is_checked_text(path: &ProjectPath) -> bool {
    let name = path.file_name();
    let text = name.ends_with(".md") || name.ends_with(".txt") || name.ends_with(".json");
    text && !path.is_local_state() && !path.is_under("exports") && !path.is_hidden()
}

/// 文書として集約に入れるか（§4.2 の形式のどれか）。
pub fn is_document(path: &ProjectPath) -> bool {
    is_checked_text(path) && DocumentFormat::of_path(path.as_str()) != DocumentFormat::Unknown
}

/// §4.3 の判定の材料だけを集める（文字コードは調べない。§14.2 の起動前の検査はこれで足りる）。
pub async fn listing<S: Storage>(storage: &S) -> Result<Listing, StorageError> {
    let entries = storage.list().await?;
    let mut listing = Listing {
        files: entries
            .iter()
            .map(|entry| entry.path.as_str().to_owned())
            .collect(),
        ..Listing::default()
    };
    fill_dirs(storage, &mut listing).await?;
    if listing.files.contains(PROJECT_FILE) {
        let path = project_file();
        listing.project_json = storage.read(&path).await.ok();
    }
    Ok(listing)
}

/// 全部読む（§4.3 の判定 ＋ 文書）。
pub async fn load<S: Storage>(storage: &S) -> Result<Loaded, StorageError> {
    let entries = storage.list().await?;
    let mut listing = Listing {
        files: entries
            .iter()
            .map(|entry| entry.path.as_str().to_owned())
            .collect(),
        ..Listing::default()
    };
    fill_dirs(storage, &mut listing).await?;
    let mut documents = Vec::new();
    for entry in &entries {
        if entry.path.as_str() == ".novnow/stats.json"
            || entry.path.as_str() == ".novnow/reader.json"
        {
            if let Ok(text) = storage.read(&entry.path).await {
                documents.push(Document::new(entry.path.as_str(), text));
            }
        }
        if !is_checked_text(&entry.path) {
            continue;
        }
        let text = match storage.read(&entry.path).await {
            Ok(text) => text,
            Err(StorageError::Encoding(path)) => {
                listing.invalid_utf8.push(path.as_str().to_owned());
                continue;
            }
            // 一覧を取ってから消えたもの（外部の操作）は無かったことにする
            Err(StorageError::NotFound(_)) => continue,
            Err(error) => return Err(error),
        };
        if entry.path.as_str() == PROJECT_FILE {
            listing.project_json = Some(text.clone());
        }
        if is_document(&entry.path) {
            let normalized = lexer::normalize_on_load(&text).into_owned();
            documents.push(Document::new(entry.path.as_str(), normalized));
        }
    }
    let validation = validate(&listing);
    let mut project = Project::with_documents(Default::default(), documents);
    for entry in &entries {
        if entry.path.as_str().starts_with("assets/") {
            if entry.stamp.len > 256 * 1024 {
                project
                    .assets
                    .insert(entry.path.as_str().to_owned(), Vec::new());
                continue;
            }
            if let Ok(bytes) = storage.read_binary(&entry.path).await {
                project.assets.insert(entry.path.as_str().to_owned(), bytes);
            }
        }
    }
    Ok(Loaded {
        listing,
        validation,
        project,
    })
}

/// 外部変更の監視の 1 回分（§8.5）: 一覧を取り、前回（`snapshot`）と比べた候補を返して、
/// `snapshot` を今回の一覧に置き換える。自分の保存は `Snapshot::acknowledge` で先に覚えておけば
/// 候補にならない。候補の中身を読んで `policy::external_change::react` で反応を決めるのは呼び出し側。
pub async fn poll<S: Storage>(
    storage: &S,
    snapshot: &mut Snapshot,
) -> Result<Vec<Change>, StorageError> {
    let now = Snapshot::from_entries(storage.list().await?);
    let changes = snapshot.diff(&now);
    *snapshot = now;
    Ok(changes)
}

/// ディレクトリの一覧を足す。ディレクトリを持たないストレージなら「暗黙にある」とする（§4.3 第 2.9 版）。
async fn fill_dirs<S: Storage>(storage: &S, listing: &mut Listing) -> Result<(), StorageError> {
    match storage.dirs().await? {
        Some(dirs) => {
            listing.dirs = dirs
                .iter()
                .map(|dir| dir.as_str().to_owned())
                .collect::<BTreeSet<_>>();
        }
        None => listing.dirs_implicit = true,
    }
    Ok(())
}

fn project_file() -> ProjectPath {
    ProjectPath::new(PROJECT_FILE).expect("project.json is a valid relative path")
}
