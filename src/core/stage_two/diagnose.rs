use super::{metadata, metrics};
use crate::core::{
    annotations,
    diagnostics::{Arg, Diagnostic, DiagnosticCode as C},
    features::Project,
    model::document::{Document, DocumentFormat},
    syntax::{characters_md, contents, materials_index},
    text::lexer::{self, FieldKey, LineKind, TextFormat},
};
fn diag(code: C, range: std::ops::Range<usize>, args: Vec<Arg>) -> Diagnostic {
    Diagnostic::new(code, Some(range), args)
}
pub fn document(doc: &Document, project: &Project, explicit: bool) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if doc.format == DocumentFormat::Contents
        && (explicit
            || doc.text.contains('^')
            || doc.text.contains('「')
            || doc.text.contains('『'))
    {
        let tree = contents::parse_with(&doc.text, &project.contents_options());
        let index = materials_index::parse(project.text("materials/index.json"));
        let addresses = metadata::addresses(project.text("characters.md"));
        let characters = characters_md::parse(project.text("characters.md"));
        let mut previous = String::new();
        let mut run = 0;
        let mut offset = 0;
        for line in &tree.lines {
            let range = offset..offset + line.raw().len();
            match line {
                contents::Line::Text(text) if annotations::illustration(text.raw).is_none() => {
                    for (at, id) in annotations::citations(text.body) {
                        if !index
                            .index
                            .iter()
                            .flat_map(|i| i.items.iter())
                            .any(|m| m.id.as_str() == id)
                        {
                            out.push(diag(
                                C::E012,
                                offset + at.start..offset + at.end,
                                vec![Arg::text(id)],
                            ));
                        }
                    }
                    let spans = crate::core::resolve::role_language::dialogue_spans(text.body);
                    if spans.is_empty() && !text.body.trim().is_empty() {
                        previous.clear();
                        run = 0;
                    }
                    if !spans.is_empty() {
                        let speaker = text.speaker.as_ref().map_or(previous.as_str(), |s| s.name);
                        if speaker == previous {
                            run += 1;
                        } else {
                            run = 1;
                        }
                        let name = speaker.to_owned();
                        if !name.is_empty()
                            && run > project.settings().writing.max_consecutive_speeches
                        {
                            out.push(diag(
                                C::W082,
                                range.clone(),
                                vec![Arg::text(&name), Arg::number(run as i64)],
                            ));
                        }
                        let mode = text.speaker.as_ref().and_then(|s| s.modes.first().copied());
                        let own: Vec<_> = addresses
                            .iter()
                            .filter(|a| {
                                a.speaker == name && (a.mode.is_none() || a.mode.as_deref() == mode)
                            })
                            .collect();
                        if !own.is_empty() {
                            for target in &characters.characters {
                                if target.name == name {
                                    continue;
                                }
                                let choice = own
                                    .iter()
                                    .filter(|a| a.target == target.name || a.target == "*")
                                    .max_by_key(|a| (a.target != "*", a.mode.is_some()));
                                let Some(allowed) = choice else { continue };
                                let body: String =
                                    spans.iter().map(|span| &text.body[span.clone()]).collect();
                                if body.contains(target.name)
                                    && !allowed.names.iter().any(|n| body.contains(n))
                                {
                                    out.push(diag(
                                        C::W084,
                                        range.clone(),
                                        vec![
                                            Arg::text(&name),
                                            Arg::text(target.name),
                                            Arg::text(target.name),
                                        ],
                                    ));
                                }
                            }
                        }
                        previous = name;
                    }
                    if explicit {
                        out.extend(style(text.body, offset, project));
                    }
                }
                contents::Line::Scene(_) | contents::Line::Break(_) => {
                    previous.clear();
                    run = 0;
                }
                _ => {}
            }
            offset += line.raw().len() + 1;
        }
    }
    if explicit && doc.format == DocumentFormat::Contents {
        let m = metrics::section(&doc.text);
        if m.total > 0 && m.ratio() > project.settings().writing.dialogue_ratio_target {
            out.push(Diagnostic::new(
                C::W090,
                None,
                vec![
                    Arg::number((m.ratio() * 100.0) as i64),
                    Arg::number((project.settings().writing.dialogue_ratio_target * 100.0) as i64),
                ],
            ));
        }
    }
    if matches!(
        doc.format,
        DocumentFormat::PlotOutline | DocumentFormat::PlotEpisode | DocumentFormat::Characters
    ) {
        let mut offset = 0;
        for raw in doc.text.lines() {
            if let LineKind::Field(field) = lexer::classify(raw, TextFormat::Markdown) {
                if field.key == FieldKey::Icon
                    || (field.key == FieldKey::Space
                        && (field.value.starts_with("./")
                            || field.value.starts_with("assets/")
                            || field.value.starts_with("../")
                            || field.value.starts_with('/')
                            || field.value.contains("://")))
                {
                    let value = field
                        .value
                        .split(" //")
                        .next()
                        .unwrap_or(field.value)
                        .trim();
                    match metadata::asset_path(value, field.key == FieldKey::Icon) {
                        None => out.push(diag(
                            C::E062,
                            offset..offset + raw.len(),
                            vec![Arg::text(value)],
                        )),
                        Some(path) if !project.has_file(&path) => out.push(diag(
                            if field.key == FieldKey::Icon {
                                C::W070
                            } else {
                                C::W092
                            },
                            offset..offset + raw.len(),
                            vec![Arg::text(value)],
                        )),
                        Some(path)
                            if field.key == FieldKey::Icon
                                && !path.to_ascii_lowercase().ends_with(".png")
                                && !path.to_ascii_lowercase().ends_with(".jpg")
                                && !path.to_ascii_lowercase().ends_with(".jpeg") =>
                        {
                            out.push(diag(
                                C::W068,
                                offset..offset + raw.len(),
                                vec![Arg::text(value)],
                            ))
                        }
                        Some(path)
                            if field.key == FieldKey::Icon
                                && project.assets.get(&path).is_some_and(|bytes| {
                                    bytes.is_empty()
                                        || bytes.len() > 256 * 1024
                                        || annotations::image_dimensions(bytes)
                                            .is_some_and(|(w, h)| w > 256 || h > 256)
                                }) =>
                        {
                            out.push(diag(
                                C::W069,
                                offset..offset + raw.len(),
                                vec![Arg::text(value)],
                            ))
                        }
                        _ => {}
                    }
                }
            }
            offset += raw.len() + 1;
        }
    }
    if matches!(
        doc.format,
        DocumentFormat::PlotOutline | DocumentFormat::PlotEpisode
    ) {
        let rows = metadata::records(project);
        if explicit {
            let progress = crate::core::features::plots::progress(project);
            for depth in 0..=3 {
                let group: Vec<_> = progress.iter().filter(|r| r.depth == depth).collect();
                let scores = metrics::t_scores(&group.iter().map(|r| r.chars).collect::<Vec<_>>());
                for (row, score) in group.into_iter().zip(scores) {
                    if score.is_some_and(|t| !(35.0..=65.0).contains(&t)) {
                        if let Some(record) =
                            rows.iter().find(|r| r.path == doc.path && r.id == row.id)
                        {
                            out.push(diag(
                                C::W091,
                                record.range.clone(),
                                vec![
                                    Arg::text(row.id.display(project.digits())),
                                    Arg::number(score.unwrap().round() as i64),
                                ],
                            ));
                        }
                    }
                }
            }
            let mut sight_run = 0;
            for row in &rows {
                if let Some(senses) = row.fields.get("senses") {
                    let sight = senses
                        .split([',', '、', ' '])
                        .filter(|s| !s.is_empty())
                        .all(|s| SIGHT_WORDS.contains(&s));
                    if sight {
                        sight_run += 1;
                    } else {
                        sight_run = 0;
                    }
                    if sight_run >= 5 && row.path == doc.path {
                        out.push(diag(
                            C::W083,
                            row.range.clone(),
                            vec![Arg::number(sight_run)],
                        ));
                    }
                }
            }
            for row in rows.iter().filter(|r| r.path == doc.path) {
                if let Some(blurb) = row.fields.get("blurb") {
                    let count = sections_count(blurb);
                    if count > 800 {
                        out.push(diag(
                            C::W086,
                            row.range.clone(),
                            vec![Arg::number(count as i64)],
                        ));
                    }
                }
            }
        }

        let times = metadata::timeline(&rows);
        let mut previous = std::collections::BTreeMap::new();
        for row in times {
            if let Some(time) = row.time.start {
                if row.record.path == doc.path
                    && !row.time.flashback
                    && previous
                        .get(&row.record.depth)
                        .is_some_and(|p| (time, row.time.clock) < *p)
                {
                    out.push(diag(
                        C::W080,
                        row.record.range.clone(),
                        vec![
                            Arg::text(previous[&row.record.depth].0.to_string()),
                            Arg::text(time.to_string()),
                        ],
                    ));
                }
                previous.insert(row.record.depth, (time, row.time.clock));
            }
        }
    }
    if doc.format == DocumentFormat::Characters {
        let addresses = metadata::addresses(&doc.text);
        let characters = characters_md::parse(&doc.text);
        let starts = crate::core::features::project::line_starts_of(&doc.text);
        let mut names = std::collections::BTreeMap::new();
        for address in addresses {
            let range = starts.get(address.line).copied().unwrap_or(0)
                ..starts
                    .get(address.line + 1)
                    .copied()
                    .unwrap_or(doc.text.len());
            if address.target != "*"
                && !characters
                    .characters
                    .iter()
                    .any(|c| c.name == address.target)
            {
                out.push(diag(
                    C::E013,
                    range.clone(),
                    vec![Arg::text(&address.target)],
                ));
            }
            for name in address.names {
                if names
                    .insert(name.clone(), address.target.clone())
                    .is_some_and(|target| target != address.target)
                {
                    out.push(diag(C::W085, range.clone(), vec![Arg::text(name)]));
                }
            }
        }
    }
    if doc.format == DocumentFormat::Proposal {
        if let Some(p) = crate::core::agents::files::parse_proposal(&doc.text)
            .filter(|p| p.kind == "human" && p.status == "open")
        {
            let current = p.target_parts().1.map(|id| {
                crate::core::agents::apply::current_text(
                    project,
                    crate::core::agents::Kind::SectionText,
                    id,
                )
            });
            if current.as_deref() != Some(p.before.as_str()) {
                out.push(Diagnostic::new(C::W093, None, vec![Arg::text(p.id)]));
            }
        }
    }
    if doc.format == DocumentFormat::MaterialsIndex {
        let cited: Vec<String> = project
            .documents()
            .filter(|d| d.format == DocumentFormat::Contents)
            .flat_map(|d| {
                contents::parse(&d.text).lines.into_iter().flat_map(|l| {
                    if let contents::Line::Text(t) = l {
                        annotations::citations(t.body)
                            .into_iter()
                            .map(|(_, id)| id.to_owned())
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    }
                })
            })
            .collect();
        let index = materials_index::parse(&doc.text);
        let unused = index
            .index
            .iter()
            .flat_map(|i| i.items.iter())
            .filter(|m| m.status == "approved" && !cited.iter().any(|id| id == m.id.as_str()))
            .count();
        if unused > 0 {
            out.push(Diagnostic::new(
                C::I054,
                None,
                vec![Arg::number(unused as i64)],
            ));
        }
    }
    if doc.format == DocumentFormat::ProjectFile {
        for role in &project.settings().agents.custom_roles {
            if !crate::core::syntax::prompt_md::is_role_name(role) {
                out.push(Diagnostic::new(C::E032, None, vec![Arg::text(role)]));
            }
            if crate::core::agents::is_role(role) {
                out.push(Diagnostic::new(C::E033, None, vec![Arg::text(role)]));
            }
        }
        if project.settings().agents.custom_roles.len() > 16 {
            out.push(Diagnostic::new(
                C::E034,
                None,
                vec![Arg::number(
                    project.settings().agents.custom_roles.len() as i64
                )],
            ));
        }
        let local = &project.settings().agents.local_llm;
        if local.enabled && !super::local_llm::is_loopback(&local.url) {
            out.push(Diagnostic::new(
                C::E064,
                None,
                vec![Arg::text("loopback URL required")],
            ));
        }
        let stock = crate::core::features::sections::all(project)
            .iter()
            .filter(|s| s.id.chapter == 0)
            .count();
        if stock > 0 {
            out.push(Diagnostic::new(
                C::I055,
                None,
                vec![Arg::number(stock as i64)],
            ));
        }
        for d in project
            .documents()
            .filter(|d| d.format == DocumentFormat::CustomPrompt)
        {
            let role = d
                .path
                .trim_start_matches("prompts/custom/")
                .trim_end_matches(".md");
            if !project
                .settings()
                .agents
                .custom_roles
                .iter()
                .any(|r| r == role)
            {
                out.push(Diagnostic::new(C::I056, None, vec![Arg::text(role)]));
            }
        }
    }
    out
}
/// i18n: format-vocabulary
fn style(body: &str, offset: usize, project: &Project) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let limit = project.settings().writing.long_sentence_chars as usize;
    let mut at = 0;
    let mut conjunctions = 0;
    for sentence in body.split_inclusive(['。', '！', '？']) {
        let count = metrics::prose(sentence)
            .chars()
            .filter(|c| !c.is_whitespace())
            .count();
        if count > limit && !sentence.contains('、') {
            out.push(diag(
                C::W081,
                offset + at..offset + at + sentence.len(),
                vec![Arg::number(count as i64)],
            ));
        }
        if count > limit * 2 {
            out.push(diag(
                C::W089,
                offset + at..offset + at + sentence.len(),
                vec![Arg::number(count as i64)],
            ));
        }
        if CONJUNCTIONS.iter().any(|p| sentence.trim().starts_with(p)) {
            conjunctions += 1;
        } else {
            conjunctions = 0;
        }
        if conjunctions >= 3 {
            out.push(diag(
                C::W088,
                offset + at..offset + at + sentence.len(),
                vec![],
            ));
        }
        at += sentence.len();
    }
    let words = tiniestsegmenter::tokenize(body);
    let mut counts = std::collections::BTreeMap::new();
    for word in words.iter().filter(|w| w.chars().count() >= 2) {
        *counts.entry(*word).or_insert(0) += 1;
    }
    for (word, count) in counts {
        if count >= 3 {
            out.push(diag(
                C::W087,
                offset..offset + body.len(),
                vec![Arg::text(word), Arg::number(count)],
            ));
        }
    }
    out
}

/// i18n: format-vocabulary
const CONJUNCTIONS: &[&str] = &["しかし", "そして", "だから", "だが", "また", "それでも"];

fn sections_count(text: &str) -> usize {
    crate::core::features::sections::count_chars(text)
}

/// i18n: format-vocabulary
const SIGHT_WORDS: &[&str] = &["視覚", "視", "sight"];
