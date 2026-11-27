//! The main window: configurations on the left; the selected one's monitors and hotkeys on the
//! right. Every change is saved and applied straight away: the window posts a copy of the
//! configuration to the tray app, which saves it and re-registers the hotkeys. Typed names are
//! committed shortly after typing stops, and on close.
//!
//! State is borrowed from a thread-local RefCell for each message. Messages that arrive while it
//! is already borrowed (from our own programmatic control changes, or while a dialog is open) skip
//! our handling, which is exactly the "don't react to our own updates" behaviour we want.

use crate::config::{AppConfig, Binding, ProfileMonitor};
use crate::ffi::*;
use crate::hotkey_list;
use crate::monitor_list;
use crate::monitors::{self, MonitorInfo};
use crate::ui::{self, Fonts};
use crate::{editor, matcher, sys, theme, tray, widgets};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;

const CLASS: &str = "InputSwitch.Settings";
const WM_POKE: u32 = WM_APP + 2;
/// Commits typed names once typing pauses, rather than on every keystroke.
const TIMER_TYPING: usize = 1;
const TYPING_MS: u32 = 700;

const ID_PROFILES: i32 = 100;
const ID_NEW: i32 = 101;
const ID_DELETE: i32 = 102;
const ID_NAME: i32 = 103;
const ID_ENABLED: i32 = 104;
const ID_MONITORS: i32 = 105;
const ID_DETECT: i32 = 107;
const ID_ADD_CONNECTED: i32 = 108;
const ID_HOTKEYS: i32 = 110;
const ID_ADD_HOTKEY: i32 = 111;
const ID_STARTUP: i32 = 115;
const ID_NOTIFY: i32 = 116;
const ID_DARK: i32 = 117;

/// Client size at 96 DPI.
const WIDTH: i32 = 1040;
const HEIGHT: i32 = 740;
const MIN_WIDTH: i32 = 960;
const MIN_HEIGHT: i32 = 620;
/// Height of a two-line item in the configurations list, at 96 DPI.
const PROFILE_ITEM_HEIGHT: i32 = 52;

enum Bg {
    Detected { id: String, inputs: Vec<u8>, current: Option<u8> },
    DetectDone,
    Tested { id: String, result: Result<u8, String> },
}

static EVENTS: Mutex<Vec<Bg>> = Mutex::new(Vec::new());
static HWND_SETTINGS: AtomicIsize = AtomicIsize::new(0);

fn post_bg(ev: Bg) {
    EVENTS.lock().unwrap_or_else(|e| e.into_inner()).push(ev);
    let h = HWND_SETTINGS.load(Ordering::Relaxed);
    if h != 0 {
        unsafe { PostMessageW(h, WM_POKE, 0, 0) };
    }
}

pub fn hwnd() -> HWND {
    HWND_SETTINGS.load(Ordering::Relaxed)
}

struct Controls {
    configs_label: HWND,
    profiles: HWND,
    new: HWND,
    delete: HWND,
    version: HWND,
    name_label: HWND,
    name: HWND,
    enabled: HWND,
    status: HWND,
    monitors_label: HWND,
    monitors_hint: HWND,
    /// The monitor cards (monitor_list).
    monitors: HWND,
    detect: HWND,
    add_connected: HWND,
    hotkeys_label: HWND,
    hotkeys_hint: HWND,
    /// The hotkey cards (hotkey_list).
    hotkeys: HWND,
    add_hotkey: HWND,
    activity: HWND,
    startup: HWND,
    notify: HWND,
    dark: HWND,
    saved_note: HWND,
    close: HWND,
}

struct Settings {
    hwnd: HWND,
    dpi: u32,
    fonts: Fonts,
    c: Controls,
    config: AppConfig,
    connected: Vec<MonitorInfo>,
    active: Option<String>,
    sel: Option<usize>,
    current: HashMap<String, Option<u8>>,
    attempted: HashSet<String>,
    detecting: bool,
    /// The banner's icon: a tick (connected, hotkeys on), a warning (hotkeys off) or info.
    status_icon: StatusIcon,
    /// A detection asked for while one was running (Some(force)); it runs when that one finishes.
    detect_queued: Option<bool>,
    /// A typed name is waiting to be committed (the typing timer is running).
    typing: bool,
    regions: Regions,
}

thread_local! {
    static STATE: RefCell<Option<Settings>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Settings) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

#[derive(Clone, Copy, PartialEq)]
enum StatusIcon {
    Tick,
    Warning,
    Info,
}

enum After {
    Nothing,
    Close,
}

// ------------------------------------------------------------------------------------------ public

pub fn open(config: AppConfig, connected: Vec<MonitorInfo>, active: Option<String>, select: Option<String>) {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| ui::register_class(CLASS, wndproc, ui::app_icon()));

    let dpi = unsafe { GetDpiForSystem() };
    let style = WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN;
    let mut r = RECT { left: 0, top: 0, right: ui::scale(WIDTH, dpi), bottom: ui::scale(HEIGHT, dpi) };
    unsafe { AdjustWindowRectExForDpi(&mut r, style, 0, WS_EX_CONTROLPARENT, dpi) };
    let hwnd = unsafe {
        CreateWindowExW(WS_EX_CONTROLPARENT, wide(CLASS).as_ptr(), wide(ui::TITLE).as_ptr(), style, CW_USEDEFAULT, CW_USEDEFAULT,
            r.right - r.left, r.bottom - r.top, 0, 0, hinstance(), core::ptr::null())
    };
    HWND_SETTINGS.store(hwnd, Ordering::Relaxed);
    theme::apply_window(hwnd);

    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let fonts = Fonts::new(dpi);
    let c = create_controls(hwnd, &fonts, dpi);
    STATE.with(|s| {
        *s.borrow_mut() = Some(Settings {
            hwnd, dpi, fonts, c, config, connected, active, sel: None,
            current: HashMap::new(), attempted: HashSet::new(), detecting: false, detect_queued: None, status_icon: StatusIcon::Info, typing: false, regions: Regions::default(),
        })
    });
    with(|s| {
        widgets::set_checked(s.c.startup, sys::startup_enabled());
        widgets::set_checked(s.c.notify, s.config.show_notifications);
        widgets::set_checked(s.c.dark, theme::palette().dark);
        s.refresh_profiles(select.as_deref());
        s.layout();
    });
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
}

pub fn update_connection(connected: Vec<MonitorInfo>, active: Option<String>) {
    with(|s| {
        s.connected = connected;
        s.active = active;
        let id = s.selected_id();
        s.refresh_profiles(id.as_deref());
    });
}

/// Brings the main window forward, or the hotkey editor if it's open, and selects id.
/// Uses the stored handle rather than the window's state: while the editor is open that state
/// is in use (the editor runs from inside a main-window handler), so it can't be borrowed.
pub fn select_and_focus(id: Option<String>) {
    if id.is_some() {
        with(|s| s.refresh_profiles(id.as_deref())); // skipped while the editor is open
    }
    let main = hwnd();
    if main == 0 {
        return;
    }
    unsafe {
        if IsIconic(main) != 0 {
            ShowWindow(main, SW_RESTORE);
        }
        // The editor if it's open (it's owned by the main window), otherwise the main window.
        SetForegroundWindow(GetLastActivePopup(main));
    }
}

/// Closes the window without asking (the app is exiting).
pub fn destroy() {
    let h = hwnd();
    if h != 0 {
        unsafe { DestroyWindow(h) };
    }
}

// ------------------------------------------------------------------------------------------ window

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_COMMAND => {
            if hiword(w) == BN_CLICKED && l != 0 {
                widgets::clicked(l as HWND); // checkboxes flip before the click is handled
            }
            let after = with(|s| s.command(loword(w) as i32, hiword(w)));
            if let Some(After::Close) = after {
                DestroyWindow(h);
            }
            return 0;
        }
        WM_NOTIFY => {
            if let Some(Some(r)) = with(|s| s.notify(l)) {
                return r;
            }
        }
        WM_CLOSE => {
            if let Some(After::Close) = with(|s| s.request_close()) {
                DestroyWindow(h);
            }
            return 0;
        }
        WM_SIZE => {
            with(|s| s.layout());
            return 0;
        }
        WM_GETMINMAXINFO => {
            let dpi = GetDpiForWindow(h).max(96);
            let mut r = RECT { left: 0, top: 0, right: ui::scale(MIN_WIDTH, dpi), bottom: ui::scale(MIN_HEIGHT, dpi) };
            AdjustWindowRectExForDpi(&mut r, WS_OVERLAPPEDWINDOW, 0, WS_EX_CONTROLPARENT, dpi);
            let info = &mut *(l as *mut MINMAXINFO);
            info.ptMinTrackSize = POINT { x: r.right - r.left, y: r.bottom - r.top };
            return 0;
        }
        WM_DPICHANGED => {
            let suggested = *(l as *const RECT);
            with(|s| s.dpi_changed(hiword(w) as u32, suggested));
            return 0;
        }
        WM_ERASEBKGND => {
            if with(|s| s.paint_background(w as HDC)).is_none() {
                let mut rc = RECT::default();
                GetClientRect(h, &mut rc);
                theme::fill(w as HDC, rc, theme::palette().window);
            }
            return 1;
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
            if let Some(brush) = theme::ctl_color(m, w as HDC, l as HWND) {
                return brush;
            }
        }
        WM_MEASUREITEM => {
            let mi = &mut *(l as *mut MEASUREITEMSTRUCT);
            if mi.CtlType == ODT_LISTBOX {
                mi.itemHeight = ui::scale(PROFILE_ITEM_HEIGHT, GetDpiForWindow(h).max(96)) as u32;
                return 1;
            }
        }
        WM_DRAWITEM => {
            let dis = &*(l as *const DRAWITEMSTRUCT);
            if widgets::draw(dis) {
                return 1;
            }
            if dis.CtlType == ODT_LISTBOX {
                with(|s| s.draw_profile(dis));
                return 1;
            }
        }
        WM_SETTINGCHANGE if theme::is_theme_change(m, l) => {
            theme::reload();
            with(|s| s.retheme());
        }
        WM_POKE => {
            with(|s| s.drain());
            return 0;
        }
        WM_TIMER if w == TIMER_TYPING => {
            with(|s| s.commit());
            return 0;
        }
        WM_DESTROY => {
            HWND_SETTINGS.store(0, Ordering::Relaxed);
            // Everything was committed as it changed; the tray just needs to know the window has gone.
            STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut g| g.take()));
            tray::post(tray::Event::SettingsClosed);
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}

fn create_controls(h: HWND, f: &Fonts, dpi: u32) -> Controls {
    use theme::Surface::{Banner, Card, Layer, Window};
    let n = f.normal;
    // The name box has no border of its own: a rounded frame is painted behind it.
    let name = ui::child(h, "EDIT", "", ES_AUTOHSCROLL | WS_TABSTOP, 0, ID_NAME, f.title);
    unsafe { SendMessageW(name, EM_SETMARGINS, EC_LEFTMARGIN | EC_RIGHTMARGIN, (ui::scale(8, dpi) | (ui::scale(8, dpi) << 16)) as LPARAM) };
    theme::apply_control(name, theme::ControlKind::Edit);
    // Two-line items (name, then status) drawn in WM_DRAWITEM.
    let profiles = ui::child(h, "LISTBOX", "", LBS_NOTIFY | LBS_NOINTEGRALHEIGHT | LBS_OWNERDRAWFIXED | LBS_HASSTRINGS | WS_VSCROLL | WS_TABSTOP, 0, ID_PROFILES, n);
    theme::set_surface(profiles, Window, false);
    theme::apply_control(profiles, theme::ControlKind::ListBox);
    let saved_note = ui::child(h, "STATIC", "Changes are saved automatically", SS_NOPREFIX | SS_RIGHT, 0, 0, f.small);
    theme::set_surface(saved_note, Layer, true);
    let c = Controls {
        configs_label: ui::label_on(h, "Configurations", f.section, Window, false),
        profiles,
        new: widgets::button(h, "New from current setup", ID_NEW, Some(widgets::ICON_ADD), f, Window),
        delete: widgets::button(h, "Delete configuration", ID_DELETE, Some(widgets::ICON_DELETE), f, Window),
        version: ui::label_on(h, &format!("Version {}", sys::display_version()), f.small, Window, true),
        name_label: ui::label_on(h, "Name:", n, Layer, true),
        name,
        enabled: widgets::checkbox(h, "Hotkeys on", ID_ENABLED, f, Banner),
        status: ui::label_on(h, "", n, Banner, false),
        monitors_label: ui::label_on(h, "Monitors", f.section, Card, false),
        monitors_hint: ui::label_on(h, "This configuration is used whenever these monitors are connected.", n, Card, true),
        monitors: monitor_list::create(h, ID_MONITORS, dpi),
        detect: widgets::button(h, "Detect inputs", ID_DETECT, Some(widgets::ICON_REFRESH), f, Card),
        add_connected: widgets::button(h, "Add connected", ID_ADD_CONNECTED, Some(widgets::ICON_ADD), f, Card),
        hotkeys_label: ui::label_on(h, "Hotkeys", f.section, Card, false),
        hotkeys_hint: ui::label_on(h, "Switch to one input, toggle between two, or cycle through several.", n, Card, true),
        hotkeys: hotkey_list::create(h, ID_HOTKEYS, dpi),
        add_hotkey: widgets::accent_button(h, "Add hotkey", ID_ADD_HOTKEY, Some(widgets::ICON_ADD), f, Card),
        activity: ui::label_on(h, "", f.small, Layer, true),
        startup: widgets::checkbox(h, "Start with Windows", ID_STARTUP, f, Layer),
        notify: widgets::checkbox(h, "Notify when the setup changes", ID_NOTIFY, f, Layer),
        dark: widgets::switch(h, "Dark mode", ID_DARK, f, Layer),
        saved_note,
        close: widgets::button(h, "Close", IDCANCEL, None, f, Layer),
    };
    c
}

/// Areas painted behind the controls (in physical pixels): the content layer, the status
/// banner, the two cards and the name box's frame.
#[derive(Default)]
struct Regions {
    layer: RECT,
    banner: RECT,
    cards: [RECT; 2],
    name_frame: RECT,
}

impl Controls {
    fn all(&self) -> Vec<HWND> {
        vec![self.configs_label, self.profiles, self.new, self.delete, self.version, self.name_label, self.name, self.enabled,
            self.status, self.monitors_label, self.monitors_hint, self.monitors, self.detect, self.add_connected,
            self.hotkeys_label, self.hotkeys_hint, self.hotkeys, self.add_hotkey,
            self.activity, self.startup, self.notify, self.dark, self.saved_note, self.close]
    }

    /// The controls that only make sense with a configuration selected.
    fn detail(&self) -> Vec<HWND> {
        vec![self.delete, self.name, self.enabled, self.monitors, self.detect, self.add_connected,
            self.hotkeys, self.add_hotkey]
    }

}

impl Settings {
    // -------------------------------------------------------------------------------- layout

    fn layout(&mut self) {
        let mut rc = RECT::default();
        unsafe { GetClientRect(self.hwnd, &mut rc) };
        let d = self.dpi.max(96) as i32;
        let (w, h) = (rc.right * 96 / d, rc.bottom * 96 / d);
        let sc = |v: i32| ui::scale(v, self.dpi);
        let p = |hw: HWND, x: i32, y: i32, cw: i32, ch: i32| ui::place(hw, sc(x), sc(y), sc(cw), sc(ch));
        let r = |x: i32, y: i32, cw: i32, ch: i32| RECT { left: sc(x), top: sc(y), right: sc(x + cw), bottom: sc(y + ch) };
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
        let rx = x0 + 24;
        let re = w - 24;
        let rw = re - rx;
        let layer = r(x0, 0, w - x0, h);

        p(c.name_label, rx, 25, 48, 20);
        let name_frame = r(rx + 52, 14, rw - 52, 40);
        p(c.name, rx + 53, 19, rw - 54, 30);

        // Status banner: the status on the left, "Hotkeys on" on the right.
        let banner = r(rx, 64, rw, 46);
        p(c.status, rx + 16 + 28, 64 + 13, rw - 32 - 28 - 150, 20);
        p(c.enabled, re - 16 - 130, 64 + 11, 130, 24);

        // Footer.
        let bottom = h - 16 - 32;
        p(c.close, re - 100, bottom, 100, 32);
        p(c.startup, rx, bottom + 4, 160, 24);
        p(c.notify, rx + 166, bottom + 4, 230, 24);
        p(c.dark, rx + 412, bottom + 4, 130, 24);
        p(c.activity, rx, bottom - 28, rw - 230, 20);
        p(c.saved_note, re - 220, bottom - 28, 220, 20);

        // Two cards: Monitors and Hotkeys.
        let top = 126;
        let end = bottom - 40;
        let gap = 16;
        let (cx, cw) = (rx + 16, rw - 32);
        // Monitors: title and subtitle, the two buttons on the right (as in the C# app), then the
        // cards. The card is as tall as the monitors need, leaving the Hotkeys card some room;
        // beyond that the cards scroll. Their height depends on the width, so size that first.
        let header = 66;
        p(c.monitors, cx, top + header, cw, 10);
        let want = header + monitor_list::content_height().max(40) + 16;
        let monitors_h = want.min(end - top - gap - 200).max(header + 60);
        p(c.monitors_label, cx, top + 12, cw - 310, 26);
        p(c.monitors_hint, cx, top + 40, cw - 310, 20);
        p(c.detect, cx + cw - 150 - 8 - 140, top + 16, 140, 32);
        p(c.add_connected, cx + cw - 150, top + 16, 150, 32);
        p(c.monitors, cx, top + header, cw, monitors_h - header - 16);

        let hy = top + monitors_h + gap;
        // Hotkeys: the same header (Add hotkey on the right), then the cards; the card is as tall
        // as they need, up to the space left.
        p(c.hotkeys, cx, hy + header, cw, 10);
        let want = header + hotkey_list::content_height() + 16;
        let hotkeys_h = want.min(end - hy).max(header + 50);
        p(c.hotkeys_label, cx, hy + 12, cw - 160, 26);
        p(c.hotkeys_hint, cx, hy + 40, cw - 160, 20);
        p(c.add_hotkey, cx + cw - 130, hy + 16, 130, 32);
        p(c.hotkeys, cx, hy + header, cw, hotkeys_h - header - 16);

        self.regions = Regions { layer, banner, cards: [r(rx, top, rw, monitors_h), r(rx, hy, rw, hotkeys_h)], name_frame };
        // All children too: moved controls otherwise keep stale pixels copied from where they were.
        unsafe { RedrawWindow(self.hwnd, core::ptr::null(), 0, RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN) };
    }

    /// Paints the window background, the content layer, the cards, the banner and the name frame.
    fn paint_background(&self, hdc: HDC) {
        let p = theme::palette();
        let mut rc = RECT::default();
        unsafe { GetClientRect(self.hwnd, &mut rc) };
        theme::fill(hdc, rc, p.window);
        let g = &self.regions;
        let radius = ui::scale(8, self.dpi);
        // Only the layer's top-left corner is rounded: extend it past the window's other edges.
        let layer = RECT { right: g.layer.right + radius * 2, bottom: g.layer.bottom + radius * 2, ..g.layer };
        theme::round_rect(hdc, layer, radius, p.layer, Some(p.stroke));
        for card in g.cards {
            theme::round_rect(hdc, card, radius, p.card, Some(p.stroke));
        }
        theme::round_rect(hdc, g.banner, ui::scale(6, self.dpi), theme::color_of(theme::Surface::Banner), None);
        // The banner's icon, coloured like the C# app's (green tick, amber warning, grey info).
        let (glyph, colour) = match self.status_icon {
            StatusIcon::Tick => ('\u{E73E}', theme::success_text()),
            StatusIcon::Warning => ('\u{E7BA}', theme::caution_text()),
            StatusIcon::Info => ('\u{E946}', p.text2),
        };
        let icon_font = ui::icon_font(self.dpi, 16);
        let b = g.banner;
        let icon = RECT { left: b.left + ui::scale(16, self.dpi), top: b.top, right: b.left + ui::scale(16 + 20, self.dpi), bottom: b.bottom };
        theme::text(hdc, &glyph.to_string(), icon, icon_font, colour, DT_VCENTER | DT_SINGLELINE);
        unsafe { DeleteObject(icon_font) };
        theme::round_rect(hdc, g.name_frame, ui::scale(4, self.dpi), p.control, Some(p.control_stroke));
    }

    /// Draws one configuration in the list: an icon, the name, and its status underneath.
    fn draw_profile(&self, dis: &DRAWITEMSTRUCT) {
        if dis.itemID == u32::MAX {
            return;
        }
        let p = theme::palette();
        let hdc = dis.hDC;
        let r = dis.rcItem;
        let sc = |v: i32| ui::scale(v, self.dpi);
        let text = unsafe {
            let n = SendMessageW(self.c.profiles, LB_GETTEXTLEN, dis.itemID as usize, 0).max(0) as usize;
            let mut buf = vec![0u16; n + 1];
            SendMessageW(self.c.profiles, LB_GETTEXT, dis.itemID as usize, buf.as_mut_ptr() as LPARAM);
            from_wide(&buf)
        };
        let (title, subtitle) = text.split_once('\t').unwrap_or((&text, ""));
        let selected = dis.itemState & ODS_SELECTED != 0;
        let connected = subtitle.starts_with("Connected");

        theme::fill(hdc, r, p.window);
        let pill = RECT { left: r.left + sc(2), top: r.top + sc(2), right: r.right - sc(2), bottom: r.bottom - sc(2) };
        if selected {
            theme::round_rect(hdc, pill, sc(5), p.selected, None);
            let bar_h = sc(16);
            let mid = (pill.top + pill.bottom) / 2;
            theme::round_rect(hdc, RECT { left: pill.left, top: mid - bar_h / 2, right: pill.left + sc(3), bottom: mid + bar_h / 2 }, sc(2), p.accent, None);
        }
        let icon = RECT { left: pill.left + sc(12), top: pill.top, right: pill.left + sc(36), bottom: pill.bottom };
        theme::text(hdc, "\u{E7F4}", icon, self.fonts.nav_icon, if connected { p.accent } else { p.text2 }, DT_VCENTER | DT_SINGLELINE);
        let tx = pill.left + sc(40);
        let mid = (pill.top + pill.bottom) / 2;
        theme::text(hdc, title, RECT { left: tx, top: pill.top + sc(4), right: pill.right - sc(8), bottom: mid + sc(1) }, self.fonts.normal, p.text, DT_SINGLELINE | DT_END_ELLIPSIS | 0x0008 /* DT_BOTTOM */);
        theme::text(hdc, subtitle, RECT { left: tx, top: mid + sc(1), right: pill.right - sc(8), bottom: pill.bottom - sc(4) }, self.fonts.small, p.text2, DT_SINGLELINE | DT_END_ELLIPSIS);
    }

    /// Re-applies the palette after Windows switches between light and dark (or the accent changes).
    fn retheme(&mut self) {
        widgets::set_checked(self.c.dark, theme::palette().dark);
        theme::apply_window(self.hwnd);
        monitor_list::retheme(self.c.monitors);
        hotkey_list::retheme(self.c.hotkeys);
        theme::apply_control(self.c.profiles, theme::ControlKind::ListBox);
        theme::apply_control(self.c.name, theme::ControlKind::Edit);
        self.update_status();
        theme::repaint(self.hwnd);
    }

    fn dpi_changed(&mut self, dpi: u32, suggested: RECT) {
        self.dpi = dpi;
        let fonts = Fonts::new(dpi);
        for h in self.c.all() {
            unsafe { SendMessageW(h, WM_SETFONT, fonts.normal as usize, 1) };
            widgets::set_fonts(h, &fonts);
        }
        for h in [self.c.configs_label, self.c.monitors_label, self.c.hotkeys_label] {
            unsafe { SendMessageW(h, WM_SETFONT, fonts.section as usize, 1) };
        }
        monitor_list::set_dpi(dpi);
        hotkey_list::set_dpi(dpi);
        for h in [self.c.activity, self.c.saved_note, self.c.version] {
            unsafe { SendMessageW(h, WM_SETFONT, fonts.small as usize, 1) };
        }
        unsafe {
            SendMessageW(self.c.name, WM_SETFONT, fonts.title as usize, 1);
            SendMessageW(self.c.profiles, LB_SETITEMHEIGHT, 0, ui::scale(PROFILE_ITEM_HEIGHT, dpi) as LPARAM);
        }
        self.fonts = fonts; // the old fonts are deleted once nothing uses them
        unsafe {
            SetWindowPos(self.hwnd, 0, suggested.left, suggested.top, suggested.right - suggested.left,
                suggested.bottom - suggested.top, SWP_NOZORDER | SWP_NOACTIVATE);
        }
        self.layout();
    }
    // -------------------------------------------------------------------------------- configurations

    fn selected_id(&self) -> Option<String> {
        self.sel.and_then(|i| self.config.profiles.get(i)).map(|p| p.id.clone())
    }

    fn is_active(&self, index: usize) -> bool {
        self.active.as_deref() == Some(self.config.profiles[index].id.as_str())
    }

    fn is_connected(&self, monitor_id: &str) -> bool {
        self.connected.iter().any(|m| m.id.eq_ignore_ascii_case(monitor_id))
    }

    fn connected_monitor(&self, monitor_id: &str) -> Option<MonitorInfo> {
        self.connected.iter().find(|m| m.id.eq_ignore_ascii_case(monitor_id)).cloned()
    }

    /// "Name\tConnected · 2 monitors · hotkeys off": drawn as two lines by draw_profile.
    fn profile_label(&self, i: usize) -> String {
        let p = &self.config.profiles[i];
        let mut parts = vec![
            if self.is_active(i) { "Connected".to_string() } else { "Not connected".to_string() },
            if p.monitors.len() == 1 { "1 monitor".to_string() } else { format!("{} monitors", p.monitors.len()) },
        ];
        if !p.enabled { parts.push("hotkeys off".into()); }
        let name = if p.name.trim().is_empty() { "Unnamed setup" } else { p.name.as_str() };
        format!("{name}\t{}", parts.join(" · "))
    }

    fn refresh_profiles(&mut self, select: Option<&str>) {
        let labels: Vec<String> = (0..self.config.profiles.len()).map(|i| self.profile_label(i)).collect();
        self.sel = select
            .and_then(|id| self.config.profiles.iter().position(|p| p.id == id))
            .or(if labels.is_empty() { None } else { Some(0) });
        ui::lb_set(self.c.profiles, &labels, self.sel);
        self.show_profile();
    }

    fn refresh_profile_label(&self) {
        if let Some(i) = self.sel {
            ui::lb_replace(self.c.profiles, i, &self.profile_label(i));
        }
    }

    fn show_profile(&mut self) {
        let has = self.sel.is_some();
        for h in self.c.detail() {
            ui::enable(h, has);
        }
        let Some(i) = self.sel else {
            ui::set_text(self.c.name, "");
            widgets::set_checked(self.c.enabled, false);
            let text = if matcher::fingerprint(&self.connected).is_empty() {
                "No external monitors are connected. Connect your monitors and a configuration will be created for them automatically."
            } else {
                "No configurations yet. Choose \"New from current setup\" to create one for the monitors connected now."
            };
            ui::set_text(self.c.status, text);
            self.set_banner(theme::palette().neutral);
            monitor_list::set_rows(Vec::new());
            hotkey_list::set_rows(Vec::new());
            return;
        };
        let (name, enabled) = (self.config.profiles[i].name.clone(), self.config.profiles[i].enabled);
        ui::set_text(self.c.name, &name);
        widgets::set_checked(self.c.enabled, enabled);
        self.update_status();
        self.refresh_monitors();
        self.refresh_hotkeys();
        self.detect(false);
    }

    fn update_status(&mut self) {
        let Some(i) = self.sel else { return };
        let p = &self.config.profiles[i];
        let present = p.monitors.iter().filter(|m| self.is_connected(&m.monitor_id)).count();
        let colours = theme::palette();
        let (text, banner, icon) = if self.is_active(i) {
            if p.enabled {
                (format!("Connected now ({present} of {} monitors present). Hotkeys are active.", p.monitors.len()), colours.success, StatusIcon::Tick)
            } else {
                (format!("Connected now ({present} of {} monitors present). Turn hotkeys on to use them.", p.monitors.len()), colours.caution, StatusIcon::Warning)
            }
        } else {
            ("Not connected now. Inputs can only be detected and tested while this setup is connected.".to_string(), colours.neutral, StatusIcon::Info)
        };
        self.status_icon = icon;
        ui::set_text(self.c.status, &text);
        self.set_banner(banner);
    }

    /// Sets the status banner's colour (green, amber or grey) and repaints it.
    fn set_banner(&self, colour: u32) {
        theme::set_banner(colour);
        let r = self.regions.banner;
        unsafe { RedrawWindow(self.hwnd, &r, 0, RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN) };
    }

    fn rename_profile(&mut self) {
        let Some(i) = self.sel else { return };
        self.config.profiles[i].name = ui::get_text(self.c.name);
        self.refresh_profile_label();
        self.commit_soon();
    }

    fn toggle_enabled(&mut self) {
        let Some(i) = self.sel else { return };
        self.config.profiles[i].enabled = widgets::checked(self.c.enabled);
        self.refresh_profile_label();
        self.update_status();
        self.commit();
    }

    fn new_from_current(&mut self) {
        let fp = matcher::fingerprint(&self.connected);
        if fp.is_empty() {
            ui::alert(self.hwnd, ui::Icon::Info, "No external monitors connected",
                "Connect the monitors for this setup, then try again. The laptop's own screen doesn't count.");
            return;
        }
        if let Some(existing) = self.config.profiles.iter().find(|p| {
            p.monitors.iter().map(|m| m.monitor_id.to_lowercase()).collect::<HashSet<_>>() == fp
        }) {
            let msg = format!("\"{}\" already covers exactly these monitors. When two configurations match, the first one in the list is used.", existing.name);
            if !ui::confirm(self.hwnd, "Create another configuration?", &msg, "Create") {
                return;
            }
        }
        let p = matcher::create_for(&self.connected, &self.config);
        let id = p.id.clone();
        self.config.profiles.push(p);
        self.commit();
        self.refresh_profiles(Some(&id));
        unsafe {
            SetFocus(self.c.name);
            SendMessageW(self.c.name, 0x00B1 /* EM_SETSEL */, 0, -1);
        }
    }

    fn delete_profile(&mut self) {
        let Some(i) = self.sel else { return };
        let p = &self.config.profiles[i];
        let hotkeys = if p.bindings.len() == 1 { "its hotkey".to_string() } else { format!("its {} hotkeys", p.bindings.len()) };
        let note = if self.is_active(i) { " This setup is connected now, so a fresh, empty configuration will be created for it when you close this window." } else { "" };
        if !ui::confirm(self.hwnd, &format!("Delete \"{}\"?", p.name), &format!("The configuration and {hotkeys} will be removed.{note}"), "Delete") {
            return;
        }
        self.config.profiles.remove(i);
        self.commit();
        self.refresh_profiles(None);
    }

    // -------------------------------------------------------------------------------- monitors

    fn refresh_monitors(&mut self) {
        let rows = match self.sel {
            None => Vec::new(),
            Some(i) => self.config.profiles[i].monitors.iter().map(|m| {
                let connected = self.is_connected(&m.monitor_id);
                let current = self.current.get(&m.monitor_id.to_lowercase()).copied().flatten();
                monitor_list::Row {
                    id: m.monitor_id.clone(),
                    name: m.name.clone(),
                    details: match &m.serial {
                        Some(s) if !s.is_empty() => format!("{} · S/N {s}", m.model),
                        _ => m.model.clone(),
                    },
                    connected,
                    chips: m.supported_inputs.iter().map(|&c| (monitors::short_name(c), current == Some(c))).collect(),
                    hint: if !m.supported_inputs.is_empty() {
                        String::new()
                    } else if connected {
                        "Inputs not detected yet".into()
                    } else {
                        "Connect this monitor to detect its inputs".into()
                    },
                }
            }).collect(),
        };
        monitor_list::set_rows(rows);
        self.layout();
    }

    /// A monitor's name box changed: keep it (unless blank) and save shortly.
    fn rename_monitor(&mut self, row: usize) {
        let (Some(i), Some(text)) = (self.sel, monitor_list::name_text(row)) else { return };
        let text = text.trim();
        if text.is_empty() || row >= self.config.profiles[i].monitors.len() {
            return;
        }
        if self.config.profiles[i].monitors[row].name != text {
            self.config.profiles[i].monitors[row].name = text.to_string();
            self.commit_soon();
        }
    }

    /// Leaving a monitor's name box: put back the saved name if it was left blank, and show the
    /// new name in the hotkeys list.
    fn finish_monitor_name(&mut self, row: usize) {
        let Some(i) = self.sel else { return };
        let Some(m) = self.config.profiles[i].monitors.get(row) else { return };
        if monitor_list::name_text(row).is_some_and(|t| t.trim() != m.name) {
            monitor_list::set_name_text(row, &m.name.clone());
        }
        self.refresh_hotkeys();
    }
    fn add_connected(&mut self) {
        let Some(i) = self.sel else { return };
        let missing: Vec<MonitorInfo> = self.connected.iter()
            .filter(|m| !m.internal && !self.config.profiles[i].has_monitor(&m.id)).cloned().collect();
        if missing.is_empty() {
            ui::alert(self.hwnd, ui::Icon::Info, "Nothing to add", "All the connected monitors are already in this configuration.");
            return;
        }
        for m in missing {
            self.attempted.remove(&m.id.to_lowercase()); // always read what was just added
            self.config.profiles[i].monitors.push(ProfileMonitor {
                monitor_id: m.id.clone(), name: m.display_name().to_string(), model: m.model.clone(), serial: m.serial.clone(), supported_inputs: Vec::new(),
            });
        }
        self.commit();
        self.refresh_profile_label();
        self.update_status();
        self.refresh_monitors();
        self.detect(false);
    }

    fn remove_monitor(&mut self, index: usize) {
        let Some(i) = self.sel else { return };
        if index >= self.config.profiles[i].monitors.len() {
            return;
        }
        let p = &self.config.profiles[i];
        let m = &p.monitors[index];
        let used = p.bindings.iter().filter(|b| b.monitor_id.eq_ignore_ascii_case(&m.monitor_id)).count();
        let also = match used { 0 => String::new(), 1 => " Its hotkey will be removed too.".into(), n => format!(" Its {n} hotkeys will be removed too.") };
        let msg = format!("Configurations are recognised by their monitors, so this changes which setups \"{}\" matches.{also}", p.name);
        if !ui::confirm(self.hwnd, &format!("Remove {}?", m.name), &msg, "Remove") {
            return;
        }
        let id = self.config.profiles[i].monitors.remove(index).monitor_id;
        self.config.profiles[i].bindings.retain(|b| !b.monitor_id.eq_ignore_ascii_case(&id));
        self.commit();
        self.refresh_profile_label();
        self.update_status();
        self.refresh_monitors();
        self.refresh_hotkeys();
    }

    /// Reads each connected monitor's inputs and current input in the background.
    fn detect(&mut self, force: bool) {
        let Some(i) = self.sel else { return };
        if self.detecting {
            self.detect_queued = Some(force || self.detect_queued.unwrap_or(false));
            return;
        }
        let targets: Vec<(String, MonitorInfo)> = self.config.profiles[i].monitors.iter()
            .filter(|m| force || !self.attempted.contains(&m.monitor_id.to_lowercase()))
            .filter_map(|m| self.connected_monitor(&m.monitor_id).map(|info| (m.monitor_id.clone(), info)))
            .collect();
        if targets.is_empty() {
            if force {
                ui::set_text(self.c.activity, "None of this configuration's monitors are connected.");
            }
            return;
        }
        for (id, _) in &targets {
            self.attempted.insert(id.to_lowercase());
        }
        self.detecting = true;
        ui::set_text(self.c.activity, "Reading monitors over DDC/CI…");
        std::thread::spawn(move || {
            for (id, m) in targets {
                let inputs = monitors::supported_inputs(&m);
                let current = monitors::current_input(&m);
                post_bg(Bg::Detected { id, inputs, current });
            }
            post_bg(Bg::DetectDone);
        });
    }

    fn drain(&mut self) {
        let events: Vec<Bg> = std::mem::take(&mut *EVENTS.lock().unwrap_or_else(|e| e.into_inner()));
        let mut changed = false;
        for ev in events {
            match ev {
                Bg::Detected { id, inputs, current } => {
                    if !inputs.is_empty() {
                        for p in &mut self.config.profiles {
                            for m in &mut p.monitors {
                                if m.monitor_id.eq_ignore_ascii_case(&id) && m.supported_inputs != inputs {
                                    m.supported_inputs = inputs.clone();
                                    changed = true;
                                }
                            }
                        }
                    }
                    self.current.insert(id.to_lowercase(), current);
                    self.refresh_monitors();
                }
                Bg::DetectDone => {
                    self.detecting = false;
                    let missing = self.sel.map(|i| self.config.profiles[i].monitors.iter()
                        .any(|m| self.is_connected(&m.monitor_id) && m.supported_inputs.is_empty())).unwrap_or(false);
                    ui::set_text(self.c.activity, if missing {
                        "Some monitors did not report their inputs. Check DDC/CI is on in the monitor's menu."
                    } else {
                        "Inputs detected."
                    });
                    if let Some(force) = self.detect_queued.take() {
                        self.detect(force);
                    }
                }
                Bg::Tested { id, result } => {
                    match result {
                        Ok(code) => {
                            self.current.insert(id.to_lowercase(), Some(code));
                            let name = self.connected_monitor(&id).map(|m| m.display_name().to_string()).unwrap_or_default();
                            ui::set_text(self.c.activity, &format!("{name}: {}", monitors::short_name(code)));
                        }
                        Err(e) => ui::set_text(self.c.activity, &e),
                    }
                    self.refresh_monitors();
                }
            }
        }
        if changed {
            self.commit();
        }
    }

    // -------------------------------------------------------------------------------- hotkeys

    fn refresh_hotkeys(&mut self) {
        let rows = match self.sel {
            None => Vec::new(),
            Some(i) => {
                let p = &self.config.profiles[i];
                p.bindings.iter().map(|b| hotkey_list::Row {
                    keys: b.hotkey.display().split(" + ").map(str::to_string).collect(),
                    action: monitors::describe_action(&b.inputs),
                    monitor: p.monitor(&b.monitor_id).map(|m| m.name.clone()).unwrap_or_else(|| "(removed monitor)".into()),
                }).collect()
            }
        };
        hotkey_list::set_rows(rows);
        self.layout();
    }
    fn add_hotkey(&mut self) {
        let Some(i) = self.sel else { return };
        if self.config.profiles[i].monitors.is_empty() {
            ui::alert(self.hwnd, ui::Icon::Info, "No monitors in this configuration", "Connect a monitor and use Add connected before adding a hotkey.");
            return;
        }
        let Some(b) = self.run_editor(i, None) else { return };
        let p = &mut self.config.profiles[i];
        p.bindings.push(b);
        if !p.enabled {
            // Adding a hotkey is a strong sign the configuration should be live.
            p.enabled = true;
            widgets::set_checked(self.c.enabled, true);
            self.refresh_profile_label();
            self.update_status();
        }
        self.commit();
        self.refresh_hotkeys();
    }

    fn edit_hotkey(&mut self, index: usize) {
        let Some(i) = self.sel else { return };
        if index >= self.config.profiles[i].bindings.len() {
            return;
        }
        let existing: Binding = self.config.profiles[i].bindings[index].clone();
        let Some(b) = self.run_editor(i, Some(&existing)) else { return };
        self.config.profiles[i].bindings[index] = b;
        self.commit();
        self.refresh_hotkeys();
    }

    fn remove_hotkey(&mut self, index: usize) {
        let Some(i) = self.sel else { return };
        if index >= self.config.profiles[i].bindings.len() {
            return;
        }
        let p = &self.config.profiles[i];
        let b = &p.bindings[index];
        let monitor = p.monitor(&b.monitor_id).map(|m| m.name.clone()).unwrap_or_else(|| "the monitor".into());
        let msg = format!("The hotkey for {monitor} ({}) will be removed.", monitors::describe_action(&b.inputs));
        if !ui::confirm(self.hwnd, &format!("Remove {}?", b.hotkey.display()), &msg, "Remove") {
            return;
        }
        self.config.profiles[i].bindings.remove(index);
        self.commit();
        self.refresh_hotkeys();
    }

    /// Shows the hotkey editor. The tray's hotkeys are paused for as long as it's open, so a
    /// combination that's already in use can be recorded (or the editor used at all) without one firing.
    fn run_editor(&mut self, profile: usize, existing: Option<&Binding>) -> Option<Binding> {
        tray::post(tray::Event::Capturing(true));
        let result = editor::run(self.hwnd, self.dpi, &self.config.profiles[profile], existing);
        tray::post(tray::Event::Capturing(false));
        result
    }

    fn test_hotkey(&mut self, index: usize) {
        let Some(i) = self.sel else { return };
        if index >= self.config.profiles[i].bindings.len() {
            return;
        }
        let b = self.config.profiles[i].bindings[index].clone();
        let Some(m) = self.connected_monitor(&b.monitor_id) else {
            ui::set_text(self.c.activity, "That monitor is not connected.");
            return;
        };
        ui::set_text(self.c.activity, &format!("Testing {}…", b.hotkey.display()));
        std::thread::spawn(move || {
            let result = monitors::run_binding(&b.inputs, &m);
            post_bg(Bg::Tested { id: b.monitor_id, result });
        });
    }

    // -------------------------------------------------------------------------------- messages

    fn command(&mut self, id: i32, code: u16) -> After {
        match (id, code) {
            (ID_PROFILES, LBN_SELCHANGE) => {
                self.sel = ui::lb_selected(self.c.profiles);
                self.show_profile();
            }
            (ID_NEW, BN_CLICKED) => self.new_from_current(),
            (ID_DELETE, BN_CLICKED) => self.delete_profile(),
            (ID_NAME, EN_CHANGE) => self.rename_profile(),
            (ID_ENABLED, BN_CLICKED) => self.toggle_enabled(),
            (ID_DETECT, BN_CLICKED) => self.detect(true),
            (ID_ADD_CONNECTED, BN_CLICKED) => self.add_connected(),
            (id, EN_CHANGE) if (monitor_list::NAME_ID..monitor_list::NAME_ID + monitor_list::MAX_ROWS).contains(&id) => {
                self.rename_monitor((id - monitor_list::NAME_ID) as usize)
            }
            (id, EN_KILLFOCUS) if (monitor_list::NAME_ID..monitor_list::NAME_ID + monitor_list::MAX_ROWS).contains(&id) => {
                self.finish_monitor_name((id - monitor_list::NAME_ID) as usize)
            }
            (id, BN_CLICKED) if (monitor_list::REMOVE_ID..monitor_list::REMOVE_ID + monitor_list::MAX_ROWS).contains(&id) => {
                self.remove_monitor((id - monitor_list::REMOVE_ID) as usize)
            }
            (ID_ADD_HOTKEY, BN_CLICKED) => self.add_hotkey(),
            (id, BN_CLICKED) if (hotkey_list::TEST_ID..hotkey_list::TEST_ID + hotkey_list::MAX_ROWS).contains(&id) => {
                self.test_hotkey((id - hotkey_list::TEST_ID) as usize)
            }
            (id, BN_CLICKED) if (hotkey_list::EDIT_ID..hotkey_list::EDIT_ID + hotkey_list::MAX_ROWS).contains(&id) => {
                self.edit_hotkey((id - hotkey_list::EDIT_ID) as usize)
            }
            (id, BN_CLICKED) if (hotkey_list::REMOVE_ID..hotkey_list::REMOVE_ID + hotkey_list::MAX_ROWS).contains(&id) => {
                self.remove_hotkey((id - hotkey_list::REMOVE_ID) as usize)
            }
            (ID_STARTUP, BN_CLICKED) => self.set_startup(),
            (ID_DARK, BN_CLICKED) => {
                self.config.theme = if widgets::checked(self.c.dark) { crate::config::Theme::Dark } else { crate::config::Theme::Light };
                theme::set_choice(self.config.theme); // re-theme straight away; the tray sets it again on save
                self.retheme();
                self.commit();
            }
            (ID_NOTIFY, BN_CLICKED) => {
                self.config.show_notifications = widgets::checked(self.c.notify);
                self.commit();
            }
            (IDCANCEL, _) => return self.request_close(),
            _ => {}
        }
        After::Nothing
    }

    fn notify(&mut self, _l: LPARAM) -> Option<LRESULT> {
        None // the lists are custom panels now; nothing sends list notifications
    }

    /// Hands a copy of the configuration to the tray app, which saves and applies it.
    fn commit(&mut self) {
        unsafe { KillTimer(self.hwnd, TIMER_TYPING) };
        self.typing = false;
        tray::post(tray::Event::ConfigChanged(self.config.clone()));
    }

    /// Commits after a short pause (for text being typed).
    fn commit_soon(&mut self) {
        self.typing = true;
        // SetTimer with an existing id restarts it.
        unsafe { SetTimer(self.hwnd, TIMER_TYPING, TYPING_MS, core::ptr::null()) };
    }

    fn set_startup(&mut self) {
        let on = widgets::checked(self.c.startup);
        if !sys::set_startup(on) {
            ui::alert(self.hwnd, ui::Icon::Warning, "Couldn't change Start with Windows", "The startup setting couldn't be written to the registry.");
            widgets::set_checked(self.c.startup, sys::startup_enabled());
        }
    }

    /// Nothing to ask: everything is already saved. Commits anything still being typed, and gives
    /// blank names a placeholder, on the way out.
    fn request_close(&mut self) -> After {
        let mut pending = std::mem::take(&mut self.typing);
        for p in &mut self.config.profiles {
            if p.name.trim().is_empty() {
                p.name = "Unnamed setup".into();
                pending = true;
            }
        }
        if pending {
            self.commit();
        }
        After::Close
    }
}