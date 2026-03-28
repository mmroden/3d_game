/// Configuration for side-by-side stereoscopic rendering.
#[derive(Debug, Clone)]
pub struct StereoConfig {
    pub eye_separation: f32,
    pub viewport_width: u32,
    pub viewport_height: u32,
}

impl Default for StereoConfig {
    fn default() -> Self {
        Self {
            eye_separation: 0.065,
            viewport_width: 1920,
            viewport_height: 1080,
        }
    }
}

pub fn left_eye_offset(config: &StereoConfig) -> [f32; 3] {
    [-config.eye_separation / 2.0, 0.0, 0.0]
}

pub fn right_eye_offset(config: &StereoConfig) -> [f32; 3] {
    [config.eye_separation / 2.0, 0.0, 0.0]
}

pub fn single_viewport_size(config: &StereoConfig) -> [u32; 2] {
    [config.viewport_width / 2, config.viewport_height]
}

pub fn left_viewport_rect(config: &StereoConfig) -> [u32; 4] {
    let half_w = config.viewport_width / 2;
    [0, 0, half_w, config.viewport_height]
}

pub fn right_viewport_rect(config: &StereoConfig) -> [u32; 4] {
    let half_w = config.viewport_width / 2;
    [half_w, 0, half_w, config.viewport_height]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_eye_offset_is_negative_half_separation() {
        let cfg = StereoConfig::default();
        let offset = left_eye_offset(&cfg);
        assert_eq!(offset, [-0.065 / 2.0, 0.0, 0.0]);
    }

    #[test]
    fn viewports_cover_full_screen_no_gap() {
        let cfg = StereoConfig::default();
        let l = left_viewport_rect(&cfg);
        let r = right_viewport_rect(&cfg);
        // Right starts where left ends
        assert_eq!(l[0] + l[2], r[0], "gap between viewports");
        // Total width = full width
        assert_eq!(l[2] + r[2], cfg.viewport_width, "viewports don't cover full width");
        // Heights match
        assert_eq!(l[3], cfg.viewport_height);
        assert_eq!(r[3], cfg.viewport_height);
    }

    #[test]
    fn odd_width_rounds_down_no_subpixel() {
        let cfg = StereoConfig {
            viewport_width: 1921,
            ..StereoConfig::default()
        };
        assert_eq!(single_viewport_size(&cfg), [960, 1080]);
        // Left + right = 1920, losing 1 pixel is acceptable vs subpixel
        let l = left_viewport_rect(&cfg);
        let r = right_viewport_rect(&cfg);
        assert_eq!(l[2], 960);
        assert_eq!(r[0], 960);
        assert_eq!(r[2], 960);
    }

    #[test]
    fn left_viewport_starts_at_origin() {
        let cfg = StereoConfig::default();
        assert_eq!(left_viewport_rect(&cfg), [0, 0, 960, 1080]);
    }

    #[test]
    fn right_viewport_starts_at_midpoint() {
        let cfg = StereoConfig::default();
        assert_eq!(right_viewport_rect(&cfg), [960, 0, 960, 1080]);
    }

    #[test]
    fn single_viewport_is_half_width() {
        let cfg = StereoConfig::default();
        assert_eq!(single_viewport_size(&cfg), [960, 1080]);
    }

    #[test]
    fn right_eye_offset_is_positive_half_separation() {
        let cfg = StereoConfig::default();
        let offset = right_eye_offset(&cfg);
        assert_eq!(offset, [0.065 / 2.0, 0.0, 0.0]);
    }

    #[test]
    fn default_eye_separation_is_human_ipd() {
        let cfg = StereoConfig::default();
        assert!(
            (cfg.eye_separation - 0.065).abs() < 0.001,
            "default IPD should be ~0.065m (human average), got {}",
            cfg.eye_separation
        );
    }
}
