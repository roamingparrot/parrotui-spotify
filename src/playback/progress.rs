use std::time::Instant;

/// Tracks playback position locally so the progress bar updates smoothly
/// without polling the Spotify API every frame.
pub struct ProgressTracker {
    started_at: Option<Instant>,
    offset_ms: u64,
    pub duration_ms: u64,
    playing: bool,
}

impl ProgressTracker {
    pub fn new() -> Self {
        Self {
            started_at: None,
            offset_ms: 0,
            duration_ms: 0,
            playing: false,
        }
    }

    /// Current position in ms, interpolated from wall clock.
    pub fn position_ms(&self) -> u64 {
        let base = self.offset_ms;
        if !self.playing {
            return base;
        }
        match self.started_at {
            Some(t) => {
                let elapsed = t.elapsed().as_millis() as u64;
                (base + elapsed).min(self.duration_ms)
            }
            None => base,
        }
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// A new track started (or resumed from a known position).
    pub fn start(&mut self, position_ms: u64, duration_ms: u64) {
        self.offset_ms = position_ms;
        self.duration_ms = duration_ms;
        self.started_at = Some(Instant::now());
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.offset_ms = self.position_ms();
        self.started_at = None;
        self.playing = false;
    }

    pub fn resume(&mut self) {
        self.started_at = Some(Instant::now());
        self.playing = true;
    }

    pub fn seek(&mut self, position_ms: u64) {
        self.offset_ms = position_ms;
        if self.playing {
            self.started_at = Some(Instant::now());
        }
    }

    pub fn stop(&mut self) {
        self.offset_ms = 0;
        self.duration_ms = 0;
        self.started_at = None;
        self.playing = false;
    }
}
