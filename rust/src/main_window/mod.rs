//! The main window: configurations on the left; the selected one's monitors and hotkeys on the
//! right. Every change is saved and applied straight away: the window posts a copy of the
//! configuration to the tray app, which saves it and re-registers the hotkeys. Typed names are
//! committed shortly after typing stops, and on close.
//!
//! - [`layout`]: the controls, where they go, and what's painted behind them
//! - [`profiles`], [`monitors`], [`hotkeys`]: what each part of the window does
//! - [`monitor_cards`], [`hotkey_cards`]: how the two lists of cards look
//!
//! Anything from outside the window's own messages (other windows, background work) arrives
//! through [`post`] and is applied when the window is free, so nothing is lost while a dialog
//! is open.

mod hotkey_cards;
mod hotkeys;
mod layout;
mod monitor_cards;
mod monitors;
mod profiles;

use crate::config::{AppConfig, Theme};
use crate::ffi::*;
use crate::monitors::MonitorInfo;
use crate::ui::{self, theme, widgets, Fonts, Slot};
use crate::{sys, tray};
use layout::{Controls, Regions, StatusIcon};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;

const CLASS: &str = "InputSwitch.MainWindow";
const WM_UPDATE: u32 = WM_APP + 2;
/// Commits typed names once typing pauses, rather than on every keystroke.
const TIMER_TYPING: usize = 1;
const TYPING_MS: u32 = 700;

/// Something for the window to apply when it's free.
enum Update {
    /// The tray re-read the connected monitors (or the active configuration changed).
    Connection { connected: Vec<MonitorInfo>, active: Option<String> },
    /// Show this configuration (from a tray click or a notification).
    Select(Option<String>),
    /// Background work: a monitor's inputs were read, all of them were, or a test switched one.
    Detected { id: String, inputs: Vec<u8>, current: Option<u8> },
    DetectDone,
    Tested { id: String, result: Result<u8, String> },
}

static UPDATES: Mutex<Vec<Update>> = Mutex::new(Vec::new());
static HWND_MAIN: AtomicIsize = AtomicIsize::new(0);

fn post(update: Update) {
    UPDATES.lock().unwrap_or_else(|e| e.into_inner()).push(update);
    poke();
}

/// Asks the window to apply the queued updates.
fn poke() {
    let h = hwnd();
    if h != 0 {
        unsafe { PostMessageW(h, WM_UPDATE, 0, 0) };
    }
}

fn take_updates() -> Vec<Update> {
    std::mem::take(&mut *UPDATES.lock().unwrap_or_else(|e| e.into_inner()))
}

fn updates_waiting() -> bool {
    !UPDATES.lock().unwrap_or_else(|e| e.into_inner()).is_empty()
}

struct MainWindow {
    hwnd: HWND,
    dpi: u32,
    fonts: Fonts,
    c: Controls,
    regions: Regions,
    status_icon: StatusIcon,
    config: AppConfig,
    connected: Vec<MonitorInfo>,
    /// The configuration matching the connected monitors.
    active: Option<String>,
    /// The configuration shown (index into `config.profiles`).
    sel: Option<usize>,
    /// Each monitor's current input (lower-case id → input), as last read.
    current: HashMap<String, Option<u8>>,
    /// Monitors whose inputs have been read this session (lower-case ids).
    attempted: HashSet<String>,
    detecting: bool,
    /// A detection asked for while one was running (its `force`); it runs when that one finishes.
    detect_queued: Option<bool>,
    /// A typed name is waiting to be committed (the typing timer is running).
    typing: bool,
}

thread_local! {
    static STATE: Slot<MainWindow> = const { Slot::new() };
}

fn with<R>(f: impl FnOnce(&mut MainWindow) -> R) -> Option<R> {
    STATE.with(|s| s.with(f))
}

// ------------------------------------------------------------------------------------------ public

pub fn hwnd() -> HWND {
    HWND_MAIN.load(Ordering::Relaxed)
}

pub fn open(config: AppConfig, connected: Vec<MonitorInfo>, active: Option<String>, select: Option<String>) {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| ui::register_class(CLASS, wndproc, ui::app_icon()));

    let hwnd = layout::create_window(CLASS);
    HWND_MAIN.store(hwnd, Ordering::Relaxed);
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let fonts = Fonts::new(dpi);
    let c = layout::create_controls(hwnd, &fonts, dpi);
    STATE.with(|s| s.set(MainWindow {
        hwnd, dpi, fonts, c, regions: Regions::default(), status_icon: StatusIcon::Info, config, connected, active, sel: None,
        current: HashMap::new(), attempted: HashSet::new(), detecting: false, detect_queued: None, typing: false,
    }));
    with(|w| {
        widgets::set_checked(w.c.startup, sys::startup_enabled());
        widgets::set_checked(w.c.notify, w.config.show_notifications);
        widgets::set_checked(w.c.dark, theme::palette().dark);
        w.refresh_profiles(select.as_deref());
        w.layout();
    });
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
}

pub fn update_connection(connected: Vec<MonitorInfo>, active: Option<String>) {
    post(Update::Connection { connected, active });
}

/// Brings the window forward (or the hotkey editor, if it's open) and shows configuration `id`.
pub fn select_and_focus(id: Option<String>) {
    if id.is_some() {
        post(Update::Select(id));
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

// ------------------------------------------------------------------------------------------ messages

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_COMMAND => {
            if hiword(w) == BN_CLICKED && l != 0 {
                widgets::clicked(l as HWND); // checkboxes flip before the click is handled
            }
            if with(|s| s.command(loword(w) as i32, hiword(w))) == Some(Some(Close)) {
                DestroyWindow(h);
            }
            // Updates that arrived while a dialog was open from this click are applied now.
            if updates_waiting() {
                poke();
            }
            return 0;
        }
        WM_CLOSE => {
            with(|s| s.commit_pending());
            DestroyWindow(h);
            return 0;
        }
        WM_SIZE => {
            with(|s| s.layout());
            return 0;
        }
        WM_GETMINMAXINFO => {
            (*(l as *mut MINMAXINFO)).ptMinTrackSize = layout::min_window_size(h);
            return 0;
        }
        WM_DPICHANGED => {
            let suggested = *(l as *const RECT);
            with(|s| s.dpi_changed(hiword(w) as u32, suggested));
            return 0;
        }
        WM_ERASEBKGND => {
            if with(|s| s.paint_background(w as HDC)).is_none() {
                ui::draw::fill(w as HDC, ui::draw::client_rect(h), theme::palette().window);
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
                mi.itemHeight = layout::profile_item_height(GetDpiForWindow(h)) as u32;
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
        WM_UPDATE => {
            with(|s| s.apply_updates());
            return 0;
        }
        WM_TIMER if w == TIMER_TYPING => {
            with(|s| s.commit());
            return 0;
        }
        WM_DESTROY => {
            HWND_MAIN.store(0, Ordering::Relaxed);
            STATE.with(|s| s.take());
            take_updates(); // for a window that's gone
            tray::post(tray::Event::MainWindowClosed);
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}

/// What a command asks of the window.
#[derive(PartialEq)]
struct Close;

// Control ids.
const ID_PROFILES: i32 = 100;
const ID_NEW: i32 = 101;
const ID_DELETE: i32 = 102;
const ID_NAME: i32 = 103;
const ID_ENABLED: i32 = 104;
const ID_MONITORS: i32 = 105;
const ID_DETECT: i32 = 106;
const ID_ADD_CONNECTED: i32 = 107;
const ID_HOTKEYS: i32 = 108;
const ID_ADD_HOTKEY: i32 = 109;
const ID_STARTUP: i32 = 110;
const ID_NOTIFY: i32 = 111;
const ID_DARK: i32 = 112;

impl MainWindow {
    fn command(&mut self, id: i32, code: u16) -> Option<Close> {
        if let Some(action) = monitor_cards::action(id, code) {
            self.monitor_action(action);
            return None;
        }
        if let Some(action) = hotkey_cards::action(id, code) {
            self.hotkey_action(action);
            return None;
        }
        match (id, code) {
            (ID_PROFILES, LBN_SELCHANGE) => self.select_from_list(),
            (ID_NEW, BN_CLICKED) => self.new_from_current(),
            (ID_DELETE, BN_CLICKED) => self.delete_profile(),
            (ID_NAME, EN_CHANGE) => self.rename_profile(),
            (ID_ENABLED, BN_CLICKED) => self.toggle_enabled(),
            (ID_DETECT, BN_CLICKED) => self.detect(true),
            (ID_ADD_CONNECTED, BN_CLICKED) => self.add_connected(),
            (ID_ADD_HOTKEY, BN_CLICKED) => self.add_hotkey(),
            (ID_STARTUP, BN_CLICKED) => self.set_startup(),
            (ID_NOTIFY, BN_CLICKED) => {
                self.config.show_notifications = widgets::checked(self.c.notify);
                self.commit();
            }
            (ID_DARK, BN_CLICKED) => self.set_theme(if widgets::checked(self.c.dark) { Theme::Dark } else { Theme::Light }),
            (IDCANCEL, _) => {
                self.commit_pending();
                return Some(Close);
            }
            _ => {}
        }
        None
    }

    fn apply_updates(&mut self) {
        let mut inputs_changed = false;
        for update in take_updates() {
            match update {
                Update::Connection { connected, active } => {
                    self.connected = connected;
                    self.active = active;
                    let id = self.selected_id();
                    self.refresh_profiles(id.as_deref());
                }
                Update::Select(id) => self.refresh_profiles(id.as_deref()),
                Update::Detected { id, inputs, current } => inputs_changed |= self.on_detected(&id, inputs, current),
                Update::DetectDone => self.on_detect_done(),
                Update::Tested { id, result } => self.on_tested(&id, result),
            }
        }
        if inputs_changed {
            self.commit();
        }
    }

    // -------------------------------------------------------------------------------- saving

    /// Hands a copy of the configuration to the tray app, which saves and applies it.
    fn commit(&mut self) {
        unsafe { KillTimer(self.hwnd, TIMER_TYPING) };
        self.typing = false;
        tray::post(tray::Event::ConfigChanged(self.config.clone()));
    }

    /// Commits after a short pause (for text being typed).
    fn commit_soon(&mut self) {
        self.typing = true;
        unsafe { SetTimer(self.hwnd, TIMER_TYPING, TYPING_MS, core::ptr::null()) }; // restarts it
    }

    /// On the way out: commits anything still being typed, and names blank configurations.
    fn commit_pending(&mut self) {
        let mut pending = self.typing;
        for p in self.config.profiles.iter_mut().filter(|p| p.name.trim().is_empty()) {
            p.name = profiles::UNNAMED.into();
            pending = true;
        }
        if pending {
            self.commit();
        }
    }

    // -------------------------------------------------------------------------------- options

    fn set_startup(&mut self) {
        if !sys::set_startup(widgets::checked(self.c.startup)) {
            ui::dialog::alert(self.hwnd, ui::dialog::Icon::Warning, "Couldn't change Start with Windows", "The startup setting couldn't be written to the registry.");
            widgets::set_checked(self.c.startup, sys::startup_enabled());
        }
    }

    fn set_theme(&mut self, choice: Theme) {
        self.config.theme = choice;
        theme::set_choice(choice); // re-theme straight away; the tray sets it again when it saves
        self.retheme();
        self.commit();
    }
}
