use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Everything a key can be bound to. Commands that need an API call return an
/// `Action` from the handler; the rest only touch `App`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Quit,
    ToggleHelp,
    CloseHelp,
    OpenSearch,
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
    /// Human-readable label for the help popup.
    pub fn label(self) -> &'static str {
        match self {
            Self::Quit => "Quit",
            Self::ToggleHelp => "Toggle help",
            Self::CloseHelp => "Close help",
            Self::OpenSearch => "Search",
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
    Help,
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

impl Default for Keymap {
    fn default() -> Self {
        let mut modes = HashMap::new();
        for (mode, specs) in DEFAULT_BINDINGS {
            let bindings = specs
                .iter()
                .filter_map(|(cmd, spec)| Binding::parse(spec).map(|b| (b, *cmd)))
                .collect();
            modes.insert(*mode, bindings);
        }
        Self { modes }
    }
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
            (Command::SeekBackward, ","),
            (Command::ToggleShuffle, "s"),
            (Command::CycleRepeat, "r"),
            (Command::CopyLink, "C"),
            (Command::OpenSearch, "/"),
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
            (",", Command::SeekBackward),
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
}
