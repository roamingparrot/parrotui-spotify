pub mod keymap;

pub use keymap::{Command, Key, Keymap, Mode};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::player::Action;
use crate::state::settings::{SettingsState, ValueEditor};
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

    // Where text is being typed, every printable character is content rather
    // than a binding. Modifier combinations still fall through to the keymap.
    if let KeyCode::Char(c) = key.code
        && key.modifiers.difference(KeyModifiers::SHIFT).is_empty()
    {
        match typing_target(app, mode) {
            Some(TypingTarget::SearchQuery) => {
                app.search.as_mut().unwrap().push_char(c);
                return None;
            }
            Some(TypingTarget::SettingValue) => {
                let settings = app.settings.as_mut().unwrap();
                settings.editor.as_mut().unwrap().push_char(c);
                return None;
            }
            None => {}
        }
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
    if let Some(settings) = &app.settings {
        return if settings.editor.is_some() {
            Mode::SettingsEditor
        } else {
            Mode::Settings
        };
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

/// Which text field, if any, is currently swallowing printable characters.
enum TypingTarget {
    SearchQuery,
    SettingValue,
}

fn typing_target(app: &App, mode: Mode) -> Option<TypingTarget> {
    match mode {
        Mode::SearchInput => Some(TypingTarget::SearchQuery),
        // Choice editors are lists, so j/k stay navigation there.
        Mode::SettingsEditor => match app.settings.as_ref()?.editor.as_ref()? {
            ValueEditor::Entry { .. } => Some(TypingTarget::SettingValue),
            ValueEditor::Choice { .. } => None,
        },
        _ => None,
    }
}

fn run_command(app: &mut App, cmd: Command) -> Option<Action> {
    if app.settings.is_some() {
        return run_settings_command(app, cmd);
    }

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
        Command::OpenSettings => {
            app.settings = Some(SettingsState::new());
            None
        }
        Command::SwitchPanel => {
            app.toggle_focus();
            None
        }
        Command::MoveDown => {
            if !app.search_origin
                && let Some(search) = app.search.as_mut()
            {
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
            if !app.search_origin
                && let Some(search) = app.search.as_mut()
            {
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
            if !app.search_origin
                && let Some(search) = app.search.as_mut()
            {
                return match app.focus {
                    FocusPanel::Sidebar => {
                        search.select_sidebar_item();
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
        // Only reachable while the settings view is open, which is handled
        // above by run_settings_command.
        Command::CloseSettings | Command::ResetSetting | Command::SettingsDeleteChar => None,
    }
}

fn run_settings_command(app: &mut App, cmd: Command) -> Option<Action> {
    if app.settings.as_ref().unwrap().editor.is_some() {
        return run_editor_command(app, cmd);
    }

    match cmd {
        Command::MoveDown => {
            app.settings.as_mut().unwrap().move_down();
            None
        }
        Command::MoveUp => {
            app.settings.as_mut().unwrap().move_up();
            None
        }
        Command::FocusLeft => {
            let settings = app.settings.as_mut().unwrap();
            let tab = settings.tab.prev();
            settings.switch_tab(tab);
            None
        }
        Command::FocusRight | Command::SwitchPanel => {
            let settings = app.settings.as_mut().unwrap();
            let tab = settings.tab.next();
            settings.switch_tab(tab);
            None
        }
        Command::Select => {
            let settings = app.settings.as_ref().unwrap();
            if let Some(key) = settings.current_key() {
                let editor = ValueEditor::open(key, &app.config);
                app.settings.as_mut().unwrap().editor = Some(editor);
            }
            None
        }
        Command::ResetSetting => {
            let key = app.settings.as_ref().unwrap().current_key()?;
            key.reset(&mut app.config);
            app.apply_config();
            Some(Action::SaveConfig)
        }
        Command::CloseSettings => {
            app.settings = None;
            None
        }
        Command::TogglePlayPause => Some(Action::TogglePlayPause),
        Command::NextTrack => Some(Action::NextTrack),
        Command::PreviousTrack => Some(Action::PreviousTrack),
        _ => None,
    }
}

fn run_editor_command(app: &mut App, cmd: Command) -> Option<Action> {
    match cmd {
        Command::MoveDown => {
            app.settings
                .as_mut()
                .unwrap()
                .editor
                .as_mut()
                .unwrap()
                .move_down();
            None
        }
        Command::MoveUp => {
            app.settings
                .as_mut()
                .unwrap()
                .editor
                .as_mut()
                .unwrap()
                .move_up();
            None
        }
        Command::SettingsDeleteChar => {
            app.settings
                .as_mut()
                .unwrap()
                .editor
                .as_mut()
                .unwrap()
                .pop_char();
            None
        }
        Command::Unfocus => {
            app.settings.as_mut().unwrap().editor = None;
            None
        }
        Command::Select => {
            let editor = app.settings.as_ref().unwrap().editor.as_ref().unwrap();
            let key = editor.key();
            let value = editor.selection();

            if !key.apply(&mut app.config, &value) {
                // Leave the popup open so the value can be corrected.
                app.notify_error(format!("invalid {}: {value}", key.label()));
                return None;
            }

            app.apply_config();
            app.settings.as_mut().unwrap().editor = None;
            Some(Action::SaveConfig)
        }
        _ => None,
    }
}

/// Returns true when the cursor is close enough to the bottom that we should prefetch.
fn near_bottom(app: &App) -> bool {
    match app.focus {
        FocusPanel::Sidebar => {
            let cursor = app.sidebar_cursor;
            let section_end = app.sidebar_current_section_end();
            section_end > 0 && cursor + 5 >= section_end && app.sidebar_can_load_more()
        }
        FocusPanel::Content => {
            let cursor = app.content.cursor();
            let len = app.content.len();
            len > 0 && cursor + 5 >= len && app.content.can_load_more()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::state::settings::{SettingKey, SettingsTab};

    fn app() -> App {
        App::new(Config::default(), Keymap::default(), "device".to_string())
    }

    fn press(app: &mut App, spec: char) -> Option<Action> {
        handle_key(app, KeyEvent::new(KeyCode::Char(spec), KeyModifiers::NONE))
    }

    fn press_code(app: &mut App, code: KeyCode) -> Option<Action> {
        handle_key(app, KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn comma_opens_and_q_closes_settings() {
        let mut app = app();
        press(&mut app, ',');
        assert!(app.settings.is_some());
        press(&mut app, 'q');
        assert!(app.settings.is_none(), "q should leave settings, not quit");
        assert!(app.running, "q in settings must not quit the app");
    }

    #[test]
    fn tabs_and_rows_navigate() {
        let mut app = app();
        press(&mut app, ',');
        press(&mut app, 'l');
        assert_eq!(app.settings.as_ref().unwrap().tab, SettingsTab::Playback);
        press(&mut app, 'j');
        assert_eq!(app.settings.as_ref().unwrap().cursor, 1);
        // Switching tab resets the row cursor.
        press(&mut app, 'h');
        assert_eq!(app.settings.as_ref().unwrap().cursor, 0);
    }

    #[test]
    fn committing_a_choice_saves_and_applies() {
        let mut app = app();
        press(&mut app, ',');
        press_code(&mut app, KeyCode::Enter); // open the theme editor
        assert!(app.settings.as_ref().unwrap().editor.is_some());

        press(&mut app, 'j'); // default -> spotify
        let action = press_code(&mut app, KeyCode::Enter);

        assert!(matches!(action, Some(Action::SaveConfig)));
        assert_eq!(app.config.theme, "spotify");
        assert!(app.settings.as_ref().unwrap().editor.is_none());
        // The live theme is re-derived, not just stored.
        assert_eq!(
            app.theme.selected,
            crate::ui::theme::Theme::spotify().selected
        );
    }

    #[test]
    fn text_editor_takes_typed_characters() {
        let mut app = app();
        press(&mut app, ',');
        press(&mut app, 'l');
        app.settings.as_mut().unwrap().cursor = SettingsTab::Playback
            .settings()
            .iter()
            .position(|k| *k == SettingKey::DeviceName)
            .unwrap();
        press_code(&mut app, KeyCode::Enter);

        // 'j' would navigate elsewhere — here it is text.
        press(&mut app, 'j');
        let editor = app.settings.as_ref().unwrap().editor.as_ref().unwrap();
        assert_eq!(
            editor.selection(),
            format!("{}j", Config::default().device_name)
        );

        press_code(&mut app, KeyCode::Backspace);
        let editor = app.settings.as_ref().unwrap().editor.as_ref().unwrap();
        assert_eq!(editor.selection(), Config::default().device_name);
    }

    #[test]
    fn escape_cancels_an_edit() {
        let mut app = app();
        press(&mut app, ',');
        press_code(&mut app, KeyCode::Enter);
        press(&mut app, 'j');
        press_code(&mut app, KeyCode::Esc);

        assert!(app.settings.as_ref().unwrap().editor.is_none());
        assert_eq!(
            app.config.theme,
            Config::default().theme,
            "should not commit"
        );
        assert!(app.settings.is_some(), "esc should close the popup only");
    }

    #[test]
    fn reset_restores_the_focused_setting() {
        let mut app = app();
        app.config.theme = "dracula".to_string();
        press(&mut app, ',');
        let action = press(&mut app, 'r');

        assert!(matches!(action, Some(Action::SaveConfig)));
        assert_eq!(app.config.theme, Config::default().theme);
    }

    #[test]
    fn ctrl_c_quits_even_while_typing() {
        let mut app = app();
        press(&mut app, '/');
        assert!(app.search.is_some());
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );
        assert!(!app.running);
        // The 'c' must not have landed in the query.
        assert_eq!(app.search.as_ref().unwrap().query, "");
    }

    #[test]
    fn abandoned_chord_still_runs_the_second_key() {
        let mut app = app();
        press(&mut app, 'g');
        assert!(!app.pending_keys.is_empty());
        // 'g' then ',' opens settings rather than being swallowed.
        press(&mut app, ',');
        assert!(app.settings.is_some());
        assert!(app.pending_keys.is_empty());
    }
}
