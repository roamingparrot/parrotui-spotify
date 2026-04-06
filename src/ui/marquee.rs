use unicode_width::UnicodeWidthChar;

/// Extract a display-width window from `s` starting at column `offset`,
/// returning exactly `visible_width` columns (padded with spaces if needed).
pub fn marquee_text(s: &str, visible_width: usize, offset: usize) -> String {
    let mut result = String::new();
    let mut col = 0; // current column in the full string
    let mut visible_cols = 0;

    for ch in s.chars() {
        let w = ch.width().unwrap_or(0);
        if col + w > offset && visible_cols < visible_width {
            // Character starts within or enters the visible window.
            result.push(ch);
            visible_cols += w;
        } else if col >= offset && visible_cols >= visible_width {
            break;
        }
        col += w;
    }

    // Pad to exact width if the text ran out.
    while visible_cols < visible_width {
        result.push(' ');
        visible_cols += 1;
    }

    result
}

/// Unicode-safe truncation: if `s` exceeds `max_width` display columns,
/// cut it and append "…". Returns the original string if it fits.
pub fn truncate_unicode(s: &str, max_width: usize) -> String {
    let mut width = 0;
    for (i, ch) in s.char_indices() {
        let w = ch.width().unwrap_or(0);
        if width + w > max_width {
            // Doesn't fit — truncate with ellipsis.
            // Find a cut point that leaves room for "…" (1 column).
            let ellipsis_width = 1;
            let target = max_width.saturating_sub(ellipsis_width);
            let mut cut_width = 0;
            let mut cut_byte = 0;
            for (j, c) in s.char_indices() {
                let cw = c.width().unwrap_or(0);
                if cut_width + cw > target {
                    break;
                }
                cut_width += cw;
                cut_byte = j + c.len_utf8();
            }
            return format!("{}…", &s[..cut_byte]);
        }
        width += w;
        // Early exit: we've measured past the point of interest.
        let _ = i; // suppress unused warning
    }
    s.to_string()
}
