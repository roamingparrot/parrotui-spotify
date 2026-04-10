use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table, TableState};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::api::Track;
use crate::state::marquee::MarqueeState;
use crate::ui::marquee::{marquee_text, truncate_unicode};
use crate::ui::theme::{panel_style, Theme};
use crate::ui::util::millis_to_minutes;

#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame,
    now_playing_uri: Option<&str>,
    marquee: &mut MarqueeState,
    tracks: &[Track],
    cursor: usize,
    title: &str,
    focused: bool,
    area: Rect,
    theme: &Theme,
) {
    let border = panel_style(focused, !focused, theme);

    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(border)
        .border_style(border);

    if tracks.is_empty() {
        frame.render_widget(block, area);
        return;
    }

    // Estimate the title column width: 45% of inner width (matching the constraint).
    let inner_w = area.width.saturating_sub(2); // borders
    let title_col_w = ((inner_w as f32 * 0.45) as usize).saturating_sub(4);

    let header = Row::new(vec![
        Cell::from(" #"),
        Cell::from("Title"),
        Cell::from("Artist"),
        Cell::from("Length"),
    ])
    .style(Style::default().fg(theme.header).add_modifier(Modifier::BOLD))
    .bottom_margin(0);

    let rows: Vec<Row> = tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_playing =
                now_playing_uri.is_some_and(|uri| track.uri.as_deref() == Some(uri));

            let num = if is_playing {
                "▶".to_string()
            } else {
                format!("{}", i + 1)
            };

            let dur = millis_to_minutes(track.duration_ms);

            let style = if is_playing {
                Style::default()
                    .fg(theme.active)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.base_style()
            };

            // Apply marquee scrolling to the selected row's title.
            let name = if i == cursor && focused && title_col_w > 0 {
                let text_w = UnicodeWidthStr::width(track.name.as_str());
                if text_w > title_col_w {
                    let off = marquee.tick(i, text_w, title_col_w);
                    marquee_text(&track.name, title_col_w, off)
                } else {
                    track.name.clone()
                }
            } else {
                truncate_unicode(&track.name, title_col_w)
            };

            Row::new(vec![
                Cell::from(Span::styled(format!(" {num}"), style)),
                Cell::from(Span::styled(name, style)),
                Cell::from(Span::styled(track.artist_names(), style)),
                Cell::from(Span::styled(dur, style)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(45),
        Constraint::Percentage(40),
        Constraint::Length(6),
    ];

    let mut state = TableState::default();
    state.select(Some(cursor));

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(theme.highlight_style(focused))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, area, &mut state);
}
