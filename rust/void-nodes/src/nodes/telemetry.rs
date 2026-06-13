//! Telemetry: timing instrumentation for the kinetic step and the
//! render side. Jitter (p99 − p50), not the mean, is the SLO metric —
//! stutter is the manifestation of problematic behavior. Exposed as
//! editor monitors and a periodic terminal line for `make run`.

use godot::classes::{Performance, RenderingServer};
use godot::prelude::*;

use void_logic::timing::TimingWindow;

/// Custom monitor ids (graphed in the editor debugger's Monitors tab).
const MONITOR_P50: &str = "kinetics/step_ms_p50";
const MONITOR_P99: &str = "kinetics/step_ms_p99";
const MONITOR_JITTER: &str = "kinetics/step_ms_jitter";
/// Two seconds of samples at the 120 Hz tick.
const TIMING_WINDOW: usize = 240;
/// Print kinetics percentiles to the terminal every ~5 s of play.
const STATS_EVERY_TICKS: u64 = 600;

pub struct Telemetry {
    /// Per-tick physics-stage duration window.
    step: TimingWindow,
    /// Per-rendered-frame delta window: the render-side stutter
    /// detector, measured identically to the physics stage.
    frame: TimingWindow,
    /// Measured viewport render times: CPU submission and GPU
    /// completion ("when did drawing actually finish"), summed across
    /// every active 3D viewport.
    render_cpu: TimingWindow,
    render_gpu: TimingWindow,
    /// The viewports whose render time is summed each frame. In mono
    /// this is the root viewport; in SBS it is the two eye
    /// sub-viewports — never the root compositor, which only blits the
    /// two eyes and does almost no 3D work.
    viewport_rids: Vec<Rid>,
    monitors_registered: bool,
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            step: TimingWindow::new(TIMING_WINDOW),
            frame: TimingWindow::new(TIMING_WINDOW),
            render_cpu: TimingWindow::new(TIMING_WINDOW),
            render_gpu: TimingWindow::new(TIMING_WINDOW),
            viewport_rids: Vec::new(),
            monitors_registered: false,
        }
    }

    /// Measure the render time of exactly these viewports — the ones
    /// actually rendering the 3D world for the current display mode.
    /// In SBS that is the two eye sub-viewports; measuring the root
    /// compositor instead would report ~0 and hide the real cost.
    pub fn measure_viewports(&mut self, rids: &[Rid]) {
        let mut rs = RenderingServer::singleton();
        let next: Vec<Rid> = rids
            .iter()
            .copied()
            .filter(|rid| *rid != Rid::Invalid)
            .collect();
        // Retire measurement on viewports leaving the set (e.g. the eyes
        // after an SBS→mono toggle), so no orphaned GPU timer keeps
        // running on a viewport we no longer sum.
        for &rid in &self.viewport_rids {
            if !next.contains(&rid) {
                rs.viewport_set_measure_render_time(rid, false);
            }
        }
        for &rid in &next {
            rs.viewport_set_measure_render_time(rid, true);
        }
        self.viewport_rids = next;
    }

    /// The viewports whose render time is currently being summed.
    pub fn measured_viewports(&self) -> Vec<Rid> {
        self.viewport_rids.clone()
    }

    pub fn register_monitors(&mut self, p50: Callable, p99: Callable, jitter: Callable) {
        let mut perf = Performance::singleton();
        if perf.has_custom_monitor(MONITOR_P50) {
            return;
        }
        perf.add_custom_monitor(MONITOR_P50, &p50);
        perf.add_custom_monitor(MONITOR_P99, &p99);
        perf.add_custom_monitor(MONITOR_JITTER, &jitter);
        self.monitors_registered = true;
    }

    pub fn unregister_monitors(&mut self) {
        if self.monitors_registered {
            let mut perf = Performance::singleton();
            perf.remove_custom_monitor(MONITOR_P50);
            perf.remove_custom_monitor(MONITOR_P99);
            perf.remove_custom_monitor(MONITOR_JITTER);
            self.monitors_registered = false;
        }
    }

    pub fn record_step_ms(&mut self, ms: f32) {
        self.step.record(ms);
    }

    /// Record one rendered frame: its delta plus the render times of
    /// every measured viewport, summed — the honest per-frame 3D cost
    /// (both eyes in SBS).
    pub fn record_frame(&mut self, delta_ms: f32) {
        self.frame.record(delta_ms);
        if !self.viewport_rids.is_empty() {
            let rs = RenderingServer::singleton();
            let mut cpu = 0.0_f32;
            let mut gpu = 0.0_f32;
            for &rid in &self.viewport_rids {
                cpu += rs.viewport_get_measured_render_time_cpu(rid) as f32;
                gpu += rs.viewport_get_measured_render_time_gpu(rid) as f32;
            }
            self.render_cpu.record(cpu);
            self.render_gpu.record(gpu);
        }
    }

    pub fn step_ms_p50(&self) -> f32 {
        self.step.percentile(50.0)
    }

    pub fn step_ms_p99(&self) -> f32 {
        self.step.percentile(99.0)
    }

    pub fn step_ms_jitter(&self) -> f32 {
        self.step.jitter()
    }

    /// Terminal-visible instrumentation for the `make run` workflow
    /// (the editor Monitors panel graphs the same counters).
    pub fn report(&self, tick: u64) {
        if tick == 0 || tick % STATS_EVERY_TICKS != 0 {
            return;
        }
        let draw_calls = Performance::singleton()
            .get_monitor(godot::classes::performance::Monitor::RENDER_TOTAL_DRAW_CALLS_IN_FRAME);
        godot_print!(
            "kinetics: p50 {:.3} | p99 {:.3} | jit {:.3} || frame: p50 {:.2} | p99 {:.2} | jit {:.2} || draw cpu p99 {:.2} | gpu p99 {:.2} | calls {}",
            self.step.percentile(50.0),
            self.step.percentile(99.0),
            self.step.jitter(),
            self.frame.percentile(50.0),
            self.frame.percentile(99.0),
            self.frame.jitter(),
            self.render_cpu.percentile(99.0),
            self.render_gpu.percentile(99.0),
            draw_calls as i64,
        );
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}
