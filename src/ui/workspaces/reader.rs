//! Reader mode (vertical preview) workspace UI (design doc `workflows` §17.3).
use super::tabs::{label, State};
use crate::core::features::{metadata, sections};
use crate::ui::{i18n::Locale, workbench::Workbench};
use egui::Ui;

pub(super) struct ReaderCache {
    key: (String, u64, usize, usize, bool, u32),
    pages: Vec<crate::core::reader::Page>,
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
    let all = sections::all(bench.project());
    let mut episodes = std::collections::BTreeMap::new();
    for section in all.iter().filter(|s| s.id.chapter > 0 && s.id.episode > 0) {
        episodes
            .entry((section.id.chapter, section.id.episode))
            .or_insert(section.path.clone());
    }
    let episodes: Vec<_> = episodes.into_iter().collect();
    if episodes.is_empty() {
        return;
    }
    if let Some(id) = state.reader_jump.take() {
        if let Some(index) = episodes
            .iter()
            .position(|((c, e), _)| *c == id.chapter && *e == id.episode)
        {
            state.reader_episode = index;
            state.reader_page = 0;
            state.reader_loaded = true;
        }
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
    let consume = |key| ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
    if consume(egui::Key::ArrowLeft) || consume(egui::Key::Space) || consume(egui::Key::PageDown) {
        state.reader_page = state.reader_page.saturating_add(1);
    }
    if consume(egui::Key::ArrowRight) || consume(egui::Key::PageUp) {
        state.reader_page = state.reader_page.saturating_sub(1);
    }
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
        if ui.button("←").clicked() {
            state.reader_page = state.reader_page.saturating_add(1);
        }
        if ui.button("→").clicked() {
            state.reader_page = state.reader_page.saturating_sub(1);
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
        ui.add(
            egui::Slider::new(&mut state.reader_columns, 5..=40)
                .text(label(locale, "stage.columns")),
        );
        ui.checkbox(&mut state.reader_vertical, label(locale, "stage.vertical"));
        ui.checkbox(&mut state.reader_ruby, label(locale, "stage.ruby"));
        ui.checkbox(&mut state.reader_dark, label(locale, "stage.dark"));
        bookmark |= ui.button(label(locale, "stage.bookmark")).clicked();
        edit |= ui.button(label(locale, "stage.edit")).clicked();
    });
    let ((chapter, episode), path) = &episodes[state.reader_episode];
    ui.heading(format!("{chapter:02}–{episode:02}"));
    let size = state.reader_font;
    let font = egui::FontId::proportional(size);
    let measured = ui.fonts(|f| f.glyph_width(&font, '漢')).max(size);
    let metrics = crate::core::reader::Metrics {
        advance: measured * 1.15,
        line_advance: measured * 1.7,
        vertical_forms: Default::default(),
        glyph_advances: Default::default(),
    };
    let options = crate::core::reader::Options {
        rows: state.reader_rows,
        columns: state.reader_columns,
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
        state.reader_columns,
        state.reader_vertical,
        size.to_bits(),
    );
    if state
        .reader_cache
        .as_ref()
        .is_none_or(|cache| cache.key != cache_key)
    {
        let text = crate::core::export::fragment(bench.project().text(path), true);
        let mut measured_metrics = metrics.clone();
        ui.fonts(|fonts| {
            // 括弧の互換形は、使用フォントに収録されている場合だけ組版で使う（§17.3.3）。
            for &(_, form) in crate::core::reader::VERTICAL_FORMS {
                if fonts.has_glyph(&font, form) {
                    measured_metrics.vertical_forms.insert(form);
                }
            }
            for ch in text.chars() {
                measured_metrics
                    .glyph_advances
                    .entry(ch)
                    .or_insert_with(|| fonts.glyph_width(&font, ch).max(size * 0.5) * 1.15);
            }
        });
        let mut pages = crate::core::reader::layout(&text, options, &measured_metrics);
        if episodes.iter().position(|((c, _), _)| c == chapter) == Some(state.reader_episode) {
            let title = crate::core::features::plots::outline_blocks(bench.project(), None)
                .into_iter()
                .find(|b| b.id.chapter == *chapter && b.id.episode == 0)
                .map_or_else(|| format!("{chapter:02}"), |b| b.title);
            let blurb = metadata::chapter_blurb(bench.project(), *chapter);
            let cover = crate::core::reader::layout(
                &format!("{title}\n\n{blurb}"),
                options,
                &measured_metrics,
            );
            pages.splice(0..0, cover);
        }

        state.reader_cache = Some(ReaderCache {
            key: cache_key,
            pages,
        });
    }
    let pages = &state.reader_cache.as_ref().unwrap().pages;
    state.reader_page = state.reader_page.min(pages.len() - 1);
    ui.label(format!("{} / {}", state.reader_page + 1, pages.len()));
    let starts = crate::core::features::project::line_starts_of(bench.project().text(path));
    // The same prefix layout maps source sections to pages for the TOC and edit return.
    let source_sections: Vec<_> = all.iter().filter(|s| s.path == *path).collect();
    ui.collapsing(label(locale, "stage.toc"), |ui| {
        for section in &source_sections {
            if ui
                .link(section.id.display(bench.project().digits()))
                .clicked()
            {
                let at = starts.get(section.lines.start).copied().unwrap_or(0);
                let prefix = crate::core::export::fragment(&bench.project().text(path)[..at], true);
                state.reader_page = crate::core::reader::layout(&prefix, options, &metrics)
                    .len()
                    .saturating_sub(1);
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
    ui.horizontal_wrapped(|ui| {
        for (marked_path, page) in state.reader_marks.clone() {
            if ui.link(format!("🔖 {marked_path}:{}", page + 1)).clicked() {
                if let Some(index) = episodes.iter().position(|(_, path)| *path == marked_path) {
                    state.reader_episode = index;
                    state.reader_page = page;
                }
            }
        }
    });
    let page = state.reader_page.min(pages.len() - 1);
    let fg = if state.reader_dark {
        egui::Color32::from_rgb(225, 225, 220)
    } else {
        egui::Color32::from_rgb(35, 35, 30)
    };
    let bg = if state.reader_dark {
        egui::Color32::from_rgb(27, 29, 32)
    } else {
        egui::Color32::from_rgb(250, 248, 239)
    };
    egui::ScrollArea::both().show(ui, |ui| {
        let width = state.reader_columns as f32
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
            egui::vec2(width + size, height + size),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(rect, 0.0, bg);
        for glyph in &pages[page].glyphs {
            let pos = rect.min + egui::vec2(glyph.x + size * 0.25, glyph.y + size * 0.25);
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
    let first = pages[page]
        .glyphs
        .iter()
        .take(6)
        .map(|g| {
            if g.compact {
                g.text.clone()
            } else {
                g.ch.to_string()
            }
        })
        .collect::<String>();
    let at = bench.project().text(path).find(&first).unwrap_or(0);
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
        for (key, value) in [
            ("episode", serde_json::json!(state.reader_episode)),
            ("page", serde_json::json!(state.reader_page)),
            ("font_size", serde_json::json!(state.reader_font)),
            ("columns", serde_json::json!(state.reader_columns)),
            ("vertical", serde_json::json!(state.reader_vertical)),
            ("ruby", serde_json::json!(state.reader_ruby)),
            ("dark", serde_json::json!(state.reader_dark)),
            ("bookmarks", serde_json::json!(state.reader_marks)),
        ] {
            state.reader_raw[key] = value;
        }
        bench.replace_document(
            ".novnow/reader.json",
            serde_json::to_string_pretty(&state.reader_raw).unwrap() + "\n",
            0.0,
        );
        state.reader_dirty = false;
    }
}
