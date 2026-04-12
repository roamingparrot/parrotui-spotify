pub mod marquee;
pub mod theme;
pub mod util;
pub mod widgets;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;

use crate::state::{App, ContentView, FocusPanel};
use theme::panel_style;
use widgets::{help_popup, playbar, sidebar, status_line, track_table};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let theme = app.theme;

    // Adaptive margin: tighter on small terminals.
    let margin = if area.height > 45 { 1 } else { 0 };

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

    if app.show_help {
        help_popup::draw(frame, area, &theme);
    }
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
