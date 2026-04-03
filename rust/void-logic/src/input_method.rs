/// Which input device is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMethod {
    Keyboard,
    Controller,
}

impl InputMethod {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Keyboard => "Keyboard",
            Self::Controller => "Controller",
        }
    }
}

impl std::fmt::Display for InputMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_keyboard() {
        assert_eq!(InputMethod::Keyboard.display_name(), "Keyboard");
    }

    #[test]
    fn display_name_controller() {
        assert_eq!(InputMethod::Controller.display_name(), "Controller");
    }

    #[test]
    fn display_trait_matches() {
        assert_eq!(format!("{}", InputMethod::Keyboard), "Keyboard");
        assert_eq!(format!("{}", InputMethod::Controller), "Controller");
    }

    #[test]
    fn equality() {
        assert_eq!(InputMethod::Keyboard, InputMethod::Keyboard);
        assert_eq!(InputMethod::Controller, InputMethod::Controller);
        assert_ne!(InputMethod::Keyboard, InputMethod::Controller);
    }

    #[test]
    fn clone_and_copy() {
        let a = InputMethod::Controller;
        let b = a;
        assert_eq!(a, b);
    }
}
