//! Owner-drawn buttons, check boxes, toggle switches and drop-down fields in a WinUI-like style:
//! rounded, with hover and pressed states, an optional Segoe Fluent Icons glyph, and an accent
//! style for the main action. Check boxes are drawn too, so their labels stay readable in dark
//! mode (themed Win32 check boxes keep black text).
//!
//! Parents forward WM_DRAWITEM to [`draw`] and call [`clicked`] on BN_CLICKED (which flips a
//! check box or switch).

use super::draw::{self, rect, RectExt};
use super::theme::{self, Palette, Surface};
use super::Fonts;
use crate::ffi::*;
use std::cell::RefCell;
use std::collections::HashMap;

// Segoe Fluent Icons glyphs.
pub const ICON_ADD: char = '\u{E710}';
pub const ICON_CLEAR: char = '\u{E711}';
pub const ICON_DELETE: char = '\u{E74D}';
pub const ICON_DOWN: char = '\u{E70D}';
pub const ICON_EDIT: char = '\u{E70F}';
pub const ICON_INFO: char = '\u{E946}';
pub const ICON_KEYBOARD: char = '\u{E765}';
pub const ICON_MONITOR: char = '\u{E7F4}';
pub const ICON_PLAY: char = '\u{E768}';
pub const ICON_REFRESH: char = '\u{E72C}';
pub const ICON_SETTINGS: char = '\u{E713}';
pub const ICON_TICK: char = '\u{E73E}';
pub const ICON_UP: char = '\u{E70E}';
pub const ICON_WARNING: char = '\u{E7BA}';

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Standard,
    Accent,
    Check,
    Switch,
    /// The choice on the left, a chevron on the right (see [`super::drop_down`]). "Checked"
    /// means its list is open: the chevron points up.
    DropDown,
}

struct Widget {
    kind: Kind,
    icon: Option<char>,
    checked: bool,
    /// The mouse is over it.
    hot: bool,
    icon_font: HFONT,
}

thread_local! {
    static WIDGETS: RefCell<HashMap<HWND, Widget>> = RefCell::new(HashMap::new());
}

const SUBCLASS_ID: usize = 1;

fn create(parent: HWND, text: &str, id: i32, kind: Kind, icon: Option<char>, fonts: &Fonts, surface: Surface) -> HWND {
    // BS_NOTIFY: the parent hears when it gets focus (a card panel scrolls it into view).
    let h = super::child(parent, "BUTTON", text, BS_OWNERDRAW | BS_NOTIFY | WS_TABSTOP, 0, id, fonts.normal);
    theme::set_surface(h, surface, false);
    WIDGETS.with(|w| w.borrow_mut().insert(h, Widget { kind, icon, checked: false, hot: false, icon_font: fonts.icon }));
    unsafe { SetWindowSubclass(h, widget_proc, SUBCLASS_ID, 0) };
    h
}

pub fn button(parent: HWND, text: &str, id: i32, icon: Option<char>, fonts: &Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Standard, icon, fonts, surface)
}

/// The window's main action: accent coloured.
pub fn accent_button(parent: HWND, text: &str, id: i32, icon: Option<char>, fonts: &Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Accent, icon, fonts, surface)
}

pub fn checkbox(parent: HWND, text: &str, id: i32, fonts: &Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Check, None, fonts, surface)
}

/// An on/off toggle switch.
pub fn switch(parent: HWND, text: &str, id: i32, fonts: &Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::Switch, None, fonts, surface)
}

/// A drop-down field showing `text` (the current choice).
pub fn drop_down(parent: HWND, text: &str, id: i32, fonts: &Fonts, surface: Surface) -> HWND {
    create(parent, text, id, Kind::DropDown, None, fonts, surface)
}

/// The fonts changed (after a DPI change).
pub fn set_fonts(h: HWND, fonts: &Fonts) {
    WIDGETS.with(|w| {
        if let Some(widget) = w.borrow_mut().get_mut(&h) {
            widget.icon_font = fonts.icon;
        }
    });
}

pub fn checked(h: HWND) -> bool {
    WIDGETS.with(|w| w.borrow().get(&h).is_some_and(|x| x.checked))
}

pub fn set_checked(h: HWND, on: bool) {
    WIDGETS.with(|w| {
        if let Some(widget) = w.borrow_mut().get_mut(&h) {
            widget.checked = on;
        }
    });
    repaint(h);
}

/// Call on BN_CLICKED: a check box or switch flips.
pub fn clicked(h: HWND) {
    let toggles = WIDGETS.with(|w| w.borrow().get(&h).is_some_and(|x| matches!(x.kind, Kind::Check | Kind::Switch)));
    if toggles {
        set_checked(h, !checked(h));
    }
}

fn repaint(h: HWND) {
    unsafe { RedrawWindow(h, core::ptr::null(), 0, RDW_INVALIDATE) };
}

fn set_hot(h: HWND, hot: bool) -> bool {
    WIDGETS.with(|w| w.borrow_mut().get_mut(&h).map(|x| std::mem::replace(&mut x.hot, hot))).unwrap_or(hot)
}

unsafe extern "system" fn widget_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM, _id: usize, _data: usize) -> LRESULT {
    match m {
        WM_MOUSEMOVE => {
            if !set_hot(h, true) {
                // Ask for WM_MOUSELEAVE, to stop looking hot.
                let mut track = TRACKMOUSEEVENT { cbSize: core::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: h, dwHoverTime: 0 };
                TrackMouseEvent(&mut track);
                repaint(h);
            }
        }
        WM_MOUSELEAVE => {
            set_hot(h, false);
            repaint(h);
        }
        WM_ERASEBKGND => return 1, // drawn entirely in WM_DRAWITEM; avoids flicker
        // An owner-drawn button turns a quick second click into a double-click, which doesn't
        // click it: treat it as the press it is (so a check box can be ticked and unticked quickly).
        WM_LBUTTONDBLCLK => return DefSubclassProc(h, WM_LBUTTONDOWN, w, l),
        WM_NCDESTROY => {
            WIDGETS.with(|w| w.borrow_mut().remove(&h));
            RemoveWindowSubclass(h, widget_proc, SUBCLASS_ID);
        }
        _ => {}
    }
    DefSubclassProc(h, m, w, l)
}

// ------------------------------------------------------------------------------------------ drawing

/// Everything a widget's picture depends on.
struct Look<'a> {
    hdc: HDC,
    r: RECT,
    dpi: u32,
    text: &'a str,
    font: HFONT,
    icon_font: HFONT,
    icon: Option<char>,
    checked: bool,
    hot: bool,
    pressed: bool,
    disabled: bool,
    /// Has keyboard focus, and focus is being shown (it was reached with the keyboard).
    focused: bool,
}

impl Look<'_> {
    fn s(&self, v: i32) -> i32 {
        super::scale(v, self.dpi)
    }

    fn label_color(&self, p: &Palette) -> u32 {
        if self.disabled { p.text_disabled } else { p.text }
    }

    fn label(&self, p: &Palette, left: i32) {
        let r = RECT { left, ..self.r };
        draw::text(self.hdc, self.text, r, self.font, self.label_color(p), DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
    }
}

/// Draws one of our widgets (WM_DRAWITEM). Returns false if it isn't one.
pub fn draw(dis: &DRAWITEMSTRUCT) -> bool {
    if dis.CtlType != ODT_BUTTON {
        return false;
    }
    let h = dis.hwndItem;
    let Some((kind, icon, checked, hot, icon_font)) = WIDGETS.with(|w| w.borrow().get(&h).map(|x| (x.kind, x.icon, x.checked, x.hot, x.icon_font))) else {
        return false;
    };
    let text = super::get_text(h);
    let look = Look {
        hdc: dis.hDC,
        r: dis.rcItem,
        dpi: super::dpi_of(h),
        text: &text,
        font: unsafe { SendMessageW(h, WM_GETFONT, 0, 0) } as HFONT,
        icon_font,
        icon,
        checked,
        hot,
        pressed: dis.itemState & ODS_SELECTED != 0,
        disabled: dis.itemState & ODS_DISABLED != 0,
        focused: dis.itemState & ODS_FOCUS != 0 && dis.itemState & ODS_NOFOCUSRECT == 0,
    };
    let p = theme::palette();
    draw::fill(look.hdc, look.r, theme::color_of(theme::surface_of(h)));
    match kind {
        Kind::Check => draw_check(&look, &p),
        Kind::Switch => draw_switch(&look, &p),
        Kind::Standard | Kind::Accent | Kind::DropDown => draw_button(&look, &p, kind),
    }
    true
}

/// A rounded button: an optional icon and the text, centred together (a drop-down field: the
/// text on the left and a chevron on the right).
fn draw_button(l: &Look, p: &Palette, kind: Kind) {
    let s = |v| l.s(v);
    let (fill, fg, border) = match kind {
        Kind::Accent if l.disabled => (p.control, p.text_disabled, Some(p.control_stroke)),
        Kind::Accent => (if l.hot && !l.pressed { p.accent_hot } else { p.accent }, p.on_accent, None),
        _ => {
            let fill = if l.disabled { p.control } else if l.pressed { p.control_pressed } else if l.hot { p.control_hot } else { p.control };
            (fill, l.label_color(p), Some(p.control_stroke))
        }
    };
    draw::round_rect(l.hdc, l.r, s(4), fill, border);

    if kind == Kind::DropDown {
        let chevron = RECT { left: l.r.right - s(12 + 16), right: l.r.right - s(12), ..l.r };
        draw::glyph(l.hdc, if l.checked { ICON_UP } else { ICON_DOWN }, chevron, l.icon_font, fg, DT_CENTER | DT_VCENTER);
        let label = RECT { left: l.r.left + s(12), right: chevron.left - s(8), ..l.r };
        draw::text(l.hdc, l.text, label, l.font, fg, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
    } else {
        let icon_w = if l.icon.is_some() { s(16) } else { 0 };
        let gap = if l.icon.is_some() && !l.text.is_empty() { s(8) } else { 0 };
        let total = icon_w + gap + draw::text_width(l.text, l.font);
        let mut x = l.r.left + (l.r.width() - total).max(0) / 2;
        if let Some(c) = l.icon {
            draw::glyph(l.hdc, c, RECT { left: x, right: x + icon_w, ..l.r }, l.icon_font, fg, DT_CENTER | DT_VCENTER);
            x += icon_w + gap;
        }
        draw::text(l.hdc, l.text, RECT { left: x, right: l.r.right - s(4), ..l.r }, l.font, fg, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
    }

    if l.focused {
        draw::outline(l.hdc, l.r.inset(s(2), s(2)), s(3), fg);
    }
}

/// A rounded box, accent-filled with a tick when checked, then the label.
fn draw_check(l: &Look, p: &Palette) {
    let s = |v| l.s(v);
    let size = s(18);
    let tick_box = rect(l.r.left + s(1), l.r.top + (l.r.height() - size) / 2, size, size);
    if l.checked {
        let fill = if l.disabled { p.text_disabled } else if l.hot { p.accent_hot } else { p.accent };
        draw::round_rect(l.hdc, tick_box, s(4), fill, None);
        draw::glyph(l.hdc, ICON_TICK, tick_box, l.icon_font, p.on_accent, DT_CENTER | DT_VCENTER);
    } else {
        let fill = if l.hot && !l.disabled { p.control_hot } else { p.control };
        draw::round_rect(l.hdc, tick_box, s(4), fill, Some(if l.disabled { p.text_disabled } else { p.text2 }));
    }
    l.label(p, tick_box.right + s(8));
    if l.focused {
        draw::outline(l.hdc, tick_box.inset(-s(1), -s(1)), s(5), p.text);
    }
}

/// A pill-shaped track (accent-filled when on) with a round knob at its left (off) or right
/// (on), then the label.
fn draw_switch(l: &Look, p: &Palette) {
    let s = |v| l.s(v);
    // Inset, so the focus ring (2 px outside the track) stays inside the control.
    let track = rect(l.r.left + s(3), l.r.top + (l.r.height() - s(20)) / 2, s(40), s(20));
    let (fill, border, knob) = if l.checked {
        (if l.disabled { p.text_disabled } else if l.hot { p.accent_hot } else { p.accent }, None, p.on_accent)
    } else {
        let line = if l.disabled { p.text_disabled } else { p.text2 };
        (if l.hot && !l.disabled { p.control_hot } else { p.control }, Some(line), line)
    };
    draw::round_rect(l.hdc, track, track.height() / 2, fill, border);
    let k = s(if l.hot && !l.disabled { 14 } else { 12 });
    let knob_x = if l.checked { track.right - s(4) - k } else { track.left + s(4) };
    draw::round_rect(l.hdc, rect(knob_x, track.top + (track.height() - k) / 2, k, k), k / 2, knob, None);
    l.label(p, track.right + s(10));
    if l.focused {
        // Kept inside the control: anything outside is clipped (DPI rounding can push it out by a pixel).
        let ring = RECT {
            left: (track.left - s(2)).max(l.r.left),
            top: (track.top - s(2)).max(l.r.top),
            right: (track.right + s(2)).min(l.r.right - 1),
            bottom: (track.bottom + s(2)).min(l.r.bottom - 1),
        };
        draw::outline(l.hdc, ring, ring.height() / 2, p.text);
    }
}
