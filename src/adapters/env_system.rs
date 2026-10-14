//! 実行環境（[`Env`] の実装）。プロセスの環境変数と、コンパイル先の OS。
//!
//! Web には環境変数もホームディレクトリも無いので、どちらも `None` を返す（履歴は IndexedDB の
//! `app` ストアが担う、§13.3.4）。ロケール（§12.4 の 3）は、Unix は環境変数、Windows は環境変数が
//! 無ければ `GetUserDefaultLocaleName`、Web は `navigator.language`。

use std::path::PathBuf;

use crate::ports::env::{posix_locale, Env, Os};

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

    fn locale(&self) -> Option<String> {
        posix_locale(self).or_else(platform_locale)
    }
}

/// 環境変数が無いときの OS のロケール（Windows のユーザー既定。`ja-JP` の形）。
#[cfg(windows)]
fn platform_locale() -> Option<String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultLocaleName(name: *mut u16, len: i32) -> i32;
    }
    // LOCALE_NAME_MAX_LENGTH（終端の NUL を含む）
    let mut buffer = [0u16; 85];
    // SAFETY: 書き込み先は長さ 85 の配列で、その長さを渡す。戻り値は NUL を含む文字数（失敗は 0）。
    let written = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), 85) };
    let len = usize::try_from(written).ok()?.checked_sub(1)?;
    String::from_utf16(buffer.get(..len)?)
        .ok()
        .filter(|name| !name.is_empty())
}

/// ブラウザの言語（`navigator.language`、§12.4 の「ブラウザのロケール」）。
#[cfg(target_arch = "wasm32")]
fn platform_locale() -> Option<String> {
    web_sys::window()?.navigator().language()
}

/// Unix は環境変数だけ（[`posix_locale`]）。
#[cfg(not(any(windows, target_arch = "wasm32")))]
fn platform_locale() -> Option<String> {
    None
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
