//! The Monitors list, drawn like the C# app's: one card per monitor with an icon, an editable
//! name box, "model · S/N" details, the inputs as chips (the current one highlighted), a
//! Connected / Not connected pill and a delete button. The name boxes and delete buttons are real
//! controls (so Tab and screen readers reach them); everything else is painted. The panel scrolls
//! when the cards don't fit.
//!
//! The parent gets the child controls' WM_COMMAND messages unchanged: EN_CHANGE / EN_KILLFOCUS
//! from name box [`NAME_ID`]` + row`, and BN_CLICKED from delete button [`REMOVE_ID`]` + row`.

use crate::ffi::*;
use crate::theme::{self, Surface};
use crate::ui::{self, Fonts};
use crate::widgets;
use std::cell::RefCell;

const CLASS: &str = "InputSwitch.MonitorList";
pub const NAME_ID: i32 = 2000;
pub const REMOVE_ID: i32 = 2500;
pub const MAX_ROWS: i32 = 400;

// Layout at 96 DPI, from the C# template.
const ROW_GAP: i32 = 6;
const PAD_X: i32 = 14;
const PAD_Y: i32 = 12;
const ICON: i32 = 28;
const ICON_GAP: i32 = 16;
const NAME_H: i32 = 32;
const NAME_MIN_W: i32 = 220;
const NAME_MAX_W: i32 = 360;
const DETAILS_H: i32 = 16;
const CHIP_H: i32 = 22;
const CHIP_GAP: i32 = 6;
const PILL_H: i32 = 22;
const BUTTON_W: i32 = 34;

/// What one card shows.
#[derive(Clone, PartialEq)]
pub struct Row {
    pub id: String,
    pub name: String,
    pub details: String,
    pub connected: bool,
    /// Input short names; true for the current input.
    pub chips: Vec<(String, bool)>,
    /// Shown instead of chips when the inputs aren't known.
    pub hint: String,
}

struct Card {
    row: Row,
    name: HWND,
    remove: HWND,
    /// Laid-out positions, in client pixels (already scrolled).
    rect: RECT,
    frame: RECT,
    chips: Vec<RECT>,
    pill: RECT,
}

struct Panel {
    hwnd: HWND,
    cards: Vec<Card>,
    fonts: Fonts,
    glyphs: HFONT,
    dpi: u32,
    scroll: i32,
    content_h: i32,
}

impl Drop for Panel {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.glyphs) };
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
        CreateWindowExW(WS_EX_CONTROLPARENT, wide(CLASS).as_ptr(), wide("Monitors").as_ptr(),
            // Not WS_TABSTOP itself: a control parent that's also a tab stop sends dialog navigation round in circles.
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_VSCROLL, 0, 0, 10, 10, parent, id as isize, hinstance(), core::ptr::null())
    };
    theme::apply_control(h, theme::ControlKind::ListBox); // dark scroll bar
    STATE.with(|s| {
        *s.borrow_mut() = Some(Panel { hwnd: h, cards: Vec::new(), fonts: Fonts::new(dpi), glyphs: ui::icon_font(dpi, ICON), dpi, scroll: 0, content_h: 0 })
    });
    h
}

/// Shows these rows. Cards for the same monitors (in the same order) are updated in place, so a
/// name being typed keeps its caret and focus; the name box's text is only replaced when it isn't
/// focused.
pub fn set_rows(rows: Vec<Row>) {
    let rebuilt = with(|p| {
        let same = p.cards.len() == rows.len() && p.cards.iter().zip(&rows).all(|(c, r)| c.row.id.eq_ignore_ascii_case(&r.id));
        if same {
            let focus = unsafe { GetFocus() };
            for (c, r) in p.cards.iter_mut().zip(rows) {
                if c.name != focus && ui::get_text(c.name) != r.name {
                    ui::set_text(c.name, &r.name);
                }
                c.row = r;
            }
            false
        } else {
            for c in p.cards.drain(..) {
                unsafe {
                    DestroyWindow(c.name);
                    DestroyWindow(c.remove);
                }
            }
            for (i, r) in rows.into_iter().enumerate().take(MAX_ROWS as usize) {
                let i = i as i32;
                let name = ui::child(p.hwnd, "EDIT", &r.name, ES_AUTOHSCROLL | WS_TABSTOP, 0, NAME_ID + i, p.fonts.bold);
                theme::apply_control(name, theme::ControlKind::Edit);
                let remove = widgets::button(p.hwnd, "", REMOVE_ID + i, Some(widgets::ICON_DELETE), &p.fonts, Surface::Row);
                p.cards.push(Card { row: r, name, remove, rect: RECT::default(), frame: RECT::default(), chips: Vec::new(), pill: RECT::default() });
            }
            p.scroll = 0;
            true
        }
    });
    if rebuilt.is_some() {
        relayout();
    }
}

/// Height the cards need, at 96 DPI (for the parent's layout).
pub fn content_height() -> i32 {
    with(|p| { let d = p.dpi.max(96) as i32; (p.content_h * 96 + d - 1) / d }).unwrap_or(0)
}

pub fn set_dpi(dpi: u32) {
    with(|p| {
        p.dpi = dpi;
        p.fonts = Fonts::new(dpi);
        unsafe { DeleteObject(p.glyphs) };
        p.glyphs = ui::icon_font(dpi, ICON);
        for c in &p.cards {
            unsafe { SendMessageW(c.name, WM_SETFONT, p.fonts.bold as usize, 1) };
            widgets::set_fonts(c.remove, &p.fonts);
        }
    });
    relayout();
}

pub fn retheme(h: HWND) {
    theme::apply_control(h, theme::ControlKind::ListBox);
    with(|p| {
        for c in &p.cards {
            theme::apply_control(c.name, theme::ControlKind::Edit);
        }
    });
    theme::repaint(h);
}

/// The name box's current text for a row.
pub fn name_text(row: usize) -> Option<String> {
    with(|p| p.cards.get(row).map(|c| ui::get_text(c.name))).flatten()
}

pub fn set_name_text(row: usize, text: &str) {
    if let Some(h) = with(|p| p.cards.get(row).map(|c| c.name)).flatten() {
        ui::set_text(h, text);
    }
}

pub fn text_width(text: &str, font: HFONT) -> i32 {
    if text.is_empty() {
        return 0;
    }
    let w: Vec<u16> = text.encode_utf16().collect();
    let mut r = RECT { left: 0, top: 0, right: 10_000, bottom: 1000 };
    unsafe {
        let dc = GetDC(0);
        let old = SelectObject(dc, font);
        DrawTextW(dc, w.as_ptr(), w.len() as i32, &mut r, DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX);
        SelectObject(dc, old);
        ReleaseDC(0, dc);
    }
    r.right - r.left
}

/// The Fluent fills the C# template uses, which are translucent; here mixed onto the card colour.
pub struct Colours {
    pub row: u32,
    pub row_border: u32,
    pub name: u32,
    pub name_border: u32,
    pub name_underline: u32,
    pub chip: u32,
    pub chip_border: u32,
}

pub fn colours() -> Colours {
    let p = theme::palette();
    if p.dark {
        let white = theme::rgb(255, 255, 255);
        Colours {
            row: theme::color_of(Surface::Row),
            row_border: theme::mix(p.card, white, 0.07), // ControlStrokeColorDefault
            name: theme::mix(p.card, white, 0.09),
            name_border: theme::mix(p.card, white, 0.08),
            name_underline: theme::mix(p.card, white, 0.55),
            chip: theme::mix(p.card, white, 0.11),       // ControlFillColorSecondary
            chip_border: theme::mix(p.card, white, 0.13),
        }
    } else {
        let black = theme::rgb(0, 0, 0);
        Colours {
            row: theme::color_of(Surface::Row),
            row_border: theme::mix(p.card, black, 0.06),
            name: theme::rgb(255, 255, 255),
            name_border: theme::mix(p.card, black, 0.07),
            name_underline: theme::mix(p.card, black, 0.45),
            chip: theme::rgb(249, 249, 249),
            chip_border: theme::mix(p.card, black, 0.07),
        }
    }
}

fn pill_text(connected: bool) -> &'static str {
    if connected { "Connected" } else { "Not connected" }
}

/// Positions every card (and its controls) for the current width and scroll position, and
/// updates the scroll bar.
fn relayout() {
    let Some(h) = with(|p| p.hwnd) else { return };
    // Twice at most: showing or hiding the scroll bar changes the width.
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

fn client(h: HWND) -> RECT {
    let mut rc = RECT::default();
    unsafe { GetClientRect(h, &mut rc) };
    rc
}

impl Panel {
    fn layout(&mut self) {
        let dpi = self.dpi;
        let s = |v: i32| ui::scale(v, dpi);
        let width = client(self.hwnd).right;
        let small = self.fonts.small;
        let mut y = 0;
        for c in &mut self.cards {
            let left = 0;
            let right = width;
            // Right side: the pill, then the delete button.
            let pill_w = s(10 + 8 + 6) + text_width(pill_text(c.row.connected), small) + s(10);
            let button_x = right - s(PAD_X + BUTTON_W);
            let pill_x = button_x - s(10) - pill_w;
            let cx = left + s(PAD_X + ICON + ICON_GAP);
            let content_right = pill_x - s(12);
            let top = y + s(PAD_Y);

            let typed = text_width(&ui::get_text(c.name), self.fonts.bold) + s(24);
            let name_w = typed.clamp(s(NAME_MIN_W), s(NAME_MAX_W)).min((content_right - cx).max(s(80)));
            c.frame = RECT { left: cx, top, right: cx + name_w, bottom: top + s(NAME_H) };
            let mut line = c.frame.bottom + s(4) + s(DETAILS_H);

            // Chips wrap like a WrapPanel.
            c.chips.clear();
            if !c.row.chips.is_empty() {
                let (mut x, mut cy) = (cx, line + s(CHIP_GAP));
                for (text, _) in &c.row.chips {
                    let w = text_width(text, small) + s(20) + 2;
                    if x > cx && x + w > content_right {
                        x = cx;
                        cy += s(CHIP_H + CHIP_GAP);
                    }
                    c.chips.push(RECT { left: x, top: cy, right: x + w, bottom: cy + s(CHIP_H) });
                    x += w + s(CHIP_GAP);
                }
                line = cy + s(CHIP_H);
            } else if !c.row.hint.is_empty() {
                line += s(6) + s(DETAILS_H);
            }
            let bottom = line + s(PAD_Y);
            c.rect = RECT { left, top: y, right, bottom };
            c.pill = RECT { left: pill_x, top: top + (s(NAME_H) - s(PILL_H)) / 2, right: pill_x + pill_w, bottom: top + (s(NAME_H) - s(PILL_H)) / 2 + s(PILL_H) };
            y = bottom + s(ROW_GAP);
        }
        self.content_h = (y - s(ROW_GAP)).max(0);

        // Apply the scroll position: shift everything up and move the real controls.
        let view = client(self.hwnd).bottom;
        self.scroll = self.scroll.clamp(0, (self.content_h - view).max(0));
        let dy = self.scroll;
        let shift = |r: &mut RECT| { r.top -= dy; r.bottom -= dy; };
        for c in &mut self.cards {
            shift(&mut c.rect);
            shift(&mut c.frame);
            shift(&mut c.pill);
            c.chips.iter_mut().for_each(shift);
            let f = c.frame;
            ui::place(c.name, f.left + s(8), f.top + (f.bottom - f.top - s(20)) / 2, f.right - f.left - s(16), s(20));
            ui::place(c.remove, c.rect.right - s(PAD_X + BUTTON_W), c.frame.top, s(BUTTON_W), s(NAME_H));
        }
    }

    fn paint(&self, hdc: HDC) {
        let p = theme::palette();
        let s = |v: i32| ui::scale(v, self.dpi);
        theme::fill(hdc, client(self.hwnd), p.card);
        let focus = unsafe { GetFocus() };
        let k = colours();
        let row_fill = k.row;
        for c in &self.cards {
            let dim = |colour: u32| if c.row.connected { colour } else { theme::mix(colour, row_fill, 0.35) };
            theme::round_rect(hdc, c.rect, s(6), row_fill, Some(k.row_border));

            let icon = RECT { left: c.rect.left + s(PAD_X), top: c.frame.top, right: c.rect.left + s(PAD_X + ICON), bottom: c.frame.top + s(ICON) + s(4) };
            theme::text(hdc, "\u{E7F4}", icon, self.glyphs, dim(p.text), DT_SINGLELINE);

            // A Fluent text box: faint border, a brighter bottom line, accent and thicker when focused.
            theme::round_rect(hdc, c.frame, s(4), k.name, Some(k.name_border));
            let focused = c.name == focus;
            let line_h = if focused { s(2) } else { 1 };
            let f = c.frame;
            theme::fill(hdc, RECT { left: f.left + s(3), top: f.bottom - line_h, right: f.right - s(3), bottom: f.bottom }, if focused { p.accent } else { k.name_underline });

            let details = RECT { left: c.frame.left + s(2), top: c.frame.bottom + s(4), right: c.pill.left - s(12), bottom: c.frame.bottom + s(4 + DETAILS_H) };
            theme::text(hdc, &c.row.details, details, self.fonts.small, dim(p.text2), DT_SINGLELINE | DT_END_ELLIPSIS);

            for (r, (text, current)) in c.chips.iter().zip(&c.row.chips) {
                if *current {
                    theme::round_rect(hdc, *r, s(CHIP_H / 2), p.accent, None);
                    theme::text(hdc, text, *r, self.fonts.small, p.on_accent, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                } else {
                    theme::round_rect(hdc, *r, s(CHIP_H / 2), k.chip, Some(k.chip_border));
                    theme::text(hdc, text, *r, self.fonts.small, dim(p.text), DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                }
            }
            if c.chips.is_empty() && !c.row.hint.is_empty() {
                let r = RECT { left: c.frame.left + s(2), top: details.bottom + s(6), right: details.right, bottom: details.bottom + s(6 + DETAILS_H) };
                theme::text(hdc, &c.row.hint, r, self.fonts.small, p.text2, DT_SINGLELINE | DT_END_ELLIPSIS);
            }

            // The status pill: a dot and the text.
            let (pill_bg, dot) = if c.row.connected {
                (p.success, theme::success_text())
            } else {
                (p.neutral, p.text_disabled)
            };
            theme::round_rect(hdc, c.pill, s(PILL_H / 2), pill_bg, None);
            let dy = c.pill.top + (s(PILL_H) - s(8)) / 2;
            theme::round_rect(hdc, RECT { left: c.pill.left + s(10), top: dy, right: c.pill.left + s(18), bottom: dy + s(8) }, s(4), dot, None);
            let tr = RECT { left: c.pill.left + s(24), top: c.pill.top, right: c.pill.right, bottom: c.pill.bottom };
            theme::text(hdc, pill_text(c.row.connected), tr, self.fonts.small, p.text, DT_VCENTER | DT_SINGLELINE);
        }
    }

    fn scroll_by(&mut self, delta: i32) {
        self.scroll += delta;
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
            // Drawn off screen, then copied, so nothing flickers.
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
            let code = loword(w) as i32;
            let pos = hiword(w) as i32;
            scroll_to(h, |p| match code {
                0 /* SB_LINEUP */ => p.scroll_by(-line),
                1 /* SB_LINEDOWN */ => p.scroll_by(line),
                2 /* SB_PAGEUP */ => p.scroll_by(-page),
                3 /* SB_PAGEDOWN */ => p.scroll_by(page),
                4 | 5 /* SB_THUMBPOSITION, SB_THUMBTRACK */ => p.scroll = pos,
                6 /* SB_TOP */ => p.scroll = 0,
                7 /* SB_BOTTOM */ => p.scroll = i32::MAX / 2,
                _ => {}
            });
            return 0;
        }
        WM_MOUSEWHEEL => {
            let delta = (hiword(w) as i16) as i32;
            let step = ui::scale(48, GetDpiForWindow(h).max(96));
            scroll_to(h, |p| p.scroll_by(-delta * step / 120));
            return 0;
        }
        WM_COMMAND => {
            let code = hiword(w);
            if code == EN_CHANGE {
                with(|p| p.layout());
                InvalidateRect(h, core::ptr::null(), 0);
            }
            if code == EN_SETFOCUS || code == EN_KILLFOCUS {
                InvalidateRect(h, core::ptr::null(), 0); // the name box's accent border
                // Keep the focused card in view (e.g. when tabbing to it).
                if code == EN_SETFOCUS {
                    let ctl = l as HWND;
                    let view = client(h).bottom;
                    scroll_to(h, |p| {
                        if let Some(c) = p.cards.iter().find(|c| c.name == ctl) {
                            if c.rect.top < 0 {
                                p.scroll += c.rect.top;
                            } else if c.rect.bottom > view {
                                p.scroll += c.rect.bottom - view;
                            }
                        }
                    });
                }
            }
            return SendMessageW(GetParent(h), WM_COMMAND, w, l);
        }
        WM_DRAWITEM => {
            if widgets::draw(&*(l as *const DRAWITEMSTRUCT)) {
                return 1;
            }
        }
        WM_CTLCOLOREDIT => {
            let bg = colours().name;
            SetTextColor(w as HDC, theme::palette().text);
            SetBkColor(w as HDC, bg);
            return theme::brush(bg);
        }
        WM_CTLCOLORSTATIC => {
            if let Some(brush) = theme::ctl_color(m, w as HDC, l as HWND) {
                return brush;
            }
        }
        WM_NCDESTROY => {
            STATE.with(|s| s.borrow_mut().take());
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}
