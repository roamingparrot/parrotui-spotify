use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::state::SearchState;
use crate::ui::theme::Theme;

pub fn draw(frame: &mut Frame, search: &SearchState, area: Rect, theme: &Theme) {
    let border_color = if search.input_active {
        theme.selected
    } else {
        theme.inactive
    };

    let block = Block::default()
        .title(" Search ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title_style(Style::default().fg(border_color))
        .border_style(Style::default().fg(border_color));

    let content = if search.loading {
        Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.active)),
            Span::styled(&search.query, Style::default().fg(theme.text)),
            Span::styled("  searching...", Style::default().fg(theme.inactive)),
        ])
    } else if search.input_active {
        let cursor_pos = search.input_cursor.min(search.query.len());
        let before = &search.query[..cursor_pos];
        if cursor_pos < search.query.len() {
            let ch_end = search.query[cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| cursor_pos + i)
                .unwrap_or(search.query.len());
            let cursor_ch = &search.query[cursor_pos..ch_end];
            let after = &search.query[ch_end..];
            Line::from(vec![
                Span::styled(" / ", Style::default().fg(theme.active)),
                Span::raw(before),
                Span::styled(
                    cursor_ch,
                    Style::default()
                        .fg(theme.text)
                        .add_modifier(Modifier::REVERSED),
                ),
                Span::raw(after),
            ])
        } else {
            Line::from(vec![
                Span::styled(" / ", Style::default().fg(theme.active)),
                Span::raw(before),
                Span::styled(
                    " ",
                    Style::default()
                        .fg(theme.text)
                        .add_modifier(Modifier::REVERSED),
                ),
            ])
        }
    } else {
        Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.active)),
            Span::styled(&search.query, Style::default().fg(theme.text)),
        ])
    };

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);
}
