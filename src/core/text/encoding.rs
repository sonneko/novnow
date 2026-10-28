//! 移行の取り込み（§14.6）と貼り付けの文字化け修復のための、byte 列から文字列への復号。
//!
//! 判定の順は固定である（Issue #29 X1）。まず厳密な UTF-8（先頭の BOM は落とす）、
//! 読めなければ `encoding_rs` で Shift_JIS、どちらでもなければ拒否する。
//! UTF-8 と Shift_JIS 以外の符号化（EUC-JP など）は対象にしない。改行の正規化は
//! 呼び出し側（`lexer::normalize_for_save`）の仕事である。

use crate::core::text::charclass;

/// byte 列を（またはこれから）どの符号化として読むか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEncoding {
    Utf8,
    ShiftJis,
}

/// 復号に成功した入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedText {
    pub text: String,
    pub encoding: SourceEncoding,
    /// UTF-8 としても Shift_JIS としても正しく読めるが、結果の文字列が違うとき `true`。
    /// プレビューで人が文字コードを切り替えられるようにするための印（§14.6.1）。
    pub ambiguous: bool,
}

/// 入力を本文の文字列にできない理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// 空白だけ（または何も無い）。
    Empty,
    /// NUL を含む。素のテキストではありえない。
    Nul,
    /// 厳密な UTF-8 でも、誤りの無い Shift_JIS でもない。
    Undecodable,
}

/// §14.6.1 の順で復号する。厳密な UTF-8、次に Shift_JIS。
pub fn decode(bytes: &[u8]) -> Result<DecodedText, DecodeError> {
    match decode_as(bytes, SourceEncoding::Utf8) {
        Ok(decoded) => Ok(decoded),
        Err(DecodeError::Undecodable) => decode_as(bytes, SourceEncoding::ShiftJis),
        Err(error) => Err(error),
    }
}

/// 指定した 1 つの符号化で復号する（プレビューでの切り替え用）。拒否の条件は
/// [`decode`] と同じで、NUL を含む入力と空白だけの結果は通さない。
pub fn decode_as(bytes: &[u8], encoding: SourceEncoding) -> Result<DecodedText, DecodeError> {
    if bytes.contains(&0) {
        return Err(DecodeError::Nul);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let utf8 = std::str::from_utf8(bytes).ok();
    let sjis = decode_sjis(bytes);
    let ambiguous = match (&utf8, &sjis) {
        (Some(utf8), Some(sjis)) => utf8 != sjis,
        _ => false,
    };
    let text = match encoding {
        SourceEncoding::Utf8 => utf8.map(str::to_owned),
        SourceEncoding::ShiftJis => sjis,
    };
    let text = match text {
        Some(text) => text,
        None => return Err(DecodeError::Undecodable),
    };
    if text.trim().is_empty() {
        return Err(DecodeError::Empty);
    }
    Ok(DecodedText {
        text,
        encoding,
        ambiguous,
    })
}

/// 厳密な Shift_JIS の復号。置換文字が 1 つでも出たら、その byte 列は
/// Shift_JIS として読めなかったものとして扱う。
fn decode_sjis(bytes: &[u8]) -> Option<String> {
    let (text, _, had_errors) = encoding_rs::SHIFT_JIS.decode(bytes);
    if had_errors {
        None
    } else {
        Some(text.into_owned())
    }
}

/// UTF-8 を Shift_JIS と読み違えた文字化けで頻出し、ふつうの日本語の文章では
/// まず使わない文字（§14.6.1 の 17 字）。同梱のサンプル小説と手引きを実際に
/// 読み違えて、経験的に選んだ（Issue #29 X1）。
const MOJIBAKE_MARKERS: &[char] = &[
    '縺', '繧', '繝', '縲', '吶', '峨', '譖', '翫', '菴', '薙', '譁', '帙', '隱', '溘', '榊', '蜷',
    '荳',
];

/// `text` が「UTF-8 を Shift_JIS と読み違えた」文字化けに見えるとき、直した候補を返す
/// （§14.6.1）。元の byte 列は、貼り付けられた文字列を Shift_JIS へ符号化し直して組み立てる。
/// 置換文字 1 つは、読み違えで落ちた byte 1 つ分なので、代わりの 1 byte として組み直し、
/// 候補の中ではまた置換文字として現れる。したがって候補は、読み違えが情報を失っていない
/// ところでは正確で、失ったところは目に見える穴になる。貼り付ける前に人がプレビューで
/// 確かめる。ふつうの文章で候補を出さないための門は 2 つある。元の文字列が文字化けの指紋
/// （印の文字、または読み違えた後ろ側の byte 由来の半角カナが複数）を持つこと、そして
/// 直した結果がかなを含み、穴が少ないことである。
pub fn mojibake_repair(text: &str) -> Option<String> {
    if text.len() < 4 || text.is_ascii() {
        return None;
    }
    let has_marker = text.chars().any(|ch| MOJIBAKE_MARKERS.contains(&ch));
    let halfwidth = text
        .chars()
        .filter(|ch| charclass::is_halfwidth_katakana(*ch))
        .count();
    if !has_marker && halfwidth < 3 {
        return None;
    }
    let bytes = rebuild_sjis_bytes(text)?;
    let repaired = String::from_utf8_lossy(&bytes);
    if repaired == text {
        return None;
    }
    let chars = repaired.chars().count();
    let holes = repaired.chars().filter(|ch| *ch == '\u{FFFD}').count();
    // 読み違えで落ちた byte は、貼り付けられた文字列にはもう無いので穴として現れる。
    // 穴が大半を占める候補は何も取り戻せていないので出さない。残りは人がプレビューで
    // 確かめる（§14.6.1「全体の文字数の半分以下」）。
    if holes * 2 > chars {
        return None;
    }
    let reads_as_japanese = repaired.chars().any(charclass::is_kana);
    if !reads_as_japanese {
        return None;
    }
    Some(repaired.into_owned())
}

/// Shift_JIS の byte 列へ符号化し直す。置換文字は代わりの 1 byte（`0xFF`。UTF-8 の
/// 損失ありの復号で置換文字に戻る）として残す。ほかの文字が 1 つでも Shift_JIS へ
/// 符号化できなければ `None` を返す。そのような文字列は Shift_JIS の読み違いでは
/// 作れないからである。
fn rebuild_sjis_bytes(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut segments = text.split('\u{FFFD}').peekable();
    while let Some(segment) = segments.next() {
        if !segment.is_empty() {
            let (encoded, _, had_errors) = encoding_rs::SHIFT_JIS.encode(segment);
            if had_errors {
                return None;
            }
            bytes.extend_from_slice(&encoded);
        }
        if segments.peek().is_some() {
            bytes.push(0xFF);
        }
    }
    Some(bytes)
}
