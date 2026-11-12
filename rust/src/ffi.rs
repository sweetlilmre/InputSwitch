//! Hand-written Windows API bindings: only what InputSwitch uses, so no binding crates are needed.

#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use core::ffi::c_void;

pub type HWND = isize;
pub type HANDLE = isize;
pub type HINSTANCE = isize;
pub type HMENU = isize;
pub type HICON = isize;
pub type HFONT = isize;
pub type HMONITOR = isize;
pub type HKEY = isize;
pub type HDC = isize;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type BOOL = i32;

pub type WNDPROC = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;
pub type MONITORENUMPROC = unsafe extern "system" fn(HMONITOR, HDC, *mut RECT, LPARAM) -> BOOL;

/// All-zero value of a plain C struct.
pub fn zeroed<T>() -> T {
    unsafe { core::mem::zeroed() }
}

/// NUL-terminated UTF-16 copy of a string.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// String from a (possibly NUL-terminated) UTF-16 buffer.
pub fn from_wide(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

/// Copies a string into a fixed UTF-16 buffer, truncating and NUL-terminating.
pub fn copy_wide(dst: &mut [u16], s: &str) {
    let mut i = 0;
    for c in s.encode_utf16() {
        if i + 1 >= dst.len() {
            break;
        }
        dst[i] = c;
        i += 1;
    }
    dst[i] = 0;
}

pub fn loword(x: usize) -> u16 {
    (x & 0xFFFF) as u16
}
pub fn hiword(x: usize) -> u16 {
    ((x >> 16) & 0xFFFF) as u16
}

// ------------------------------------------------------------------------------------------ structs

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
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

#[repr(C)]
pub struct NMHDR {
    pub hwndFrom: HWND,
    pub idFrom: usize,
    pub code: u32,
}

#[repr(C)]
pub struct NMLISTVIEW {
    pub hdr: NMHDR,
    pub iItem: i32,
    pub iSubItem: i32,
    pub uNewState: u32,
    pub uOldState: u32,
    pub uChanged: u32,
    pub ptAction: POINT,
    pub lParam: LPARAM,
}

#[repr(C)]
pub struct LVCOLUMNW {
    pub mask: u32,
    pub fmt: i32,
    pub cx: i32,
    pub pszText: *mut u16,
    pub cchTextMax: i32,
    pub iSubItem: i32,
    pub iImage: i32,
    pub iOrder: i32,
    pub cxMin: i32,
    pub cxDefault: i32,
    pub cxIdeal: i32,
}

#[repr(C)]
pub struct LVITEMW {
    pub mask: u32,
    pub iItem: i32,
    pub iSubItem: i32,
    pub state: u32,
    pub stateMask: u32,
    pub pszText: *mut u16,
    pub cchTextMax: i32,
    pub iImage: i32,
    pub lParam: LPARAM,
    pub iIndent: i32,
    pub iGroupId: i32,
    pub cColumns: u32,
    pub puColumns: *mut u32,
    pub piColFmt: *mut i32,
    pub iGroup: i32,
}

#[repr(C)]
pub struct NMLVDISPINFOW {
    pub hdr: NMHDR,
    pub item: LVITEMW,
}

#[repr(C)]
pub struct INITCOMMONCONTROLSEX {
    pub dwSize: u32,
    pub dwICC: u32,
}

#[repr(C)]
pub struct MONITORINFOEXW {
    pub cbSize: u32,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: u32,
    pub szDevice: [u16; 32],
}

#[repr(C)]
pub struct PHYSICAL_MONITOR {
    pub hPhysicalMonitor: HANDLE,
    pub szPhysicalMonitorDescription: [u16; 128],
}

#[repr(C)]
pub struct SYSTEMTIME {
    pub wYear: u16,
    pub wMonth: u16,
    pub wDayOfWeek: u16,
    pub wDay: u16,
    pub wHour: u16,
    pub wMinute: u16,
    pub wSecond: u16,
    pub wMilliseconds: u16,
}

// CCD (display configuration) API.

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LUID {
    pub LowPart: u32,
    pub HighPart: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DISPLAYCONFIG_PATH_SOURCE_INFO {
    pub adapterId: LUID,
    pub id: u32,
    pub modeInfoIdx: u32,
    pub statusFlags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DISPLAYCONFIG_PATH_TARGET_INFO {
    pub adapterId: LUID,
    pub id: u32,
    pub modeInfoIdx: u32,
    pub outputTechnology: u32,
    pub rotation: u32,
    pub scaling: u32,
    pub refreshNumerator: u32,
    pub refreshDenominator: u32,
    pub scanLineOrdering: u32,
    pub targetAvailable: BOOL,
    pub statusFlags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DISPLAYCONFIG_PATH_INFO {
    pub sourceInfo: DISPLAYCONFIG_PATH_SOURCE_INFO,
    pub targetInfo: DISPLAYCONFIG_PATH_TARGET_INFO,
    pub flags: u32,
}

/// Opaque 64-byte DISPLAYCONFIG_MODE_INFO (only needed as buffer space).
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct DISPLAYCONFIG_MODE_INFO {
    pub data: [u8; 64],
}

#[repr(C)]
pub struct DISPLAYCONFIG_DEVICE_INFO_HEADER {
    pub type_: u32,
    pub size: u32,
    pub adapterId: LUID,
    pub id: u32,
}

#[repr(C)]
pub struct DISPLAYCONFIG_TARGET_DEVICE_NAME {
    pub header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    pub flags: u32,
    pub outputTechnology: u32,
    pub edidManufactureId: u16,
    pub edidProductCodeId: u16,
    pub connectorInstance: u32,
    pub monitorFriendlyDeviceName: [u16; 64],
    pub monitorDevicePath: [u16; 128],
}

#[repr(C)]
pub struct DISPLAYCONFIG_SOURCE_DEVICE_NAME {
    pub header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    pub viewGdiDeviceName: [u16; 32],
}

// ------------------------------------------------------------------------------------------ constants

// Window messages.
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_SIZE: u32 = 0x0005;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_GETMINMAXINFO: u32 = 0x0024;
pub const WM_SETFONT: u32 = 0x0030;
pub const WM_NOTIFY: u32 = 0x004E;
pub const WM_CONTEXTMENU: u32 = 0x007B;
pub const WM_DISPLAYCHANGE: u32 = 0x007E;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_POWERBROADCAST: u32 = 0x0218;
pub const WM_DEVICECHANGE: u32 = 0x0219;
pub const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
pub const WM_DPICHANGED: u32 = 0x02E0;
pub const WM_HOTKEY: u32 = 0x0312;
pub const WM_NULL: u32 = 0x0000;
pub const WM_APP: u32 = 0x8000;

pub const PBT_APMRESUMESUSPEND: usize = 0x7;
pub const PBT_APMRESUMEAUTOMATIC: usize = 0x12;
pub const WTS_SESSION_UNLOCK: usize = 0x8;

// Window styles.
pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
pub const WS_POPUP: u32 = 0x8000_0000;
pub const WS_CHILD: u32 = 0x4000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const WS_CLIPCHILDREN: u32 = 0x0200_0000;
pub const WS_CAPTION: u32 = 0x00C0_0000;
pub const WS_SYSMENU: u32 = 0x0008_0000;
pub const WS_VSCROLL: u32 = 0x0020_0000;
pub const WS_TABSTOP: u32 = 0x0001_0000;
pub const WS_EX_CLIENTEDGE: u32 = 0x0000_0200;
pub const WS_EX_DLGMODALFRAME: u32 = 0x0000_0001;
pub const WS_EX_CONTROLPARENT: u32 = 0x0001_0000;
pub const CW_USEDEFAULT: i32 = 0x8000_0000u32 as i32;

pub const SW_SHOW: i32 = 5;
pub const SW_RESTORE: i32 = 9;

pub const SWP_NOZORDER: u32 = 0x0004;
pub const SWP_NOACTIVATE: u32 = 0x0010;

pub const COLOR_BTNFACE: isize = 15;
pub const IDC_ARROW: usize = 32512;
pub const SM_CXSMICON: i32 = 49;

// Controls.
pub const BS_PUSHBUTTON: u32 = 0x0;
pub const BS_DEFPUSHBUTTON: u32 = 0x1;
pub const BS_AUTOCHECKBOX: u32 = 0x3;
pub const BM_GETCHECK: u32 = 0x00F0;
pub const BM_SETCHECK: u32 = 0x00F1;
pub const BN_CLICKED: u16 = 0;
pub const ES_AUTOHSCROLL: u32 = 0x0080;
pub const EN_CHANGE: u16 = 0x0300;
pub const SS_ENDELLIPSIS: u32 = 0x4000;
pub const SS_NOPREFIX: u32 = 0x0080;
pub const LBS_NOTIFY: u32 = 0x0001;
pub const LBS_NOINTEGRALHEIGHT: u32 = 0x0100;
pub const LB_ADDSTRING: u32 = 0x0180;
pub const LB_INSERTSTRING: u32 = 0x0181;
pub const LB_DELETESTRING: u32 = 0x0182;
pub const LB_RESETCONTENT: u32 = 0x0184;
pub const LB_SETCURSEL: u32 = 0x0186;
pub const LB_GETCURSEL: u32 = 0x0188;
pub const LBN_SELCHANGE: u16 = 1;
pub const CBS_DROPDOWNLIST: u32 = 0x0003;
pub const CB_ADDSTRING: u32 = 0x0143;
pub const CB_GETCURSEL: u32 = 0x0147;
pub const CB_RESETCONTENT: u32 = 0x014B;
pub const CB_SETCURSEL: u32 = 0x014E;
pub const CBN_SELCHANGE: u16 = 1;
pub const IDOK: i32 = 1;
pub const IDCANCEL: i32 = 2;
pub const IDYES: i32 = 6;
pub const IDNO: i32 = 7;

// List view.
pub const LVS_REPORT: u32 = 0x0001;
pub const LVS_SINGLESEL: u32 = 0x0004;
pub const LVS_SHOWSELALWAYS: u32 = 0x0008;
pub const LVS_EDITLABELS: u32 = 0x0200;
pub const LVS_NOCOLUMNHEADER: u32 = 0x4000;
pub const LVS_NOSORTHEADER: u32 = 0x8000;
pub const LVS_EX_CHECKBOXES: u32 = 0x0004;
pub const LVS_EX_FULLROWSELECT: u32 = 0x0020;
pub const LVS_EX_DOUBLEBUFFER: u32 = 0x0001_0000;
pub const LVM_GETITEMCOUNT: u32 = 0x1004;
pub const LVM_DELETEALLITEMS: u32 = 0x1009;
pub const LVM_GETNEXTITEM: u32 = 0x100C;
pub const LVM_ENSUREVISIBLE: u32 = 0x1013;
pub const LVM_SETCOLUMNWIDTH: u32 = 0x101E;
pub const LVM_SETITEMSTATE: u32 = 0x102B;
pub const LVM_GETITEMSTATE: u32 = 0x102C;
pub const LVM_SETEXTENDEDLISTVIEWSTYLE: u32 = 0x1036;
pub const LVM_GETITEMW: u32 = 0x104B;
pub const LVM_INSERTITEMW: u32 = 0x104D;
pub const LVM_INSERTCOLUMNW: u32 = 0x1061;
pub const LVM_SETITEMTEXTW: u32 = 0x1074;
pub const LVM_EDITLABELW: u32 = 0x1076;
pub const LVIF_TEXT: u32 = 0x1;
pub const LVIF_STATE: u32 = 0x8;
pub const LVIF_PARAM: u32 = 0x4;
pub const LVCF_TEXT: u32 = 0x4;
pub const LVCF_WIDTH: u32 = 0x2;
pub const LVIS_FOCUSED: u32 = 0x1;
pub const LVIS_SELECTED: u32 = 0x2;
pub const LVIS_STATEIMAGEMASK: u32 = 0xF000;
pub const LVNI_SELECTED: isize = 0x2;
pub const LVSCW_AUTOSIZE_USEHEADER: isize = -2;
pub const NM_DBLCLK: u32 = (-3i32) as u32;
pub const LVN_ITEMCHANGED: u32 = (-101i32) as u32;
pub const LVN_ENDLABELEDITW: u32 = (-176i32) as u32;

pub const ICC_LISTVIEW_CLASSES: u32 = 0x0001;
pub const ICC_STANDARD_CLASSES: u32 = 0x4000;

// Keyboard (for the hotkey capture box).
pub const WM_ACTIVATE: u32 = 0x0006;
pub const WM_SETFOCUS: u32 = 0x0007;
pub const WM_KILLFOCUS: u32 = 0x0008;
pub const WM_NCDESTROY: u32 = 0x0082;
pub const WM_KEYFIRST: u32 = 0x0100;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_KEYUP: u32 = 0x0101;
pub const WM_SYSKEYDOWN: u32 = 0x0104;
pub const WM_SYSKEYUP: u32 = 0x0105;
pub const WM_KEYLAST: u32 = 0x0109;
pub const WM_CTLCOLORSTATIC: u32 = 0x0138;
pub const ES_READONLY: u32 = 0x0800;
pub const COLOR_WINDOW: i32 = 5;
pub const COLOR_WINDOWTEXT: i32 = 8;
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

#[repr(C)]
pub struct KBDLLHOOKSTRUCT {
    pub vkCode: u32,
    pub scanCode: u32,
    pub flags: u32,
    pub time: u32,
    pub dwExtraInfo: usize,
}

pub type HOOKPROC = unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT;
pub type SUBCLASSPROC = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM, usize, usize) -> LRESULT;

// Hotkeys.
pub const MOD_NOREPEAT: u32 = 0x4000;

// Menus.
pub const MF_STRING: u32 = 0x0000;
pub const MF_GRAYED: u32 = 0x0001;
pub const MF_CHECKED: u32 = 0x0008;
pub const MF_POPUP: u32 = 0x0010;
pub const MF_SEPARATOR: u32 = 0x0800;
pub const TPM_RIGHTBUTTON: u32 = 0x0002;
pub const TPM_NONOTIFY: u32 = 0x0080;
pub const TPM_RETURNCMD: u32 = 0x0100;

// Notification area.
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

// TaskDialog.
pub const TDCBF_OK_BUTTON: u32 = 0x1;
pub const TDCBF_YES_BUTTON: u32 = 0x2;
pub const TDCBF_NO_BUTTON: u32 = 0x4;
pub const TDCBF_CANCEL_BUTTON: u32 = 0x8;
pub const TD_WARNING_ICON: *const u16 = 0xFFFF as *const u16;
pub const TD_INFORMATION_ICON: *const u16 = 0xFFFD as *const u16;

// Registry.
pub const HKEY_CURRENT_USER: HKEY = 0x8000_0001u32 as i32 as isize;
pub const HKEY_LOCAL_MACHINE: HKEY = 0x8000_0002u32 as i32 as isize;
pub const KEY_READ: u32 = 0x20019;
pub const KEY_WRITE: u32 = 0x20006;
pub const REG_SZ: u32 = 1;

// CCD.
pub const QDC_ONLY_ACTIVE_PATHS: u32 = 0x2;
pub const ERROR_INSUFFICIENT_BUFFER: i32 = 122;
pub const ERROR_ALREADY_EXISTS: u32 = 183;
pub const DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME: u32 = 1;
pub const DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME: u32 = 2;

// ------------------------------------------------------------------------------------------ functions

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
    pub fn SetFocus(h: HWND) -> HWND;
    pub fn LoadCursorW(inst: HINSTANCE, name: *const u16) -> isize;
    pub fn RegisterHotKey(h: HWND, id: i32, mods: u32, vk: u32) -> BOOL;
    pub fn UnregisterHotKey(h: HWND, id: i32) -> BOOL;
    pub fn SetTimer(h: HWND, id: usize, ms: u32, f: *const c_void) -> usize;
    pub fn KillTimer(h: HWND, id: usize) -> BOOL;
    pub fn CreatePopupMenu() -> HMENU;
    pub fn AppendMenuW(m: HMENU, flags: u32, id: usize, text: *const u16) -> BOOL;
    pub fn TrackPopupMenu(m: HMENU, flags: u32, x: i32, y: i32, reserved: i32, h: HWND, r: *const RECT) -> BOOL;
    pub fn DestroyMenu(m: HMENU) -> BOOL;
    pub fn SetMenuDefaultItem(m: HMENU, item: u32, by_pos: u32) -> BOOL;
    pub fn GetCursorPos(p: *mut POINT) -> BOOL;
    pub fn EnumDisplayMonitors(hdc: HDC, clip: *const RECT, f: MONITORENUMPROC, data: LPARAM) -> BOOL;
    pub fn GetMonitorInfoW(m: HMONITOR, info: *mut MONITORINFOEXW) -> BOOL;
    pub fn GetDisplayConfigBufferSizes(flags: u32, paths: *mut u32, modes: *mut u32) -> i32;
    pub fn QueryDisplayConfig(flags: u32, np: *mut u32, paths: *mut DISPLAYCONFIG_PATH_INFO, nm: *mut u32,
        modes: *mut DISPLAYCONFIG_MODE_INFO, topology: *mut c_void) -> i32;
    pub fn DisplayConfigGetDeviceInfo(header: *mut DISPLAYCONFIG_DEVICE_INFO_HEADER) -> i32;
    pub fn RegisterWindowMessageW(s: *const u16) -> u32;
    pub fn GetDpiForWindow(h: HWND) -> u32;
    pub fn GetDpiForSystem() -> u32;
    pub fn AdjustWindowRectExForDpi(r: *mut RECT, style: u32, menu: BOOL, ex: u32, dpi: u32) -> BOOL;
    pub fn LoadImageW(inst: HINSTANCE, name: *const u16, kind: u32, cx: i32, cy: i32, flags: u32) -> isize;
    pub fn DestroyIcon(h: HICON) -> BOOL;
    pub fn GetSystemMetrics(i: i32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateFontW(h: i32, w: i32, esc: i32, orient: i32, weight: i32, italic: u32, underline: u32, strike: u32,
        charset: u32, out_prec: u32, clip_prec: u32, quality: u32, pitch: u32, face: *const u16) -> HFONT;
    pub fn DeleteObject(o: isize) -> BOOL;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
    pub fn CreateMutexW(attrs: *const c_void, owner: BOOL, name: *const u16) -> HANDLE;
    pub fn GetLastError() -> u32;
    pub fn GetSystemTime(t: *mut SYSTEMTIME);
}

#[link(name = "shell32")]
extern "system" {
    pub fn Shell_NotifyIconW(msg: u32, data: *const NOTIFYICONDATAW) -> BOOL;
}

#[link(name = "dxva2")]
extern "system" {
    pub fn GetNumberOfPhysicalMonitorsFromHMONITOR(m: HMONITOR, n: *mut u32) -> BOOL;
    pub fn GetPhysicalMonitorsFromHMONITOR(m: HMONITOR, n: u32, out: *mut PHYSICAL_MONITOR) -> BOOL;
    pub fn DestroyPhysicalMonitors(n: u32, arr: *const PHYSICAL_MONITOR) -> BOOL;
    pub fn GetVCPFeatureAndVCPFeatureReply(h: HANDLE, code: u8, kind: *mut u32, cur: *mut u32, max: *mut u32) -> BOOL;
    pub fn SetVCPFeature(h: HANDLE, code: u8, value: u32) -> BOOL;
    pub fn GetCapabilitiesStringLength(h: HANDLE, len: *mut u32) -> BOOL;
    pub fn CapabilitiesRequestAndCapabilitiesReply(h: HANDLE, buf: *mut u8, len: u32) -> BOOL;
}

#[link(name = "advapi32")]
extern "system" {
    pub fn RegOpenKeyExW(key: HKEY, sub: *const u16, opts: u32, sam: u32, out: *mut HKEY) -> i32;
    pub fn RegCreateKeyExW(key: HKEY, sub: *const u16, reserved: u32, class: *const u16, opts: u32, sam: u32,
        sec: *const c_void, out: *mut HKEY, disp: *mut u32) -> i32;
    pub fn RegQueryValueExW(key: HKEY, name: *const u16, reserved: *mut u32, kind: *mut u32, data: *mut u8, len: *mut u32) -> i32;
    pub fn RegSetValueExW(key: HKEY, name: *const u16, reserved: u32, kind: u32, data: *const u8, len: u32) -> i32;
    pub fn RegDeleteValueW(key: HKEY, name: *const u16) -> i32;
    pub fn RegCloseKey(key: HKEY) -> i32;
}

#[link(name = "comctl32")]
extern "system" {
    pub fn InitCommonControlsEx(icc: *const INITCOMMONCONTROLSEX) -> BOOL;
    pub fn TaskDialog(owner: HWND, inst: HINSTANCE, title: *const u16, main: *const u16, content: *const u16,
        buttons: u32, icon: *const u16, pressed: *mut i32) -> i32;
    pub fn SetWindowSubclass(h: HWND, proc: SUBCLASSPROC, id: usize, data: usize) -> BOOL;
    pub fn RemoveWindowSubclass(h: HWND, proc: SUBCLASSPROC, id: usize) -> BOOL;
    pub fn DefSubclassProc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT;
}

#[link(name = "user32")]
extern "system" {
    pub fn GetKeyState(vk: i32) -> i16;
    pub fn GetFocus() -> HWND;
    pub fn GetForegroundWindow() -> HWND;
    pub fn GetAncestor(h: HWND, flags: u32) -> HWND;
    pub fn HideCaret(h: HWND) -> BOOL;
    pub fn IsChild(parent: HWND, h: HWND) -> BOOL;
    pub fn InvalidateRect(h: HWND, r: *const RECT, erase: BOOL) -> BOOL;
    pub fn GetSysColor(index: i32) -> u32;
    pub fn GetSysColorBrush(index: i32) -> isize;
    pub fn SetWindowsHookExW(kind: i32, proc: HOOKPROC, inst: HINSTANCE, thread: u32) -> isize;
    pub fn UnhookWindowsHookEx(hook: isize) -> BOOL;
    pub fn CallNextHookEx(hook: isize, code: i32, w: WPARAM, l: LPARAM) -> LRESULT;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn SetBkColor(hdc: HDC, color: u32) -> u32;
    pub fn SetTextColor(hdc: HDC, color: u32) -> u32;
    pub fn CreateSolidBrush(color: u32) -> isize;
}

#[link(name = "uxtheme")]
extern "system" {
    pub fn SetWindowTheme(h: HWND, app: *const u16, idlist: *const u16) -> i32;
}

#[link(name = "wtsapi32")]
extern "system" {
    pub fn WTSRegisterSessionNotification(h: HWND, flags: u32) -> BOOL;
    pub fn WTSUnRegisterSessionNotification(h: HWND) -> BOOL;
}

#[link(name = "bcrypt")]
extern "system" {
    pub fn BCryptGenRandom(alg: isize, buf: *mut u8, len: u32, flags: u32) -> i32;
}

pub fn hinstance() -> HINSTANCE {
    unsafe { GetModuleHandleW(core::ptr::null()) }
}
