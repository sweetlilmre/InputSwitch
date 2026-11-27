//! A text box that records a key combination, Win key included (Windows' own hotkey control
//! only knows Ctrl, Alt and Shift).
//!
//! It's a read-only EDIT control, subclassed to take over its keystrokes. While it has focus in
//! the active window, a low-level keyboard hook swallows the Win key, so Win+M is recorded
//! instead of minimising everything and Win alone doesn't open Start. The hook is removed as
//! soon as the box loses focus. There is one box at a time (in the modal hotkey editor).

use crate::ffi::*;
use crate::hotkey::{self, Hotkey};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

/// Posted to the box by the hook when Win goes down or up.
const WM_WIN_CHANGED: u32 = WM_APP + 10;
const SUBCLASS_ID: usize = 1;
/// GetAncestor's "the parent window".
const GA_PARENT: u32 = 1;
const PLACEHOLDER_LISTENING: &str = "Press the keys now…";
const PLACEHOLDER_IDLE: &str = "Click here to record a key combination";
const HINT_LISTENING: &str = "Listening — press the keys now. Win shortcuts are captured while this box is selected. Backspace clears it.";
const HINT_IDLE: &str = "Click the box and press the keys. Use Ctrl, Alt or Win with a key (or F13–F24). Backspace clears it.";

/// The box, or 0 when there isn't one. The hook needs it, so it's shared rather than per window.
static BOX: AtomicIsize = AtomicIsize::new(0);
/// The label that says whether the box is listening (0 for none).
static HINT: AtomicIsize = AtomicIsize::new(0);
/// The box has keyboard focus, so it's recording (and the hook is on).
static LISTENING: AtomicBool = AtomicBool::new(false);
/// The low-level keyboard hook, or 0 while it's off.
static HOOK: AtomicIsize = AtomicIsize::new(0);
/// Win is down (as seen by the hook, which hides it from the rest of the system).
static WIN_DOWN: AtomicBool = AtomicBool::new(false);

thread_local! {
    static CAPTURED: Cell<Hotkey> = const { Cell::new(Hotkey { mods: 0, vk: 0 }) };
}

/// What a key press means for the box.
#[derive(Debug, PartialEq)]
pub enum KeyAction {
    /// Only modifiers so far: show them as a hint.
    Partial(u8),
    /// Backspace or Delete on their own: clear the box.
    Clear,
    /// A complete combination.
    Set(Hotkey),
}

/// What pressing key `vk` with modifiers `mods` held does to the box.
pub fn classify(vk: u16, mods: u8) -> KeyAction {
    match vk {
        VK_SHIFT | VK_CONTROL | VK_MENU | VK_LWIN | VK_RWIN | 0xA0..=0xA5 // left/right Shift, Ctrl, Alt
        | 0xE5 | 0xE7 | 0xFF => KeyAction::Partial(mods), // IME processing, packet, unassigned
        VK_BACK | VK_DELETE if mods == 0 => KeyAction::Clear,
        _ => KeyAction::Set(Hotkey { mods, vk }),
    }
}

/// Plain Tab, Enter and Esc keep their dialog meaning (next field, Save, Cancel); everything
/// else typed into the box is recorded.
fn is_dialog_key(vk: u16, mods: u8) -> bool {
    match vk {
        VK_TAB => mods & !hotkey::SHIFT == 0,
        VK_RETURN | VK_ESCAPE => mods == 0,
        _ => false,
    }
}

fn down(vk: u16) -> bool {
    unsafe { GetKeyState(vk as i32) < 0 }
}

fn current_mods() -> u8 {
    let mut m = 0;
    if down(VK_CONTROL) { m |= hotkey::CONTROL; }
    if down(VK_MENU) { m |= hotkey::ALT; }
    if down(VK_SHIFT) { m |= hotkey::SHIFT; }
    if WIN_DOWN.load(Ordering::Relaxed) || down(VK_LWIN) || down(VK_RWIN) { m |= hotkey::WIN; }
    m
}

/// Creates the box as a child of `parent` with the given control id and font. `hint` is a label
/// that's kept up to date with whether the box is listening.
pub fn create(parent: HWND, id: i32, font: HFONT, initial: Hotkey, hint: HWND) -> HWND {
    // No border: the editor paints a text box frame around it (with an accent underline while listening).
    let h = crate::ui::child(parent, "EDIT", "", ES_READONLY | ES_AUTOHSCROLL | WS_TABSTOP, 0, id, font);
    CAPTURED.with(|c| c.set(initial));
    BOX.store(h, Ordering::Relaxed);
    HINT.store(hint, Ordering::Relaxed);
    LISTENING.store(false, Ordering::Relaxed);
    unsafe { SetWindowSubclass(h, box_proc, SUBCLASS_ID, 0) };
    set_listening(h, false);
    h
}

/// The combination recorded so far (empty if none).
pub fn value() -> Hotkey {
    CAPTURED.with(|c| c.get())
}

/// For the modal loop: send this key message straight to the box (no dialog handling)?
pub fn is_raw_key(msg: &MSG) -> bool {
    let b = BOX.load(Ordering::Relaxed);
    b != 0 && msg.hwnd == b && !(msg.message == WM_KEYDOWN && is_dialog_key(msg.wParam as u16, current_mods()))
}

/// The box's background: a text box's, darker while it's recording (it has focus then).
fn background() -> u32 {
    crate::ui::text_box::field_color(LISTENING.load(Ordering::Relaxed))
}

/// Whether the box is recording (it has focus), so its frame shows the accent underline.
pub fn is_listening() -> bool {
    LISTENING.load(Ordering::Relaxed)
}

/// Paints the box with the theme's colours rather than the grey of a read-only edit.
/// Call from the parent's WM_CTLCOLORSTATIC; returns the brush if it was for the box.
pub fn ctl_color(hdc: HDC, control: HWND) -> Option<LRESULT> {
    if control != BOX.load(Ordering::Relaxed) {
        return None;
    }
    let bg = background();
    unsafe {
        SetTextColor(hdc, crate::ui::theme::palette().text);
        SetBkColor(hdc, bg);
    }
    Some(crate::ui::theme::brush(bg))
}

/// Updates the tint, the hint and the placeholder for whether the box is listening.
fn set_listening(h: HWND, on: bool) {
    LISTENING.store(on, Ordering::Relaxed);
    let hint = HINT.load(Ordering::Relaxed);
    if hint != 0 {
        crate::ui::set_text(hint, if on { HINT_LISTENING } else { HINT_IDLE });
    }
    show(h, None);
    unsafe {
        InvalidateRect(h, core::ptr::null(), 1);
        // The frame around the box (drawn by the editor) changes colour too.
        RedrawWindow(GetAncestor(h, GA_PARENT), core::ptr::null(), 0, RDW_INVALIDATE | RDW_ERASE);
    }
}

/// Shows the modifiers held so far (`partial`), else the recorded combination, else a placeholder.
fn show(h: HWND, partial: Option<u8>) {
    let text = match partial {
        Some(m) if m != 0 => hotkey::partial_display(m),
        _ => {
            let v = value();
            if !v.is_empty() {
                v.display()
            } else if LISTENING.load(Ordering::Relaxed) {
                PLACEHOLDER_LISTENING.to_string()
            } else {
                PLACEHOLDER_IDLE.to_string()
            }
        }
    };
    crate::ui::set_text(h, &text);
}

/// Shows the modifiers held now, or the recorded combination once they're all released.
fn show_held(h: HWND) {
    let mods = current_mods();
    show(h, (mods != 0).then_some(mods));
}

/// Records a combination (or clears it, with the empty one) and shows it.
fn record(h: HWND, hk: Hotkey) {
    CAPTURED.with(|c| c.set(hk));
    show(h, None);
}

unsafe extern "system" fn box_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM, _id: usize, _data: usize) -> LRESULT {
    match m {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            match classify(w as u16, current_mods()) {
                KeyAction::Partial(mods) => show(h, Some(mods)),
                KeyAction::Clear => record(h, Hotkey::default()),
                KeyAction::Set(hk) => record(h, hk),
            }
            return 0;
        }
        WM_KEYUP | WM_SYSKEYUP => {
            show_held(h);
            return 0;
        }
        _ if (WM_KEYFIRST..=WM_KEYLAST).contains(&m) => return 0, // characters: nothing to type
        WM_WIN_CHANGED => {
            show_held(h);
            return 0;
        }
        WM_SETFOCUS => {
            let r = DefSubclassProc(h, m, w, l);
            HideCaret(h);
            WIN_DOWN.store(false, Ordering::Relaxed);
            if HOOK.load(Ordering::Relaxed) == 0 {
                HOOK.store(SetWindowsHookExW(WH_KEYBOARD_LL, hook_proc, hinstance(), 0), Ordering::Relaxed);
            }
            set_listening(h, true);
            return r;
        }
        WM_KILLFOCUS => {
            unhook();
            set_listening(h, false);
        }
        WM_CONTEXTMENU => return 0, // no Cut/Copy/Paste menu
        WM_NCDESTROY => {
            unhook();
            BOX.store(0, Ordering::Relaxed);
            HINT.store(0, Ordering::Relaxed);
            LISTENING.store(false, Ordering::Relaxed);
            RemoveWindowSubclass(h, box_proc, SUBCLASS_ID);
        }
        _ => {}
    }
    DefSubclassProc(h, m, w, l)
}

/// Removes the keyboard hook, if it's on.
fn unhook() {
    let hook = HOOK.swap(0, Ordering::Relaxed);
    if hook != 0 {
        unsafe { UnhookWindowsHookEx(hook) };
    }
    WIN_DOWN.store(false, Ordering::Relaxed);
}

/// Swallows the Win keys while the box has focus in the foreground window, remembering their
/// state for `current_mods`. All other keys pass through untouched.
unsafe extern "system" fn hook_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let k = &*(l as *const KBDLLHOOKSTRUCT);
        let b = BOX.load(Ordering::Relaxed);
        let is_win = k.vkCode == VK_LWIN as u32 || k.vkCode == VK_RWIN as u32;
        if is_win && b != 0 && GetFocus() == b && GetForegroundWindow() == GetAncestor(b, GA_ROOT) {
            let pressed = w as u32 == WM_KEYDOWN || w as u32 == WM_SYSKEYDOWN;
            WIN_DOWN.store(pressed, Ordering::Relaxed);
            PostMessageW(b, WM_WIN_CHANGED, 0, 0);
            return 1;
        }
    }
    CallNextHookEx(0, code, w, l)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::{ALT, CONTROL, SHIFT, WIN};

    #[test]
    fn key_presses() {
        assert_eq!(classify(VK_CONTROL, CONTROL), KeyAction::Partial(CONTROL));
        assert_eq!(classify(VK_LWIN, WIN), KeyAction::Partial(WIN));
        assert_eq!(classify(0xA4, CONTROL | ALT), KeyAction::Partial(CONTROL | ALT)); // left Alt
        assert_eq!(classify(b'M' as u16, CONTROL | WIN), KeyAction::Set(Hotkey { mods: CONTROL | WIN, vk: b'M' as u16 }));
        assert_eq!(classify(VK_BACK, 0), KeyAction::Clear);
        assert_eq!(classify(VK_DELETE, CONTROL | ALT), KeyAction::Set(Hotkey { mods: CONTROL | ALT, vk: VK_DELETE }));
    }

    #[test]
    fn dialog_keys_keep_their_meaning() {
        assert!(is_dialog_key(VK_TAB, 0));
        assert!(is_dialog_key(VK_TAB, SHIFT));
        assert!(!is_dialog_key(VK_TAB, CONTROL));
        assert!(is_dialog_key(VK_RETURN, 0));
        assert!(!is_dialog_key(VK_RETURN, CONTROL));
        assert!(is_dialog_key(VK_ESCAPE, 0));
        assert!(!is_dialog_key(b'M' as u16, 0));
    }
}
