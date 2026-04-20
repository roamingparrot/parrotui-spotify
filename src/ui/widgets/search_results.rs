use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState};

use crate::state::{SearchState, SearchTab};
use crate::ui::theme::Theme;
use crate::ui::util::millis_to_minutes;

pub fn draw(frame: &mut Frame, search: &mut SearchState, area: Rect, focused: bool, theme: &Theme) {
    match search.active_tab {
        SearchTab::Tracks => draw_track_results(frame, search, area, focused, theme),
        SearchTab::Artists => draw_artist_results(frame, search, area, focused, theme),
        SearchTab::Albums => draw_album_results(frame, search, area, focused, theme),
        SearchTab::Playlists => draw_playlist_results(frame, search, area, focused, theme),
    }
}

fn draw_track_results(
    frame: &mut Frame,
    search: &mut SearchState,
    area: Rect,
    focused: bool,
    theme: &Theme,
) {
    let border = crate::ui::theme::panel_style(focused, !focused, theme);
    let block = Block::default()
        .title(format!(" {} ", SearchTab::Tracks.label()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(border)
        .border_style(border);

    if search.result_tracks.is_empty() {
        let p = Paragraph::new("  No tracks found")
            .style(Style::default().fg(theme.inactive))
            .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(" #"),
        Cell::from("Title"),
        Cell::from("Artist"),
        Cell::from("Length"),
    ])
    .style(
        Style::default()
            .fg(theme.header)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = search
        .result_tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let style = theme.base_style();
            Row::new(vec![
                Cell::from(format!(" {}", i + 1)).style(style),
                Cell::from(track.name.as_str()).style(style),
                Cell::from(track.artist_names()).style(style),
                Cell::from(millis_to_minutes(track.duration_ms)).style(style),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(45),
        Constraint::Percentage(40),
        Constraint::Length(6),
    ];

    let cursor = search.tab_cursors[SearchTab::Tracks as usize];
    let scroll_offset = &mut search.tab_scroll_offsets[SearchTab::Tracks as usize];
    let mut state = TableState::default();
    *state.offset_mut() = *scroll_offset;
    state.select(Some(cursor));

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(theme.highlight_style(focused))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, area, &mut state);
    *scroll_offset = state.offset();
}

fn draw_artist_results(
    frame: &mut Frame,
    search: &mut SearchState,
    area: Rect,
    focused: bool,
    theme: &Theme,
) {
    let border = crate::ui::theme::panel_style(focused, !focused, theme);
    let block = Block::default()
        .title(format!(" {} ", SearchTab::Artists.label()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(border)
        .border_style(border);

    if search.result_artists.is_empty() {
        let p = Paragraph::new("  No artists found")
            .style(Style::default().fg(theme.inactive))
            .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(" #"),
        Cell::from("Name"),
        Cell::from("Genres"),
    ])
    .style(
        Style::default()
            .fg(theme.header)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = search
        .result_artists
        .iter()
        .enumerate()
        .map(|(i, artist)| {
            let style = theme.base_style();
            Row::new(vec![
                Cell::from(format!(" {}", i + 1)).style(style),
                Cell::from(artist.name.as_str()).style(style),
                Cell::from(artist.genre_string()).style(style),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(40),
        Constraint::Percentage(55),
    ];

    let cursor = search.tab_cursors[SearchTab::Artists as usize];
    let scroll_offset = &mut search.tab_scroll_offsets[SearchTab::Artists as usize];
    let mut state = TableState::default();
    *state.offset_mut() = *scroll_offset;
    state.select(Some(cursor));

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(theme.highlight_style(focused))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, area, &mut state);
    *scroll_offset = state.offset();
}

fn draw_album_results(
    frame: &mut Frame,
    search: &mut SearchState,
    area: Rect,
    focused: bool,
    theme: &Theme,
) {
    let border = crate::ui::theme::panel_style(focused, !focused, theme);
    let block = Block::default()
        .title(format!(" {} ", SearchTab::Albums.label()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(border)
        .border_style(border);

    if search.result_albums.is_empty() {
        let p = Paragraph::new("  No albums found")
            .style(Style::default().fg(theme.inactive))
            .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(" #"),
        Cell::from("Name"),
        Cell::from("Artist"),
        Cell::from("Tracks"),
    ])
    .style(
        Style::default()
            .fg(theme.header)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = search
        .result_albums
        .iter()
        .enumerate()
        .map(|(i, album)| {
            let style = theme.base_style();
            Row::new(vec![
                Cell::from(format!(" {}", i + 1)).style(style),
                Cell::from(album.name.as_str()).style(style),
                Cell::from(album.artist_names()).style(style),
                Cell::from(format!("{}", album.total_tracks)).style(style),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(45),
        Constraint::Percentage(40),
        Constraint::Length(8),
    ];

    let cursor = search.tab_cursors[SearchTab::Albums as usize];
    let scroll_offset = &mut search.tab_scroll_offsets[SearchTab::Albums as usize];
    let mut state = TableState::default();
    *state.offset_mut() = *scroll_offset;
    state.select(Some(cursor));

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(theme.highlight_style(focused))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, area, &mut state);
    *scroll_offset = state.offset();
}

fn draw_playlist_results(
    frame: &mut Frame,
    search: &mut SearchState,
    area: Rect,
    focused: bool,
    theme: &Theme,
) {
    let border = crate::ui::theme::panel_style(focused, !focused, theme);
    let block = Block::default()
        .title(format!(" {} ", SearchTab::Playlists.label()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(border)
        .border_style(border);

    if search.result_playlists.is_empty() {
        let p = Paragraph::new("  No playlists found")
            .style(Style::default().fg(theme.inactive))
            .block(block);
        frame.render_widget(p, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(" #"),
        Cell::from("Name"),
        Cell::from("Tracks"),
    ])
    .style(
        Style::default()
            .fg(theme.header)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = search
        .result_playlists
        .iter()
        .enumerate()
        .map(|(i, pl)| {
            let style = theme.base_style();
            let track_count = pl
                .tracks
                .as_ref()
                .map(|t| format!("{}", t.total))
                .unwrap_or_default();
            Row::new(vec![
                Cell::from(format!(" {}", i + 1)).style(style),
                Cell::from(pl.name.as_str()).style(style),
                Cell::from(track_count).style(style),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Percentage(70),
        Constraint::Length(8),
    ];

    let cursor = search.tab_cursors[SearchTab::Playlists as usize];
    let scroll_offset = &mut search.tab_scroll_offsets[SearchTab::Playlists as usize];
    let mut state = TableState::default();
    *state.offset_mut() = *scroll_offset;
    state.select(Some(cursor));

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(theme.highlight_style(focused))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, area, &mut state);
    *scroll_offset = state.offset();
}
