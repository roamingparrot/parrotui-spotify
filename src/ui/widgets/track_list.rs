use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::api::Track;
use crate::state::MarqueeState;
use crate::ui::marquee::{marquee_text, truncate_unicode};

/// Render a list of tracks inside a content block. Used by both playlist detail
/// and liked songs views.
pub fn draw(
    frame: &mut Frame,
    tracks: &[Track],
    cursor: usize,
    focused: bool,
    block: Block<'_>,
    area: Rect,
    marquee: &mut MarqueeState,
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
            let name_col = if is_cursor && focused {
                let name_w = UnicodeWidthStr::width(track.name.as_str());
                if name_w > 38 {
                    let off = marquee.tick(i, name_w, 38);
                    marquee_text(&track.name, 38, off)
                } else {
                    truncate_unicode(&track.name, 38)
                }
            } else {
                truncate_unicode(&track.name, 38)
            };
            let artist_col = truncate_unicode(&track.artist_names(), 28);
            let text = format!(
                "{prefix}{:<40}  {:<30}  {dur}",
                name_col,
                artist_col,
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

