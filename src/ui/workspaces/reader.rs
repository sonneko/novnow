//! 読者モード（縦書きのプレビュー）の画面（§10.7・§17.3）。
use super::tabs::{label, State};
use crate::core::features::metadata;
use crate::core::id::SceneId;
use crate::ui::{i18n::Locale, workbench::Workbench};
use egui::Ui;

pub(super) struct ReaderCache {
    key: (String, u64, usize, usize, bool, u32),
    pages: Vec<crate::core::reader::Page>,
    page_scenes: Vec<Option<SceneId>>,
    section_pages: Vec<(SceneId, usize)>,
    chapter_title: String,
}

#[allow(clippy::too_many_arguments)]
fn ensure_cache(
    ui: &Ui,
    state: &mut State,
    bench: &Workbench,
    episodes: &[((u16, u16), String)],
    episode_index: usize,
    columns: usize,
    size: f32,
    font: &egui::FontId,
    base_metrics: &crate::core::reader::Metrics,
) {
    let ((chapter, _), path) = &episodes[episode_index];
    let options = crate::core::reader::Options {
        rows: state.reader_rows,
        columns,
        vertical: state.reader_vertical,
    };
    use std::hash::{Hash, Hasher};
    let mut fingerprint = std::collections::hash_map::DefaultHasher::new();
    bench.project().text(path).hash(&mut fingerprint);
    bench
        .project()
        .text("plots/all.cnov")
        .hash(&mut fingerprint);
    bench
        .project()
        .text(&bench.project().chapter_plot_path(*chapter))
        .hash(&mut fingerprint);
    let cache_key = (
        path.clone(),
        fingerprint.finish(),
        state.reader_rows,
        columns,
        state.reader_vertical,
        size.to_bits(),
    );
    if state
        .reader_cache
        .as_ref()
        .is_some_and(|cache| cache.key == cache_key)
    {
        return;
    }

    let source_sections: Vec<_> = bench
        .project()
        .sections()
        .iter()
        .filter(|s| s.path == *path)
        .cloned()
        .collect();
    let fragment = crate::core::export::fragment_with_scenes(
        bench.project().text(path),
        &source_sections,
        true,
    );
    let chapter_title = crate::core::features::plots::outline_blocks(bench.project(), None)
        .into_iter()
        .find(|b| b.id.chapter == *chapter && b.id.episode == 0)
        .map_or_else(|| format!("{chapter:02}"), |b| b.title);
    let blurb = metadata::chapter_blurb(bench.project(), *chapter);
    let cover_text = if blurb.trim().is_empty() {
        chapter_title.clone()
    } else {
        format!("{chapter_title}\n\n{}", blurb.trim())
    };

    let mut measured_metrics = base_metrics.clone();
    ui.fonts(|fonts| {
        // 括弧の互換形は、使用フォントに収録されている場合だけ組版で使う（§17.3.3）。
        for &(_, form) in crate::core::reader::VERTICAL_FORMS {
            if fonts.has_glyph(font, form) {
                measured_metrics.vertical_forms.insert(form);
            }
        }
        for ch in fragment.text.chars().chain(cover_text.chars()) {
            measured_metrics
                .glyph_advances
                .entry(ch)
                .or_insert_with(|| fonts.glyph_width(font, ch).max(size * 0.5) * 1.15);
        }
    });

    let laid = crate::core::reader::layout_with_scenes(&fragment, options, &measured_metrics);
    let mut pages = laid.pages;
    let mut page_scenes = laid.page_scenes;
    let mut section_pages = laid.section_pages;

    // §17.3.2: 章の先頭に、章題と blurb だけの表紙ページを必ず 1 枚挟む。
    if episodes.iter().position(|((c, _), _)| c == chapter) == Some(episode_index) {
        let mut cover_pages = crate::core::reader::layout(&cover_text, options, &measured_metrics);
        let had_overflow = cover_pages.len() > 1;
        let mut single_cover = cover_pages.drain(..1).next().unwrap_or_default();
        if had_overflow {
            if let Some(last) = single_cover.glyphs.last_mut() {
                last.ch = '…';
                last.text = "…".to_owned();
                last.ruby.clear();
                last.compact = false;
                last.punctuation = false;
                last.rotate =
                    state.reader_vertical && !measured_metrics.vertical_forms.contains(&'…');
            }
        }
        pages.insert(0, single_cover);
        page_scenes.insert(0, source_sections.first().map(|s| s.id));
        for (_, page_idx) in &mut section_pages {
            *page_idx += 1;
        }
    }
    for &(sec_id, sec_page) in section_pages.iter().rev() {
        if let Some(slot) = page_scenes.get_mut(sec_page) {
            *slot = Some(sec_id);
        }
    }

    state.reader_cache = Some(ReaderCache {
        key: cache_key,
        pages,
        page_scenes,
        section_pages,
        chapter_title,
    });
}

fn target_page_and_scene(
    section_pages: &[(SceneId, usize)],
    target: SceneId,
) -> (usize, Option<SceneId>) {
    if target.scene == 0 && target.section == 0 {
        return (0, section_pages.first().map(|&(id, _)| id));
    }
    if let Some(&(id, page)) = section_pages.iter().find(|(id, _)| *id == target) {
        return (page, Some(id));
    }
    if target.section == 0 {
        if let Some(&(id, page)) = section_pages
            .iter()
            .find(|(id, _)| id.scene == target.scene)
        {
            return (page, Some(id));
        }
    }
    section_pages
        .iter()
        .find(|(id, _)| *id >= target)
        .or_else(|| section_pages.last())
        .map_or((0, None), |&(id, page)| (page, Some(id)))
}

pub(super) fn reader(
    ui: &mut Ui,
    state: &mut State,
    bench: &mut Workbench,
    locale: Locale,
    jump: &mut Option<(String, usize)>,
) {
    if !state.reader_loaded {
        state.reader_raw = serde_json::from_str(bench.project().text(".novnow/reader.json"))
            .unwrap_or_else(|_| serde_json::json!({}));
        let raw = &state.reader_raw;
        let defaults = bench.project().settings().unknown.get("reader");
        let default_auto = defaults
            .and_then(|d| d.get("columns"))
            .and_then(serde_json::Value::as_str)
            == Some("auto");
        state.reader_auto_columns = raw.get("columns").is_none() && default_auto;
        state.reader_columns = raw
            .get("columns")
            .and_then(serde_json::Value::as_u64)
            .or_else(|| {
                defaults
                    .and_then(|d| d.get("columns"))
                    .and_then(serde_json::Value::as_u64)
            })
            .unwrap_or(18)
            .clamp(5, 40) as usize;
        state.reader_rows = defaults
            .and_then(|d| d.get("chars_per_column"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(42)
            .clamp(10, 80) as usize;
        state.reader_font = raw
            .get("font_size")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(20.0)
            .clamp(12.0, 36.0) as f32;
        state.reader_vertical = raw
            .get("vertical")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        state.reader_ruby = raw
            .get("ruby")
            .and_then(serde_json::Value::as_bool)
            .or_else(|| {
                defaults
                    .and_then(|d| d.get("ruby"))
                    .and_then(serde_json::Value::as_bool)
            })
            .unwrap_or(true);
        state.reader_dark = raw
            .get("dark")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        state.reader_episode = raw
            .get("episode")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        state.reader_page = raw
            .get("page")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        state.reader_marks = serde_json::from_value(
            raw.get("bookmarks")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )
        .unwrap_or_default();
        state.reader_loaded = true;
    }

    let all = bench.project().sections();
    let mut episodes = std::collections::BTreeMap::new();
    for section in all.iter().filter(|s| s.id.chapter > 0 && s.id.episode > 0) {
        episodes
            .entry((section.id.chapter, section.id.episode))
            .or_insert_with(|| section.path.clone());
    }
    let episodes: Vec<_> = episodes.into_iter().collect();
    if episodes.is_empty() {
        return;
    }

    state.reader_episode = state.reader_episode.min(episodes.len() - 1);
    let old = (
        state.reader_episode,
        state.reader_page,
        state.reader_font.to_bits(),
        state.reader_columns,
        state.reader_vertical,
        state.reader_ruby,
        state.reader_dark,
    );

    let size = state.reader_font;
    let font = egui::FontId::proportional(size);
    let measured = ui.fonts(|f| f.glyph_width(&font, '漢')).max(size);
    let base_metrics = crate::core::reader::Metrics {
        advance: measured * 1.15,
        line_advance: measured * 1.7,
        vertical_forms: Default::default(),
        glyph_advances: Default::default(),
    };
    let effective_columns = if state.reader_auto_columns && state.reader_vertical {
        let usable = (ui.available_width() - size * 4.0).max(base_metrics.line_advance);
        ((usable / base_metrics.line_advance).floor() as usize).clamp(5, 40)
    } else {
        state.reader_columns
    };

    if let Some(id) = state.reader_jump.take() {
        let found = episodes
            .iter()
            .position(|((c, e), _)| *c == id.chapter && (id.episode == 0 || *e == id.episode))
            .or_else(|| episodes.iter().position(|((c, _), _)| *c >= id.chapter));
        if let Some(index) = found {
            state.reader_episode = index;
            ensure_cache(
                ui,
                state,
                bench,
                &episodes,
                state.reader_episode,
                effective_columns,
                size,
                &font,
                &base_metrics,
            );
            if let Some(cache) = &state.reader_cache {
                let (target_page, pinned_sec) = target_page_and_scene(&cache.section_pages, id);
                state.reader_page = target_page;
                state.reader_pinned_section =
                    pinned_sec.map(|sec| (state.reader_episode, target_page, sec));
            } else {
                state.reader_page = 0;
                state.reader_pinned_section = None;
            }
            state.reader_loaded = true;
        }
    }

    ensure_cache(
        ui,
        state,
        bench,
        &episodes,
        state.reader_episode,
        effective_columns,
        size,
        &font,
        &base_metrics,
    );
    let page_count = state
        .reader_cache
        .as_ref()
        .map_or(1, |cache| cache.pages.len().max(1));
    state.reader_page = state.reader_page.min(page_count - 1);

    let consume = |key| ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
    let mut step_next =
        consume(egui::Key::ArrowLeft) || consume(egui::Key::Space) || consume(egui::Key::PageDown);
    let mut step_prev = consume(egui::Key::ArrowRight) || consume(egui::Key::PageUp);
    if consume(egui::Key::Home) {
        state.reader_episode = 0;
        state.reader_page = 0;
    }
    if consume(egui::Key::End) {
        state.reader_episode = episodes.len() - 1;
        state.reader_page = usize::MAX;
    }
    if consume(egui::Key::R) {
        state.reader_ruby = !state.reader_ruby;
    }
    if consume(egui::Key::T) {
        state.reader_dark = !state.reader_dark;
    }
    if consume(egui::Key::Plus) || consume(egui::Key::Equals) {
        state.reader_font = (state.reader_font + 1.0).min(36.0);
    }
    if consume(egui::Key::Minus) {
        state.reader_font = (state.reader_font - 1.0).max(12.0);
    }
    if consume(egui::Key::Num0) {
        state.reader_font = 20.0;
    }
    let exit = consume(egui::Key::Escape);
    let mut bookmark = consume(egui::Key::B);
    let mut edit = exit;

    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(label(locale, "stage.reader")).size(11.0));
        if ui.button("←").clicked() {
            step_next = true;
        }
        if ui.button("→").clicked() {
            step_prev = true;
        }
        if ui.button(label(locale, "stage.previous_episode")).clicked() {
            state.reader_episode = state.reader_episode.saturating_sub(1);
            state.reader_page = 0;
        }
        if ui.button(label(locale, "stage.next_episode")).clicked() {
            state.reader_episode = (state.reader_episode + 1).min(episodes.len() - 1);
            state.reader_page = 0;
        }
        ui.add(
            egui::Slider::new(&mut state.reader_font, 12.0..=36.0)
                .text(label(locale, "stage.font")),
        );
        let prev_cols = state.reader_columns;
        if ui
            .add(
                egui::Slider::new(&mut state.reader_columns, 5..=40)
                    .text(label(locale, "stage.columns")),
            )
            .changed()
            && state.reader_columns != prev_cols
        {
            state.reader_auto_columns = false;
        }
        ui.checkbox(&mut state.reader_vertical, label(locale, "stage.vertical"));
        ui.checkbox(&mut state.reader_ruby, label(locale, "stage.ruby"));
        ui.checkbox(&mut state.reader_dark, label(locale, "stage.dark"));
        bookmark |= ui.button(label(locale, "stage.bookmark")).clicked();
        edit |= ui.button(label(locale, "stage.edit")).clicked();
    });

    if step_next {
        if state.reader_page + 1 < page_count {
            state.reader_page += 1;
        } else if state.reader_episode + 1 < episodes.len() {
            state.reader_episode += 1;
            state.reader_page = 0;
        }
    }
    if step_prev {
        if state.reader_page > 0 {
            state.reader_page -= 1;
        } else if state.reader_episode > 0 {
            state.reader_episode -= 1;
            state.reader_page = usize::MAX;
        }
    }

    let size = state.reader_font;
    let font = egui::FontId::proportional(size);
    let measured = ui.fonts(|f| f.glyph_width(&font, '漢')).max(size);
    let metrics = crate::core::reader::Metrics {
        advance: measured * 1.15,
        line_advance: measured * 1.7,
        vertical_forms: Default::default(),
        glyph_advances: Default::default(),
    };
    let effective_columns = if state.reader_auto_columns && state.reader_vertical {
        let usable = (ui.available_width() - size * 4.0).max(metrics.line_advance);
        ((usable / metrics.line_advance).floor() as usize).clamp(5, 40)
    } else {
        state.reader_columns
    };

    ensure_cache(
        ui,
        state,
        bench,
        &episodes,
        state.reader_episode,
        effective_columns,
        size,
        &font,
        &metrics,
    );

    let (_, path) = &episodes[state.reader_episode];
    let total_pages = state.reader_cache.as_ref().unwrap().pages.len().max(1);
    state.reader_page = state.reader_page.min(total_pages - 1);

    let section_pages = state.reader_cache.as_ref().unwrap().section_pages.clone();
    let digits = bench.project().digits();
    ui.collapsing(label(locale, "stage.toc"), |ui| {
        for &(sec_id, sec_page) in &section_pages {
            let link_label = format!("{}  p.{}", sec_id.display(digits), sec_page + 1);
            let resp = ui.link(link_label);
            let enter = resp.has_focus()
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            if resp.clicked() || enter {
                let target_page = sec_page.min(total_pages - 1);
                state.reader_page = target_page;
                state.reader_pinned_section = Some((state.reader_episode, target_page, sec_id));
            }
        }
    });

    if bookmark {
        let mark = (path.clone(), state.reader_page);
        if let Some(index) = state.reader_marks.iter().position(|m| *m == mark) {
            state.reader_marks.remove(index);
        } else {
            state.reader_marks.push(mark);
        }
        state.reader_dirty = true;
    }
    if !state.reader_marks.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (marked_path, page) in state.reader_marks.clone() {
                let resp = ui.link(format!("🔖 {marked_path}:{}", page + 1));
                let enter = resp.has_focus()
                    && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                if resp.clicked() || enter {
                    if let Some(index) = episodes.iter().position(|(_, p)| *p == marked_path) {
                        state.reader_episode = index;
                        state.reader_page = page;
                    }
                }
            }
        });
    }

    ensure_cache(
        ui,
        state,
        bench,
        &episodes,
        state.reader_episode,
        effective_columns,
        size,
        &font,
        &metrics,
    );
    let (_, path) = &episodes[state.reader_episode];
    let cache = state.reader_cache.as_ref().unwrap();
    let pages = &cache.pages;
    let page = state.reader_page.min(pages.len() - 1);
    state.reader_page = page;

    let fg = if state.reader_dark {
        egui::Color32::from_rgb(225, 225, 220)
    } else {
        egui::Color32::from_rgb(35, 35, 30)
    };
    let fg_faint = if state.reader_dark {
        egui::Color32::from_rgb(130, 132, 128)
    } else {
        egui::Color32::from_rgb(138, 134, 126)
    };
    let bg = if state.reader_dark {
        egui::Color32::from_rgb(27, 29, 32)
    } else {
        egui::Color32::from_rgb(250, 248, 239)
    };

    // §17.3.2: 余白は上下 1 字分・左右 2 字分、柱（章題）とノンブルを fg.faint 9px で表示。
    let margin_x = size * 2.0;
    let margin_y = size * 1.0;
    let folio_text = format!("{} / {}", page + 1, pages.len());
    let chapter_title = cache.chapter_title.clone();

    egui::ScrollArea::both().show(ui, |ui| {
        let width = effective_columns as f32
            * if state.reader_vertical {
                metrics.line_advance
            } else {
                metrics.advance
            };
        let height = state.reader_rows as f32
            * if state.reader_vertical {
                metrics.advance
            } else {
                metrics.line_advance
            };
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(width + margin_x * 2.0, height + margin_y * 2.0),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(rect, 0.0, bg);
        let header_font = egui::FontId::proportional(9.0);
        ui.painter().text(
            egui::pos2(rect.center().x, rect.top() + margin_y * 0.45),
            egui::Align2::CENTER_CENTER,
            &chapter_title,
            header_font.clone(),
            fg_faint,
        );
        ui.painter().text(
            egui::pos2(rect.center().x, rect.bottom() - margin_y * 0.45),
            egui::Align2::CENTER_CENTER,
            &folio_text,
            header_font,
            fg_faint,
        );

        let content_origin = rect.min + egui::vec2(margin_x, margin_y);
        for glyph in &pages[page].glyphs {
            let pos = content_origin + egui::vec2(glyph.x, glyph.y);
            let glyph_size = if glyph.compact { size * 0.65 } else { size };
            let galley = ui.painter().layout_no_wrap(
                glyph.text.clone(),
                egui::FontId::proportional(glyph_size),
                fg,
            );
            let rotated = if glyph.rotate {
                pos + egui::vec2(size, 0.0)
            } else {
                pos
            };
            ui.painter().add(
                egui::epaint::TextShape::new(rotated, galley, fg).with_angle(if glyph.rotate {
                    std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                }),
            );
            if state.reader_ruby && !glyph.ruby.is_empty() {
                for (index, ch) in glyph.ruby.chars().enumerate() {
                    let offset = if state.reader_vertical {
                        egui::vec2(size, index as f32 * size * 0.5)
                    } else {
                        egui::vec2(index as f32 * size * 0.5, -size * 0.5)
                    };
                    ui.painter().text(
                        pos + offset,
                        egui::Align2::LEFT_TOP,
                        ch,
                        egui::FontId::proportional(size * 0.5),
                        fg,
                    );
                }
            }
        }
    });

    // 現在表示中のページに対応する正確な SceneId と本文先頭 byte を保持する（§17.3.4 Esc / Alt+1）。
    let pinned = match state.reader_pinned_section {
        Some((ep, pg, sec)) if ep == state.reader_episode && pg == page => Some(sec),
        _ => {
            state.reader_pinned_section = None;
            None
        }
    };
    let current_scene = pinned
        .or_else(|| {
            state
                .reader_cache
                .as_ref()
                .and_then(|c| c.page_scenes.get(page).copied().flatten())
        })
        .or_else(|| all.iter().find(|s| s.path == *path).map(|s| s.id));
    state.reader_section = current_scene;
    let starts = crate::core::features::project::line_starts_of(bench.project().text(path));
    let at = current_scene
        .and_then(|id| all.iter().find(|s| s.path == *path && s.id == id))
        .and_then(|s| starts.get(s.lines.start).copied())
        .unwrap_or(0);
    state.reader_position = Some((path.clone(), at));

    if edit {
        *jump = state.reader_position.clone();
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("reader-exit"), true));
    }
    if old
        != (
            state.reader_episode,
            state.reader_page,
            state.reader_font.to_bits(),
            state.reader_columns,
            state.reader_vertical,
            state.reader_ruby,
            state.reader_dark,
        )
    {
        state.reader_dirty = true;
    }
    if state.reader_dirty {
        if !state.reader_raw.is_object() {
            state.reader_raw = serde_json::json!({});
        }
        let cols_val = if state.reader_auto_columns {
            serde_json::Value::Null
        } else {
            serde_json::json!(state.reader_columns)
        };
        for (key, value) in [
            ("episode", serde_json::json!(state.reader_episode)),
            ("page", serde_json::json!(state.reader_page)),
            ("font_size", serde_json::json!(state.reader_font)),
            ("vertical", serde_json::json!(state.reader_vertical)),
            ("ruby", serde_json::json!(state.reader_ruby)),
            ("dark", serde_json::json!(state.reader_dark)),
            ("bookmarks", serde_json::json!(state.reader_marks)),
        ] {
            state.reader_raw[key] = value;
        }
        if cols_val.is_null() {
            if let Some(obj) = state.reader_raw.as_object_mut() {
                obj.remove("columns");
            }
        } else {
            state.reader_raw["columns"] = cols_val;
        }
        bench.replace_document(
            ".novnow/reader.json",
            serde_json::to_string_pretty(&state.reader_raw).unwrap() + "\n",
            0.0,
        );
        state.reader_dirty = false;
    }
}
