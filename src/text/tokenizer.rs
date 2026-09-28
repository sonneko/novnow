//! カスタム文法の高速トークナイザ。
//!
//! 複数行にまたがるトークンは存在しない前提なので、行単位で完全に分割できる。

use crate::types::plots::SceneId;
use regex::Regex;
use std::str::FromStr;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// contents/*.txt
    Contents,
    /// plots/*.txt
    Plots,
    /// それ以外（キャラ辞書など、素のテキストとして扱う）。
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    SceneHeader,
    Comment,
    PlotPoint,
    Speaker,
    Narration,
    ParagraphBreak,
    Blank,
    Text,
    Eof,
}

impl TokenKind {
    pub fn label(self) -> &'static str {
        match self {
            TokenKind::SceneHeader => "見出し",
            TokenKind::Comment => "コメント",
            TokenKind::PlotPoint => "プロット",
            TokenKind::Speaker => "セリフ",
            TokenKind::Narration => "地の文",
            TokenKind::ParagraphBreak => "区切り",
            TokenKind::Blank => "空行",
            TokenKind::Text => "本文",
            TokenKind::Eof => "EOF",
        }
    }

    /// 文法エラー判定に使う（内容を持つトークンか）。
    pub fn is_content(self) -> bool {
        matches!(
            self,
            TokenKind::SceneHeader
                | TokenKind::Comment
                | TokenKind::PlotPoint
                | TokenKind::Speaker
                | TokenKind::Narration
                | TokenKind::Text
        )
    }
}

/// 1 トークン。
///
/// 仕様の `text: ????` は、メモリ確保を避けて上位の Rope からスライスを取り出す方針とし、
/// ここでは所有権を持たず `byte_start..byte_end` のみを保持する（`line_text` ヘルパで参照する）。
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}

#[allow(dead_code)]
impl Token {
    pub fn len(&self) -> usize {
        self.byte_end.saturating_sub(self.byte_start)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Rope の行テキストから自身のスライスを取り出す。
    pub fn slice<'a>(&self, line_text: &'a str) -> &'a str {
        let s = self.byte_start.min(line_text.len());
        let e = self.byte_end.min(line_text.len());
        &line_text[s..e]
    }
}

fn header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // @A-B-C または @A-B-C-D（本文中の行頭アノテーション）
    RE.get_or_init(|| Regex::new(r"^@\d{2}-\d{2}-\d{2}(?:-\d{2})?").unwrap())
}

fn speaker_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // 行末アノテーション形式: 「……セリフ……」@キャラ名
    RE.get_or_init(|| Regex::new(r"@([^\s@/]+)\s*$").unwrap())
}

fn comment_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"//").unwrap())
}

/// 行頭の `@...` を SceneId として解釈する。失敗時は None。
pub fn parse_header(line: &str) -> Option<(SceneId, usize)> {
    let m = header_re().find(line)?;
    let id = SceneId::from_str(m.as_str()).ok()?;
    Some((id, m.end()))
}

/// 行末の `@キャラ名` を取り出す（contents 用）。`byte_start` はアノテーション開始位置。
pub fn parse_speaker(line: &str) -> Option<(String, usize)> {
    let m = speaker_re().captures(line)?;
    let name = m.get(1)?.as_str().trim().to_string();
    if name.is_empty() {
        return None;
    }
    let idx = m.get(0)?.start();
    Some((name, idx))
}

/// 1 行をトークン化する。返り値は行内のトークン列（最大 1 個を基本とする）。
///
/// 行単位の再解析で呼ばれるホットパスなので、正規表現は `OnceLock` でキャッシュし、
/// 不要なアロケーションを避ける。
pub fn tokenize_line(kind: FileKind, line_idx: usize, line: &str) -> Token {
    let trimmed = line.trim_start();
    let lead = line.len() - trimmed.len();

    // 空行
    if line.trim().is_empty() {
        return Token {
            kind: TokenKind::Blank,
            line: line_idx,
            byte_start: 0,
            byte_end: line.len(),
        };
    }

    // 段落区切り `;`
    if trimmed.starts_with(';') && trimmed.trim_end() == ";" {
        return Token {
            kind: TokenKind::ParagraphBreak,
            line: line_idx,
            byte_start: lead,
            byte_end: line.len(),
        };
    }

    // 見出し `@A-B-C[-D]`（全ファイル種別で共通）
    if let Some((_id, end)) = parse_header(trimmed) {
        return Token {
            kind: TokenKind::SceneHeader,
            line: line_idx,
            byte_start: lead,
            byte_end: lead + end,
        };
    }

    match kind {
        FileKind::Plots => {
            // 行コメント `//`
            if let Some(m) = comment_re().find(line) {
                return Token {
                    kind: TokenKind::Comment,
                    line: line_idx,
                    byte_start: m.start(),
                    byte_end: line.len(),
                };
            }
            Token {
                kind: TokenKind::PlotPoint,
                line: line_idx,
                byte_start: lead,
                byte_end: line.len(),
            }
        }
        FileKind::Contents => {
            if let Some((_name, at)) = parse_speaker(line) {
                Token {
                    kind: TokenKind::Speaker,
                    line: line_idx,
                    byte_start: at,
                    byte_end: line.len(),
                }
            } else {
                Token {
                    kind: TokenKind::Narration,
                    line: line_idx,
                    byte_start: lead,
                    byte_end: line.len(),
                }
            }
        }
        FileKind::Plain => Token {
            kind: TokenKind::Text,
            line: line_idx,
            byte_start: 0,
            byte_end: line.len(),
        },
    }
}

/// 行頭にエラーがあれば返す簡易バリデーション（波線表示用）。
#[allow(dead_code)]
pub fn line_error(kind: FileKind, line: &str) -> Option<String> {
    let t = line.trim_start();
    if t.starts_with('@') && parse_header(t).is_none() {
        return Some("見出しは @A-B-C または @A-B-C-D の形式である必要があります".to_string());
    }
    // contents では本文中の `@` は行末の話者アノテーション以外は警告。
    if kind == FileKind::Contents && t.contains('@') && parse_speaker(line).is_none() {
        return Some("`@` は行末の話者アノテーション (@キャラ名) にのみ使用できます".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_parse() {
        let (id, _) = parse_header("@01-04-02-03").unwrap();
        assert_eq!(id.full(), "01-04-02-03");
        let (id2, _) = parse_header("@01-04-01 /* x */").unwrap();
        assert_eq!(id2.scene_key(), "01-04-01");
        assert_eq!(id2.section, 0);
    }

    #[test]
    fn speaker_parse() {
        let (name, at) = parse_speaker("「やあ」@リリア").unwrap();
        assert_eq!(name, "リリア");
        assert_eq!(&"「やあ」@リリア"[at..], "@リリア");
    }

    #[test]
    fn break_and_blank() {
        assert_eq!(tokenize_line(FileKind::Contents, 0, ";").kind, TokenKind::ParagraphBreak);
        assert_eq!(tokenize_line(FileKind::Contents, 0, "   ").kind, TokenKind::Blank);
    }

    #[test]
    fn plot_comment() {
        assert_eq!(
            tokenize_line(FileKind::Plots, 0, "本文 // 注釈").kind,
            TokenKind::Comment
        );
    }
}
