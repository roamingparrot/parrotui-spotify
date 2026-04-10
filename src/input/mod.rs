use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::player::Action;
use crate::state::{App, FocusPanel};

/// Map a terminal key event to an app action (or None to ignore).
pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<Action> {
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

        // G — jump to bottom
        KeyCode::Char('G') => {
            app.jump_to_bottom();
            Some(Action::LoadMore)
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
            if app.focus == FocusPanel::Content {
                Some(Action::GoBack)
            } else {
                None
            }
        }

        KeyCode::Esc => {
            if app.focus == FocusPanel::Content {
                app.focus = FocusPanel::Sidebar;
            }
            None
        }

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
