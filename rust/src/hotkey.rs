//! Hotkeys: a virtual key plus modifiers, stored in the settings file the same way the C# app did
//! ({"Modifiers": "Alt, Control", "Key": "M"}, using .NET `Keys` names).

use crate::json::Json;

// Same bit values as the Win32 MOD_* flags used by RegisterHotKey.
pub const ALT: u8 = 1;
pub const CONTROL: u8 = 2;
pub const SHIFT: u8 = 4;
pub const WIN: u8 = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hotkey {
    pub mods: u8,
    pub vk: u16,
}

const F13: u16 = 0x7C;
const F24: u16 = 0x87;

impl Hotkey {
    pub fn is_empty(&self) -> bool {
        self.vk == 0
    }

    /// Needs Ctrl, Alt or Win (Shift alone would swallow typing), except F13–F24.
    pub fn is_valid(&self) -> bool {
        !self.is_empty() && ((self.mods & !SHIFT) != 0 || (F13..=F24).contains(&self.vk))
    }

    /// "Ctrl + Alt + M".
    pub fn display(&self) -> String {
        if self.is_empty() {
            return "(none)".into();
        }
        self.key_names().join(" + ")
    }

    /// The modifiers and the key, each as written on the key cap ("Ctrl", "Alt", "M").
    pub fn key_names(&self) -> Vec<String> {
        let mut parts = modifier_names(self.mods);
        parts.push(display_key(self.vk));
        parts
    }

    pub fn to_json(self) -> Json {
        let mut names = Vec::new();
        for (bit, name) in [(ALT, "Alt"), (CONTROL, "Control"), (SHIFT, "Shift"), (WIN, "Win")] {
            if self.mods & bit != 0 {
                names.push(name);
            }
        }
        let mods = if names.is_empty() { "None".to_string() } else { names.join(", ") };
        let key = if self.vk == 0 { "None".to_string() } else { key_name(self.vk).map(str::to_string).unwrap_or_else(|| self.vk.to_string()) };
        Json::Obj(vec![("Modifiers".into(), Json::Str(mods)), ("Key".into(), Json::Str(key))])
    }

    pub fn from_json(v: &Json) -> Hotkey {
        let mut mods = 0u8;
        match v.get("Modifiers") {
            Some(Json::Str(s)) => {
                for part in s.split(',').map(str::trim) {
                    mods |= match part.to_ascii_lowercase().as_str() {
                        "alt" => ALT,
                        "control" => CONTROL,
                        "shift" => SHIFT,
                        "win" => WIN,
                        n => n.parse::<u8>().unwrap_or(0),
                    };
                }
            }
            Some(Json::Num(n)) => mods = *n as u8,
            _ => {}
        }
        let vk = match v.get("Key") {
            Some(Json::Str(s)) => parse_key(s),
            Some(Json::Num(n)) => *n as u16,
            _ => 0,
        };
        Hotkey { mods: mods & 0xF, vk }
    }
}

fn modifier_names(mods: u8) -> Vec<String> {
    [(CONTROL, "Ctrl"), (ALT, "Alt"), (SHIFT, "Shift"), (WIN, "Win")]
        .iter()
        .filter(|(bit, _)| mods & bit != 0)
        .map(|(_, name)| name.to_string())
        .collect()
}

/// Modifiers held so far, while a combination is being recorded: "Ctrl + Win + …".
pub fn partial_display(mods: u8) -> String {
    let mut parts = modifier_names(mods);
    parts.push("…".into());
    parts.join(" + ")
}

/// Every named .NET `Keys` value outside the digit, letter, number-pad and F-key ranges, so
/// any hotkey the C# app records reads back. The first name for a key is the one written;
/// the others are accepted aliases.
const KEY_NAMES: &[(u16, &str)] = &[
    (0x03, "Cancel"), (0x08, "Back"), (0x09, "Tab"), (0x0A, "LineFeed"), (0x0C, "Clear"), (0x0D, "Return"), (0x0D, "Enter"),
    (0x13, "Pause"), (0x14, "Capital"), (0x14, "CapsLock"),
    (0x15, "KanaMode"), (0x15, "HangulMode"), (0x15, "HanguelMode"), (0x17, "JunjaMode"), (0x18, "FinalMode"),
    (0x19, "HanjaMode"), (0x19, "KanjiMode"), (0x1B, "Escape"),
    (0x1C, "IMEConvert"), (0x1D, "IMENonconvert"), (0x1E, "IMEAccept"), (0x1E, "IMEAceept"), (0x1F, "IMEModeChange"),
    (0x20, "Space"), (0x21, "PageUp"), (0x21, "Prior"), (0x22, "PageDown"), (0x22, "Next"), (0x23, "End"),
    (0x24, "Home"), (0x25, "Left"), (0x26, "Up"), (0x27, "Right"), (0x28, "Down"), (0x29, "Select"),
    (0x2A, "Print"), (0x2B, "Execute"), (0x2C, "PrintScreen"), (0x2C, "Snapshot"), (0x2D, "Insert"), (0x2E, "Delete"),
    (0x2F, "Help"), (0x5D, "Apps"), (0x5F, "Sleep"),
    (0x6A, "Multiply"), (0x6B, "Add"), (0x6C, "Separator"), (0x6D, "Subtract"), (0x6E, "Decimal"), (0x6F, "Divide"),
    (0x90, "NumLock"), (0x91, "Scroll"),
    (0xA6, "BrowserBack"), (0xA7, "BrowserForward"), (0xA8, "BrowserRefresh"), (0xA9, "BrowserStop"),
    (0xAA, "BrowserSearch"), (0xAB, "BrowserFavorites"), (0xAC, "BrowserHome"),
    (0xAD, "VolumeMute"), (0xAE, "VolumeDown"), (0xAF, "VolumeUp"), (0xB0, "MediaNextTrack"),
    (0xB1, "MediaPreviousTrack"), (0xB2, "MediaStop"), (0xB3, "MediaPlayPause"),
    (0xB4, "LaunchMail"), (0xB5, "SelectMedia"), (0xB6, "LaunchApplication1"), (0xB7, "LaunchApplication2"),
    (0xBA, "OemSemicolon"), (0xBA, "Oem1"), (0xBB, "Oemplus"), (0xBC, "Oemcomma"), (0xBD, "OemMinus"), (0xBE, "OemPeriod"),
    (0xBF, "OemQuestion"), (0xBF, "Oem2"), (0xC0, "Oemtilde"), (0xC0, "Oem3"), (0xDB, "OemOpenBrackets"),
    (0xDB, "Oem4"), (0xDC, "OemPipe"), (0xDC, "Oem5"), (0xDD, "OemCloseBrackets"), (0xDD, "Oem6"),
    (0xDE, "OemQuotes"), (0xDE, "Oem7"), (0xDF, "Oem8"), (0xE2, "OemBackslash"), (0xE2, "Oem102"),
    (0xE5, "ProcessKey"), (0xE7, "Packet"), (0xF6, "Attn"), (0xF7, "Crsel"), (0xF8, "Exsel"), (0xF9, "EraseEof"),
    (0xFA, "Play"), (0xFB, "Zoom"), (0xFC, "NoName"), (0xFD, "Pa1"), (0xFE, "OemClear"),
];
fn key_name(vk: u16) -> Option<&'static str> {
    match vk {
        0x30..=0x39 => Some(["D0", "D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9"][(vk - 0x30) as usize]),
        0x41..=0x5A => Some(["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q",
            "R", "S", "T", "U", "V", "W", "X", "Y", "Z"][(vk - 0x41) as usize]),
        0x60..=0x69 => Some(["NumPad0", "NumPad1", "NumPad2", "NumPad3", "NumPad4", "NumPad5", "NumPad6", "NumPad7",
            "NumPad8", "NumPad9"][(vk - 0x60) as usize]),
        0x70..=0x87 => Some(["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14",
            "F15", "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24"][(vk - 0x70) as usize]),
        _ => KEY_NAMES.iter().find(|(k, _)| *k == vk).map(|(_, n)| *n),
    }
}

fn parse_key(s: &str) -> u16 {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") || s.is_empty() {
        return 0;
    }
    if let Ok(n) = s.parse::<u16>() {
        return n;
    }
    for vk in (0x30..=0x39).chain(0x41..=0x5A).chain(0x60..=0x69).chain(0x70..=0x87) {
        if key_name(vk).is_some_and(|n| n.eq_ignore_ascii_case(s)) {
            return vk;
        }
    }
    KEY_NAMES.iter().find(|(_, n)| n.eq_ignore_ascii_case(s)).map(|(k, _)| *k).unwrap_or(0)
}

/// How a key is shown to people: "1" rather than "D1", "," rather than "Oemcomma".
fn display_key(vk: u16) -> String {
    match vk {
        0x30..=0x39 => ((b'0' + (vk - 0x30) as u8) as char).to_string(),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        0xBC => ",".into(),
        0xBE => ".".into(),
        0xBD => "-".into(),
        0xBB => "=".into(),
        0xBF => "/".into(),
        0xBA => ";".into(),
        0xDE => "'".into(),
        0xDB => "[".into(),
        0xDD => "]".into(),
        0xDC => "\\".into(),
        0xC0 => "`".into(),
        _ => key_name(vk).map(str::to_string).unwrap_or_else(|| format!("Key {vk:#04X}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hk(mods: u8, vk: u16) -> Hotkey {
        Hotkey { mods, vk }
    }

    #[test]
    fn validity() {
        assert!(hk(CONTROL | ALT, b'M' as u16).is_valid());
        assert!(hk(WIN, 0x70).is_valid());
        assert!(hk(0, F13).is_valid());
        assert!(!hk(0, b'M' as u16).is_valid());
        assert!(!hk(SHIFT, b'M' as u16).is_valid());
        assert!(!Hotkey::default().is_valid());
    }

    #[test]
    fn display() {
        assert_eq!(hk(ALT | CONTROL, b'M' as u16).display(), "Ctrl + Alt + M");
        assert_eq!(hk(CONTROL | SHIFT, 0x31).display(), "Ctrl + Shift + 1");
        assert_eq!(Hotkey::default().display(), "(none)");
    }

    #[test]
    fn json_matches_the_csharp_format() {
        let h = hk(ALT | CONTROL, b'M' as u16);
        let j = h.to_json();
        assert_eq!(j.str("Modifiers"), Some("Alt, Control"));
        assert_eq!(j.str("Key"), Some("M"));
        assert_eq!(Hotkey::from_json(&j), h);
        let csharp = crate::json::parse(r#"{"Modifiers":"Control, Shift","Key":"D1"}"#).unwrap();
        assert_eq!(Hotkey::from_json(&csharp), hk(CONTROL | SHIFT, 0x31));
        let next = crate::json::parse(r#"{"Modifiers":"Alt","Key":"Next"}"#).unwrap();
        assert_eq!(Hotkey::from_json(&next).vk, 0x22);
    }

    #[test]
    fn reads_every_key_the_csharp_app_can_write() {
        for (name, vk) in [("Apps", 0x5D), ("CapsLock", 0x14), ("Capital", 0x14), ("Cancel", 0x03), ("BrowserHome", 0xAC),
            ("LaunchMail", 0xB4), ("Oem8", 0xDF), ("Sleep", 0x5F), ("OemClear", 0xFE), ("93", 93)] {
            assert_eq!(parse_key(name), vk, "{name}");
        }
        // Written back under a name C# reads.
        assert_eq!(key_name(0x5D), Some("Apps"));
        assert_eq!(key_name(0x14), Some("Capital"));
    }
}
