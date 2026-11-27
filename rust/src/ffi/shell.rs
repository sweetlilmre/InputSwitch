//! The notification area (tray icon and notifications), and session, power and device events.

use super::*;

#[repr(C)]
pub struct NOTIFYICONDATAW {
    pub cbSize: u32,
    pub hWnd: HWND,
    pub uID: u32,
    pub uFlags: u32,
    pub uCallbackMessage: u32,
    pub hIcon: HICON,
    pub szTip: [u16; 128],
    pub dwState: u32,
    pub dwStateMask: u32,
    pub szInfo: [u16; 256],
    pub uVersion: u32,
    pub szInfoTitle: [u16; 64],
    pub dwInfoFlags: u32,
    pub guidItem: [u8; 16],
    pub hBalloonIcon: HICON,
}

pub const WM_DISPLAYCHANGE: u32 = 0x007E;
pub const WM_POWERBROADCAST: u32 = 0x0218;
pub const WM_DEVICECHANGE: u32 = 0x0219;
pub const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
pub const PBT_APMRESUMESUSPEND: usize = 0x7;
pub const PBT_APMRESUMEAUTOMATIC: usize = 0x12;
pub const WTS_SESSION_UNLOCK: usize = 0x8;
pub const NIM_ADD: u32 = 0;
pub const NIM_MODIFY: u32 = 1;
pub const NIM_DELETE: u32 = 2;
pub const NIM_SETVERSION: u32 = 4;
pub const NIF_MESSAGE: u32 = 0x01;
pub const NIF_ICON: u32 = 0x02;
pub const NIF_TIP: u32 = 0x04;
pub const NIF_INFO: u32 = 0x10;
pub const NIF_SHOWTIP: u32 = 0x80;
pub const NIIF_NONE: u32 = 0;
pub const NIIF_INFO: u32 = 1;
pub const NIIF_WARNING: u32 = 2;
pub const NIIF_ERROR: u32 = 3;
pub const NOTIFYICON_VERSION_4: u32 = 4;
pub const NIN_SELECT: u16 = 0x0400;
pub const NIN_KEYSELECT: u16 = 0x0401;
pub const NIN_BALLOONUSERCLICK: u16 = 0x0405;

#[link(name = "shell32")]
extern "system" {
    pub fn Shell_NotifyIconW(msg: u32, data: *const NOTIFYICONDATAW) -> BOOL;
}

#[link(name = "wtsapi32")]
extern "system" {
    pub fn WTSRegisterSessionNotification(h: HWND, flags: u32) -> BOOL;
    pub fn WTSUnRegisterSessionNotification(h: HWND) -> BOOL;
}
