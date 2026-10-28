//! ストレージのポート（§3.1・§13.1.2、`docs/history.md` B-90）。
//!
//! **すべてのファイル書き込みはこの trait を通る**（§13.1.2 MUST）。実装は 2 つ:
//!
//! | 実装 | 置き場所 | 規則 10 の守り方 |
//! |---|---|---|
//! | ネイティブ fs | `adapters::storage_fs::FsStorage` | 書き込みの system call は関門（`FsGate`）だけが持ち、毎回 `core::policy::WritePolicy` で判定する |
//! | IndexedDB | `adapters::storage_idb`（wasm） | 1 プロジェクトのレコードの中だけを読み書きする（§13.3）。`app` ストアは `recent_projects` だけ |
//!
//! # 非同期（B-90）
//!
//! IndexedDB は非同期の API しか持たないので、**メソッドは `Future` を返す**（両方の実装で 1 つの
//! trait にするため）。ネイティブの実装は中で同期 I/O をして即座に完了する `Future` を返すので、
//! CLI やテストは `adapters::block_on` で待てばよい。Web の `Future` は `Send` でない（`JsValue` を
//! 持つ）ので、`Send` は要求しない。
//!
//! # パス
//!
//! パスは**プロジェクトルートからの相対**（[`ProjectPath`]）。絶対パスや `..` は型を作る時点で
//! 拒否されるので、この trait から字句的にルートの外へは書けない（シンボリックリンクによる脱出は
//! ネイティブの関門が止める）。テキストは書かれたままのバイト列で読み書きする（BOM・改行の正規化は
//! 呼び出し側。§5.0.4 の `lexer::normalize_on_load` / `normalize_for_save`）。

use std::future::Future;

pub use crate::core::policy::{FileEntry, PathError, PolicyViolation, ProjectPath, Stamp};

/// ストレージの失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// 規則 10 の違反（**E060**。`PolicyViolation::diagnostic`）。リリースビルドでだけ返る
    /// （デバッグビルドは関門が panic する、§13.1.2）。
    Policy(PolicyViolation),
    /// プロジェクト内の相対パスにできない。
    InvalidPath(PathError),
    /// ファイルが無い。
    NotFound(ProjectPath),
    /// UTF-8 として読めない（§4.3-3）。
    Encoding(ProjectPath),
    /// I/O の失敗（権限・ディスクフル等。§8.3 の `SaveFailed` に入る）。
    Io {
        /// 対象（プロジェクト内なら相対パス、そうでなければ表示用のパス）。
        path: String,
        /// 種類。
        kind: std::io::ErrorKind,
        /// OS / ブラウザのメッセージ（ログ用。ユーザー向けの文言ではない）。
        message: String,
    },
    /// 保存領域の上限（Web の `QuotaExceededError`、§13.3.6。`SaveFailed` として扱う）。
    QuotaExceeded,
    /// ストレージが使えない（IndexedDB が無い等。**E062**、§13.3.6）。
    Unavailable(String),
    /// この実装ではできない操作（Web でバイナリを書く等）。
    Unsupported,
}

/// プロジェクト 1 つ分のストレージ（§3.1 の `Storage`）。
pub trait Storage {
    /// プロジェクトに書き込めるか（§4.1 の W041。読み取り専用メディアでは `false`）。
    ///
    /// 実装は**プロジェクトの中へ試し書きを 1 回**して決める（モードビットだけでは、読み取り専用で
    /// マウントされた媒体・書き込みを拒否するサンドボックス・容量の尽きた領域が分からない）。
    /// 書けなくても何も残さない。結果は実装が覚えてよい（毎回書かない）。既定は `true`
    /// （ディレクトリを持たない IndexedDB・メモリ上の仮想プロジェクトは常に書ける）。
    fn writable(&self) -> impl Future<Output = Result<bool, StorageError>> {
        async { Ok(true) }
    }

    /// 全ファイルの一覧（パスの昇順）。`.novnow/` も含む（使い道に応じて呼び出し側が選ぶ）。
    /// `.git/` の中は見ない（§14.2.1「`.git/` を読まない」）。原子的保存の一時ファイルも含めない。
    fn list(&self) -> impl Future<Output = Result<Vec<FileEntry>, StorageError>>;

    /// ディレクトリの一覧（パスの昇順。§4.3 の必須ディレクトリの判定に使う）。
    /// ディレクトリを持たないストレージ（IndexedDB）は `None`（必須ディレクトリは常にあるとみなす）。
    fn dirs(&self) -> impl Future<Output = Result<Option<Vec<ProjectPath>>, StorageError>>;

    /// テキストを読む（UTF-8 として読めなければ [`StorageError::Encoding`]）。
    fn read(&self, path: &ProjectPath) -> impl Future<Output = Result<String, StorageError>>;

    /// プロジェクト内の資産のバイト列を読む（`assets/` の画像など）。実装できない場合は
    /// [`StorageError::Unsupported`] を返す（Web は取り込んだバイナリだけを読める）。
    fn read_binary(
        &self,
        _path: &ProjectPath,
    ) -> impl Future<Output = Result<Vec<u8>, StorageError>> {
        async { Err(StorageError::Unsupported) }
    }

    /// テキストを**原子的に**書く（§8.3）。親ディレクトリは無ければ作る。新しいスタンプを返す
    /// （外部変更の検出で自分の保存を候補にしないため、`policy::external_change`）。
    fn write(
        &self,
        path: &ProjectPath,
        text: &str,
    ) -> impl Future<Output = Result<Stamp, StorageError>>;

    /// 末尾に追記する（`agents/log/*.jsonl` 用、§5.10.5）。無ければ作る。
    fn append(
        &self,
        path: &ProjectPath,
        text: &str,
    ) -> impl Future<Output = Result<Stamp, StorageError>>;

    /// ファイルを消す。無ければ [`StorageError::NotFound`]。
    fn remove(&self, path: &ProjectPath) -> impl Future<Output = Result<(), StorageError>>;
}
