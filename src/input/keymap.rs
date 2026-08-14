use std::collections::HashMap;
use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::error::{Result, SpotError};

/// Everything a key can be bound to. Commands that need an API call return an
/// `Action` from the handler; the rest only touch `App`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    Quit,
    ToggleHelp,
    CloseHelp,
    OpenSearch,
    OpenSettings,
    CloseSettings,
    ResetSetting,
    SettingsDeleteChar,
    SwitchPanel,
    MoveDown,
    MoveUp,
    JumpTop,
    JumpBottom,
    FocusRight,
    FocusLeft,
    Select,
    GoBack,
    Unfocus,
    CopyLink,
    TogglePlayPause,
    NextTrack,
    PreviousTrack,
    VolumeUp,
    VolumeDown,
    SeekForward,
    SeekBackward,
    ToggleShuffle,
    CycleRepeat,
    SearchFocusInput,
    SearchSubmit,
    SearchDeleteChar,
    SearchExitInput,
    CloseSearch,
}

impl Command {
    /// Name used in keybindings.toml.
    pub fn name(self) -> &'static str {
        match self {
            Self::Quit => "quit",
            Self::ToggleHelp => "toggle_help",
            Self::CloseHelp => "close_help",
            Self::OpenSearch => "open_search",
            Self::OpenSettings => "open_settings",
            Self::CloseSettings => "close_settings",
            Self::ResetSetting => "reset_setting",
            Self::SettingsDeleteChar => "settings_delete_char",
            Self::SwitchPanel => "switch_panel",
            Self::MoveDown => "move_down",
            Self::MoveUp => "move_up",
            Self::JumpTop => "jump_top",
            Self::JumpBottom => "jump_bottom",
            Self::FocusRight => "focus_right",
            Self::FocusLeft => "focus_left",
            Self::Select => "select",
            Self::GoBack => "go_back",
            Self::Unfocus => "unfocus",
            Self::CopyLink => "copy_link",
            Self::TogglePlayPause => "toggle_play_pause",
            Self::NextTrack => "next_track",
            Self::PreviousTrack => "previous_track",
            Self::VolumeUp => "volume_up",
            Self::VolumeDown => "volume_down",
            Self::SeekForward => "seek_forward",
            Self::SeekBackward => "seek_backward",
            Self::ToggleShuffle => "toggle_shuffle",
            Self::CycleRepeat => "cycle_repeat",
            Self::SearchFocusInput => "search_focus_input",
            Self::SearchSubmit => "search_submit",
            Self::SearchDeleteChar => "search_delete_char",
            Self::SearchExitInput => "search_exit_input",
            Self::CloseSearch => "close_search",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        DEFAULT_BINDINGS
            .iter()
            .flat_map(|(_, specs)| specs.iter())
            .map(|(cmd, _)| *cmd)
            .find(|cmd| cmd.name() == name)
    }

    /// Human-readable label for the help popup.
    pub fn label(self) -> &'static str {
        match self {
            Self::Quit => "Quit",
            Self::ToggleHelp => "Toggle help",
            Self::CloseHelp => "Close help",
            Self::OpenSearch => "Search",
            Self::OpenSettings => "Settings",
            Self::CloseSettings => "Close settings",
            Self::ResetSetting => "Reset to default",
            Self::SettingsDeleteChar => "Delete character",
            Self::SwitchPanel => "Switch panel",
            Self::MoveDown => "Move down",
            Self::MoveUp => "Move up",
            Self::JumpTop => "Jump to top",
            Self::JumpBottom => "Jump to bottom",
            Self::FocusRight => "Move focus right",
            Self::FocusLeft => "Move focus left",
            Self::Select => "Select / expand",
            Self::GoBack => "Go back",
            Self::Unfocus => "Leave panel",
            Self::CopyLink => "Copy link",
            Self::TogglePlayPause => "Play / pause",
            Self::NextTrack => "Next track",
            Self::PreviousTrack => "Previous track",
            Self::VolumeUp => "Volume up",
            Self::VolumeDown => "Volume down",
            Self::SeekForward => "Seek forward",
            Self::SeekBackward => "Seek backward",
            Self::ToggleShuffle => "Toggle shuffle",
            Self::CycleRepeat => "Cycle repeat",
            Self::SearchFocusInput => "Edit query",
            Self::SearchSubmit => "Submit query",
            Self::SearchDeleteChar => "Delete character",
            Self::SearchExitInput => "Leave query input",
            Self::CloseSearch => "Exit search",
        }
    }
}

/// Which key table applies. Each maps to a `[section]` in keybindings.toml.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Normal,
    Search,
    SearchInput,
    Settings,
    SettingsEditor,
    Help,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Search => "search",
            Self::SearchInput => "search_input",
            Self::Settings => "settings",
            Self::SettingsEditor => "settings_editor",
            Self::Help => "help",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        DEFAULT_BINDINGS
            .iter()
            .map(|(mode, _)| *mode)
            .find(|mode| mode.name() == name)
    }
}

/// A single key press: a code plus the modifiers that matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    code: KeyCode,
    mods: KeyModifiers,
}

impl Key {
    pub fn from_event(key: KeyEvent) -> Self {
        normalize(Self {
            code: key.code,
            mods: key.modifiers,
        })
    }
}

/// Shift is folded into the character itself ('G' rather than shift+g), so
/// comparing it separately would make uppercase bindings depend on how the
/// terminal reports them.
fn normalize(mut key: Key) -> Key {
    if matches!(key.code, KeyCode::Char(_)) {
        key.mods.remove(KeyModifiers::SHIFT);
    }
    key
}

/// One or more keys pressed in sequence — `gg` is a two-key binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding(Vec<Key>);

impl Binding {
    fn parse(spec: &str) -> Option<Self> {
        let keys: Option<Vec<Key>> = spec.split_whitespace().map(parse_key).collect();
        let keys = keys?;
        if keys.is_empty() {
            None
        } else {
            Some(Self(keys))
        }
    }

    pub fn display(&self) -> String {
        self.0
            .iter()
            .map(|k| format_key(*k))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn parse_key(spec: &str) -> Option<Key> {
    let mut mods = KeyModifiers::NONE;
    let mut rest = spec;

    // Modifier prefixes, e.g. "ctrl+alt+x". A trailing '+' is the plus key.
    while let Some(idx) = rest.find('+') {
        if idx == 0 {
            break;
        }
        let (prefix, tail) = rest.split_at(idx);
        match prefix.to_ascii_lowercase().as_str() {
            "ctrl" => mods |= KeyModifiers::CONTROL,
            "alt" => mods |= KeyModifiers::ALT,
            "shift" => mods |= KeyModifiers::SHIFT,
            _ => break,
        }
        rest = &tail[1..];
    }

    let code = match rest.to_ascii_lowercase().as_str() {
        "space" => KeyCode::Char(' '),
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        lower => {
            if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
                KeyCode::F(n)
            } else {
                // Use the original spelling so 'G' stays distinct from 'g'.
                let mut chars = rest.chars();
                let ch = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                KeyCode::Char(ch)
            }
        }
    };

    Some(normalize(Key { code, mods }))
}

fn format_key(key: Key) -> String {
    let base = match key.code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "BackTab".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Insert => "Insert".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        KeyCode::F(n) => format!("F{n}"),
        other => format!("{other:?}"),
    };

    let mut out = String::new();
    if key.mods.contains(KeyModifiers::CONTROL) {
        out.push_str("Ctrl+");
    }
    if key.mods.contains(KeyModifiers::ALT) {
        out.push_str("Alt+");
    }
    if key.mods.contains(KeyModifiers::SHIFT) {
        out.push_str("Shift+");
    }
    out.push_str(&base);
    out
}

/// Result of feeding a key to the keymap.
pub enum Lookup {
    /// A binding matched outright.
    Command(Command),
    /// The key starts a longer binding — wait for the next press.
    Pending,
    /// Nothing matched.
    None,
}

pub struct Keymap {
    modes: HashMap<Mode, Vec<(Binding, Command)>>,
}

impl Keymap {
    /// Resolve a key press, taking into account any keys already pressed as
    /// part of an unfinished chord.
    pub fn lookup(&self, mode: Mode, pending: &[Key], key: Key) -> Lookup {
        let Some(bindings) = self.modes.get(&mode) else {
            return Lookup::None;
        };

        let mut sequence = pending.to_vec();
        sequence.push(key);

        if let Some((_, cmd)) = bindings.iter().find(|(b, _)| b.0 == sequence) {
            return Lookup::Command(*cmd);
        }
        if bindings
            .iter()
            .any(|(b, _)| b.0.len() > sequence.len() && b.0.starts_with(&sequence))
        {
            return Lookup::Pending;
        }
        Lookup::None
    }

    /// Bindings for a mode in declaration order, for the help popup.
    pub fn bindings(&self, mode: Mode) -> &[(Binding, Command)] {
        self.modes.get(&mode).map(Vec::as_slice).unwrap_or(&[])
    }
}

impl Keymap {
    /// Read the keymap from disk, writing the defaults out first if the file
    /// isn't there yet. Individual entries that don't make sense are logged and
    /// skipped rather than failing startup.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(path, default_file())?;
            return Ok(Self::default());
        }

        Self::from_toml(&std::fs::read_to_string(path)?)
    }

    fn from_toml(raw: &str) -> Result<Self> {
        let sections: HashMap<String, HashMap<String, Vec<String>>> = toml::from_str(raw)
            .map_err(|e| SpotError::Config(format!("bad keybindings toml: {e}")))?;

        let mut overrides: HashMap<(Mode, Command), Vec<Binding>> = HashMap::new();
        for (mode_name, commands) in sections {
            let Some(mode) = Mode::from_name(&mode_name) else {
                tracing::warn!(section = %mode_name, "unknown keybinding section, ignoring");
                continue;
            };
            for (cmd_name, specs) in commands {
                let Some(cmd) = Command::from_name(&cmd_name) else {
                    tracing::warn!(command = %cmd_name, "unknown command, ignoring");
                    continue;
                };
                let bindings = overrides.entry((mode, cmd)).or_default();
                for spec in specs {
                    match Binding::parse(&spec) {
                        Some(b) => bindings.push(b),
                        None => {
                            tracing::warn!(key = %spec, command = %cmd_name, "unparseable key, ignoring")
                        }
                    }
                }
            }
        }

        Ok(Self::build(&overrides))
    }

    /// Merge overrides onto the defaults. Display order follows
    /// `DEFAULT_BINDINGS` so the help popup stays readable no matter how the
    /// user's file is arranged.
    fn build(overrides: &HashMap<(Mode, Command), Vec<Binding>>) -> Self {
        let mut modes: HashMap<Mode, Vec<(Binding, Command)>> = HashMap::new();
        let mut overridden: Vec<(Mode, Command)> = Vec::new();

        for (mode, specs) in DEFAULT_BINDINGS {
            let list = modes.entry(*mode).or_default();
            for (cmd, spec) in *specs {
                match overrides.get(&(*mode, *cmd)) {
                    Some(bindings) => {
                        // A command with several default aliases appears more
                        // than once — only expand its override the first time.
                        if !overridden.contains(&(*mode, *cmd)) {
                            overridden.push((*mode, *cmd));
                            list.extend(bindings.iter().cloned().map(|b| (b, *cmd)));
                        }
                    }
                    None => {
                        if let Some(b) = Binding::parse(spec) {
                            list.push((b, *cmd));
                        }
                    }
                }
            }
        }

        // Commands bound in a mode that has no default for them.
        let mut extra: Vec<_> = overrides
            .iter()
            .filter(|((mode, cmd), _)| !overridden.contains(&(*mode, *cmd)))
            .collect();
        extra.sort_by_key(|((mode, cmd), _)| (mode.name(), cmd.name()));
        for ((mode, cmd), bindings) in extra {
            modes
                .entry(*mode)
                .or_default()
                .extend(bindings.iter().cloned().map(|b| (b, *cmd)));
        }

        Self { modes }
    }
}

impl Default for Keymap {
    fn default() -> Self {
        Self::build(&HashMap::new())
    }
}

/// The commented default keybindings.toml. Written verbatim on first run so the
/// explanation survives — we never rewrite this file.
fn default_file() -> String {
    let mut out = String::from(
        "# Keybindings for parrotui-spotify.\n\
         #\n\
         # Keys are named (\"space\", \"esc\", \"down\", \"f5\"), single characters\n\
         # (\"j\", \"G\"), modifier combinations (\"ctrl+x\"), or space-separated\n\
         # sequences (\"g g\"). Uppercase and lowercase are different keys.\n\
         #\n\
         # Sections are the context the key applies in. Commands you leave out keep\n\
         # their defaults; give one an empty list to unbind it.\n\
         #\n\
         # Ctrl+C always quits and can't be rebound, and in [search_input] every\n\
         # printable character is typed into the query rather than run as a\n\
         # command.\n",
    );

    for (mode, specs) in DEFAULT_BINDINGS {
        out.push_str(&format!("\n[{}]\n", mode.name()));

        // Group a command's aliases onto one line, in declaration order.
        let mut grouped: Vec<(Command, Vec<&str>)> = Vec::new();
        for (cmd, spec) in *specs {
            match grouped.iter_mut().find(|(c, _)| c == cmd) {
                Some((_, keys)) => keys.push(spec),
                None => grouped.push((*cmd, vec![spec])),
            }
        }

        for (cmd, keys) in grouped {
            let list: Vec<String> = keys.iter().map(|k| format!("\"{k}\"")).collect();
            out.push_str(&format!("{} = [{}]\n", cmd.name(), list.join(", ")));
        }
    }

    out
}

/// The default bindings, in the order they appear in the help popup.
type ModeDefaults = (Mode, &'static [(Command, &'static str)]);

const DEFAULT_BINDINGS: &[ModeDefaults] = &[
    (
        Mode::Normal,
        &[
            (Command::MoveDown, "j"),
            (Command::MoveDown, "down"),
            (Command::MoveUp, "k"),
            (Command::MoveUp, "up"),
            (Command::JumpTop, "g g"),
            (Command::JumpBottom, "G"),
            (Command::FocusRight, "l"),
            (Command::FocusRight, "right"),
            (Command::Select, "enter"),
            (Command::GoBack, "h"),
            (Command::GoBack, "left"),
            (Command::Unfocus, "esc"),
            (Command::SwitchPanel, "tab"),
            (Command::SwitchPanel, "backtab"),
            (Command::TogglePlayPause, "space"),
            (Command::NextTrack, "n"),
            (Command::PreviousTrack, "p"),
            (Command::VolumeUp, "+"),
            (Command::VolumeUp, "="),
            (Command::VolumeDown, "-"),
            (Command::SeekForward, ">"),
            (Command::SeekForward, "."),
            (Command::SeekBackward, "<"),
            (Command::ToggleShuffle, "s"),
            (Command::CycleRepeat, "r"),
            (Command::CopyLink, "C"),
            (Command::OpenSearch, "/"),
            (Command::OpenSettings, ","),
            (Command::ToggleHelp, "?"),
            (Command::Quit, "q"),
        ],
    ),
    (
        Mode::Search,
        &[
            (Command::MoveDown, "j"),
            (Command::MoveDown, "down"),
            (Command::MoveUp, "k"),
            (Command::MoveUp, "up"),
            (Command::FocusRight, "l"),
            (Command::FocusRight, "right"),
            (Command::FocusLeft, "h"),
            (Command::FocusLeft, "left"),
            (Command::Select, "enter"),
            (Command::SwitchPanel, "tab"),
            (Command::SwitchPanel, "backtab"),
            (Command::SearchFocusInput, "/"),
            (Command::Unfocus, "esc"),
            (Command::CloseSearch, "q"),
            (Command::TogglePlayPause, "space"),
            (Command::NextTrack, "n"),
            (Command::PreviousTrack, "p"),
        ],
    ),
    (
        Mode::SearchInput,
        &[
            (Command::SearchSubmit, "enter"),
            (Command::SearchDeleteChar, "backspace"),
            (Command::SearchExitInput, "esc"),
        ],
    ),
    (
        Mode::Settings,
        &[
            (Command::MoveDown, "j"),
            (Command::MoveDown, "down"),
            (Command::MoveUp, "k"),
            (Command::MoveUp, "up"),
            (Command::Select, "enter"),
            (Command::FocusLeft, "h"),
            (Command::FocusLeft, "left"),
            (Command::FocusRight, "l"),
            (Command::FocusRight, "right"),
            (Command::SwitchPanel, "tab"),
            (Command::ResetSetting, "r"),
            (Command::CloseSettings, "esc"),
            (Command::CloseSettings, "q"),
            (Command::CloseSettings, ","),
            (Command::TogglePlayPause, "space"),
            (Command::NextTrack, "n"),
            (Command::PreviousTrack, "p"),
        ],
    ),
    (
        Mode::SettingsEditor,
        &[
            (Command::MoveDown, "j"),
            (Command::MoveDown, "down"),
            (Command::MoveUp, "k"),
            (Command::MoveUp, "up"),
            (Command::Select, "enter"),
            (Command::SettingsDeleteChar, "backspace"),
            (Command::Unfocus, "esc"),
        ],
    ),
    (
        Mode::Help,
        &[
            (Command::CloseHelp, "esc"),
            (Command::CloseHelp, "q"),
            (Command::CloseHelp, "?"),
            (Command::CloseHelp, "enter"),
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn key(spec: &str) -> Key {
        parse_key(spec).expect("spec should parse")
    }

    fn command(keymap: &Keymap, mode: Mode, pending: &[Key], k: Key) -> Option<Command> {
        match keymap.lookup(mode, pending, k) {
            Lookup::Command(cmd) => Some(cmd),
            _ => None,
        }
    }

    #[test]
    fn parses_named_and_literal_keys() {
        assert_eq!(key("space").code, KeyCode::Char(' '));
        assert_eq!(key("esc").code, KeyCode::Esc);
        assert_eq!(key("down").code, KeyCode::Down);
        assert_eq!(key("f5").code, KeyCode::F(5));
        // '+' is a bindable key, not a dangling modifier separator.
        assert_eq!(key("+").code, KeyCode::Char('+'));
    }

    #[test]
    fn parses_modifiers() {
        let k = key("ctrl+c");
        assert_eq!(k.code, KeyCode::Char('c'));
        assert!(k.mods.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn uppercase_is_distinct_from_lowercase() {
        assert_eq!(key("G").code, KeyCode::Char('G'));
        assert_eq!(key("g").code, KeyCode::Char('g'));
        assert_ne!(key("G"), key("g"));
    }

    #[test]
    fn shift_is_ignored_for_characters() {
        // Terminals disagree on whether 'G' arrives with SHIFT set, so a
        // binding on "G" has to match either report.
        let event = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(Key::from_event(event), key("G"));
    }

    #[test]
    fn rejects_unparseable_specs() {
        assert!(parse_key("nope").is_none());
        assert!(Binding::parse("").is_none());
    }

    #[test]
    fn chord_resolves_only_once_complete() {
        let keymap = Keymap::default();
        let g = key("g");

        assert!(matches!(
            keymap.lookup(Mode::Normal, &[], g),
            Lookup::Pending
        ));
        assert_eq!(
            command(&keymap, Mode::Normal, &[g], g),
            Some(Command::JumpTop)
        );
        // 'g' followed by anything else is not a binding.
        assert!(matches!(
            keymap.lookup(Mode::Normal, &[g], key("j")),
            Lookup::None
        ));
    }

    #[test]
    fn defaults_cover_the_documented_bindings() {
        let keymap = Keymap::default();
        for (spec, expected) in [
            ("j", Command::MoveDown),
            ("down", Command::MoveDown),
            ("k", Command::MoveUp),
            ("G", Command::JumpBottom),
            ("enter", Command::Select),
            ("h", Command::GoBack),
            ("tab", Command::SwitchPanel),
            ("space", Command::TogglePlayPause),
            ("n", Command::NextTrack),
            ("p", Command::PreviousTrack),
            ("+", Command::VolumeUp),
            ("=", Command::VolumeUp),
            ("-", Command::VolumeDown),
            (">", Command::SeekForward),
            (".", Command::SeekForward),
            ("<", Command::SeekBackward),
            (",", Command::OpenSettings),
            ("s", Command::ToggleShuffle),
            ("r", Command::CycleRepeat),
            ("C", Command::CopyLink),
            ("/", Command::OpenSearch),
            ("?", Command::ToggleHelp),
            ("q", Command::Quit),
        ] {
            assert_eq!(
                command(&keymap, Mode::Normal, &[], key(spec)),
                Some(expected),
                "normal mode binding for {spec}"
            );
        }
    }

    #[test]
    fn search_mode_rebinds_shared_keys() {
        let keymap = Keymap::default();
        // 'q' leaves search rather than quitting the app, and 'h' moves focus
        // instead of going back.
        assert_eq!(
            command(&keymap, Mode::Search, &[], key("q")),
            Some(Command::CloseSearch)
        );
        assert_eq!(
            command(&keymap, Mode::Search, &[], key("h")),
            Some(Command::FocusLeft)
        );
        assert_eq!(
            command(&keymap, Mode::Help, &[], key("q")),
            Some(Command::CloseHelp)
        );
    }

    #[test]
    fn search_input_only_binds_control_keys() {
        let keymap = Keymap::default();
        assert_eq!(
            command(&keymap, Mode::SearchInput, &[], key("enter")),
            Some(Command::SearchSubmit)
        );
        // Printable characters are text, so they must not resolve to commands.
        assert_eq!(command(&keymap, Mode::SearchInput, &[], key("q")), None);
        assert_eq!(command(&keymap, Mode::SearchInput, &[], key("j")), None);
    }

    #[test]
    fn default_file_round_trips() {
        let parsed = Keymap::from_toml(&default_file()).expect("default file should parse");
        let defaults = Keymap::default();
        for mode in [Mode::Normal, Mode::Search, Mode::SearchInput, Mode::Help] {
            assert_eq!(
                parsed.bindings(mode),
                defaults.bindings(mode),
                "{} bindings should survive a round trip",
                mode.name()
            );
        }
    }

    #[test]
    fn override_replaces_the_default_keys() {
        let keymap = Keymap::from_toml("[normal]\nmove_down = [\"x\"]\n").unwrap();
        assert_eq!(
            command(&keymap, Mode::Normal, &[], key("x")),
            Some(Command::MoveDown)
        );
        // The defaults it replaced are gone...
        assert_eq!(command(&keymap, Mode::Normal, &[], key("j")), None);
        assert_eq!(command(&keymap, Mode::Normal, &[], key("down")), None);
        // ...but untouched commands keep theirs.
        assert_eq!(
            command(&keymap, Mode::Normal, &[], key("k")),
            Some(Command::MoveUp)
        );
    }

    #[test]
    fn empty_list_unbinds() {
        let keymap = Keymap::from_toml("[normal]\nquit = []\n").unwrap();
        assert_eq!(command(&keymap, Mode::Normal, &[], key("q")), None);
    }

    #[test]
    fn unknown_names_are_skipped_not_fatal() {
        let keymap = Keymap::from_toml(
            "[nonsense]\nmove_down = [\"x\"]\n\n[normal]\nfly_away = [\"y\"]\nnot_a_key = [\"nope\"]\n",
        )
        .expect("unknown entries should not fail the load");
        // Defaults survive intact.
        assert_eq!(
            command(&keymap, Mode::Normal, &[], key("j")),
            Some(Command::MoveDown)
        );
        assert_eq!(command(&keymap, Mode::Normal, &[], key("x")), None);
        assert_eq!(command(&keymap, Mode::Normal, &[], key("y")), None);
    }

    #[test]
    fn malformed_toml_is_an_error() {
        assert!(Keymap::from_toml("[normal\nbroken").is_err());
    }

    #[test]
    fn chord_can_be_rebound() {
        let keymap = Keymap::from_toml("[normal]\njump_top = [\"z z\"]\n").unwrap();
        let z = key("z");
        assert!(matches!(
            keymap.lookup(Mode::Normal, &[], z),
            Lookup::Pending
        ));
        assert_eq!(
            command(&keymap, Mode::Normal, &[z], z),
            Some(Command::JumpTop)
        );
        assert!(matches!(
            keymap.lookup(Mode::Normal, &[], key("g")),
            Lookup::None
        ));
    }
}
