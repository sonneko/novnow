//! LSP の枠 — `Content-Length: N\r\n\r\n<JSON>`（§19.4.1）。
//!
//! MCP は行区切りの JSON だが、LSP は先頭に長さを置く。どちらも新クレートを入れずに
//! 標準ライブラリだけで読み書きできる（§19.1.2）。

use std::io::{BufRead, Write};

/// 1 メッセージの上限。これを超える枠は壊れているか攻撃なので読まない。
pub const MAX_MESSAGE: usize = 16 * 1024 * 1024;

/// 1 メッセージを枠に入れて返す。
///
/// 本文の長さは **byte** 数（UTF-8）。日本語の本文でも `str::len` で正しい。
pub fn encode(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{body}", body.len())
}

/// 本文を枠に入れて書き出す。
pub fn write_message<W: Write>(out: &mut W, body: &str) -> std::io::Result<()> {
    out.write_all(encode(body).as_bytes())?;
    out.flush()
}

/// 1 メッセージの枠を読み、JSON 本文を返す。
///
/// `None` は **正常な切断**（エディタが口を閉じた）。壊れた枠（長さが無い、長さが 0、
/// 上限超え、UTF-8 でない）も `None` — LSP は切断と同じ扱いでよい。
pub fn read_message<R: BufRead + ?Sized>(input: &mut R) -> Option<String> {
    let mut length = 0usize;
    let mut saw_length = false;
    loop {
        let mut line = String::new();
        let read = input.read_line(&mut line).ok()?;
        if read == 0 {
            // 枠の途中で口が閉じた。
            return None;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("content-length") {
            length = value.trim().parse().ok()?;
            saw_length = true;
        }
    }
    if !saw_length || length == 0 || length > MAX_MESSAGE {
        return None;
    }
    let mut body = vec![0u8; length];
    input.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}
