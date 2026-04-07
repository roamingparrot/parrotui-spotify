use ratatui::style::{Color, Modifier, Style};

#[derive(Copy, Clone, Debug)]
pub struct Theme {
    pub active: Color,
    pub selected: Color,
    pub hovered: Color,
    pub inactive: Color,
    pub text: Color,
    pub header: Color,
    pub error_text: Color,
    pub hint: Color,
    pub playbar_background: Color,
    pub playbar_progress: Color,
    pub playbar_progress_text: Color,
    pub playbar_text: Color,
}

impl Theme {
    pub fn base_style(&self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn highlight_style(&self, is_active: bool) -> Style {
        let color = if is_active {
            self.selected
        } else {
            self.hovered
        };
        Style::default()
            .fg(color)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::cyan()
    }
}

impl Theme {
    /// Default cyan theme — matches the original spotatui default.
    pub fn cyan() -> Self {
        Theme {
            active: Color::Rgb(0, 180, 180),
            selected: Color::Rgb(0, 200, 200),
            hovered: Color::Rgb(180, 0, 180),
            inactive: Color::Rgb(128, 128, 128),
            text: Color::Reset,
            header: Color::Reset,
            error_text: Color::Rgb(255, 100, 100),
            hint: Color::Rgb(200, 200, 0),
            playbar_background: Color::Reset,
            playbar_progress: Color::Rgb(0, 200, 200),
            playbar_progress_text: Color::Rgb(255, 255, 255),
            playbar_text: Color::Reset,
        }
    }

    pub fn spotify() -> Self {
        let green = Color::Rgb(29, 185, 84);
        Theme {
            active: green,
            selected: green,
            hovered: Color::Rgb(180, 180, 180),
            inactive: Color::Rgb(100, 100, 100),
            text: Color::Rgb(255, 255, 255),
            header: Color::Rgb(180, 180, 180),
            error_text: Color::Rgb(255, 100, 100),
            hint: Color::Rgb(200, 200, 0),
            playbar_background: Color::Rgb(24, 24, 24),
            playbar_progress: green,
            playbar_progress_text: Color::Rgb(255, 255, 255),
            playbar_text: Color::Rgb(180, 180, 180),
        }
    }

    pub fn dracula() -> Self {
        let purple = Color::Rgb(189, 147, 249);
        let green = Color::Rgb(80, 250, 123);
        Theme {
            active: purple,
            selected: green,
            hovered: Color::Rgb(255, 121, 198),
            inactive: Color::Rgb(98, 114, 164),
            text: Color::Rgb(248, 248, 242),
            header: Color::Rgb(98, 114, 164),
            error_text: Color::Rgb(255, 85, 85),
            hint: Color::Rgb(241, 250, 140),
            playbar_background: Color::Rgb(40, 42, 54),
            playbar_progress: purple,
            playbar_progress_text: Color::Rgb(248, 248, 242),
            playbar_text: Color::Rgb(189, 147, 249),
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "spotify" => Self::spotify(),
            "dracula" => Self::dracula(),
            _ => Self::cyan(),
        }
    }
}

/// Pick a Style based on (is_active, is_hovered) state.
pub fn panel_style(is_active: bool, is_hovered: bool, theme: &Theme) -> Style {
    match (is_active, is_hovered) {
        (true, _) => Style::default().fg(theme.selected),
        (false, true) => Style::default().fg(theme.hovered),
        _ => Style::default().fg(theme.inactive),
    }
}
