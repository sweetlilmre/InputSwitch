//! Shared Win32 UI helpers: controls, fonts, list views, task dialogs, modal loops, the app icon.

use crate::ffi::*;
use core::ptr::null;

pub const TITLE: &str = "InputSwitch";

pub fn scale(v: i32, dpi: u32) -> i32 {
    (v as i64 * dpi as i64 / 96) as i32
}

pub fn make_font(dpi: u32, points: i32, weight: i32) -> HFONT {
    unsafe {
        // DEFAULT_CHARSET = 1, CLEARTYPE_QUALITY = 5
        CreateFontW(-(points * dpi as i32 / 72), 0, 0, 0, weight, 0, 0, 0, 1, 0, 0, 5, 0, wide("Segoe UI").as_ptr())
    }
}

pub struct Fonts {
    pub normal: HFONT,
    pub bold: HFONT,
    pub title: HFONT,
}

impl Fonts {
    pub fn new(dpi: u32) -> Fonts {
        Fonts { normal: make_font(dpi, 9, 400), bold: make_font(dpi, 9, 600), title: make_font(dpi, 14, 600) }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.normal);
            DeleteObject(self.bold);
            DeleteObject(self.title);
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

pub fn button(parent: HWND, text: &str, id: i32, font: HFONT) -> HWND {
    child(parent, "BUTTON", text, BS_PUSHBUTTON | WS_TABSTOP, 0, id, font)
}

pub fn checkbox(parent: HWND, text: &str, id: i32, font: HFONT) -> HWND {
    child(parent, "BUTTON", text, BS_AUTOCHECKBOX | WS_TABSTOP, 0, id, font)
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

pub fn checked(h: HWND) -> bool {
    unsafe { SendMessageW(h, BM_GETCHECK, 0, 0) == 1 }
}

pub fn set_checked(h: HWND, on: bool) {
    unsafe { SendMessageW(h, BM_SETCHECK, on as usize, 0) };
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
    let h = child(parent, "SysListView32", "", LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS | LVS_NOSORTHEADER | WS_TABSTOP | style,
        WS_EX_CLIENTEDGE, id, font);
    unsafe {
        let ex = LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER | ex_style;
        SendMessageW(h, LVM_SETEXTENDEDLISTVIEWSTYLE, ex as usize, ex as LPARAM);
        SetWindowTheme(h, wide("Explorer").as_ptr(), null());
    }
    h
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

fn td(owner: HWND, icon: *const u16, main: &str, content: &str, buttons: u32) -> i32 {
    let mut pressed = 0;
    unsafe {
        TaskDialog(owner, hinstance(), wide(TITLE).as_ptr(), wide(main).as_ptr(), wide(content).as_ptr(), buttons, icon, &mut pressed);
    }
    pressed
}

pub fn alert(owner: HWND, icon: Icon, main: &str, content: &str) {
    let i = match icon {
        Icon::Info => TD_INFORMATION_ICON,
        Icon::Warning => TD_WARNING_ICON,
    };
    td(owner, i, main, content, TDCBF_OK_BUTTON);
}

/// Yes/No question with a warning icon. True for Yes.
pub fn confirm(owner: HWND, main: &str, content: &str) -> bool {
    td(owner, TD_WARNING_ICON, main, content, TDCBF_YES_BUTTON | TDCBF_NO_BUTTON) == IDYES
}

/// Yes / No / Cancel. Returns IDYES, IDNO or IDCANCEL.
pub fn ask_yes_no_cancel(owner: HWND, main: &str, content: &str) -> i32 {
    td(owner, null(), main, content, TDCBF_YES_BUTTON | TDCBF_NO_BUTTON | TDCBF_CANCEL_BUTTON)
}

/// Runs a modal message loop for `dialog` (owner disabled) until `done()` is true.
pub fn modal_loop(owner: HWND, dialog: HWND, done: &dyn Fn() -> bool) {
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