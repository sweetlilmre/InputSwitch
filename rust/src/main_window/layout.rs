//! The main window's controls, where they go, and what's painted behind them: the content layer,
//! the status banner, the two cards and the name box's frame. Positions are at 96 DPI.

use super::*;
use crate::ui::draw::{self, rect, RectExt};
use crate::ui::lists;
use crate::ui::theme::Surface;

/// Client size at 96 DPI (the C# window's).
const WIDTH: i32 = 1040;
const HEIGHT: i32 = 740;
const MIN_WIDTH: i32 = 960;
const MIN_HEIGHT: i32 = 620;
const STYLE: u32 = WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN;
const EX_STYLE: u32 = WS_EX_CONTROLPARENT;

/// A two-line item in the configurations list, at 96 DPI.
const PROFILE_ITEM_HEIGHT: i32 = 52;
/// The Monitors and Hotkeys cards: title and subtitle, then the cards.
const CARD_HEADER: i32 = 66;
const CARD_PAD: i32 = 16;
/// What the Monitors card leaves for the Hotkeys card, at least.
const HOTKEYS_MIN: i32 = 200;

pub(super) fn create_window(class: &str) -> HWND {
    let dpi = unsafe { GetDpiForSystem() };
    let mut r = rect(0, 0, ui::scale(WIDTH, dpi), ui::scale(HEIGHT, dpi));
    unsafe {
        AdjustWindowRectExForDpi(&mut r, STYLE, 0, EX_STYLE, dpi);
        let hwnd = CreateWindowExW(EX_STYLE, wide(class).as_ptr(), wide(ui::TITLE).as_ptr(), STYLE, CW_USEDEFAULT, CW_USEDEFAULT,
            r.width(), r.height(), 0, 0, hinstance(), core::ptr::null());
        theme::apply_window(hwnd);
        hwnd
    }
}

pub(super) fn min_window_size(h: HWND) -> POINT {
    let dpi = unsafe { GetDpiForWindow(h) }.max(96);
    let mut r = rect(0, 0, ui::scale(MIN_WIDTH, dpi), ui::scale(MIN_HEIGHT, dpi));
    unsafe { AdjustWindowRectExForDpi(&mut r, STYLE, 0, EX_STYLE, dpi) };
    POINT { x: r.width(), y: r.height() }
}

pub(super) fn profile_item_height(dpi: u32) -> i32 {
    ui::scale(PROFILE_ITEM_HEIGHT, dpi.max(96))
}

pub(super) struct Controls {
    pub configs_label: HWND,
    pub profiles: HWND,
    pub new: HWND,
    pub delete: HWND,
    pub version: HWND,
    pub name_label: HWND,
    pub name: HWND,
    pub enabled: HWND,
    pub status: HWND,
    pub monitors_label: HWND,
    pub monitors_hint: HWND,
    /// The monitor cards ([`monitor_cards`]).
    pub monitors: HWND,
    pub detect: HWND,
    pub add_connected: HWND,
    pub hotkeys_label: HWND,
    pub hotkeys_hint: HWND,
    /// The hotkey cards ([`hotkey_cards`]).
    pub hotkeys: HWND,
    pub add_hotkey: HWND,
    pub activity: HWND,
    pub startup: HWND,
    pub notify: HWND,
    pub dark: HWND,
    pub saved_note: HWND,
    pub close: HWND,
}

impl Controls {
    /// The controls that only make sense with a configuration selected.
    pub fn detail(&self) -> [HWND; 8] {
        [self.delete, self.name, self.enabled, self.monitors, self.detect, self.add_connected, self.hotkeys, self.add_hotkey]
    }

    /// Every control, with the font it uses (for DPI changes).
    fn fonts(&self, f: &Fonts) -> [(HWND, HFONT); 22] {
        let c = self;
        [(c.configs_label, f.section), (c.profiles, f.normal), (c.new, f.normal), (c.delete, f.normal), (c.version, f.small),
            (c.name_label, f.normal), (c.name, f.title), (c.enabled, f.normal), (c.status, f.normal),
            (c.monitors_label, f.section), (c.monitors_hint, f.normal), (c.detect, f.normal), (c.add_connected, f.normal),
            (c.hotkeys_label, f.section), (c.hotkeys_hint, f.normal), (c.add_hotkey, f.normal), (c.activity, f.small),
            (c.startup, f.normal), (c.notify, f.normal), (c.dark, f.normal), (c.saved_note, f.small), (c.close, f.normal)]
    }
}

pub(super) fn create_controls(h: HWND, f: &Fonts, dpi: u32) -> Controls {
    use Surface::{Banner, Card, Layer, Window};
    let n = f.normal;
    // The name box has no border of its own: a rounded frame is painted behind it.
    let name = ui::edit(h, ID_NAME, f.title);
    let margin = ui::scale(8, dpi) as LPARAM;
    unsafe { SendMessageW(name, EM_SETMARGINS, EC_LEFTMARGIN | EC_RIGHTMARGIN, margin | (margin << 16)) };
    // Two-line items (name, then status), drawn by draw_profile.
    let profiles = ui::child(h, "LISTBOX", "", LBS_NOTIFY | LBS_NOINTEGRALHEIGHT | LBS_OWNERDRAWFIXED | LBS_HASSTRINGS | WS_VSCROLL | WS_TABSTOP, 0, ID_PROFILES, n);
    theme::set_surface(profiles, Window, false);
    theme::apply_control(profiles, theme::ControlKind::ListBox);
    let saved_note = ui::child(h, "STATIC", "Changes are saved automatically", SS_NOPREFIX | SS_RIGHT, 0, 0, f.small);
    theme::set_surface(saved_note, Layer, true);
    Controls {
        configs_label: ui::label(h, "Configurations", f.section, Window, false),
        profiles,
        new: widgets::button(h, "New from current setup", ID_NEW, Some(widgets::ICON_ADD), f, Window),
        delete: widgets::button(h, "Delete configuration", ID_DELETE, Some(widgets::ICON_DELETE), f, Window),
        version: ui::label(h, &format!("Version {}", sys::display_version()), f.small, Window, true),
        name_label: ui::label(h, "Name:", n, Layer, true),
        name,
        enabled: widgets::checkbox(h, "Hotkeys on", ID_ENABLED, f, Banner),
        status: ui::label(h, "", n, Banner, false),
        monitors_label: ui::label(h, "Monitors", f.section, Card, false),
        monitors_hint: ui::label(h, "This configuration is used whenever these monitors are connected.", n, Card, true),
        monitors: monitor_cards::create(h, ID_MONITORS, dpi),
        detect: widgets::button(h, "Detect inputs", ID_DETECT, Some(widgets::ICON_REFRESH), f, Card),
        add_connected: widgets::button(h, "Add connected", ID_ADD_CONNECTED, Some(widgets::ICON_ADD), f, Card),
        hotkeys_label: ui::label(h, "Hotkeys", f.section, Card, false),
        hotkeys_hint: ui::label(h, "Switch to one input, toggle between two, or cycle through several.", n, Card, true),
        hotkeys: hotkey_cards::create(h, ID_HOTKEYS, dpi),
        add_hotkey: widgets::accent_button(h, "Add hotkey", ID_ADD_HOTKEY, Some(widgets::ICON_ADD), f, Card),
        activity: ui::label(h, "", f.small, Layer, true),
        startup: widgets::checkbox(h, "Start with Windows", ID_STARTUP, f, Layer),
        notify: widgets::checkbox(h, "Notify when the setup changes", ID_NOTIFY, f, Layer),
        dark: widgets::switch(h, "Dark mode", ID_DARK, f, Layer),
        saved_note,
        close: widgets::button(h, "Close", IDCANCEL, None, f, Layer),
    }
}

/// Areas painted behind the controls (in physical pixels).
#[derive(Default, PartialEq)]
pub(super) struct Regions {
    layer: RECT,
    pub banner: RECT,
    cards: [RECT; 2],
    name_frame: RECT,
}

/// The status banner's icon.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum StatusIcon {
    /// Connected, hotkeys on.
    Tick,
    /// Connected, hotkeys off.
    Warning,
    /// Not connected.
    Info,
}

impl MainWindow {
    pub(super) fn layout(&mut self) {
        let rc = draw::client_rect(self.hwnd);
        let d = self.dpi.max(96) as i32;
        let (w, h) = (rc.right * 96 / d, rc.bottom * 96 / d);
        let sc = |v: i32| ui::scale(v, self.dpi);
        let p = |hw: HWND, x: i32, y: i32, cw: i32, ch: i32| ui::place(hw, sc(x), sc(y), sc(cw), sc(ch));
        let r = |x: i32, y: i32, cw: i32, ch: i32| rect(sc(x), sc(y), sc(cw), sc(ch));
        let c = &self.c;

        // Left: the configurations, on the window background.
        let lw = 236;
        p(c.configs_label, 16, 16, lw, 26);
        let version_y = h - 14 - 18;
        let delete_y = version_y - 10 - 32;
        let new_y = delete_y - 6 - 32;
        p(c.profiles, 12, 50, lw, new_y - 10 - 50);
        p(c.new, 12, new_y, lw, 32);
        p(c.delete, 12, delete_y, lw, 32);
        p(c.version, 16, version_y, lw, 18);

        // Right: the selected configuration, on the content layer.
        let x0 = 12 + lw + 12;
        let (rx, re) = (x0 + 24, w - 24);
        let rw = re - rx;
        let layer = r(x0, 0, w - x0, h);

        p(c.name_label, rx, 25, 48, 20);
        let name_frame = r(rx + 52, 14, rw - 52, 40);
        p(c.name, rx + 53, 19, rw - 54, 30);

        // The status banner: the icon and status on the left, "Hotkeys on" on the right.
        let banner = r(rx, 64, rw, 46);
        p(c.status, rx + 16 + 28, 64 + 13, rw - 32 - 28 - 150, 20);
        p(c.enabled, re - 16 - 130, 64 + 11, 130, 24);

        // The footer.
        let bottom = h - 16 - 32;
        p(c.close, re - 100, bottom, 100, 32);
        p(c.startup, rx, bottom + 4, 160, 24);
        p(c.notify, rx + 166, bottom + 4, 230, 24);
        p(c.dark, rx + 412, bottom + 4, 130, 24);
        p(c.activity, rx, bottom - 28, rw - 230, 20);
        p(c.saved_note, re - 220, bottom - 28, 220, 20);

        // The two cards, each with a title, a subtitle and buttons on the right (as in the C#
        // app), then its cards. Each is as tall as its cards need (the cards' height depends on
        // the width, so that's set first, keeping the panel's height: resizing it when nothing
        // changed would repaint every card); beyond that they scroll. Monitors leaves room for Hotkeys.
        let (top, end, gap) = (126, bottom - 40, 16);
        let (cx, cw) = (rx + CARD_PAD, rw - 2 * CARD_PAD);
        let card_height = |panel: HWND, y: i32, max: i32| {
            ui::place(panel, sc(cx), sc(y + CARD_HEADER), sc(cw), window_height(panel));
            (CARD_HEADER + card_panel_height(panel) + CARD_PAD).min(max)
        };

        let monitors_h = card_height(c.monitors, top, end - top - gap - HOTKEYS_MIN).max(CARD_HEADER + 60);
        p(c.monitors_label, cx, top + 12, cw - 310, 26);
        p(c.monitors_hint, cx, top + 40, cw - 310, 20);
        p(c.detect, cx + cw - 150 - 8 - 140, top + 16, 140, 32);
        p(c.add_connected, cx + cw - 150, top + 16, 150, 32);
        p(c.monitors, cx, top + CARD_HEADER, cw, monitors_h - CARD_HEADER - CARD_PAD);

        let hy = top + monitors_h + gap;
        let hotkeys_h = card_height(c.hotkeys, hy, end - hy).max(CARD_HEADER + 50);
        p(c.hotkeys_label, cx, hy + 12, cw - 160, 26);
        p(c.hotkeys_hint, cx, hy + 40, cw - 160, 20);
        p(c.add_hotkey, cx + cw - 130, hy + 16, 130, 32);
        p(c.hotkeys, cx, hy + CARD_HEADER, cw, hotkeys_h - CARD_HEADER - CARD_PAD);

        // Repaint only if something moved (all children too: moved controls otherwise keep stale
        // pixels copied from where they were). Repainting when nothing moved makes everything flicker.
        let regions = Regions { layer, banner, cards: [r(rx, top, rw, monitors_h), r(rx, hy, rw, hotkeys_h)], name_frame };
        if regions != self.regions {
            self.regions = regions;
            unsafe { RedrawWindow(self.hwnd, core::ptr::null(), 0, RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN) };
        }
    }

    pub(super) fn paint_background(&self, hdc: HDC) {
        let p = theme::palette();
        let g = &self.regions;
        let s = |v| ui::scale(v, self.dpi);
        draw::fill(hdc, draw::client_rect(self.hwnd), p.window);
        // Only the layer's top-left corner is rounded: it runs past the window's other edges.
        let radius = s(8);
        let layer = RECT { right: g.layer.right + radius * 2, bottom: g.layer.bottom + radius * 2, ..g.layer };
        draw::round_rect(hdc, layer, radius, p.layer, Some(p.stroke));
        for card in g.cards {
            draw::round_rect(hdc, card, radius, p.card, Some(p.stroke));
        }
        draw::round_rect(hdc, g.banner, s(6), theme::color_of(Surface::Banner), None);
        let (glyph, colour) = match self.status_icon {
            StatusIcon::Tick => (widgets::ICON_TICK, p.success_text),
            StatusIcon::Warning => (widgets::ICON_WARNING, p.caution_text),
            StatusIcon::Info => (widgets::ICON_INFO, p.text2),
        };
        let icon = RECT { left: g.banner.left + s(16), right: g.banner.left + s(16 + 20), ..g.banner };
        draw::glyph(hdc, glyph, icon, self.fonts.icon_16, colour, DT_VCENTER);
        widgets::text_box(hdc, g.name_frame, self.dpi, unsafe { GetFocus() } == self.c.name);
    }

    /// The name box's frame shows whether it has focus.
    pub(super) fn repaint_name_frame(&self) {
        let r = self.regions.name_frame;
        unsafe { RedrawWindow(self.hwnd, &r, 0, RDW_INVALIDATE | RDW_ERASE) };
    }

    /// Draws one configuration in the list: an icon, the name, and its status underneath.
    pub(super) fn draw_profile(&self, dis: &DRAWITEMSTRUCT) {
        if dis.itemID == u32::MAX {
            return; // an empty list being focused
        }
        let p = theme::palette();
        let (hdc, r) = (dis.hDC, dis.rcItem);
        let s = |v| ui::scale(v, self.dpi);
        let text = lists::lb_text(self.c.profiles, dis.itemID as usize);
        let (title, subtitle) = text.split_once('\t').unwrap_or((&text, ""));
        let connected = subtitle.starts_with(profiles::CONNECTED);

        draw::fill(hdc, r, p.window);
        let pill = r.inset(s(2), s(2));
        if dis.itemState & ODS_SELECTED != 0 {
            draw::round_rect(hdc, pill, s(5), p.selected, None);
            let mid = (pill.top + pill.bottom) / 2;
            draw::round_rect(hdc, rect(pill.left, mid - s(8), s(3), s(16)), s(2), p.accent, None);
        }
        let icon = RECT { left: pill.left + s(12), right: pill.left + s(36), ..pill };
        draw::glyph(hdc, widgets::ICON_MONITOR, icon, self.fonts.icon_18, if connected { p.accent } else { p.text2 }, DT_VCENTER);
        let (left, right, mid) = (pill.left + s(40), pill.right - s(8), (pill.top + pill.bottom) / 2);
        draw::text(hdc, title, RECT { left, top: pill.top + s(4), right, bottom: mid + s(1) }, self.fonts.normal, p.text, DT_SINGLELINE | DT_END_ELLIPSIS | DT_BOTTOM);
        draw::text(hdc, subtitle, RECT { left, top: mid + s(1), right, bottom: pill.bottom - s(4) }, self.fonts.small, p.text2, DT_SINGLELINE | DT_END_ELLIPSIS);
    }

    /// Re-applies the palette after Windows switches between light and dark (or the accent changes).
    pub(super) fn retheme(&mut self) {
        widgets::set_checked(self.c.dark, theme::palette().dark);
        theme::apply_window(self.hwnd);
        ui::card_panel::retheme(self.c.monitors);
        ui::card_panel::retheme(self.c.hotkeys);
        theme::apply_control(self.c.profiles, theme::ControlKind::ListBox);
        theme::apply_control(self.c.name, theme::ControlKind::Edit);
        self.update_status();
        theme::repaint(self.hwnd);
    }

    pub(super) fn dpi_changed(&mut self, dpi: u32, suggested: RECT) {
        self.dpi = dpi;
        let fonts = Fonts::new(dpi);
        for (h, font) in self.c.fonts(&fonts) {
            ui::set_font(h, font);
            widgets::set_fonts(h, &fonts);
        }
        ui::card_panel::set_dpi(self.c.monitors, dpi);
        ui::card_panel::set_dpi(self.c.hotkeys, dpi);
        unsafe { SendMessageW(self.c.profiles, LB_SETITEMHEIGHT, 0, profile_item_height(dpi) as LPARAM) };
        self.fonts = fonts; // the old ones are deleted now that nothing uses them
        ui::place_rect(self.hwnd, suggested);
        self.layout();
    }
}

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
