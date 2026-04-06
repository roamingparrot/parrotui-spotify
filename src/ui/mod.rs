pub mod marquee;
pub mod widgets;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::state::{App, ContentView, FocusPanel};
use marquee::{marquee_text, truncate_unicode};
use widgets::{help_popup, now_playing_bar, track_list};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24),
            Constraint::Min(0),
        ])
        .split(outer[0]);

    draw_sidebar(frame, app, main[0]);
    draw_content(frame, app, main[1]);
    now_playing_bar::draw(frame, app, outer[1]);
    draw_status_line(frame, app, outer[2]);

    if app.show_help {
        help_popup::draw(frame, area);
    }
}

fn draw_sidebar(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == FocusPanel::Sidebar;
    let block = Block::default()
        .title(" Library ")
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let label_width = area.width.saturating_sub(4) as usize; // borders (2) + prefix (2)

    let items: Vec<Line> = app
        .sidebar_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let raw_label = match item {
                crate::state::SidebarItem::LikedSongs => "Liked Songs".to_string(),
                crate::state::SidebarItem::Playlist(pl) => pl.name.clone(),
            };
            let style = if i == app.sidebar_cursor && focused {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else if i == app.sidebar_cursor {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };

            let prefix = if i == app.sidebar_cursor { "▸ " } else { "  " };
            let label = if i == app.sidebar_cursor {
                let text_w = UnicodeWidthStr::width(raw_label.as_str());
                if text_w > label_width {
                    let off = app.sidebar_marquee.tick(i, text_w, label_width);
                    marquee_text(&raw_label, label_width, off)
                } else {
                    raw_label
                }
            } else {
                truncate_unicode(&raw_label, label_width)
            };
            Line::from(Span::styled(format!("{prefix}{label}"), style))
        })
        .collect();

    let offset = scroll_offset(app.sidebar_cursor, inner_height(area));
    let list = Paragraph::new(items).block(block).scroll((offset as u16, 0));
    frame.render_widget(list, area);
}

fn draw_content(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == FocusPanel::Content;
    match &app.content {
        ContentView::Empty => {
            let block = content_block(" Tracks ", focused);
            let p = Paragraph::new("  Select a playlist")
                .style(Style::default().fg(Color::DarkGray))
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
            let title = format!(" {playlist_name} ({total}) ");
            let block = content_block(&title, focused);

            if *loading {
                let p = Paragraph::new("  Loading tracks...")
                    .style(Style::default().fg(Color::DarkGray))
                    .block(block);
                frame.render_widget(p, area);
                return;
            }

            track_list::draw(frame, tracks, *cursor, focused, block, area, &mut app.track_marquee);
        }

        ContentView::LikedSongs {
            tracks,
            cursor,
            total,
            loading,
        } => {
            let title = format!(" Liked Songs ({total}) ");
            let block = content_block(&title, focused);

            if *loading {
                let p = Paragraph::new("  Loading liked songs...")
                    .style(Style::default().fg(Color::DarkGray))
                    .block(block);
                frame.render_widget(p, area);
                return;
            }

            track_list::draw(frame, tracks, *cursor, focused, block, area, &mut app.track_marquee);
        }
    }
}

fn draw_status_line(frame: &mut Frame, app: &App, area: Rect) {
    let left = format!(" ♫ {}", app.device_name);

    let right = {
        let s = if app.shuffle { "S" } else { "-" };
        let r = app.repeat.label();
        let v = app.volume;
        format!("shuf:{s}  rep:{r}  vol:{v}% ")
    };

    let text = if let Some(n) = &app.notification {
        let style = if n.is_error {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Yellow)
        };
        Line::from(Span::styled(format!(" {} ", n.message), style))
    } else {
        let padding = area
            .width
            .saturating_sub(left.len() as u16 + right.len() as u16);
        Line::from(vec![
            Span::styled(left, Style::default().fg(Color::Green)),
            Span::raw(" ".repeat(padding as usize)),
            Span::styled(right, Style::default().fg(Color::DarkGray)),
        ])
    };

    let bar = Paragraph::new(text).style(Style::default().bg(Color::Rgb(30, 30, 30)));
    frame.render_widget(bar, area);
}

fn content_block(title: &str, focused: bool) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        })
}

fn inner_height(area: Rect) -> usize {
    area.height.saturating_sub(2) as usize
}

fn scroll_offset(cursor: usize, visible: usize) -> usize {
    if visible == 0 {
        return 0;
    }
    if cursor >= visible {
        cursor - visible + 1
    } else {
        0
    }
}
