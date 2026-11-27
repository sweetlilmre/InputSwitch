//! The Hotkeys list, drawn like the C# app's: one card per hotkey with its keys as key caps, the
//! action (and monitor) beside them, and Test, Edit and Delete buttons. The buttons are real
//! controls; everything else is painted. The panel scrolls when the cards don't fit.
//!
//! The parent gets the buttons' BN_CLICKED messages unchanged, from [`TEST_ID`], [`EDIT_ID`] and
//! [`REMOVE_ID`]` + row`.

use crate::ffi::*;
use crate::monitor_list::{colours, text_width};
use crate::theme::{self, Surface};
use crate::ui::{self, Fonts};
use crate::widgets;
use std::cell::RefCell;

const CLASS: &str = "InputSwitch.HotkeyList";
pub const TEST_ID: i32 = 3000;
pub const EDIT_ID: i32 = 3400;
pub const REMOVE_ID: i32 = 3800;
pub const MAX_ROWS: i32 = 400;

// Layout at 96 DPI, from the C# template.
const ROW_GAP: i32 = 6;
const PAD_X: i32 = 14;
const PAD_Y: i32 = 10;
const KEYS_W: i32 = 220;
const CAP_H: i32 = 25;
const CAP_MIN_W: i32 = 28;
const CAP_GAP: i32 = 4;
const BUTTON_W: i32 = 34;
const BUTTON_H: i32 = 32;
const BUTTON_GAP: i32 = 4;
const CONTENT_H: i32 = 36;

const EMPTY: &str = "No hotkeys yet. Add one to switch a monitor's input from the keyboard.";

/// What one card shows.
#[derive(Clone, PartialEq)]
pub struct Row {
    pub keys: Vec<String>,
    pub action: String,
    pub monitor: String,
}

struct Card {
    row: Row,
    buttons: [HWND; 3],
    rect: RECT,
}

struct Panel {
    hwnd: HWND,
    cards: Vec<Card>,
    fonts: Fonts,
    caps: HFONT,
    dpi: u32,
    scroll: i32,
    content_h: i32,
}

impl Drop for Panel {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.caps) };
    }
}

thread_local! {
    static STATE: RefCell<Option<Panel>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Panel) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

pub fn create(parent: HWND, id: i32, dpi: u32) -> HWND {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| ui::register_class(CLASS, wndproc, 0));
    let h = unsafe {
        CreateWindowExW(WS_EX_CONTROLPARENT, wide(CLASS).as_ptr(), wide("Hotkeys").as_ptr(),
            // Not WS_TABSTOP itself: the buttons inside are.
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_VSCROLL, 0, 0, 10, 10, parent, id as isize, hinstance(), core::ptr::null())
    };
    theme::apply_control(h, theme::ControlKind::ListBox); // dark scroll bar
    STATE.with(|s| {
        *s.borrow_mut() = Some(Panel { hwnd: h, cards: Vec::new(), fonts: Fonts::new(dpi), caps: ui::make_font(dpi, 12, 600), dpi, scroll: 0, content_h: 0 })
    });
    h
}

/// Shows these rows; the buttons are made again only when the number of rows changes.
pub fn set_rows(rows: Vec<Row>) {
    with(|p| {
        if p.cards.len() != rows.len() {
            for c in p.cards.drain(..) {
                for b in c.buttons {
                    unsafe { DestroyWindow(b) };
                }
            }
            for (i, r) in rows.into_iter().enumerate().take(MAX_ROWS as usize) {
                let i = i as i32;
                let make = |id: i32, icon: char| widgets::button(p.hwnd, "", id + i, Some(icon), &p.fonts, Surface::Row);
                let buttons = [make(TEST_ID, widgets::ICON_PLAY), make(EDIT_ID, widgets::ICON_EDIT), make(REMOVE_ID, widgets::ICON_DELETE)];
                p.cards.push(Card { row: r, buttons, rect: RECT::default() });
            }
        } else {
            for (c, r) in p.cards.iter_mut().zip(rows) {
                c.row = r;
            }
        }
    });
    relayout();
}

/// Height the cards need, at 96 DPI (for the parent's layout).
pub fn content_height() -> i32 {
    with(|p| { let d = p.dpi.max(96) as i32; (p.content_h * 96 + d - 1) / d }).unwrap_or(0)
}

pub fn set_dpi(dpi: u32) {
    with(|p| {
        p.dpi = dpi;
        p.fonts = Fonts::new(dpi);
        unsafe { DeleteObject(p.caps) };
        p.caps = ui::make_font(dpi, 12, 600);
        for c in &p.cards {
            for b in c.buttons {
                widgets::set_fonts(b, &p.fonts);
            }
        }
    });
    relayout();
}

pub fn retheme(h: HWND) {
    theme::apply_control(h, theme::ControlKind::ListBox);
    theme::repaint(h);
}

fn client(h: HWND) -> RECT {
    let mut rc = RECT::default();
    unsafe { GetClientRect(h, &mut rc) };
    rc
}

fn relayout() {
    let Some(h) = with(|p| p.hwnd) else { return };
    for _ in 0..2 {
        let width_before = client(h).right;
        with(|p| p.layout());
        update_scrollbar(h);
        if client(h).right == width_before {
            break;
        }
    }
    unsafe { InvalidateRect(h, core::ptr::null(), 0) };
}

impl Panel {
    fn layout(&mut self) {
        let s = |v: i32| ui::scale(v, self.dpi);
        let width = client(self.hwnd).right;
        let row_h = s(PAD_Y * 2 + CONTENT_H);
        self.content_h = if self.cards.is_empty() {
            s(16 + 20) // the "No hotkeys yet" line
        } else {
            self.cards.len() as i32 * (row_h + s(ROW_GAP)) - s(ROW_GAP)
        };
        let view = client(self.hwnd).bottom;
        self.scroll = self.scroll.clamp(0, (self.content_h - view).max(0));
        let mut y = -self.scroll;
        for c in &mut self.cards {
            c.rect = RECT { left: 0, top: y, right: width, bottom: y + row_h };
            let by = y + (row_h - s(BUTTON_H)) / 2;
            for (k, b) in c.buttons.iter().enumerate() {
                let x = width - s(PAD_X) - (3 - k as i32) * s(BUTTON_W) - (2 - k as i32) * s(BUTTON_GAP);
                ui::place(*b, x, by, s(BUTTON_W), s(BUTTON_H));
            }
            y += row_h + s(ROW_GAP);
        }
    }

    fn paint(&self, hdc: HDC) {
        let p = theme::palette();
        let k = colours();
        let s = |v: i32| ui::scale(v, self.dpi);
        let rc = client(self.hwnd);
        theme::fill(hdc, rc, p.card);
        if self.cards.is_empty() {
            let r = RECT { left: 0, top: s(16) - self.scroll, right: rc.right, bottom: s(16 + 20) - self.scroll };
            theme::text(hdc, EMPTY, r, self.fonts.normal, p.text2, DT_SINGLELINE | DT_END_ELLIPSIS);
            return;
        }
        for c in &self.cards {
            theme::round_rect(hdc, c.rect, s(6), k.row, Some(k.row_border));
            let mid = (c.rect.top + c.rect.bottom) / 2;

            // Key caps: a slightly thicker bottom edge, like a key.
            let mut x = c.rect.left + s(PAD_X);
            let cap_top = mid - s(CAP_H) / 2;
            for key in &c.row.keys {
                let w = (text_width(key, self.caps) + s(16) + 2).max(s(CAP_MIN_W));
                let cap = RECT { left: x, top: cap_top, right: x + w, bottom: cap_top + s(CAP_H) };
                theme::round_rect(hdc, cap, s(4), k.chip_border, None);
                theme::round_rect(hdc, RECT { left: cap.left + 1, top: cap.top + 1, right: cap.right - 1, bottom: cap.bottom - s(2) }, s(3), k.name, None);
                theme::text(hdc, key, RECT { bottom: cap.bottom - 1, ..cap }, self.caps, p.text, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                x += w + s(CAP_GAP);
            }

            // The action, with the monitor underneath.
            let tx = (c.rect.left + s(PAD_X + KEYS_W)).max(x + s(12));
            let right = c.rect.right - s(PAD_X) - 3 * s(BUTTON_W) - 2 * s(BUTTON_GAP) - s(12);
            let top = mid - s(CONTENT_H) / 2;
            theme::text(hdc, &c.row.action, RECT { left: tx, top, right, bottom: top + s(20) }, self.fonts.normal, p.text, DT_SINGLELINE | DT_END_ELLIPSIS);
            theme::text(hdc, &c.row.monitor, RECT { left: tx, top: top + s(20), right, bottom: top + s(CONTENT_H) }, self.fonts.small, p.text2, DT_SINGLELINE | DT_END_ELLIPSIS);
        }
    }
}

fn update_scrollbar(h: HWND) {
    let Some((content, scroll)) = with(|p| (p.content_h, p.scroll)) else { return };
    let view = client(h).bottom;
    let si = SCROLLINFO {
        cbSize: core::mem::size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
        nMin: 0,
        nMax: (content - 1).max(0),
        nPage: view.max(0) as u32,
        nPos: scroll,
        nTrackPos: 0,
    };
    unsafe { SetScrollInfo(h, SB_VERT, &si, 1) };
}

fn scroll_to(h: HWND, f: impl FnOnce(&mut Panel)) {
    with(f);
    with(|p| p.layout());
    update_scrollbar(h);
    unsafe { InvalidateRect(h, core::ptr::null(), 0) };
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let hdc = BeginPaint(h, &mut ps);
            let rc = client(h);
            let mem = CreateCompatibleDC(hdc);
            let bmp = CreateCompatibleBitmap(hdc, rc.right.max(1), rc.bottom.max(1));
            let old = SelectObject(mem, bmp);
            if with(|p| p.paint(mem)).is_none() {
                theme::fill(mem, rc, theme::palette().card);
            }
            BitBlt(hdc, 0, 0, rc.right, rc.bottom, mem, 0, 0, SRCCOPY);
            SelectObject(mem, old);
            DeleteObject(bmp);
            DeleteDC(mem);
            EndPaint(h, &ps);
            return 0;
        }
        WM_SIZE => relayout(),
        WM_VSCROLL => {
            let line = ui::scale(40, GetDpiForWindow(h).max(96));
            let page = client(h).bottom;
            let (code, pos) = (loword(w) as i32, hiword(w) as i32);
            scroll_to(h, |p| match code {
                0 => p.scroll -= line,
                1 => p.scroll += line,
                2 => p.scroll -= page,
                3 => p.scroll += page,
                4 | 5 => p.scroll = pos,
                6 => p.scroll = 0,
                7 => p.scroll = i32::MAX / 2,
                _ => {}
            });
            return 0;
        }
        WM_MOUSEWHEEL => {
            let delta = (hiword(w) as i16) as i32;
            let step = ui::scale(48, GetDpiForWindow(h).max(96));
            scroll_to(h, |p| p.scroll -= delta * step / 120);
            return 0;
        }
        WM_COMMAND => return SendMessageW(GetParent(h), WM_COMMAND, w, l),
        WM_DRAWITEM => {
            if widgets::draw(&*(l as *const DRAWITEMSTRUCT)) {
                return 1;
            }
        }
        WM_NCDESTROY => {
            STATE.with(|s| s.borrow_mut().take());
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}
