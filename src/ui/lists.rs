//! Thin helpers for the standard list box control.

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
