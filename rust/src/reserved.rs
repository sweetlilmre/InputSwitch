//! Shortcuts Windows keeps for itself. Some are handled before any app sees the keys, so a
//! hotkey on them can register successfully and still never fire; the hotkey editor warns
//! before saving one. Not exhaustive: the editor also test-registers every new combination.

use crate::hotkey::{Hotkey, ALT, CONTROL, SHIFT, WIN};

const fn k(mods: u8, vk: u8, what: &'static str) -> (u8, u16, &'static str) {
    (mods, vk as u16, what)
}

const LEFT: u8 = 0x25;
const UP: u8 = 0x26;
const RIGHT: u8 = 0x27;
const DOWN: u8 = 0x28;
const RETURN: u8 = 0x0D;
const TAB: u8 = 0x09;
const SPACE: u8 = 0x20;
const ESCAPE: u8 = 0x1B;
const HOME: u8 = 0x24;
const PAUSE: u8 = 0x13;
const PRINT: u8 = 0x2C;
const PLUS: u8 = 0xBB;
const MINUS: u8 = 0xBD;
const COMMA: u8 = 0xBC;
const PERIOD: u8 = 0xBE;
const SEMICOLON: u8 = 0xBA;
const F4: u8 = 0x73;

const SHORTCUTS: &[(u8, u16, &str)] = &[
    // Win + key
    k(WIN, b'A', "Quick Settings"), k(WIN, b'B', "notification area"), k(WIN, b'C', "Copilot"),
    k(WIN, b'D', "show desktop"), k(WIN, b'E', "File Explorer"), k(WIN, b'G', "Game Bar"),
    k(WIN, b'H', "voice typing"), k(WIN, b'I', "Settings"), k(WIN, b'K', "Cast"), k(WIN, b'L', "lock"),
    k(WIN, b'M', "minimise all"), k(WIN, b'N', "notifications"), k(WIN, b'O', "rotation lock"),
    k(WIN, b'P', "projection"), k(WIN, b'Q', "search"), k(WIN, b'R', "Run"), k(WIN, b'S', "search"),
    k(WIN, b'T', "taskbar apps"), k(WIN, b'U', "Accessibility settings"), k(WIN, b'V', "clipboard history"),
    k(WIN, b'W', "Widgets"), k(WIN, b'X', "Quick Link menu"), k(WIN, b'Z', "Snap layouts"),
    k(WIN, TAB, "Task View"), k(WIN, SPACE, "switch keyboard layout"), k(WIN, ESCAPE, "close Magnifier"),
    k(WIN, LEFT, "snap window"), k(WIN, RIGHT, "snap window"), k(WIN, UP, "maximise window"),
    k(WIN, DOWN, "minimise window"), k(WIN, HOME, "minimise other windows"), k(WIN, PAUSE, "System settings"),
    k(WIN, PRINT, "screenshot"), k(WIN, PLUS, "Magnifier"), k(WIN, MINUS, "Magnifier"),
    k(WIN, COMMA, "peek at desktop"), k(WIN, PERIOD, "emoji panel"), k(WIN, SEMICOLON, "emoji panel"),
    // Win + Shift + key
    k(WIN | SHIFT, b'S', "Snipping Tool"), k(WIN | SHIFT, b'M', "restore minimised windows"),
    k(WIN | SHIFT, LEFT, "move window to another monitor"), k(WIN | SHIFT, RIGHT, "move window to another monitor"),
    k(WIN | SHIFT, UP, "stretch window"), k(WIN | SHIFT, DOWN, "restore window"),
    // Win + Ctrl + key
    k(WIN | CONTROL, b'D', "new virtual desktop"), k(WIN | CONTROL, F4, "close virtual desktop"),
    k(WIN | CONTROL, LEFT, "switch virtual desktop"), k(WIN | CONTROL, RIGHT, "switch virtual desktop"),
    k(WIN | CONTROL, RETURN, "Narrator"), k(WIN | CONTROL, b'N', "Narrator settings"),
    k(WIN | CONTROL, b'O', "on-screen keyboard"), k(WIN | CONTROL, b'C', "colour filters"),
    k(WIN | CONTROL, b'M', "Magnifier settings"), k(WIN | CONTROL, b'S', "speech recognition"),
    k(WIN | CONTROL, b'F', "find computers"), k(WIN | CONTROL, b'Q', "Quick Assist"),
    k(WIN | CONTROL | SHIFT, b'B', "restart the graphics driver"),
    // Win + Alt + key
    k(WIN | ALT, b'D', "date and time"), k(WIN | ALT, b'R', "Game Bar recording"), k(WIN | ALT, b'B', "HDR"),
    k(WIN | ALT, b'K', "mute microphone"), k(WIN | ALT, PRINT, "Game Bar screenshot"),
];

/// What Windows uses this combination for, if it's a known Windows shortcut. Digits with Win
/// (with or without Shift, Ctrl or Alt) open or switch taskbar apps.
pub fn windows_use(h: &Hotkey) -> Option<&'static str> {
    if h.mods & WIN != 0 && (0x30..=0x39).contains(&h.vk) {
        return Some("taskbar apps");
    }
    SHORTCUTS.iter().find(|(m, vk, _)| *m == h.mods && *vk == h.vk).map(|(_, _, what)| *what)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_windows_shortcuts() {
        assert_eq!(windows_use(&Hotkey { mods: CONTROL | WIN, vk: b'M' as u16 }), Some("Magnifier settings"));
        assert_eq!(windows_use(&Hotkey { mods: WIN, vk: b'L' as u16 }), Some("lock"));
        assert_eq!(windows_use(&Hotkey { mods: WIN | SHIFT, vk: 0x33 }), Some("taskbar apps"));
        assert_eq!(windows_use(&Hotkey { mods: CONTROL | ALT, vk: b'M' as u16 }), None);
        assert_eq!(windows_use(&Hotkey { mods: CONTROL | ALT | WIN, vk: b'M' as u16 }), None);
    }
}
