//! The main window's controls, where they go, and what's painted behind them: the content layer,
//! the status banner, the two cards and the name box's frame. Sizes are at 96 DPI.

use super::*;
use crate::i18n::{t, tf};
use crate::ui::draw::{self, rect, RectExt};
use crate::ui::lists;
use crate::ui::text_box::TextBox;
use crate::ui::theme::{Severity, Surface};
use std::cell::RefCell;

// The window, at 96 DPI.
const WIDTH: i32 = 1040;
const HEIGHT: i32 = 740;
const MIN_WIDTH: i32 = 960;
const MIN_HEIGHT: i32 = 620;
const STYLE: u32 = WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN;
const EX_STYLE: u32 = WS_EX_CONTROLPARENT;

// Controls.
const BUTTON_H: i32 = 32;
/// A button's width beyond its text: padding, plus an icon and the gap after it.
const BUTTON_EXTRA: i32 = 32;
const ICON_BUTTON_EXTRA: i32 = BUTTON_EXTRA + 16 + 8;
/// A check box's width beyond its label: the box and the gap after it.
const CHECK_EXTRA: i32 = 18 + 8 + 4;
const CHECK_H: i32 = 24;
const LABEL_H: i32 = 20;
/// A two-line item in the configurations list.
const PROFILE_ITEM_H: i32 = 52;

// Areas.
/// The configurations list and the buttons under it: their left edge and width.
const NAV_X: i32 = 12;
const NAV_W: i32 = 236;
const MARGIN: i32 = 24;
const BANNER_Y: i32 = 64;
const BANNER_H: i32 = 46;
/// The Monitors and Hotkeys cards: a title and subtitle, then the cards.
const CARD_HEADER: i32 = 66;
const CARD_PAD: i32 = 16;
/// What the Monitors card leaves for the Hotkeys card, at least.
const HOTKEYS_MIN: i32 = 200;

/// Creates the window, sized for the DPI of the monitor it opens on.
pub(super) fn create_window(class: &str) -> HWND {
    let size = |dpi: u32| {
        let mut r = rect(0, 0, ui::scale(WIDTH, dpi), ui::scale(HEIGHT, dpi));
        unsafe { AdjustWindowRectExForDpi(&mut r, STYLE, 0, EX_STYLE, dpi) };
        (r.width(), r.height())
    };
    let system_dpi = unsafe { GetDpiForSystem() };
    let (w, h) = size(system_dpi);
    let hwnd = unsafe {
        CreateWindowExW(EX_STYLE, wide(class).as_ptr(), wide(ui::app_name()).as_ptr(), STYLE, CW_USEDEFAULT, CW_USEDEFAULT,
            w, h, 0, 0, hinstance(), core::ptr::null())
    };
    // It may have opened on a monitor with another DPI (or the system DPI may be out of date).
    let dpi = ui::dpi_of(hwnd);
    if dpi != system_dpi {
        let (w, h) = size(dpi);
        unsafe { SetWindowPos(hwnd, 0, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) };
    }
    theme::apply_window(hwnd);
    hwnd
}

/// The smallest the window can be made, in pixels for its DPI.
pub(super) fn min_window_size(h: HWND) -> POINT {
    let dpi = ui::dpi_of(h);
    let mut r = rect(0, 0, ui::scale(MIN_WIDTH, dpi), ui::scale(MIN_HEIGHT, dpi));
    unsafe { AdjustWindowRectExForDpi(&mut r, STYLE, 0, EX_STYLE, dpi) };
    POINT { x: r.width(), y: r.height() }
}

/// The height of an item in the configurations list, in pixels.
pub(super) fn profile_item_height(dpi: u32) -> i32 {
    ui::scale(PROFILE_ITEM_H, dpi)
}

/// The window's controls (the cards' own controls belong to the card panels).
pub(super) struct Controls {
    pub configs_label: HWND,
    pub profiles: HWND,
    pub new: HWND,
    pub delete: HWND,
    pub version: HWND,
    pub name_label: HWND,
    pub name: TextBox,
    pub status: HWND,
    pub enabled: HWND,
    pub monitors_label: HWND,
    pub monitors_hint: HWND,
    pub detect: HWND,
    pub add_connected: HWND,
    /// The monitor cards ([`monitor_cards`]).
    pub monitors: HWND,
    pub hotkeys_label: HWND,
    pub hotkeys_hint: HWND,
    pub add_hotkey: HWND,
    /// The hotkey cards ([`hotkey_cards`]).
    pub hotkeys: HWND,
    pub activity: HWND,
    pub saved_note: HWND,
    pub settings: HWND,
    pub close: HWND,
}

impl Controls {
    /// The controls that only make sense with a configuration selected.
    pub fn detail(&self) -> [HWND; 8] {
        [self.delete, self.name.edit, self.enabled, self.monitors, self.detect, self.add_connected, self.hotkeys, self.add_hotkey]
    }

    /// Gives every control its font (on creation, and after a DPI change).
    fn set_fonts(&self, f: &Rc<Fonts>) {
        let fonts = [
            (self.configs_label, f.section), (self.profiles, f.normal), (self.new, f.normal), (self.delete, f.normal), (self.version, f.small),
            (self.name_label, f.normal), (self.status, f.normal), (self.enabled, f.normal),
            (self.monitors_label, f.section), (self.monitors_hint, f.normal), (self.detect, f.normal), (self.add_connected, f.normal),
            (self.hotkeys_label, f.section), (self.hotkeys_hint, f.normal), (self.add_hotkey, f.normal),
            (self.activity, f.small), (self.saved_note, f.small), (self.settings, f.normal), (self.close, f.normal),
        ];
        for (h, font) in fonts {
            ui::set_font(h, font);
            widgets::set_fonts(h, f);
        }
        self.name.set_font(f.title, f);
        ui::card_panel::set_fonts(self.monitors, f);
        ui::card_panel::set_fonts(self.hotkeys, f);
    }

    /// Gives every control its text in the current language (on creation, and when the
    /// language changes).
    pub fn set_texts(&self) {
        let texts = [
            (self.configs_label, t("main.configurations")), (self.new, t("main.new")), (self.delete, t("main.delete")),
            (self.name_label, t("main.name")), (self.enabled, t("main.hotkeys_on")),
            (self.monitors_label, t("main.monitors")), (self.monitors_hint, t("main.monitors_hint")),
            (self.detect, t("main.detect")), (self.add_connected, t("main.add_connected")),
            (self.hotkeys_label, t("main.hotkeys")), (self.hotkeys_hint, t("main.hotkeys_hint")), (self.add_hotkey, t("main.add_hotkey")),
            (self.saved_note, t("main.saved_automatically")), (self.settings, t("main.settings")), (self.close, t("common.close")),
        ];
        for (h, text) in texts {
            ui::set_text(h, text);
        }
        ui::set_text(self.version, &tf("main.version", &[("version", &sys::display_version())]));
    }
}

/// Creates the controls in the order they're laid out (left to right, top to bottom), which is
/// the order Tab moves through them.
pub(super) fn create_controls(h: HWND, f: &Rc<Fonts>) -> Controls {
    use Surface::{Card, Layer, Window};
    let banner = Surface::InfoBar(Severity::Informational);
    let n = f.normal;
    let label = |surface, secondary| ui::label(h, "", n, surface, secondary);
    let c = Controls {
        configs_label: label(Window, false),
        // Two-line items (name, then status), drawn by draw_profile.
        profiles: {
            let list = ui::child(h, "LISTBOX", "", LBS_NOTIFY | LBS_NOINTEGRALHEIGHT | LBS_OWNERDRAWFIXED | LBS_HASSTRINGS | WS_VSCROLL | WS_TABSTOP, 0, ID_PROFILES, n);
            theme::set_surface(list, Window, false);
            theme::apply_control(list, theme::ControlKind::ListBox);
            list
        },
        new: widgets::button(h, "", ID_NEW, Some(widgets::ICON_ADD), f, Window),
        delete: widgets::button(h, "", ID_DELETE, Some(widgets::ICON_DELETE), f, Window),
        version: label(Window, true),
        name_label: label(Layer, true),
        name: TextBox::create(h, ID_NAME, f.title, f),
        status: label(banner, false),
        enabled: widgets::checkbox(h, "", ID_ENABLED, f, banner),
        monitors_label: label(Card, false),
        monitors_hint: label(Card, true),
        detect: widgets::button(h, "", ID_DETECT, Some(widgets::ICON_REFRESH), f, Card),
        add_connected: widgets::button(h, "", ID_ADD_CONNECTED, Some(widgets::ICON_ADD), f, Card),
        monitors: monitor_cards::create(h, ID_MONITORS, f),
        hotkeys_label: label(Card, false),
        hotkeys_hint: label(Card, true),
        add_hotkey: widgets::accent_button(h, "", ID_ADD_HOTKEY, Some(widgets::ICON_ADD), f, Card),
        hotkeys: hotkey_cards::create(h, ID_HOTKEYS, f),
        activity: label(Layer, true),
        saved_note: {
            let note = ui::child(h, "STATIC", "", SS_NOPREFIX | SS_RIGHT, 0, 0, n);
            theme::set_surface(note, Layer, true);
            note
        },
        settings: widgets::button(h, "", ID_SETTINGS, Some(widgets::ICON_SETTINGS), f, Layer),
        close: widgets::button(h, "", IDCANCEL, None, f, Layer),
    };
    c.set_fonts(f);
    c.set_texts();
    c
}

/// Areas painted behind the controls (in physical pixels).
#[derive(Clone, Copy, Default, PartialEq)]
pub(super) struct Regions {
    layer: RECT,
    pub banner: RECT,
    cards: [RECT; 2],
    name_frame: RECT,
}

impl MainWindow {
    /// Places every control for the window's size. Positions are worked out at 96 DPI.
    pub(super) fn layout(&mut self) {
        let rc = draw::client_rect(self.hwnd);
        let d = self.dpi as i32;
        let (w, h) = (rc.right * 96 / d, rc.bottom * 96 / d);
        self.layout_nav(h);

        // Right: the selected configuration, on the content layer.
        let layer_x = NAV_X + NAV_W + 12;
        let (left, right) = (layer_x + MARGIN, w - MARGIN);
        let layer = self.scaled_rect(layer_x, 0, w - layer_x, h);
        let name_frame = self.layout_name(left, right);
        let banner = self.layout_banner(left, right);
        let bottom = self.layout_footer(left, right, h);
        let cards = self.layout_cards(left, right, BANNER_Y + BANNER_H + 16, bottom - 40);

        // Repaint only if something moved (all children too: moved controls otherwise keep stale
        // pixels copied from where they were). Repainting when nothing moved makes everything flicker.
        let regions = Regions { layer, banner, cards, name_frame };
        if regions != self.regions {
            self.regions = regions;
            self.share_background();
            unsafe { RedrawWindow(self.hwnd, core::ptr::null(), 0, RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN) };
        }
    }

    /// Left: the configurations, on the window background.
    fn layout_nav(&self, h: i32) {
        let c = &self.c;
        let label_x = 16;
        self.place(c.configs_label, label_x, 16, NAV_W, 26);
        let version_y = h - 14 - 18;
        let delete_y = version_y - 10 - BUTTON_H;
        let new_y = delete_y - 6 - BUTTON_H;
        self.place(c.profiles, NAV_X, 50, NAV_W, new_y - 10 - 50);
        self.place(c.new, NAV_X, new_y, NAV_W, BUTTON_H);
        self.place(c.delete, NAV_X, delete_y, NAV_W, BUTTON_H);
        self.place(c.version, label_x, version_y, NAV_W, 18);
    }

    /// The name box along the top, with its label. Returns the box's frame.
    fn layout_name(&self, left: i32, right: i32) -> RECT {
        let label_w = self.text_width(self.c.name_label, 0, 40);
        self.place(self.c.name_label, left, 25, label_w, LABEL_H);
        let frame = self.scaled_rect(left + label_w + 4, 12, right - left - label_w - 4, 44);
        self.c.name.place(frame, self.dpi);
        frame
    }

    /// The status banner: an icon and the status on the left, "Hotkeys on" on the right.
    fn layout_banner(&self, left: i32, right: i32) -> RECT {
        let enabled_w = self.text_width(self.c.enabled, CHECK_EXTRA, 130);
        let status_x = left + 16 + 28;
        self.place(self.c.status, status_x, BANNER_Y + 13, right - 16 - enabled_w - 4 - status_x, LABEL_H);
        self.place(self.c.enabled, right - 16 - enabled_w, BANNER_Y + (BANNER_H - CHECK_H) / 2, enabled_w, CHECK_H);
        self.scaled_rect(left, BANNER_Y, right - left, BANNER_H)
    }

    /// The footer: what's happening, then the options and Close. Returns where the buttons start.
    fn layout_footer(&self, left: i32, right: i32, h: i32) -> i32 {
        let c = &self.c;
        let bottom = h - 16 - BUTTON_H;
        let note_w = self.text_width(c.saved_note, 4, 0);
        self.place(c.activity, left, bottom - 28, right - left - note_w - 10, LABEL_H);
        self.place(c.saved_note, right - note_w, bottom - 28, note_w, LABEL_H);
        self.place(c.settings, left, bottom, self.text_width(c.settings, ICON_BUTTON_EXTRA, 120), BUTTON_H);
        let close_w = self.text_width(c.close, BUTTON_EXTRA, 100);
        self.place(c.close, right - close_w, bottom, close_w, BUTTON_H);
        bottom
    }

    /// The two cards between `top` and `end`, each with a title, a subtitle and buttons on the
    /// right, then its cards. Each is as tall as its cards need, beyond which they scroll;
    /// Monitors leaves room for Hotkeys. Returns the two cards' rectangles.
    fn layout_cards(&self, left: i32, right: i32, top: i32, end: i32) -> [RECT; 2] {
        let c = &self.c;
        let width = right - left;
        let gap = 16;
        let (cx, cw) = (left + CARD_PAD, width - 2 * CARD_PAD);
        // The cards' height depends on the width, so that's set first, keeping the panel's
        // height (resizing it when nothing has changed would repaint every card).
        let card_height = |panel: HWND, y: i32, max: i32| {
            let sc = |v: i32| ui::scale(v, self.dpi);
            ui::place(panel, sc(cx), sc(y + CARD_HEADER), sc(cw), window_height(panel));
            (CARD_HEADER + card_panel_height(panel) + CARD_PAD).min(max)
        };
        // A card's title and subtitle, with its buttons (rightmost last) along the right.
        let header = |y: i32, title: HWND, subtitle: HWND, buttons: &[(HWND, i32)]| {
            let buttons_w: i32 = buttons.iter().map(|(_, bw)| bw + 8).sum();
            self.place(title, cx, y + 12, cw - buttons_w - 10, 26);
            self.place(subtitle, cx, y + 40, cw - buttons_w - 10, LABEL_H);
            let mut x = cx + cw - buttons_w + 8;
            for &(button, bw) in buttons {
                self.place(button, x, y + 16, bw, BUTTON_H);
                x += bw + 8;
            }
        };

        let monitors_h = card_height(c.monitors, top, end - top - gap - HOTKEYS_MIN).max(CARD_HEADER + 60);
        let button_w = |b: HWND| self.text_width(b, ICON_BUTTON_EXTRA, 110);
        header(top, c.monitors_label, c.monitors_hint, &[(c.detect, button_w(c.detect)), (c.add_connected, button_w(c.add_connected))]);
        self.place(c.monitors, cx, top + CARD_HEADER, cw, monitors_h - CARD_HEADER - CARD_PAD);

        let hy = top + monitors_h + gap;
        let hotkeys_h = card_height(c.hotkeys, hy, end - hy).max(CARD_HEADER + 50);
        header(hy, c.hotkeys_label, c.hotkeys_hint, &[(c.add_hotkey, button_w(c.add_hotkey))]);
        self.place(c.hotkeys, cx, hy + CARD_HEADER, cw, hotkeys_h - CARD_HEADER - CARD_PAD);

        [self.scaled_rect(left, top, width, monitors_h), self.scaled_rect(left, hy, width, hotkeys_h)]
    }

    /// Places a control, given its position and size at 96 DPI.
    fn place(&self, control: HWND, x: i32, y: i32, w: i32, h: i32) {
        let s = |v: i32| ui::scale(v, self.dpi);
        ui::place(control, s(x), s(y), s(w), s(h));
    }

    /// How wide `control` needs to be for its text, at 96 DPI: the text, plus `extra` (padding,
    /// a check box or an icon), and at least `min`. Translations can be longer than English.
    fn text_width(&self, control: HWND, extra: i32, min: i32) -> i32 {
        let font = unsafe { SendMessageW(control, WM_GETFONT, 0, 0) } as HFONT;
        let text = draw::text_width(&ui::get_text(control), font) * 96 / self.dpi as i32;
        (text + extra + 1).max(min)
    }

    /// A rectangle given at 96 DPI, in pixels.
    fn scaled_rect(&self, x: i32, y: i32, w: i32, h: i32) -> RECT {
        let s = |v: i32| ui::scale(v, self.dpi);
        rect(s(x), s(y), s(w), s(h))
    }

    /// Updates what the background is painted from (after the layout, the status or the fonts change).
    pub(super) fn share_background(&self) {
        let background = Background { regions: self.regions, status: self.status, dpi: self.dpi, fonts: self.fonts.clone(), name: self.c.name };
        BACKGROUND.with(|b| *b.borrow_mut() = Some(background));
    }

    /// The name box's frame shows whether it has focus.
    pub(super) fn repaint_name_frame(&self) {
        let r = self.regions.name_frame;
        unsafe { RedrawWindow(self.hwnd, &r, 0, RDW_INVALIDATE | RDW_ERASE) };
    }

    /// Re-applies the palette after Windows switches between light and dark (or the accent changes).
    pub(super) fn retheme(&mut self) {
        theme::apply_window(self.hwnd);
        ui::card_panel::retheme(self.c.monitors);
        ui::card_panel::retheme(self.c.hotkeys);
        theme::apply_control(self.c.profiles, theme::ControlKind::ListBox);
        theme::apply_control(self.c.name.edit, theme::ControlKind::Edit);
        self.update_status();
        theme::repaint(self.hwnd);
    }

    /// The window moved to a monitor with another DPI: new fonts, then Windows' suggested size.
    pub(super) fn dpi_changed(&mut self, dpi: u32, suggested: RECT) {
        self.dpi = dpi;
        self.fonts = Rc::new(Fonts::new(dpi)); // the old ones go once nothing uses them
        set_list_fonts(&self.fonts);
        self.c.set_fonts(&self.fonts);
        unsafe { SendMessageW(self.c.profiles, LB_SETITEMHEIGHT, 0, profile_item_height(dpi) as LPARAM) };
        ui::place_rect(self.hwnd, suggested);
        self.layout();
        self.share_background(); // the new fonts, even if nothing moved
    }
}

// ------------------------------------------------------------------------------------------ background

/// What the window's background is painted from. Kept apart from the window's state (like the
/// list's fonts) so the window can repaint while that's lent out: while a dialog opened from it
/// is showing, or a setting changed in one is being applied.
pub(super) struct Background {
    regions: Regions,
    status: Severity,
    dpi: u32,
    fonts: Rc<Fonts>,
    name: TextBox,
}

thread_local! {
    static BACKGROUND: RefCell<Option<Background>> = const { RefCell::new(None) };
}

/// Forgets the background (the window is closing).
pub(super) fn forget_background() {
    BACKGROUND.with(|b| b.borrow_mut().take());
}

/// Paints what's behind the controls: the content layer, the cards, the banner and the name
/// box's frame.
pub(super) fn paint_background(hwnd: HWND, hdc: HDC) {
    let p = theme::palette();
    draw::fill(hdc, draw::client_rect(hwnd), p.window);
    BACKGROUND.with(|b| {
        let Some(b) = &*b.borrow() else { return };
        let g = &b.regions;
        let s = |v| ui::scale(v, b.dpi);
        // Only the layer's top-left corner is rounded: it runs past the window's other edges.
        let radius = s(8);
        let layer = RECT { right: g.layer.right + radius * 2, bottom: g.layer.bottom + radius * 2, ..g.layer };
        draw::round_rect(hdc, layer, radius, p.layer, Some(p.stroke));
        for card in g.cards {
            draw::round_rect(hdc, card, radius, p.card, Some(p.stroke));
        }
        draw::round_rect(hdc, g.banner, s(6), theme::color_of(Surface::InfoBar(b.status)), None);
        let (glyph, colour) = match b.status {
            Severity::Success => (widgets::ICON_TICK, p.success_text),
            Severity::Warning => (widgets::ICON_WARNING, p.caution_text),
            Severity::Informational => (widgets::ICON_INFO, p.text2),
        };
        let icon = RECT { left: g.banner.left + s(16), right: g.banner.left + s(16 + 20), ..g.banner };
        draw::glyph(hdc, glyph, icon, b.fonts.icon_16, colour, DT_VCENTER);
        b.name.paint(hdc, g.name_frame, b.dpi);
    });
}

// ------------------------------------------------------------------------------------------ configurations list

thread_local! {
    /// The fonts the configurations list is drawn with. Kept apart from the window's state so the
    /// list can repaint even while that's lent out (a dialog is open over it).
    static LIST_FONTS: RefCell<Option<Rc<Fonts>>> = const { RefCell::new(None) };
}

/// Sets the fonts the configurations list is drawn with.
pub(super) fn set_list_fonts(fonts: &Rc<Fonts>) {
    LIST_FONTS.with(|f| *f.borrow_mut() = Some(fonts.clone()));
}

/// Draws one configuration in the list: an icon, the name, and its status underneath (the
/// item's text is "Name\tStatus").
pub(super) fn draw_profile(dis: &DRAWITEMSTRUCT) {
    let Some(fonts) = LIST_FONTS.with(|f| f.borrow().clone()) else { return };
    if dis.itemID == u32::MAX {
        return; // an empty list being focused
    }
    let p = theme::palette();
    let (hdc, r) = (dis.hDC, dis.rcItem);
    let s = |v| ui::scale(v, fonts.dpi);
    let text = lists::lb_text(dis.hwndItem, dis.itemID as usize);
    let (title, subtitle) = text.split_once('\t').unwrap_or((&text, ""));
    let connected = subtitle.starts_with(t("common.connected"));

    draw::fill(hdc, r, p.window);
    let item = r.inset(s(2), s(2));
    let mid = (item.top + item.bottom) / 2;
    if dis.itemState & ODS_SELECTED != 0 {
        draw::round_rect(hdc, item, s(5), p.selected, None);
        draw::round_rect(hdc, rect(item.left, mid - s(8), s(3), s(16)), s(2), p.accent, None); // the accent bar
    }
    let icon = RECT { left: item.left + s(12), right: item.left + s(36), ..item };
    draw::glyph(hdc, widgets::ICON_MONITOR, icon, fonts.icon_18, if connected { p.accent } else { p.text2 }, DT_VCENTER);
    let (left, right) = (item.left + s(40), item.right - s(8));
    draw::text(hdc, title, RECT { left, top: item.top + s(4), right, bottom: mid + s(1) }, fonts.normal, p.text, DT_SINGLELINE | DT_END_ELLIPSIS | DT_BOTTOM);
    draw::text(hdc, subtitle, RECT { left, top: mid + s(1), right, bottom: item.bottom - s(4) }, fonts.small, p.text2, DT_SINGLELINE | DT_END_ELLIPSIS);
}

// ------------------------------------------------------------------------------------------ helpers

/// A window's current height, in pixels.
fn window_height(h: HWND) -> i32 {
    let mut r = RECT::default();
    unsafe { GetWindowRect(h, &mut r) };
    r.height()
}

/// A card panel's height at 96 DPI, at least enough for one line.
fn card_panel_height(panel: HWND) -> i32 {
    ui::card_panel::content_height(panel).max(40)
}
