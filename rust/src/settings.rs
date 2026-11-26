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
const ID_RENAME: i32 = 106;
const ID_DETECT: i32 = 107;
const ID_ADD_CONNECTED: i32 = 108;
const ID_REMOVE_MONITOR: i32 = 109;
const ID_HOTKEYS: i32 = 110;
const ID_ADD_HOTKEY: i32 = 111;
const ID_EDIT_HOTKEY: i32 = 112;
const ID_REMOVE_HOTKEY: i32 = 113;
const ID_TEST_HOTKEY: i32 = 114;
const ID_STARTUP: i32 = 115;
const ID_NOTIFY: i32 = 116;
const ID_DARK: i32 = 117;

/// Client size at 96 DPI.
const WIDTH: i32 = 1000;
const HEIGHT: i32 = 700;
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
    monitors: HWND,
    rename: HWND,
    detect: HWND,
    add_connected: HWND,
    remove_monitor: HWND,
    hotkeys_label: HWND,
    hotkeys: HWND,
    add_hotkey: HWND,
    edit_hotkey: HWND,
    remove_hotkey: HWND,
    test_hotkey: HWND,
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
            current: HashMap::new(), attempted: HashSet::new(), detecting: false, typing: false, regions: Regions::default(),
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
    let saved_note = ui::child(h, "STATIC", "Changes are saved automatically", SS_NOPREFIX | SS_RIGHT, 0, 0, n);
    theme::set_surface(saved_note, Layer, true);
    let c = Controls {
        configs_label: ui::label_on(h, "Configurations", f.section, Window, false),
        profiles,
        new: widgets::button(h, "New from current setup", ID_NEW, Some(widgets::ICON_ADD), f, Window),
        delete: widgets::button(h, "Delete configuration", ID_DELETE, Some(widgets::ICON_DELETE), f, Window),
        version: ui::label_on(h, &format!("Version {}", sys::display_version()), n, Window, true),
        name_label: ui::label_on(h, "Name:", n, Layer, true),
        name,
        enabled: widgets::checkbox(h, "Hotkeys on", ID_ENABLED, f, Banner),
        status: ui::label_on(h, "", n, Banner, false),
        monitors_label: ui::label_on(h, "Monitors", f.section, Card, false),
        monitors: ui::listview(h, ID_MONITORS, LVS_EDITLABELS, 0, n),
        rename: widgets::button(h, "Rename", ID_RENAME, Some(widgets::ICON_RENAME), f, Card),
        detect: widgets::button(h, "Detect inputs", ID_DETECT, Some(widgets::ICON_REFRESH), f, Card),
        add_connected: widgets::button(h, "Add connected", ID_ADD_CONNECTED, Some(widgets::ICON_ADD), f, Card),
        remove_monitor: widgets::button(h, "Remove", ID_REMOVE_MONITOR, Some(widgets::ICON_DELETE), f, Card),
        hotkeys_label: ui::label_on(h, "Hotkeys", f.section, Card, false),
        hotkeys: ui::listview(h, ID_HOTKEYS, 0, 0, n),
        add_hotkey: widgets::accent_button(h, "Add hotkey", ID_ADD_HOTKEY, Some(widgets::ICON_ADD), f, Card),
        edit_hotkey: widgets::button(h, "Edit", ID_EDIT_HOTKEY, Some(widgets::ICON_EDIT), f, Card),
        remove_hotkey: widgets::button(h, "Remove", ID_REMOVE_HOTKEY, Some(widgets::ICON_DELETE), f, Card),
        test_hotkey: widgets::button(h, "Test", ID_TEST_HOTKEY, Some(widgets::ICON_PLAY), f, Card),
        activity: ui::label_on(h, "", n, Layer, true),
        startup: widgets::checkbox(h, "Start with Windows", ID_STARTUP, f, Layer),
        notify: widgets::checkbox(h, "Notify when the setup changes", ID_NOTIFY, f, Layer),
        dark: widgets::switch(h, "Dark mode", ID_DARK, f, Layer),
        saved_note,
        close: widgets::button(h, "Close", IDCANCEL, None, f, Layer),
    };
    let s = |v| ui::scale(v, dpi);
    for (i, (title, width)) in [("Name", 170), ("Model", 80), ("Serial", 100), ("Status", 95), ("Current input", 105), ("Available inputs", 250)].iter().enumerate() {
        ui::lv_add_column(c.monitors, i as i32, title, s(*width));
    }
    for (i, (title, width)) in [("Hotkey", 150), ("Monitor", 170), ("Action", 330)].iter().enumerate() {
        ui::lv_add_column(c.hotkeys, i as i32, title, s(*width));
    }
    c
}

/// Sizes a list view's last column to the width the other columns leave.
fn fill_last_column(list: HWND, last: usize) {
    unsafe {
        let mut rc = RECT::default();
        GetClientRect(list, &mut rc);
        let used: i32 = (0..last).map(|i| SendMessageW(list, LVM_GETCOLUMNWIDTH, i, 0) as i32).sum();
        SendMessageW(list, LVM_SETCOLUMNWIDTH, last, (rc.right - used).max(ui::scale(80, GetDpiForWindow(list).max(96))) as LPARAM);
    }
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
            self.status, self.monitors_label, self.monitors, self.rename, self.detect, self.add_connected, self.remove_monitor,
            self.hotkeys_label, self.hotkeys, self.add_hotkey, self.edit_hotkey, self.remove_hotkey, self.test_hotkey,
            self.activity, self.startup, self.notify, self.dark, self.saved_note, self.close]
    }

    /// The controls that only make sense with a configuration selected.
    fn detail(&self) -> Vec<HWND> {
        vec![self.delete, self.name, self.enabled, self.monitors, self.rename, self.detect, self.add_connected,
            self.remove_monitor, self.hotkeys, self.add_hotkey]
    }

    /// The buttons that act on the selected hotkey.
    fn hotkey_actions(&self) -> [HWND; 3] {
        [self.edit_hotkey, self.remove_hotkey, self.test_hotkey]
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
        p(c.status, rx + 16, 64 + 13, rw - 32 - 150, 20);
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
        let monitors_h = (end - top - gap) * 45 / 100;
        let (cx, cw) = (rx + 16, rw - 32);
        p(c.monitors_label, cx, top + 12, cw, 26);
        p(c.monitors, cx, top + 44, cw, monitors_h - 44 - 12 - 32 - 12);
        let by = top + monitors_h - 12 - 32;
        p(c.rename, cx, by, 110, 32);
        p(c.detect, cx + 118, by, 140, 32);
        p(c.add_connected, cx + 266, by, 150, 32);
        p(c.remove_monitor, cx + 424, by, 110, 32);

        let hy = top + monitors_h + gap;
        let hotkeys_h = end - hy;
        p(c.hotkeys_label, cx, hy + 12, cw, 26);
        p(c.hotkeys, cx, hy + 44, cw, hotkeys_h - 44 - 12 - 32 - 12);
        let by = end - 12 - 32;
        p(c.add_hotkey, cx, by, 130, 32);
        p(c.edit_hotkey, cx + 138, by, 100, 32);
        p(c.remove_hotkey, cx + 246, by, 110, 32);
        p(c.test_hotkey, cx + 364, by, 100, 32);

        self.regions = Regions { layer, banner, cards: [r(rx, top, rw, monitors_h), r(rx, hy, rw, hotkeys_h)], name_frame };

        // The last column fills whatever width is left (exactly, so there's no horizontal scroll bar).
        fill_last_column(c.monitors, 5);
        fill_last_column(c.hotkeys, 2);
        unsafe { RedrawWindow(self.hwnd, core::ptr::null(), 0, RDW_INVALIDATE | RDW_ERASE) };
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
        theme::text(hdc, "\u{E7F4}", icon, self.fonts.icon, if connected { p.accent } else { p.text2 }, DT_VCENTER | DT_SINGLELINE);
        let tx = pill.left + sc(40);
        let mid = (pill.top + pill.bottom) / 2;
        theme::text(hdc, title, RECT { left: tx, top: pill.top + sc(4), right: pill.right - sc(8), bottom: mid + sc(1) }, self.fonts.normal, p.text, DT_SINGLELINE | DT_END_ELLIPSIS | 0x0008 /* DT_BOTTOM */);
        theme::text(hdc, subtitle, RECT { left: tx, top: mid + sc(1), right: pill.right - sc(8), bottom: pill.bottom - sc(4) }, self.fonts.normal, p.text2, DT_SINGLELINE | DT_END_ELLIPSIS);
    }

    /// Re-applies the palette after Windows switches between light and dark (or the accent changes).
    fn retheme(&mut self) {
        widgets::set_checked(self.c.dark, theme::palette().dark);
        theme::apply_window(self.hwnd);
        theme::apply_control(self.c.monitors, theme::ControlKind::ListView);
        theme::apply_control(self.c.hotkeys, theme::ControlKind::ListView);
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
            ui::lv_clear(self.c.monitors);
            ui::lv_clear(self.c.hotkeys);
            self.update_hotkey_buttons();
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

    fn update_status(&self) {
        let Some(i) = self.sel else { return };
        let p = &self.config.profiles[i];
        let present = p.monitors.iter().filter(|m| self.is_connected(&m.monitor_id)).count();
        let colours = theme::palette();
        let (text, banner) = if self.is_active(i) {
            if p.enabled {
                (format!("✔  Connected now ({present} of {} monitors present). Hotkeys are active.", p.monitors.len()), colours.success)
            } else {
                (format!("⚠  Connected now ({present} of {} monitors present). Turn hotkeys on to use them.", p.monitors.len()), colours.caution)
            }
        } else {
            ("Not connected now. Inputs can only be detected and tested while this setup is connected.".to_string(), colours.neutral)
        };
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
            if !ui::confirm(self.hwnd, "Create another configuration?", &msg) {
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
        if !ui::confirm(self.hwnd, &format!("Delete \"{}\"?", p.name), &format!("The configuration and {hotkeys} will be removed.{note}")) {
            return;
        }
        self.config.profiles.remove(i);
        self.commit();
        self.refresh_profiles(None);
    }

    // -------------------------------------------------------------------------------- monitors

    fn refresh_monitors(&self) {
        ui::lv_clear(self.c.monitors);
        let Some(i) = self.sel else { return };
        for (row, m) in self.config.profiles[i].monitors.iter().enumerate() {
            let connected = self.is_connected(&m.monitor_id);
            let current = match self.current.get(&m.monitor_id.to_lowercase()) {
                Some(Some(c)) => monitors::short_name(*c),
                Some(None) => "no reply".into(),
                None => String::new(),
            };
            let inputs = if !m.supported_inputs.is_empty() {
                m.supported_inputs.iter().map(|&c| monitors::short_name(c)).collect::<Vec<_>>().join(", ")
            } else if connected {
                "not detected yet".into()
            } else {
                String::new()
            };
            ui::lv_add(self.c.monitors, &[m.name.clone(), m.model.clone(), m.serial.clone().unwrap_or_default(),
                (if connected { "Connected" } else { "Not connected" }).into(), current, inputs], row as isize);
        }
    }

    fn selected_monitor(&self) -> Option<usize> {
        ui::lv_selected(self.c.monitors).map(|r| ui::lv_param(self.c.monitors, r) as usize)
    }

    fn start_rename(&self) {
        let row = ui::lv_selected(self.c.monitors).unwrap_or(0);
        if ui::lv_count(self.c.monitors) > 0 {
            unsafe {
                SetFocus(self.c.monitors);
                SendMessageW(self.c.monitors, LVM_EDITLABELW, row as usize, 0);
            }
        }
    }

    fn finish_rename(&mut self, row: i32, text: String) -> bool {
        let text = text.trim().to_string();
        let Some(i) = self.sel else { return false };
        let index = ui::lv_param(self.c.monitors, row) as usize;
        if text.is_empty() || index >= self.config.profiles[i].monitors.len() {
            return false;
        }
        self.config.profiles[i].monitors[index].name = text;
        self.commit();
        self.refresh_hotkeys();
        true
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

    fn remove_monitor(&mut self) {
        let (Some(i), Some(index)) = (self.sel, self.selected_monitor()) else { return };
        let p = &self.config.profiles[i];
        let m = &p.monitors[index];
        let used = p.bindings.iter().filter(|b| b.monitor_id.eq_ignore_ascii_case(&m.monitor_id)).count();
        let also = match used { 0 => String::new(), 1 => " Its hotkey will be removed too.".into(), n => format!(" Its {n} hotkeys will be removed too.") };
        let msg = format!("Configurations are recognised by their monitors, so this changes which setups \"{}\" matches.{also}", p.name);
        if !ui::confirm(self.hwnd, &format!("Remove {}?", m.name), &msg) {
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

    fn refresh_hotkeys(&self) {
        ui::lv_clear(self.c.hotkeys);
        let Some(i) = self.sel else { return };
        let p = &self.config.profiles[i];
        for (row, b) in p.bindings.iter().enumerate() {
            let monitor = p.monitor(&b.monitor_id).map(|m| m.name.clone()).unwrap_or_else(|| "(removed monitor)".into());
            ui::lv_add(self.c.hotkeys, &[b.hotkey.display(), monitor, monitors::describe_action(&b.inputs)], row as isize);
        }
        self.update_hotkey_buttons();
    }

    /// Edit, Remove and Test are only available while a hotkey is selected.
    fn update_hotkey_buttons(&self) {
        let on = self.sel.is_some() && ui::lv_selected(self.c.hotkeys).is_some();
        for h in self.c.hotkey_actions() {
            ui::enable(h, on);
        }
    }

    fn selected_binding(&self) -> Option<usize> {
        ui::lv_selected(self.c.hotkeys).map(|r| ui::lv_param(self.c.hotkeys, r) as usize)
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

    fn edit_hotkey(&mut self) {
        let (Some(i), Some(index)) = (self.sel, self.selected_binding()) else { return };
        let existing: Binding = self.config.profiles[i].bindings[index].clone();
        let Some(b) = self.run_editor(i, Some(&existing)) else { return };
        self.config.profiles[i].bindings[index] = b;
        self.commit();
        self.refresh_hotkeys();
    }

    fn remove_hotkey(&mut self) {
        let (Some(i), Some(index)) = (self.sel, self.selected_binding()) else { return };
        let p = &self.config.profiles[i];
        let b = &p.bindings[index];
        let monitor = p.monitor(&b.monitor_id).map(|m| m.name.clone()).unwrap_or_else(|| "the monitor".into());
        let msg = format!("The hotkey for {monitor} ({}) will be removed.", monitors::describe_action(&b.inputs));
        if !ui::confirm(self.hwnd, &format!("Remove {}?", b.hotkey.display()), &msg) {
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

    fn test_hotkey(&mut self) {
        let (Some(i), Some(index)) = (self.sel, self.selected_binding()) else { return };
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
            (ID_RENAME, BN_CLICKED) => self.start_rename(),
            (ID_DETECT, BN_CLICKED) => self.detect(true),
            (ID_ADD_CONNECTED, BN_CLICKED) => self.add_connected(),
            (ID_REMOVE_MONITOR, BN_CLICKED) => self.remove_monitor(),
            (ID_ADD_HOTKEY, BN_CLICKED) => self.add_hotkey(),
            (ID_EDIT_HOTKEY, BN_CLICKED) => self.edit_hotkey(),
            (ID_REMOVE_HOTKEY, BN_CLICKED) => self.remove_hotkey(),
            (ID_TEST_HOTKEY, BN_CLICKED) => self.test_hotkey(),
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

    fn notify(&mut self, l: LPARAM) -> Option<LRESULT> {
        let hdr = unsafe { &*(l as *const NMHDR) };
        match (hdr.idFrom as i32, hdr.code) {
            (ID_MONITORS, LVN_ENDLABELEDITW) => {
                let info = unsafe { &*(l as *const NMLVDISPINFOW) };
                if info.item.pszText.is_null() {
                    return Some(0);
                }
                let text = unsafe {
                    let mut n = 0;
                    while *info.item.pszText.add(n) != 0 { n += 1; }
                    String::from_utf16_lossy(std::slice::from_raw_parts(info.item.pszText, n))
                };
                Some(self.finish_rename(info.item.iItem, text) as LRESULT)
            }
            (ID_HOTKEYS, NM_DBLCLK) => {
                self.edit_hotkey();
                Some(0)
            }
            (ID_HOTKEYS, LVN_ITEMCHANGED) => {
                self.update_hotkey_buttons();
                None
            }
            _ => None,
        }
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