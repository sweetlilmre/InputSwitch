//! The hotkey editor's inputs: one row per input with a check box, in the order the hotkey steps
//! through them. The row last clicked is selected, for moving it up or down.

use crate::ffi::*;
use crate::monitors;
use crate::ui::card_panel::{self, Cards, Ctx};
use crate::ui::draw::{self, rect, RectExt};
use crate::ui::theme::{self, Surface};
use crate::ui::{self, widgets};

/// The check boxes have ids from this base, plus the row.
const CHECK_ID: i32 = 400;
const MAX_ROWS: usize = 256;

const ROW_H: i32 = 56;
const PAD_X: i32 = 10;

struct Row {
    code: u8,
    check: HWND,
}

pub struct InputList {
    rows: Vec<Row>,
    selected: Option<usize>,
    /// The panel's width and a row's height, in pixels, as last laid out.
    width: i32,
    row_h: i32,
}

/// True if this notification is one of the check boxes being clicked.
pub fn is_check(id: i32, code: u16) -> bool {
    code == BN_CLICKED && (CHECK_ID..CHECK_ID + MAX_ROWS as i32).contains(&id)
}

pub fn create(parent: HWND, id: i32, dpi: u32) -> HWND {
    card_panel::create(parent, id, dpi, InputList { rows: Vec::new(), selected: None, width: 0, row_h: 0 })
}

/// Shows these inputs, in order, each ticked or not.
pub fn set_inputs(panel: HWND, inputs: &[(u8, bool)]) {
    card_panel::update(panel, |l: &mut InputList, cx| {
        for r in l.rows.drain(..) {
            unsafe { DestroyWindow(r.check) };
        }
        l.selected = None;
        for &(code, on) in inputs.iter().take(MAX_ROWS) {
            l.push(cx, code, on);
        }
    });
}

/// The ticked inputs, in order.
pub fn checked(panel: HWND) -> Vec<u8> {
    card_panel::with(panel, |l: &mut InputList, _| l.rows.iter().filter(|r| widgets::checked(r.check)).map(|r| r.code).collect())
        .unwrap_or_default()
}

/// Adds an input (or finds it), ticks it and selects it.
pub fn add(panel: HWND, code: u8) {
    card_panel::update(panel, |l: &mut InputList, cx| {
        let i = l.rows.iter().position(|r| r.code == code).unwrap_or_else(|| {
            l.push(cx, code, false);
            l.rows.len() - 1
        });
        widgets::set_checked(l.rows[i].check, true);
        l.select(Some(i));
    });
}

/// Moves the selected input up (-1) or down (+1).
pub fn move_selected(panel: HWND, delta: isize) {
    card_panel::update(panel, |l: &mut InputList, _| {
        let Some(i) = l.selected else { return };
        let Some(j) = i.checked_add_signed(delta).filter(|&j| j < l.rows.len()) else { return };
        // Swap what the two rows show (their controls stay in place, keeping the tab order).
        let (a, b) = (&l.rows[i], &l.rows[j]);
        let (a_on, b_on) = (widgets::checked(a.check), widgets::checked(b.check));
        let (a_code, b_code) = (a.code, b.code);
        l.show(i, b_code, b_on);
        l.show(j, a_code, a_on);
        l.select(Some(j));
    });
}

impl InputList {
    fn push(&mut self, cx: &Ctx, code: u8, on: bool) {
        let id = CHECK_ID + self.rows.len() as i32;
        let check = widgets::checkbox(cx.panel, &monitors::long_name(code), id, &cx.fonts, Surface::Card);
        widgets::set_checked(check, on);
        self.rows.push(Row { code, check });
    }

    fn show(&mut self, i: usize, code: u8, on: bool) {
        let r = &mut self.rows[i];
        r.code = code;
        ui::set_text(r.check, &monitors::long_name(code));
        widgets::set_checked(r.check, on);
    }

    /// Row i, unscrolled.
    fn row(&self, i: usize) -> RECT {
        rect(0, i as i32 * self.row_h, self.width, self.row_h)
    }

    /// The selected row is drawn on the row colour, like a selected list item.
    fn select(&mut self, i: Option<usize>) {
        for (k, r) in self.rows.iter().enumerate() {
            theme::set_surface(r.check, if Some(k) == i { Surface::Row } else { Surface::Card }, false);
            unsafe { InvalidateRect(r.check, core::ptr::null(), 0) };
        }
        self.selected = i;
    }
}

impl Cards for InputList {
    fn layout(&mut self, cx: &Ctx, width: i32) -> i32 {
        (self.width, self.row_h) = (width, cx.s(ROW_H));
        self.rows.len() as i32 * self.row_h
    }

    fn place_controls(&mut self, cx: &Ctx, dy: i32) {
        for (i, r) in self.rows.iter().enumerate() {
            ui::place_rect(r.check, self.row(i).offset(0, -dy).inset(cx.s(PAD_X), cx.s(12)));
        }
    }

    fn paint(&self, cx: &Ctx, hdc: HDC, dy: i32) {
        if let Some(i) = self.selected {
            draw::round_rect(hdc, self.row(i).offset(0, -dy).inset(cx.s(2), cx.s(2)), cx.s(4), theme::color_of(Surface::Row), None);
        }
    }

    fn on_command(&mut self, _cx: &Ctx, control: HWND, code: u16) -> bool {
        if code == BN_CLICKED {
            if let Some(i) = self.rows.iter().position(|r| r.check == control) {
                self.select(Some(i));
                return true;
            }
        }
        false
    }

    fn card_of(&self, control: HWND) -> Option<RECT> {
        self.rows.iter().position(|r| r.check == control).map(|i| self.row(i))
    }

    fn fonts_changed(&mut self, cx: &Ctx) {
        for r in &self.rows {
            widgets::set_fonts(r.check, &cx.fonts);
        }
    }
}
