//! Drop-down fields, like WinUI's ComboBox: a field showing the current choice (a widget, see
//! [`create`]) and, when it's clicked, a list of the choices just under it ([`choose`]). The
//! current choice is marked with an accent bar; the one under the mouse is highlighted.

use super::draw::{self, rect, RectExt};
use super::theme::{self, Surface};
use super::{widgets, Fonts, Slot};
use crate::ffi::*;

const CLASS: &str = "ui.DropDownList";

// At 96 DPI, measured from the C# app's (WPF Fluent) drop-down.
const GAP: i32 = 2;
const PAD: i32 = 5;
const ITEM_H: i32 = 40;
const ITEM_INSET: i32 = 4;
const TEXT_X: i32 = 13;

/// A drop-down field showing `text` (the current choice). Its parent shows the choices with
/// [`choose`] when it's clicked.
pub fn create(parent: HWND, text: &str, id: i32, fonts: &Fonts, surface: Surface) -> HWND {
    widgets::drop_down(parent, text, id, fonts, surface)
}

struct List {
    hwnd: HWND,
    choices: Vec<String>,
    current: Option<usize>,
    /// The item under the mouse, or picked with the arrow keys.
    hot: Option<usize>,
    fonts: Fonts,
    dpi: u32,
    /// Set when the list closes: Some(index) picked, None dismissed.
    result: Option<Option<usize>>,
}

thread_local! {
    static STATE: Slot<List> = const { Slot::new() };
}

fn with<R>(f: impl FnOnce(&mut List) -> R) -> Option<R> {
    STATE.with(|s| s.with(f))
}

/// Shows `choices` in a list under drop-down `field`, the current one marked. Returns the one
/// picked, or None if the list was dismissed (Esc, or a click elsewhere).
pub fn choose(field: HWND, choices: &[String], current: Option<usize>) -> Option<usize> {
    if choices.is_empty() || STATE.with(|s| s.occupied()) {
        return None;
    }
    super::register_class(CLASS, wndproc, false, CS_DROPSHADOW);
    let dpi = unsafe { GetDpiForWindow(field) }.max(96);
    let s = |v| super::scale(v, dpi);
    let mut r = RECT::default();
    unsafe { GetWindowRect(field, &mut r) };
    let height = s(2 * PAD) + choices.len() as i32 * s(ITEM_H);
    // Below the field, or above it if it would run off the bottom of the screen.
    let mut y = r.bottom + s(GAP);
    if y + height > work_area(field).bottom {
        y = r.top - s(GAP) - height;
    }
    let owner = unsafe { GetAncestor(field, GA_ROOT) };
    let hwnd = unsafe {
        CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_TOPMOST, wide(CLASS).as_ptr(), core::ptr::null(), WS_POPUP,
            r.left, y, r.width(), height, owner, 0, hinstance(), core::ptr::null())
    };
    unsafe {
        let round = DWMWCP_ROUND;
        DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &round as *const u32 as *const _, 4);
    }
    STATE.with(|st| st.set(List { hwnd, choices: choices.to_vec(), current, hot: current, fonts: Fonts::new(dpi), dpi, result: None }));

    widgets::set_checked(field, true); // the chevron points up while the list is open
    super::modal_loop(owner, hwnd, &|| STATE.with(|s| s.with(|l| l.result.is_some()).unwrap_or(true)), &|m| m.hwnd == hwnd);
    widgets::set_checked(field, false);
    let picked = STATE.with(|s| s.take()).and_then(|l| l.result.flatten());
    unsafe { DestroyWindow(hwnd) };
    picked
}

fn work_area(h: HWND) -> RECT {
    let mut info: MONITORINFOEXW = zeroed();
    info.cbSize = core::mem::size_of::<MONITORINFOEXW>() as u32;
    unsafe { GetMonitorInfoW(MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST), &mut info) };
    info.rcWork
}

impl List {
    fn item(&self, i: usize) -> RECT {
        let s = |v| super::scale(v, self.dpi);
        let width = draw::client_rect(self.hwnd).width();
        rect(0, s(PAD) + i as i32 * s(ITEM_H), width, s(ITEM_H)).inset(s(ITEM_INSET), s(2))
    }

    fn item_at(&self, x: i32, y: i32) -> Option<usize> {
        (0..self.choices.len()).find(|&i| {
            let r = self.item(i);
            x >= r.left && x < r.right && y >= r.top - super::scale(2, self.dpi) && y < r.bottom + super::scale(2, self.dpi)
        })
    }

    fn paint(&self, hdc: HDC, rc: RECT) {
        let p = theme::palette();
        let s = |v| super::scale(v, self.dpi);
        draw::fill(hdc, rc, p.flyout);
        for (i, text) in self.choices.iter().enumerate() {
            let r = self.item(i);
            if Some(i) == self.current {
                draw::round_rect(hdc, r, s(4), p.selected, None);
                let mid = (r.top + r.bottom) / 2;
                draw::round_rect(hdc, rect(r.left + s(2), mid - s(8), s(3), s(16)), s(2), p.accent, None);
            } else if Some(i) == self.hot {
                draw::round_rect(hdc, r, s(4), p.control_hot, None);
            }
            let label = RECT { left: r.left + s(TEXT_X), right: r.right - s(8), ..r };
            draw::text(hdc, text, label, self.fonts.normal, p.text, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
        }
    }

    fn set_hot(&mut self, hot: Option<usize>) {
        if hot != self.hot {
            self.hot = hot;
            unsafe { InvalidateRect(self.hwnd, core::ptr::null(), 0) };
        }
    }

    fn key(&mut self, vk: u16) {
        let last = self.choices.len() - 1;
        match vk {
            VK_UP => self.set_hot(Some(self.hot.map_or(last, |h| h.saturating_sub(1)))),
            VK_DOWN => self.set_hot(Some(self.hot.map_or(0, |h| (h + 1).min(last)))),
            VK_RETURN => self.result = Some(self.hot),
            VK_ESCAPE => self.result = Some(None),
            _ => {}
        }
    }
}

/// The mouse position in a mouse message, relative to the list.
fn point(l: LPARAM) -> (i32, i32) {
    ((l & 0xFFFF) as i16 as i32, ((l >> 16) & 0xFFFF) as i16 as i32)
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            draw::paint_buffered(h, |hdc, rc| {
                if with(|list| list.paint(hdc, rc)).is_none() {
                    draw::fill(hdc, rc, theme::palette().flyout);
                }
            });
            return 0;
        }
        WM_SHOWWINDOW if w != 0 => {
            // Mouse capture shows clicks outside the list too, so they can close it.
            SetCapture(h);
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(l);
            with(|list| list.set_hot(list.item_at(x, y)));
            return 0;
        }
        WM_LBUTTONDOWN => {
            let (x, y) = point(l);
            with(|list| {
                if list.item_at(x, y).is_none() {
                    list.result = Some(None); // a click outside the items
                }
            });
            return 0;
        }
        WM_LBUTTONUP => {
            let (x, y) = point(l);
            with(|list| {
                if let Some(i) = list.item_at(x, y) {
                    list.result = Some(Some(i));
                }
            });
            return 0;
        }
        WM_KEYDOWN => {
            with(|list| list.key(w as u16));
            return 0;
        }
        WM_CAPTURECHANGED | WM_ACTIVATEAPP => {
            with(|list| if list.result.is_none() { list.result = Some(None) }); // focus went elsewhere
            return 0;
        }
        WM_DESTROY => {
            ReleaseCapture();
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}
