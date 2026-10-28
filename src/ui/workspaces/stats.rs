//! 字数の統計タブ。執筆統計・話者別・分布図はどれも `metrics` を使う（§10.9.5）。
use super::tabs::{label, State};
use crate::core::features::metrics;
use crate::ui::i18n::{self, Locale};
use egui::Ui;

fn bars(ui: &mut Ui, values: &[(String, f64)]) {
    let max = values.iter().map(|(_, n)| n.abs()).fold(1.0, f64::max);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(100.0), 100.0),
        egui::Sense::hover(),
    );
    let width = rect.width() / values.len().max(1) as f32;
    for (index, (label, value)) in values.iter().enumerate() {
        let height = (value.abs() / max) as f32 * 90.0;
        let bar = egui::Rect::from_min_size(
            egui::pos2(rect.left() + index as f32 * width, rect.bottom() - height),
            egui::vec2((width - 1.0).max(1.0), height),
        );
        ui.painter()
            .rect_filled(bar, 0.0, ui.visuals().selection.bg_fill);
        ui.interact(bar, ui.id().with(index), egui::Sense::hover())
            .on_hover_text(format!("{label}: {value:.1}"));
    }
}
pub(super) fn stats_ui(ui: &mut Ui, state: &State, locale: Locale) {
    let dates = metrics::recent_dates(state.stamp.get(..10).unwrap_or(""), 400);
    let stats = &state.stats;
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.label(i18n::format(
            locale,
            "stage.stats_averages",
            &[
                &format!(
                    "{:.0}",
                    stats.average(&dates.iter().take(7).cloned().collect::<Vec<_>>())
                ),
                &format!(
                    "{:.0}",
                    stats.average(&dates.iter().take(30).cloned().collect::<Vec<_>>())
                ),
                &stats.streak(&dates).to_string(),
            ],
        ));
        ui.heading(label(locale, "stage.daily"));
        let daily = dates
            .iter()
            .take(90)
            .rev()
            .map(|d| (d.clone(), stats.days.get(d).map_or(0, |d| d.chars) as f64))
            .collect::<Vec<_>>();
        bars(ui, &daily);
        ui.heading(label(locale, "stage.weekdays"));
        let mut sums = [0i64; 7];
        let mut counts = [0usize; 7];
        for date in dates.iter().take(90) {
            if let Some(day) = metrics::weekday(date) {
                sums[day] += stats.days.get(date).map_or(0, |d| d.chars);
                counts[day] += 1;
            }
        }
        bars(
            ui,
            &(0..7)
                .map(|i| (i.to_string(), sums[i] as f64 / counts[i].max(1) as f64))
                .collect::<Vec<_>>(),
        );
        ui.heading(label(locale, "stage.hours"));
        let mut hours = [0; 24];
        for day in stats.days.values() {
            for hour in &day.hours {
                if *hour < 24 {
                    hours[*hour] += 1;
                }
            }
        }
        bars(
            ui,
            &hours
                .iter()
                .enumerate()
                .map(|(h, n)| (format!("{h:02}:00"), *n as f64))
                .collect::<Vec<_>>(),
        );
        for (date, day) in stats.days.iter().rev() {
            ui.label(format!(
                "{date}   {:+}   {} {}   ×{}",
                day.chars,
                day.active_seconds / 60,
                label(locale, "stage.minutes"),
                day.sessions
            ));
        }
    });
}
