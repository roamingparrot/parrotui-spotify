use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub fn draw(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 70, area);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Keybindings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            " Navigation",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("   j/↓       Move down"),
        Line::from("   k/↑       Move up"),
        Line::from("   gg        Jump to top"),
        Line::from("   G         Jump to bottom"),
        Line::from("   l/→/Enter Select / expand"),
        Line::from("   h/←       Go back"),
        Line::from("   Tab       Switch panel"),
        Line::from("   Esc       Back to sidebar"),
        Line::from(""),
        Line::from(Span::styled(
            " Playback",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("   Space     Play / pause"),
        Line::from("   n         Next track"),
        Line::from("   p         Previous track"),
        Line::from("   +/-       Volume up/down"),
        Line::from("   >/<       Seek forward/back"),
        Line::from("   s         Toggle shuffle"),
        Line::from("   r         Cycle repeat"),
        Line::from(""),
        Line::from(Span::styled(
            " General",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("   ?         Toggle this help"),
        Line::from("   q         Quit"),
        Line::from(""),
    ];

    let help = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(help, popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1])[1]
}
