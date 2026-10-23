//! 保存ステートマシン（§8.2・§8.4、指示 3「保存を意識させない」）。
//!
//! **egui に依存しない純粋な状態機械**（時刻は呼び出し側が秒で渡す。`egui::InputState::time` など）。
//! 書き込みそのものはしない: [`SaveMachine::due`] が「いま書く」を返したら、ホストが書いて
//! [`SaveMachine::succeeded`] / [`SaveMachine::failed`] で結果を戻す（`docs/history.md` B-114）。
//!
//! ```text
//! Clean ─編集→ Dirty ─400ms idle / 即時→ Saving ─成功→ Clean（保存中に編集があれば Dirty）
//!                 ↑ 追加編集でタイマ再起動         └失敗→ SaveFailed ─1s, 2s, 4s, 8s, 以後 8s→ Saving
//! ```

/// 失敗したときの再試行までの秒数（§8.2。4 回目以降は 8 秒固定）。
pub const BACKOFF_SECS: [f64; 4] = [1.0, 2.0, 4.0, 8.0];

/// 保存中のスピナーを出すまでの秒数（§8.4「300ms 未満で終わる場合は描画しない」）。
pub const SPINNER_DELAY_SECS: f64 = 0.3;

/// 既定の idle（§8.2「最後の打鍵から 400ms」）。
pub const DEFAULT_IDLE_MS: u32 = 400;

/// 状態（§8.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// 保存済み。
    Clean,
    /// 未保存の変更がある。
    Dirty,
    /// 書き込み中。
    Saving,
    /// 書き込みに失敗した（編集はメモリに保持したまま再試行する、§8.3）。
    Failed,
}

/// ステータスバー右端のインジケータ（§8.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indicator {
    /// 何も出さない（Clean と、300ms 未満の Saving）。
    Nothing,
    /// `●`（`state.unsaved`）。
    Unsaved,
    /// 回転スピナー（`fg.muted`）。
    Spinner,
    /// `!` を内包する三角（`state.error`。点滅しない）。
    Failed,
}

/// 保存ステートマシン。
#[derive(Debug, Clone)]
pub struct SaveMachine {
    state: State,
    idle_secs: f64,
    last_edit: f64,
    flush: bool,
    saving_since: f64,
    edited_while_saving: bool,
    failures: u32,
    retry_at: f64,
    error: Option<String>,
}

impl Default for SaveMachine {
    fn default() -> Self {
        Self::new(DEFAULT_IDLE_MS)
    }
}

impl SaveMachine {
    /// `idle_ms` は `ui.json` の値（§8.2「200–2000ms」に丸める）。
    pub fn new(idle_ms: u32) -> Self {
        Self {
            state: State::Clean,
            idle_secs: f64::from(idle_ms.clamp(200, 2000)) / 1000.0,
            last_edit: 0.0,
            flush: false,
            saving_since: 0.0,
            edited_while_saving: false,
            failures: 0,
            retry_at: 0.0,
            error: None,
        }
    }

    /// 今の状態。
    pub fn state(&self) -> State {
        self.state
    }

    /// 最後の失敗の内容（`SaveFailed` の詳細モーダル用）。
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// 編集があった（打鍵ごと）。Dirty のタイマを再起動する。
    pub fn edited(&mut self, now: f64) {
        self.last_edit = now;
        match self.state {
            State::Clean | State::Dirty => self.state = State::Dirty,
            State::Saving => self.edited_while_saving = true,
            // 失敗中は再試行の予定を変えない（最新の内容が次の再試行で書かれる）
            State::Failed => {}
        }
    }

    /// 即時の保存を求める（§8.2「節移動・ファイル切替・ワークスペース切替・ウィンドウ非フォーカス・
    /// アプリ終了要求」と `Ctrl+S`）。未保存の変更が無ければ何もしない。
    pub fn request_now(&mut self) {
        if matches!(self.state, State::Dirty | State::Failed) {
            self.flush = true;
        }
    }

    /// いま書き込みを始めるべきか。
    pub fn due(&self, now: f64) -> bool {
        match self.state {
            // 1µs の余裕: 時刻は f64 の秒なので 1.7 - 1.3 が 0.4 を僅かに下回ることがある
            State::Dirty => self.flush || now - self.last_edit + 1e-6 >= self.idle_secs,
            State::Failed => self.flush || now >= self.retry_at,
            State::Clean | State::Saving => false,
        }
    }

    /// 書き込みを始めた。
    pub fn begin(&mut self, now: f64) {
        self.state = State::Saving;
        self.saving_since = now;
        self.flush = false;
        self.edited_while_saving = false;
    }

    /// 書き込みに成功した。保存中に編集があれば Dirty に戻る（その編集はまだ書かれていない）。
    pub fn succeeded(&mut self) {
        self.failures = 0;
        self.error = None;
        self.state = if self.edited_while_saving {
            State::Dirty
        } else {
            State::Clean
        };
        self.edited_while_saving = false;
    }

    /// 書き込みに失敗した。指数バックオフで再試行を予約する。
    pub fn failed(&mut self, now: f64, message: impl Into<String>) {
        self.failures += 1;
        let step = (self.failures as usize - 1).min(BACKOFF_SECS.len() - 1);
        self.retry_at = now + BACKOFF_SECS[step];
        self.error = Some(message.into());
        self.state = State::Failed;
        self.edited_while_saving = false;
    }

    /// 次の再試行の時刻（失敗中のみ）。
    pub fn retry_at(&self) -> Option<f64> {
        (self.state == State::Failed).then_some(self.retry_at)
    }

    /// ステータスバーのインジケータ（§8.4）。
    pub fn indicator(&self, now: f64) -> Indicator {
        match self.state {
            State::Clean => Indicator::Nothing,
            State::Dirty => Indicator::Unsaved,
            State::Saving if now - self.saving_since >= SPINNER_DELAY_SECS => Indicator::Spinner,
            State::Saving => Indicator::Nothing,
            State::Failed => Indicator::Failed,
        }
    }

    /// 「閉じないで」（錠前の開いた形）を出すか・終了を止めるか（§8.4）。
    pub fn guards_close(&self) -> bool {
        self.state != State::Clean
    }

    /// 次に状態を確かめるべき時刻（再描画の予約に使う）。
    pub fn next_wakeup(&self, now: f64) -> Option<f64> {
        match self.state {
            State::Dirty => Some((self.last_edit + self.idle_secs).max(now)),
            State::Failed => Some(self.retry_at.max(now)),
            State::Saving => Some(self.saving_since + SPINNER_DELAY_SECS),
            State::Clean => None,
        }
    }
}
