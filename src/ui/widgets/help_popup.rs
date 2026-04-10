use ratatui::layout::Constraint;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Clear, Row, Table};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::ui::theme::Theme;
use crate::ui::util::centered_rect;

const BINDINGS: &[(&str, &str)] = &[
    ("j / ↓", "Move down"),
    ("k / ↑", "Move up"),
    ("g g", "Jump to top"),
    ("G", "Jump to bottom"),
    ("l / →", "Move focus right"),
    ("Enter", "Select / expand"),
    ("h / ← / Esc", "Go back"),
    ("Tab", "Switch panel"),
    ("", ""),
    ("Space", "Play / pause"),
    ("n", "Next track"),
    ("p", "Previous track"),
    ("+ / =", "Volume up"),
    ("- ", "Volume down"),
    ("> / .", "Seek forward"),
    ("< / ,", "Seek backward"),
    ("s", "Toggle shuffle"),
    ("r", "Cycle repeat"),
    ("", ""),
    ("?", "Toggle help"),
    ("q", "Quit"),
];

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme) {
    let popup = centered_rect(60, 70, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Keybindings ")
        .title_style(Style::default().fg(theme.active).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.active));

    let rows: Vec<Row> = BINDINGS
        .iter()
        .map(|(key, desc)| {
            Row::new(vec![
                Cell::from(*key).style(
                    Style::default()
                        .fg(theme.selected)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(*desc).style(Style::default().fg(theme.text)),
            ])
        })
        .collect();

    let widths = [Constraint::Length(16), Constraint::Min(0)];
    let table = Table::new(rows, widths).block(block);

    frame.render_widget(table, popup);
}
