//! 実行環境（[`Env`] の実装）。プロセスの環境変数と、コンパイル先の OS。
//!
//! Web には環境変数もホームディレクトリも無いので、どちらも `None` を返す（履歴は IndexedDB の
//! `app` ストアが担う、§13.3.4）。

use std::path::PathBuf;

use crate::ports::env::{Env, Os};

/// このプロセスの環境。
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemEnv;

impl Env for SystemEnv {
    fn var(&self, name: &str) -> Option<String> {
        if cfg!(target_arch = "wasm32") {
            return None;
        }
        std::env::var(name).ok().filter(|value| !value.is_empty())
    }

    fn os(&self) -> Os {
        if cfg!(target_arch = "wasm32") {
            Os::Web
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::OtherUnix
        }
    }

    fn home_dir(&self) -> Option<PathBuf> {
        home_dir()
    }
}

/// ホームディレクトリ（`HOME` → OS の既定。`directories` クレート、`PLANS.md` §2.1）。
#[cfg(not(target_arch = "wasm32"))]
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()))
}

#[cfg(target_arch = "wasm32")]
fn home_dir() -> Option<PathBuf> {
    None
}
