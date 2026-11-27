//! Shared Win32 UI helpers: controls, fonts, list views, task dialogs, modal loops, the app icon.

use crate::ffi::*;
use core::ptr::null;

pub const TITLE: &str = "InputSwitch";

pub fn scale(v: i32, dpi: u32) -> i32 {
    (v as i64 * dpi as i64 / 96) as i32
}

pub fn make_font(dpi: u32, points: i32, weight: i32) -> HFONT {
    font_face("Segoe UI", dpi, points, weight)
}

fn font_face(face: &str, dpi: u32, points: i32, weight: i32) -> HFONT {
    unsafe {
        // DEFAULT_CHARSET = 1, CLEARTYPE_QUALITY = 5
        CreateFontW(-(points * dpi as i32 / 72), 0, 0, 0, weight, 0, 0, 0, 1, 0, 0, 5, 0, wide(face).as_ptr())
    }
}

/// Segoe Fluent Icons on Windows 11; Segoe MDL2 Assets (mostly the same glyphs) on Windows 10.
fn icon_face() -> &'static str {
    let installed = unsafe {
        let mut key: HKEY = 0;
        let path = wide(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts");
        let found = RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.as_ptr(), 0, KEY_READ, &mut key) == 0
            && RegQueryValueExW(key, wide("Segoe Fluent Icons (TrueType)").as_ptr(), core::ptr::null_mut(),
                core::ptr::null_mut(), core::ptr::null_mut(), core::ptr::null_mut()) == 0;
        if key != 0 { RegCloseKey(key); }
        found
    };
    if installed { "Segoe Fluent Icons" } else { "Segoe MDL2 Assets" }
}

pub struct Fonts {
    pub normal: HFONT,
    pub bold: HFONT,
    pub title: HFONT,
    /// Section headings ("Configurations", "Monitors", "Hotkeys").
    pub section: HFONT,
    /// Fluent icon glyphs for buttons.
    pub icon: HFONT,
}

impl Fonts {
    pub fn new(dpi: u32) -> Fonts {
        Fonts {
            normal: make_font(dpi, 9, 400),
            bold: make_font(dpi, 9, 600),
            title: make_font(dpi, 16, 600),
            section: make_font(dpi, 12, 600),
            icon: font_face(icon_face(), dpi, 9, 400),
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.normal);
            DeleteObject(self.bold);
            DeleteObject(self.title);
            DeleteObject(self.section);
            DeleteObject(self.icon);
        }
    }
}

pub fn register_class(name: &str, proc: WNDPROC, icon: HICON) {
    let class = wide(name);
    let wc = WNDCLASSEXW {
        cbSize: core::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance(),
        hIcon: icon,
        hCursor: unsafe { LoadCursorW(0, IDC_ARROW as *const u16) },
        hbrBackground: COLOR_BTNFACE + 1,
        lpszMenuName: null(),
        lpszClassName: class.as_ptr(),
        hIconSm: icon,
    };
    unsafe { RegisterClassExW(&wc) };
}

pub fn child(parent: HWND, class: &str, text: &str, style: u32, ex: u32, id: i32, font: HFONT) -> HWND {
    unsafe {
        let h = CreateWindowExW(ex, wide(class).as_ptr(), wide(text).as_ptr(), WS_CHILD | WS_VISIBLE | style,
            0, 0, 10, 10, parent, id as isize, hinstance(), null());
        SendMessageW(h, WM_SETFONT, font as usize, 1);
        h
    }
}

pub fn label(parent: HWND, text: &str, font: HFONT) -> HWND {
    child(parent, "STATIC", text, SS_NOPREFIX | SS_ENDELLIPSIS, 0, 0, font)
}

/// A label on a given surface, in primary or secondary (greyer) text.
pub fn label_on(parent: HWND, text: &str, font: HFONT, surface: crate::theme::Surface, secondary: bool) -> HWND {
    let h = label(parent, text, font);
    crate::theme::set_surface(h, surface, secondary);
    h
}

pub fn place(h: HWND, x: i32, y: i32, w: i32, height: i32) {
    unsafe { SetWindowPos(h, 0, x, y, w.max(0), height.max(0), SWP_NOZORDER | SWP_NOACTIVATE) };
}

pub fn set_text(h: HWND, s: &str) {
    unsafe { SetWindowTextW(h, wide(s).as_ptr()) };
}

pub fn get_text(h: HWND) -> String {
    unsafe {
        let n = GetWindowTextLengthW(h);
        let mut buf = vec![0u16; n as usize + 1];
        GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
        from_wide(&buf)
    }
}

pub fn enable(h: HWND, on: bool) {
    unsafe { EnableWindow(h, on as BOOL) };
}

// ------------------------------------------------------------------------------------------ list/combo box

pub fn lb_set(h: HWND, items: &[String], selected: Option<usize>) {
    unsafe {
        SendMessageW(h, LB_RESETCONTENT, 0, 0);
        for s in items {
            SendMessageW(h, LB_ADDSTRING, 0, wide(s).as_ptr() as LPARAM);
        }
        SendMessageW(h, LB_SETCURSEL, selected.map(|i| i as usize).unwrap_or(usize::MAX), 0);
    }
}

pub fn lb_replace(h: HWND, i: usize, text: &str) {
    unsafe {
        SendMessageW(h, LB_DELETESTRING, i, 0);
        SendMessageW(h, LB_INSERTSTRING, i, wide(text).as_ptr() as LPARAM);
        SendMessageW(h, LB_SETCURSEL, i, 0);
    }
}

pub fn lb_selected(h: HWND) -> Option<usize> {
    let r = unsafe { SendMessageW(h, LB_GETCURSEL, 0, 0) };
    (r >= 0).then_some(r as usize)
}

pub fn cb_set(h: HWND, items: &[String], selected: usize) {
    unsafe {
        SendMessageW(h, CB_RESETCONTENT, 0, 0);
        for s in items {
            SendMessageW(h, CB_ADDSTRING, 0, wide(s).as_ptr() as LPARAM);
        }
        SendMessageW(h, CB_SETCURSEL, selected, 0);
    }
}

pub fn cb_selected(h: HWND) -> Option<usize> {
    let r = unsafe { SendMessageW(h, CB_GETCURSEL, 0, 0) };
    (r >= 0).then_some(r as usize)
}

// ------------------------------------------------------------------------------------------ list view

pub fn listview(parent: HWND, id: i32, style: u32, ex_style: u32, font: HFONT) -> HWND {
    // No border: list views sit on a card or inside a drawn frame.
    let h = child(parent, "SysListView32", "", LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS | LVS_NOSORTHEADER | WS_TABSTOP | style,
        0, id, font);
    // Theme first: checkbox images are made from the theme when LVS_EX_CHECKBOXES is set.
    crate::theme::apply_control(h, crate::theme::ControlKind::ListView);
    unsafe {
        let ex = LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER | ex_style;
        SendMessageW(h, LVM_SETEXTENDEDLISTVIEWSTYLE, ex as usize, ex as LPARAM);
    }
    h
}

/// Widens a list view's last (or only) column to the edge of the list, less any scroll bar, so
/// no empty strip that looks like another column is left beside it.
pub fn lv_fill_last_column(h: HWND, last: usize) {
    const LVSCW_AUTOSIZE_USEHEADER: isize = -2;
    unsafe { SendMessageW(h, LVM_SETCOLUMNWIDTH, last, LVSCW_AUTOSIZE_USEHEADER) };
}

pub fn lv_add_column(h: HWND, index: i32, title: &str, width: i32) {
    let mut text = wide(title);
    let mut col: LVCOLUMNW = zeroed();
    col.mask = LVCF_TEXT | LVCF_WIDTH;
    col.cx = width;
    col.pszText = text.as_mut_ptr();
    unsafe { SendMessageW(h, LVM_INSERTCOLUMNW, index as usize, &col as *const _ as LPARAM) };
}

pub fn lv_clear(h: HWND) {
    unsafe { SendMessageW(h, LVM_DELETEALLITEMS, 0, 0) };
}

pub fn lv_count(h: HWND) -> i32 {
    unsafe { SendMessageW(h, LVM_GETITEMCOUNT, 0, 0) as i32 }
}

pub fn lv_set_text(h: HWND, row: i32, col: i32, text: &str) {
    let mut t = wide(text);
    let mut item: LVITEMW = zeroed();
    item.iSubItem = col;
    item.pszText = t.as_mut_ptr();
    unsafe { SendMessageW(h, LVM_SETITEMTEXTW, row as usize, &item as *const _ as LPARAM) };
}

/// Appends a row; `param` is stored with it.
pub fn lv_add(h: HWND, cells: &[String], param: isize) -> i32 {
    let mut first = wide(cells.first().map(String::as_str).unwrap_or(""));
    let mut item: LVITEMW = zeroed();
    item.mask = LVIF_TEXT | LVIF_PARAM;
    item.iItem = lv_count(h);
    item.pszText = first.as_mut_ptr();
    item.lParam = param;
    let row = unsafe { SendMessageW(h, LVM_INSERTITEMW, 0, &item as *const _ as LPARAM) as i32 };
    for (col, text) in cells.iter().enumerate().skip(1) {
        lv_set_text(h, row, col as i32, text);
    }
    row
}

pub fn lv_selected(h: HWND) -> Option<i32> {
    let r = unsafe { SendMessageW(h, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED) };
    (r >= 0).then_some(r as i32)
}

pub fn lv_select(h: HWND, row: i32) {
    let mut item: LVITEMW = zeroed();
    item.stateMask = LVIS_SELECTED | LVIS_FOCUSED;
    item.state = LVIS_SELECTED | LVIS_FOCUSED;
    unsafe {
        SendMessageW(h, LVM_SETITEMSTATE, row as usize, &item as *const _ as LPARAM);
        SendMessageW(h, LVM_ENSUREVISIBLE, row as usize, 0);
    }
}

pub fn lv_param(h: HWND, row: i32) -> isize {
    let mut item: LVITEMW = zeroed();
    item.mask = LVIF_PARAM;
    item.iItem = row;
    unsafe { SendMessageW(h, LVM_GETITEMW, 0, &mut item as *mut _ as LPARAM) };
    item.lParam
}

pub fn lv_checked(h: HWND, row: i32) -> bool {
    let state = unsafe { SendMessageW(h, LVM_GETITEMSTATE, row as usize, LVIS_STATEIMAGEMASK as LPARAM) } as u32;
    (state >> 12) == 2
}

pub fn lv_set_checked(h: HWND, row: i32, on: bool) {
    let mut item: LVITEMW = zeroed();
    item.stateMask = LVIS_STATEIMAGEMASK;
    item.state = (if on { 2 } else { 1 }) << 12;
    unsafe { SendMessageW(h, LVM_SETITEMSTATE, row as usize, &item as *const _ as LPARAM) };
}

// ------------------------------------------------------------------------------------------ dialogs

#[derive(Clone, Copy)]
pub enum Icon {
    Info,
    Warning,
}

/// A themed message with an OK button.
pub fn alert(owner: HWND, icon: Icon, main: &str, content: &str) {
    let icon = match icon {
        Icon::Info => crate::dialog::Icon::Info,
        Icon::Warning => crate::dialog::Icon::Warning,
    };
    crate::dialog::show(owner, icon, main, content, &["OK"]);
}

/// A themed confirmation with a warning icon: ction (e.g. "Delete") or Cancel. True for ction.
pub fn confirm(owner: HWND, main: &str, content: &str, action: &str) -> bool {
    crate::dialog::show(owner, crate::dialog::Icon::Warning, main, content, &[action, "Cancel"]) == 0
}

/// A Segoe Fluent Icons font at points.
pub fn icon_font(dpi: u32, points: i32) -> HFONT {
    font_face(icon_face(), dpi, points, 400)
}

/// Runs a modal message loop for `dialog` (owner disabled) until `done()` is true.
/// Keyboard messages for which `raw_key(msg)` is true skip dialog navigation and character
/// translation and go straight to their window (used by the hotkey capture box).
pub fn modal_loop(owner: HWND, dialog: HWND, done: &dyn Fn() -> bool, raw_key: &dyn Fn(&MSG) -> bool) {
    unsafe {
        if owner != 0 {
            EnableWindow(owner, 0);
        }
        ShowWindow(dialog, SW_SHOW);
        let mut msg: MSG = zeroed();
        while !done() {
            let r = GetMessageW(&mut msg, 0, 0, 0);
            if r <= 0 {
                PostQuitMessage(msg.wParam as i32);
                break;
            }
            if (WM_KEYFIRST..=WM_KEYLAST).contains(&msg.message) && raw_key(&msg) {
                DispatchMessageW(&msg);
                continue;
            }
            if IsDialogMessageW(dialog, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if owner != 0 {
            EnableWindow(owner, 1);
            SetActiveWindow(owner);
        }
    }
}

// ------------------------------------------------------------------------------------------ icon

/// The app icon: resource 1, written into the exe by build.rs from `assets/InputSwitch.ico` (the C#
/// app's artwork). Loaded at the small-icon (tray) size; Windows picks the best frame.
pub fn app_icon() -> HICON {
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    // IMAGE_ICON = 1; resource id 1 as MAKEINTRESOURCE.
    unsafe { LoadImageW(hinstance(), 1 as *const u16, 1, size, size, 0) }
}