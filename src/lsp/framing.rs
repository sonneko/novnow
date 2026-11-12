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

/// バッファ内にある完結済みフレームを 1 件取り出す（ブロックしない）。
///
/// - `Ok(Some(body))` — 1 フレーム取り出して `buf` から消費した。
/// - `Ok(None)` — まだフレームが完結していない（`buf` はそのまま）。
/// - `Err(())` — 壊れた枠（`Content-Length` 不正・上限超え・非 UTF-8）。
pub(crate) fn take_buffered(buf: &mut Vec<u8>) -> Result<Option<String>, ()> {
    let Some((header_end, body_start)) = find_header_end(buf) else {
        return Ok(None);
    };
    let header_text = std::str::from_utf8(&buf[..header_end]).map_err(|_| ())?;
    let mut length = 0usize;
    let mut saw_length = false;
    for line in header_text.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once(':').ok_or(())?;
        if name.trim().eq_ignore_ascii_case("content-length") {
            length = value.trim().parse().map_err(|_| ())?;
            saw_length = true;
        }
    }
    if !saw_length || length == 0 || length > MAX_MESSAGE {
        return Err(());
    }
    let frame_end = body_start.checked_add(length).ok_or(())?;
    if buf.len() < frame_end {
        return Ok(None);
    }
    let body = String::from_utf8(buf[body_start..frame_end].to_vec()).map_err(|_| ())?;
    buf.drain(..frame_end);
    Ok(Some(body))
}

/// 少なくとも 1 フレームが揃うまで読み、同じ読み込みでバッファ内に揃っている後続フレームも
/// ブロックせずにまとめて返す（`$/cancelRequest` の先読み用、§19.1.9）。
pub(crate) fn read_available_messages<R: BufRead + ?Sized>(
    buf: &mut Vec<u8>,
    input: &mut R,
) -> Vec<String> {
    let mut out = Vec::new();
    loop {
        match take_buffered(buf) {
            Ok(Some(body)) => out.push(body),
            Ok(None) => break,
            Err(()) => {
                buf.clear();
                return out;
            }
        }
    }
    if !out.is_empty() {
        return out;
    }
    loop {
        let chunk = match input.fill_buf() {
            Ok([]) | Err(_) => return out,
            Ok(chunk) => chunk,
        };
        let count = chunk.len();
        buf.extend_from_slice(chunk);
        input.consume(count);
        loop {
            match take_buffered(buf) {
                Ok(Some(body)) => out.push(body),
                Ok(None) => break,
                Err(()) => {
                    buf.clear();
                    return out;
                }
            }
        }
        if !out.is_empty() {
            return out;
        }
    }
}

fn find_header_end(buf: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0usize;
    while i + 1 < buf.len() {
        if i + 3 < buf.len() && &buf[i..i + 4] == b"\r\n\r\n" {
            return Some((i, i + 4));
        }
        if &buf[i..i + 2] == b"\n\n" {
            return Some((i, i + 2));
        }
        i += 1;
    }
    None
}
