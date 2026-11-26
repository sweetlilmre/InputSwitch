//! The tray icon and the app's core: watches for display changes, picks the configuration that
//! fits the connected monitors, and keeps its hotkeys registered.
//!
//! State lives in a thread-local RefCell and is only borrowed for short, non-reentrant work;
//! menus and anything that destroys windows run outside the borrow.

use crate::config::{self, AppConfig};
use crate::ffi::*;
use crate::monitors::{self, MonitorInfo};
use crate::{matcher, settings, sys, ui};
use std::cell::RefCell;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::Mutex;

const CLASS: &str = "InputSwitch.Tray";
const WM_TRAY: u32 = WM_APP + 1;
const WM_POKE: u32 = WM_APP + 2;
const TIMER_SETTLE: usize = 1;
/// Docking produces a burst of events as monitors come up one by one; wait for it to settle.
const SETTLE_MS: u32 = 2500;

/// Results from background work and other windows, delivered on the tray's thread.
pub enum Event {
    SwitchFailed(String),
    /// The main window changed something: save it and apply it straight away.
    ConfigChanged(AppConfig),
    /// The hotkey editor opened (true) or closed (false): pause or resume the hotkeys.
    Capturing(bool),
    SettingsClosed,
}

static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());
static HWND_TRAY: AtomicIsize = AtomicIsize::new(0);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

pub fn post(ev: Event) {
    EVENTS.lock().unwrap_or_else(|e| e.into_inner()).push(ev);
    unsafe { PostMessageW(HWND_TRAY.load(Ordering::Relaxed), WM_POKE, 0, 0) };
}

enum MenuAction {
    Run(usize),
    SetInput(MonitorInfo, u8),
    Configure,
    Redetect,
    ToggleStartup,
    OpenFolder,
    Exit,
}

struct Tray {
    hwnd: HWND,
    icon: HICON,
    config: AppConfig,
    connected: Vec<MonitorInfo>,
    active: Option<String>,
    settings_open: bool,
    /// The hotkey editor is open: hotkeys are paused meanwhile.
    capturing: bool,
    /// Registration failures last reported, so the same ones aren't reported on every change.
    last_failures: String,
    /// Profile to open when the current balloon is clicked (outer None: no action).
    balloon: Option<Option<String>>,
    /// Registered hotkey id → binding index in the active profile.
    hotkeys: Vec<(i32, usize)>,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Tray) -> R) -> Option<R> {
    TRAY.with(|t| t.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

pub fn run(open_settings: bool) -> i32 {
    let icon = ui::app_icon();
    ui::register_class(CLASS, wndproc, icon);
    let hwnd = unsafe {
        // A hidden top-level window: message-only windows don't get display/device broadcasts.
        CreateWindowExW(0, wide(CLASS).as_ptr(), wide(ui::TITLE).as_ptr(), WS_OVERLAPPEDWINDOW, 0, 0, 0, 0, 0, 0, hinstance(), core::ptr::null())
    };
    HWND_TRAY.store(hwnd, Ordering::Relaxed);
    TASKBAR_CREATED.store(unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) }, Ordering::Relaxed);

    let (config, error) = config::load();
    if let Some(e) = error {
        ui::alert(0, ui::Icon::Warning, "Your settings couldn't be read",
            &format!("{e}\n\nInputSwitch will start with fresh settings. The unreadable file was kept as config.json.bad in {}.", config::dir().display()));
    }
    let first_run = config.profiles.is_empty();
    crate::theme::set_choice(config.theme);

    TRAY.with(|t| {
        *t.borrow_mut() = Some(Tray {
            hwnd, icon, config, connected: Vec::new(), active: None, settings_open: false, capturing: false, last_failures: String::new(), balloon: None, hotkeys: Vec::new(),
        })
    });
    unsafe { WTSRegisterSessionNotification(hwnd, 0) };
    with(|t| {
        t.add_icon();
        t.evaluate(first_run);
        if open_settings && !t.settings_open {
            let active = t.active.clone();
            t.open_settings(active);
        }
    });

    let mut msg: MSG = zeroed();
    unsafe {
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            let s = settings::hwnd();
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
                    with(|t| {
                        let active = t.active.clone();
                        t.open_settings(active)
                    });
                }
                x if x as u32 == WM_CONTEXTMENU => show_menu(h),
                NIN_BALLOONUSERCLICK => {
                    with(|t| {
                        if let Some(target) = t.balloon.take() {
                            t.open_settings(target);
                        }
                    });
                }
                _ => {}
            }
            return 0;
        }
        WM_POKE => {
            let events: Vec<Event> = std::mem::take(&mut *EVENTS.lock().unwrap_or_else(|e| e.into_inner()));
            for ev in events {
                with(|t| t.on_event(ev));
            }
            return 0;
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

fn show_menu(h: HWND) {
    let Some((menu, actions)) = with(|t| t.build_menu()) else { return };
    let cmd = unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt);
        SetForegroundWindow(h);
        let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, pt.x, pt.y, 0, h, core::ptr::null());
        PostMessageW(h, WM_NULL, 0, 0);
        DestroyMenu(menu);
        cmd
    };
    if cmd <= 0 {
        return;
    }
    let mut actions = actions;
    let action = actions.swap_remove(cmd as usize - 1);
    if matches!(action, MenuAction::Exit) {
        exit(h);
    } else {
        with(|t| t.run_menu(action));
    }
}

fn exit(h: HWND) {
    settings::destroy();
    with(|t| {
        t.unregister_hotkeys();
        t.remove_icon();
        unsafe {
            WTSUnRegisterSessionNotification(t.hwnd);
            DestroyIcon(t.icon);
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
        if index.is_none() && !fp.is_empty() && !self.settings_open {
            let p = matcher::create_for(&self.connected, &self.config);
            created = Some((p.id.clone(), p.name.clone(), describe(&p)));
            self.config.profiles.push(p);
            self.save();
            index = Some(self.config.profiles.len() - 1);
        }

        let new_active = index.map(|i| self.config.profiles[i].id.clone());
        let changed = new_active != self.active;
        self.active = new_active;

        if self.settings_open {
            settings::update_connection(self.connected.clone(), self.active.clone());
        }
        self.register_hotkeys(changed);
        self.update_tip();

        if let Some((id, name, what)) = created {
            if interactive {
                self.open_settings(Some(id));
            } else {
                self.notify("New display setup detected",
                    &format!("Created \"{name}\" for {what}. Click to name it and assign hotkeys."), NIIF_INFO, Some(Some(id)));
            }
        } else if changed && !self.settings_open {
            if let Some(p) = self.active_profile() {
                let text = if p.enabled { format!("{} hotkey(s) active.", p.bindings.len()) } else { "This configuration is disabled. Click to configure.".into() };
                let (title, id) = (format!("Display setup: {}", p.name), p.id.clone());
                self.notify(&title, &text, NIIF_NONE, Some(Some(id)));
            }
        } else if interactive && self.active.is_none() {
            self.open_settings(None);
        }
    }

    fn active_profile(&self) -> Option<&config::Profile> {
        let id = self.active.as_deref()?;
        self.config.profiles.iter().find(|p| p.id == id)
    }

    fn save(&mut self) {
        if let Err(e) = config::save(&self.config) {
            self.notify("Could not save configuration", &e.to_string(), NIIF_ERROR, None);
        }
    }

    fn on_event(&mut self, ev: Event) {
        match ev {
            Event::SwitchFailed(msg) => self.notify("Input switch failed", &msg, NIIF_WARNING, None),
            Event::ConfigChanged(cfg) => {
                self.config = cfg;
                crate::theme::set_choice(self.config.theme);
                self.save();
                // Configurations may have been added, deleted or changed: keep the same one active
                // if it still fits.
                let index = matcher::find(&self.config, &matcher::fingerprint(&self.connected), self.active.as_deref());
                let new_active = index.map(|i| self.config.profiles[i].id.clone());
                if new_active != self.active {
                    self.active = new_active;
                    if self.settings_open {
                        settings::update_connection(self.connected.clone(), self.active.clone());
                    }
                }
                self.register_hotkeys(true);
                self.update_tip();
            }
            Event::Capturing(on) => {
                self.capturing = on;
                self.register_hotkeys(!on);
            }
            Event::SettingsClosed => {
                self.settings_open = false;
                self.capturing = false;
                self.evaluate(false); // creates a configuration if the setup is now unknown
            }
        }
    }

    // -------------------------------------------------------------------------------- hotkeys

    fn unregister_hotkeys(&mut self) {
        for (id, _) in self.hotkeys.drain(..) {
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
                registered.push((id, i));
            } else {
                failed.push(b.hotkey.display());
            }
        }
        let target = p.id.clone();
        self.hotkeys = registered;
        let failures = failed.join(", ");
        let news = failures != self.last_failures;
        self.last_failures = failures;
        if report && !failed.is_empty() && news {
            self.notify("Some hotkeys could not be registered",
                &format!("{} — probably in use by another application.", failed.join(", ")), NIIF_WARNING, Some(Some(target)));
        }
    }

    fn on_hotkey(&mut self, id: i32) {
        let Some(&(_, index)) = self.hotkeys.iter().find(|(h, _)| *h == id) else { return };
        self.run_binding(index);
    }

    fn run_binding(&mut self, index: usize) {
        let Some(p) = self.active_profile() else { return };
        let Some(b) = p.bindings.get(index) else { return };
        let monitor = self.connected.iter().find(|m| m.id.eq_ignore_ascii_case(&b.monitor_id)).cloned();
        let Some(monitor) = monitor else {
            let name = p.monitor(&b.monitor_id).map(|m| m.name.clone()).unwrap_or_else(|| "The monitor".into());
            self.notify("Monitor not connected", &format!("{name} is not currently connected to this computer."), NIIF_WARNING, None);
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

    fn open_settings(&mut self, select: Option<String>) {
        if self.settings_open {
            settings::select_and_focus(select);
            return;
        }
        self.settings_open = true;
        settings::open(self.config.clone(), self.connected.clone(), self.active.clone(), select.or(self.active.clone()));
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
        d.hIcon = self.icon;
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
    fn notify(&mut self, title: &str, text: &str, kind: u32, on_click: Option<Option<String>>) {
        if !self.config.show_notifications && kind == NIIF_NONE {
            return;
        }
        self.balloon = on_click;
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
        let mut add = |menu: HMENU, flags: u32, text: &str, action: Option<MenuAction>| unsafe {
            let id = match action {
                Some(a) => {
                    actions.push(a);
                    actions.len()
                }
                None => 0,
            };
            AppendMenuW(menu, flags, id, wide(text).as_ptr());
            id
        };

        unsafe {
            let menu = CreatePopupMenu();
            let header = match self.active_profile() {
                None if matcher::fingerprint(&self.connected).is_empty() => "No external monitors".to_string(),
                None => "Unknown display setup".to_string(),
                Some(p) => format!("Setup: {}{}", p.name, if p.enabled { "" } else { " (disabled)" }),
            };
            add(menu, MF_STRING | MF_GRAYED, &header, None);

            if let Some(p) = self.active_profile() {
                for (i, b) in p.bindings.iter().enumerate() {
                    let monitor = p.monitor(&b.monitor_id).map(|m| m.name.as_str()).unwrap_or("?");
                    let text = format!("{monitor}: {}\t{}", monitors::describe_action(&b.inputs), b.hotkey.display());
                    add(menu, MF_STRING, &text, Some(MenuAction::Run(i)));
                }
            }

            let externals: Vec<&MonitorInfo> = self.connected.iter().filter(|m| !m.internal).collect();
            if !externals.is_empty() {
                add(menu, MF_SEPARATOR, "", None);
                for m in externals {
                    let pm = self.active_profile().and_then(|p| p.monitor(&m.id));
                    let name = pm.map(|p| p.name.clone()).unwrap_or_else(|| m.display_name().to_string());
                    let inputs = match pm {
                        Some(p) if !p.supported_inputs.is_empty() => p.supported_inputs.clone(),
                        _ => monitors::COMMON_INPUTS.to_vec(),
                    };
                    let sub = CreatePopupMenu();
                    for code in inputs {
                        add(sub, MF_STRING, &monitors::long_name(code), Some(MenuAction::SetInput(m.clone(), code)));
                    }
                    AppendMenuW(menu, MF_POPUP, sub as usize, wide(&format!("Switch {name} to")).as_ptr());
                }
            }

            add(menu, MF_SEPARATOR, "", None);
            let configure = add(menu, MF_STRING, "Configure…", Some(MenuAction::Configure));
            SetMenuDefaultItem(menu, configure as u32, 0);
            add(menu, MF_STRING, "Re-detect displays", Some(MenuAction::Redetect));
            let startup = if sys::startup_enabled() { MF_CHECKED } else { 0 };
            add(menu, MF_STRING | startup, "Start with Windows", Some(MenuAction::ToggleStartup));
            add(menu, MF_STRING, "Open config folder", Some(MenuAction::OpenFolder));
            add(menu, MF_SEPARATOR, "", None);
            add(menu, MF_STRING | MF_GRAYED, &format!("InputSwitch {}", sys::VERSION), None);
            add(menu, MF_STRING, "Exit", Some(MenuAction::Exit));
            (menu, actions)
        }
    }

    fn run_menu(&mut self, action: MenuAction) {
        match action {
            MenuAction::Run(i) => self.run_binding(i),
            MenuAction::SetInput(m, code) => {
                std::thread::spawn(move || {
                    if let Err(e) = monitors::set(&m, code) {
                        post(Event::SwitchFailed(e));
                    }
                });
            }
            MenuAction::Configure => {
                let active = self.active.clone();
                self.open_settings(active);
            }
            MenuAction::Redetect => self.evaluate(false),
            MenuAction::ToggleStartup => {
                sys::set_startup(!sys::startup_enabled());
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

fn describe(p: &config::Profile) -> String {
    if p.monitors.len() == 1 { p.monitors[0].name.clone() } else { format!("{} monitors", p.monitors.len()) }
}
