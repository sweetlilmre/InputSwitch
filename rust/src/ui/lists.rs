//! Thin helpers for the standard list box, combo box and list view controls.

use crate::ffi::*;

// ------------------------------------------------------------------------------------------ list box

pub fn lb_set(h: HWND, items: &[String], selected: Option<usize>) {
    unsafe {
        SendMessageW(h, LB_RESETCONTENT, 0, 0);
        for s in items {
            SendMessageW(h, LB_ADDSTRING, 0, wide(s).as_ptr() as LPARAM);
        }
        SendMessageW(h, LB_SETCURSEL, selected.unwrap_or(usize::MAX), 0);
    }
}

/// Replaces item `i`'s text and selects it.
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

pub fn lb_text(h: HWND, i: usize) -> String {
    unsafe {
        let n = SendMessageW(h, LB_GETTEXTLEN, i, 0).max(0) as usize;
        let mut buf = vec![0u16; n + 1];
        SendMessageW(h, LB_GETTEXT, i, buf.as_mut_ptr() as LPARAM);
        from_wide(&buf)
    }
}

// ------------------------------------------------------------------------------------------ combo box

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

/// A single-selection report-view list without a border (it sits inside a painted frame).
pub fn listview(parent: HWND, id: i32, style: u32, ex_style: u32, font: HFONT) -> HWND {
    let h = super::child(parent, "SysListView32", "", LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS | LVS_NOSORTHEADER | WS_TABSTOP | style,
        0, id, font);
    // Theme first: checkbox images are made from the theme when LVS_EX_CHECKBOXES is set.
    super::theme::apply_control(h, super::theme::ControlKind::ListView);
    unsafe {
        let ex = LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER | ex_style;
        SendMessageW(h, LVM_SETEXTENDEDLISTVIEWSTYLE, ex as usize, ex as LPARAM);
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

/// Widens the last (or only) column to the edge of the list, less any scroll bar, so no empty
/// strip that looks like another column is left beside it.
pub fn lv_fill_last_column(h: HWND, last: usize) {
    unsafe { SendMessageW(h, LVM_SETCOLUMNWIDTH, last, LVSCW_AUTOSIZE_USEHEADER) };
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
    let mut first = wide(cells.first().map_or("", String::as_str));
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

pub fn lv_set_param(h: HWND, row: i32, param: isize) {
    let mut item: LVITEMW = zeroed();
    item.mask = LVIF_PARAM;
    item.iItem = row;
    item.lParam = param;
    unsafe { SendMessageW(h, LVM_SETITEMW, 0, &item as *const _ as LPARAM) };
}

// The check box is state image 2 (unchecked is 1), stored in bits 12-15 of the item state.
const STATE_IMAGE_SHIFT: u32 = 12;

pub fn lv_checked(h: HWND, row: i32) -> bool {
    let state = unsafe { SendMessageW(h, LVM_GETITEMSTATE, row as usize, LVIS_STATEIMAGEMASK as LPARAM) } as u32;
    (state >> STATE_IMAGE_SHIFT) == 2
}

pub fn lv_set_checked(h: HWND, row: i32, on: bool) {
    let mut item: LVITEMW = zeroed();
    item.stateMask = LVIS_STATEIMAGEMASK;
    item.state = (if on { 2 } else { 1 }) << STATE_IMAGE_SHIFT;
    unsafe { SendMessageW(h, LVM_SETITEMSTATE, row as usize, &item as *const _ as LPARAM) };
}
