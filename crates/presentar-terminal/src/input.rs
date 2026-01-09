//! Input handling for terminal applications.

use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};
use presentar_core::{Event, Key, MouseButton, Point};

/// Key binding configuration.
#[derive(Debug, Clone)]
pub struct KeyBinding {
    /// Key code.
    pub code: KeyCode,
    /// Required modifiers.
    pub modifiers: KeyModifiers,
    /// Action name.
    pub action: String,
}

impl KeyBinding {
    /// Create a new key binding.
    #[must_use]
    pub fn new(code: KeyCode, modifiers: KeyModifiers, action: impl Into<String>) -> Self {
        Self {
            code,
            modifiers,
            action: action.into(),
        }
    }

    /// Create a simple key binding without modifiers.
    #[must_use]
    pub fn simple(code: KeyCode, action: impl Into<String>) -> Self {
        Self::new(code, KeyModifiers::NONE, action)
    }

    /// Check if this binding matches a key event.
    #[must_use]
    pub fn matches(&self, event: &KeyEvent) -> bool {
        event.code == self.code && event.modifiers.contains(self.modifiers)
    }
}

/// Input handler for converting crossterm events to presentar events.
#[derive(Debug, Default)]
pub struct InputHandler {
    bindings: Vec<KeyBinding>,
}

impl InputHandler {
    /// Create a new input handler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a key binding.
    pub fn add_binding(&mut self, binding: KeyBinding) {
        self.bindings.push(binding);
    }

    /// Convert a crossterm event to a presentar event.
    #[must_use]
    pub fn convert(&self, event: CrosstermEvent) -> Option<Event> {
        match event {
            CrosstermEvent::Key(key) => self.convert_key(key),
            CrosstermEvent::Mouse(mouse) => Some(self.convert_mouse(mouse)),
            CrosstermEvent::Resize(width, height) => Some(Event::Resize {
                width: f32::from(width),
                height: f32::from(height),
            }),
            CrosstermEvent::FocusGained => Some(Event::FocusIn),
            CrosstermEvent::FocusLost => Some(Event::FocusOut),
            CrosstermEvent::Paste(text) => Some(Event::TextInput { text }),
        }
    }

    fn convert_key(&self, key: KeyEvent) -> Option<Event> {
        let presentar_key = match key.code {
            KeyCode::Char('a' | 'A') => Key::A,
            KeyCode::Char('b' | 'B') => Key::B,
            KeyCode::Char('c' | 'C') => Key::C,
            KeyCode::Char('d' | 'D') => Key::D,
            KeyCode::Char('e' | 'E') => Key::E,
            KeyCode::Char('f' | 'F') => Key::F,
            KeyCode::Char('g' | 'G') => Key::G,
            KeyCode::Char('h' | 'H') => Key::H,
            KeyCode::Char('i' | 'I') => Key::I,
            KeyCode::Char('j' | 'J') => Key::J,
            KeyCode::Char('k' | 'K') => Key::K,
            KeyCode::Char('l' | 'L') => Key::L,
            KeyCode::Char('m' | 'M') => Key::M,
            KeyCode::Char('n' | 'N') => Key::N,
            KeyCode::Char('o' | 'O') => Key::O,
            KeyCode::Char('p' | 'P') => Key::P,
            KeyCode::Char('q' | 'Q') => Key::Q,
            KeyCode::Char('r' | 'R') => Key::R,
            KeyCode::Char('s' | 'S') => Key::S,
            KeyCode::Char('t' | 'T') => Key::T,
            KeyCode::Char('u' | 'U') => Key::U,
            KeyCode::Char('v' | 'V') => Key::V,
            KeyCode::Char('w' | 'W') => Key::W,
            KeyCode::Char('x' | 'X') => Key::X,
            KeyCode::Char('y' | 'Y') => Key::Y,
            KeyCode::Char('z' | 'Z') => Key::Z,
            KeyCode::Char('0') => Key::Num0,
            KeyCode::Char('1') => Key::Num1,
            KeyCode::Char('2') => Key::Num2,
            KeyCode::Char('3') => Key::Num3,
            KeyCode::Char('4') => Key::Num4,
            KeyCode::Char('5') => Key::Num5,
            KeyCode::Char('6') => Key::Num6,
            KeyCode::Char('7') => Key::Num7,
            KeyCode::Char('8') => Key::Num8,
            KeyCode::Char('9') => Key::Num9,
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Escape,
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Tab => Key::Tab,
            KeyCode::Delete => Key::Delete,
            KeyCode::Insert => Key::Insert,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            KeyCode::F(1) => Key::F1,
            KeyCode::F(2) => Key::F2,
            KeyCode::F(3) => Key::F3,
            KeyCode::F(4) => Key::F4,
            KeyCode::F(5) => Key::F5,
            KeyCode::F(6) => Key::F6,
            KeyCode::F(7) => Key::F7,
            KeyCode::F(8) => Key::F8,
            KeyCode::F(9) => Key::F9,
            KeyCode::F(10) => Key::F10,
            KeyCode::F(11) => Key::F11,
            KeyCode::F(12) => Key::F12,
            KeyCode::Char(' ') => Key::Space,
            KeyCode::Char('-') => Key::Minus,
            KeyCode::Char('=') => Key::Equal,
            KeyCode::Char('[') => Key::BracketLeft,
            KeyCode::Char(']') => Key::BracketRight,
            KeyCode::Char('\\') => Key::Backslash,
            KeyCode::Char(';') => Key::Semicolon,
            KeyCode::Char('\'') => Key::Quote,
            KeyCode::Char('`') => Key::Grave,
            KeyCode::Char(',') => Key::Comma,
            KeyCode::Char('.') => Key::Period,
            KeyCode::Char('/') => Key::Slash,
            // Unknown keys are ignored
            _ => return None,
        };

        Some(Event::KeyDown { key: presentar_key })
    }

    fn convert_mouse(&self, mouse: crossterm::event::MouseEvent) -> Event {
        use crossterm::event::{MouseButton as CtMouseButton, MouseEventKind};

        let position = Point::new(f32::from(mouse.column), f32::from(mouse.row));

        match mouse.kind {
            MouseEventKind::Down(button) => Event::MouseDown {
                position,
                button: match button {
                    CtMouseButton::Left => MouseButton::Left,
                    CtMouseButton::Right => MouseButton::Right,
                    CtMouseButton::Middle => MouseButton::Middle,
                },
            },
            MouseEventKind::Up(button) => Event::MouseUp {
                position,
                button: match button {
                    CtMouseButton::Left => MouseButton::Left,
                    CtMouseButton::Right => MouseButton::Right,
                    CtMouseButton::Middle => MouseButton::Middle,
                },
            },
            MouseEventKind::Moved | MouseEventKind::Drag(_) => Event::MouseMove { position },
            MouseEventKind::ScrollUp => Event::Scroll {
                delta_x: 0.0,
                delta_y: -1.0,
            },
            MouseEventKind::ScrollDown => Event::Scroll {
                delta_x: 0.0,
                delta_y: 1.0,
            },
            MouseEventKind::ScrollLeft => Event::Scroll {
                delta_x: -1.0,
                delta_y: 0.0,
            },
            MouseEventKind::ScrollRight => Event::Scroll {
                delta_x: 1.0,
                delta_y: 0.0,
            },
        }
    }

    /// Find a matching binding for a key event.
    #[must_use]
    pub fn find_binding(&self, event: &KeyEvent) -> Option<&KeyBinding> {
        self.bindings.iter().find(|b| b.matches(event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{MouseButton as CtMouseButton, MouseEvent, MouseEventKind};

    #[test]
    fn test_key_binding_simple() {
        let binding = KeyBinding::simple(KeyCode::Char('q'), "quit");
        assert_eq!(binding.action, "quit");
        assert_eq!(binding.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_key_binding_with_modifiers() {
        let binding = KeyBinding::new(KeyCode::Char('c'), KeyModifiers::CONTROL, "copy");
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(binding.matches(&event));
    }

    #[test]
    fn test_key_binding_no_match() {
        let binding = KeyBinding::new(KeyCode::Char('c'), KeyModifiers::CONTROL, "copy");
        let event = KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL);
        assert!(!binding.matches(&event));
    }

    #[test]
    fn test_input_handler_add_binding() {
        let mut handler = InputHandler::new();
        handler.add_binding(KeyBinding::simple(KeyCode::Char('q'), "quit"));
        assert_eq!(handler.bindings.len(), 1);
    }

    #[test]
    fn test_input_handler_find_binding() {
        let mut handler = InputHandler::new();
        handler.add_binding(KeyBinding::simple(KeyCode::Char('q'), "quit"));
        handler.add_binding(KeyBinding::new(
            KeyCode::Char('s'),
            KeyModifiers::CONTROL,
            "save",
        ));

        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let binding = handler.find_binding(&event);
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().action, "quit");

        let event2 = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(handler.find_binding(&event2).is_none());
    }

    #[test]
    fn test_convert_letter_keys() {
        let handler = InputHandler::new();
        for ch in 'a'..='z' {
            let event = CrosstermEvent::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            let result = handler.convert(event);
            assert!(result.is_some());
        }
        for ch in 'A'..='Z' {
            let event = CrosstermEvent::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            let result = handler.convert(event);
            assert!(result.is_some());
        }
    }

    #[test]
    fn test_convert_number_keys() {
        let handler = InputHandler::new();
        for ch in '0'..='9' {
            let event = CrosstermEvent::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            let result = handler.convert(event);
            assert!(result.is_some());
        }
    }

    #[test]
    fn test_convert_special_keys() {
        let handler = InputHandler::new();
        let special_keys = [
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Backspace,
            KeyCode::Tab,
            KeyCode::Delete,
            KeyCode::Insert,
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Home,
            KeyCode::End,
            KeyCode::PageUp,
            KeyCode::PageDown,
        ];
        for key in special_keys {
            let event = CrosstermEvent::Key(KeyEvent::new(key, KeyModifiers::NONE));
            let result = handler.convert(event);
            assert!(result.is_some(), "Failed for {:?}", key);
        }
    }

    #[test]
    fn test_convert_function_keys() {
        let handler = InputHandler::new();
        for n in 1..=12 {
            let event = CrosstermEvent::Key(KeyEvent::new(KeyCode::F(n), KeyModifiers::NONE));
            let result = handler.convert(event);
            assert!(result.is_some(), "Failed for F{}", n);
        }
    }

    #[test]
    fn test_convert_punctuation_keys() {
        let handler = InputHandler::new();
        let punct = [' ', '-', '=', '[', ']', '\\', ';', '\'', '`', ',', '.', '/'];
        for ch in punct {
            let event = CrosstermEvent::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            let result = handler.convert(event);
            assert!(result.is_some(), "Failed for {:?}", ch);
        }
    }

    #[test]
    fn test_convert_unknown_key() {
        let handler = InputHandler::new();
        let event = CrosstermEvent::Key(KeyEvent::new(KeyCode::Char('£'), KeyModifiers::NONE));
        let result = handler.convert(event);
        assert!(result.is_none());
    }

    #[test]
    fn test_convert_mouse_down() {
        let handler = InputHandler::new();
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(CtMouseButton::Left),
            column: 10,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        let event = CrosstermEvent::Mouse(mouse);
        let result = handler.convert(event).unwrap();
        assert!(
            matches!(result, Event::MouseDown { position, button: MouseButton::Left } if position.x == 10.0 && position.y == 5.0)
        );
    }

    #[test]
    fn test_convert_mouse_up() {
        let handler = InputHandler::new();
        let mouse = MouseEvent {
            kind: MouseEventKind::Up(CtMouseButton::Right),
            column: 15,
            row: 8,
            modifiers: KeyModifiers::NONE,
        };
        let event = CrosstermEvent::Mouse(mouse);
        let result = handler.convert(event).unwrap();
        assert!(matches!(
            result,
            Event::MouseUp {
                button: MouseButton::Right,
                ..
            }
        ));
    }

    #[test]
    fn test_convert_mouse_middle() {
        let handler = InputHandler::new();
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(CtMouseButton::Middle),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let event = CrosstermEvent::Mouse(mouse);
        let result = handler.convert(event).unwrap();
        assert!(matches!(
            result,
            Event::MouseDown {
                button: MouseButton::Middle,
                ..
            }
        ));
    }

    #[test]
    fn test_convert_mouse_move() {
        let handler = InputHandler::new();
        let mouse = MouseEvent {
            kind: MouseEventKind::Moved,
            column: 20,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        let event = CrosstermEvent::Mouse(mouse);
        let result = handler.convert(event).unwrap();
        assert!(matches!(result, Event::MouseMove { position } if position.x == 20.0));
    }

    #[test]
    fn test_convert_mouse_drag() {
        let handler = InputHandler::new();
        let mouse = MouseEvent {
            kind: MouseEventKind::Drag(CtMouseButton::Left),
            column: 25,
            row: 12,
            modifiers: KeyModifiers::NONE,
        };
        let event = CrosstermEvent::Mouse(mouse);
        let result = handler.convert(event).unwrap();
        assert!(matches!(result, Event::MouseMove { .. }));
    }

    #[test]
    fn test_convert_scroll_events() {
        let handler = InputHandler::new();

        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let result = handler.convert(CrosstermEvent::Mouse(scroll_up)).unwrap();
        assert!(matches!(result, Event::Scroll { delta_y, .. } if delta_y < 0.0));

        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let result = handler.convert(CrosstermEvent::Mouse(scroll_down)).unwrap();
        assert!(matches!(result, Event::Scroll { delta_y, .. } if delta_y > 0.0));

        let scroll_left = MouseEvent {
            kind: MouseEventKind::ScrollLeft,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let result = handler.convert(CrosstermEvent::Mouse(scroll_left)).unwrap();
        assert!(matches!(result, Event::Scroll { delta_x, .. } if delta_x < 0.0));

        let scroll_right = MouseEvent {
            kind: MouseEventKind::ScrollRight,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let result = handler
            .convert(CrosstermEvent::Mouse(scroll_right))
            .unwrap();
        assert!(matches!(result, Event::Scroll { delta_x, .. } if delta_x > 0.0));
    }

    #[test]
    fn test_convert_resize() {
        let handler = InputHandler::new();
        let event = CrosstermEvent::Resize(120, 40);
        let result = handler.convert(event).unwrap();
        assert!(
            matches!(result, Event::Resize { width, height } if width == 120.0 && height == 40.0)
        );
    }

    #[test]
    fn test_convert_focus_events() {
        let handler = InputHandler::new();

        let result = handler.convert(CrosstermEvent::FocusGained).unwrap();
        assert!(matches!(result, Event::FocusIn));

        let result = handler.convert(CrosstermEvent::FocusLost).unwrap();
        assert!(matches!(result, Event::FocusOut));
    }

    #[test]
    fn test_convert_paste() {
        let handler = InputHandler::new();
        let event = CrosstermEvent::Paste("hello world".to_string());
        let result = handler.convert(event).unwrap();
        assert!(matches!(result, Event::TextInput { text } if text == "hello world"));
    }
}
