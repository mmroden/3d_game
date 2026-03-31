//! Pure layout helpers for UI positioning.
//! No Godot dependency — fully testable.

/// Corner identifiers for UI element placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomCenter,
}

/// Returns `[left, top, right, bottom]` margins to center a container
/// of `content_w x content_h` inside a `viewport_w x viewport_h` viewport.
pub fn centered_container_margins(
    viewport_w: f32,
    viewport_h: f32,
    content_w: f32,
    content_h: f32,
) -> [f32; 4] {
    let h_margin = (viewport_w - content_w) / 2.0;
    let v_margin = (viewport_h - content_h) / 2.0;
    [h_margin, v_margin, h_margin, v_margin]
}

/// Returns `[x, y]` position for an element at the given corner,
/// inset by `margin` pixels from the edge.
/// For `TopRight`, x is measured from the right edge (returns `viewport_w - margin`).
/// For `BottomCenter`, returns centered x at `viewport_h - margin` y.
pub fn corner_offset(
    viewport_w: f32,
    viewport_h: f32,
    corner: Corner,
    margin: f32,
) -> [f32; 2] {
    match corner {
        Corner::TopLeft => [margin, margin],
        Corner::TopRight => [viewport_w - margin, margin],
        Corner::BottomCenter => [viewport_w / 2.0, viewport_h - margin],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_margins_1920x1080_600x500() {
        let [l, t, r, b] = centered_container_margins(1920.0, 1080.0, 600.0, 500.0);
        assert_eq!(l, 660.0);
        assert_eq!(t, 290.0);
        assert_eq!(r, 660.0);
        assert_eq!(b, 290.0);
    }

    #[test]
    fn centered_margins_symmetric() {
        let [l, t, r, b] = centered_container_margins(800.0, 600.0, 400.0, 300.0);
        assert_eq!(l, r);
        assert_eq!(t, b);
        assert_eq!(l, 200.0);
        assert_eq!(t, 150.0);
    }

    #[test]
    fn centered_margins_full_size_content() {
        let [l, t, r, b] = centered_container_margins(1920.0, 1080.0, 1920.0, 1080.0);
        assert_eq!(l, 0.0);
        assert_eq!(t, 0.0);
        assert_eq!(r, 0.0);
        assert_eq!(b, 0.0);
    }

    #[test]
    fn corner_top_left() {
        let [x, y] = corner_offset(1920.0, 1080.0, Corner::TopLeft, 20.0);
        assert_eq!(x, 20.0);
        assert_eq!(y, 20.0);
    }

    #[test]
    fn corner_top_right() {
        let [x, y] = corner_offset(1920.0, 1080.0, Corner::TopRight, 20.0);
        assert_eq!(x, 1900.0);
        assert_eq!(y, 20.0);
    }

    #[test]
    fn corner_bottom_center() {
        let [x, y] = corner_offset(1920.0, 1080.0, Corner::BottomCenter, 40.0);
        assert_eq!(x, 960.0);
        assert_eq!(y, 1040.0);
    }

    #[test]
    fn corner_offset_with_zero_margin() {
        let [x, y] = corner_offset(1920.0, 1080.0, Corner::TopLeft, 0.0);
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
    }
}
