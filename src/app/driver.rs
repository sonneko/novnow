//! セッションをワーカースレッド 1 本の上で所有する口（§19.3.4）。ネイティブ専用。
//!
//! [`Session`] は `Send` ではない。ホスト（[`Host`]）は Web が `Rc` で 1 スレッドを前提に
//! するので `Send` を要求できず、イベントのコールバックも `Send` を持たない。だから
//! 「呼び出し側のスレッドで GIL を解放したまま待ち、結果だけ受け取る」Python API（§19.3）
//! は、`Session` をスレッド間に**動かす**のではなく、**ワーカースレッドの中で作って
//! そこで使い、そこで捨てる**（§19.2.5 の排他的所有はそのまま）。この口がその 1 本化を担う。
//!
//! - ワーカーは `Session` を 1 つだけ持ち、届いた 1 通を 1 通ずつ順に処理する。だから
//!   複数スレッドからの同時呼び出しはデータ競合でなく**直列実行**になる（§19.3.4 MUST）。
//! - 待つ側が渡す 1 通も受け取る結果も Python に触れない（`Send` だけのデータ）。だから
//!   待機区間を `Python::allow_threads` で包める（GIL 解放。§19.3.4 MUST）。
//! - イベント（§19.2.4）は別の 1 本へ送る。`Session::poll_events` と同じ列が、待った側の
//!   スレッドでそのまま取り出せる（§19.3.5）。
//!
//! 1 通 = 1 往復。[`Driver::run`] は応答が返るまでブロックするので、呼び出し側は必ず
//! GIL を解放した区間で呼ぶ（`REF:python/novnow-py/src/lib.rs`）。
//!
//! ```no_run
//! use novnow::app::driver::Driver;
//! use novnow::app::session::CreateOptions;
//!
//! # fn main() -> novnow::app::error::Result<()> {
//! let mut driver = Driver::create(
//!     "my-novel",
//!     CreateOptions {
//!         title: "星屑の時計".to_owned(),
//!         ..CreateOptions::default()
//!     },
//! )?;
//! let info = driver.run("project.info", serde_json::json!({}))?;
//! let _ = info["title"];
//! # Ok(())
//! # }
//! ```

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use serde_json::Value;

use crate::app::error::{Class, Error, Result};
use crate::app::event::AppEvent;
use crate::app::host::Host;
use crate::app::session::{CreateOptions, Interface, Session};
use crate::ui::i18n::{self, Locale};

/// ワーカーへ渡す 1 通（すべて `Send`。GIL を解放した区間をまたげる）。
enum Job {
    /// `Session` を作る（`create` があれば新規作成、無ければ既存を開く）。
    Open {
        /// プロジェクトのルート。
        root: PathBuf,
        /// 呼んでいるインターフェース（§19.1.5 の関門と §19.7 の例外列）。
        interface: Interface,
        /// 新規作成のオプション（あれば `Session::create`）。
        create: Option<CreateOptions>,
        /// 結果。
        reply: Sender<Result<()>>,
    },
    /// 与えられたホストで `Session` を作る（仮想プロジェクト）。
    Adopt {
        /// ホスト。ワーカースレッドへ動かすので `Send` を要求する。
        host: Box<dyn Host + Send>,
        /// 呼んでいるインターフェース。
        interface: Interface,
        /// 結果。
        reply: Sender<Result<()>>,
    },
    /// 正規操作 1 件（§19.1.4 の単一実装点）。
    Run {
        /// 正規操作 ID。
        op: String,
        /// 引数（JSON と同じ構造）。
        args: Value,
        /// 結果。
        reply: Sender<Result<Value>>,
    },
    /// パレットのコマンド 1 件（§19.2.3）。
    Command {
        /// ラベル（`command_of_name` が引く名前）。
        name: String,
        /// 結果。
        reply: Sender<Result<Value>>,
    },
    /// 長時間の操作の中断（§19.1.9）。
    Cancel {
        /// 対象（`T012`。空なら全部）。
        handle: String,
        /// 結果。
        reply: Sender<Result<Value>>,
    },
    /// 文言の言語（§19.1.8）。応答を待たない（1 本は順番どおりに流れる）。
    Locale(Locale),
    /// 保存 1 回分（§8.3）。
    Flush {
        /// 結果。
        reply: Sender<Result<Value>>,
    },
    /// プロジェクトのルート。
    Root {
        /// 結果（仮想プロジェクトなら `None`）。
        reply: Sender<Option<PathBuf>>,
    },
    /// 溜まったイベントを呼び出し側へ送る（応答は転送の**後**）。
    Drain {
        /// 送り終わった合図。
        reply: Sender<()>,
    },
    /// ワーカーを止める（未保存の変更は捨てる。§19.2.5）。
    Close,
}

/// ワーカースレッドの上で 1 つのプロジェクトに対する全操作（§19.3.4）。
///
/// `Send` だが `Sync` ではない（応答を待つ 1 本を共有しないため）。共有する側は
/// `Mutex<Driver>` にして、**GIL を解放した区間で**ロックする（§19.3.4 の直列化）。
pub struct Driver {
    /// ワーカーへの 1 本。
    jobs: Sender<Job>,
    /// イベントの 1 本（`Session::poll_events` と同じ列）。
    events: Receiver<AppEvent>,
    /// ワーカー（`Close` で `join` する）。
    worker: Option<JoinHandle<()>>,
    /// 今の言語（`Session` が無い間の失敗の文言に使う）。
    locale: Locale,
}

impl Driver {
    /// 既存のプロジェクトを開く（Rust 経路。§19.2.1）。
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        Self::open_with(root.as_ref(), Interface::Rust)
    }

    /// 既存のプロジェクトを、呼んでいるインターフェースを名乗って開く。
    pub fn open_with(root: &Path, interface: Interface) -> Result<Self> {
        let root = root.to_path_buf();
        Self::start(move |reply| Job::Open {
            root,
            interface,
            create: None,
            reply,
        })
    }

    /// 新しいプロジェクトを作って開く（`novnow new` と同じ。§19.2.1）。
    pub fn create(root: impl AsRef<Path>, options: CreateOptions) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        Self::start(move |reply| Job::Open {
            root,
            interface: Interface::Rust,
            create: Some(options),
            reply,
        })
    }

    /// 与えられたホストで仮想プロジェクトを開く（テスト用。ファイルシステムに触れない）。
    ///
    /// ホストはワーカースレッドへ動かすので `Send` を要求する（`MemoryHost` のように
    /// `Rc` を共有するホストは渡せない。実ディレクトリを使う）。
    pub fn adopt<H: Host + Send + 'static>(host: H, interface: Interface) -> Result<Self> {
        Self::start(move |reply| Job::Adopt {
            host: Box::new(host),
            interface,
            reply,
        })
    }

    /// ワーカーを起こし、最初の 1 通（`Session` を作る）の応答を待ってから返す。
    fn start(build: impl FnOnce(Sender<Result<()>>) -> Job) -> Result<Self> {
        let (jobs, queue) = mpsc::channel::<Job>();
        let (events, drained) = mpsc::channel::<AppEvent>();
        let spawned = std::thread::Builder::new()
            .name("novnow-session".to_owned())
            .spawn(move || serve(queue, events));
        let worker = match spawned {
            Ok(worker) => worker,
            Err(_) => return Err(start_failed(Locale::Ja)),
        };
        let mut driver = Self {
            jobs,
            events: drained,
            worker: Some(worker),
            locale: Locale::Ja,
        };
        // 開けなかったら、`Drop` が `Close` を送ってワーカーを畳む
        driver.call(build)??;
        Ok(driver)
    }

    /// 正規操作を 1 つ実行し、応答が返るまで待つ（§19.1.4）。**ブロックする**。
    pub fn run(&mut self, op: &str, args: Value) -> Result<Value> {
        let op = op.to_owned();
        self.call(move |reply| Job::Run { op, args, reply })?
    }

    /// パレットのコマンドを実行し、応答が返るまで待つ（§19.2.3）。**ブロックする**。
    pub fn command(&mut self, name: &str) -> Result<Value> {
        let name = name.to_owned();
        self.call(move |reply| Job::Command { name, reply })?
    }

    /// 長時間の操作を中断する（§19.1.9）。**ブロックする**。
    pub fn cancel(&mut self, handle: &str) -> Result<Value> {
        let handle = handle.to_owned();
        self.call(move |reply| Job::Cancel { handle, reply })?
    }

    /// 文言の言語を変える（§19.1.8）。順番は保証されるので応答を待たない。
    pub fn set_locale(&mut self, locale: Locale) -> Result<()> {
        self.locale = locale;
        self.jobs
            .send(Job::Locale(locale))
            .map_err(|_| self.stopped())
    }

    /// 保存 1 回分（§8.3）。**ブロックする**。
    pub fn flush(&mut self) -> Result<Value> {
        self.call(|reply| Job::Flush { reply })?
    }

    /// プロジェクトのルート（仮想プロジェクトなら `None`）。**ブロックする**。
    pub fn root(&mut self) -> Result<Option<PathBuf>> {
        self.call(|reply| Job::Root { reply })
    }

    /// 溜まったイベントを取り出す（§19.2.4・§19.3.5）。**ブロックする**。
    ///
    /// 応答はイベントの転送の後に送るので、返ったときには送り終えている。
    pub fn poll_events(&mut self) -> Result<Vec<AppEvent>> {
        self.call(|reply| Job::Drain { reply })?;
        Ok(self.events.try_iter().collect())
    }

    /// ワーカーを止める（§19.3.2 の `close`）。**保存はしない**: 未保存の変更は捨てる
    /// （§19.2.5 と同じ）。書き出したいときは先に [`Driver::flush`]（＝ `save.now`）。
    pub fn shutdown(&mut self) {
        let _ = self.jobs.send(Job::Close);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }

    /// 1 通を送り、応答を待つ。
    fn call<T: Send + 'static>(&mut self, build: impl FnOnce(Sender<T>) -> Job) -> Result<T> {
        let (reply, wait) = mpsc::channel::<T>();
        self.jobs.send(build(reply)).map_err(|_| self.stopped())?;
        wait.recv().map_err(|_| self.stopped())
    }

    /// ワーカーが居なくなった（パニック・`Close` の後）。
    fn stopped(&self) -> Error {
        Error::new(
            Class::Io,
            i18n::format(self.locale, "api.error.thread_stopped", &[]),
        )
    }
}

impl Drop for Driver {
    /// 捨てるときにワーカーを畳む（未保存の変更は捨てる。§19.2.5）。
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// ワーカースレッドを始められなかった（`io`。§19.1.7）。
fn start_failed(locale: Locale) -> Error {
    Error::new(
        Class::Io,
        i18n::format(locale, "api.error.thread_start", &[]),
    )
}

/// ワーカースレッド（1 本）。`Session` はここで作り、ここで捨てる。
fn serve(queue: Receiver<Job>, events: Sender<AppEvent>) {
    let mut session: Option<Session> = None;
    for job in queue {
        match job {
            // 応答を転送より先に送ると呼び出し側が取り逃すので、`Drain` だけ先に転送する
            Job::Drain { reply } => {
                forward(&mut session, &events);
                let _ = reply.send(());
            }
            Job::Close => break,
            job => {
                handle(job, &mut session);
                forward(&mut session, &events);
            }
        }
    }
}

/// 1 通を処理する（ワーカースレッドの上）。
fn handle(job: Job, session: &mut Option<Session>) {
    match job {
        Job::Open {
            root,
            interface,
            create,
            reply,
        } => {
            let opened = match create {
                Some(options) => Session::create(&root, options),
                None => Session::open_with(&root, interface),
            };
            match opened {
                Ok(opened) => {
                    *session = Some(opened);
                    let _ = reply.send(Ok(()));
                }
                Err(error) => {
                    let _ = reply.send(Err(error));
                }
            }
        }
        Job::Adopt {
            host,
            interface,
            reply,
        } => {
            let mut opened = Session::with_host(host);
            opened.set_interface(interface);
            *session = Some(opened);
            let _ = reply.send(Ok(()));
        }
        Job::Run { op, args, reply } => {
            let outcome = match session.as_mut() {
                Some(session) => session.run(&op, args),
                None => Err(start_failed(Locale::Ja)),
            };
            let _ = reply.send(outcome);
        }
        Job::Command { name, reply } => {
            let outcome = match session.as_mut() {
                Some(session) => session.command(&name),
                None => Err(start_failed(Locale::Ja)),
            };
            let _ = reply.send(outcome);
        }
        Job::Cancel { handle, reply } => {
            let outcome = match session.as_mut() {
                Some(session) => session.cancel(&handle),
                None => Err(start_failed(Locale::Ja)),
            };
            let _ = reply.send(outcome);
        }
        Job::Locale(locale) => {
            if let Some(session) = session.as_mut() {
                session.set_locale(locale);
            }
        }
        Job::Flush { reply } => {
            let outcome = match session.as_mut() {
                Some(session) => session.flush(),
                None => Err(start_failed(Locale::Ja)),
            };
            let _ = reply.send(outcome);
        }
        Job::Root { reply } => {
            let root = session
                .as_ref()
                .and_then(|session| session.root())
                .map(Path::to_path_buf);
            let _ = reply.send(root);
        }
        Job::Drain { .. } | Job::Close => {}
    }
}

/// 溜まったイベントを呼び出し側のスレッドへ送る（§19.2.4 の列をそのまま）。
fn forward(session: &mut Option<Session>, events: &Sender<AppEvent>) {
    let Some(session) = session.as_mut() else {
        return;
    };
    for event in session.poll_events() {
        let _ = events.send(event);
    }
}
