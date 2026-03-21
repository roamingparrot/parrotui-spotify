use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use crate::state::App;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(track) = app.now_playing() else {
        let empty = Paragraph::new("  Nothing playing")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // track info
            Constraint::Percentage(50), // progress
        ])
        .split(inner);

    // Track info
    let status = if app.is_playing() { "▶" } else { "⏸" };
    let info = format!(
        " {status}  {} — {}",
        track.name,
        track.artist_names()
    );
    let info_widget =
        Paragraph::new(info).style(Style::default().fg(Color::White));
    frame.render_widget(info_widget, cols[0]);

    // Progress bar
    let progress = app.progress_ms();
    let duration = track.duration_ms;
    let ratio = if duration > 0 {
        (progress as f64 / duration as f64).min(1.0)
    } else {
        0.0
    };

    let elapsed = format_time(progress);
    let total = format_time(duration);
    let label = format!("{elapsed} / {total}");

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
        .ratio(ratio)
        .label(label);
    frame.render_widget(gauge, cols[1]);
}

fn format_time(ms: u64) -> String {
    let secs = ms / 1000;
    let m = secs / 60;
    let s = secs % 60;
    format!("{m}:{s:02}")
}
