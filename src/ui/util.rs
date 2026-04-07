use ratatui::layout::{Constraint, Layout, Rect};

pub fn millis_to_minutes(ms: u64) -> String {
    let minutes = ms / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    format!("{minutes}:{seconds:02}")
}

pub fn track_progress_label(pos_ms: u64, dur_ms: u64) -> String {
    let pos = millis_to_minutes(pos_ms);
    let dur = millis_to_minutes(dur_ms);
    let rem = millis_to_minutes(dur_ms.saturating_sub(pos_ms));
    format!("{pos}/{dur} (-{rem})")
}

pub fn track_progress_ratio(pos_ms: u64, dur_ms: u64) -> f64 {
    if dur_ms == 0 {
        return 0.0;
    }
    (pos_ms as f64 / dur_ms as f64).clamp(0.0, 1.0)
}

pub fn centered_rect(pct_w: u16, pct_h: u16, area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - pct_h) / 2),
        Constraint::Percentage(pct_h),
        Constraint::Percentage((100 - pct_h) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - pct_w) / 2),
        Constraint::Percentage(pct_w),
        Constraint::Percentage((100 - pct_w) / 2),
    ])
    .split(v[1])[1]
}
