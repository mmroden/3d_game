/// The rows of a `len`-row list that a `visible`-row window shows, given
/// the row the cursor is on — `None` when the cursor is in another list.
/// Stateless: the window centers on the focused row and clamps at the
/// ends, so the same cursor always shows the same rows (a refresh cannot
/// jump the view). A list that fits shows whole; an unfocused list shows
/// its top.
pub fn window(len: usize, focus: Option<usize>, visible: usize) -> std::ops::Range<usize> {
    let visible = visible.max(1).min(len);
    let start = match focus {
        Some(focus) => focus.saturating_sub(visible / 2).min(len - visible),
        None => 0,
    };
    start..start + visible
}

/// Wrapping cursor for menu navigation.
/// Pure data — no Godot dependency.
pub struct MenuCursor {
    index: usize,
    count: usize,
}

impl MenuCursor {
    pub fn new(count: usize) -> Self {
        Self { index: 0, count }
    }

    pub fn new_at(index: usize, count: usize) -> Self {
        debug_assert!(index < count, "initial index must be < count");
        Self { index, count }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn move_up(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.count - 1;
        }
    }

    pub fn move_down(&mut self) {
        self.index = (self.index + 1) % self.count;
    }

    pub fn reset(&mut self) {
        self.index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero() {
        let c = MenuCursor::new(4);
        assert_eq!(c.index(), 0);
    }

    #[test]
    fn move_down_increments() {
        let mut c = MenuCursor::new(4);
        c.move_down();
        assert_eq!(c.index(), 1);
    }

    #[test]
    fn move_down_wraps_to_zero() {
        let mut c = MenuCursor::new(3);
        c.move_down();
        c.move_down();
        c.move_down();
        assert_eq!(c.index(), 0);
    }

    #[test]
    fn move_up_wraps_to_last() {
        let mut c = MenuCursor::new(4);
        c.move_up();
        assert_eq!(c.index(), 3);
    }

    #[test]
    fn move_up_decrements() {
        let mut c = MenuCursor::new(4);
        c.move_down();
        c.move_down();
        c.move_up();
        assert_eq!(c.index(), 1);
    }


    #[test]
    fn a_list_that_fits_shows_whole_and_an_unfocused_list_shows_its_top() {
        assert_eq!(window(3, Some(2), 4), 0..3);
        assert_eq!(window(3, None, 4), 0..3);
        assert_eq!(window(10, None, 4), 0..4);
        assert_eq!(window(0, None, 4), 0..0);
    }

    #[test]
    fn the_window_centers_on_the_cursor_and_clamps_at_the_ends() {
        assert_eq!(window(10, Some(0), 4), 0..4);
        assert_eq!(window(10, Some(1), 4), 0..4);
        assert_eq!(window(10, Some(2), 4), 0..4, "centered: two rows above, one below");
        assert_eq!(window(10, Some(3), 4), 1..5);
        assert_eq!(window(10, Some(7), 4), 5..9);
        assert_eq!(window(10, Some(8), 4), 6..10, "clamped at the end");
        assert_eq!(window(10, Some(9), 4), 6..10);
        for focus in 0..10 {
            let w = window(10, Some(focus), 4);
            assert!(w.contains(&focus), "the cursor row is always shown (focus {focus} -> {w:?})");
            assert_eq!(w.len(), 4);
        }
    }

    #[test]
    fn a_window_is_at_least_one_row() {
        assert_eq!(window(5, Some(3), 0), 3..4);
        assert_eq!(window(5, None, 0), 0..1);
    }

    #[test]
    fn reset_goes_to_zero() {
        let mut c = MenuCursor::new(4);
        c.move_down();
        c.move_down();
        c.reset();
        assert_eq!(c.index(), 0);
    }
}
