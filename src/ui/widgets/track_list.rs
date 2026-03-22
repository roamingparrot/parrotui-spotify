use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph};

use crate::api::Track;

/// Render a list of tracks inside a content block. Used by both playlist detail
/// and liked songs views.
pub fn draw(
    frame: &mut Frame,
    tracks: &[Track],
    cursor: usize,
    focused: bool,
    block: Block<'_>,
    area: Rect,
) {
    let inner = block.inner(area);
    let visible = inner.height.saturating_sub(0) as usize; // no sub-header for now

    let offset = if cursor >= visible && visible > 0 {
        cursor - visible + 1
    } else {
        0
    };

    let lines: Vec<Line> = tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_cursor = i == cursor;
            let style = if is_cursor && focused {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else if is_cursor {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };

            let prefix = if is_cursor && focused {
                "▸ "
            } else {
                "  "
            };
            let dur = format_duration(track.duration_ms);
            let text = format!(
                "{prefix}{:<40}  {:<30}  {dur}",
                truncate(&track.name, 38),
                truncate(&track.artist_names(), 28),
            );
            Line::from(Span::styled(text, style))
        })
        .collect();

    let p = Paragraph::new(lines)
        .block(block)
        .scroll((offset as u16, 0));
    frame.render_widget(p, area);
}

fn format_duration(ms: u64) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
