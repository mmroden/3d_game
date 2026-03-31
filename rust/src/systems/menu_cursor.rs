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
    fn reset_goes_to_zero() {
        let mut c = MenuCursor::new(4);
        c.move_down();
        c.move_down();
        c.reset();
        assert_eq!(c.index(), 0);
    }
}
