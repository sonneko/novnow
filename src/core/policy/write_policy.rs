//! 規則 10 の関門の判定（§13.1.2、`docs/history.md` の作業項目 1・B-91）。
//!
//! novnow が書き込んでよい場所は **2 つだけ**（§13.1.1 MUST）:
//!
//! 1. 現在開いているプロジェクトルート配下
//! 2. パス履歴ファイル 1 個（§13.2）。その親ディレクトリの作成も履歴の書き込みの一部として許す（第 2.9 版）
//!
//! **特例は 1 つだけ**（§13.1.1、ユーザー裁定 2026-10-13・第 2.10 / 2.11 版）: エクスポート（Zip と
//! テキスト）で、ユーザーが保存先として**明示した**パス（保存ダイアログ・`novnow export --out`）は
//! プロジェクトの外でも書いてよい。書けるのはエクスポートの種類ごとの拡張子（[`ExportKind`]）の
//! ファイルだけ。判定は [`WritePolicy::check_chosen_export`] だけが行い、[`WritePolicy::check`] は
//! 特例を知らない（ほかの書き込みの口が特例を使えないようにするため。`docs/history.md` B-99・B-109）。
//!
//! # 分担（§13.1.2 第 2.9 版）
//!
//! core は I/O を持たない（§3.2）ので、**シンボリックリンクの解決は adapters**
//! （`adapters::storage_fs` の関門）が行い、ここの [`WritePolicy::check`] は**解決済みの絶対パス**を
//! 字句的に判定する。`..` は正規化してから判定し、ルートより上に出るものは拒否する。
//! 違反時に panic するか E060 を返すか（§13.1.2「デバッグビルドは `panic!`」）も関門の側で決める。
//!
//! # プロジェクト内の相対パス
//!
//! ストレージの API はパスを [`ProjectPath`]（ルートからの相対・`/` 区切り）で受ける。`..` や
//! 絶対パスは**型を作る時点で**拒否されるので、相対パスを使う限り字句的にはルートの外に出られない
//! （シンボリックリンクによる脱出は関門が [`WritePolicy::check`] で止める）。

use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};

// ============================================================ WritePolicy

/// 書き込みの許可表（§13.1.2 の `WritePolicy`）。
///
/// 仕様の例は `history_file: PathBuf` だが、履歴の置き場所が決まらない環境（`HOME` も
/// `APPDATA` も無い・Web）もあるので `Option` で持つ（`None` なら履歴は書かない）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WritePolicy {
    /// 開いているプロジェクトのルート（解決済みの絶対パス）。開いていないときは `None`。
    project_root: Option<PathBuf>,
    /// 唯一の例外（解決済みの絶対パス）。
    history_file: Option<PathBuf>,
}

/// 許可された理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allowed {
    /// プロジェクトルート配下（ルート自身を含む）。
    Project,
    /// 履歴ファイルそのもの。
    History,
    /// 履歴ファイルの祖先ディレクトリ（作成だけに使う。[`WritePolicy::check_dir`]）。
    HistoryDir,
    /// ユーザーが保存先として明示したエクスポート（プロジェクトの外。§13.1.1 の特例、第 2.10 / 2.11 版）。
    /// [`WritePolicy::check_chosen_export`] だけが返す。
    ChosenExport,
}

/// 拒否の理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Violation {
    /// 絶対パスでない（関門は解決済みの絶対パスを渡す決まり）。
    Relative,
    /// `..` がルートより上に出る。
    Escape,
    /// プロジェクトを開いていない（書けるのは履歴だけ）。
    NoProject,
    /// プロジェクトの外（§13.1.1）。
    Outside,
    /// シンボリックリンクを解決できない（壊れたリンク・読めないディレクトリ）。行き先を
    /// 確かめられないので書かない（adapters の関門が使う）。
    Unresolved,
    /// 明示された保存先の特例（§13.1.1）を使えない: ファイル名がエクスポートの種類の拡張子
    /// （[`ExportKind::extensions`]）で終わらない、または履歴ファイル（[`WritePolicy::check_chosen_export`]）。
    NotAnExport,
}

/// 特例で書けるエクスポートの種類（§13.1.1）。種類ごとに書ける拡張子が決まっている
/// （`.zip` を装って原稿や設定ファイルを潰させないため）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportKind {
    /// Zip（§13.5）。`.zip` だけ。
    Zip,
    /// アノテーションを除いたテキスト（§17.2）。`.txt` と `.md` だけ。
    Text,
}

impl ExportKind {
    /// 書ける拡張子（小文字。比べるときは大文字小文字を問わない）。
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Zip => &["zip"],
            Self::Text => &["txt", "md"],
        }
    }

    /// `path` の拡張子がこの種類のものか。
    pub fn accepts(self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                self.extensions()
                    .iter()
                    .any(|allowed| extension.eq_ignore_ascii_case(allowed))
            })
    }
}

/// 規則 10 の違反（§13.1.2）。表示は **E060**（[`PolicyViolation::diagnostic`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyViolation {
    /// 書こうとしたパス。
    pub target: PathBuf,
    /// 理由。
    pub reason: Violation,
}

impl PolicyViolation {
    /// E060「プロジェクトの外には書き込めません: {0}」（§7.1。文書位置を持たない）。
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::new(
            DiagnosticCode::E060,
            None,
            vec![Arg::text(self.target.to_string_lossy())],
        )
    }
}

impl fmt::Display for PolicyViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 開発者向け（ログ・panic のメッセージ）。ユーザー向けの文言は E060 を ui/i18n が引く
        write!(
            formatter,
            "E060 write policy violation ({:?}): {}",
            self.reason,
            self.target.to_string_lossy()
        )
    }
}

impl WritePolicy {
    /// 履歴ファイルだけを許す（プロジェクトを開く前）。`history_file` は解決済みの絶対パス。
    /// 絶対パスでなければ履歴も書かない（`None` と同じ）。
    pub fn new(history_file: Option<PathBuf>) -> Self {
        Self {
            project_root: None,
            history_file: history_file
                .filter(|path| path.is_absolute())
                .and_then(|path| normalize(&path)),
        }
    }

    /// プロジェクトを開く（それまでのプロジェクトは閉じる）。`root` は解決済みの絶対パス。
    pub fn open(&mut self, root: &Path) -> Result<(), PolicyViolation> {
        let violation = |reason| PolicyViolation {
            target: root.to_path_buf(),
            reason,
        };
        if !root.is_absolute() {
            return Err(violation(Violation::Relative));
        }
        let normal = normalize(root).ok_or_else(|| violation(Violation::Escape))?;
        self.project_root = Some(normal);
        Ok(())
    }

    /// プロジェクトを閉じる（以後は履歴しか書けない）。
    pub fn close(&mut self) {
        self.project_root = None;
    }

    /// 開いているプロジェクトのルート。
    pub fn project_root(&self) -> Option<&Path> {
        self.project_root.as_deref()
    }

    /// 履歴ファイル。
    pub fn history_file(&self) -> Option<&Path> {
        self.history_file.as_deref()
    }

    /// **ファイル**を書いてよいか（§13.1.2 の `check`）。`target` は解決済みの絶対パス。
    ///
    /// 許すのはプロジェクトルート配下（[`Allowed::Project`]）と履歴ファイルそのもの
    /// （[`Allowed::History`]）だけ。どちらで許されたかを返すので、関門は「履歴の書き込み口からは
    /// 履歴だけ・それ以外の口からはプロジェクト内だけ」に絞れる。
    pub fn check(&self, target: &Path) -> Result<Allowed, PolicyViolation> {
        let normal = self.normal(target)?;
        if self.history_file.as_ref() == Some(&normal) {
            return Ok(Allowed::History);
        }
        self.inside_project(target, &normal)
    }

    /// ユーザーが保存先として明示したエクスポートを書いてよいか（§13.1.1 の特例、第 2.10 / 2.11 版）。
    /// `target` は解決済みの絶対パス（シンボリックリンクなら行き先）。
    ///
    /// ファイル名が `kind` の拡張子（[`ExportKind::extensions`]）で終わらないもの・履歴ファイルは
    /// [`Violation::NotAnExport`]。プロジェクトの中なら [`Allowed::Project`]（ふつうの原子的保存で書く）、
    /// 外なら [`Allowed::ChosenExport`]。プロジェクトを開いていなくても外のパスは許す。
    /// **許可は覚えない**（関門がその 1 回の書き込みの直前に呼ぶ）。
    pub fn check_chosen_export(
        &self,
        target: &Path,
        kind: ExportKind,
    ) -> Result<Allowed, PolicyViolation> {
        let normal = self.normal(target)?;
        if !kind.accepts(&normal) || self.history_file.as_ref() == Some(&normal) {
            return Err(PolicyViolation {
                target: target.to_path_buf(),
                reason: Violation::NotAnExport,
            });
        }
        Ok(self
            .inside_project(target, &normal)
            .unwrap_or(Allowed::ChosenExport))
    }

    /// **ディレクトリ**を作ってよいか。プロジェクトルート配下（ルート自身を含む）と、
    /// 履歴ファイルの祖先（[`Allowed::HistoryDir`]、§13.2 の `novnow/` 等）を許す。
    pub fn check_dir(&self, dir: &Path) -> Result<Allowed, PolicyViolation> {
        let normal = self.normal(dir)?;
        if let Some(history) = &self.history_file {
            if history != &normal && history.starts_with(&normal) {
                return Ok(Allowed::HistoryDir);
            }
        }
        self.inside_project(dir, &normal)
    }

    /// 絶対パスか確かめて正規化する。
    fn normal(&self, target: &Path) -> Result<PathBuf, PolicyViolation> {
        let violation = |reason| PolicyViolation {
            target: target.to_path_buf(),
            reason,
        };
        if !target.is_absolute() {
            return Err(violation(Violation::Relative));
        }
        normalize(target).ok_or_else(|| violation(Violation::Escape))
    }

    /// プロジェクトルート配下か。
    fn inside_project(&self, target: &Path, normal: &Path) -> Result<Allowed, PolicyViolation> {
        let reason = match &self.project_root {
            None => Violation::NoProject,
            Some(root) if normal.starts_with(root) => return Ok(Allowed::Project),
            Some(_) => Violation::Outside,
        };
        Err(PolicyViolation {
            target: target.to_path_buf(),
            reason,
        })
    }
}

/// パスを字句的に正規化する（`.` を除き、`..` で 1 つ戻る）。**ファイルシステムを見ない**。
///
/// `..` がルート（または相対パスの先頭）より上に出るときは `None`（脱出として拒否する）。
/// シンボリックリンクは解決しないので、呼び出し側（adapters）が先に解決しておくこと。
pub fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    let mut depth = 0usize;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return None;
                }
                out.pop();
                depth -= 1;
            }
            Component::Normal(name) => {
                out.push(name);
                depth += 1;
            }
        }
    }
    Some(out)
}

// ============================================================ ProjectPath

/// プロジェクトルートからの相対パス（`/` 区切り。§13.3.2「先頭スラッシュ禁止」）。
///
/// **作れた時点で字句的にルートの外に出ない**ことが保証される: 空・絶対パス・`\`・`.` / `..` の
/// 要素・空の要素（`a//b`・末尾の `/`）・`:`（Windows のドライブ名と代替データストリーム）・
/// 制御文字を拒否する。Zip の取り込み（§13.5）や IndexedDB のキー（§13.3.2）もこの型で受ける。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProjectPath(String);

/// [`ProjectPath`] にできない理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathError {
    /// 空。
    Empty,
    /// `/` で始まる（絶対パス）。
    Absolute,
    /// `\` を含む（区切りは `/` だけ）。
    Backslash,
    /// `.` の要素。
    CurrentDir,
    /// `..` の要素（ルートの外に出うる）。
    ParentDir,
    /// 空の要素（`a//b`・末尾の `/`）。
    EmptySegment,
    /// `:` を含む（`C:` のようなドライブ名・`file:stream`）。
    Colon,
    /// 制御文字を含む。
    Control,
}

impl ProjectPath {
    /// 文字列から作る（規則は型の説明のとおり）。
    pub fn new(path: &str) -> Result<Self, PathError> {
        if path.is_empty() {
            return Err(PathError::Empty);
        }
        if path.starts_with('/') {
            return Err(PathError::Absolute);
        }
        if path.contains('\\') {
            return Err(PathError::Backslash);
        }
        if path.contains(':') {
            return Err(PathError::Colon);
        }
        if path.chars().any(char::is_control) {
            return Err(PathError::Control);
        }
        for segment in path.split('/') {
            match segment {
                "" => return Err(PathError::EmptySegment),
                "." => return Err(PathError::CurrentDir),
                ".." => return Err(PathError::ParentDir),
                _ => {}
            }
        }
        Ok(Self(path.to_owned()))
    }

    /// 文字列として。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `/` で区切った要素。
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// 最後の要素（ファイル名）。
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// 親ディレクトリ（ルート直下なら `None`）。
    pub fn parent(&self) -> Option<&str> {
        self.0.rsplit_once('/').map(|(parent, _)| parent)
    }

    /// `dir`（`exports` のような相対ディレクトリ）の配下か。
    pub fn is_under(&self, dir: &str) -> bool {
        self.0
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/'))
    }

    /// `.novnow/` の配下か（§4.4 のローカル状態。Zip には入れない、§13.5）。
    pub fn is_local_state(&self) -> bool {
        self.is_under(LOCAL_STATE_DIR)
    }

    /// 名前がドットで始まる要素を含むか（`.git/`・`.DS_Store`・原子的保存の一時ファイル等）。
    pub fn is_hidden(&self) -> bool {
        self.segments().any(|segment| segment.starts_with('.'))
    }

    /// ルート（解決済みの絶対パス）につないだネイティブのパス（字句の結合だけで I/O はしない）。
    pub fn under(&self, root: &Path) -> PathBuf {
        let mut out = root.to_path_buf();
        for segment in self.segments() {
            out.push(segment);
        }
        out
    }
}

/// `.novnow/`（§4.4）。
pub const LOCAL_STATE_DIR: &str = ".novnow";

impl fmt::Display for ProjectPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl AsRef<str> for ProjectPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ProjectPath {
    type Error = PathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl TryFrom<&str> for ProjectPath {
    type Error = PathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ProjectPath> for String {
    fn from(value: ProjectPath) -> Self {
        value.0
    }
}

impl fmt::Display for PathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
