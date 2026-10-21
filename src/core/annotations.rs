//! Shared literal-safe annotation recognition. All ranges are UTF-8 byte offsets.
use crate::core::text::lexer::{self, LineKind, TextFormat};
use std::ops::Range;

pub fn illustration(line: &str) -> Option<&str> {
    match lexer::classify(line, TextFormat::Contents) {
        LineKind::IllustrationMemo(memo) => Some(memo),
        _ => None,
    }
}

/// Recognize a suffix of citations, before any speaker annotation / comment.
pub fn citations(body: &str) -> Vec<(Range<usize>, &str)> {
    let mut end = body[..lexer::find_line_comment(body).unwrap_or(body.len())]
        .trim_end()
        .len();
    let mut out = Vec::new();
    loop {
        let bytes = body.as_bytes();
        let digits = bytes[..end]
            .iter()
            .rev()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digits < 4 || end < digits + 2 {
            break;
        }
        let at = end - digits - 2;
        if bytes[at] != b'^' || !matches!(bytes[at + 1], b'M' | b'S') {
            break;
        }
        let slashes = body.as_bytes()[..at]
            .iter()
            .rev()
            .take_while(|c| **c == b'\\')
            .count();
        if slashes % 2 == 1 {
            break;
        }
        // A citation inside an inline comment is not an annotation.
        if lexer::find_inline_comment(body).is_some_and(|r| r.contains(&at)) {
            break;
        }
        let start = body[..at].trim_end().len();
        out.push((start..end, &body[at + 1..end]));
        end = start;
    }
    out.reverse();
    out
}

pub fn without_citations(body: &str) -> &str {
    citations(body)
        .first()
        .map_or(body, |(r, _)| &body[..r.start])
}

/// Bounded header inspection only; decoding remains an adapter/UI responsibility.
pub fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 {
        return Some((
            u32::from_be_bytes(bytes[16..20].try_into().ok()?),
            u32::from_be_bytes(bytes[20..24].try_into().ok()?),
        ));
    }
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut at = 2;
    while at + 4 <= bytes.len() {
        if bytes[at] != 0xff {
            return None;
        }
        let marker = bytes[at + 1];
        at += 2;
        if matches!(marker, 0xd8 | 0xd9 | 0x01) {
            continue;
        }
        let length = u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?) as usize;
        if length < 2 || at + length > bytes.len() {
            return None;
        }
        if matches!(marker,0xc0..=0xc3|0xc5..=0xc7|0xc9..=0xcb|0xcd..=0xcf) && length >= 7 {
            return Some((
                u16::from_be_bytes(bytes[at + 5..at + 7].try_into().ok()?) as u32,
                u16::from_be_bytes(bytes[at + 3..at + 5].try_into().ok()?) as u32,
            ));
        }
        at += length;
    }
    None
}
