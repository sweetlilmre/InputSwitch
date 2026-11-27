//! The Hotkeys list, drawn like the C# app's: one card per hotkey with its keys as key caps,
//! the action (and monitor) beside them, and Test, Edit and Delete buttons.

use crate::ffi::*;
use crate::ui::card_panel::{self, Cards, Ctx};
use crate::ui::draw::{self, rect, RectExt};
use crate::ui::theme::{self, Surface};
use crate::ui::{self, widgets};

/// What one card shows.
#[derive(Clone, PartialEq)]
pub struct Row {
    pub keys: Vec<String>,
    pub action: String,
    pub monitor: String,
}

/// What the user did on a card (the row is the hotkey's index).
pub enum Action {
    Test(usize),
    Edit(usize),
    Remove(usize),
}

// Each card's buttons have ids from these bases, plus the row.
const TEST_ID: i32 = 3000;
const EDIT_ID: i32 = 3500;
const REMOVE_ID: i32 = 4000;
const MAX_ROWS: usize = 500;

/// Recognises a card button's click.
pub fn action(id: i32, code: u16) -> Option<Action> {
    if code != BN_CLICKED {
        return None;
    }
    let row = |base: i32| (base..base + MAX_ROWS as i32).contains(&id).then(|| (id - base) as usize);
    row(TEST_ID).map(Action::Test).or_else(|| row(EDIT_ID).map(Action::Edit)).or_else(|| row(REMOVE_ID).map(Action::Remove))
}

/// Creates the (empty) list of hotkey cards.
pub fn create(parent: HWND, id: i32, fonts: &std::rc::Rc<crate::ui::Fonts>) -> HWND {
    card_panel::create(parent, id, fonts, HotkeyCards { cards: Vec::new() })
}

/// Shows these hotkeys.
pub fn set_rows(panel: HWND, rows: Vec<Row>) {
    card_panel::update(panel, |h: &mut HotkeyCards, cx| h.set_rows(cx, rows));
}

// Layout at 96 DPI, from the C# template.
const GAP: i32 = 6;
const PAD_X: i32 = 14;
const PAD_Y: i32 = 10;
const KEYS_W: i32 = 220;
const CAP_H: i32 = 25;
const CAP_MIN_W: i32 = 28;
const CAP_GAP: i32 = 4;
/// The action and monitor lines together; the action line is ACTION_H of it.
const TEXT_H: i32 = 36;
const ACTION_H: i32 = 20;
/// Test, Edit and Delete.
const BUTTONS: i32 = 3;
const BUTTON_W: i32 = 34;
const BUTTON_H: i32 = 32;
const BUTTON_GAP: i32 = 4;
const EMPTY_TOP: i32 = 16;
const EMPTY_H: i32 = 20;

const EMPTY: &str = "No hotkeys yet. Add one to switch a monitor's input from the keyboard.";

struct Card {
    row: Row,
    /// Test, Edit, Delete.
    buttons: [HWND; 3],
    /// Laid out unscrolled, in panel pixels.
    rect: RECT,
}

/// The cards, and the buttons on them.
pub struct HotkeyCards {
    cards: Vec<Card>,
}

impl HotkeyCards {
    /// The buttons are made again only when the number of hotkeys changes.
    fn set_rows(&mut self, cx: &Ctx, rows: Vec<Row>) {
        if self.cards.len() == rows.len() {
            for (c, r) in self.cards.iter_mut().zip(rows) {
                c.row = r;
            }
            return;
        }
        for b in self.cards.drain(..).flat_map(|c| c.buttons) {
            unsafe { DestroyWindow(b) };
        }
        for (i, row) in rows.into_iter().take(MAX_ROWS).enumerate() {
            let button = |base: i32, icon| widgets::button(cx.panel, "", base + i as i32, Some(icon), &cx.fonts, Surface::Row);
            let buttons = [button(TEST_ID, widgets::ICON_PLAY), button(EDIT_ID, widgets::ICON_EDIT), button(REMOVE_ID, widgets::ICON_DELETE)];
            self.cards.push(Card { row, buttons, rect: RECT::default() });
        }
    }
}

impl Cards for HotkeyCards {
    fn layout(&mut self, cx: &Ctx, width: i32) -> i32 {
        if self.cards.is_empty() {
            return cx.s(EMPTY_TOP + EMPTY_H);
        }
        let height = cx.s(PAD_Y * 2 + TEXT_H);
        for (i, c) in self.cards.iter_mut().enumerate() {
            c.rect = rect(0, i as i32 * (height + cx.s(GAP)), width, height);
        }
        self.cards.len() as i32 * (height + cx.s(GAP)) - cx.s(GAP)
    }

    fn place_controls(&mut self, cx: &Ctx, dy: i32) {
        for c in &self.cards {
            let card = c.rect.offset(0, -dy);
            let y = card.top + (card.height() - cx.s(BUTTON_H)) / 2;
            let mut x = card.right - cx.s(PAD_X) - buttons_width(cx);
            for &b in &c.buttons {
                ui::place(b, x, y, cx.s(BUTTON_W), cx.s(BUTTON_H));
                x += cx.s(BUTTON_W) + cx.s(BUTTON_GAP);
            }
        }
    }

    fn paint(&self, cx: &Ctx, hdc: HDC, dy: i32) {
        let p = theme::palette();
        let s = |v| cx.s(v);
        if self.cards.is_empty() {
            let r = RECT { left: 0, top: s(EMPTY_TOP) - dy, right: draw::client_rect(cx.panel).right, bottom: s(EMPTY_TOP + EMPTY_H) - dy };
            draw::text(hdc, EMPTY, r, cx.fonts.normal, p.text2, DT_SINGLELINE | DT_END_ELLIPSIS);
            return;
        }
        for c in &self.cards {
            let card = c.rect.offset(0, -dy);
            draw::round_rect(hdc, card, s(6), p.row, Some(p.row_stroke));
            let mid = (card.top + card.bottom) / 2;

            // Key caps, with a slightly deeper bottom edge, like a key.
            let mut x = card.left + s(PAD_X);
            let cap_top = mid - s(CAP_H) / 2;
            for key in &c.row.keys {
                let cap_w = draw::text_width(key, cx.fonts.key_cap) + s(16) + 2; // padding, and a pixel of border each side
                let cap = rect(x, cap_top, cap_w.max(s(CAP_MIN_W)), s(CAP_H));
                draw::round_rect(hdc, cap, s(4), p.chip_stroke, None);
                let face = RECT { left: cap.left + 1, top: cap.top + 1, right: cap.right - 1, bottom: cap.bottom - s(2) };
                draw::round_rect(hdc, face, s(3), p.field, None);
                draw::text(hdc, key, RECT { bottom: cap.bottom - 1, ..cap }, cx.fonts.key_cap, p.text, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                x = cap.right + s(CAP_GAP);
            }

            // The action, with the monitor underneath.
            let left = (card.left + s(PAD_X + KEYS_W)).max(x + s(12));
            let right = card.right - s(PAD_X) - buttons_width(cx) - s(12);
            let top = mid - s(TEXT_H) / 2;
            draw::text(hdc, &c.row.action, RECT { left, top, right, bottom: top + s(ACTION_H) }, cx.fonts.normal, p.text, DT_SINGLELINE | DT_END_ELLIPSIS);
            draw::text(hdc, &c.row.monitor, RECT { left, top: top + s(ACTION_H), right, bottom: top + s(TEXT_H) }, cx.fonts.small, p.text2, DT_SINGLELINE | DT_END_ELLIPSIS);
        }
    }

    fn card_of(&self, control: HWND) -> Option<RECT> {
        self.cards.iter().find(|c| c.buttons.contains(&control)).map(|c| c.rect)
    }

    fn fonts_changed(&mut self, cx: &Ctx) {
        for &b in self.cards.iter().flat_map(|c| &c.buttons) {
            widgets::set_fonts(b, &cx.fonts);
        }
    }
}

/// The width of a card's buttons, side by side, in pixels.
fn buttons_width(cx: &Ctx) -> i32 {
    BUTTONS * cx.s(BUTTON_W) + (BUTTONS - 1) * cx.s(BUTTON_GAP)
}

