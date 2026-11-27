//! The main window: configurations on the left; the selected one's monitors and hotkeys on the
//! right. Every change is saved and applied straight away: the window posts a copy of the
//! configuration to the tray app, which saves it and re-registers the hotkeys. Typed names are
//! committed shortly after typing stops, and on close.
//!
//! - [`layout`]: the controls, where they go, and what's painted behind them
//! - [`profiles`], [`monitors`], [`hotkeys`]: what each part of the window does
//! - [`monitor_cards`], [`hotkey_cards`]: how the two lists of cards look
//!
//! Anything from outside the window's own messages (the tray, background work, theme and DPI
//! changes) arrives in its [`Mailbox`] and is applied when the window is free, so nothing is lost
//! while a dialog is open.

mod hotkey_cards;
mod hotkeys;
mod layout;
mod monitor_cards;
mod monitors;
mod profiles;

use crate::config::AppConfig;
use crate::ffi::*;
use crate::i18n::{self, t};
use crate::monitors::MonitorInfo;
use crate::ui::{self, theme, widgets, Fonts, Mailbox, Slot, WM_MAIL};
use crate::{settings_window, sys, tray};
use layout::{Controls, Regions};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, Ordering};
use theme::Severity;

const CLASS: &str = "InputSwitch.MainWindow";
/// Commits typed names once typing pauses for TYPING_MS, rather than on every keystroke.
const TIMER_TYPING: usize = 1;
const TYPING_MS: u32 = 700;

/// Something for the window to apply when it's free.
enum Update {
    /// The tray re-read the connected monitors (or the active configuration changed).
    Connection { connected: Vec<MonitorInfo>, active: Option<String> },
    /// Show this configuration (from a tray click or a notification).
    Select(Option<String>),
    /// Windows switched between light and dark (or the accent colour changed).
    Theme,
    /// The window moved to a monitor with another DPI.
    Dpi { dpi: u32, suggested: RECT },
    /// Background work read a monitor's inputs and its current input.
    Detected { id: String, inputs: Vec<u8>, current: Option<u8> },
    /// Background work has read every monitor it was asked to.
    DetectDone,
    /// A hotkey's Test button switched a monitor (or failed to).
    Tested { id: String, result: Result<u8, String> },
}

static MAIL: Mailbox<Update> = Mailbox::new();
static HWND_MAIN: AtomicIsize = AtomicIsize::new(0);

/// The window's state, kept in [`STATE`] while it's open.
struct MainWindow {
    hwnd: HWND,
    dpi: u32,
    fonts: Rc<Fonts>,
    c: Controls,
    regions: Regions,
    /// How the status banner is tinted.
    status: Severity,
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
    /// Inputs are being read in the background.
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

/// The window, or 0 if it isn't open.
pub fn hwnd() -> HWND {
    HWND_MAIN.load(Ordering::Relaxed)
}

/// Whether the window is open.
pub fn is_open() -> bool {
    hwnd() != 0
}

/// Opens the window on `config`, showing configuration `select`. `connected` and `active` are
/// the monitors connected now and the configuration that matches them.
pub fn open(config: AppConfig, connected: Vec<MonitorInfo>, active: Option<String>, select: Option<&str>) {
    ui::register_class(CLASS, wndproc, true, 0);
    let hwnd = layout::create_window(CLASS);
    HWND_MAIN.store(hwnd, Ordering::Relaxed);
    MAIL.open(hwnd);
    let dpi = ui::dpi_of(hwnd);
    let fonts = Rc::new(Fonts::new(dpi));
    layout::set_list_fonts(&fonts);
    let c = layout::create_controls(hwnd, &fonts);
    STATE.with(|s| s.set(MainWindow {
        hwnd, dpi, fonts, c, regions: Regions::default(), status: Severity::Informational, config, connected, active, sel: None,
        current: HashMap::new(), attempted: HashSet::new(), detecting: false, detect_queued: None, typing: false,
    }));
    with(|w| {
        w.refresh_profiles(select);
        w.layout();
    });
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
}

/// The connected monitors (or the configuration that matches them) changed.
pub fn update_connection(connected: Vec<MonitorInfo>, active: Option<String>) {
    MAIL.post(Update::Connection { connected, active });
}

/// Brings the window forward (or the hotkey editor, if it's open) and shows configuration `id`.
pub fn select_and_focus(id: Option<String>) {
    if id.is_some() {
        MAIL.post(Update::Select(id));
    }
    if !is_open() {
        return;
    }
    let main = hwnd();
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
    if is_open() {
        unsafe { DestroyWindow(hwnd()) };
    }
}

// ------------------------------------------------------------------------------------------ messages

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_COMMAND => {
            if hiword(w) == BN_CLICKED && l != 0 {
                widgets::clicked(l as HWND); // checkboxes flip before the click is handled
            }
            if loword(w) as i32 == IDOK {
                ui::click_focused_button(h); // Enter on a button presses it
            } else if with(|s| s.command(loword(w) as i32, hiword(w))) == Some(true) {
                DestroyWindow(h);
            }
            MAIL.wake_if_waiting(); // mail that arrived while a dialog was open from this click
            return 0;
        }
        WM_CLOSE => {
            with(|s| s.commit_pending());
            DestroyWindow(h);
            return 0;
        }
        WM_ACTIVATE => {
            ui::keep_focus(h, w, 0);
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
            MAIL.post(Update::Dpi { dpi: hiword(w) as u32, suggested: *(l as *const RECT) });
            return 0;
        }
        WM_ERASEBKGND => {
            layout::paint_background(h, w as HDC);
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
                mi.itemHeight = layout::profile_item_height(ui::dpi_of(h)) as u32;
                return 1;
            }
        }
        WM_DRAWITEM => {
            let dis = &*(l as *const DRAWITEMSTRUCT);
            if widgets::draw(dis) {
                return 1;
            }
            if dis.CtlType == ODT_LISTBOX {
                layout::draw_profile(dis); // needs no state, so it draws even while a dialog is open
                return 1;
            }
        }
        WM_SETTINGCHANGE if theme::is_theme_change(m, l) => MAIL.post(Update::Theme),
        WM_MAIL => {
            with(|s| s.apply_updates());
            return 0;
        }
        WM_TIMER if w == TIMER_TYPING => {
            with(|s| s.commit());
            return 0;
        }
        WM_DESTROY => {
            HWND_MAIN.store(0, Ordering::Relaxed);
            MAIL.close();
            STATE.with(|s| s.take());
            layout::forget_background();
            tray::post(tray::Event::MainWindowClosed);
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}

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
const ID_SETTINGS: i32 = 110;

impl MainWindow {
    /// A control's notification. Returns true if the window should close.
    fn command(&mut self, id: i32, code: u16) -> bool {
        if let Some(action) = monitor_cards::action(id, code) {
            self.monitor_action(action);
            return false;
        }
        if let Some(action) = hotkey_cards::action(id, code) {
            self.hotkey_action(action);
            return false;
        }
        match (id, code) {
            (ID_PROFILES, LBN_SELCHANGE) => self.select_from_list(),
            (ID_NEW, BN_CLICKED) => self.new_from_current(),
            (ID_DELETE, BN_CLICKED) => self.delete_profile(),
            (ID_NAME, EN_CHANGE) => {
                self.c.name.on_command(code);
                self.rename_profile();
            }
            (ID_NAME, EN_SETFOCUS | EN_KILLFOCUS) => {
                self.c.name.on_command(code);
                self.repaint_name_frame();
            }
            (ID_ENABLED, BN_CLICKED) => self.toggle_enabled(),
            (ID_DETECT, BN_CLICKED) => self.detect(true),
            (ID_ADD_CONNECTED, BN_CLICKED) => self.add_connected(),
            (ID_ADD_HOTKEY, BN_CLICKED) => self.add_hotkey(),
            (ID_SETTINGS, BN_CLICKED) => self.open_settings(),
            (IDCANCEL, _) => {
                self.commit_pending();
                return true;
            }
            _ => {}
        }
        false
    }

    /// Applies everything that arrived in the mailbox, then saves once if detection changed anything.
    fn apply_updates(&mut self) {
        let mut inputs_changed = false;
        for update in MAIL.take() {
            match update {
                Update::Connection { connected, active } => {
                    self.connected = connected;
                    self.active = active;
                    let id = self.selected_id();
                    self.refresh_profiles(id.as_deref());
                }
                Update::Select(id) => self.refresh_profiles(id.as_deref()),
                Update::Theme => {
                    theme::reload();
                    self.retheme();
                }
                Update::Dpi { dpi, suggested } => self.dpi_changed(dpi, suggested),
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
            p.name = t("profile.unnamed").into();
            pending = true;
        }
        if pending {
            self.commit();
        }
    }

    // -------------------------------------------------------------------------------- settings

    /// Opens the Settings dialog; each change in it is applied and saved as it's made.
    fn open_settings(&mut self) {
        let config = self.config.clone();
        settings_window::run(self.hwnd, self.dpi, &config, &mut |change| self.apply_setting(change));
    }

    fn apply_setting(&mut self, change: settings_window::Change) {
        use settings_window::Change;
        match change {
            Change::Language(code) => {
                self.config.language = code.into();
                i18n::set_language(code);
                self.show_texts();
            }
            Change::Theme(choice) => {
                self.config.theme = choice;
                theme::set_dark(choice.dark()); // re-theme straight away; the tray sets it again when it saves
                self.retheme();
            }
            Change::Notify(on) => self.config.show_notifications = on,
        }
        self.commit();
    }

    /// Shows everything in the current language.
    fn show_texts(&mut self) {
        self.c.set_texts();
        let id = self.selected_id();
        self.refresh_profiles(id.as_deref());
        ui::set_text(self.c.activity, "");
        self.layout();
        theme::repaint(self.hwnd);
    }
}
