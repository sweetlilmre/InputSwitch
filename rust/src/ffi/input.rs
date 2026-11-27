//! The keyboard: key messages, virtual keys, the low-level keyboard hook, and hotkeys.

use super::*;

#[repr(C)]
pub struct KBDLLHOOKSTRUCT {
    pub vkCode: u32,
    pub scanCode: u32,
    pub flags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}

pub type HOOKPROC = unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT;

pub const WM_KEYFIRST: u32 = 0x0100;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_KEYUP: u32 = 0x0101;
pub const WM_SYSKEYDOWN: u32 = 0x0104;
pub const WM_SYSKEYUP: u32 = 0x0105;
pub const WM_KEYLAST: u32 = 0x0109;
pub const WH_KEYBOARD_LL: i32 = 13;
pub const GA_ROOT: u32 = 2;
pub const VK_BACK: u16 = 0x08;
pub const VK_TAB: u16 = 0x09;
pub const VK_RETURN: u16 = 0x0D;
pub const VK_SHIFT: u16 = 0x10;
pub const VK_CONTROL: u16 = 0x11;
pub const VK_MENU: u16 = 0x12;
pub const VK_ESCAPE: u16 = 0x1B;
pub const VK_DELETE: u16 = 0x2E;
pub const VK_LWIN: u16 = 0x5B;
pub const VK_RWIN: u16 = 0x5C;
pub const MOD_NOREPEAT: u32 = 0x4000;

#[link(name = "user32")]
extern "system" {
    pub fn RegisterHotKey(h: HWND, id: i32, mods: u32, vk: u32) -> BOOL;
    pub fn UnregisterHotKey(h: HWND, id: i32) -> BOOL;
    pub fn GetKeyState(vk: i32) -> i16;
    pub fn HideCaret(h: HWND) -> BOOL;
    pub fn SetWindowsHookExW(kind: i32, proc: HOOKPROC, inst: HINSTANCE, thread: u32) -> isize;
    pub fn UnhookWindowsHookEx(hook: isize) -> BOOL;
    pub fn CallNextHookEx(hook: isize, code: i32, w: WPARAM, l: LPARAM) -> LRESULT;
}
