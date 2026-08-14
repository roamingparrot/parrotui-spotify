use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Appearance,
    Playback,
    Behavior,
    Keybindings,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 4] = [
        Self::Appearance,
        Self::Playback,
        Self::Behavior,
        Self::Keybindings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Playback => "Playback",
            Self::Behavior => "Behavior",
            Self::Keybindings => "Keybindings",
        }
    }

    /// The rows shown on this tab. Keybindings renders the keymap instead.
    pub fn settings(self) -> &'static [SettingKey] {
        match self {
            Self::Appearance => &[SettingKey::Theme],
            Self::Playback => &[
                SettingKey::InitialVolume,
                SettingKey::VolumeStep,
                SettingKey::SeekStep,
                SettingKey::DeviceName,
                SettingKey::Bitrate,
                SettingKey::Normalisation,
                SettingKey::AudioCacheMb,
            ],
            Self::Behavior => &[SettingKey::TickRate, SettingKey::RefreshInterval],
            Self::Keybindings => &[],
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// How a setting's value is edited.
pub enum SettingKind {
    Choice(&'static [&'static str]),
    Number { min: u64, max: u64 },
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKey {
    Theme,
    InitialVolume,
    VolumeStep,
    SeekStep,
    DeviceName,
    Bitrate,
    Normalisation,
    AudioCacheMb,
    TickRate,
    RefreshInterval,
}

impl SettingKey {
    pub fn label(self) -> &'static str {
        match self {
            Self::Theme => "Theme",
            Self::InitialVolume => "Initial volume (%)",
            Self::VolumeStep => "Volume step (%)",
            Self::SeekStep => "Seek step (seconds)",
            Self::DeviceName => "Device name",
            Self::Bitrate => "Bitrate (kbps)",
            Self::Normalisation => "Volume normalisation",
            Self::AudioCacheMb => "Audio cache (MB)",
            Self::TickRate => "Tick rate (ms)",
            Self::RefreshInterval => "Refresh interval (seconds)",
        }
    }

    /// Shown next to the value when it won't take effect straight away.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::InitialVolume => Some("applies on next launch"),
            Self::DeviceName | Self::Bitrate | Self::Normalisation | Self::AudioCacheMb => {
                Some("requires restart")
            }
            _ => None,
        }
    }

    pub fn kind(self) -> SettingKind {
        match self {
            Self::Theme => SettingKind::Choice(&["default", "spotify", "dracula"]),
            Self::Bitrate => SettingKind::Choice(&["96", "160", "320"]),
            Self::Normalisation => SettingKind::Choice(&["on", "off"]),
            Self::DeviceName => SettingKind::Text,
            Self::InitialVolume => SettingKind::Number { min: 0, max: 100 },
            Self::VolumeStep => SettingKind::Number { min: 1, max: 50 },
            Self::SeekStep => SettingKind::Number { min: 1, max: 600 },
            Self::AudioCacheMb => SettingKind::Number {
                min: 0,
                max: 100_000,
            },
            Self::TickRate => SettingKind::Number { min: 10, max: 1000 },
            Self::RefreshInterval => SettingKind::Number { min: 1, max: 3600 },
        }
    }

    pub fn value(self, config: &Config) -> String {
        match self {
            // An empty theme name falls back to the default palette.
            Self::Theme => {
                if config.theme.is_empty() {
                    "default".to_string()
                } else {
                    config.theme.clone()
                }
            }
            Self::InitialVolume => config.initial_volume.to_string(),
            Self::VolumeStep => config.volume_step.to_string(),
            Self::SeekStep => config.seek_step_secs.to_string(),
            Self::DeviceName => config.device_name.clone(),
            Self::Bitrate => config.bitrate.to_string(),
            Self::Normalisation => if config.normalisation { "on" } else { "off" }.to_string(),
            Self::AudioCacheMb => config.audio_cache_mb.to_string(),
            Self::TickRate => config.tick_rate_ms.to_string(),
            Self::RefreshInterval => config.refresh_interval_secs.to_string(),
        }
    }

    /// Write a value edited in the popup back to the config. Numbers are
    /// clamped; anything unparseable is rejected so the old value survives.
    pub fn apply(self, config: &mut Config, raw: &str) -> bool {
        let raw = raw.trim();
        if let SettingKind::Number { min, max } = self.kind() {
            let Ok(n) = raw.parse::<u64>() else {
                return false;
            };
            let n = n.clamp(min, max);
            match self {
                Self::InitialVolume => config.initial_volume = n as u8,
                Self::VolumeStep => config.volume_step = n as u8,
                Self::SeekStep => config.seek_step_secs = n,
                Self::AudioCacheMb => config.audio_cache_mb = n,
                Self::TickRate => config.tick_rate_ms = n,
                Self::RefreshInterval => config.refresh_interval_secs = n,
                _ => return false,
            }
            return true;
        }

        match self {
            Self::Theme => config.theme = raw.to_string(),
            Self::Bitrate => match raw.parse::<u16>() {
                Ok(kbps) => config.bitrate = kbps,
                Err(_) => return false,
            },
            Self::Normalisation => config.normalisation = raw == "on",
            Self::DeviceName => {
                if raw.is_empty() {
                    return false;
                }
                config.device_name = raw.to_string();
            }
            _ => return false,
        }
        true
    }

    pub fn reset(self, config: &mut Config) {
        let d = Config::default();
        match self {
            Self::Theme => config.theme = d.theme,
            Self::InitialVolume => config.initial_volume = d.initial_volume,
            Self::VolumeStep => config.volume_step = d.volume_step,
            Self::SeekStep => config.seek_step_secs = d.seek_step_secs,
            Self::DeviceName => config.device_name = d.device_name,
            Self::Bitrate => config.bitrate = d.bitrate,
            Self::Normalisation => config.normalisation = d.normalisation,
            Self::AudioCacheMb => config.audio_cache_mb = d.audio_cache_mb,
            Self::TickRate => config.tick_rate_ms = d.tick_rate_ms,
            Self::RefreshInterval => config.refresh_interval_secs = d.refresh_interval_secs,
        }
    }
}

/// The value editor popup. Choices are picked from a list, everything else is
/// typed; like the search bar, text is appended and deleted at the end.
pub enum ValueEditor {
    Choice {
        key: SettingKey,
        options: &'static [&'static str],
        cursor: usize,
    },
    Entry {
        key: SettingKey,
        buffer: String,
    },
}

impl ValueEditor {
    pub fn open(key: SettingKey, config: &Config) -> Self {
        match key.kind() {
            SettingKind::Choice(options) => {
                let current = key.value(config);
                let cursor = options.iter().position(|o| *o == current).unwrap_or(0);
                Self::Choice {
                    key,
                    options,
                    cursor,
                }
            }
            SettingKind::Number { .. } | SettingKind::Text => Self::Entry {
                key,
                buffer: key.value(config),
            },
        }
    }

    pub fn key(&self) -> SettingKey {
        match self {
            Self::Choice { key, .. } | Self::Entry { key, .. } => *key,
        }
    }

    /// The popup's natural size: tall enough for its rows and no taller, wide
    /// enough for its title and content and no wider. Keeps a one-line text
    /// field from getting a multi-row list's worth of empty space.
    pub fn popup_size(&self) -> (u16, u16) {
        let title_width = self.key().label().chars().count();

        let (content_width, body_rows) = match self {
            Self::Choice { options, .. } => (
                options.iter().map(|o| o.chars().count()).max().unwrap_or(0),
                options.len(),
            ),
            Self::Entry { buffer, .. } => (buffer.chars().count(), 1),
        };

        let width = (title_width.max(content_width) + 6).clamp(24, 60) as u16;
        let height = (body_rows + 2) as u16;
        (width, height)
    }

    /// The value to commit.
    pub fn selection(&self) -> String {
        match self {
            Self::Choice {
                options, cursor, ..
            } => options[*cursor].to_string(),
            Self::Entry { buffer, .. } => buffer.clone(),
        }
    }

    pub fn move_down(&mut self) {
        if let Self::Choice {
            options, cursor, ..
        } = self
            && *cursor + 1 < options.len()
        {
            *cursor += 1;
        }
    }

    pub fn move_up(&mut self) {
        if let Self::Choice { cursor, .. } = self {
            *cursor = cursor.saturating_sub(1);
        }
    }

    pub fn push_char(&mut self, ch: char) {
        if let Self::Entry { buffer, .. } = self {
            buffer.push(ch);
        }
    }

    pub fn pop_char(&mut self) {
        if let Self::Entry { buffer, .. } = self {
            buffer.pop();
        }
    }
}

pub struct SettingsState {
    pub tab: SettingsTab,
    pub cursor: usize,
    pub editor: Option<ValueEditor>,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            tab: SettingsTab::Appearance,
            cursor: 0,
            editor: None,
        }
    }

    pub fn current_key(&self) -> Option<SettingKey> {
        self.tab.settings().get(self.cursor).copied()
    }

    pub fn move_down(&mut self) {
        let len = self.tab.settings().len();
        if len > 0 && self.cursor + 1 < len {
            self.cursor += 1;
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn switch_tab(&mut self, tab: SettingsTab) {
        self.tab = tab;
        self.cursor = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_appears_on_exactly_one_tab() {
        let all = [
            SettingKey::Theme,
            SettingKey::InitialVolume,
            SettingKey::VolumeStep,
            SettingKey::SeekStep,
            SettingKey::DeviceName,
            SettingKey::Bitrate,
            SettingKey::Normalisation,
            SettingKey::AudioCacheMb,
            SettingKey::TickRate,
            SettingKey::RefreshInterval,
        ];
        for key in all {
            let count = SettingsTab::ALL
                .iter()
                .filter(|tab| tab.settings().contains(&key))
                .count();
            assert_eq!(count, 1, "{:?} should be on one tab", key);
        }
    }

    #[test]
    fn numbers_are_clamped_to_their_range() {
        let mut config = Config::default();
        assert!(SettingKey::InitialVolume.apply(&mut config, "500"));
        assert_eq!(config.initial_volume, 100);
        assert!(SettingKey::VolumeStep.apply(&mut config, "0"));
        assert_eq!(config.volume_step, 1);
        assert!(SettingKey::TickRate.apply(&mut config, "1"));
        assert_eq!(config.tick_rate_ms, 10);
    }

    #[test]
    fn unparseable_values_are_rejected() {
        let mut config = Config::default();
        let before = config.tick_rate_ms;
        assert!(!SettingKey::TickRate.apply(&mut config, "soon"));
        assert_eq!(config.tick_rate_ms, before, "value should be untouched");
        // A device name has to be something.
        assert!(!SettingKey::DeviceName.apply(&mut config, "   "));
        assert_eq!(config.device_name, Config::default().device_name);
    }

    #[test]
    fn choices_round_trip_through_apply() {
        let mut config = Config::default();
        for key in [
            SettingKey::Theme,
            SettingKey::Bitrate,
            SettingKey::Normalisation,
        ] {
            let SettingKind::Choice(options) = key.kind() else {
                panic!("{key:?} should be a choice");
            };
            for option in options {
                assert!(key.apply(&mut config, option), "{option} should apply");
                assert_eq!(key.value(&config), *option);
            }
        }
    }

    #[test]
    fn editor_opens_on_the_current_value() {
        let config = Config {
            theme: "dracula".to_string(),
            ..Config::default()
        };
        let editor = ValueEditor::open(SettingKey::Theme, &config);
        assert_eq!(editor.selection(), "dracula");

        // Numbers and text are edited as a buffer seeded with the value.
        let editor = ValueEditor::open(SettingKey::DeviceName, &config);
        assert_eq!(editor.selection(), config.device_name);
    }

    #[test]
    fn reset_restores_a_single_field() {
        let mut config = Config {
            initial_volume: 12,
            device_name: "other".to_string(),
            ..Config::default()
        };

        SettingKey::InitialVolume.reset(&mut config);

        assert_eq!(config.initial_volume, Config::default().initial_volume);
        assert_eq!(config.device_name, "other", "should not touch other fields");
    }

    #[test]
    fn choice_cursor_stops_at_the_ends() {
        let config = Config::default();
        let mut editor = ValueEditor::open(SettingKey::Bitrate, &config);
        editor.move_up();
        assert_eq!(editor.selection(), "96");
        for _ in 0..10 {
            editor.move_down();
        }
        assert_eq!(editor.selection(), "320");
    }

    #[test]
    fn popup_height_matches_row_count_not_a_fixed_fraction() {
        let config = Config::default();

        // A one-line text field needs one row, regardless of terminal size.
        let editor = ValueEditor::open(SettingKey::TickRate, &config);
        assert_eq!(editor.popup_size().1, 3, "1 line + top/bottom border");

        // A choice list needs exactly its option count.
        let editor = ValueEditor::open(SettingKey::Theme, &config);
        let SettingKind::Choice(options) = SettingKey::Theme.kind() else {
            panic!("theme should be a choice");
        };
        assert_eq!(editor.popup_size().1, options.len() as u16 + 2);

        let editor = ValueEditor::open(SettingKey::Bitrate, &config);
        let SettingKind::Choice(options) = SettingKey::Bitrate.kind() else {
            panic!("bitrate should be a choice");
        };
        assert_eq!(editor.popup_size().1, options.len() as u16 + 2);
    }
}
