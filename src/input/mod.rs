pub mod keymap;

pub use keymap::{Command, Key, Keymap, Mode};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::player::Action;
use crate::state::{App, ContentView, FocusPanel};
use keymap::Lookup;

/// Map a terminal key event to an app action (or None to ignore).
pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<Action> {
    // Any keypress cancels a pending jump-to-bottom chain.
    app.pending_jump_to_bottom = false;

    // Ctrl+C always quits — a broken keybindings.toml must not be able to trap
    // the user in a mode with no reachable exit.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.running = false;
        return None;
    }

    let mode = current_mode(app);

    // In the query input every printable character is text, so it can't be a
    // binding. Modifier combinations still fall through to the keymap.
    if mode == Mode::SearchInput
        && let KeyCode::Char(c) = key.code
        && key.modifiers.difference(KeyModifiers::SHIFT).is_empty()
    {
        app.search.as_mut().unwrap().push_char(c);
        return None;
    }

    let pressed = Key::from_event(key);
    let pending = std::mem::take(&mut app.pending_keys);

    match app.keymap.lookup(mode, &pending, pressed) {
        Lookup::Command(cmd) => run_command(app, cmd),
        Lookup::Pending => {
            app.pending_keys = pending;
            app.pending_keys.push(pressed);
            None
        }
        // An abandoned chord shouldn't swallow the key that abandoned it —
        // 'g' then 'j' still moves down.
        Lookup::None if !pending.is_empty() => match app.keymap.lookup(mode, &[], pressed) {
            Lookup::Command(cmd) => run_command(app, cmd),
            Lookup::Pending => {
                app.pending_keys.push(pressed);
                None
            }
            Lookup::None => None,
        },
        Lookup::None => None,
    }
}

fn current_mode(app: &App) -> Mode {
    if app.show_help {
        return Mode::Help;
    }
    // A drill-down opened from search is navigated like the main view.
    match &app.search {
        Some(search) if !app.search_origin => {
            if search.input_active {
                Mode::SearchInput
            } else {
                Mode::Search
            }
        }
        _ => Mode::Normal,
    }
}

fn run_command(app: &mut App, cmd: Command) -> Option<Action> {
    match cmd {
        Command::Quit => {
            app.running = false;
            None
        }
        Command::ToggleHelp => {
            app.show_help = true;
            None
        }
        Command::CloseHelp => {
            app.show_help = false;
            None
        }
        Command::OpenSearch => {
            app.open_search();
            None
        }
        Command::SwitchPanel => {
            app.toggle_focus();
            None
        }
        Command::MoveDown => {
            if app.search.is_some() && !app.search_origin {
                let search = app.search.as_mut().unwrap();
                match app.focus {
                    FocusPanel::Sidebar => search.sidebar_move_down(),
                    FocusPanel::Content => search.move_down(),
                }
                return None;
            }
            app.move_cursor_down();
            // Trigger load-more when near bottom
            if near_bottom(app) {
                return Some(Action::LoadMore);
            }
            None
        }
        Command::MoveUp => {
            if app.search.is_some() && !app.search_origin {
                let search = app.search.as_mut().unwrap();
                match app.focus {
                    FocusPanel::Sidebar => search.sidebar_move_up(),
                    FocusPanel::Content => search.move_up(),
                }
                return None;
            }
            app.move_cursor_up();
            None
        }
        Command::JumpTop => {
            app.jump_to_top();
            None
        }
        Command::JumpBottom => {
            // Background loading fills in the rest of the list behind us.
            if app.content.can_load_more() || app.content.is_loading() {
                app.pending_jump_to_bottom = true;
            }
            app.jump_to_bottom();
            None
        }
        Command::FocusRight => {
            if app.focus == FocusPanel::Sidebar {
                app.focus = FocusPanel::Content;
            }
            None
        }
        Command::FocusLeft => {
            if app.focus == FocusPanel::Content {
                app.focus = FocusPanel::Sidebar;
            }
            None
        }
        Command::Select => {
            if app.search.is_some() && !app.search_origin {
                return match app.focus {
                    FocusPanel::Sidebar => {
                        app.search.as_mut().unwrap().select_sidebar_item();
                        app.focus = FocusPanel::Content;
                        None
                    }
                    FocusPanel::Content => Some(Action::SearchSelect),
                };
            }
            Some(Action::Select)
        }
        Command::GoBack => {
            if app.search_origin {
                // Return from a search drill-down to the results
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
        Command::Unfocus => {
            if app.search.is_some() && !app.search_origin {
                if app.focus == FocusPanel::Content {
                    app.focus = FocusPanel::Sidebar;
                } else {
                    // The sidebar is the outermost search level — leave search
                    // entirely. Re-enter the query input with '/' instead.
                    app.close_search();
                }
                return None;
            }
            if app.search_origin {
                app.search_origin = false;
                app.content = ContentView::Empty;
            } else if app.focus == FocusPanel::Content {
                app.focus = FocusPanel::Sidebar;
            }
            None
        }
        Command::CopyLink => Some(Action::CopyLink),
        Command::TogglePlayPause => Some(Action::TogglePlayPause),
        Command::NextTrack => Some(Action::NextTrack),
        Command::PreviousTrack => Some(Action::PreviousTrack),
        Command::VolumeUp => Some(Action::VolumeUp),
        Command::VolumeDown => Some(Action::VolumeDown),
        Command::SeekForward => Some(Action::SeekForward),
        Command::SeekBackward => Some(Action::SeekBackward),
        Command::ToggleShuffle => Some(Action::ToggleShuffle),
        Command::CycleRepeat => Some(Action::CycleRepeat),
        Command::SearchFocusInput => {
            app.search.as_mut().unwrap().input_active = true;
            None
        }
        Command::SearchSubmit => {
            let query = app.search.as_ref().unwrap().query.trim().to_string();
            if query.is_empty() {
                return None;
            }
            let search = app.search.as_mut().unwrap();
            search.loading = true;
            search.clear_results();
            Some(Action::SubmitSearch { query })
        }
        Command::SearchDeleteChar => {
            app.search.as_mut().unwrap().pop_char();
            None
        }
        Command::SearchExitInput => {
            if app.search.as_ref().unwrap().has_results() {
                app.search.as_mut().unwrap().input_active = false;
            } else {
                app.search = None;
            }
            None
        }
        Command::CloseSearch => {
            app.close_search();
            None
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
