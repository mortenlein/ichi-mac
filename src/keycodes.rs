//! Parsing human-written shortcut strings into Carbon hotkey parameters.
//!
//! A shortcut is modifiers plus exactly one key, joined by `+`:
//!
//! ```text
//! ctrl+alt+keypad4      the default bindings
//! ctrl+opt+left         `opt` and `alt` are the same physical key
//! cmd+shift+u
//! ```
//!
//! Parsing is case-insensitive and tolerant of spaces around the `+`.

/// Carbon modifier masks from `Events.h`.
pub const CMD_KEY: u32 = 1 << 8;
pub const SHIFT_KEY: u32 = 1 << 9;
pub const OPTION_KEY: u32 = 1 << 11;
pub const CONTROL_KEY: u32 = 1 << 12;

/// `kVK_*` virtual key codes from `HIToolbox/Events.h`.
///
/// Note the keypad digits are not contiguous: 8 and 9 are 0x5B/0x5C because
/// 0x5A is `kVK_F20`. Getting that wrong silently binds the wrong key.
const KEY_NAMES: &[(&str, u32)] = &[
    // Keypad — what the Windows build's numpad bindings map onto.
    ("keypad0", 0x52),
    ("keypad1", 0x53),
    ("keypad2", 0x54),
    ("keypad3", 0x55),
    ("keypad4", 0x56),
    ("keypad5", 0x57),
    ("keypad6", 0x58),
    ("keypad7", 0x59),
    ("keypad8", 0x5B),
    ("keypad9", 0x5C),
    ("keypadclear", 0x47),
    ("keypaddivide", 0x4B),
    ("keypadenter", 0x4C),
    ("keypadequals", 0x51),
    ("keypadminus", 0x4E),
    ("keypadmultiply", 0x43),
    ("keypadplus", 0x45),
    ("keypaddecimal", 0x41),
    // Letters.
    ("a", 0x00),
    ("b", 0x0B),
    ("c", 0x08),
    ("d", 0x02),
    ("e", 0x0E),
    ("f", 0x03),
    ("g", 0x05),
    ("h", 0x04),
    ("i", 0x22),
    ("j", 0x26),
    ("k", 0x28),
    ("l", 0x25),
    ("m", 0x2E),
    ("n", 0x2D),
    ("o", 0x1F),
    ("p", 0x23),
    ("q", 0x0C),
    ("r", 0x0F),
    ("s", 0x01),
    ("t", 0x11),
    ("u", 0x20),
    ("v", 0x09),
    ("w", 0x0D),
    ("x", 0x07),
    ("y", 0x10),
    ("z", 0x06),
    // Number row.
    ("0", 0x1D),
    ("1", 0x12),
    ("2", 0x13),
    ("3", 0x14),
    ("4", 0x15),
    ("5", 0x17),
    ("6", 0x16),
    ("7", 0x1A),
    ("8", 0x1C),
    ("9", 0x19),
    // Arrows — the natural laptop alternative to the keypad.
    ("left", 0x7B),
    ("right", 0x7C),
    ("down", 0x7D),
    ("up", 0x7E),
    // Editing and whitespace.
    ("return", 0x24),
    ("enter", 0x24),
    ("tab", 0x30),
    ("space", 0x31),
    ("delete", 0x33),
    ("backspace", 0x33),
    ("escape", 0x35),
    ("esc", 0x35),
    ("forwarddelete", 0x75),
    ("home", 0x73),
    ("end", 0x77),
    ("pageup", 0x74),
    ("pagedown", 0x79),
    // Punctuation.
    ("minus", 0x1B),
    ("equal", 0x18),
    ("leftbracket", 0x21),
    ("rightbracket", 0x1E),
    ("backslash", 0x2A),
    ("semicolon", 0x29),
    ("quote", 0x27),
    ("comma", 0x2B),
    ("period", 0x2F),
    ("slash", 0x2C),
    ("grave", 0x32),
    // Function keys.
    ("f1", 0x7A),
    ("f2", 0x78),
    ("f3", 0x63),
    ("f4", 0x76),
    ("f5", 0x60),
    ("f6", 0x61),
    ("f7", 0x62),
    ("f8", 0x64),
    ("f9", 0x65),
    ("f10", 0x6D),
    ("f11", 0x67),
    ("f12", 0x6F),
    ("f13", 0x69),
    ("f14", 0x6B),
    ("f15", 0x71),
    ("f16", 0x6A),
    ("f17", 0x40),
    ("f18", 0x4F),
    ("f19", 0x50),
    ("f20", 0x5A),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub modifiers: u32,
    pub key_code: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    UnknownToken(String),
    NoKey,
    MultipleKeys,
    NoModifiers,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "shortcut is empty"),
            Self::UnknownToken(t) => write!(f, "unknown key or modifier {t:?}"),
            Self::NoKey => write!(f, "shortcut has modifiers but no key"),
            Self::MultipleKeys => write!(f, "shortcut has more than one non-modifier key"),
            Self::NoModifiers => write!(
                f,
                "shortcut needs at least one modifier, or it would swallow the bare keypress"
            ),
        }
    }
}

/// Parse something like `"ctrl+alt+keypad4"` into modifiers and a key code.
pub fn parse(input: &str) -> Result<Shortcut, ParseError> {
    let normalised = input.trim().to_ascii_lowercase();
    if normalised.is_empty() {
        return Err(ParseError::Empty);
    }

    let mut modifiers = 0u32;
    let mut key_code: Option<u32> = None;

    for token in normalised.split('+') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        let modifier = match token {
            "cmd" | "command" | "super" => Some(CMD_KEY),
            "shift" => Some(SHIFT_KEY),
            // `alt` and `opt` are the same physical key; accepting both keeps
            // configs portable from the Windows build.
            "alt" | "opt" | "option" => Some(OPTION_KEY),
            "ctrl" | "control" => Some(CONTROL_KEY),
            _ => None,
        };

        if let Some(mask) = modifier {
            modifiers |= mask;
            continue;
        }

        let code = KEY_NAMES
            .iter()
            .find(|(name, _)| *name == token)
            .map(|(_, code)| *code)
            .ok_or_else(|| ParseError::UnknownToken(token.to_string()))?;

        if key_code.is_some() {
            return Err(ParseError::MultipleKeys);
        }
        key_code = Some(code);
    }

    let key_code = key_code.ok_or(ParseError::NoKey)?;

    // A modifier-less hotkey would capture the key globally and make it
    // unusable for typing, so refuse rather than let someone lock up their "a".
    if modifiers == 0 {
        return Err(ParseError::NoModifiers);
    }

    Ok(Shortcut {
        modifiers,
        key_code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_default_keypad_bindings() {
        let s = parse("ctrl+alt+keypad4").unwrap();
        assert_eq!(s.modifiers, CONTROL_KEY | OPTION_KEY);
        assert_eq!(s.key_code, 0x56);
    }

    #[test]
    fn keypad_8_and_9_skip_the_f20_slot() {
        // 0x5A is kVK_F20; binding it by mistake would be silent and baffling.
        assert_eq!(parse("ctrl+keypad8").unwrap().key_code, 0x5B);
        assert_eq!(parse("ctrl+keypad9").unwrap().key_code, 0x5C);
        assert_eq!(parse("ctrl+f20").unwrap().key_code, 0x5A);
    }

    #[test]
    fn alt_and_opt_are_interchangeable() {
        assert_eq!(parse("ctrl+alt+left"), parse("ctrl+opt+left"));
        assert_eq!(parse("ctrl+alt+left"), parse("ctrl+option+left"));
    }

    #[test]
    fn is_case_and_whitespace_insensitive() {
        assert_eq!(parse("CTRL+Alt+KeyPad4"), parse("ctrl+alt+keypad4"));
        assert_eq!(parse("  ctrl + alt + keypad4  "), parse("ctrl+alt+keypad4"));
    }

    #[test]
    fn accumulates_all_four_modifiers() {
        let s = parse("cmd+shift+ctrl+opt+u").unwrap();
        assert_eq!(s.modifiers, CMD_KEY | SHIFT_KEY | CONTROL_KEY | OPTION_KEY);
        assert_eq!(s.key_code, 0x20);
    }

    #[test]
    fn rejects_a_shortcut_with_no_modifiers() {
        // Otherwise Ichi would eat the bare key system-wide.
        assert_eq!(parse("a"), Err(ParseError::NoModifiers));
        assert_eq!(parse("keypad4"), Err(ParseError::NoModifiers));
    }

    #[test]
    fn rejects_malformed_input() {
        assert_eq!(parse(""), Err(ParseError::Empty));
        assert_eq!(parse("ctrl+alt"), Err(ParseError::NoKey));
        assert_eq!(parse("ctrl+a+b"), Err(ParseError::MultipleKeys));
        assert_eq!(
            parse("ctrl+nonsense"),
            Err(ParseError::UnknownToken("nonsense".into()))
        );
    }

    #[test]
    fn key_table_has_no_duplicate_names() {
        let mut names: Vec<&str> = KEY_NAMES.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate key name in KEY_NAMES");
    }

    #[test]
    fn arrow_keys_resolve_for_laptop_friendly_bindings() {
        assert_eq!(parse("ctrl+opt+left").unwrap().key_code, 0x7B);
        assert_eq!(parse("ctrl+opt+right").unwrap().key_code, 0x7C);
        assert_eq!(parse("ctrl+opt+down").unwrap().key_code, 0x7D);
        assert_eq!(parse("ctrl+opt+up").unwrap().key_code, 0x7E);
    }
}
