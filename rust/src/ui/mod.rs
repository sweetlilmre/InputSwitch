//! A small Win32 UI toolkit with a Fluent look, independent of what InputSwitch does with it:
//! controls and window classes ([this module](self)), fonts, the theme, drawing, list helpers,
//! owner-drawn widgets, themed message boxes and a scrolling card panel.

pub mod card_panel;
pub mod dialog;
pub mod draw;
pub mod drop_down;
pub mod fonts;
pub mod lists;
pub mod mailbox;
pub mod text_box;
pub mod theme;
pub mod widgets;

pub use fonts::Fonts;
pub use mailbox::{Mailbox, WM_MAIL};

use crate::ffi::*;
use core::ptr::null;
use std::cell::RefCell;

/// The app's name, for title bars. Set once at start-up with [`set_app_name`].
static APP_NAME: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

pub fn set_app_name(name: &'static str) {
    let _ = APP_NAME.set(name);
}

pub fn app_name() -> &'static str {
    APP_NAME.get().copied().unwrap_or("")
}

/// A length at 96 DPI scaled to `dpi`.
pub fn scale(v: i32, dpi: u32) -> i32 {
    (v as i64 * dpi as i64 / 96) as i32
}

/// A window's DPI (96 if Windows doesn't say).
pub fn dpi_of(h: HWND) -> u32 {
    unsafe { GetDpiForWindow(h) }.max(96)
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

/// Registers window class `name` the first time it's asked for (later calls do nothing), with
/// the app icon if `icon`.
pub fn register_class(name: &'static str, proc: WNDPROC, icon: bool, style: u32) {
    thread_local! {
        static REGISTERED: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    }
    if REGISTERED.with(|r| r.borrow().contains(&name)) {
        return;
    }
    REGISTERED.with(|r| r.borrow_mut().push(name));
    let class = wide(name);
    let wc = WNDCLASSEXW {
        cbSize: core::mem::size_of::<WNDCLASSEXW>() as u32,
        style,
        lpfnWndProc: Some(proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance(),
        hIcon: if icon { app_icon(IconSize::Large) } else { 0 },
        hCursor: unsafe { LoadCursorW(0, IDC_ARROW as *const u16) },
        hbrBackground: COLOR_BTNFACE + 1,
        lpszMenuName: null(),
        lpszClassName: class.as_ptr(),
        hIconSm: if icon { app_icon(IconSize::Small) } else { 0 },
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

#[derive(Clone, Copy)]
pub enum IconSize {
    /// Title bars and the notification area.
    Small,
    /// Alt-Tab and the taskbar.
    Large,
}

/// The app icon (resource 1 in the exe, written by build.rs), at a standard size. Shared: don't
/// destroy it.
pub fn app_icon(size: IconSize) -> HICON {
    const IMAGE_ICON: u32 = 1;
    const LR_SHARED: u32 = 0x8000;
    let metric = match size { IconSize::Small => SM_CXSMICON, IconSize::Large => SM_CXICON };
    let px = unsafe { GetSystemMetrics(metric) };
    let resource_id_1 = core::ptr::without_provenance::<u16>(1); // MAKEINTRESOURCE(1)
    unsafe { LoadImageW(hinstance(), resource_id_1, IMAGE_ICON, px, px, LR_SHARED) }
}

// ------------------------------------------------------------------------------------------ keyboard

thread_local! {
    /// The control that last had focus in each window.
    static FOCUS: RefCell<std::collections::HashMap<HWND, HWND>> = RefCell::new(std::collections::HashMap::new());
}

/// Call on WM_ACTIVATE: remembers the focused control when `window` is deactivated and puts it
/// back when it's activated again (as a dialog does), else focuses `default` (or the window).
pub fn keep_focus(window: HWND, w: WPARAM, default: HWND) {
    unsafe {
        if loword(w) == WA_INACTIVE {
            let focus = GetFocus();
            if focus != 0 && IsChild(window, focus) != 0 {
                FOCUS.with(|f| f.borrow_mut().insert(window, focus));
            }
            return;
        }
        let remembered = FOCUS.with(|f| f.borrow().get(&window).copied()).filter(|&c| IsWindow(c) != 0 && IsChild(window, c) != 0);
        let target = remembered.unwrap_or(if default != 0 { default } else { window });
        SetFocus(target);
    }
}

/// What Enter does in a window: presses the focused button, if a button has focus. Returns
/// true if it did.
pub fn click_focused_button(window: HWND) -> bool {
    unsafe {
        let focus = GetFocus();
        if focus == 0 || IsChild(window, focus) == 0 || class_name(focus) != "Button" {
            return false;
        }
        SendMessageW(focus, BM_CLICK, 0, 0);
        true
    }
}

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 64];
    unsafe { GetClassNameW(h, buf.as_mut_ptr(), buf.len() as i32) };
    from_wide(&buf)
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
