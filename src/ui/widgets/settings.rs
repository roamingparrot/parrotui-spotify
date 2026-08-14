use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table,
};

use crate::input::{Keymap, Mode};
use crate::state::App;
use crate::state::settings::{SettingsState, SettingsTab, ValueEditor};
use crate::ui::theme::Theme;
use crate::ui::util::centered_rect;

pub fn draw(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let settings = app.settings.as_ref().unwrap();

    let outer = Layout::vertical([
        Constraint::Length(3), // tab bar
        Constraint::Min(0),    // rows
        Constraint::Length(1), // hint
    ])
    .split(area);

    draw_tabs(frame, settings, outer[0], theme);

    match settings.tab {
        SettingsTab::Keybindings => draw_keybindings(frame, &app.keymap, outer[1], theme),
        _ => draw_rows(frame, app, settings, outer[1], theme),
    }

    let hint = match settings.tab {
        SettingsTab::Keybindings => "  h/l switch tab   q close",
        _ => "  j/k move   Enter edit   r reset   h/l switch tab   q close",
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(theme.inactive))),
        outer[2],
    );

    if let Some(editor) = &settings.editor {
        draw_editor(frame, editor, area, theme);
    }
}

fn draw_tabs(frame: &mut Frame, settings: &SettingsState, area: Rect, theme: &Theme) {
    let block = Block::default()
        .title(" Settings ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(
            Style::default()
                .fg(theme.active)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(theme.active));

    let mut spans = Vec::new();
    for tab in SettingsTab::ALL {
        let style = if tab == settings.tab {
            Style::default()
                .fg(theme.selected)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.inactive)
        };
        spans.push(Span::styled(format!("  {}  ", tab.label()), style));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
}

fn draw_rows(frame: &mut Frame, app: &App, settings: &SettingsState, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.inactive));

    let rows: Vec<Row> = settings
        .tab
        .settings()
        .iter()
        .map(|key| {
            let note = key.note().unwrap_or("");
            Row::new(vec![
                Cell::from(format!("  {}", key.label())).style(Style::default().fg(theme.text)),
                Cell::from(key.value(&app.config)).style(
                    Style::default()
                        .fg(theme.selected)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(note).style(Style::default().fg(theme.inactive)),
            ])
        })
        .collect();

    // Leaves the note column enough room to survive an 80-column terminal.
    let widths = [
        Constraint::Length(28),
        Constraint::Length(18),
        Constraint::Min(0),
    ];
    let table = Table::new(rows, widths)
        .block(block)
        .row_highlight_style(theme.highlight_style(true))
        .highlight_symbol("▶ ");

    let mut state = ratatui::widgets::TableState::default();
    state.select(Some(settings.cursor));
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_keybindings(frame: &mut Frame, keymap: &Keymap, area: Rect, theme: &Theme) {
    let path = crate::config::keybindings_path();
    let block = Block::default()
        .title(format!(" edit {} ", path.display()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(Style::default().fg(theme.inactive))
        .border_style(Style::default().fg(theme.inactive));

    let mut rows: Vec<Row> = Vec::new();
    for mode in Mode::ALL {
        rows.push(Row::new(vec![
            Cell::from(format!("  [{}]", mode.name())).style(
                Style::default()
                    .fg(theme.active)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from(""),
        ]));

        // Join a command's aliases onto one line.
        let mut grouped: Vec<(&'static str, String)> = Vec::new();
        for (binding, cmd) in keymap.bindings(mode) {
            match grouped.iter_mut().find(|(name, _)| *name == cmd.name()) {
                Some((_, keys)) => {
                    keys.push_str(" / ");
                    keys.push_str(&binding.display());
                }
                None => grouped.push((cmd.name(), binding.display())),
            }
        }

        for (name, keys) in grouped {
            rows.push(Row::new(vec![
                Cell::from(format!("    {name}")).style(Style::default().fg(theme.text)),
                Cell::from(keys).style(Style::default().fg(theme.selected)),
            ]));
        }
    }

    let widths = [Constraint::Length(30), Constraint::Min(0)];
    frame.render_widget(Table::new(rows, widths).block(block), area);
}

fn draw_editor(frame: &mut Frame, editor: &ValueEditor, area: Rect, theme: &Theme) {
    let key = editor.key();

    let (height_pct, body) = match editor {
        ValueEditor::Choice {
            options, cursor, ..
        } => (40, Some((*options, *cursor))),
        ValueEditor::Entry { .. } => (20, None),
    };

    let popup = centered_rect(50, height_pct, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(format!(" {} ", key.label()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(
            Style::default()
                .fg(theme.active)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(theme.selected));

    match body {
        Some((options, cursor)) => {
            let items: Vec<ListItem> = options
                .iter()
                .map(|o| ListItem::new(format!("  {o}")).style(Style::default().fg(theme.text)))
                .collect();
            let list = List::new(items)
                .block(block)
                .highlight_style(theme.highlight_style(true))
                .highlight_symbol("▶ ");
            let mut state = ListState::default();
            state.select(Some(cursor));
            frame.render_stateful_widget(list, popup, &mut state);
        }
        None => {
            let buffer = editor.selection();
            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(buffer, Style::default().fg(theme.text)),
                // Caret sits at the end — text is appended and deleted there.
                Span::styled(
                    " ",
                    Style::default()
                        .fg(theme.text)
                        .add_modifier(Modifier::REVERSED),
                ),
            ]);
            frame.render_widget(Paragraph::new(line).block(block), popup);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::input::Keymap;
    use crate::state::settings::{SettingKey, SettingsState};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn app_with_settings() -> App {
        let mut app = App::new(Config::default(), Keymap::default(), "device".to_string());
        app.settings = Some(SettingsState::new());
        app
    }

    fn render(app: &mut App, width: u16, height: u16) {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| crate::ui::draw(f, app)).unwrap();
    }

    #[test]
    fn every_tab_renders() {
        let mut app = app_with_settings();
        for tab in SettingsTab::ALL {
            app.settings.as_mut().unwrap().switch_tab(tab);
            render(&mut app, 100, 40);
        }
    }

    #[test]
    fn both_editor_kinds_render() {
        let mut app = app_with_settings();
        for key in [SettingKey::Theme, SettingKey::DeviceName] {
            let editor = ValueEditor::open(key, &app.config);
            app.settings.as_mut().unwrap().editor = Some(editor);
            render(&mut app, 100, 40);
        }
    }

    #[test]
    fn renders_in_a_cramped_terminal() {
        // The playback tab has the most rows, and the editor popup is sized as
        // a percentage of the frame.
        let mut app = app_with_settings();
        app.settings
            .as_mut()
            .unwrap()
            .switch_tab(SettingsTab::Playback);
        render(&mut app, 20, 6);

        let editor = ValueEditor::open(SettingKey::Bitrate, &app.config);
        app.settings.as_mut().unwrap().editor = Some(editor);
        render(&mut app, 20, 6);
    }
}
