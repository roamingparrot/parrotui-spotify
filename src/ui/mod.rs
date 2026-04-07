pub mod marquee;
pub mod theme;
pub mod util;
pub mod widgets;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

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
    let main = Layout::horizontal([
        Constraint::Length(sidebar_w),
        Constraint::Min(0),
    ])
    .split(outer[1]);

    sidebar::draw(frame, app, main[0], &theme);
    draw_content(frame, app, main[1], &theme);
    playbar::draw(frame, app, outer[2], &theme);
    status_line::draw(frame, app, outer[3], &theme);

    if app.show_help {
        help_popup::draw(frame, area, &theme);
    }
}

fn draw_content(frame: &mut Frame, app: &App, area: Rect, theme: &theme::Theme) {
    let focused = app.focus == FocusPanel::Content;

    match &app.content {
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
        }

        ContentView::PlaylistDetail {
            playlist_name,
            tracks,
            cursor,
            total,
            loading,
            ..
        } => {
            let title = format!("{playlist_name} ({total})");
            if *loading && tracks.is_empty() {
                let border = panel_style(focused, !focused, theme);
                let block = ratatui::widgets::Block::default()
                    .title(format!(" {title} "))
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .title_style(border)
                    .border_style(border);
                let p = Paragraph::new("  Loading tracks...")
                    .style(Style::default().fg(theme.inactive))
                    .block(block);
                frame.render_widget(p, area);
                return;
            }
            track_table::draw(frame, app, tracks, *cursor, &title, focused, area, theme);
        }

        ContentView::LikedSongs {
            tracks,
            cursor,
            total,
            loading,
        } => {
            let title = format!("Liked Songs ({total})");
            if *loading && tracks.is_empty() {
                let border = panel_style(focused, !focused, theme);
                let block = ratatui::widgets::Block::default()
                    .title(format!(" {title} "))
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .title_style(border)
                    .border_style(border);
                let p = Paragraph::new("  Loading liked songs...")
                    .style(Style::default().fg(theme.inactive))
                    .block(block);
                frame.render_widget(p, area);
                return;
            }
            track_table::draw(frame, app, tracks, *cursor, &title, focused, area, theme);
        }
    }
}
