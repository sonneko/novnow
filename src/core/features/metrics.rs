//! 記述的な観察だけを置く。点数や評価はしない（§9.4.2 の前提）。
use crate::core::{
    annotations,
    features::{sections, Project},
    resolve::role_language,
    syntax::contents,
    text::sentence,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Speech {
    pub lines: usize,
    pub chars: usize,
}
#[derive(Debug, Clone, Default)]
pub struct Metrics {
    pub total: usize,
    pub dialogue: usize,
    pub speakers: BTreeMap<String, Speech>,
    pub lengths: Vec<usize>,
}
impl Metrics {
    pub fn ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.dialogue as f64 / self.total as f64
        }
    }
}
/// コメント・参照の記号・注釈を除いた素の本文。除き方はエクスポートと同じ変換を使う（§9.4.2）。
pub fn prose(text: &str) -> String {
    crate::core::export::plain(text)
}
pub fn section(text: &str) -> Metrics {
    let plain = prose(text);
    let mut metrics = Metrics {
        total: plain.chars().filter(|c| !c.is_whitespace()).count(),
        ..Default::default()
    };
    let parsed = contents::parse(text);
    let mut previous: Option<String> = None;
    for line in parsed.lines {
        match line {
            contents::Line::Break(_) | contents::Line::Scene(_) => previous = None,
            contents::Line::Text(line) if annotations::illustration(line.raw).is_none() => {
                let spans = role_language::dialogue_spans(line.body);
                if spans.is_empty() {
                    if !line.body.trim().is_empty() {
                        previous = None;
                    }
                    continue;
                }
                let name = line
                    .speaker
                    .as_ref()
                    .map(|s| s.name.to_owned())
                    .or_else(|| previous.clone())
                    .unwrap_or_default();
                previous = Some(name.clone());
                let speech = metrics.speakers.entry(name).or_default();
                speech.lines += 1;
                for span in spans {
                    let count = prose(&line.body[span])
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .count();
                    speech.chars += count;
                    metrics.dialogue += count;
                }
            }
            _ => {}
        }
    }
    metrics
}
pub fn project(project: &Project) -> Metrics {
    let mut metrics = Metrics::default();
    for item in sections::all(project)
        .iter()
        .filter(|s| s.id.episode != 0 && s.id.chapter != 0)
    {
        let part = section(&sections::text_of(project.text(&item.path), item));
        metrics.total += part.total;
        metrics.dialogue += part.dialogue;
        metrics.lengths.push(part.total);
        for (name, speech) in part.speakers {
            let sum = metrics.speakers.entry(name).or_default();
            sum.chars += speech.chars;
            sum.lines += speech.lines;
        }
    }
    metrics
}
pub fn t_scores(values: &[usize]) -> Vec<Option<f64>> {
    let population: Vec<f64> = values
        .iter()
        .copied()
        .filter(|v| *v > 0)
        .map(|v| v as f64)
        .collect();
    if population.len() < 3 {
        return vec![None; values.len()];
    }
    let mean = population.iter().sum::<f64>() / population.len() as f64;
    let sd = (population.iter().map(|n| (n - mean).powi(2)).sum::<f64>() / population.len() as f64)
        .sqrt();
    values
        .iter()
        .map(|n| {
            if *n == 0 {
                None
            } else if sd == 0.0 {
                Some(50.0)
            } else {
                Some(50.0 + 10.0 * (*n as f64 - mean) / sd)
            }
        })
        .collect()
}
pub fn histogram(values: &[usize], bin: usize) -> BTreeMap<usize, usize> {
    let mut out = BTreeMap::new();
    for n in values {
        *out.entry(n / bin.max(1) * bin.max(1)).or_default() += 1;
    }
    out
}
/// 読点提案の閾値（§9.4.3 の `writing.long_sentence_chars`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommaOptions {
    /// この字数を**超える**文（読点 0 個）だけを対象にする。既定 120。
    pub long_sentence_chars: usize,
}

impl Default for CommaOptions {
    fn default() -> Self {
        Self {
            long_sentence_chars: 120,
        }
    }
}

impl CommaOptions {
    /// 閾値だけを変えた指定。
    pub fn long(chars: usize) -> Self {
        Self {
            long_sentence_chars: chars,
        }
    }
}

/// 優先度 1: 接続助詞（§9.4.3 の表）。**長いものから**照合する（`けれども` を `けれど` で切らない）。
/// i18n: format-vocabulary
const CONNECTIVES: &[&str] = &[
    "けれども",
    "けれど",
    "ので",
    "のに",
    "から",
    "ても",
    "たら",
    "なら",
    "ば",
    "が",
    "と",
    "て",
];

/// 優先度 2: 読点の直前に来やすい副詞句（§9.4.3 の表）。
/// i18n: format-vocabulary
const ADVERBIALS: &[&str] = &["ときに", "ために", "ように", "ながら", "つつ"];

/// 優先度 3: 文頭から [`TOPIC_WINDOW`] 字以内の主題の助詞（§9.4.3 の表）。
/// i18n: format-vocabulary
const TOPIC_PARTICLES: &[char] = &['は', 'も'];

/// 優先度 4: 列挙の助詞。[`ENUM_COUNT`] 回以上あるとき、各直後を候補にする（§9.4.3 の表）。
/// i18n: format-vocabulary
const ENUM_PARTICLES: &[char] = &['と', 'や'];

/// 表示する候補の数（§9.4.3「上位 3 件だけ」）。
const COMMA_TOP: usize = 3;
/// 既に `、` がある位置の前後、この字数以内は候補にしない（§9.4.3 の除外規則）。
const COMMA_GAP: usize = 6;
/// 行末からこの字数以内は候補にしない（§9.4.3 の除外規則）。
const LINE_END_GAP: usize = 4;
/// 主題の助詞を見る範囲（文頭から何字まで。§9.4.3 の優先度 3）。
const TOPIC_WINDOW: usize = 10;
/// 列挙と見なす回数（§9.4.3 の優先度 4「3 回以上」）。
const ENUM_COUNT: usize = 3;

/// 台詞の外側で、決定論的に「読点を打てそうな場所」を返す（§9.4.3）。文字は挿入しない。
///
/// 対象は**読点を含まず** [`CommaOptions::long_sentence_chars`] を超える文だけ。文の中の
/// 4 つの優先度の手がかりを集め、優先度の高い順に [`COMMA_TOP`] 件を選ぶ。返すのは候補位置の
/// byte の列（昇順）。除外規則は §9.4.3 の 4 つ（台詞の中・`、` の前後 [`COMMA_GAP`] 字以内・
/// 行末から [`LINE_END_GAP`] 字以内・`'参照` と `｜ルビ《…》` の途中）。
pub fn comma_candidates(text: &str, options: CommaOptions) -> Vec<usize> {
    let dialogue = role_language::dialogue_spans(text);
    let commas: Vec<usize> = text
        .char_indices()
        .filter(|(_, ch)| *ch == '、')
        .map(|(at, _)| text[..at].chars().count())
        .collect();
    let mut out: Vec<usize> = Vec::new();
    for (range, sentence) in sentence::split(text) {
        let count = sentence.chars().filter(|ch| !ch.is_whitespace()).count();
        if count <= options.long_sentence_chars || sentence.contains('、') {
            continue;
        }
        let base = text[..range.start].chars().count();
        let to = sentence.chars().filter(|ch| *ch == 'と').count();
        let ya = sentence.chars().filter(|ch| *ch == 'や').count();
        let mut scored: Vec<(u8, usize, usize)> = Vec::new();
        for (index, (at, ch)) in sentence.char_indices().enumerate() {
            let abs = range.start + at;
            if dialogue.iter().any(|span| span.contains(&abs))
                || matches!(ch, '「' | '」' | '『' | '』')
            {
                continue;
            }
            let rest = &sentence[at..];
            let mut best: Option<(u8, usize)> = None;
            for (priority, table) in [(1u8, CONNECTIVES), (2, ADVERBIALS)] {
                for pattern in table {
                    if rest.starts_with(pattern) {
                        let found = (priority, pattern.len());
                        if best.is_none_or(|(_, len)| found.1 > len) {
                            best = Some(found);
                        }
                    }
                }
            }
            if TOPIC_PARTICLES.contains(&ch) && index < TOPIC_WINDOW {
                best = best.or(Some((3, ch.len_utf8())));
            }
            let enumerated =
                ENUM_PARTICLES.contains(&ch) && (if ch == 'と' { to } else { ya }) >= ENUM_COUNT;
            if enumerated {
                best = best.or(Some((4, ch.len_utf8())));
            }
            let Some((priority, len)) = best else {
                continue;
            };
            let after = range.start + at + len;
            let tail = &text[after.min(text.len())..];
            let line_end = tail.find('\n').map_or(text.len(), |offset| after + offset);
            if text[after..line_end].chars().count() <= LINE_END_GAP {
                continue;
            }
            let char_at = base + index + rest[..len].chars().count();
            if commas
                .iter()
                .any(|comma| comma.abs_diff(char_at) <= COMMA_GAP)
            {
                continue;
            }
            let line_start = text[..after].rfind('\n').map_or(0, |offset| offset + 1);
            let prefix = &text[line_start..after];
            if inside_ruby(prefix) || inside_reference(prefix) {
                continue;
            }
            scored.push((priority, after, char_at));
        }
        scored.sort_unstable();
        scored.truncate(COMMA_TOP);
        out.extend(scored.into_iter().map(|(_, at, _)| at));
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// `prefix` の末尾がルビの読み（`《…》` の内側）か。§9.4.3 の除外規則。
fn inside_ruby(prefix: &str) -> bool {
    match (prefix.rfind('《'), prefix.rfind('》')) {
        (Some(open), Some(close)) => open > close,
        (Some(_), None) => true,
        _ => false,
    }
}

/// `prefix` の末尾が `'参照`（または `'{…}`）の途中か。§9.4.3 の除外規則。
fn inside_reference(prefix: &str) -> bool {
    let Some(at) = prefix.rfind('\'') else {
        return false;
    };
    let escaped = prefix[..at]
        .chars()
        .rev()
        .take_while(|ch| *ch == '\\')
        .count()
        % 2
        == 1;
    if escaped {
        return false;
    }
    let rest = &prefix[at + 1..];
    if let Some(form) = rest.strip_prefix('{') {
        return !form.contains('}');
    }
    !rest.is_empty() && rest.chars().all(crate::core::text::charclass::is_word_char)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Day {
    pub chars: i64,
    pub active_seconds: u64,
    pub sessions: u64,
    pub hours: Vec<usize>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, serde_json::Value>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub schema_version: u32,
    pub days: BTreeMap<String, Day>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, serde_json::Value>,
}
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub started: Option<f64>,
    pub last_activity: f64,
    pub baseline: usize,
    pub last_count: usize,
    pub delta: i64,
    fraction: f64,
}
impl Session {
    pub fn begin(&mut self, now: f64, chars: usize) {
        self.started = Some(now);
        self.last_activity = now;
        self.baseline = chars;
        self.last_count = chars;
        self.delta = 0;
        self.fraction = 0.0;
    }
    pub fn edit(&mut self, now: f64, chars: usize) -> (i64, u64, bool) {
        let reset = self.started.is_none() || now - self.last_activity >= 1800.0;
        let change = chars as i64 - self.last_count as i64;
        if reset {
            self.begin(now, self.last_count);
        }
        self.fraction += (now - self.last_activity).clamp(0.0, 1800.0);
        let active = self.fraction as u64;
        self.fraction -= active as f64;
        self.delta = chars as i64 - self.baseline as i64;
        self.last_count = chars;
        self.last_activity = now;
        (change, active, reset)
    }
    pub fn minutes(&self, now: f64) -> u64 {
        self.started.map_or(0, |s| {
            ((if now - self.last_activity >= 1800.0 {
                self.last_activity
            } else {
                now
            }) - s)
                .max(0.0) as u64
                / 60
        })
    }
}
impl Stats {
    pub fn record(&mut self, date: &str, hour: usize, chars: i64, seconds: u64, session: bool) {
        self.schema_version = 1;
        let day = self.days.entry(date.to_owned()).or_default();
        day.chars += chars;
        day.active_seconds += seconds;
        day.sessions += u64::from(session);
        if !day.hours.contains(&hour.min(23)) {
            day.hours.push(hour.min(23));
            day.hours.sort_unstable();
        }
        while self.days.len() > 400
            || (self.days.len() > 1 && serde_json::to_vec(self).is_ok_and(|v| v.len() > 64 * 1024))
        {
            self.days.pop_first();
        }
    }
}

/// 日付の計算は呼び出し側から、並べた暦の日付（書かなかった日も含む）として渡す。
impl Stats {
    pub fn average(&self, dates: &[String]) -> f64 {
        if dates.is_empty() {
            return 0.0;
        }
        dates
            .iter()
            .map(|d| self.days.get(d).map_or(0, |d| d.chars))
            .sum::<i64>() as f64
            / dates.len() as f64
    }
    pub fn streak(&self, dates_newest_first: &[String]) -> usize {
        dates_newest_first
            .iter()
            .take_while(|d| self.days.get(*d).is_some_and(|day| day.chars > 0))
            .count()
    }
}

/// 暦だけを扱う補助。日付は渡され、システムの時計からは決して読まない（§3.2）。
pub fn recent_dates(today: &str, count: usize) -> Vec<String> {
    let mut parts = today.split('-');
    let Some((mut y, mut m, mut d)) = parts
        .next()
        .and_then(|y| y.parse::<u32>().ok())
        .zip(parts.next().and_then(|m| m.parse::<u32>().ok()))
        .zip(parts.next().and_then(|d| d.parse::<u32>().ok()))
        .map(|((y, m), d)| (y, m, d))
    else {
        return Vec::new();
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Vec::new();
    }
    let mut dates = Vec::new();
    for _ in 0..count.min(400) {
        dates.push(format!("{y:04}-{m:02}-{d:02}"));
        if d > 1 {
            d -= 1;
        } else {
            if m > 1 {
                m -= 1;
            } else {
                if y == 0 {
                    break;
                }
                y -= 1;
                m = 12;
            }
            d = match m {
                4 | 6 | 9 | 11 => 30,
                2 => {
                    if y.is_multiple_of(400) || y.is_multiple_of(4) && !y.is_multiple_of(100) {
                        29
                    } else {
                        28
                    }
                }
                _ => 31,
            };
        }
    }
    dates
}
pub fn weekday(date: &str) -> Option<usize> {
    let mut p = date.split('-');
    let mut y = p.next()?.parse::<usize>().ok()?;
    let m = p.next()?.parse::<usize>().ok()?;
    let d = p.next()?.parse::<usize>().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || y < 1 {
        return None;
    }
    if m < 3 {
        y -= 1;
    }
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    Some((y + y / 4 - y / 100 + y / 400 + t[m - 1] + d) % 7)
}
