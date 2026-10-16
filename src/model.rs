use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    pub macros: Vec<Macro>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macro {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub hotkey: Hotkey,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotkey {
    #[serde(default)]
    pub modifiers: Vec<Modifier>,
    pub key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Win,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    InsertText { text: String },
}

/// Atajo ya comprobado. El nombre de la tecla no depende de Windows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
    pub key: Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Letter(char),
    Digit(char),
    Function(u8),
    Space,
    Enter,
    Tab,
    Escape,
    Left,
    Right,
    Up,
    Down,
    Numpad(u8),
    NumpadAdd,
    NumpadSub,
    NumpadMul,
    NumpadDiv,
    NumpadDec,
}

impl Chord {
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("ctrl");
        }
        if self.shift {
            parts.push("shift");
        }
        if self.alt {
            parts.push("alt");
        }
        if self.win {
            parts.push("win");
        }
        parts.push(self.key.label());
        parts.join("+")
    }
}

impl Key {
    pub fn label(&self) -> &'static str {
        match self {
            Key::Letter(c) => letter_label(*c),
            Key::Digit(c) => digit_label(*c),
            Key::Function(1) => "f1",
            Key::Function(2) => "f2",
            Key::Function(3) => "f3",
            Key::Function(4) => "f4",
            Key::Function(5) => "f5",
            Key::Function(6) => "f6",
            Key::Function(7) => "f7",
            Key::Function(8) => "f8",
            Key::Function(9) => "f9",
            Key::Function(10) => "f10",
            Key::Function(11) => "f11",
            Key::Function(12) => "f12",
            Key::Function(13) => "f13",
            Key::Function(14) => "f14",
            Key::Function(15) => "f15",
            Key::Function(16) => "f16",
            Key::Function(17) => "f17",
            Key::Function(18) => "f18",
            Key::Function(19) => "f19",
            Key::Function(20) => "f20",
            Key::Function(21) => "f21",
            Key::Function(22) => "f22",
            Key::Function(23) => "f23",
            Key::Function(24) => "f24",
            Key::Function(_) => "f?",
            Key::Space => "space",
            Key::Enter => "enter",
            Key::Tab => "tab",
            Key::Escape => "escape",
            Key::Left => "left",
            Key::Right => "right",
            Key::Up => "up",
            Key::Down => "down",
            Key::Numpad(0) => "num0",
            Key::Numpad(1) => "num1",
            Key::Numpad(2) => "num2",
            Key::Numpad(3) => "num3",
            Key::Numpad(4) => "num4",
            Key::Numpad(5) => "num5",
            Key::Numpad(6) => "num6",
            Key::Numpad(7) => "num7",
            Key::Numpad(8) => "num8",
            Key::Numpad(9) => "num9",
            Key::Numpad(_) => "num?",
            Key::NumpadAdd => "num_add",
            Key::NumpadSub => "num_sub",
            Key::NumpadMul => "num_mul",
            Key::NumpadDiv => "num_div",
            Key::NumpadDec => "num_dec",
        }
    }
}

fn letter_label(c: char) -> &'static str {
    match c {
        'a' => "a",
        'b' => "b",
        'c' => "c",
        'd' => "d",
        'e' => "e",
        'f' => "f",
        'g' => "g",
        'h' => "h",
        'i' => "i",
        'j' => "j",
        'k' => "k",
        'l' => "l",
        'm' => "m",
        'n' => "n",
        'o' => "o",
        'p' => "p",
        'q' => "q",
        'r' => "r",
        's' => "s",
        't' => "t",
        'u' => "u",
        'v' => "v",
        'w' => "w",
        'x' => "x",
        'y' => "y",
        'z' => "z",
        _ => "?",
    }
}

fn digit_label(c: char) -> &'static str {
    match c {
        '0' => "0",
        '1' => "1",
        '2' => "2",
        '3' => "3",
        '4' => "4",
        '5' => "5",
        '6' => "6",
        '7' => "7",
        '8' => "8",
        '9' => "9",
        _ => "?",
    }
}

pub fn parse_chord(hotkey: &Hotkey) -> Result<Chord, String> {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut win = false;
    for modifier in &hotkey.modifiers {
        match modifier {
            Modifier::Ctrl => ctrl = true,
            Modifier::Shift => shift = true,
            Modifier::Alt => alt = true,
            Modifier::Win => win = true,
        }
    }
    Ok(Chord {
        ctrl,
        shift,
        alt,
        win,
        key: parse_key(&hotkey.key)?,
    })
}

pub fn parse_key(raw: &str) -> Result<Key, String> {
    let name = raw.trim().to_ascii_lowercase();
    if name.len() == 1 {
        let c = name.chars().next().unwrap();
        if c.is_ascii_lowercase() {
            return Ok(Key::Letter(c));
        }
        if c.is_ascii_digit() {
            return Ok(Key::Digit(c));
        }
    }
    if let Some(rest) = name.strip_prefix('f') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Ok(Key::Function(n));
            }
        }
    }
    if let Some(rest) = name.strip_prefix("num") {
        if rest.len() == 1 {
            let c = rest.chars().next().unwrap();
            if c.is_ascii_digit() {
                return Ok(Key::Numpad(c.to_digit(10).unwrap() as u8));
            }
        }
    }
    let key = match name.as_str() {
        "space" => Key::Space,
        "enter" | "return" => Key::Enter,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "num_add" | "numpad_add" => Key::NumpadAdd,
        "num_sub" | "numpad_sub" | "num_minus" => Key::NumpadSub,
        "num_mul" | "numpad_mul" => Key::NumpadMul,
        "num_div" | "numpad_div" => Key::NumpadDiv,
        "num_dec" | "numpad_dec" => Key::NumpadDec,
        _ => return Err(format!("tecla desconocida: {raw}")),
    };
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_letter_and_ignores_modifier_order() {
        let hotkey = Hotkey {
            modifiers: vec![Modifier::Shift, Modifier::Ctrl],
            key: "H".into(),
        };
        let chord = parse_chord(&hotkey).unwrap();
        assert!(chord.ctrl && chord.shift && !chord.alt);
        assert_eq!(chord.key, Key::Letter('h'));
        assert_eq!(chord.label(), "ctrl+shift+h");
    }

    #[test]
    fn rejects_unknown_key() {
        assert!(parse_key("not-a-key").is_err());
    }
}
