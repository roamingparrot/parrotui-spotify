use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::state::App;
use crate::ui::theme::Theme;

pub fn draw(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    // If there's an active notification, show it instead of the normal status.
    if let Some(n) = &app.notification {
        let color = if n.is_error {
            theme.error_text
        } else {
            theme.hint
        };
        let msg = Paragraph::new(Span::styled(&n.message, Style::default().fg(color)))
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    let left = Span::styled(
        format!(" ♫ {}", app.device_name),
        Style::default().fg(theme.active),
    );

    let shuffle = if app.shuffle { "S" } else { "-" };
    let repeat = match app.repeat {
        crate::api::RepeatMode::Off => "off",
        crate::api::RepeatMode::Context => "all",
        crate::api::RepeatMode::Track => "one",
    };
    let right_text = format!("shuf:{shuffle}  rep:{repeat}  vol:{}% ", app.volume);
    let right = Span::styled(right_text, Style::default().fg(theme.inactive));

    // Pad middle to push right span to the edge.
    let pad = area
        .width
        .saturating_sub(left.width() as u16 + right.width() as u16);

    let line = Line::from(vec![left, Span::raw(" ".repeat(pad as usize)), right]);
    let p = Paragraph::new(line);
    frame.render_widget(p, area);
}
