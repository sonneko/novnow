//! Descriptive observations, never scores or evaluations.
use crate::core::{
    annotations,
    features::{sections, Project},
    resolve::role_language,
    syntax::contents,
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
/// Raw prose with comments, reference sigils and annotations removed, using the same export conversion.
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
/// Deterministic advisory positions outside all dialogue spans; never inserts text.
/// i18n: format-vocabulary
pub fn comma_candidates(text: &str, threshold: usize) -> Vec<usize> {
    let spans = role_language::dialogue_spans(text);
    let mut count = 0;
    let mut out = Vec::new();
    for (at, ch) in text.char_indices() {
        if spans.iter().any(|r| r.contains(&at)) || matches!(ch, '「' | '」' | '『' | '』') {
            count = 0;
            continue;
        }
        if matches!(ch, '。' | '、' | '！' | '？' | '\n') {
            count = 0;
            continue;
        }
        if !ch.is_whitespace() {
            count += 1;
        }
        if count >= threshold.max(20) && matches!(ch, 'は' | 'が' | 'を' | 'に' | 'で' | 'と')
        {
            out.push(at + ch.len_utf8());
            count = 0;
        }
    }
    out
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

/// Date arithmetic is supplied by the caller as ordered calendar dates, including empty days.
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

/// Calendar-only helpers: dates are supplied, never read from a system clock.
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
