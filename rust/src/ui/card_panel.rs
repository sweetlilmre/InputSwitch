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

const CLASS: &str = "ui.CardPanel";

/// What a [`Cards`] implementation gets to work with.
pub struct Ctx {
    /// The panel: the parent for the cards' controls.
    pub panel: HWND,
    pub dpi: u32,
    /// The parent window's fonts.
    pub fonts: Rc<Fonts>,
}

impl Ctx {
    /// A length at 96 DPI, scaled for the panel.
    pub fn s(&self, v: i32) -> i32 {
        super::scale(v, self.dpi)
    }
}

/// The cards a panel shows. Positions are in panel pixels, laid out from y = 0; the panel
/// scrolls by shifting them up by `dy`.
pub trait Cards: Any {
    /// Lays the cards out for a panel `width` pixels wide and returns their total height.
    fn layout(&mut self, cx: &Ctx, width: i32) -> i32;

    /// Moves the cards' controls to where they are with the panel scrolled by `dy`.
    fn place_controls(&mut self, cx: &Ctx, dy: i32);

    /// Paints the cards, shifted up by `dy` (the background is already filled).
    fn paint(&self, cx: &Ctx, hdc: HDC, dy: i32);

    /// A notification from one of the cards' controls, before it goes on to the parent.
    /// Returns true if the cards need laying out again.
    fn on_command(&mut self, _cx: &Ctx, _control: HWND, _code: u16) -> bool {
        false
    }

    /// A click on the panel itself (not on a control), at `y` (unscrolled). Returns true if the
    /// cards need repainting.
    fn on_click(&mut self, _cx: &Ctx, _x: i32, _y: i32) -> bool {
        false
    }

    /// The card holding `control` (unscrolled), so it can be scrolled into view when it gets focus.
    fn card_of(&self, _control: HWND) -> Option<RECT> {
        None
    }

    /// The fonts changed (after a DPI change): give them to the controls.
    fn fonts_changed(&mut self, _cx: &Ctx) {}

    /// The theme changed: re-style the controls.
    fn retheme(&mut self, _cx: &Ctx) {}
}

struct Panel {
    cx: Ctx,
    cards: Box<dyn Cards>,
    scroll: i32,
    content_h: i32,
}

impl Panel {
    fn view_height(&self) -> i32 {
        draw::client_rect(self.cx.panel).height()
    }

    /// Lays the cards out again (after they or the panel's width changed).
    fn layout(&mut self) {
        let width = draw::client_rect(self.cx.panel).width();
        self.content_h = self.cards.layout(&self.cx, width);
        self.scroll_to(self.scroll);
    }

    /// Scrolls to `y` (kept in range) and moves the controls to match.
    fn scroll_to(&mut self, y: i32) {
        self.scroll = y.clamp(0, (self.content_h - self.view_height()).max(0));
        self.cards.place_controls(&self.cx, self.scroll);
    }

    /// Scrolls just enough for `control`'s card to be fully in view.
    fn reveal(&mut self, control: HWND) {
        let Some(card) = self.cards.card_of(control) else { return };
        let view = self.view_height();
        if card.top < self.scroll {
            self.scroll_to(card.top);
        } else if card.bottom > self.scroll + view {
            self.scroll_to(card.bottom - view);
        }
    }
}

type Shared = Rc<RefCell<Panel>>;

thread_local! {
    static PANELS: RefCell<HashMap<HWND, Shared>> = RefCell::new(HashMap::new());
}

/// The panel behind `h`, lent out unless it's already busy with a message further up the stack.
fn with_panel<R>(h: HWND, f: impl FnOnce(&mut Panel) -> R) -> Option<R> {
    let panel = PANELS.with(|p| p.borrow().get(&h).cloned())?;
    let mut panel = panel.try_borrow_mut().ok()?;
    Some(f(&mut panel))
}

// ------------------------------------------------------------------------------------------ public

/// Creates a panel showing `cards`, as a child of `parent` with the given control id, using the
/// parent's fonts.
pub fn create<T: Cards>(parent: HWND, id: i32, fonts: &Rc<Fonts>, cards: T) -> HWND {
    super::register_class(CLASS, wndproc, false, 0);
    let h = unsafe {
        // Not WS_TABSTOP itself: a control parent that's also a tab stop sends dialog navigation
        // round in circles. The controls on the cards are the tab stops.
        CreateWindowExW(WS_EX_CONTROLPARENT, wide(CLASS).as_ptr(), core::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_VSCROLL, 0, 0, 0, 0, parent, id as isize, hinstance(), core::ptr::null())
    };
    theme::apply_control(h, theme::ControlKind::ListBox); // dark scroll bar
    let cx = Ctx { panel: h, dpi: fonts.dpi, fonts: fonts.clone() };
    let panel = Rc::new(RefCell::new(Panel { cx, cards: Box::new(cards), scroll: 0, content_h: 0 }));
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
        let cards = (p.cards.as_mut() as &mut dyn Any).downcast_mut::<T>()?;
        Some(f(cards, &p.cx))
    })
    .flatten()
}

/// The height the cards need, at 96 DPI (for the parent's layout).
pub fn content_height(h: HWND) -> i32 {
    with_panel(h, |p| {
        let dpi = p.cx.dpi as i32;
        (p.content_h * 96 + dpi - 1) / dpi
    })
    .unwrap_or(0)
}

/// The parent's fonts changed (after a DPI change).
pub fn set_fonts(h: HWND, fonts: &Rc<Fonts>) {
    with_panel(h, |p| {
        p.cx.dpi = fonts.dpi;
        p.cx.fonts = fonts.clone();
        p.cards.fonts_changed(&p.cx);
    });
    relayout(h);
}

pub fn retheme(h: HWND) {
    theme::apply_control(h, theme::ControlKind::ListBox);
    with_panel(h, |p| p.cards.retheme(&p.cx));
    theme::repaint(h);
}

// ------------------------------------------------------------------------------------------ window

/// Lays the cards out and updates the scroll bar. Showing or hiding the scroll bar changes the
/// width (and sends WM_SIZE back here), so this settles in at most two rounds.
fn relayout(h: HWND) {
    for _ in 0..2 {
        let width = draw::client_rect(h).width();
        with_panel(h, Panel::layout);
        update_scrollbar(h);
        if draw::client_rect(h).width() == width {
            break;
        }
    }
    unsafe { InvalidateRect(h, core::ptr::null(), 0) };
}

fn update_scrollbar(h: HWND) {
    let Some((content, scroll)) = with_panel(h, |p| (p.content_h, p.scroll)) else { return };
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

/// Scrolls with `f`, then updates the scroll bar and repaints.
fn scroll(h: HWND, f: impl FnOnce(&mut Panel)) {
    with_panel(h, f);
    update_scrollbar(h);
    unsafe { InvalidateRect(h, core::ptr::null(), 0) };
}

/// Where the scroll bar's thumb is being dragged to (the full 32-bit position).
fn thumb_position(h: HWND) -> i32 {
    let mut si = SCROLLINFO { cbSize: core::mem::size_of::<SCROLLINFO>() as u32, fMask: SIF_TRACKPOS, nMin: 0, nMax: 0, nPage: 0, nPos: 0, nTrackPos: 0 };
    unsafe { GetScrollInfo(h, SB_VERT, &mut si) };
    si.nTrackPos
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let s = |v: i32| super::scale(v, super::dpi_of(h));
    match m {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            draw::paint_buffered(h, |hdc, rc| {
                draw::fill(hdc, rc, theme::palette().card);
                with_panel(h, |p| p.cards.paint(&p.cx, hdc, p.scroll));
            });
            return 0;
        }
        WM_SIZE => relayout(h),
        WM_VSCROLL => {
            let (page, line) = (draw::client_rect(h).height(), s(40));
            let code = loword(w) as u32;
            let thumb = matches!(code, SB_THUMBPOSITION | SB_THUMBTRACK).then(|| thumb_position(h));
            scroll(h, |p| match code {
                SB_LINEUP => p.scroll_to(p.scroll - line),
                SB_LINEDOWN => p.scroll_to(p.scroll + line),
                SB_PAGEUP => p.scroll_to(p.scroll - page),
                SB_PAGEDOWN => p.scroll_to(p.scroll + page),
                SB_TOP => p.scroll_to(0),
                SB_BOTTOM => p.scroll_to(i32::MAX / 2),
                _ => thumb.into_iter().for_each(|y| p.scroll_to(y)),
            });
            return 0;
        }
        WM_MOUSEWHEEL => {
            let notches = (hiword(w) as i16) as i32; // WHEEL_DELTA (120) per notch
            scroll(h, |p| p.scroll_to(p.scroll - notches * s(48) / 120));
            return 0;
        }
        WM_LBUTTONDOWN => {
            let (x, y) = ((l & 0xFFFF) as i16 as i32, ((l >> 16) & 0xFFFF) as i16 as i32);
            if with_panel(h, |p| p.cards.on_click(&p.cx, x, y + p.scroll)) == Some(true) {
                InvalidateRect(h, core::ptr::null(), 0);
            }
            return 0;
        }
        WM_COMMAND => {
            let (control, code) = (l as HWND, hiword(w));
            if with_panel(h, |p| p.cards.on_command(&p.cx, control, code)) == Some(true) {
                relayout(h);
            }
            if matches!(code, EN_SETFOCUS | BN_SETFOCUS) {
                scroll(h, |p| p.reveal(control)); // e.g. tabbing to a card out of view
            }
            if matches!(code, EN_SETFOCUS | EN_KILLFOCUS) {
                InvalidateRect(h, core::ptr::null(), 0); // focus changes how a text box looks
            }
            return SendMessageW(GetParent(h), WM_COMMAND, w, l);
        }
        WM_DRAWITEM if widgets::draw(&*(l as *const DRAWITEMSTRUCT)) => return 1,
        WM_CTLCOLOREDIT => return super::text_box::edit_colors(w as HDC, l as HWND),
        WM_NCDESTROY => {
            PANELS.with(|p| p.borrow_mut().remove(&h));
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}
