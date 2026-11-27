//! Shortcuts Windows keeps for itself. Some are handled before any app sees the keys, so a
//! hotkey on them can register successfully and still never fire; the hotkey editor warns
//! before saving one. Not exhaustive: the editor also test-registers every new combination.

use crate::hotkey::{Hotkey, ALT, CONTROL, SHIFT, WIN};
use crate::i18n::t;

/// One table entry: the modifiers, the key, and the text key for what Windows uses the
/// combination for (see `lang/en.txt`).
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
const DIGITS: std::ops::RangeInclusive<u16> = 0x30..=0x39;

const SHORTCUTS: &[(u8, u16, &str)] = &[
    // Win + key
    k(WIN, b'A', "shortcut.quick_settings"), k(WIN, b'B', "shortcut.notification_area"), k(WIN, b'C', "shortcut.copilot"),
    k(WIN, b'D', "shortcut.show_desktop"), k(WIN, b'E', "shortcut.file_explorer"), k(WIN, b'G', "shortcut.game_bar"),
    k(WIN, b'H', "shortcut.voice_typing"), k(WIN, b'I', "shortcut.settings"), k(WIN, b'K', "shortcut.cast"), k(WIN, b'L', "shortcut.lock"),
    k(WIN, b'M', "shortcut.minimise_all"), k(WIN, b'N', "shortcut.notifications"), k(WIN, b'O', "shortcut.rotation_lock"),
    k(WIN, b'P', "shortcut.projection"), k(WIN, b'Q', "shortcut.search"), k(WIN, b'R', "shortcut.run"), k(WIN, b'S', "shortcut.search"),
    k(WIN, b'T', "shortcut.taskbar_apps"), k(WIN, b'U', "shortcut.accessibility"), k(WIN, b'V', "shortcut.clipboard"),
    k(WIN, b'W', "shortcut.widgets"), k(WIN, b'X', "shortcut.quick_link"), k(WIN, b'Z', "shortcut.snap_layouts"),
    k(WIN, TAB, "shortcut.task_view"), k(WIN, SPACE, "shortcut.keyboard_layout"), k(WIN, ESCAPE, "shortcut.close_magnifier"),
    k(WIN, LEFT, "shortcut.snap_window"), k(WIN, RIGHT, "shortcut.snap_window"), k(WIN, UP, "shortcut.maximise_window"),
    k(WIN, DOWN, "shortcut.minimise_window"), k(WIN, HOME, "shortcut.minimise_others"), k(WIN, PAUSE, "shortcut.system_settings"),
    k(WIN, PRINT, "shortcut.screenshot"), k(WIN, PLUS, "shortcut.magnifier"), k(WIN, MINUS, "shortcut.magnifier"),
    k(WIN, COMMA, "shortcut.peek"), k(WIN, PERIOD, "shortcut.emoji"), k(WIN, SEMICOLON, "shortcut.emoji"),
    // Win + Shift + key
    k(WIN | SHIFT, b'S', "shortcut.snipping_tool"), k(WIN | SHIFT, b'M', "shortcut.restore_minimised"),
    k(WIN | SHIFT, LEFT, "shortcut.move_window"), k(WIN | SHIFT, RIGHT, "shortcut.move_window"),
    k(WIN | SHIFT, UP, "shortcut.stretch_window"), k(WIN | SHIFT, DOWN, "shortcut.restore_window"),
    // Win + Ctrl + key
    k(WIN | CONTROL, b'D', "shortcut.new_desktop"), k(WIN | CONTROL, F4, "shortcut.close_desktop"),
    k(WIN | CONTROL, LEFT, "shortcut.switch_desktop"), k(WIN | CONTROL, RIGHT, "shortcut.switch_desktop"),
    k(WIN | CONTROL, RETURN, "shortcut.narrator"), k(WIN | CONTROL, b'N', "shortcut.narrator_settings"),
    k(WIN | CONTROL, b'O', "shortcut.on_screen_keyboard"), k(WIN | CONTROL, b'C', "shortcut.colour_filters"),
    k(WIN | CONTROL, b'M', "shortcut.magnifier_settings"), k(WIN | CONTROL, b'S', "shortcut.speech"),
    k(WIN | CONTROL, b'F', "shortcut.find_computers"), k(WIN | CONTROL, b'Q', "shortcut.quick_assist"),
    k(WIN | CONTROL | SHIFT, b'B', "shortcut.restart_graphics"),
    // Win + Alt + key
    k(WIN | ALT, b'D', "shortcut.date_time"), k(WIN | ALT, b'R', "shortcut.recording"), k(WIN | ALT, b'B', "shortcut.hdr"),
    k(WIN | ALT, b'K', "shortcut.mute"), k(WIN | ALT, PRINT, "shortcut.game_bar_screenshot"),
];

/// What Windows uses this combination for, if it's a known Windows shortcut. Digits with Win
/// (with or without Shift, Ctrl or Alt) open or switch taskbar apps.
pub fn windows_use(h: &Hotkey) -> Option<&'static str> {
    if h.mods & WIN != 0 && DIGITS.contains(&h.vk) {
        return Some(t("shortcut.taskbar_apps"));
    }
    SHORTCUTS.iter().find(|(m, vk, _)| *m == h.mods && *vk == h.vk).map(|&(_, _, what)| t(what))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_windows_shortcuts() {
        assert_eq!(windows_use(&Hotkey { mods: CONTROL | WIN, vk: b'M' as u16 }), Some("Magnifier settings"));
        assert_eq!(windows_use(&Hotkey { mods: WIN, vk: b'L' as u16 }), Some("locking the PC"));
        assert_eq!(windows_use(&Hotkey { mods: WIN | SHIFT, vk: 0x33 }), Some("taskbar apps"));
        assert_eq!(windows_use(&Hotkey { mods: CONTROL | ALT, vk: b'M' as u16 }), None);
        assert_eq!(windows_use(&Hotkey { mods: CONTROL | ALT | WIN, vk: b'M' as u16 }), None);
    }
}
