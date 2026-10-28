//! ホスト（外部環境の口）と保存 1 回分の規則。**egui に依存しない**（§19.1 の単一実装点）。
//!
//! `Host` は「プロジェクトに対する I/O・時刻・LLM 呼び出し」を抽象化した境界で、GUI
//! （デスクトップのファイル・Web の IndexedDB）もヘッドレスのセッション（§19.2）も同じ
//! トレイトを実装する。書き込みの関門（規則 10・`FsGate`）は実装側の責務のまま。

use crate::core::diagnostics::Diagnostic;
use crate::ports::dialog::ChosenPath;

/// 開いたプロジェクトに対する I/O（デスクトップはファイル、Web は IndexedDB）。
pub trait Host {
    /// ファイルを書く（プロジェクトの中）。失敗は表示用の文言。
    fn write(&mut self, path: &str, text: &str) -> Result<(), String>;
    /// プロジェクトに書き込めるか（§4.1 の W041。読み取り専用メディアでは `false`）。
    ///
    /// 実装は**プロジェクトの中へ 1 回だけ試し書き**をして決める（[`crate::ports::storage::Storage::writable`]
    /// と同じ規則）。結果は実装が覚えてよい。既定は `true`（メモリ上の仮想プロジェクト・Web）。
    fn writable(&self) -> bool {
        true
    }
    /// 外部変更の監視 1 回分（§8.5）: 変わったファイルと中身（消えたら `None`）。
    fn poll(&mut self) -> Vec<(String, Option<String>)>;
    /// プロジェクトの名前（ヘッダ・ウィンドウ題名）。
    fn name(&self) -> String;
    /// 今の日時 `YYYYMMDD-HHMM`（テキストのエクスポートの既定の名前、§17.2）。
    fn stamp(&self) -> String;
    /// 今の日時（RFC 3339。アイデアの `created`・資料の `saved_at` / `updated_at`）。
    fn now(&self) -> String;
    /// 単調増加の秒（保存状態機のタイマ、§9.6）。持たない環境は 0（タイマは進まない）。
    fn seconds(&self) -> f64 {
        0.0
    }
    /// プロジェクトの中のファイルを消す（資料の採用で `inbox/` から `docs/` へ動かすとき、§16.1）。
    fn remove(&mut self, path: &str) -> Result<(), String> {
        Err(format!("cannot remove {path}"))
    }
    /// 「名前を付けて保存」の保存ダイアログ（§17.2・§13.1.1 の特例）。`suggested` は既定のファイル名。
    /// 取り消されたとき・ダイアログを持たない環境では `None`。
    fn choose_export(&mut self, suggested: &str) -> Option<ChosenPath> {
        let _ = suggested;
        None
    }
    /// 選ばれた保存先にテキストを書く（関門の特例の口だけを通る）。書いた場所（表示用）を返す。
    fn write_export(&mut self, chosen: ChosenPath, text: &str) -> Result<String, ExportFailure> {
        let _ = text;
        Err(ExportFailure::Failed(
            chosen.path().to_string_lossy().into_owned(),
        ))
    }
    /// 利用者が明示した書き出し先へバイナリを書く（§13.1.1 の特例の口。`project.export_zip`
    /// の `out`・`export.text` の `out`）。ダイアログを持たない環境では `Err`。
    fn write_chosen(
        &mut self,
        path: &str,
        bytes: &[u8],
        kind: crate::ports::dialog::ExportKind,
    ) -> Result<String, String> {
        let _ = (bytes, kind);
        Err(format!("cannot write to {path} here"))
    }
    /// プロジェクトの中のファイルに追記する（監査ログ `agents/log/*.jsonl`、§5.10.5）。
    fn append(&mut self, path: &str, text: &str) -> Result<(), String> {
        let _ = text;
        Err(format!("cannot append to {path}"))
    }
    /// LLM（stdio の MCP サーバ）を呼べるか（Web は呼べない、§15.8.2）。
    fn mcp_available(&self) -> bool {
        false
    }
    /// MCP の呼び出しを始める（別のスレッドで走る）。番号を返す。呼べなければ `None`。
    fn mcp_start(&mut self, job: crate::app::agents::Job) -> Option<u64> {
        let _ = job;
        None
    }
    /// 終わった呼び出し（[`crate::app::agents::Finished`]）。
    fn mcp_poll(&mut self) -> Vec<crate::app::agents::Finished> {
        Vec::new()
    }
    fn local_probe_start(&mut self, _job: crate::app::agents::Job) -> Option<u64> {
        None
    }
    fn local_probe_poll(&mut self) -> Vec<crate::app::agents::Finished> {
        Vec::new()
    }
    /// 呼び出しを取り消す。
    fn mcp_cancel(&mut self, job: u64) {
        let _ = job;
    }
    /// 環境変数（`agents/mcp.json` の `${NAME}` を解決する。値は画面にもログにも出さない）。
    fn env_var(&self, name: &str) -> Option<String> {
        let _ = name;
        None
    }
    /// ディスク上にその名前の何か（ファイル・ディレクトリ・リンク）がある。一覧に無いのにあるもの（外を指す
    /// シンボリックリンク）には既定のプロンプトを書かない（`docs/history.md` B-155）。
    fn occupied(&self, path: &str) -> bool {
        let _ = path;
        false
    }
    /// 保存 1 回分（§8.3）: `writes` を書き、全部書けたら `removals` を消す。同期の Host（デスクトップ・
    /// テスト）は既定のまま、その場で書いて結果を返す。非同期の Host（Web の IndexedDB）は始めるだけで
    /// `None` を返し、終わったら [`Host::save_finished`] が結果を返す（`docs/history.md` B-156）。
    fn save(
        &mut self,
        writes: Vec<(String, String)>,
        removals: Vec<String>,
    ) -> Option<SaveOutcome> {
        Some(save_now(self, writes, removals))
    }
    /// 非同期の保存（[`Host::save`] が `None` を返したもの）の結果。終わっていなければ `None`。
    fn save_finished(&mut self) -> Option<SaveOutcome> {
        None
    }
    /// 後から分かったこと（Web の非同期の書き込みの失敗・Zip の書き出しの結果など）。毎フレーム読む。
    fn take_events(&mut self) -> Vec<HostEvent> {
        Vec::new()
    }
    /// 未保存の変更があるか（毎フレーム。Web はタブを閉じるときの確認に使う）。
    fn unsaved_changed(&mut self, unsaved: bool) {
        let _ = unsaved;
    }
    /// Zip でエクスポートする（§13.5）。`name` は既定のファイル名（`<題>.zip`）。デスクトップは保存
    /// ダイアログで選んだ場所に書いてその場で結果を返す（取り消したら `Some(Ok(None))`）。Web は
    /// ダウンロードを始めるだけで `None` を返し、結果は [`Host::take_events`] で知らせる。
    fn export_zip(&mut self, name: &str) -> Option<Result<Option<String>, String>> {
        let _ = name;
        Some(Err("zip export is not available here".to_owned()))
    }
}

/// 保存 1 回分をその場で行う（[`Host::save`] の既定）: `writes` を書き、全部書けたら `removals` を消す
/// （動かした文書の元は、新しい側を書けてから消す）。
pub fn save_now<H: Host + ?Sized>(
    host: &mut H,
    writes: Vec<(String, String)>,
    removals: Vec<String>,
) -> SaveOutcome {
    let mut outcome = SaveOutcome::default();
    for (path, text) in writes {
        match host.write(&path, &text) {
            Ok(()) => outcome.written.push(path),
            Err(message) => outcome.failures.push((path, message)),
        }
    }
    if !outcome.failures.is_empty() {
        outcome.removals_left = removals;
        return outcome;
    }
    for path in removals {
        if let Err(message) = host.remove(&path) {
            outcome.failures.push((path.clone(), message));
            outcome.removals_left.push(path);
        }
    }
    outcome
}

/// 保存 1 回分の結果（[`Host::save`]）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveOutcome {
    /// 書けたファイル。
    pub written: Vec<String>,
    /// 書けなかった（消せなかった）ファイルと、その理由。
    pub failures: Vec<(String, String)>,
    /// まだ消していない元のファイル（次の保存でもう一度）。
    pub removals_left: Vec<String>,
}

/// Host が後から知らせること（[`Host::take_events`]。文言は `NovnowApp` がカタログで引く）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostEvent {
    /// 保存の外で書いたもの（`ui.json`・監査ログ・`exports/`）が書けなかった。
    WriteFailed(String, String),
    /// Zip を書き出した（Web はダウンロードを始めた）。
    ZipExported(String),
    /// Zip を書き出せなかった。
    ZipFailed(String),
    /// 保存領域の空きが少ない（W062、§13.3.6）。
    LowSpace,
}

/// 「名前を付けて保存」に失敗した理由（文言は `NovnowApp` がカタログで引く）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportFailure {
    /// 関門が拒んだ（E060: 名前が `.txt` / `.md` で終わらない・履歴ファイル。§13.1.1）。
    Refused(Diagnostic),
    /// 書けなかった（OS のメッセージ）。
    Failed(String),
}
