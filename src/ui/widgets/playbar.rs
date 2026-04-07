use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, LineGauge, Paragraph};
use ratatui::Frame;

use crate::api::RepeatMode;
use crate::state::App;
use crate::ui::theme::Theme;
use crate::ui::util::{track_progress_label, track_progress_ratio};

pub fn draw(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let shuffle_label = if app.shuffle { "On" } else { "Off" };
    let repeat_label = match app.repeat {
        RepeatMode::Off => "Off",
        RepeatMode::Context => "All",
        RepeatMode::Track => "One",
    };

    let play_state = if app.progress.is_playing() {
        "Playing"
    } else {
        "Paused"
    };

    let title = format!(
        " {play_state} ({} | Shuffle: {shuffle_label} | Repeat: {repeat_label} | Vol: {}%) ",
        app.device_name, app.volume,
    );

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(theme.active))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.active))
        .style(Style::default().bg(theme.playbar_background));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 3 {
        return;
    }

    let rows =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
            .split(inner);

    match &app.now_playing_track {
        Some(track) => {
            draw_track_info(frame, track, rows[0], theme);
            draw_controls(frame, app, rows[1], theme);
            draw_progress(frame, app, track.duration_ms, rows[2], theme);
        }
        None => {
            let msg = Paragraph::new("Nothing playing")
                .style(Style::default().fg(theme.inactive))
                .alignment(Alignment::Center);
            frame.render_widget(msg, rows[1]);
        }
    }
}

fn draw_track_info(frame: &mut Frame, track: &crate::api::Track, area: Rect, theme: &Theme) {
    let name = Span::styled(
        &track.name,
        Style::default()
            .fg(theme.selected)
            .add_modifier(Modifier::BOLD),
    );
    let sep = Span::raw("  —  ");
    let artist = Span::styled(track.artist_names(), Style::default().fg(theme.playbar_text));

    let line = Line::from(vec![name, sep, artist]);
    let p = Paragraph::new(line).alignment(Alignment::Center);
    frame.render_widget(p, area);
}

fn draw_controls(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let style = Style::default().fg(theme.playbar_text);
    let active = Style::default()
        .fg(theme.active)
        .add_modifier(Modifier::BOLD);

    let shuffle_style = if app.shuffle { active } else { style };
    let repeat_style = if app.repeat != RepeatMode::Off {
        active
    } else {
        style
    };
    let play_icon = if app.progress.is_playing() {
        "⏸"
    } else {
        "▶"
    };

    let line = Line::from(vec![
        Span::styled("⏮", style),
        Span::raw("  "),
        Span::styled(play_icon, active),
        Span::raw("  "),
        Span::styled("⏭", style),
        Span::raw("    "),
        Span::styled("🔀", shuffle_style),
        Span::raw("  "),
        Span::styled("🔁", repeat_style),
    ]);

    let p = Paragraph::new(line).alignment(Alignment::Center);
    frame.render_widget(p, area);
}

fn draw_progress(frame: &mut Frame, app: &App, dur_ms: u64, area: Rect, theme: &Theme) {
    let pos = app.progress.position_ms();
    let ratio = track_progress_ratio(pos, dur_ms);
    let label = track_progress_label(pos, dur_ms);

    let gauge = LineGauge::default()
        .filled_style(
            Style::default()
                .fg(theme.playbar_progress)
                .add_modifier(Modifier::BOLD),
        )
        .unfilled_style(Style::default().fg(theme.inactive))
        .ratio(ratio)
        .label(Span::styled(
            label,
            Style::default().fg(theme.playbar_progress_text),
        ));

    frame.render_widget(gauge, area);
}
