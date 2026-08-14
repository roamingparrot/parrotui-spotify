pub mod marquee;
pub mod theme;
pub mod util;
pub mod widgets;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};

use crate::state::{App, ContentView, FocusPanel, SearchTab};
use theme::panel_style;
use widgets::{help_popup, playbar, search_bar, search_results, sidebar, status_line, track_table};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let theme = app.theme;

    // Adaptive margin: tighter on small terminals.
    let margin = if area.height > 45 { 1 } else { 0 };

    let search_active = app.search.is_some();

    if search_active {
        let outer = Layout::vertical([
            Constraint::Length(margin),
            Constraint::Length(3), // search bar
            Constraint::Min(0),    // main content area
            Constraint::Length(5), // playbar
            Constraint::Length(1), // status line
            Constraint::Length(margin),
        ])
        .split(area);

        let search = app.search.as_ref().unwrap();
        search_bar::draw(frame, search, outer[1], &theme);

        let sidebar_w = (area.width / 5).max(20);
        let main =
            Layout::horizontal([Constraint::Length(sidebar_w), Constraint::Min(0)]).split(outer[2]);

        // Always show search category sidebar while search is active
        let sidebar_focused = app.focus == FocusPanel::Sidebar && !app.search_origin;
        draw_search_sidebar(frame, app, main[0], sidebar_focused, &theme);

        if app.search_origin {
            // Drilled into a detail view — show content panel
            draw_content(frame, app, main[1], &theme);
        } else {
            // Browsing search results
            let content_focused = app.focus == FocusPanel::Content;
            let search = app.search.as_mut().unwrap();
            search_results::draw(frame, search, main[1], content_focused, &theme);
        }

        playbar::draw(frame, app, outer[3], &theme);
        status_line::draw(frame, app, outer[4], &theme);
    } else {
        let outer = Layout::vertical([
            Constraint::Length(margin),
            Constraint::Min(0),
            Constraint::Length(5), // playbar
            Constraint::Length(1), // status line
            Constraint::Length(margin),
        ])
        .split(area);

        let sidebar_w = (area.width / 5).max(20);
        let main =
            Layout::horizontal([Constraint::Length(sidebar_w), Constraint::Min(0)]).split(outer[1]);

        sidebar::draw(frame, app, main[0], &theme);
        draw_content(frame, app, main[1], &theme);
        playbar::draw(frame, app, outer[2], &theme);
        status_line::draw(frame, app, outer[3], &theme);
    }

    if app.show_help {
        help_popup::draw(frame, &app.keymap, area, &theme);
    }
}

fn draw_search_sidebar(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    focused: bool,
    theme: &theme::Theme,
) {
    let search = app.search.as_ref().unwrap();
    let border = panel_style(focused, !focused, theme);

    let block = Block::default()
        .title(" Results ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(border)
        .border_style(border);

    let items: Vec<ListItem> = SearchTab::ALL
        .iter()
        .map(|tab| {
            let count = match tab {
                SearchTab::Tracks => search.result_tracks.len(),
                SearchTab::Artists => search.result_artists.len(),
                SearchTab::Albums => search.result_albums.len(),
                SearchTab::Playlists => search.result_playlists.len(),
            };
            let label = format!("{} ({})", tab.label(), count);
            let style = if *tab == search.active_tab {
                Style::default()
                    .fg(theme.active)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.base_style()
            };
            ListItem::new(format!("  {label}")).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(theme.highlight_style(focused))
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(search.sidebar_cursor));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_content(frame: &mut Frame, app: &mut App, area: Rect, theme: &theme::Theme) {
    let focused = app.focus == FocusPanel::Content;

    // Extract info from content before splitting borrows for marquee.
    let (title, cursor, tracks_empty, loading) = match &app.content {
        ContentView::Empty => {
            let border = panel_style(false, false, theme);
            let block = ratatui::widgets::Block::default()
                .title(" Tracks ")
                .borders(ratatui::widgets::Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title_style(border)
                .border_style(border);
            let p = Paragraph::new("  Select a playlist")
                .style(Style::default().fg(theme.inactive))
                .block(block);
            frame.render_widget(p, area);
            return;
        }
        ContentView::PlaylistDetail {
            playlist_name,
            tracks,
            cursor,
            total,
            loading,
            ..
        } => (
            format!("{playlist_name} ({total})"),
            *cursor,
            tracks.is_empty(),
            *loading,
        ),
        ContentView::LikedSongs {
            tracks,
            cursor,
            total,
            loading,
        } => (
            format!("Liked Songs ({total})"),
            *cursor,
            tracks.is_empty(),
            *loading,
        ),
        ContentView::AlbumDetail {
            album_name,
            artist_name,
            tracks,
            cursor,
            total,
            loading,
            ..
        } => (
            format!("{album_name} — {artist_name} ({total})"),
            *cursor,
            tracks.is_empty(),
            *loading,
        ),
        ContentView::ArtistTopTracks {
            artist_name,
            tracks,
            cursor,
            ..
        } => (
            format!("{artist_name} — Top Tracks"),
            *cursor,
            tracks.is_empty(),
            false,
        ),
    };

    if loading && tracks_empty {
        let border = panel_style(focused, !focused, theme);
        let block = ratatui::widgets::Block::default()
            .title(format!(" {title} "))
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title_style(border)
            .border_style(border);
        let p = Paragraph::new("  Loading...")
            .style(Style::default().fg(theme.inactive))
            .block(block);
        frame.render_widget(p, area);
        return;
    }

    let now_playing_uri = app
        .now_playing_track
        .as_ref()
        .and_then(|t| t.uri.as_deref())
        .map(|s| s.to_string());

    // Split borrows: content (tracks), track_marquee, and content_scroll_offset
    // are disjoint fields.
    let tracks = app.content.tracks();
    let marquee = &mut app.track_marquee;
    let scroll_offset = &mut app.content_scroll_offset;
    track_table::draw(
        frame,
        now_playing_uri.as_deref(),
        marquee,
        tracks,
        cursor,
        &title,
        focused,
        area,
        theme,
        scroll_offset,
    );
}
