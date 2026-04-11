use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::state::{App, FocusPanel, SidebarItem};
use crate::ui::marquee::{marquee_text, truncate_unicode};
use crate::ui::theme::{panel_style, Theme};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let focused = app.focus == FocusPanel::Sidebar;

    let sections = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(area);

    draw_library(frame, app, sections[0], focused, theme);
    draw_playlists(frame, app, sections[1], focused, theme);
}

fn draw_library(frame: &mut Frame, app: &App, area: Rect, focused: bool, theme: &Theme) {
    let is_active = focused && app.sidebar_cursor == 0;
    let border = panel_style(is_active, focused && !is_active, theme);

    let block = Block::default()
        .title(" Library ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(border)
        .border_style(border);

    let items = vec![ListItem::new("♥ Liked Songs")];

    let mut state = ListState::default();
    if app.sidebar_cursor == 0 {
        state.select(Some(0));
    }

    let list = List::new(items)
        .block(block)
        .style(theme.base_style())
        .highlight_style(theme.highlight_style(focused))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_playlists(frame: &mut Frame, app: &mut App, area: Rect, focused: bool, theme: &Theme) {
    let playlist_count = app.sidebar_items.len().saturating_sub(1);
    let has_selection = app.sidebar_cursor >= 1;
    let is_active = focused && has_selection;
    let border = panel_style(is_active, focused && !is_active, theme);

    let title = if app.sidebar_loading && playlist_count == 0 {
        " Playlists (loading…) ".to_string()
    } else {
        format!(" Playlists ({playlist_count}) ")
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(border)
        .border_style(border);

    let inner = block.inner(area);
    let label_width = inner.width.saturating_sub(3) as usize; // "▶ " prefix

    let items: Vec<ListItem> = app
        .sidebar_items
        .iter()
        .skip(1) // skip LikedSongs — shown in Library section
        .enumerate()
        .map(|(i, item)| {
            let raw = match item {
                SidebarItem::Playlist(pl) => pl.name.clone(),
                SidebarItem::LikedSongs => "Liked Songs".into(),
            };
            let cursor_idx = app.sidebar_cursor.saturating_sub(1);
            let is_selected = has_selection && i == cursor_idx;
            let label = if is_selected && focused {
                let text_w = unicode_width::UnicodeWidthStr::width(raw.as_str());
                if text_w > label_width {
                    let off = app.sidebar_marquee.tick(i, text_w, label_width);
                    marquee_text(&raw, label_width, off)
                } else {
                    raw
                }
            } else {
                truncate_unicode(&raw, label_width)
            };
            ListItem::new(Span::raw(label))
        })
        .collect();

    let mut state = ListState::default();
    if has_selection {
        state.select(Some(app.sidebar_cursor - 1));
    }

    let list = List::new(items)
        .block(block)
        .style(theme.base_style())
        .highlight_style(theme.highlight_style(focused))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut state);
}
