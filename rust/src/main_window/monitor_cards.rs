//! The Monitors list, drawn like the C# app's: one card per monitor with an icon, an editable
//! name box, "model · S/N" details, the inputs as chips (the current one highlighted), a
//! Connected / Not connected pill, and a delete button.

use crate::ffi::*;
use crate::ui::card_panel::{self, Cards, Ctx};
use crate::ui::draw::{self, rect, RectExt};
use crate::ui::theme::{self, Surface};
use crate::ui::{self, widgets};

/// What one card shows.
#[derive(Clone, PartialEq)]
pub struct Row {
    pub id: String,
    pub name: String,
    /// "Model · S/N serial".
    pub details: String,
    pub connected: bool,
    /// The inputs' short names, and whether each is the current one.
    pub chips: Vec<(String, bool)>,
    /// Shown instead of chips when the inputs aren't known.
    pub hint: String,
}

/// What the user did on a card (the row is the monitor's index).
pub enum Action {
    /// Typed in a name box.
    Renaming(usize),
    /// Left a name box.
    Renamed(usize),
    Remove(usize),
}

// The cards' controls have ids from these bases, plus the row.
const NAME_ID: i32 = 2000;
const REMOVE_ID: i32 = 2500;
const MAX_ROWS: usize = 500;

/// Recognises a card control's notification.
pub fn action(id: i32, code: u16) -> Option<Action> {
    let row = |base: i32| (base..base + MAX_ROWS as i32).contains(&id).then(|| (id - base) as usize);
    match code {
        EN_CHANGE => row(NAME_ID).map(Action::Renaming),
        EN_KILLFOCUS => row(NAME_ID).map(Action::Renamed),
        BN_CLICKED => row(REMOVE_ID).map(Action::Remove),
        _ => None,
    }
}

pub fn create(parent: HWND, id: i32, dpi: u32) -> HWND {
    card_panel::create(parent, id, dpi, MonitorCards { cards: Vec::new() })
}

/// Shows these monitors. Cards for the same monitors (in the same order) are updated in place,
/// so a name being typed keeps its caret; a name box's text is only replaced when it isn't focused.
pub fn set_rows(panel: HWND, rows: Vec<Row>) {
    card_panel::update(panel, |m: &mut MonitorCards, cx| m.set_rows(cx, rows));
}

/// What's typed in a monitor's name box.
pub fn name_text(panel: HWND, row: usize) -> Option<String> {
    card_panel::with(panel, |m: &mut MonitorCards, _| m.cards.get(row).map(|c| ui::get_text(c.name))).flatten()
}

pub fn set_name_text(panel: HWND, row: usize, text: &str) {
    if let Some(h) = card_panel::with(panel, |m: &mut MonitorCards, _| m.cards.get(row).map(|c| c.name)).flatten() {
        ui::set_text(h, text);
    }
}

// Layout at 96 DPI, from the C# template.
const GAP: i32 = 6;
const PAD_X: i32 = 14;
const PAD_Y: i32 = 12;
const ICON: i32 = 28;
const ICON_GAP: i32 = 16;
const NAME_H: i32 = 32;
const NAME_MIN_W: i32 = 220;
const NAME_MAX_W: i32 = 360;
const LINE_H: i32 = 16;
const CHIP_H: i32 = 22;
const CHIP_GAP: i32 = 6;
const PILL_H: i32 = 22;
const BUTTON_W: i32 = 34;

struct Card {
    row: Row,
    name: HWND,
    remove: HWND,
    // Laid out unscrolled, in panel pixels.
    rect: RECT,
    name_frame: RECT,
    chips: Vec<RECT>,
    pill: RECT,
}

pub struct MonitorCards {
    cards: Vec<Card>,
}

fn pill_text(connected: bool) -> &'static str {
    if connected { "Connected" } else { "Not connected" }
}

impl MonitorCards {
    fn set_rows(&mut self, cx: &Ctx, rows: Vec<Row>) {
        let same_monitors = self.cards.len() == rows.len()
            && self.cards.iter().zip(&rows).all(|(c, r)| c.row.id.eq_ignore_ascii_case(&r.id));
        if same_monitors {
            let focus = unsafe { GetFocus() };
            for (c, r) in self.cards.iter_mut().zip(rows) {
                if c.name != focus && ui::get_text(c.name) != r.name {
                    ui::set_text(c.name, &r.name);
                }
                c.row = r;
            }
            return;
        }
        for c in self.cards.drain(..) {
            unsafe {
                DestroyWindow(c.name);
                DestroyWindow(c.remove);
            }
        }
        for (i, row) in rows.into_iter().take(MAX_ROWS).enumerate() {
            let i = i as i32;
            let name = ui::edit(cx.panel, NAME_ID + i, cx.fonts.bold);
            ui::set_text(name, &row.name);
            let remove = widgets::button(cx.panel, "", REMOVE_ID + i, Some(widgets::ICON_DELETE), &cx.fonts, Surface::Row);
            self.cards.push(Card { row, name, remove, rect: RECT::default(), name_frame: RECT::default(), chips: Vec::new(), pill: RECT::default() });
        }
    }
}

impl Cards for MonitorCards {
    fn layout(&mut self, cx: &Ctx, width: i32) -> i32 {
        let s = |v| cx.s(v);
        let mut y = 0;
        for c in &mut self.cards {
            // Right: the status pill, then the delete button.
            let pill_w = s(10 + 8 + 6 + 10) + draw::text_width(pill_text(c.row.connected), cx.fonts.small);
            let button_x = width - s(PAD_X + BUTTON_W);
            let pill_x = button_x - s(10) - pill_w;
            let top = y + s(PAD_Y);
            c.pill = rect(pill_x, top + (s(NAME_H) - s(PILL_H)) / 2, pill_w, s(PILL_H));

            // Middle: the name box (as wide as its text, like the C# TextBox), the details, then
            // the chips, wrapping like a WrapPanel.
            let left = s(PAD_X + ICON + ICON_GAP);
            let right = pill_x - s(12);
            let typed = draw::text_width(&ui::get_text(c.name), cx.fonts.bold) + s(24);
            let name_w = typed.clamp(s(NAME_MIN_W), s(NAME_MAX_W)).min((right - left).max(s(80)));
            c.name_frame = rect(left, top, name_w, s(NAME_H));
            let mut bottom = c.name_frame.bottom + s(4 + LINE_H);
            c.chips.clear();
            if !c.row.chips.is_empty() {
                let (mut x, mut chip_y) = (left, bottom + s(CHIP_GAP));
                for (text, _) in &c.row.chips {
                    let w = draw::text_width(text, cx.fonts.small) + s(20) + 2;
                    if x > left && x + w > right {
                        (x, chip_y) = (left, chip_y + s(CHIP_H + CHIP_GAP));
                    }
                    c.chips.push(rect(x, chip_y, w, s(CHIP_H)));
                    x += w + s(CHIP_GAP);
                }
                bottom = chip_y + s(CHIP_H);
            } else if !c.row.hint.is_empty() {
                bottom += s(6 + LINE_H);
            }
            c.rect = RECT { left: 0, top: y, right: width, bottom: bottom + s(PAD_Y) };
            y = c.rect.bottom + s(GAP);
        }
        (y - s(GAP)).max(0)
    }

    fn place_controls(&mut self, cx: &Ctx, dy: i32) {
        for c in &self.cards {
            let f = c.name_frame.offset(0, -dy);
            ui::place(c.name, f.left + cx.s(8), f.top + (f.height() - cx.s(20)) / 2, f.width() - cx.s(16), cx.s(20));
            ui::place(c.remove, c.rect.right - cx.s(PAD_X + BUTTON_W), f.top, cx.s(BUTTON_W), cx.s(NAME_H));
        }
    }

    fn paint(&self, cx: &Ctx, hdc: HDC, dy: i32) {
        let p = theme::palette();
        let s = |v| cx.s(v);
        let focus = unsafe { GetFocus() };
        for c in &self.cards {
            let up = |r: RECT| r.offset(0, -dy);
            // A monitor that isn't connected is dimmed, like the C# card's 70% opacity.
            let dim = |colour| if c.row.connected { colour } else { theme::mix(colour, p.row, 0.35) };
            let card = up(c.rect);
            draw::round_rect(hdc, card, s(6), p.row, Some(p.row_stroke));

            let name = up(c.name_frame);
            let icon = rect(card.left + s(PAD_X), name.top, s(ICON), s(ICON + 4));
            draw::glyph(hdc, widgets::ICON_MONITOR, icon, cx.fonts.icon_28, dim(p.text), 0);

            widgets::text_box(hdc, name, cx.dpi, c.name == focus);

            let pill = up(c.pill);
            let details = RECT { left: name.left + s(2), top: name.bottom + s(4), right: pill.left - s(12), bottom: name.bottom + s(4 + LINE_H) };
            draw::text(hdc, &c.row.details, details, cx.fonts.small, dim(p.text2), DT_SINGLELINE | DT_END_ELLIPSIS);

            for (r, (text, current)) in c.chips.iter().map(|&r| up(r)).zip(&c.row.chips) {
                let (fill, border, fg) = if *current { (p.accent, None, p.on_accent) } else { (p.chip, Some(p.chip_stroke), dim(p.text)) };
                draw::round_rect(hdc, r, s(CHIP_H / 2), fill, border);
                draw::text(hdc, text, r, cx.fonts.small, fg, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }
            if c.chips.is_empty() {
                let hint = details.offset(0, s(6 + LINE_H));
                draw::text(hdc, &c.row.hint, hint, cx.fonts.small, p.text2, DT_SINGLELINE | DT_END_ELLIPSIS);
            }

            // The status pill: a dot and the text.
            let (bg, dot) = if c.row.connected { (p.success, p.success_text) } else { (p.neutral, p.text_disabled) };
            draw::round_rect(hdc, pill, s(PILL_H / 2), bg, None);
            let dot_rect = rect(pill.left + s(10), pill.top + (pill.height() - s(8)) / 2, s(8), s(8));
            draw::round_rect(hdc, dot_rect, s(4), dot, None);
            let label = RECT { left: pill.left + s(24), ..pill };
            draw::text(hdc, pill_text(c.row.connected), label, cx.fonts.small, p.text, DT_VCENTER | DT_SINGLELINE);
        }
    }

    /// Typing changes how wide the name box is.
    fn on_command(&mut self, _cx: &Ctx, _control: HWND, code: u16) -> bool {
        code == EN_CHANGE
    }

    fn card_of(&self, control: HWND) -> Option<RECT> {
        self.cards.iter().find(|c| c.name == control || c.remove == control).map(|c| c.rect)
    }

    fn fonts_changed(&mut self, cx: &Ctx) {
        for c in &self.cards {
            ui::set_font(c.name, cx.fonts.bold);
            widgets::set_fonts(c.remove, &cx.fonts);
        }
    }

    fn retheme(&mut self, _cx: &Ctx) {
        for c in &self.cards {
            theme::apply_control(c.name, theme::ControlKind::Edit);
        }
    }
}
