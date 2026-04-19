//! Action primitives the engine can dispatch.
//!
//! These are the units the top-level API (`Engine::dispatch`) and the
//! trace recorder (D2) work in. They are deliberately backend-agnostic:
//! a terminal-mode driver translates them into tmux `send-keys`/
//! mouse-SGR sequences, while an adapter-mode driver forwards them to a
//! framework adapter over RPC.

/// Keyboard key that isn't represented directly as a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Tab,
    BackTab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// A resolved click target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

/// High-level interaction primitive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Type literal text (no special-key interpretation).
    Type(String),
    /// Press a non-printable key.
    Press(Key),
    /// Press a chord, e.g. Ctrl+C. Modifier bitflags are documented on
    /// [`Modifiers`]; `key` is the base key being modified.
    Chord { mods: Modifiers, key: ChordKey },
    /// Click at a resolved point with the given mouse button.
    Click { at: Point, button: MouseButton },
    /// Resize the pane in cells.
    Resize { width: u16, height: u16 },
}

/// Modifier bitmask for [`Action::Chord`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const CTRL: Self = Self(1 << 0);
    pub const ALT: Self = Self(1 << 1);
    pub const SHIFT: Self = Self(1 << 2);

    #[must_use]
    pub fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub fn bits(self) -> u8 {
        self.0
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Base key for a chord. Either a printable character or a named key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordKey {
    Char(char),
    Named(Key),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_bitor_and_contains() {
        let m = Modifiers::CTRL | Modifiers::SHIFT;
        assert!(m.contains(Modifiers::CTRL));
        assert!(m.contains(Modifiers::SHIFT));
        assert!(!m.contains(Modifiers::ALT));
        assert_eq!(m.bits(), 0b101);
    }

    #[test]
    fn empty_modifiers_contains_nothing() {
        let m = Modifiers::empty();
        assert!(!m.contains(Modifiers::CTRL));
    }

    #[test]
    fn action_equality_is_value_based() {
        let a = Action::Type("hi".into());
        let b = Action::Type("hi".into());
        assert_eq!(a, b);
        let c = Action::Press(Key::Enter);
        assert_ne!(a, c);
    }

    #[test]
    fn chord_key_variants() {
        let c1 = Action::Chord {
            mods: Modifiers::CTRL,
            key: ChordKey::Char('c'),
        };
        let c2 = Action::Chord {
            mods: Modifiers::CTRL,
            key: ChordKey::Named(Key::Left),
        };
        assert_ne!(c1, c2);
    }
}
