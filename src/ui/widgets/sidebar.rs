use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};

use crate::state::{App, FocusPanel};
use crate::ui::marquee::{marquee_text, truncate_unicode};
use crate::ui::theme::{Theme, panel_style};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let focused = app.focus == FocusPanel::Sidebar;

    let show_liked = app.config.sidebar_show_liked_songs;
    let show_playlists = app.config.sidebar_show_playlists;
    let show_albums = app.config.sidebar_show_albums;

    let mut constraints = Vec::new();
    if show_liked {
        constraints.push(Constraint::Length(3));
    }
    if show_playlists {
        constraints.push(Constraint::Min(0));
    }
    if show_albums {
        constraints.push(Constraint::Min(0));
    }
    if constraints.is_empty() {
        return;
    }

    let sections = Layout::vertical(constraints).split(area);
    let mut idx = 0;

    if show_liked {
        draw_library(frame, app, sections[idx], focused, theme);
        idx += 1;
    }
    if show_playlists {
        let start = app.sidebar_playlists_start();
        let labels: Vec<String> = app
            .sidebar_playlists
            .iter()
            .map(|pl| pl.name.clone())
            .collect();
        let mut offset = app.sidebar_playlists_scroll_offset;
        draw_item_list(
            frame,
            app,
            sections[idx],
            focused,
            theme,
            "Playlists",
            &labels,
            start,
            app.sidebar_playlists_loading,
            &mut offset,
        );
        app.sidebar_playlists_scroll_offset = offset;
        idx += 1;
    }
    if show_albums {
        let start = app.sidebar_albums_start();
        let labels: Vec<String> = app
            .sidebar_albums
            .iter()
            .map(|al| al.name.clone())
            .collect();
        let mut offset = app.sidebar_albums_scroll_offset;
        draw_item_list(
            frame,
            app,
            sections[idx],
            focused,
            theme,
            "Albums",
            &labels,
            start,
            app.sidebar_albums_loading,
            &mut offset,
        );
        app.sidebar_albums_scroll_offset = offset;
    }
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

/// Renders one paginated sidebar section (Playlists or Albums). `section_start`
/// is the index within `app.sidebar_items` where this section's rows begin —
/// used to translate the global cursor into a row-local selection.
#[allow(clippy::too_many_arguments)]
fn draw_item_list(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    focused: bool,
    theme: &Theme,
    title_word: &str,
    labels: &[String],
    section_start: usize,
    loading: bool,
    scroll_offset: &mut usize,
) {
    let count = labels.len();
    let has_selection =
        app.sidebar_cursor >= section_start && app.sidebar_cursor < section_start + count;
    let is_active = focused && has_selection;
    let border = panel_style(is_active, focused && !is_active, theme);

    let title = if loading && count == 0 {
        format!(" {title_word} (loading…) ")
    } else {
        format!(" {title_word} ({count}) ")
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(border)
        .border_style(border);

    let inner = block.inner(area);
    let label_width = inner.width.saturating_sub(3) as usize; // "▶ " prefix

    let cursor_idx = app.sidebar_cursor.saturating_sub(section_start);
    let items: Vec<ListItem> = labels
        .iter()
        .enumerate()
        .map(|(i, raw)| {
            let is_selected = has_selection && i == cursor_idx;
            let label = if is_selected && focused {
                let text_w = unicode_width::UnicodeWidthStr::width(raw.as_str());
                if text_w > label_width {
                    let off = app.sidebar_marquee.tick(i, text_w, label_width);
                    marquee_text(raw, label_width, off)
                } else {
                    raw.clone()
                }
            } else {
                truncate_unicode(raw, label_width)
            };
            ListItem::new(Span::raw(label))
        })
        .collect();

    let mut state = ListState::default();
    *state.offset_mut() = *scroll_offset;
    if has_selection {
        state.select(Some(cursor_idx));
    }

    let list = List::new(items)
        .block(block)
        .style(theme.base_style())
        .highlight_style(theme.highlight_style(focused))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut state);
    *scroll_offset = state.offset();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Album, Artist};
    use crate::config::Config;
    use crate::input::Keymap;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn sample_album(name: &str) -> Album {
        Album {
            id: "id".into(),
            name: name.into(),
            uri: "spotify:album:id".into(),
            artists: vec![Artist {
                name: "Artist".into(),
            }],
            total_tracks: 10,
            images: Vec::new(),
        }
    }

    fn render(app: &mut App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| crate::ui::draw(f, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn hidden_sections_do_not_render() {
        let mut app = App::new(Config::default(), Keymap::default(), "device".to_string());
        app.set_sidebar_albums(vec![sample_album("My Album")], 1);
        app.config.sidebar_show_playlists = false;
        app.config.sidebar_show_albums = false;
        app.rebuild_sidebar_items();

        let text = render(&mut app, 60, 30);
        assert!(!text.contains("Playlists"));
        assert!(!text.contains("Albums"));
        assert!(text.contains("Liked Songs"));
    }

    #[test]
    fn visible_album_renders_its_name() {
        let mut app = App::new(Config::default(), Keymap::default(), "device".to_string());
        app.set_sidebar_albums(vec![sample_album("My Album")], 1);

        let text = render(&mut app, 60, 30);
        assert!(text.contains("My Album"));
    }

    #[test]
    fn everything_hidden_renders_without_panicking() {
        let mut app = App::new(Config::default(), Keymap::default(), "device".to_string());
        app.config.sidebar_show_playlists = false;
        app.config.sidebar_show_albums = false;
        app.config.sidebar_show_liked_songs = false;
        app.rebuild_sidebar_items();

        render(&mut app, 60, 30);
        assert!(app.current_sidebar_item().is_none());
    }
}
