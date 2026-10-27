//! Byte-to-text decoding for migration (§14.6) and paste mojibake repair.
//!
//! Decision order is fixed (Issue #29 X1): strict UTF-8 first (a leading BOM
//! is dropped), otherwise Shift_JIS via `encoding_rs`, otherwise reject.
//! Encodings other than UTF-8 / Shift_JIS are out of scope. Newline
//! normalization stays with the caller (`lexer::normalize_for_save`).

/// The encoding a byte sequence was (or should be) read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEncoding {
    Utf8,
    ShiftJis,
}

/// A successfully decoded input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedText {
    pub text: String,
    pub encoding: SourceEncoding,
    /// True when the bytes read cleanly as both UTF-8 and Shift_JIS but give
    /// different text, so a human may want to switch in a preview.
    pub ambiguous: bool,
}

/// Why an input cannot become document text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Nothing but whitespace (or nothing at all).
    Empty,
    /// Contains NUL; never plain text.
    Nul,
    /// Neither strict UTF-8 nor error-free Shift_JIS.
    Undecodable,
}

/// Decode with the §14.6.1 precedence: strict UTF-8, then Shift_JIS.
pub fn decode(bytes: &[u8]) -> Result<DecodedText, DecodeError> {
    match decode_as(bytes, SourceEncoding::Utf8) {
        Ok(decoded) => Ok(decoded),
        Err(DecodeError::Undecodable) => decode_as(bytes, SourceEncoding::ShiftJis),
        Err(error) => Err(error),
    }
}

/// Decode as one fixed encoding (preview switching). Rejections are the
/// same as [`decode`]: NUL bytes and whitespace-only results never pass.
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

/// Strict Shift_JIS decode: any replacement character produced by the
/// decoder means the bytes were not readable as Shift_JIS.
fn decode_sjis(bytes: &[u8]) -> Option<String> {
    let (text, _, had_errors) = encoding_rs::SHIFT_JIS.decode(bytes);
    if had_errors {
        None
    } else {
        Some(text.into_owned())
    }
}

/// Characters that UTF-8-as-Shift_JIS mojibake produces constantly but
/// ordinary Japanese prose practically never uses. Derived empirically by
/// mis-decoding the bundled sample novel and the user guide (Issue #29 X1).
const MOJIBAKE_MARKERS: &[char] = &[
    '縺', '繧', '繝', '縲', '吶', '峨', '譖', '翫', '菴', '薙', '譁', '帙', '隱', '溘', '榊', '蜷',
    '荳',
];

/// If `text` looks like UTF-8 that was mis-decoded as Shift_JIS, return a
/// repaired candidate (§14.6.1). The original bytes are rebuilt by encoding
/// the pasted string back to Shift_JIS; each replacement character stands
/// for one byte the mis-decoding dropped, so it is rebuilt as a placeholder
/// byte and surfaces as a replacement character again in the candidate.
/// The candidate is therefore exact where the mis-decoding was lossless
/// and has visible holes where it was not — a human confirms it in a
/// preview before anything is pasted. Two gates keep ordinary text from
/// ever prompting: the original must carry a mojibake fingerprint (a marker
/// character, or several halfwidth katakana from misread trail bytes), and
/// the repaired text must read as Japanese with only few holes.
pub fn mojibake_repair(text: &str) -> Option<String> {
    if text.len() < 4 || text.is_ascii() {
        return None;
    }
    let has_marker = text.chars().any(|ch| MOJIBAKE_MARKERS.contains(&ch));
    let halfwidth = text
        .chars()
        .filter(|ch| ('\u{FF66}'..='\u{FF9D}').contains(ch))
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
    // The mis-decoding dropped bytes that the pasted string no longer
    // carries; they show up as holes. A candidate that is mostly holes
    // recovers nothing, so do not propose it — a human confirms the
    // rest in the preview.
    if holes * 2 > chars {
        return None;
    }
    let reads_as_japanese = repaired.chars().any(|ch| {
        ('\u{3041}'..='\u{3096}').contains(&ch) || ('\u{30A1}'..='\u{30FA}').contains(&ch)
    });
    if !reads_as_japanese {
        return None;
    }
    Some(repaired.into_owned())
}

/// Encode back to Shift_JIS bytes, keeping every replacement character as
/// one placeholder byte (0xFF, which UTF-8 lossy decoding turns back into
/// a replacement character). Returns None when any other character cannot
/// be encoded to Shift_JIS — such text was not produced by a Shift_JIS
/// mis-decode.
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
