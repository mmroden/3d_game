//! Always-on timing instrumentation: a fixed-capacity sliding window
//! of duration samples with percentile readout. Pure and deterministic
//! — the shell feeds it measured milliseconds and surfaces the
//! percentiles (Godot custom monitors); this module never reads a
//! clock. Jitter, not the mean, is the comfort metric (players adapt
//! to constant delay, not to variance), so percentile spread is the
//! first-class output.

/// Sliding window of duration samples, in milliseconds.
#[derive(Debug, Clone)]
pub struct TimingWindow {
    samples: Vec<f32>,
    capacity: usize,
    cursor: usize,
}

impl TimingWindow {
    /// `capacity` of zero is promoted to one: a window must hold
    /// something to answer anything.
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::new(),
            capacity: capacity.max(1),
            cursor: 0,
        }
    }

    pub fn record(&mut self, milliseconds: f32) {
        if self.samples.len() < self.capacity {
            self.samples.push(milliseconds);
        } else {
            self.samples[self.cursor] = milliseconds;
        }
        self.cursor = (self.cursor + 1) % self.capacity;
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Nearest-rank percentile over the current window; `p` in
    /// [0, 100]. An empty window answers 0.0.
    pub fn percentile(&self, p: f32) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = sorted.len();
        let rank = ((p.clamp(0.0, 100.0) / 100.0) * n as f32).ceil() as usize;
        sorted[rank.clamp(1, n) - 1]
    }

    /// p99 − p50 spread: the jitter readout the SLO watches.
    pub fn jitter(&self) -> f32 {
        self.percentile(99.0) - self.percentile(50.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_window_answers_zero() {
        let window = TimingWindow::new(16);
        assert_eq!(window.percentile(50.0), 0.0);
        assert!(window.is_empty());
    }

    #[test]
    fn single_sample_is_every_percentile() {
        let mut window = TimingWindow::new(16);
        window.record(7.5);
        assert_eq!(window.percentile(1.0), 7.5);
        assert_eq!(window.percentile(50.0), 7.5);
        assert_eq!(window.percentile(99.0), 7.5);
    }

    #[test]
    fn percentiles_follow_nearest_rank() {
        let mut window = TimingWindow::new(128);
        for i in 1..=100 {
            window.record(i as f32);
        }
        assert_eq!(window.percentile(50.0), 50.0);
        assert_eq!(window.percentile(75.0), 75.0);
        assert_eq!(window.percentile(99.0), 99.0);
        assert_eq!(window.percentile(100.0), 100.0);
    }

    #[test]
    fn the_window_slides_old_samples_out() {
        let mut window = TimingWindow::new(4);
        for v in [1.0, 2.0, 3.0, 4.0] {
            window.record(v);
        }
        for _ in 0..4 {
            window.record(10.0);
        }
        assert_eq!(
            window.percentile(50.0),
            10.0,
            "fully displaced window must only see the new samples"
        );
    }

    #[test]
    fn jitter_is_the_p99_p50_spread() {
        // 50 samples: nearest-rank p99 = ceil(0.99 * 50) = rank 50,
        // which is the straggler.
        let mut window = TimingWindow::new(128);
        for _ in 0..49 {
            window.record(8.0);
        }
        window.record(12.0);
        assert!(
            (window.jitter() - 4.0).abs() < 0.01,
            "one straggler against a steady 8 ms gives ~4 ms spread, got {}",
            window.jitter()
        );
    }
}
