use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table, TableState};
use ratatui::Frame;

use crate::api::Track;
use crate::state::App;
use crate::ui::theme::{panel_style, Theme};
use crate::ui::util::millis_to_minutes;

#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame,
    app: &App,
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

    let now_playing_uri = app
        .now_playing_track
        .as_ref()
        .and_then(|t| t.uri.as_deref());

    let header = Row::new(vec![
        Cell::from(" #"),
        Cell::from("Title"),
        Cell::from("Artist"),
        Cell::from("Album"),
        Cell::from("Time"),
    ])
    .style(Style::default().fg(theme.header).add_modifier(Modifier::BOLD))
    .bottom_margin(0);

    let rows: Vec<Row> = tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_playing = now_playing_uri
                .is_some_and(|uri| track.uri.as_deref() == Some(uri));

            let num = if is_playing {
                "▶".to_string()
            } else {
                format!("{}", i + 1)
            };

            let album = track
                .album
                .as_ref()
                .map(|a| a.name.as_str())
                .unwrap_or("");

            let dur = millis_to_minutes(track.duration_ms);

            let style = if is_playing {
                Style::default()
                    .fg(theme.active)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.base_style()
            };

            Row::new(vec![
                Cell::from(Span::styled(format!(" {num}"), style)),
                Cell::from(Span::styled(track.name.clone(), style)),
                Cell::from(Span::styled(track.artist_names(), style)),
                Cell::from(Span::styled(album.to_string(), style)),
                Cell::from(Span::styled(dur, style)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(35),
        Constraint::Percentage(25),
        Constraint::Percentage(27),
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
