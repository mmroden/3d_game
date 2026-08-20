/// System-wide rendering/display options that persist across game sessions.
///
/// All default off: SBS is opt-in, 4× MSAA costs ~5–10ms/frame in SBS so
/// players turn it on only on beefier machines, and dynamic stereo (the
/// stereo director's convergence + interaxial tracking, experiment v2) is
/// judged against the static baseline in glasses before it earns a
/// default.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameOptions {
    pub sbs_enabled: bool,
    pub msaa_enabled: bool,
    pub dynamic_stereo: bool,
}

impl GameOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle_sbs(&mut self) -> bool {
        self.sbs_enabled = !self.sbs_enabled;
        self.sbs_enabled
    }

    pub fn toggle_msaa(&mut self) -> bool {
        self.msaa_enabled = !self.msaa_enabled;
        self.msaa_enabled
    }

    pub fn toggle_dynamic_stereo(&mut self) -> bool {
        self.dynamic_stereo = !self.dynamic_stereo;
        self.dynamic_stereo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_sbs_off() {
        let opts = GameOptions::new();
        assert!(!opts.sbs_enabled);
    }

    #[test]
    fn defaults_msaa_off() {
        // MSAA is opt-in: too costly in SBS to enable by default.
        let opts = GameOptions::new();
        assert!(!opts.msaa_enabled);
    }

    #[test]
    fn defaults_dynamic_stereo_off() {
        // The stereo director is an experiment: the static baseline is the
        // control condition, so the dynamic mode never ships on by default
        // until it wins in glasses.
        let opts = GameOptions::new();
        assert!(!opts.dynamic_stereo);
    }

    #[test]
    fn toggle_sbs_returns_new_state() {
        let mut opts = GameOptions::new();
        let result = opts.toggle_sbs();
        assert!(result);
        assert!(opts.sbs_enabled);
    }

    #[test]
    fn toggle_sbs_twice_roundtrips() {
        let mut opts = GameOptions::new();
        opts.toggle_sbs();
        let result = opts.toggle_sbs();
        assert!(!result);
        assert!(!opts.sbs_enabled);
    }

    #[test]
    fn toggle_msaa_returns_new_state() {
        let mut opts = GameOptions::new();
        let result = opts.toggle_msaa();
        assert!(result);
        assert!(opts.msaa_enabled);
    }

    #[test]
    fn toggle_msaa_twice_roundtrips() {
        let mut opts = GameOptions::new();
        opts.toggle_msaa();
        let result = opts.toggle_msaa();
        assert!(!result);
        assert!(!opts.msaa_enabled);
    }

    #[test]
    fn toggle_dynamic_stereo_returns_new_state() {
        let mut opts = GameOptions::new();
        let result = opts.toggle_dynamic_stereo();
        assert!(result);
        assert!(opts.dynamic_stereo);
    }

    #[test]
    fn toggle_dynamic_stereo_twice_roundtrips() {
        let mut opts = GameOptions::new();
        opts.toggle_dynamic_stereo();
        let result = opts.toggle_dynamic_stereo();
        assert!(!result);
        assert!(!opts.dynamic_stereo);
    }
}
