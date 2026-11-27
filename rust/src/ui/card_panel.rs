//! A scrolling panel of cards. The panel owns the window, scrolling, the scroll bar and
//! flicker-free painting; what the cards are is up to a [`Cards`] implementation, which lays
//! them out, paints them, and owns any real controls on them (edit boxes, buttons).
//!
//! The controls' WM_COMMAND messages go on to the panel's parent unchanged, so the parent
//! handles a card's button like any other.

use super::draw::{self, RectExt};
use super::{theme, widgets, Fonts};
use crate::ffi::*;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const CLASS: &str = "InputSwitch.CardPanel";

/// What a [`Cards`] implementation gets to work with.
pub struct Ctx {
    /// The panel: the parent for the cards' controls.
    pub panel: HWND,
    pub dpi: u32,
    pub fonts: Fonts,
}

impl Ctx {
    /// A length at 96 DPI, scaled for the panel.
    pub fn s(&self, v: i32) -> i32 {
        super::scale(v, self.dpi)
    }
}

pub trait Cards: 'static {
    /// Lays the cards out for a panel `width` pixels wide, starting at y = 0, and returns
    /// their total height.
    fn layout(&mut self, cx: &Ctx, width: i32) -> i32;

    /// Moves the cards' controls to where they are with the panel scrolled by `dy` pixels.
    fn place_controls(&mut self, cx: &Ctx, dy: i32);

    /// Paints the cards, shifted up by `dy` pixels (the background is already filled).
    fn paint(&self, cx: &Ctx, hdc: HDC, dy: i32);

    /// A notification from one of the cards' controls, before it goes on to the parent.
    /// Returns true if the cards need laying out again.
    fn on_command(&mut self, _cx: &Ctx, _control: HWND, _code: u16) -> bool {
        false
    }

    /// The card holding `control` (unscrolled), to keep it in view when it gets focus.
    fn card_of(&self, _control: HWND) -> Option<RECT> {
        None
    }

    /// The fonts changed (after a DPI change): give them to the controls.
    fn fonts_changed(&mut self, _cx: &Ctx) {}

    /// The theme changed: re-style the controls.
    fn retheme(&mut self, _cx: &Ctx) {}
}

/// A panel's state; `T` is known only to the code that created it.
struct Panel<T> {
    cx: Ctx,
    cards: T,
    scroll: i32,
    content_h: i32,
}

/// The type-independent side of a panel, for the window procedure.
trait AnyPanel {
    fn as_any(&mut self) -> &mut dyn Any;
    fn layout(&mut self);
    fn paint(&self, hdc: HDC);
    fn on_command(&mut self, control: HWND, code: u16) -> bool;
    /// Scrolls so `control`'s card is fully in view.
    fn reveal(&mut self, control: HWND);
    fn scroll_by(&mut self, dy: i32);
    fn scroll_to(&mut self, y: i32);
    fn scroll_info(&self) -> (i32, i32);
    fn set_dpi(&mut self, dpi: u32);
    fn retheme(&mut self);
}

impl<T: Cards> AnyPanel for Panel<T> {
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn layout(&mut self) {
        let rc = draw::client_rect(self.cx.panel);
        self.content_h = self.cards.layout(&self.cx, rc.width());
        self.scroll = self.scroll.clamp(0, (self.content_h - rc.height()).max(0));
        self.cards.place_controls(&self.cx, self.scroll);
    }

    fn paint(&self, hdc: HDC) {
        draw::fill(hdc, draw::client_rect(self.cx.panel), theme::palette().card);
        self.cards.paint(&self.cx, hdc, self.scroll);
    }

    fn on_command(&mut self, control: HWND, code: u16) -> bool {
        self.cards.on_command(&self.cx, control, code)
    }

    fn reveal(&mut self, control: HWND) {
        let Some(card) = self.cards.card_of(control) else { return };
        let view = draw::client_rect(self.cx.panel).height();
        if card.top < self.scroll {
            self.scroll = card.top;
        } else if card.bottom > self.scroll + view {
            self.scroll = card.bottom - view;
        }
    }

    fn scroll_by(&mut self, dy: i32) {
        self.scroll += dy;
    }

    fn scroll_to(&mut self, y: i32) {
        self.scroll = y;
    }

    fn scroll_info(&self) -> (i32, i32) {
        (self.content_h, self.scroll)
    }

    fn set_dpi(&mut self, dpi: u32) {
        self.cx.dpi = dpi;
        self.cx.fonts = Fonts::new(dpi);
        self.cards.fonts_changed(&self.cx);
    }

    fn retheme(&mut self) {
        self.cards.retheme(&self.cx);
    }
}

type Shared = Rc<RefCell<dyn AnyPanel>>;

thread_local! {
    static PANELS: RefCell<HashMap<HWND, Shared>> = RefCell::new(HashMap::new());
}

/// The panel behind `h`, lent out unless it's already busy with a message further up the stack.
fn with_panel<R>(h: HWND, f: impl FnOnce(&mut dyn AnyPanel) -> R) -> Option<R> {
    let panel = PANELS.with(|p| p.borrow().get(&h).cloned())?;
    let mut panel = panel.try_borrow_mut().ok()?;
    Some(f(&mut *panel))
}

// ------------------------------------------------------------------------------------------ public

/// Creates a panel showing `cards`, as a child of `parent` with the given control id.
pub fn create<T: Cards>(parent: HWND, id: i32, dpi: u32, cards: T) -> HWND {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| super::register_class(CLASS, wndproc, 0));
    let h = unsafe {
        // Not WS_TABSTOP itself: a control parent that's also a tab stop sends dialog navigation
        // round in circles. The controls on the cards are the tab stops.
        CreateWindowExW(WS_EX_CONTROLPARENT, wide(CLASS).as_ptr(), core::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_VSCROLL, 0, 0, 0, 0, parent, id as isize, hinstance(), core::ptr::null())
    };
    theme::apply_control(h, theme::ControlKind::ListBox); // dark scroll bar
    let panel: Shared = Rc::new(RefCell::new(Panel { cx: Ctx { panel: h, dpi, fonts: Fonts::new(dpi) }, cards, scroll: 0, content_h: 0 }));
    PANELS.with(|p| p.borrow_mut().insert(h, panel));
    h
}

/// Changes the cards, then lays them out again.
pub fn update<T: Cards, R>(h: HWND, f: impl FnOnce(&mut T, &Ctx) -> R) -> Option<R> {
    let r = with(h, f);
    relayout(h);
    r
}

/// Looks at (or quietly changes) the cards without laying them out again.
pub fn with<T: Cards, R>(h: HWND, f: impl FnOnce(&mut T, &Ctx) -> R) -> Option<R> {
    with_panel(h, |p| {
        let panel = p.as_any().downcast_mut::<Panel<T>>()?;
        Some(f(&mut panel.cards, &panel.cx))
    })
    .flatten()
}

/// The height the cards need, at 96 DPI (for the parent's layout).
pub fn content_height(h: HWND) -> i32 {
    let dpi = unsafe { GetDpiForWindow(h) }.max(96) as i32;
    with_panel(h, |p| p.scroll_info().0).map_or(0, |px| (px * 96 + dpi - 1) / dpi)
}

pub fn set_dpi(h: HWND, dpi: u32) {
    with_panel(h, |p| p.set_dpi(dpi));
    relayout(h);
}

pub fn retheme(h: HWND) {
    theme::apply_control(h, theme::ControlKind::ListBox);
    with_panel(h, |p| p.retheme());
    theme::repaint(h);
}

// ------------------------------------------------------------------------------------------ window

/// Lays the cards out and updates the scroll bar. Showing or hiding the scroll bar changes the
/// width (and sends WM_SIZE back here), so this settles in at most two rounds.
fn relayout(h: HWND) {
    for _ in 0..2 {
        let width = draw::client_rect(h).width();
        with_panel(h, |p| p.layout());
        update_scrollbar(h);
        if draw::client_rect(h).width() == width {
            break;
        }
    }
    unsafe { InvalidateRect(h, core::ptr::null(), 0) };
}

fn update_scrollbar(h: HWND) {
    let Some((content, scroll)) = with_panel(h, |p| p.scroll_info()) else { return };
    let si = SCROLLINFO {
        cbSize: core::mem::size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
        nMin: 0,
        nMax: (content - 1).max(0),
        nPage: draw::client_rect(h).height().max(0) as u32,
        nPos: scroll,
        nTrackPos: 0,
    };
    unsafe { SetScrollInfo(h, SB_VERT, &si, 1) };
}

/// Changes the scroll position, then moves everything to match.
fn scroll(h: HWND, f: impl FnOnce(&mut dyn AnyPanel)) {
    with_panel(h, |p| {
        f(p);
        p.layout();
    });
    update_scrollbar(h);
    unsafe { InvalidateRect(h, core::ptr::null(), 0) };
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let s = |v: i32| super::scale(v, GetDpiForWindow(h).max(96));
    match m {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            draw::paint_buffered(h, |hdc, rc| {
                if with_panel(h, |p| p.paint(hdc)).is_none() {
                    draw::fill(hdc, rc, theme::palette().card);
                }
            });
            return 0;
        }
        WM_SIZE => relayout(h),
        WM_VSCROLL => {
            let (page, line, thumb) = (draw::client_rect(h).height(), s(40), hiword(w) as i32);
            scroll(h, |p| match loword(w) as u32 {
                SB_LINEUP => p.scroll_by(-line),
                SB_LINEDOWN => p.scroll_by(line),
                SB_PAGEUP => p.scroll_by(-page),
                SB_PAGEDOWN => p.scroll_by(page),
                SB_THUMBPOSITION | SB_THUMBTRACK => p.scroll_to(thumb),
                SB_TOP => p.scroll_to(0),
                SB_BOTTOM => p.scroll_to(i32::MAX / 2),
                _ => {}
            });
            return 0;
        }
        WM_MOUSEWHEEL => {
            let notches = (hiword(w) as i16) as i32; // WHEEL_DELTA (120) per notch
            scroll(h, |p| p.scroll_by(-notches * s(48) / 120));
            return 0;
        }
        WM_COMMAND => {
            let (control, code) = (l as HWND, hiword(w));
            if with_panel(h, |p| p.on_command(control, code)) == Some(true) {
                relayout(h);
            }
            if code == EN_SETFOCUS || code == EN_KILLFOCUS {
                if code == EN_SETFOCUS {
                    scroll(h, |p| p.reveal(control)); // e.g. tabbing to a card out of view
                }
                InvalidateRect(h, core::ptr::null(), 0); // focus changes how a text box looks
            }
            return SendMessageW(GetParent(h), WM_COMMAND, w, l);
        }
        WM_DRAWITEM if widgets::draw(&*(l as *const DRAWITEMSTRUCT)) => return 1,
        WM_CTLCOLOREDIT => {
            let p = theme::palette();
            return theme::colour_control(w as HDC, p.text, p.field);
        }
        WM_NCDESTROY => {
            PANELS.with(|p| p.borrow_mut().remove(&h));
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}
