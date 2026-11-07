//! History and ranking lists. They replace the file catalog, side by side.
//! Rows come from the metadata worker, which also checks that each file is
//! still present in the workspace.

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, Local, Utc};
use egui::*;
use egui_extras::{Size, StripBuilder};

use crate::playmetadata::RankingOrder;
use crate::playmetadata_manager::HistoryFile;

use super::VirtualBookApp;

/// How many rows to ask from the database for each list.
pub(crate) const HISTORY_LIST_LIMIT: usize = 40;

/// Left side of the playlist screen: the folder tree, or play history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) enum CatalogPane {
    #[default]
    Files,
    History,
}

/// Catalog / history switch, kept above whichever list is showing.
pub(crate) fn catalog_pane_switch(app: &mut VirtualBookApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui
            .selectable_label(
                app.catalog_pane == CatalogPane::Files,
                format!("{} {}", egui_phosphor::regular::FOLDER, app.i18n.catalog),
            )
            .clicked()
        {
            app.catalog_pane = CatalogPane::Files;
        }
        if ui
            .selectable_label(
                app.catalog_pane == CatalogPane::History,
                format!(
                    "{} {}",
                    egui_phosphor::regular::CLOCK_COUNTER_CLOCKWISE,
                    app.i18n.history
                ),
            )
            .clicked()
            && app.catalog_pane != CatalogPane::History
        {
            app.catalog_pane = CatalogPane::History;
            app.current_typed_no.clear();
            app.request_history_lists();
        }
    });
}

/// Recent plays and the ranking, side by side, in place of the file tree.
pub(crate) fn ui_side_by_side(app: &mut VirtualBookApp, ui: &mut Ui) {
    StripBuilder::new(ui)
        .size(Size::remainder())
        .size(Size::exact(12.0))
        .size(Size::remainder())
        .horizontal(|mut strip| {
            strip.cell(|ui| {
                ui_recent_list(app, ui);
            });
            strip.cell(|ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.separator();
                });
            });
            strip.cell(|ui| {
                ui_ranking_panel(app, ui);
            });
        });
}

/// Recent plays, in the space the file tree normally uses.
pub(crate) fn ui_recent_list(app: &mut VirtualBookApp, ui: &mut Ui) {
    show_history_section(
        app,
        ui,
        &format!(
            "{} {}",
            egui_phosphor::regular::CLOCK_COUNTER_CLOCKWISE,
            app.i18n.recent_played
        ),
        true,
    );
}

/// Star ranking, in the space the filter pad normally uses.
pub(crate) fn ui_ranking_panel(app: &mut VirtualBookApp, ui: &mut Ui) {
    show_history_section(
        app,
        ui,
        &format!("{} {}", egui_phosphor::regular::TROPHY, app.i18n.ranking),
        false,
    );
}

fn show_history_section(app: &mut VirtualBookApp, ui: &mut Ui, title: &str, recent: bool) {
    let has_folder = app.file_store_path.is_some();
    let entries = if recent {
        app.history_recent.clone()
    } else {
        app.history_ranked.clone()
    };

    ui.label(
        RichText::new(title)
            .heading()
            .color(ui.visuals().text_color()),
    );
    if !recent {
        ranking_sort_switch(app, ui);
    }
    ui.separator();

    let mut chosen: Option<PathBuf> = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| match (&entries, has_folder) {
            (_, false) => {
                ui.label(&app.i18n.history_needs_folder);
            }
            (None, true) => {
                ui.label(&app.i18n.history_loading);
            }
            (Some(files), true) if files.is_empty() => {
                ui.label(&app.i18n.history_empty);
            }
            (Some(files), true) => {
                for file in files {
                    if render_history_row(ui, file, app) {
                        chosen = Some(file.absolute_path.clone());
                    }
                }
            }
        });

    if let Some(path) = chosen {
        add_history_file(app, &path);
    }
}

fn ranking_sort_switch(app: &mut VirtualBookApp, ui: &mut Ui) {
    let current = app.ranking_order;
    let stars = format!("{} {}", egui_phosphor::fill::STAR, app.i18n.sort_stars);
    let plays = format!(
        "{} {}",
        egui_phosphor::regular::MUSIC_NOTES,
        app.i18n.sort_plays
    );
    let both = app.i18n.sort_both.clone();
    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(current == RankingOrder::Stars, stars)
            .clicked()
        {
            choose_ranking_order(app, RankingOrder::Stars);
        }
        if ui
            .selectable_label(current == RankingOrder::Plays, plays)
            .clicked()
        {
            choose_ranking_order(app, RankingOrder::Plays);
        }
        if ui
            .selectable_label(current == RankingOrder::StarsThenPlays, both)
            .clicked()
        {
            choose_ranking_order(app, RankingOrder::StarsThenPlays);
        }
    });
}

fn choose_ranking_order(app: &mut VirtualBookApp, order: RankingOrder) {
    if app.ranking_order == order {
        return;
    }
    app.ranking_order = order;
    app.request_history_lists();
}

fn render_history_row(ui: &mut Ui, file: &HistoryFile, app: &VirtualBookApp) -> bool {
    let width = ui.available_width().max(80.0);
    let name_chars = ((width / 9.0) as usize).clamp(12, 48);
    let name = truncate_name(&file_label(&file.relative_path), name_chars);
    let exact = exact_play_time(file.latest_play_time);
    let when = relative_play_time(
        file.latest_play_time,
        Utc::now(),
        local_offset(),
        &phrases(app),
    );
    let exact_hover = format!("{}: {exact}", app.i18n.last_played);
    let mut add = false;

    ui.horizontal(|ui| {
        let button = Button::new(egui_phosphor::regular::PLUS).min_size(vec2(40.0, 40.0));
        let response = ui
            .add_enabled(file.exists, button)
            .on_hover_text_at_pointer(if file.exists {
                app.i18n.add_to_playlist.as_str()
            } else {
                app.i18n.file_missing.as_str()
            });
        if response.clicked() {
            add = true;
        }

        ui.vertical(|ui| {
            // `.strong()` uses the active-widget color, which is white in the light theme.
            let text_color = ui.visuals().text_color();
            let name_color = if file.exists {
                text_color
            } else {
                text_color.gamma_multiply(0.55)
            };
            ui.label(RichText::new(name).color(name_color))
                .on_hover_text_at_pointer(&file.relative_path);

            ui.horizontal(|ui| {
                if file.star_count > 0 {
                    let star_color = if ui.visuals().dark_mode {
                        Color32::from_rgb(255, 215, 0)
                    } else {
                        Color32::from_rgb(146, 96, 0)
                    };
                    ui.label(
                        RichText::new(format!("{} {}", egui_phosphor::fill::STAR, file.star_count))
                            .color(star_color)
                            .small(),
                    )
                    .on_hover_text_at_pointer(&app.i18n.star_count_tooltip);
                }
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        egui_phosphor::regular::MUSIC_NOTES,
                        file.play_count
                    ))
                    .small()
                    .color(text_color),
                )
                .on_hover_text_at_pointer(&app.i18n.play_count_tooltip);
                ui.label(RichText::new(when).small().color(text_color))
                    .on_hover_text_at_pointer(&exact_hover);
                if !file.exists {
                    ui.label(
                        RichText::new(egui_phosphor::regular::FILE_X)
                            .color(ui.style().visuals.warn_fg_color),
                    )
                    .on_hover_text_at_pointer(&app.i18n.file_missing);
                }
            });
        });
    });
    ui.separator();
    add
}

fn add_history_file(app: &mut VirtualBookApp, path: &Path) {
    if !path.is_file() {
        mark_missing(&mut app.history_recent, path);
        mark_missing(&mut app.history_ranked, path);
        return;
    }
    let path_buf = path.to_path_buf();
    let was_empty = {
        let mut locked_playlist = app
            .appplayer
            .playlist
            .lock()
            .expect("fail to lock the playlist");
        let was_empty = locked_playlist.file_list.is_empty();
        locked_playlist.add_from_path_and_expand_playlists(&path_buf);
        was_empty
    };
    if was_empty && app.appplayer.play_mod {
        app.appplayer.play_file_on_top();
    }
}

fn mark_missing(list: &mut Option<Vec<HistoryFile>>, path: &Path) {
    if let Some(files) = list {
        for file in files {
            if file.absolute_path == path {
                file.exists = false;
            }
        }
    }
}

fn file_label(relative: &str) -> String {
    Path::new(relative)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| relative.to_string())
}

fn truncate_name(name: &str, max_chars: usize) -> String {
    if name.chars().count() > max_chars {
        let keep = max_chars.saturating_sub(1);
        let mut short: String = name.chars().take(keep).collect();
        short.push('…');
        short
    } else {
        name.to_string()
    }
}

fn exact_play_time(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

fn local_offset() -> FixedOffset {
    *Local::now().offset()
}

struct Rel<'a> {
    seconds: &'a str,
    minute: &'a str,
    minutes: &'a str,
    hour: &'a str,
    hours: &'a str,
    yesterday: &'a str,
    days: &'a str,
    month: &'a str,
    months: &'a str,
    year: &'a str,
    years: &'a str,
}

fn phrases(app: &VirtualBookApp) -> Rel<'_> {
    Rel {
        seconds: &app.i18n.rel_seconds,
        minute: &app.i18n.rel_minute,
        minutes: &app.i18n.rel_minutes,
        hour: &app.i18n.rel_hour,
        hours: &app.i18n.rel_hours,
        yesterday: &app.i18n.rel_yesterday,
        days: &app.i18n.rel_days,
        month: &app.i18n.rel_month,
        months: &app.i18n.rel_months,
        year: &app.i18n.rel_year,
        years: &app.i18n.rel_years,
    }
}

/// Moment-style phrase, with yesterday taken from the local calendar day.
fn relative_play_time(
    time: DateTime<Utc>,
    now: DateTime<Utc>,
    offset: FixedOffset,
    phrases: &Rel<'_>,
) -> String {
    let then_local = time.with_timezone(&offset);
    let now_local = now.with_timezone(&offset);
    let day_delta = (now_local.date_naive() - then_local.date_naive()).num_days();
    let secs = (now - time).num_seconds().max(0);

    if day_delta <= 0 {
        return same_day_phrase(secs, phrases);
    }
    if day_delta == 1 {
        return phrases.yesterday.to_string();
    }
    if day_delta < 26 {
        return fill_n(phrases.days, day_delta);
    }
    let months = ((day_delta as f64) / 30.436875).round() as i64;
    if months <= 1 {
        phrases.month.to_string()
    } else if months < 11 {
        fill_n(phrases.months, months)
    } else {
        let years = ((day_delta as f64) / 365.25).round().max(1.0) as i64;
        if years <= 1 {
            phrases.year.to_string()
        } else {
            fill_n(phrases.years, years)
        }
    }
}

fn same_day_phrase(secs: i64, phrases: &Rel<'_>) -> String {
    if secs < 45 {
        phrases.seconds.to_string()
    } else if secs < 90 {
        phrases.minute.to_string()
    } else if secs < 45 * 60 {
        fill_n(phrases.minutes, round_div(secs, 60))
    } else if secs < 90 * 60 {
        phrases.hour.to_string()
    } else {
        fill_n(phrases.hours, round_div(secs, 3600).max(1))
    }
}

fn round_div(secs: i64, unit: i64) -> i64 {
    ((secs as f64) / (unit as f64)).round() as i64
}

fn fill_n(template: &str, n: i64) -> String {
    template.replace("{n}", &n.to_string())
}

#[cfg(test)]
mod relative_time_tests {
    use super::{relative_play_time, Rel};
    use chrono::{DateTime, TimeZone, Utc};

    fn english() -> Rel<'static> {
        Rel {
            seconds: "a few seconds ago",
            minute: "a minute ago",
            minutes: "{n} minutes ago",
            hour: "an hour ago",
            hours: "{n} hours ago",
            yesterday: "yesterday",
            days: "{n} days ago",
            month: "a month ago",
            months: "{n} months ago",
            year: "a year ago",
            years: "{n} years ago",
        }
    }

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    fn phrase(then: DateTime<Utc>, now: DateTime<Utc>) -> String {
        let offset = chrono::FixedOffset::east_opt(0).unwrap();
        relative_play_time(then, now, offset, &english())
    }

    #[test]
    fn same_day_uses_moment_buckets() {
        let now = at(2026, 11, 7, 15, 0);
        assert_eq!(
            phrase(now - chrono::Duration::seconds(10), now),
            "a few seconds ago"
        );
        assert_eq!(
            phrase(now - chrono::Duration::seconds(50), now),
            "a minute ago"
        );
        assert_eq!(
            phrase(now - chrono::Duration::minutes(12), now),
            "12 minutes ago"
        );
        assert_eq!(
            phrase(now - chrono::Duration::minutes(50), now),
            "an hour ago"
        );
        assert_eq!(phrase(now - chrono::Duration::hours(5), now), "5 hours ago");
    }

    #[test]
    fn yesterday_is_the_previous_calendar_day() {
        let now = at(2026, 11, 7, 10, 0);
        assert_eq!(phrase(at(2026, 11, 6, 23, 0), now), "yesterday");
        assert_eq!(phrase(at(2026, 11, 2, 10, 0), now), "5 days ago");
    }

    #[test]
    fn older_dates_use_months_and_years() {
        let now = at(2026, 11, 7, 12, 0);
        assert_eq!(phrase(at(2026, 10, 1, 12, 0), now), "a month ago");
        assert_eq!(phrase(at(2026, 6, 7, 12, 0), now), "5 months ago");
        assert_eq!(phrase(at(2025, 11, 7, 12, 0), now), "a year ago");
        assert_eq!(phrase(at(2024, 11, 7, 12, 0), now), "2 years ago");
    }
}
