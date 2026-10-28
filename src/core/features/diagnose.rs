use super::{metadata, metrics};
use crate::core::resolve::role_language;
use crate::core::{
    annotations,
    diagnostics::{Arg, Diagnostic, DiagnosticCode as C},
    features::Project,
    model::document::{Document, DocumentFormat},
    syntax::{characters_md, contents, materials_index},
    text::{
        charclass,
        lexer::{self, FieldKey, LineKind, TextFormat},
        sentence,
    },
};
fn diag(code: C, range: std::ops::Range<usize>, args: Vec<Arg>) -> Diagnostic {
    Diagnostic::new(code, Some(range), args)
}
pub fn document(doc: &Document, project: &Project, explicit: bool) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // W097–W106（§7.2.3–§7.2.12）は組み込み既定では無効。`[features.diagnostics]`
    // で有効化されたとき、または文体の明示操作（explicit）のときだけ出す（W096 と同じ）。
    let overrides = &project.settings().features.diagnostics;
    let lint_on = |code: C| {
        explicit
            || crate::core::diagnostics::level_for(code, overrides)
                .is_some_and(|level| level != crate::core::diagnostics::DiagnosticLevel::Off)
    };
    let section_active: Vec<C> = [C::W097, C::W099, C::W100, C::W101, C::W102, C::W106]
        .into_iter()
        .filter(|code| lint_on(*code))
        .collect();
    let scene_active: Vec<C> = [C::W098, C::W103, C::W104, C::W105]
        .into_iter()
        .filter(|code| lint_on(*code))
        .collect();
    // 文体の観察（W081 / W087 / W088 / W089）は節ではなく行ごとに数える（§7.2 の実行コスト表）。
    let style_active: Vec<C> = [C::W081, C::W087, C::W088, C::W089]
        .into_iter()
        .filter(|code| lint_on(*code))
        .collect();
    let writing_on = !section_active.is_empty()
        || !scene_active.is_empty()
        || !style_active.is_empty()
        || lint_on(C::W090);
    if doc.format == DocumentFormat::Contents
        && (explicit
            || doc.text.contains('^')
            || doc.text.contains('「')
            || doc.text.contains('『')
            || writing_on)
    {
        let tree = contents::parse_with(&doc.text, &project.contents_options());
        // 日本語主体の判定は文書ごとに 1 回だけ（§7.2.2 と同一）。
        let jp = if writing_on {
            japanese_dominant(project)
        } else {
            true
        };
        let index = materials_index::parse(project.text("materials/index.json"));
        let addresses = metadata::addresses(project.text("characters.md"));
        let characters = characters_md::parse(project.text("characters.md"));
        let mut previous = String::new();
        let mut run = 0;
        let mut offset = 0;
        // 節（`;` 区切り）単位の執筆支援（W097 / W099 / W100 / W101 / W102 / W106）。
        // 改行・段落は連続を切らず、節をまたがない（§7.2.3 ほか）。
        let mut section: Vec<(String, usize)> = Vec::new();
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
                    let spans = role_language::dialogue_spans(text.body);
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
                    if !style_active.is_empty() {
                        out.extend(style(text.body, offset, project, &style_active));
                    }
                    if jp && !section_active.is_empty() {
                        section.push((text.body.to_string(), offset));
                    }
                }
                contents::Line::Scene(_) | contents::Line::Break(_) => {
                    previous.clear();
                    run = 0;
                    if !section.is_empty() {
                        section_lints(&section, &section_active, &mut out);
                        section.clear();
                    }
                }
                _ => {}
            }
            offset += line.raw().len() + 1;
        }
        if !section.is_empty() {
            section_lints(&section, &section_active, &mut out);
        }
        // W090: 台詞比率が上限目安を超えた節（§9.4.2 の算出、§7.2 の実行コスト表で「節のみ」）。
        if lint_on(C::W090) {
            let target = project.settings().writing.dialogue_ratio_target;
            let mut starts = Vec::with_capacity(tree.lines.len());
            let mut at = 0usize;
            for line in &tree.lines {
                starts.push(at);
                at += line.raw().len() + 1;
            }
            for section in &tree.sections {
                let mut text = String::new();
                let mut range = None;
                for index in section.content.clone() {
                    let Some(line) = tree.lines.get(index) else {
                        continue;
                    };
                    let start = starts.get(index).copied().unwrap_or(0);
                    range.get_or_insert(start..start + line.raw().len());
                    if let Some(range) = range.as_mut() {
                        range.end = start + line.raw().len();
                    }
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(line.raw());
                }
                let m = metrics::section(&text);
                if m.total > 0 && m.ratio() > target {
                    out.push(Diagnostic::new(
                        C::W090,
                        range,
                        vec![
                            Arg::number((m.ratio() * 100.0) as i64),
                            Arg::number((target * 100.0) as i64),
                        ],
                    ));
                }
            }
        }
        // 場面単位の執筆支援（W098 / W103 / W104 / W105）: 場面内の全節を集計する（§7.2.4 ほか）。
        if jp && !scene_active.is_empty() {
            let mut scene: Vec<(String, usize)> = Vec::new();
            let mut scene_offset = 0usize;
            for line in &tree.lines {
                let line_len = line.raw().len() + 1;
                match line {
                    contents::Line::Text(text) if annotations::illustration(text.raw).is_none() => {
                        scene.push((text.body.to_string(), scene_offset));
                    }
                    contents::Line::Scene(_) => {
                        scene_lints(&scene, &scene_active, &mut out);
                        scene.clear();
                    }
                    _ => {}
                }
                scene_offset += line_len;
            }
            scene_lints(&scene, &scene_active, &mut out);
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
                    // 解決できない値（絶対パス・`../`・`icon` が `assets/` の外）は、
                    // 「参照先が無い」と同じ扱いにする（W070 / W092。§5.3.6・§5.5.3）。
                    let missing = if field.key == FieldKey::Icon {
                        C::W070
                    } else {
                        C::W092
                    };
                    match metadata::asset_path(value, field.key == FieldKey::Icon) {
                        None => out.push(diag(
                            missing,
                            offset..offset + raw.len(),
                            vec![Arg::text(value)],
                        )),
                        Some(path) if !project.has_file(&path) => out.push(diag(
                            missing,
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
    // W096（§7.2.2）は組み込み既定では無効。有効化されているか、文体の明示操作のときだけ出す。
    if doc.format == DocumentFormat::Contents
        && (explicit
            || crate::core::diagnostics::level_for(
                C::W096,
                &project.settings().features.diagnostics,
            )
            .is_some_and(|level| level != crate::core::diagnostics::DiagnosticLevel::Off))
    {
        out.extend(dialogue_spacing(doc, project));
    }
    out
}

/// 台詞内の句読点の後の空白を集計する記号（§7.2.2）。
const DIALOGUE_MARKS: [char; 3] = ['。', '！', '？'];

/// 台詞内 1 件（記号の位置〔文書内 byte〕、記号の添字、全角空白の有無）。
struct SpacingEvent {
    range: std::ops::Range<usize>,
    mark: usize,
    spaced: bool,
}

/// 1 つの本文行から台詞内の句読点イベントを拾う（`base` は文書内での行頭 byte）。
///
/// 台詞の内側の判定は [`role_language::dialogue_spans`] に任せる（入れ子と閉じ忘れの
/// 扱いを 1 か所に保つため）。行を渡しているので、閉じ忘れた台詞は行末で切れる。
fn spacing_events(body: &str, base: usize, out: &mut Vec<SpacingEvent>) {
    let chars: Vec<(usize, char)> = body.char_indices().collect();
    let spans = role_language::dialogue_spans(body);
    for (index, (at, character)) in chars.iter().enumerate() {
        if !spans.iter().any(|span| span.contains(at)) {
            continue;
        }
        let Some(mark) = DIALOGUE_MARKS.iter().position(|m| m == character) else {
            continue;
        };
        // 記号の次を見る。全角空白だけを「空白あり」とし、同じ台詞内に文字が
        // 続く場合だけを数える（閉じ括弧の直前は集計にも診断にも含めない）。
        let mut cursor = index + 1;
        let mut spaced = false;
        if chars.get(cursor).is_some_and(|(_, c)| *c == '　') {
            spaced = true;
            cursor += 1;
        }
        match chars.get(cursor) {
            Some((_, next)) if *next != '」' && *next != '』' => {
                let end = if spaced {
                    chars[index + 1].0 + '　'.len_utf8()
                } else {
                    at + character.len_utf8()
                };
                out.push(SpacingEvent {
                    range: base + at..base + end,
                    mark,
                    spaced,
                });
            }
            _ => {}
        }
    }
}

/// プロジェクトの全本文からイベントを集める（W096 の作品全体集計）。
fn all_spacing_events(project: &Project) -> Vec<(String, SpacingEvent)> {
    let mut out = Vec::new();
    for document in project
        .documents()
        .filter(|d| d.format == DocumentFormat::Contents)
    {
        let tree = contents::parse_with(&document.text, &project.contents_options());
        let mut offset = 0usize;
        for line in &tree.lines {
            if let contents::Line::Text(text) = line {
                // body は raw の部分文字列。raw 内での開始位置をポインタ差で取る。
                let within = text.body.as_ptr() as usize - text.raw.as_ptr() as usize;
                let mut events = Vec::new();
                spacing_events(text.body, offset + within, &mut events);
                out.extend(
                    events
                        .into_iter()
                        .map(|event| (document.path.clone(), event)),
                );
            }
            offset += line.raw().len() + 1;
        }
    }
    out
}

/// 日本語主体かどうかを本文の文字種だけで決める（表示言語は使わない、§7.2.2）。
fn japanese_dominant(project: &Project) -> bool {
    let mut japanese = 0usize;
    let mut latin = 0usize;
    for document in project
        .documents()
        .filter(|d| d.format == DocumentFormat::Contents)
    {
        for character in document.text.chars() {
            if charclass::is_body_japanese(character) {
                japanese += 1;
            } else if charclass::is_body_latin(character) {
                latin += 1;
            }
        }
    }
    japanese > latin
}

/// W096: 作品全体の多数派に揃っていない箇所だけを、この文書について出す（§7.2.2）。
fn dialogue_spacing(doc: &Document, project: &Project) -> Vec<Diagnostic> {
    if !japanese_dominant(project) {
        return Vec::new();
    }
    let events = all_spacing_events(project);
    // 記号ごとに（空白あり, 空白なし）を数え、合計 5 件以上かつ差 2 件以上の
    // ときだけ少数派を指摘する。
    let mut counts = [(0usize, 0usize); 3];
    for (_, event) in &events {
        if event.spaced {
            counts[event.mark].0 += 1;
        } else {
            counts[event.mark].1 += 1;
        }
    }
    let mut out = Vec::new();
    for (path, event) in &events {
        if path != &doc.path {
            continue;
        }
        let (spaced, unspaced) = counts[event.mark];
        if spaced + unspaced < 5 || spaced.abs_diff(unspaced) < 2 {
            continue;
        }
        let minority = if spaced > unspaced {
            !event.spaced
        } else {
            event.spaced
        };
        if minority {
            out.push(diag(
                C::W096,
                event.range.clone(),
                vec![Arg::text(DIALOGUE_MARKS[event.mark].to_string())],
            ));
        }
    }
    out
}
/// 文体の観察（W081 / W087 / W088 / W089、§9.4.4）。`active` に入っているものだけ出す。
/// i18n: format-vocabulary
fn style(body: &str, offset: usize, project: &Project, active: &[C]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let limit = project.settings().writing.long_sentence_chars as usize;
    let mut conjunctions = 0;
    for (range, sentence) in sentence::split(body) {
        let count = metrics::prose(sentence)
            .chars()
            .filter(|c| !c.is_whitespace())
            .count();
        if active.contains(&C::W081) && count > limit && !sentence.contains('、') {
            out.push(diag(
                C::W081,
                offset + range.start..offset + range.end,
                vec![Arg::number(count as i64)],
            ));
        }
        if active.contains(&C::W089) && count > limit * 2 {
            out.push(diag(
                C::W089,
                offset + range.start..offset + range.end,
                vec![Arg::number(count as i64)],
            ));
        }
        if CONJUNCTIONS.iter().any(|p| sentence.trim().starts_with(p)) {
            conjunctions += 1;
        } else {
            conjunctions = 0;
        }
        // 3 連続の 3 文目に 1 件だけ（W097 / W100 / W101 と同じ数え方）。
        if active.contains(&C::W088) && conjunctions == 3 {
            out.push(diag(
                C::W088,
                offset + range.start..offset + range.end,
                vec![],
            ));
        }
    }
    let words = tiniestsegmenter::tokenize(body);
    let mut counts = std::collections::BTreeMap::new();
    for word in words.iter().filter(|w| w.chars().count() >= 2) {
        *counts.entry(*word).or_insert(0) += 1;
    }
    for (word, count) in counts {
        if active.contains(&C::W087) && count >= 3 {
            out.push(diag(
                C::W087,
                offset..offset + body.len(),
                vec![Arg::text(word), Arg::number(count)],
            ));
        }
    }
    out
}

/// 文頭の接続詞（W088、§9.4.4 の固定リスト 14 語）。
/// i18n: format-vocabulary
const CONJUNCTIONS: &[&str] = &[
    "しかし",
    "だが",
    "でも",
    "そして",
    "それから",
    "だから",
    "つまり",
    "また",
    "ところが",
    "しかも",
    "けれど",
    "それでも",
    "そのため",
    "なぜなら",
];

/// 分詞のトークン列と各トークンの `text` 内の byte 位置（§9.4.4）。
///
/// `tiniestsegmenter::tokenize` は位置を返さないが、トークンは入力から順番に
/// 現れるため、カーソルで進みながら位置を復元する。
///
/// 見つからないトークンは飛ばす（辞書の変更などで原文に無いトークンが返っても、
/// 執筆中の本文で落とさない。位置が取れないトークンは数えないだけ）。
fn token_positions(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    for token in tiniestsegmenter::tokenize(text) {
        let Some(at) = text[cursor..].find(token) else {
            continue;
        };
        let start = cursor + at;
        out.push((start, token));
        cursor = start + token.len();
    }
    out
}

/// トークンが語か（文字種の判定。標点・空白のトークンは語にしない、§9.4.4）。
fn is_word_token(token: &str) -> bool {
    token.chars().any(charclass::is_word_char)
}

/// 文末の固定除外リスト（W097、§9.4.4）: 1 字助詞と 2 字助詞。
/// i18n: format-vocabulary
const SENTENCE_END_EXCLUDE_1: &[char] = &[
    'わ', 'が', 'を', 'に', 'で', 'と', 'え', 'の', 'は', 'や', 'も', 'ね', 'よ', 'か', 'ぜ', 'ぞ',
    'さ', 'あ',
];
/// i18n: format-vocabulary
const SENTENCE_END_EXCLUDE_2: &[&str] = &["から", "まで"];

/// トークンが助詞か（W099 の「間が助詞のみ」判定。1 字助詞リストと `から` / `まで`）。
fn is_particle_token(token: &str) -> bool {
    if SENTENCE_END_EXCLUDE_2.contains(&token) {
        return true;
    }
    let mut chars = token.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(c), None) if SENTENCE_END_EXCLUDE_1.contains(&c)
    )
}

/// 文の末尾から除外リストの助詞を除いた最後の自立語（W097 の文末尾、§9.4.4）。
fn sentence_end(narrative: &str) -> Option<&str> {
    for token in tiniestsegmenter::tokenize(narrative).iter().rev() {
        if !is_word_token(token) || is_particle_token(token) {
            continue;
        }
        return Some(token);
    }
    None
}

/// 文（`body` の `base` 起点）から台詞（`spans`）を除いた地の文部分。
fn narrative_of(sentence: &str, base: usize, spans: &[std::ops::Range<usize>]) -> String {
    let mut out = String::with_capacity(sentence.len());
    for (at, character) in sentence.char_indices() {
        let pos = base + at;
        if spans.iter().any(|span| span.contains(&pos)) {
            continue;
        }
        out.push(character);
    }
    out
}

/// W097: 文末尾の反復（§7.2.3）。
fn sentence_end_repeat(body: &str, offset: usize) -> Vec<Diagnostic> {
    let spans = role_language::dialogue_spans(body);
    let mut out = Vec::new();
    let mut run: Option<String> = None;
    let mut count = 0usize;
    for (sentence_range, sentence) in sentence::split(body) {
        let narrative = narrative_of(sentence, sentence_range.start, &spans);
        match sentence_end(&narrative) {
            None => {
                run = None;
                count = 0;
            }
            Some(word) => {
                if run.as_deref() == Some(word) {
                    count += 1;
                } else {
                    run = Some(word.to_string());
                    count = 1;
                }
                if count == 3 {
                    out.push(diag(
                        C::W097,
                        offset + sentence_range.start..offset + sentence_range.end,
                        vec![],
                    ));
                }
            }
        }
    }
    out
}

/// W099: 隣接同語反復（§7.2.5）。
fn adjacent_word_repeat(body: &str, offset: usize) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut last: Option<&str> = None;
    for (pos, token) in token_positions(body) {
        if !is_word_token(token) || is_particle_token(token) {
            continue;
        }
        if Some(token) == last {
            out.push(diag(
                C::W099,
                offset + pos..offset + pos + token.len(),
                vec![Arg::text(token)],
            ));
            last = None;
            continue;
        }
        last = Some(token);
    }
    out
}

/// 文末助詞の固定リスト（W100、§9.4.4）。
/// i18n: format-vocabulary
const END_PARTICLE_2: &[&str] = &["だぜ", "わけ", "かな"];
/// i18n: format-vocabulary
const END_PARTICLE_1: &[char] = &[
    'よ', 'ね', 'か', 'ぞ', 'だ', 'ざ', 'な', 'わ', 'や', 'さ', 'ぜ',
];

/// 文の末尾の助詞（2 字リストを先に調べる、`かな` が `な` として拾われないように）。
fn end_particle(sentence: &str) -> Option<String> {
    let core = sentence
        .strip_suffix(&['。', '！', '？'][..])
        .unwrap_or(sentence);
    END_PARTICLE_2
        .iter()
        .find(|particle| core.ends_with(**particle))
        .map(|particle| particle.to_string())
        .or_else(|| {
            END_PARTICLE_1
                .iter()
                .find(|particle| core.ends_with(**particle))
                .map(|particle| particle.to_string())
        })
}

/// W100: 文末助詞の反復（§7.2.6）。
fn end_particle_repeat(body: &str, offset: usize) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut run: Option<String> = None;
    let mut count = 0usize;
    for (sentence_range, sentence) in sentence::split(body) {
        match end_particle(sentence) {
            None => {
                run = None;
                count = 0;
            }
            Some(particle) => {
                if run.as_deref() == Some(particle.as_str()) {
                    count += 1;
                } else {
                    run = Some(particle.clone());
                    count = 1;
                }
                if count == 3 {
                    out.push(diag(
                        C::W100,
                        offset + sentence_range.start..offset + sentence_range.end,
                        vec![Arg::text(particle)],
                    ));
                }
            }
        }
    }
    out
}

/// W101: 文頭語の反復（§7.2.7）。
fn start_word_repeat(body: &str, offset: usize) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut run: Option<String> = None;
    let mut count = 0usize;
    for (sentence_range, sentence) in sentence::split(body) {
        let word = tiniestsegmenter::tokenize(sentence)
            .first()
            .filter(|token| token.chars().count() >= 2)
            .map(|token| token.to_string());
        match word {
            None => {
                run = None;
                count = 0;
            }
            Some(word) => {
                if run.as_deref() == Some(word.as_str()) {
                    count += 1;
                } else {
                    run = Some(word.clone());
                    count = 1;
                }
                if count == 3 {
                    out.push(diag(
                        C::W101,
                        offset + sentence_range.start..offset + sentence_range.end,
                        vec![Arg::text(word)],
                    ));
                }
            }
        }
    }
    out
}

/// W102 / W106: 記号の非重複出現を数えて、閾値回目の位置に 1 件（§7.2.8 / §7.2.12）。
fn repeated_symbol(
    code: C,
    body: &str,
    offset: usize,
    symbol: &str,
    threshold: usize,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut count = 0usize;
    let mut cursor = 0usize;
    while let Some(at) = body[cursor..].find(symbol) {
        count += 1;
        let pos = cursor + at;
        if count == threshold {
            out.push(diag(
                code,
                offset + pos..offset + pos + symbol.len(),
                vec![Arg::number(count as i64)],
            ));
        }
        cursor = pos + symbol.len();
    }
    out
}

/// 不確か表現の固定リスト（W103、§9.4.4）。
/// i18n: format-vocabulary
const HEDGE_WORDS: &[&str] = &[
    "だがしかし",
    "たぶん",
    "たしかに",
    "もしかすると",
    "かまわず",
    "おそらく",
    "実は",
    "かね",
];

/// 活用接尾辞の固定リスト（W104、§9.4.4）。セグメンタは活用を別のトークンに
/// 切る（「走った」→ `走っ` + `た`）ため、語（2 字以上の語トークン）の**直後**の
/// トークンがこれに属するとき、その語を動詞の語幹として数える。
/// i18n: format-vocabulary
const VERB_SUFFIX_TOKENS: &[&str] = &[
    "た",
    "て",
    "だ",
    "ながら",
    "られる",
    "させる",
    "ない",
    "たず",
    "ちゃう",
    "たい",
    "よう",
];

/// 節（`;` 区切り）の本文行を 1 本に連結し、連結位置を文書内 byte に戻せるようにする。
///
/// 仕様は「改行・段落は連続を切らず、節をまたがない」（§7.2.3 ほか）なので、
/// 節の中の複数行は改行を挟んで連結してから数える。
struct SectionText {
    joined: String,
    /// (連結内の行頭, 文書内の行頭)
    starts: Vec<(usize, usize)>,
}

impl SectionText {
    fn new(lines: &[(String, usize)]) -> Self {
        let mut joined = String::new();
        let mut starts = Vec::new();
        for (i, (body, doc)) in lines.iter().enumerate() {
            if i > 0 {
                joined.push('\n');
            }
            starts.push((joined.len(), *doc));
            joined.push_str(body);
        }
        Self { joined, starts }
    }

    /// 連結位置 → 文書内 byte。
    fn doc(&self, pos: usize) -> usize {
        match self.starts.binary_search_by(|(start, _)| start.cmp(&pos)) {
            Ok(i) => self.starts[i].1,
            Err(0) => self.starts[0].1,
            Err(i) => {
                let (start, doc) = self.starts[i - 1];
                doc + (pos - start)
            }
        }
    }

    /// 連結位置の範囲 → 文書内の範囲（範囲は行をまたがない）。
    fn span(&self, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
        let start = self.doc(range.start);
        let end = if range.end > range.start {
            self.doc(range.end - 1) + 1
        } else {
            start
        };
        start..end
    }
}

/// 節単位の執筆支援（§7.2.3 / §7.2.5 / §7.2.6 / §7.2.7 / §7.2.8 / §7.2.12）。
fn section_lints(section: &[(String, usize)], active: &[C], out: &mut Vec<Diagnostic>) {
    let map = SectionText::new(section);
    let body = map.joined.as_str();
    let mut found: Vec<Diagnostic> = Vec::new();
    if active.contains(&C::W097) {
        found.extend(sentence_end_repeat(body, 0));
    }
    if active.contains(&C::W099) {
        found.extend(adjacent_word_repeat(body, 0));
    }
    if active.contains(&C::W100) {
        found.extend(end_particle_repeat(body, 0));
    }
    if active.contains(&C::W101) {
        found.extend(start_word_repeat(body, 0));
    }
    if active.contains(&C::W102) {
        found.extend(repeated_symbol(C::W102, body, 0, "……", 3));
    }
    if active.contains(&C::W106) {
        found.extend(repeated_symbol(C::W106, body, 0, "——", 4));
    }
    for mut diagnostic in found {
        if let Some(range) = diagnostic.range.clone() {
            diagnostic.range = Some(map.span(range));
        }
        out.push(diagnostic);
    }
}

/// 場面（その場面内の全節: `(本文, 文書内 byte)`）を受け取って場面単位の 4 診断を出す。
fn scene_lints(scene: &[(String, usize)], active: &[C], out: &mut Vec<Diagnostic>) {
    eprintln!(
        "PROBE scene_lints: scene={:?} active={:?}",
        scene.iter().map(|(b, o)| (o, b)).collect::<Vec<_>>(),
        active
    );
    if scene.is_empty() {
        return;
    }
    if active.contains(&C::W098) {
        out.extend(scene_word_repeat(scene));
    }
    if active.contains(&C::W103) {
        out.extend(scene_hedge_overuse(scene));
    }
    if active.contains(&C::W104) {
        out.extend(scene_verb_stem_repeat(scene));
    }
    if active.contains(&C::W105) {
        out.extend(scene_exclamation_overuse(scene));
    }
}

/// W098: 場面内の同語反復（§7.2.4）。W087 と同じ語抽出を場面単位に適用する。
fn scene_word_repeat(scene: &[(String, usize)]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut counts: std::collections::BTreeMap<String, (usize, i64)> =
        std::collections::BTreeMap::new();
    for (body, doc_offset) in scene {
        for (pos, token) in token_positions(body) {
            if token.chars().count() < 2 {
                continue;
            }
            let entry = counts.entry(token.to_string()).or_insert((0, 0));
            entry.0 = doc_offset + pos;
            entry.1 += 1;
        }
    }
    for (body, _) in scene.iter().take(1) {
        eprintln!(
            "PROBE tok: {body:?} => {tokens:?}",
            tokens = token_positions(body)
        );
    }
    eprintln!("PROBE scene_word_repeat counts: {counts:?}");
    for (word, (last_pos, count)) in counts {
        if count >= 5 {
            out.push(diag(
                C::W098,
                last_pos..last_pos + word.len(),
                vec![Arg::text(word), Arg::number(count)],
            ));
        }
    }
    out
}

/// W103: やわらか・不確か表現の多用（§7.2.9）。
fn scene_hedge_overuse(scene: &[(String, usize)]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for word in HEDGE_WORDS {
        let mut last: Option<(usize, i64)> = None;
        for (body, doc_offset) in scene {
            let mut cursor = 0usize;
            while let Some(at) = body[cursor..].find(word) {
                let pos = doc_offset + cursor + at;
                let entry = last.get_or_insert((pos, 0));
                entry.0 = pos;
                entry.1 += 1;
                cursor += at + word.len();
            }
        }
        if let Some((pos, count)) = last {
            if count >= 3 {
                out.push(diag(
                    C::W103,
                    pos..pos + word.len(),
                    vec![Arg::text(*word), Arg::number(count)],
                ));
            }
        }
    }
    out
}

/// W104: 同一動詞語幹の反復（§7.2.10）。
fn scene_verb_stem_repeat(scene: &[(String, usize)]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut counts: std::collections::BTreeMap<String, (usize, i64)> =
        std::collections::BTreeMap::new();
    for (body, doc_offset) in scene {
        let tokens = token_positions(body);
        for (i, (pos, token)) in tokens.iter().enumerate() {
            if !is_word_token(token) || token.chars().count() < 2 {
                continue;
            }
            let Some((_, next)) = tokens[i + 1..].iter().find(|(_, t)| is_word_token(t)) else {
                continue;
            };
            if !VERB_SUFFIX_TOKENS.contains(next) {
                continue;
            }
            let entry = counts.entry(token.to_string()).or_insert((0, 0));
            entry.0 = doc_offset + pos;
            entry.1 += 1;
        }
    }
    for (stem, (last_pos, count)) in counts {
        if count >= 5 {
            out.push(diag(
                C::W104,
                last_pos..last_pos + stem.len(),
                vec![Arg::text(stem), Arg::number(count)],
            ));
        }
    }
    out
}

/// W105: 感嘆符の多用（§7.2.11）。
fn scene_exclamation_overuse(scene: &[(String, usize)]) -> Vec<Diagnostic> {
    let mut count = 0i64;
    let mut last_pos = 0usize;
    for (body, doc_offset) in scene {
        for (at, character) in body.char_indices() {
            if character == '！' || character == '!' {
                count += 1;
                last_pos = doc_offset + at;
            }
        }
    }
    if count < 5 {
        return Vec::new();
    }
    vec![diag(
        C::W105,
        last_pos..last_pos + 1,
        vec![Arg::number(count)],
    )]
}

fn sections_count(text: &str) -> usize {
    crate::core::features::sections::count_chars(text)
}

/// i18n: format-vocabulary
const SIGHT_WORDS: &[&str] = &["視覚", "視", "sight"];
