//! Windows, window classes and messages, menus, timers, scrolling and DPI.

use super::*;
use core::ffi::c_void;

pub type WNDPROC = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: u32,
    pub pt: POINT,
    pub lPrivate: u32,
}

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: Option<WNDPROC>,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: isize,
    pub hbrBackground: isize,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: HICON,
}

#[repr(C)]
pub struct MINMAXINFO {
    pub ptReserved: POINT,
    pub ptMaxSize: POINT,
    pub ptMaxPosition: POINT,
    pub ptMinTrackSize: POINT,
    pub ptMaxTrackSize: POINT,
}

#[repr(C)]
pub struct TRACKMOUSEEVENT {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub hwndTrack: HWND,
    pub dwHoverTime: u32,
}

#[repr(C)]
pub struct SCROLLINFO {
    pub cbSize: u32,
    pub fMask: u32,
    pub nMin: i32,
    pub nMax: i32,
    pub nPage: u32,
    pub nPos: i32,
    pub nTrackPos: i32,
}

pub const WM_DESTROY: u32 = 0x0002;
pub const WM_SIZE: u32 = 0x0005;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_GETMINMAXINFO: u32 = 0x0024;
pub const WM_SETFONT: u32 = 0x0030;
pub const WM_CONTEXTMENU: u32 = 0x007B;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_DPICHANGED: u32 = 0x02E0;
pub const WM_HOTKEY: u32 = 0x0312;
pub const WM_NULL: u32 = 0x0000;
pub const WM_APP: u32 = 0x8000;
pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
pub const WS_POPUP: u32 = 0x8000_0000;
pub const WS_CHILD: u32 = 0x4000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const WS_CLIPCHILDREN: u32 = 0x0200_0000;
pub const WS_CAPTION: u32 = 0x00C0_0000;
pub const WS_SYSMENU: u32 = 0x0008_0000;
pub const WS_VSCROLL: u32 = 0x0020_0000;
pub const WS_TABSTOP: u32 = 0x0001_0000;
pub const WS_EX_DLGMODALFRAME: u32 = 0x0000_0001;
pub const WS_EX_CONTROLPARENT: u32 = 0x0001_0000;
pub const WS_EX_TOPMOST: u32 = 0x0000_0008;
pub const WS_EX_APPWINDOW: u32 = 0x0004_0000;
pub const CW_USEDEFAULT: i32 = 0x8000_0000u32 as i32;
pub const SW_HIDE: i32 = 0;
pub const SW_SHOWNOACTIVATE: i32 = 4;
pub const SW_SHOW: i32 = 5;
pub const SW_RESTORE: i32 = 9;
pub const SWP_NOMOVE: u32 = 0x0002;
pub const SWP_NOZORDER: u32 = 0x0004;
pub const SWP_NOACTIVATE: u32 = 0x0010;
pub const COLOR_BTNFACE: isize = 15;
pub const IDC_ARROW: usize = 32512;
pub const SM_CXSMICON: i32 = 49;
pub const SM_CXICON: i32 = 11;
pub const WA_INACTIVE: u16 = 0;
pub const WM_ACTIVATE: u32 = 0x0006;
pub const WM_SETFOCUS: u32 = 0x0007;
pub const WM_KILLFOCUS: u32 = 0x0008;
pub const WM_NCDESTROY: u32 = 0x0082;
pub const WM_CTLCOLORSTATIC: u32 = 0x0138;
pub const MF_STRING: u32 = 0x0000;
pub const MF_GRAYED: u32 = 0x0001;
pub const MF_CHECKED: u32 = 0x0008;
pub const MF_POPUP: u32 = 0x0010;
pub const MF_SEPARATOR: u32 = 0x0800;
pub const TPM_RIGHTBUTTON: u32 = 0x0002;
pub const TPM_NONOTIFY: u32 = 0x0080;
pub const TPM_RETURNCMD: u32 = 0x0100;
pub const WM_ERASEBKGND: u32 = 0x0014;
pub const WM_SETTINGCHANGE: u32 = 0x001A;
pub const WM_DRAWITEM: u32 = 0x002B;
pub const WM_MEASUREITEM: u32 = 0x002C;
pub const WM_CTLCOLOREDIT: u32 = 0x0133;
pub const WM_CTLCOLORLISTBOX: u32 = 0x0134;
pub const WM_CTLCOLORBTN: u32 = 0x0135;
pub const WM_SHOWWINDOW: u32 = 0x0018;
pub const WM_MOUSEACTIVATE: u32 = 0x0021;
pub const MA_NOACTIVATE: LRESULT = 3;
pub const WM_GETFONT: u32 = 0x0031;
pub const WM_ACTIVATEAPP: u32 = 0x001C;
pub const WM_MOUSEMOVE: u32 = 0x0200;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_LBUTTONDBLCLK: u32 = 0x0203;
pub const WM_CAPTURECHANGED: u32 = 0x0215;
pub const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
pub const CS_DROPSHADOW: u32 = 0x0002_0000;
pub const WM_MOUSELEAVE: u32 = 0x02A3;
pub const WM_THEMECHANGED: u32 = 0x031A;
pub const MB_ICONEXCLAMATION: u32 = 0x30;
pub const MB_ICONASTERISK: u32 = 0x40;
pub const TME_LEAVE: u32 = 0x0002;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_VSCROLL: u32 = 0x0115;
pub const WM_MOUSEWHEEL: u32 = 0x020A;
pub const SB_VERT: i32 = 1;
pub const SB_LINEUP: u32 = 0;
pub const SB_LINEDOWN: u32 = 1;
pub const SB_PAGEUP: u32 = 2;
pub const SB_PAGEDOWN: u32 = 3;
pub const SB_THUMBPOSITION: u32 = 4;
pub const SB_THUMBTRACK: u32 = 5;
pub const SB_TOP: u32 = 6;
pub const SB_BOTTOM: u32 = 7;
pub const SM_CXSCREEN: i32 = 0;
pub const SM_CYSCREEN: i32 = 1;
pub const SIF_RANGE: u32 = 0x1;
pub const SIF_PAGE: u32 = 0x2;
pub const SIF_POS: u32 = 0x4;
pub const SIF_TRACKPOS: u32 = 0x10;

#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassExW(wc: *const WNDCLASSEXW) -> u16;
    pub fn CreateWindowExW(ex: u32, class: *const u16, name: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32,
        parent: HWND, menu: HMENU, inst: HINSTANCE, param: *const c_void) -> HWND;
    pub fn DefWindowProcW(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT;
    pub fn DestroyWindow(h: HWND) -> BOOL;
    pub fn ShowWindow(h: HWND, cmd: i32) -> BOOL;
    pub fn GetMessageW(m: *mut MSG, h: HWND, min: u32, max: u32) -> BOOL;
    pub fn TranslateMessage(m: *const MSG) -> BOOL;
    pub fn DispatchMessageW(m: *const MSG) -> LRESULT;
    pub fn PostMessageW(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> BOOL;
    pub fn SendMessageW(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT;
    pub fn PostQuitMessage(code: i32);
    pub fn IsDialogMessageW(h: HWND, m: *mut MSG) -> BOOL;
    pub fn SetWindowPos(h: HWND, after: HWND, x: i32, y: i32, w: i32, cy: i32, flags: u32) -> BOOL;
    pub fn GetClientRect(h: HWND, r: *mut RECT) -> BOOL;
    pub fn GetWindowRect(h: HWND, r: *mut RECT) -> BOOL;
    pub fn EnableWindow(h: HWND, enable: BOOL) -> BOOL;
    pub fn SetWindowTextW(h: HWND, s: *const u16) -> BOOL;
    pub fn GetWindowTextW(h: HWND, s: *mut u16, n: i32) -> i32;
    pub fn GetWindowTextLengthW(h: HWND) -> i32;
    pub fn SetForegroundWindow(h: HWND) -> BOOL;
    pub fn SetActiveWindow(h: HWND) -> HWND;
    pub fn IsIconic(h: HWND) -> BOOL;
    pub fn GetLastActivePopup(h: HWND) -> HWND;
    pub fn SetFocus(h: HWND) -> HWND;
    pub fn LoadCursorW(inst: HINSTANCE, name: *const u16) -> isize;
    pub fn SetTimer(h: HWND, id: usize, ms: u32, f: *const c_void) -> usize;
    pub fn KillTimer(h: HWND, id: usize) -> BOOL;
    pub fn CreatePopupMenu() -> HMENU;
    pub fn AppendMenuW(m: HMENU, flags: u32, id: usize, text: *const u16) -> BOOL;
    pub fn TrackPopupMenu(m: HMENU, flags: u32, x: i32, y: i32, reserved: i32, h: HWND, r: *const RECT) -> BOOL;
    pub fn DestroyMenu(m: HMENU) -> BOOL;
    pub fn SetMenuDefaultItem(m: HMENU, item: u32, by_pos: u32) -> BOOL;
    pub fn RegisterWindowMessageW(s: *const u16) -> u32;
    pub fn GetDpiForWindow(h: HWND) -> u32;
    pub fn GetDpiForSystem() -> u32;
    pub fn AdjustWindowRectExForDpi(r: *mut RECT, style: u32, menu: BOOL, ex: u32, dpi: u32) -> BOOL;
    pub fn LoadImageW(inst: HINSTANCE, name: *const u16, kind: u32, cx: i32, cy: i32, flags: u32) -> isize;
    pub fn GetSystemMetrics(i: i32) -> i32;
    pub fn GetFocus() -> HWND;
    pub fn GetForegroundWindow() -> HWND;
    pub fn GetAncestor(h: HWND, flags: u32) -> HWND;
    pub fn IsChild(parent: HWND, h: HWND) -> BOOL;
    pub fn TrackMouseEvent(t: *mut TRACKMOUSEEVENT) -> BOOL;
    pub fn MessageBeep(kind: u32) -> BOOL;
    pub fn GetScrollInfo(h: HWND, bar: i32, si: *mut SCROLLINFO) -> BOOL;
    pub fn SetScrollInfo(h: HWND, bar: i32, si: *const SCROLLINFO, redraw: BOOL) -> i32;
    pub fn GetParent(h: HWND) -> HWND;
    pub fn SetCapture(h: HWND) -> HWND;
    pub fn IsWindow(h: HWND) -> BOOL;
    pub fn GetClassNameW(h: HWND, name: *mut u16, len: i32) -> i32;
    pub fn ReleaseCapture() -> BOOL;
}
