use std::time::{Duration, Instant};

const PAUSE_DURATION: Duration = Duration::from_millis(1500);
const SCROLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Pausing at the start — full text visible from offset 0.
    PauseStart,
    /// Scrolling one column at a time.
    Scrolling,
    /// Pausing at the end — tail of text visible.
    PauseEnd,
    /// Text fits, no animation needed.
    Idle,
}

/// Tracks horizontal scroll animation for a single selected item.
///
/// Call `tick()` once per frame with the current cursor index, text width,
/// and visible width. It returns the column offset to use when rendering.
/// When the cursor moves to a different item, the animation auto-resets.
pub struct MarqueeState {
    active_id: Option<usize>,
    offset: usize,
    phase: Phase,
    phase_start: Instant,
}

impl MarqueeState {
    pub fn new() -> Self {
        Self {
            active_id: None,
            offset: 0,
            phase: Phase::Idle,
            phase_start: Instant::now(),
        }
    }

    /// Advance the animation and return the current scroll offset.
    ///
    /// - `id`: cursor index of the item being rendered (used to detect cursor movement)
    /// - `text_width`: display-column width of the full text
    /// - `visible_width`: how many columns the viewport can show
    pub fn tick(&mut self, id: usize, text_width: usize, visible_width: usize) -> usize {
        // Text fits — no scrolling needed.
        if text_width <= visible_width {
            self.phase = Phase::Idle;
            self.offset = 0;
            self.active_id = Some(id);
            return 0;
        }

        // Cursor moved to a different item — reset animation.
        if self.active_id != Some(id) {
            self.active_id = Some(id);
            self.offset = 0;
            self.phase = Phase::PauseStart;
            self.phase_start = Instant::now();
            return 0;
        }

        let max_offset = text_width - visible_width;
        let elapsed = self.phase_start.elapsed();

        match self.phase {
            Phase::Idle => 0,
            Phase::PauseStart => {
                if elapsed >= PAUSE_DURATION {
                    self.phase = Phase::Scrolling;
                    self.phase_start = Instant::now();
                }
                0
            }
            Phase::Scrolling => {
                if elapsed >= SCROLL_INTERVAL {
                    self.offset += 1;
                    self.phase_start = Instant::now();
                }
                if self.offset >= max_offset {
                    self.offset = max_offset;
                    self.phase = Phase::PauseEnd;
                    self.phase_start = Instant::now();
                }
                self.offset
            }
            Phase::PauseEnd => {
                if elapsed >= PAUSE_DURATION {
                    self.offset = 0;
                    self.phase = Phase::PauseStart;
                    self.phase_start = Instant::now();
                }
                self.offset
            }
        }
    }

    /// Force-reset to initial state.
    pub fn reset(&mut self) {
        self.active_id = None;
        self.offset = 0;
        self.phase = Phase::Idle;
        self.phase_start = Instant::now();
    }
}
