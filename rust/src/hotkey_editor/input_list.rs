//! The hotkey editor's inputs, like the C# app's list: one row per input with a check box, in
//! the order the hotkey steps through them. Ticking is the check box's job; clicking a row (or
//! tabbing to its check box) selects it, for moving it up or down.

use crate::ffi::*;
use crate::monitors;
use crate::ui::card_panel::{self, Cards, Ctx};
use crate::ui::draw::{self, rect, RectExt};
use crate::ui::theme::{self, Surface};
use crate::ui::{self, widgets, Fonts};
use std::rc::Rc;

/// The check boxes have ids from this base, plus the row.
const CHECK_ID: i32 = 400;
const MAX_ROWS: usize = 256;

// At 96 DPI.
const ROW_H: i32 = 56;
const PAD_X: i32 = 14;
const CHECK: i32 = 22;
const LABEL_GAP: i32 = 10;

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

/// True if this notification is one of the check boxes being ticked or unticked.
pub fn is_check(id: i32, code: u16) -> bool {
    code == BN_CLICKED && (CHECK_ID..CHECK_ID + MAX_ROWS as i32).contains(&id)
}

pub fn create(parent: HWND, id: i32, fonts: &Rc<Fonts>) -> HWND {
    card_panel::create(parent, id, fonts, InputList { rows: Vec::new(), selected: None, width: 0, row_h: 0 })
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

/// Moves the selected input up (-1) or down (+1), ticked or not.
pub fn move_selected(panel: HWND, delta: isize) {
    card_panel::update(panel, |l: &mut InputList, _| {
        let Some(i) = l.selected else { return };
        let Some(j) = i.checked_add_signed(delta).filter(|&j| j < l.rows.len()) else { return };
        // Swap what the two rows show; their check boxes stay in place, keeping the tab order.
        let (a_code, a_on) = (l.rows[i].code, widgets::checked(l.rows[i].check));
        let (b_code, b_on) = (l.rows[j].code, widgets::checked(l.rows[j].check));
        l.show(i, b_code, b_on);
        l.show(j, a_code, a_on);
        l.select(Some(j));
    });
}

impl InputList {
    fn push(&mut self, cx: &Ctx, code: u8, on: bool) {
        let id = CHECK_ID + self.rows.len() as i32;
        // Just the box: the input's name is painted beside it, so clicking that selects the row.
        let check = widgets::checkbox(cx.panel, "", id, &cx.fonts, Surface::Card);
        widgets::set_checked(check, on);
        self.rows.push(Row { code, check });
    }

    fn show(&mut self, i: usize, code: u8, on: bool) {
        self.rows[i].code = code;
        widgets::set_checked(self.rows[i].check, on);
    }

    /// Selects row `i`: it's highlighted (its check box too).
    fn select(&mut self, i: Option<usize>) {
        for (k, r) in self.rows.iter().enumerate() {
            theme::set_surface(r.check, if Some(k) == i { Surface::Selection } else { Surface::Card }, false);
            unsafe { InvalidateRect(r.check, core::ptr::null(), 0) };
        }
        self.selected = i;
    }

    /// Row `i`, unscrolled.
    fn row(&self, i: usize) -> RECT {
        rect(0, i as i32 * self.row_h, self.width, self.row_h)
    }
}

impl Cards for InputList {
    fn layout(&mut self, cx: &Ctx, width: i32) -> i32 {
        (self.width, self.row_h) = (width, cx.s(ROW_H));
        self.rows.len() as i32 * self.row_h
    }

    fn place_controls(&mut self, cx: &Ctx, dy: i32) {
        for (i, r) in self.rows.iter().enumerate() {
            let row = self.row(i).offset(0, -dy);
            let top = row.top + (row.height() - cx.s(CHECK)) / 2;
            ui::place(r.check, row.left + cx.s(PAD_X), top, cx.s(CHECK), cx.s(CHECK));
        }
    }

    fn paint(&self, cx: &Ctx, hdc: HDC, dy: i32) {
        let p = theme::palette();
        for (i, r) in self.rows.iter().enumerate() {
            let row = self.row(i).offset(0, -dy);
            if Some(i) == self.selected {
                draw::fill(hdc, row, p.selection);
            }
            let label = RECT { left: row.left + cx.s(PAD_X + CHECK + LABEL_GAP), right: row.right - cx.s(8), ..row };
            draw::text(hdc, &monitors::long_name(r.code), label, cx.fonts.normal, p.text, DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
        }
    }

    /// Tabbing to a row's check box selects the row.
    fn on_command(&mut self, _cx: &Ctx, control: HWND, code: u16) -> bool {
        if code == BN_SETFOCUS {
            if let Some(i) = self.rows.iter().position(|r| r.check == control) {
                self.select(Some(i));
            }
        }
        false
    }

    /// Clicking a row selects it (without ticking it).
    fn on_click(&mut self, _cx: &Ctx, _x: i32, y: i32) -> bool {
        let i = (self.row_h > 0).then(|| (y / self.row_h) as usize).filter(|&i| i < self.rows.len());
        if i.is_some() {
            self.select(i);
        }
        i.is_some()
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
