use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::player::Action;
use crate::state::{App, ContentView, FocusPanel};

/// Map a terminal key event to an app action (or None to ignore).
pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<Action> {
    // Any keypress cancels a pending jump-to-bottom chain.
    app.pending_jump_to_bottom = false;

    // Help popup is modal — only Esc/q/?/Enter dismiss it
    if app.show_help {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter => {
                app.show_help = false;
            }
            _ => {}
        }
        return None;
    }

    // Search modal — captures all input when active (unless in a detail drill-down)
    if app.search.is_some() && !app.search_origin {
        return handle_search_key(app, key);
    }

    // Handle the "gg" combo: if we saw a 'g' press before, check for second 'g'
    if app.pending_g {
        app.pending_g = false;
        if key.code == KeyCode::Char('g') {
            app.jump_to_top();
            return None;
        }
        // Not a second 'g' — fall through to normal handling
    }

    match key.code {
        // Quit
        KeyCode::Char('q') => {
            app.running = false;
            None
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.running = false;
            None
        }

        // Help
        KeyCode::Char('?') => {
            app.show_help = true;
            None
        }

        // Search
        KeyCode::Char('/') => {
            app.open_search();
            None
        }

        // Focus switching
        KeyCode::Tab | KeyCode::BackTab => {
            app.toggle_focus();
            None
        }

        // Movement
        KeyCode::Char('j') | KeyCode::Down => {
            app.move_cursor_down();
            // Trigger load-more when near bottom
            if near_bottom(app) {
                return Some(Action::LoadMore);
            }
            None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.move_cursor_up();
            None
        }

        // gg (first g)
        KeyCode::Char('g') => {
            app.pending_g = true;
            None
        }

        // G — jump to bottom of loaded tracks (background loading fills the rest)
        KeyCode::Char('G') => {
            if app.content.can_load_more() || app.content.is_loading() {
                app.pending_jump_to_bottom = true;
            }
            app.jump_to_bottom();
            None
        }

        // Move focus right (no reload)
        KeyCode::Char('l') | KeyCode::Right => {
            if app.focus == FocusPanel::Sidebar {
                app.focus = FocusPanel::Content;
            }
            None
        }

        // Select / expand (loads playlist or plays track)
        KeyCode::Enter => Some(Action::Select),

        // Back / collapse
        KeyCode::Char('h') | KeyCode::Left => {
            if app.search_origin {
                // Return from search detail view to search results
                app.search_origin = false;
                app.content = ContentView::Empty;
                app.focus = FocusPanel::Sidebar;
                None
            } else if app.focus == FocusPanel::Content {
                Some(Action::GoBack)
            } else {
                None
            }
        }

        KeyCode::Esc => {
            if app.search_origin {
                app.search_origin = false;
                app.content = ContentView::Empty;
                None
            } else if app.focus == FocusPanel::Content {
                app.focus = FocusPanel::Sidebar;
                None
            } else {
                None
            }
        }

        // Copy link
        KeyCode::Char('C') => Some(Action::CopyLink),

        // Playback
        KeyCode::Char(' ') => Some(Action::TogglePlayPause),
        KeyCode::Char('n') => Some(Action::NextTrack),
        KeyCode::Char('p') => Some(Action::PreviousTrack),
        KeyCode::Char('+') | KeyCode::Char('=') => Some(Action::VolumeUp),
        KeyCode::Char('-') => Some(Action::VolumeDown),
        KeyCode::Char('>') | KeyCode::Char('.') => Some(Action::SeekForward),
        KeyCode::Char('<') | KeyCode::Char(',') => Some(Action::SeekBackward),
        KeyCode::Char('s') => Some(Action::ToggleShuffle),
        KeyCode::Char('r') => Some(Action::CycleRepeat),

        _ => None,
    }
}

fn handle_search_key(app: &mut App, key: KeyEvent) -> Option<Action> {
    let input_active = app.search.as_ref().unwrap().input_active;

    if input_active {
        match key.code {
            KeyCode::Esc => {
                let has_results = app.search.as_ref().unwrap().has_results();
                if has_results {
                    app.search.as_mut().unwrap().input_active = false;
                } else {
                    app.search = None;
                }
                None
            }
            KeyCode::Enter => {
                let query = app.search.as_ref().unwrap().query.trim().to_string();
                if query.is_empty() {
                    return None;
                }
                let search = app.search.as_mut().unwrap();
                search.loading = true;
                search.clear_results();
                Some(Action::SubmitSearch { query })
            }
            KeyCode::Char(c) => {
                app.search.as_mut().unwrap().push_char(c);
                None
            }
            KeyCode::Backspace => {
                app.search.as_mut().unwrap().pop_char();
                None
            }
            _ => None,
        }
    } else {
        // Results browsing mode — sidebar/content focus model
        match key.code {
            KeyCode::Char('/') => {
                app.search.as_mut().unwrap().input_active = true;
                None
            }
            KeyCode::Char('q') => {
                app.close_search();
                None
            }
            KeyCode::Tab | KeyCode::BackTab => {
                app.toggle_focus();
                None
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if app.focus == FocusPanel::Sidebar {
                    app.focus = FocusPanel::Content;
                }
                None
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if app.focus == FocusPanel::Content {
                    app.focus = FocusPanel::Sidebar;
                }
                None
            }
            KeyCode::Char('j') | KeyCode::Down => {
                match app.focus {
                    FocusPanel::Sidebar => app.search.as_mut().unwrap().sidebar_move_down(),
                    FocusPanel::Content => app.search.as_mut().unwrap().move_down(),
                }
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                match app.focus {
                    FocusPanel::Sidebar => app.search.as_mut().unwrap().sidebar_move_up(),
                    FocusPanel::Content => app.search.as_mut().unwrap().move_up(),
                }
                None
            }
            KeyCode::Enter => match app.focus {
                FocusPanel::Sidebar => {
                    app.search.as_mut().unwrap().select_sidebar_item();
                    app.focus = FocusPanel::Content;
                    None
                }
                FocusPanel::Content => Some(Action::SearchSelect),
            },
            KeyCode::Esc => {
                if app.focus == FocusPanel::Content {
                    app.focus = FocusPanel::Sidebar;
                } else {
                    // Sidebar is the outermost search level — leave search
                    // entirely. Re-enter the query input with '/' instead.
                    app.close_search();
                }
                None
            }
            // Playback controls still work in search results
            KeyCode::Char(' ') => Some(Action::TogglePlayPause),
            KeyCode::Char('n') => Some(Action::NextTrack),
            KeyCode::Char('p') => Some(Action::PreviousTrack),
            _ => None,
        }
    }
}

/// Returns true when the cursor is close enough to the bottom that we should prefetch.
fn near_bottom(app: &App) -> bool {
    match app.focus {
        FocusPanel::Sidebar => {
            let cursor = app.sidebar_cursor;
            let len = app.sidebar_items.len();
            len > 0 && cursor + 5 >= len && app.sidebar_can_load_more()
        }
        FocusPanel::Content => {
            let cursor = app.content.cursor();
            let len = app.content.len();
            len > 0 && cursor + 5 >= len && app.content.can_load_more()
        }
    }
}
