//! A small Win32 UI toolkit with a Fluent look, independent of what InputSwitch does with it:
//! controls and window classes ([this module](self)), fonts, the theme, drawing, list helpers,
//! owner-drawn widgets, themed message boxes and a scrolling card panel.

pub mod card_panel;
pub mod dialog;
pub mod draw;
pub mod fonts;
pub mod lists;
pub mod theme;
pub mod widgets;

pub use fonts::Fonts;

use crate::ffi::*;
use core::ptr::null;
use std::cell::RefCell;

pub const TITLE: &str = "InputSwitch";

/// A length at 96 DPI scaled to `dpi`.
pub fn scale(v: i32, dpi: u32) -> i32 {
    (v as i64 * dpi as i64 / 96) as i32
}

// ------------------------------------------------------------------------------------------ window state

/// A window's state, kept in a thread-local and lent out for one message at a time.
///
/// Win32 calls back into a window procedure while the window is already handling a message
/// (when it changes a control, opens a dialog, and so on). Those nested messages find the slot
/// busy and get `None`: they skip the window's own handling, which is exactly "don't react to our
/// own updates". Anything that must not be lost that way goes through a queue instead.
pub struct Slot<T>(RefCell<Option<T>>);

impl<T> Slot<T> {
    pub const fn new() -> Self {
        Slot(RefCell::new(None))
    }

    pub fn set(&self, value: T) {
        *self.0.borrow_mut() = Some(value);
    }

    /// Removes the state (when the window is destroyed). None if busy or empty.
    pub fn take(&self) -> Option<T> {
        self.0.try_borrow_mut().ok().and_then(|mut s| s.take())
    }

    /// Lends the state out, unless it's empty or already lent.
    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        self.0.try_borrow_mut().ok().and_then(|mut s| s.as_mut().map(f))
    }

    /// True while there's state (the window exists), or it's lent out.
    pub fn occupied(&self) -> bool {
        self.0.try_borrow().map_or(true, |s| s.is_some())
    }
}

// ------------------------------------------------------------------------------------------ windows

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

/// Creates a fixed-size popup window (a dialog) whose client area is `width` × `height` at 96
/// DPI, centred on `owner`. With no owner it's centred on the screen, kept on top, and given a
/// taskbar button so it can't get lost. Shown later, by [`modal_loop`].
pub fn popup_window(class: &str, title: &str, owner: HWND, width: i32, height: i32, dpi: u32) -> HWND {
    let style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
    let mut ex = WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT;
    if owner == 0 {
        ex |= WS_EX_TOPMOST | WS_EX_APPWINDOW;
    }
    let mut r = RECT { left: 0, top: 0, right: scale(width, dpi), bottom: scale(height, dpi) };
    let mut area = RECT::default();
    unsafe {
        AdjustWindowRectExForDpi(&mut r, style, 0, ex, dpi);
        if owner != 0 {
            GetWindowRect(owner, &mut area);
        } else {
            area.right = GetSystemMetrics(SM_CXSCREEN);
            area.bottom = GetSystemMetrics(SM_CYSCREEN);
        }
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        let x = area.left + ((area.right - area.left) - w) / 2;
        let y = area.top + ((area.bottom - area.top) - h) / 2;
        let hwnd = CreateWindowExW(ex, wide(class).as_ptr(), wide(title).as_ptr(), style, x, y, w, h, owner, 0, hinstance(), null());
        theme::apply_window(hwnd);
        hwnd
    }
}

/// Shows `dialog` and runs a modal message loop (owner disabled) until `done()` is true.
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
            if GetMessageW(&mut msg, 0, 0, 0) <= 0 {
                PostQuitMessage(msg.wParam as i32); // leave the quit for the outer loop too
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
            // Before the dialog goes, so the owner doesn't drop behind other apps.
            EnableWindow(owner, 1);
            SetActiveWindow(owner);
        }
    }
}

/// The app icon: resource 1, written into the exe by build.rs from `assets/InputSwitch.ico` (the C#
/// app's artwork). Loaded at the small-icon (tray) size; Windows picks the best frame.
pub fn app_icon() -> HICON {
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    const IMAGE_ICON: u32 = 1;
    let resource_id_1 = core::ptr::without_provenance::<u16>(1); // MAKEINTRESOURCE(1)
    unsafe { LoadImageW(hinstance(), resource_id_1, IMAGE_ICON, size, size, 0) }
}

// ------------------------------------------------------------------------------------------ controls

pub fn child(parent: HWND, class: &str, text: &str, style: u32, ex: u32, id: i32, font: HFONT) -> HWND {
    unsafe {
        let h = CreateWindowExW(ex, wide(class).as_ptr(), wide(text).as_ptr(), WS_CHILD | WS_VISIBLE | style,
            0, 0, 10, 10, parent, id as isize, hinstance(), null());
        SendMessageW(h, WM_SETFONT, font as usize, 1);
        h
    }
}

/// A label on `surface`, in primary or secondary (greyer) text; it ends in "…" if cut off.
pub fn label(parent: HWND, text: &str, font: HFONT, surface: theme::Surface, secondary: bool) -> HWND {
    let h = child(parent, "STATIC", text, SS_NOPREFIX | SS_ENDELLIPSIS, 0, 0, font);
    theme::set_surface(h, surface, secondary);
    h
}

/// A borderless single-line edit box (it sits inside a painted frame).
pub fn edit(parent: HWND, id: i32, font: HFONT) -> HWND {
    let h = child(parent, "EDIT", "", ES_AUTOHSCROLL | WS_TABSTOP, 0, id, font);
    theme::apply_control(h, theme::ControlKind::Edit);
    h
}

pub fn place(h: HWND, x: i32, y: i32, w: i32, height: i32) {
    unsafe { SetWindowPos(h, 0, x, y, w.max(0), height.max(0), SWP_NOZORDER | SWP_NOACTIVATE) };
}

pub fn place_rect(h: HWND, r: RECT) {
    place(h, r.left, r.top, r.right - r.left, r.bottom - r.top);
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

pub fn set_font(h: HWND, font: HFONT) {
    unsafe { SendMessageW(h, WM_SETFONT, font as usize, 1) };
}

pub fn enable(h: HWND, on: bool) {
    unsafe { EnableWindow(h, on as BOOL) };
}
