use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Clear, Row, Table};

use crate::input::{Keymap, Mode};
use crate::ui::theme::Theme;
use crate::ui::util::centered_rect;

/// Collapse the keymap into one row per command, joining aliases with '/' so
/// "j" and "down" read as a single "j / ↓" entry.
fn rows_for_mode(keymap: &Keymap, mode: Mode) -> Vec<(String, &'static str)> {
    let mut rows: Vec<(String, &'static str)> = Vec::new();
    for (binding, cmd) in keymap.bindings(mode) {
        match rows.iter_mut().find(|(_, label)| *label == cmd.label()) {
            Some((keys, _)) => {
                keys.push_str(" / ");
                keys.push_str(&binding.display());
            }
            None => rows.push((binding.display(), cmd.label())),
        }
    }
    rows
}

pub fn draw(frame: &mut Frame, keymap: &Keymap, area: Rect, theme: &Theme) {
    let popup = centered_rect(60, 70, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Keybindings ")
        .title_style(
            Style::default()
                .fg(theme.active)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.active));

    let rows: Vec<Row> = rows_for_mode(keymap, Mode::Normal)
        .into_iter()
        .map(|(keys, desc)| {
            Row::new(vec![
                Cell::from(keys).style(
                    Style::default()
                        .fg(theme.selected)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(desc).style(Style::default().fg(theme.text)),
            ])
        })
        .collect();

    let widths = [Constraint::Length(16), Constraint::Min(0)];
    let table = Table::new(rows, widths).block(block);

    frame.render_widget(table, popup);
}
