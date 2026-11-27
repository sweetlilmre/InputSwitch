//! The tray icon and the app's core: watches for display changes, picks the configuration that
//! fits the connected monitors, and keeps its hotkeys registered.
//!
//! Other windows and background work send it [`Event`]s through its mailbox ([`post`]). Its state
//! is lent out one message at a time; menus and anything that destroys windows run outside that.

use crate::config::{self, AppConfig};
use crate::ffi::*;
use crate::monitors::{self, MonitorInfo};
use crate::ui::{self, dialog, IconSize, Mailbox, Slot, WM_MAIL};
use crate::{main_window, matcher, sys};
use std::sync::atomic::{AtomicU32, Ordering};

const CLASS: &str = "InputSwitch.Tray";
/// The notification area's messages about the icon.
const WM_TRAY: u32 = WM_APP + 1;
const TIMER_SETTLE: usize = 1;
/// Docking produces a burst of events as monitors come up one by one; wait for it to settle.
const SETTLE_MS: u32 = 2500;

/// What other windows and background work tell the tray.
pub enum Event {
    /// A hotkey's or the menu's switch failed (the message says why).
    SwitchFailed(String),
    /// The main window changed something: save it and apply it straight away.
    ConfigChanged(AppConfig),
    /// The hotkey editor opened (true) or closed (false): pause or resume the hotkeys.
    Capturing(bool),
    /// The main window closed: anything held back while it was open can happen now.
    MainWindowClosed,
}

static MAIL: Mailbox<Event> = Mailbox::new();
/// The message Explorer broadcasts when it restarts (the icon must be added again).
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

pub fn post(event: Event) {
    MAIL.post(event);
}

enum MenuAction {
    /// Run a hotkey's action. By id: the configuration can change while the menu is open.
    Run { profile: String, binding: String },
    SetInput(MonitorInfo, u8),
    Configure,
    Redetect,
    ToggleStartup,
    OpenFolder,
    Exit,
}

struct Tray {
    hwnd: HWND,
    config: AppConfig,
    connected: Vec<MonitorInfo>,
    /// The configuration that matches the connected monitors.
    active: Option<String>,
    /// The hotkey editor is open: hotkeys are paused meanwhile.
    capturing: bool,
    /// Registration failures last reported, so the same ones aren't reported on every change.
    last_failures: String,
    /// What clicking the current notification does.
    on_balloon_click: OnClick,
    /// The ids of the active configuration's registered hotkeys; hotkey id n is binding n - 1.
    hotkeys: Vec<i32>,
}

/// What clicking a notification does.
#[derive(Clone)]
enum OnClick {
    Nothing,
    /// Open the main window on this configuration (or the active one).
    Open(Option<String>),
}

thread_local! {
    static TRAY: Slot<Tray> = const { Slot::new() };
}

fn with<R>(f: impl FnOnce(&mut Tray) -> R) -> Option<R> {
    TRAY.with(|t| t.with(f))
}

pub fn run(open_main_window: bool) -> i32 {
    ui::register_class(CLASS, wndproc, false, 0);
    let hwnd = unsafe {
        // A hidden top-level window: message-only windows don't get display/device broadcasts.
        CreateWindowExW(0, wide(CLASS).as_ptr(), wide(ui::app_name()).as_ptr(), WS_OVERLAPPEDWINDOW, 0, 0, 0, 0, 0, 0, hinstance(), core::ptr::null())
    };
    MAIL.open(hwnd);
    TASKBAR_CREATED.store(unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) }, Ordering::Relaxed);

    let (config, error) = config::load();
    if let Some(e) = error {
        dialog::alert(0, dialog::Icon::Warning, "Your settings couldn't be read",
            &format!("{e}\n\nInputSwitch will start with fresh settings."));
    }
    let first_run = config.profiles.is_empty();
    ui::theme::set_dark(config.theme.dark());

    TRAY.with(|t| t.set(Tray {
        hwnd, config, connected: Vec::new(), active: None, capturing: false,
        last_failures: String::new(), on_balloon_click: OnClick::Nothing, hotkeys: Vec::new(),
    }));
    unsafe { WTSRegisterSessionNotification(hwnd, 0) };
    with(|t| {
        t.add_icon();
        t.evaluate(first_run);
        if open_main_window && !main_window::is_open() {
            t.open_on_active();
        }
    });

    let mut msg: MSG = zeroed();
    unsafe {
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            let s = main_window::hwnd();
            if s != 0 && IsDialogMessageW(s, &mut msg) != 0 {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    msg.wParam as i32
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_HOTKEY => {
            with(|t| t.on_hotkey(w as i32));
            return 0;
        }
        WM_TIMER if w == TIMER_SETTLE => {
            KillTimer(h, TIMER_SETTLE);
            with(|t| t.evaluate(false));
            return 0;
        }
        WM_DISPLAYCHANGE | WM_DEVICECHANGE => settle(h),
        WM_POWERBROADCAST if w == PBT_APMRESUMEAUTOMATIC || w == PBT_APMRESUMESUSPEND => settle(h),
        WM_WTSSESSION_CHANGE if w == WTS_SESSION_UNLOCK => settle(h),
        WM_TRAY => {
            match loword(l as usize) {
                NIN_SELECT | NIN_KEYSELECT => {
                    with(Tray::open_on_active);
                }
                CONTEXT_MENU => show_menu(h, menu_anchor(w)),
                NIN_BALLOONUSERCLICK => {
                    with(|t| {
                        if let OnClick::Open(target) = std::mem::replace(&mut t.on_balloon_click, OnClick::Nothing) {
                            t.open_main_window(target);
                        }
                    });
                }
                _ => {}
            }
            return 0;
        }
        WM_MAIL => {
            // Taken only once the tray is free, so none are lost (they wait for the next wake).
            with(|t| {
                for event in MAIL.take() {
                    t.on_event(event);
                }
            });
            return 0;
        }
        WM_SETTINGCHANGE if ui::theme::is_theme_change(m, l) => {
            ui::theme::reload(); // so the next window (and the menu) opens in the new mode
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            return 0;
        }
        _ if m != 0 && m == TASKBAR_CREATED.load(Ordering::Relaxed) => {
            with(|t| t.add_icon()); // Explorer restarted
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}

fn settle(h: HWND) {
    // SetTimer with an existing id restarts it.
    unsafe { SetTimer(h, TIMER_SETTLE, SETTLE_MS, core::ptr::null()) };
}

/// WM_CONTEXTMENU, as the low word of the icon's message (the notification-area version 4 way).
const CONTEXT_MENU: u16 = WM_CONTEXTMENU as u16;

/// Where to open the menu: the point the notification area gives (the icon, when it's opened
/// from the keyboard; the mouse otherwise).
fn menu_anchor(w: WPARAM) -> POINT {
    POINT { x: loword(w) as i16 as i32, y: hiword(w) as i16 as i32 }
}

fn show_menu(h: HWND, at: POINT) {
    let Some((menu, mut actions)) = with(|t| t.build_menu()) else { return };
    let cmd = unsafe {
        // The documented dance so the menu closes when the user clicks elsewhere.
        SetForegroundWindow(h);
        let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, at.x, at.y, 0, h, core::ptr::null());
        PostMessageW(h, WM_NULL, 0, 0);
        DestroyMenu(menu);
        cmd
    };
    if cmd <= 0 {
        return; // dismissed
    }
    let action = actions.swap_remove(cmd as usize - 1);
    if matches!(action, MenuAction::Exit) {
        exit(h);
    } else {
        with(|t| t.run_menu(action));
    }
}

fn exit(h: HWND) {
    main_window::destroy();
    with(|t| {
        t.unregister_hotkeys();
        t.remove_icon();
        unsafe {
            WTSUnRegisterSessionNotification(t.hwnd);
        }
    });
    unsafe { DestroyWindow(h) };
}

impl Tray {
    // -------------------------------------------------------------------------------- setup detection

    /// Re-reads the connected monitors and activates the matching configuration, creating one
    /// for a new setup. While the main window is open, new configurations and setup notifications
    /// wait until it closes, so its list doesn't change under the user.
    fn evaluate(&mut self, interactive: bool) {
        self.connected = monitors::enumerate();
        let fp = matcher::fingerprint(&self.connected);
        let mut index = matcher::find(&self.config, &fp, self.active.as_deref());

        let mut created = None;
        if index.is_none() && !fp.is_empty() && !main_window::is_open() {
            let p = matcher::create_for(&self.connected, &self.config);
            created = Some((p.id.clone(), p.name.clone(), describe_monitors(&p)));
            self.config.profiles.push(p);
            self.save();
            index = Some(self.config.profiles.len() - 1);
        }

        let changed = self.set_active(index);
        if main_window::is_open() && !changed {
            main_window::update_connection(self.connected.clone(), self.active.clone()); // new monitors, same setup
        }
        self.register_hotkeys(changed);
        self.update_tip();

        if let Some((id, name, what)) = created {
            if interactive {
                self.open_main_window(Some(id));
            } else {
                self.notify("New display setup detected",
                    &format!("Created \"{name}\" for {what}. Click to name it and assign hotkeys."), NIIF_INFO, OnClick::Open(Some(id)));
            }
        } else if changed && !main_window::is_open() {
            if let Some(p) = self.active_profile() {
                let text = match (p.enabled, self.hotkeys.len()) {
                    (false, _) => "This configuration is disabled. Click to configure.".into(),
                    (true, 1) => "1 hotkey active.".into(),
                    (true, n) => format!("{n} hotkeys active."),
                };
                let (title, id) = (format!("Display setup: {}", p.name), p.id.clone());
                self.notify(&title, &text, NIIF_NONE, OnClick::Open(Some(id)));
            }
        } else if interactive && self.active.is_none() {
            self.open_main_window(None);
        }
    }

    /// Makes configuration index the active one; tells the main window. True if it changed.
    fn set_active(&mut self, index: Option<usize>) -> bool {
        let new_active = index.map(|i| self.config.profiles[i].id.clone());
        if new_active == self.active {
            return false;
        }
        self.active = new_active;
        if main_window::is_open() {
            main_window::update_connection(self.connected.clone(), self.active.clone());
        }
        true
    }

    fn active_profile(&self) -> Option<&config::Profile> {
        let id = self.active.as_deref()?;
        self.config.profiles.iter().find(|p| p.id == id)
    }

    fn save(&mut self) {
        if let Err(e) = config::save(&self.config) {
            self.notify("Could not save configuration", &e.to_string(), NIIF_ERROR, OnClick::Nothing);
        }
    }

    fn on_event(&mut self, ev: Event) {
        match ev {
            Event::SwitchFailed(msg) => self.notify("Input switch failed", &msg, NIIF_WARNING, OnClick::Nothing),
            Event::ConfigChanged(cfg) => {
                self.config = cfg;
                ui::theme::set_dark(self.config.theme.dark());
                self.save();
                // Configurations may have been added, deleted or changed: keep the same one active
                // if it still fits.
                let index = matcher::find(&self.config, &matcher::fingerprint(&self.connected), self.active.as_deref());
                self.set_active(index);
                self.register_hotkeys(true);
                self.update_tip();
            }
            Event::Capturing(on) => {
                self.capturing = on;
                self.register_hotkeys(!on);
            }
            Event::MainWindowClosed => {
                self.capturing = false;
                self.evaluate(false); // creates a configuration if the setup is now unknown
            }
        }
    }

    // -------------------------------------------------------------------------------- hotkeys

    fn unregister_hotkeys(&mut self) {
        for id in self.hotkeys.drain(..) {
            unsafe { UnregisterHotKey(self.hwnd, id) };
        }
    }

    fn register_hotkeys(&mut self, report: bool) {
        self.unregister_hotkeys();
        if self.capturing {
            return;
        }
        let Some(p) = self.active_profile() else { return };
        if !p.enabled {
            return;
        }
        let mut failed = Vec::new();
        let mut registered = Vec::new();
        for (i, b) in p.bindings.iter().enumerate() {
            if !b.hotkey.is_valid() {
                continue;
            }
            let id = i as i32 + 1;
            if unsafe { RegisterHotKey(self.hwnd, id, b.hotkey.mods as u32 | MOD_NOREPEAT, b.hotkey.vk as u32) } != 0 {
                registered.push(id);
            } else {
                failed.push(b.hotkey.display());
            }
        }
        let target = p.id.clone();
        self.hotkeys = registered;
        let failures = failed.join(", ");
        let news = failures != self.last_failures;
        self.last_failures = failures.clone();
        if report && news && !failures.is_empty() {
            self.notify("Some hotkeys could not be registered",
                &format!("{failures} — probably in use by another application."), NIIF_WARNING, OnClick::Open(Some(target)));
        }
    }

    fn on_hotkey(&mut self, id: i32) {
        if self.hotkeys.contains(&id) {
            self.run_binding(id as usize - 1);
        }
    }

    fn run_binding(&mut self, index: usize) {
        let Some(p) = self.active_profile() else { return };
        let Some(b) = p.bindings.get(index) else { return };
        let monitor = self.connected.iter().find(|m| m.id.eq_ignore_ascii_case(&b.monitor_id)).cloned();
        let Some(monitor) = monitor else {
            let name = p.monitor(&b.monitor_id).map(|m| m.name.clone()).unwrap_or_else(|| "The monitor".into());
            self.notify("Monitor not connected", &format!("{name} is not currently connected to this computer."), NIIF_WARNING, OnClick::Nothing);
            return;
        };
        let inputs = b.inputs.clone();
        std::thread::spawn(move || {
            if let Err(e) = monitors::run_binding(&inputs, &monitor) {
                post(Event::SwitchFailed(e));
            }
        });
    }

    // -------------------------------------------------------------------------------- settings

    /// Opens the main window on the active configuration, or brings it forward and selects that.
    fn open_on_active(&mut self) {
        let active = self.active.clone();
        self.open_main_window(active);
    }

    /// Opens the main window on configuration `select` (or the active one), or brings it forward.
    fn open_main_window(&mut self, select: Option<String>) {
        if main_window::is_open() {
            main_window::select_and_focus(select);
            return;
        }
        let select = select.or_else(|| self.active.clone());
        main_window::open(self.config.clone(), self.connected.clone(), self.active.clone(), select.as_deref());
    }

    // -------------------------------------------------------------------------------- tray icon

    fn icon_data(&self) -> NOTIFYICONDATAW {
        let mut d: NOTIFYICONDATAW = zeroed();
        d.cbSize = core::mem::size_of::<NOTIFYICONDATAW>() as u32;
        d.hWnd = self.hwnd;
        d.uID = 1;
        d
    }

    fn add_icon(&self) {
        let mut d = self.icon_data();
        d.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        d.uCallbackMessage = WM_TRAY;
        d.hIcon = ui::app_icon(IconSize::Small);
        copy_wide(&mut d.szTip, &self.tip());
        unsafe {
            Shell_NotifyIconW(NIM_ADD, &d);
            d.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &d);
        }
    }

    fn remove_icon(&self) {
        let d = self.icon_data();
        unsafe { Shell_NotifyIconW(NIM_DELETE, &d) };
    }

    fn tip(&self) -> String {
        match self.active_profile() {
            None => "InputSwitch — no configuration".into(),
            Some(p) => format!("InputSwitch — {}{}", p.name, if p.enabled { "" } else { " (disabled)" }),
        }
    }

    fn update_tip(&self) {
        let mut d = self.icon_data();
        d.uFlags = NIF_TIP | NIF_SHOWTIP;
        copy_wide(&mut d.szTip, &self.tip());
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &d) };
    }

    /// Shows a notification. The "notify when the setup changes" option silences only routine
    /// ones (NIIF_NONE); problems and new setups always show.
    fn notify(&mut self, title: &str, text: &str, kind: u32, on_click: OnClick) {
        if !self.config.show_notifications && kind == NIIF_NONE {
            return;
        }
        self.on_balloon_click = on_click;
        let mut d = self.icon_data();
        d.uFlags = NIF_INFO;
        d.dwInfoFlags = kind;
        copy_wide(&mut d.szInfoTitle, title);
        copy_wide(&mut d.szInfo, text);
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &d) };
    }

    // -------------------------------------------------------------------------------- menu

    fn build_menu(&self) -> (HMENU, Vec<MenuAction>) {
        let mut actions = Vec::new();
        // Appends an item; its command id is its action's position (from 1), or 0 for none.
        let mut add = |menu: HMENU, flags: u32, text: &str, action: Option<MenuAction>| {
            let id = action.map_or(0, |a| {
                actions.push(a);
                actions.len()
            });
            unsafe { AppendMenuW(menu, flags, id, wide(text).as_ptr()) };
            id
        };
        let menu = unsafe { CreatePopupMenu() };
        let active = self.active_profile();

        let header = match active {
            None if matcher::fingerprint(&self.connected).is_empty() => "No external monitors".to_string(),
            None => "Unknown display setup".to_string(),
            Some(p) => format!("Setup: {}{}", p.name, if p.enabled { "" } else { " (disabled)" }),
        };
        add(menu, MF_STRING | MF_GRAYED, &header, None);

        if let Some(p) = active {
            for b in &p.bindings {
                let monitor = p.monitor(&b.monitor_id).map_or("?", |m| m.name.as_str());
                let text = format!("{monitor}: {}\t{}", monitors::describe_action(&b.inputs), b.hotkey.display());
                add(menu, MF_STRING, &text, Some(MenuAction::Run { profile: p.id.clone(), binding: b.id.clone() }));
            }
        }

        let externals: Vec<&MonitorInfo> = self.connected.iter().filter(|m| !m.internal).collect();
        if !externals.is_empty() {
            add(menu, MF_SEPARATOR, "", None);
            for m in externals {
                let known = active.and_then(|p| p.monitor(&m.id));
                let name = known.map_or_else(|| m.display_name().to_string(), |k| k.name.clone());
                let inputs = match known {
                    Some(k) if !k.supported_inputs.is_empty() => k.supported_inputs.clone(),
                    _ => monitors::COMMON_INPUTS.to_vec(),
                };
                let sub = unsafe { CreatePopupMenu() };
                for code in inputs {
                    add(sub, MF_STRING, &monitors::long_name(code), Some(MenuAction::SetInput(m.clone(), code)));
                }
                unsafe { AppendMenuW(menu, MF_POPUP, sub as usize, wide(&format!("Switch {name} to")).as_ptr()) };
            }
        }

        add(menu, MF_SEPARATOR, "", None);
        let configure = add(menu, MF_STRING, "Configure…", Some(MenuAction::Configure));
        unsafe { SetMenuDefaultItem(menu, configure as u32, 0) };
        add(menu, MF_STRING, "Re-detect displays", Some(MenuAction::Redetect));
        let startup = if sys::startup_enabled() { MF_CHECKED } else { 0 };
        add(menu, MF_STRING | startup, "Start with Windows", Some(MenuAction::ToggleStartup));
        add(menu, MF_STRING, "Open config folder", Some(MenuAction::OpenFolder));
        add(menu, MF_SEPARATOR, "", None);
        add(menu, MF_STRING | MF_GRAYED, &format!("InputSwitch {}", sys::VERSION), None);
        add(menu, MF_STRING, "Exit", Some(MenuAction::Exit));
        (menu, actions)
    }

    fn run_menu(&mut self, action: MenuAction) {
        match action {
            MenuAction::Run { profile, binding } => {
                let index = self.active_profile().filter(|p| p.id == profile).and_then(|p| p.bindings.iter().position(|b| b.id == binding));
                if let Some(index) = index {
                    self.run_binding(index);
                }
            }
            MenuAction::SetInput(m, code) => {
                std::thread::spawn(move || {
                    if let Err(e) = monitors::switch_to(&m, code) {
                        post(Event::SwitchFailed(e));
                    }
                });
            }
            MenuAction::Configure => self.open_on_active(),
            MenuAction::Redetect => self.evaluate(false),
            MenuAction::ToggleStartup => {
                if !sys::set_startup(!sys::startup_enabled()) {
                    self.notify("Couldn't change Start with Windows", "The startup setting couldn't be written to the registry.", NIIF_WARNING, OnClick::Nothing);
                }
                main_window::startup_changed();
            }
            MenuAction::OpenFolder => {
                let dir = config::dir();
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::process::Command::new("explorer.exe").arg(dir).spawn();
            }
            MenuAction::Exit => {}
        }
    }
}

/// The monitor's name, or "2 monitors".
fn describe_monitors(p: &config::Profile) -> String {
    if p.monitors.len() == 1 { p.monitors[0].name.clone() } else { format!("{} monitors", p.monitors.len()) }
}
