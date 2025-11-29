//! Input events for widgets.

use crate::geometry::Point;
use serde::{Deserialize, Serialize};

/// Input event types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// Mouse moved to position
    MouseMove {
        /// New position
        position: Point,
    },
    /// Mouse button pressed
    MouseDown {
        /// Position of click
        position: Point,
        /// Button pressed
        button: MouseButton,
    },
    /// Mouse button released
    MouseUp {
        /// Position of release
        position: Point,
        /// Button released
        button: MouseButton,
    },
    /// Mouse wheel scrolled
    Scroll {
        /// Horizontal scroll delta
        delta_x: f32,
        /// Vertical scroll delta
        delta_y: f32,
    },
    /// Key pressed
    KeyDown {
        /// Key pressed
        key: Key,
    },
    /// Key released
    KeyUp {
        /// Key released
        key: Key,
    },
    /// Text input received
    TextInput {
        /// Input text
        text: String,
    },
    /// Widget gained focus
    FocusIn,
    /// Widget lost focus
    FocusOut,
    /// Mouse entered widget bounds
    MouseEnter,
    /// Mouse left widget bounds
    MouseLeave,
    /// Window resized
    Resize {
        /// New width
        width: f32,
        /// New height
        height: f32,
    },
}

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Right mouse button
    Right,
    /// Middle mouse button (wheel click)
    Middle,
    /// Additional button 1
    Button4,
    /// Additional button 2
    Button5,
}

/// Keyboard key identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    // Letters
    /// A key
    A,
    /// B key
    B,
    /// C key
    C,
    /// D key
    D,
    /// E key
    E,
    /// F key
    F,
    /// G key
    G,
    /// H key
    H,
    /// I key
    I,
    /// J key
    J,
    /// K key
    K,
    /// L key
    L,
    /// M key
    M,
    /// N key
    N,
    /// O key
    O,
    /// P key
    P,
    /// Q key
    Q,
    /// R key
    R,
    /// S key
    S,
    /// T key
    T,
    /// U key
    U,
    /// V key
    V,
    /// W key
    W,
    /// X key
    X,
    /// Y key
    Y,
    /// Z key
    Z,

    // Numbers
    /// 0 key
    Num0,
    /// 1 key
    Num1,
    /// 2 key
    Num2,
    /// 3 key
    Num3,
    /// 4 key
    Num4,
    /// 5 key
    Num5,
    /// 6 key
    Num6,
    /// 7 key
    Num7,
    /// 8 key
    Num8,
    /// 9 key
    Num9,

    // Function keys
    /// F1 key
    F1,
    /// F2 key
    F2,
    /// F3 key
    F3,
    /// F4 key
    F4,
    /// F5 key
    F5,
    /// F6 key
    F6,
    /// F7 key
    F7,
    /// F8 key
    F8,
    /// F9 key
    F9,
    /// F10 key
    F10,
    /// F11 key
    F11,
    /// F12 key
    F12,

    // Control keys
    /// Enter/Return key
    Enter,
    /// Escape key
    Escape,
    /// Backspace key
    Backspace,
    /// Tab key
    Tab,
    /// Space key
    Space,
    /// Delete key
    Delete,
    /// Insert key
    Insert,
    /// Home key
    Home,
    /// End key
    End,
    /// Page Up key
    PageUp,
    /// Page Down key
    PageDown,

    // Arrow keys
    /// Up arrow
    Up,
    /// Down arrow
    Down,
    /// Left arrow
    Left,
    /// Right arrow
    Right,

    // Modifiers
    /// Left Shift
    ShiftLeft,
    /// Right Shift
    ShiftRight,
    /// Left Control
    ControlLeft,
    /// Right Control
    ControlRight,
    /// Left Alt
    AltLeft,
    /// Right Alt
    AltRight,
    /// Left Meta (Windows/Command)
    MetaLeft,
    /// Right Meta (Windows/Command)
    MetaRight,

    // Punctuation
    /// Minus key
    Minus,
    /// Equals key
    Equal,
    /// Left bracket
    BracketLeft,
    /// Right bracket
    BracketRight,
    /// Backslash
    Backslash,
    /// Semicolon
    Semicolon,
    /// Quote/apostrophe
    Quote,
    /// Grave/backtick
    Grave,
    /// Comma
    Comma,
    /// Period
    Period,
    /// Slash
    Slash,
}

impl Event {
    /// Check if this is a mouse event.
    #[must_use]
    pub fn is_mouse(&self) -> bool {
        matches!(
            self,
            Self::MouseMove { .. }
                | Self::MouseDown { .. }
                | Self::MouseUp { .. }
                | Self::MouseEnter
                | Self::MouseLeave
        )
    }

    /// Check if this is a keyboard event.
    #[must_use]
    pub fn is_keyboard(&self) -> bool {
        matches!(
            self,
            Self::KeyDown { .. } | Self::KeyUp { .. } | Self::TextInput { .. }
        )
    }

    /// Check if this is a focus event.
    #[must_use]
    pub fn is_focus(&self) -> bool {
        matches!(self, Self::FocusIn | Self::FocusOut)
    }

    /// Get the position if this is a positional mouse event.
    #[must_use]
    pub fn position(&self) -> Option<Point> {
        match self {
            Self::MouseMove { position }
            | Self::MouseDown { position, .. }
            | Self::MouseUp { position, .. } => Some(*position),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_is_mouse() {
        assert!(Event::MouseMove {
            position: Point::ORIGIN
        }
        .is_mouse());
        assert!(Event::MouseEnter.is_mouse());
        assert!(!Event::KeyDown { key: Key::Enter }.is_mouse());
    }

    #[test]
    fn test_event_is_keyboard() {
        assert!(Event::KeyDown { key: Key::A }.is_keyboard());
        assert!(Event::TextInput {
            text: "x".to_string()
        }
        .is_keyboard());
        assert!(!Event::MouseMove {
            position: Point::ORIGIN
        }
        .is_keyboard());
    }

    #[test]
    fn test_event_is_focus() {
        assert!(Event::FocusIn.is_focus());
        assert!(Event::FocusOut.is_focus());
        assert!(!Event::KeyDown { key: Key::Tab }.is_focus());
    }

    #[test]
    fn test_event_position() {
        let pos = Point::new(100.0, 200.0);
        assert_eq!(Event::MouseMove { position: pos }.position(), Some(pos));
        assert_eq!(
            Event::MouseDown {
                position: pos,
                button: MouseButton::Left
            }
            .position(),
            Some(pos)
        );
        assert_eq!(Event::FocusIn.position(), None);
    }

    #[test]
    fn test_event_mouse_up_position() {
        let pos = Point::new(50.0, 75.0);
        let event = Event::MouseUp {
            position: pos,
            button: MouseButton::Right,
        };
        assert_eq!(event.position(), Some(pos));
        assert!(event.is_mouse());
    }

    #[test]
    fn test_event_scroll() {
        let event = Event::Scroll {
            delta_x: 10.0,
            delta_y: -5.0,
        };
        assert!(!event.is_mouse());
        assert!(!event.is_keyboard());
        assert!(event.position().is_none());
    }

    #[test]
    fn test_event_resize() {
        let event = Event::Resize {
            width: 800.0,
            height: 600.0,
        };
        assert!(!event.is_mouse());
        assert!(!event.is_keyboard());
        assert!(!event.is_focus());
    }

    #[test]
    fn test_event_key_up() {
        let event = Event::KeyUp { key: Key::Space };
        assert!(event.is_keyboard());
        assert!(!event.is_mouse());
    }

    #[test]
    fn test_mouse_button_equality() {
        assert_eq!(MouseButton::Left, MouseButton::Left);
        assert_ne!(MouseButton::Left, MouseButton::Right);
    }

    #[test]
    fn test_key_equality() {
        assert_eq!(Key::Enter, Key::Enter);
        assert_ne!(Key::Enter, Key::Space);
    }

    #[test]
    fn test_event_mouse_leave() {
        let event = Event::MouseLeave;
        assert!(event.is_mouse());
        assert!(event.position().is_none());
    }
}
