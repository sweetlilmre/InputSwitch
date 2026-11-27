//! The hotkey editor: a modal window to choose the monitor, the key combination (recorded by
//! `hotkey_box`, Win key included) and the inputs to switch between.

use crate::config::{Binding, Profile, ProfileMonitor};
use crate::ffi::*;
use crate::hotkey::Hotkey;
use crate::hotkey_box;
use crate::monitors;
use crate::theme::{self, Surface};
use crate::ui::{self, Fonts};
use crate::widgets;
use std::cell::RefCell;
use std::sync::atomic::{AtomicIsize, Ordering};

const CLASS: &str = "InputSwitch.Editor";
const ID_MONITOR: i32 = 200;
const ID_HOTKEY: i32 = 201;
const ID_INPUTS: i32 = 202;
const ID_UP: i32 = 203;
const ID_DOWN: i32 = 204;
const ID_CODE: i32 = 205;
const ID_ADD_CODE: i32 = 206;

const WIDTH: i32 = 480;
const HEIGHT: i32 = 496;

struct Controls {
    monitor: HWND,
    inputs: HWND,
    code: HWND,
    summary: HWND,
}

struct Editor {
    hwnd: HWND,
    _fonts: Fonts,
    c: Controls,
    monitors: Vec<ProfileMonitor>,
    /// Other bindings' hotkeys in this configuration, to refuse duplicates.
    taken: Vec<Hotkey>,
    existing: Option<Binding>,
    result: Option<Option<Binding>>,
    dpi: u32,
    /// Rounded frames painted behind the borderless inputs: (rect, is the hotkey box).
    frames: Vec<(RECT, bool)>,
}

thread_local! {
    static STATE: RefCell<Option<Editor>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Editor) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

/// Shows the editor modally over `owner`. Returns the new or changed binding, or None if cancelled.
pub fn run(owner: HWND, dpi: u32, profile: &Profile, existing: Option<&Binding>) -> Option<Binding> {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| ui::register_class(CLASS, wndproc, ui::app_icon()));

    let style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
    let ex = WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT;
    let mut r = RECT { left: 0, top: 0, right: ui::scale(WIDTH, dpi), bottom: ui::scale(HEIGHT, dpi) };
    let mut owner_rect = RECT::default();
    unsafe {
        AdjustWindowRectExForDpi(&mut r, style, 0, ex, dpi);
        GetWindowRect(owner, &mut owner_rect);
    }
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    let x = owner_rect.left + ((owner_rect.right - owner_rect.left) - w) / 2;
    let y = owner_rect.top + ((owner_rect.bottom - owner_rect.top) - h) / 2;
    let title = if existing.is_some() { "Edit hotkey" } else { "Add hotkey" };
    let hwnd = unsafe {
        CreateWindowExW(ex, wide(CLASS).as_ptr(), wide(title).as_ptr(), style, x, y, w, h, owner, 0, hinstance(), core::ptr::null())
    };
    theme::apply_window(hwnd);

    LAST_FOCUS.store(0, Ordering::Relaxed);
    let fonts = Fonts::new(dpi);
    let (c, frames) = create_controls(hwnd, &fonts, dpi, existing.map(|b| b.hotkey).unwrap_or_default());

    let taken = profile.bindings.iter().filter(|b| Some(&b.id) != existing.map(|e| &e.id)).map(|b| b.hotkey).collect();
    let names: Vec<String> = profile.monitors.iter().map(|m| format!("{}  ·  {}", m.name, m.model)).collect();
    let selected = existing.and_then(|e| profile.monitors.iter().position(|m| m.monitor_id.eq_ignore_ascii_case(&e.monitor_id))).unwrap_or(0);
    ui::cb_set(c.monitor, &names, selected);

    STATE.with(|s| {
        *s.borrow_mut() = Some(Editor {
            hwnd, _fonts: fonts, c, monitors: profile.monitors.clone(), taken, existing: existing.cloned(),
            result: None, dpi, frames,
        })
    });
    with(|e| {
        e.fill_inputs();
    });

    ui::modal_loop(owner, hwnd, &|| {
        STATE.with(|s| s.try_borrow().map(|g| g.as_ref().is_none_or(|e| e.result.is_some())).unwrap_or(false))
    }, &hotkey_box::is_raw_key);
    let result = STATE.with(|s| s.borrow_mut().take()).and_then(|e| e.result.flatten());
    unsafe { DestroyWindow(hwnd) };
    result
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_COMMAND => {
            if hiword(w) == BN_CLICKED && l != 0 {
                widgets::clicked(l as HWND);
            }
            with(|e| e.command(loword(w) as i32, hiword(w)));
            return 0;
        }
        WM_ERASEBKGND => {
            if with(|e| e.paint_background(w as HDC)).is_none() {
                let mut rc = RECT::default();
                GetClientRect(h, &mut rc);
                theme::fill(w as HDC, rc, theme::palette().window);
            }
            return 1;
        }
        WM_DRAWITEM => {
            if widgets::draw(&*(l as *const DRAWITEMSTRUCT)) {
                return 1;
            }
        }
        WM_SETTINGCHANGE if theme::is_theme_change(m, l) => {
            theme::reload();
            with(|e| e.retheme());
        }
        WM_NOTIFY => {
            let hdr = &*(l as *const NMHDR);
            if hdr.idFrom as i32 == ID_INPUTS && hdr.code == LVN_ITEMCHANGED {
                let nm = &*(l as *const NMLISTVIEW);
                if nm.uChanged & LVIF_STATE != 0 && (nm.uNewState ^ nm.uOldState) & LVIS_STATEIMAGEMASK != 0 {
                    with(|e| e.update_summary());
                }
            }
        }
        WM_CLOSE => {
            with(|e| e.result = Some(None));
            return 0;
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
            if let Some(brush) = hotkey_box::ctl_color(w as HDC, l as HWND).or_else(|| theme::ctl_color(m, w as HDC, l as HWND)) {
                return brush;
            }
        }
        // Like a dialog: remember which control had focus when the editor is deactivated, and put
        // it back (initially, the Monitor list) when it's activated again.
        WM_ACTIVATE => {
            if loword(w) == 0 {
                let f = GetFocus();
                if f != 0 && IsChild(h, f) != 0 {
                    LAST_FOCUS.store(f, Ordering::Relaxed);
                }
            } else {
                restore_focus();
            }
            return 0;
        }
        WM_SETFOCUS => {
            restore_focus();
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}

/// Control to focus when the editor becomes active: the last one used, else the Monitor list.
/// (Not the hotkey box: it starts listening only when the user clicks or tabs into it.)
static LAST_FOCUS: AtomicIsize = AtomicIsize::new(0);

fn restore_focus() {
    let mut target = LAST_FOCUS.load(Ordering::Relaxed);
    if target == 0 {
        target = STATE.with(|s| s.try_borrow().ok().and_then(|g| g.as_ref().map(|e| e.c.monitor))).unwrap_or(0);
    }
    if target != 0 {
        unsafe { SetFocus(target) };
    }
}

fn create_controls(h: HWND, f: &Fonts, dpi: u32, initial: Hotkey) -> (Controls, Vec<(RECT, bool)>) {
    let n = f.normal;
    let sc = |v: i32| ui::scale(v, dpi);
    let p = |hw: HWND, x: i32, y: i32, w: i32, ht: i32| ui::place(hw, sc(x), sc(y), sc(w), sc(ht));
    let r = |x: i32, y: i32, w: i32, ht: i32| RECT { left: sc(x), top: sc(y), right: sc(x + w), bottom: sc(y + ht) };
    let (m, w) = (16, WIDTH - 32);
    let mut frames = Vec::new();

    p(ui::label_on(h, "Monitor", f.bold, Surface::Window, false), m, 14, w, 20);
    let monitor = ui::child(h, "COMBOBOX", "", CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL, 0, ID_MONITOR, n);
    theme::apply_control(monitor, theme::ControlKind::ComboBox);
    p(monitor, m, 36, w, 240);

    p(ui::label_on(h, "Key combination", f.bold, Surface::Window, false), m, 74, w, 20);
    // The hint under the box says whether it's listening; it wraps onto a second line.
    let hint = ui::child(h, "STATIC", "", SS_NOPREFIX, 0, 0, n);
    theme::set_surface(hint, Surface::Window, true);
    p(hint, m, 134, w, 34);
    let hotkey = hotkey_box::create(h, ID_HOTKEY, n, initial, hint);
    frames.push((r(m, 96, w, 32), true));
    p(hotkey, m + 2, 103, w - 4, 20);

    p(ui::label_on(h, "Inputs", f.bold, Surface::Window, false), m, 174, w, 20);
    p(ui::label_on(h, "Tick one to switch to it, two to toggle, more to cycle in the order shown.", n, Surface::Window, true), m, 196, w, 20);
    let inputs = ui::listview(h, ID_INPUTS, LVS_NOCOLUMNHEADER, LVS_EX_CHECKBOXES, n);
    frames.push((r(m, 220, w - 100, 150), false));
    p(inputs, m + 1, 221, w - 102, 148);
    ui::lv_add_column(inputs, 0, "Input", sc(w - 102 - 24));
    p(widgets::button(h, "Up", ID_UP, Some(widgets::ICON_UP), f, Surface::Window), m + w - 92, 220, 92, 32);
    p(widgets::button(h, "Down", ID_DOWN, Some(widgets::ICON_DOWN), f, Surface::Window), m + w - 92, 258, 92, 32);

    p(ui::label_on(h, "Other input code (hex):", n, Surface::Window, false), m, 389, 150, 20);
    let code = ui::child(h, "EDIT", "", ES_AUTOHSCROLL | WS_TABSTOP, 0, ID_CODE, n);
    theme::apply_control(code, theme::ControlKind::Edit);
    frames.push((r(m + 156, 382, 80, 32), false));
    p(code, m + 162, 389, 68, 20);
    p(widgets::button(h, "Add", ID_ADD_CODE, Some(widgets::ICON_ADD), f, Surface::Window), m + 244, 382, 84, 32);

    let summary = ui::label_on(h, "", f.bold, Surface::Window, false);
    p(summary, m, 424, w, 20);

    p(widgets::accent_button(h, "Save", IDOK, None, f, Surface::Window), WIDTH - 16 - 100 - 8 - 100, 450, 100, 32);
    p(widgets::button(h, "Cancel", IDCANCEL, None, f, Surface::Window), WIDTH - 16 - 100, 450, 100, 32);

    (Controls { monitor, inputs, code, summary }, frames)
}
impl Editor {
    /// Paints the background and the rounded frames behind the key box, input list and code box.
    fn paint_background(&self, hdc: HDC) {
        let p = theme::palette();
        let mut rc = RECT::default();
        unsafe { GetClientRect(self.hwnd, &mut rc) };
        theme::fill(hdc, rc, p.window);
        for &(frame, is_hotkey) in &self.frames {
            let fill = if is_hotkey { hotkey_box::background() } else { p.control };
            let border = if is_hotkey && hotkey_box::is_listening() { p.accent } else { p.control_stroke };
            theme::round_rect(hdc, frame, ui::scale(4, self.dpi), fill, Some(border));
        }
    }

    fn retheme(&mut self) {
        theme::apply_window(self.hwnd);
        theme::apply_control(self.c.monitor, theme::ControlKind::ComboBox);
        theme::apply_control(self.c.code, theme::ControlKind::Edit);
        theme::apply_control(self.c.inputs, theme::ControlKind::ListView);
        // Checkbox images are made when the style is set: clear and set it again to redraw them.
        unsafe {
            SendMessageW(self.c.inputs, LVM_SETEXTENDEDLISTVIEWSTYLE, LVS_EX_CHECKBOXES as usize, 0);
            SendMessageW(self.c.inputs, LVM_SETEXTENDEDLISTVIEWSTYLE, LVS_EX_CHECKBOXES as usize, LVS_EX_CHECKBOXES as LPARAM);
        }
        let checked = self.checked_inputs();
        self.fill_inputs_keeping(&checked);
        theme::repaint(self.hwnd);
    }

    fn selected_monitor(&self) -> Option<&ProfileMonitor> {
        ui::cb_selected(self.c.monitor).and_then(|i| self.monitors.get(i))
    }

    /// The binding's inputs first (in its order), then the rest of what the monitor offers.
    fn fill_inputs(&mut self) {
        ui::lv_clear(self.c.inputs);
        let Some(m) = self.selected_monitor() else { return };
        let selected: Vec<u8> = match &self.existing {
            Some(b) if b.monitor_id.eq_ignore_ascii_case(&m.monitor_id) => b.inputs.clone(),
            _ => Vec::new(),
        };
        let available = if m.supported_inputs.is_empty() { monitors::COMMON_INPUTS.to_vec() } else { m.supported_inputs.clone() };
        let mut codes = selected.clone();
        codes.extend(available.into_iter().filter(|c| !selected.contains(c)));
        for code in codes {
            let row = ui::lv_add(self.c.inputs, &[monitors::long_name(code)], code as isize);
            ui::lv_set_checked(self.c.inputs, row, selected.contains(&code));
        }
        ui::lv_fill_last_column(self.c.inputs, 0);
        self.update_summary();
    }

    /// Re-applies tick marks (after the checkbox images were rebuilt), keeping the order.
    fn fill_inputs_keeping(&self, checked: &[u8]) {
        for r in 0..ui::lv_count(self.c.inputs) {
            let code = ui::lv_param(self.c.inputs, r) as u8;
            ui::lv_set_checked(self.c.inputs, r, checked.contains(&code));
        }
    }

    fn checked_inputs(&self) -> Vec<u8> {
        (0..ui::lv_count(self.c.inputs)).filter(|&r| ui::lv_checked(self.c.inputs, r)).map(|r| ui::lv_param(self.c.inputs, r) as u8).collect()
    }

    fn update_summary(&self) {
        let Some(m) = self.selected_monitor() else { return ui::set_text(self.c.summary, "") };
        let mut text = format!("Action: {}", monitors::describe_action(&self.checked_inputs()));
        if m.supported_inputs.is_empty() {
            text.push_str("   (inputs not detected; common inputs listed)");
        }
        ui::set_text(self.c.summary, &text);
    }

    fn move_selected(&mut self, delta: i32) {
        let Some(i) = ui::lv_selected(self.c.inputs) else { return };
        let j = i + delta;
        if j < 0 || j >= ui::lv_count(self.c.inputs) {
            return;
        }
        let row = |r: i32| (ui::lv_param(self.c.inputs, r) as u8, ui::lv_checked(self.c.inputs, r));
        let (a, b) = (row(i), row(j));
        for (r, (code, on)) in [(i, b), (j, a)] {
            ui::lv_set_text(self.c.inputs, r, 0, &monitors::long_name(code));
            let mut item: LVITEMW = zeroed();
            item.mask = LVIF_PARAM;
            item.iItem = r;
            item.lParam = code as isize;
            unsafe { SendMessageW(self.c.inputs, 0x104C /* LVM_SETITEMW */, 0, &item as *const _ as LPARAM) };
            ui::lv_set_checked(self.c.inputs, r, on);
        }
        ui::lv_select(self.c.inputs, j);
        self.update_summary();
    }

    fn add_code(&mut self) {
        let text = ui::get_text(self.c.code);
        let t = text.trim();
        let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
        let Some(code) = u8::from_str_radix(t, 16).ok().filter(|&c| c != 0) else {
            ui::alert(self.hwnd, ui::Icon::Warning, "Not a valid input code", "Enter a hexadecimal code between 01 and FF, for example 1B.");
            return;
        };
        let existing = (0..ui::lv_count(self.c.inputs)).find(|&r| ui::lv_param(self.c.inputs, r) as u8 == code);
        let row = existing.unwrap_or_else(|| ui::lv_add(self.c.inputs, &[monitors::long_name(code)], code as isize));
        ui::lv_fill_last_column(self.c.inputs, 0); // a scroll bar may have appeared
        ui::lv_set_checked(self.c.inputs, row, true);
        ui::lv_select(self.c.inputs, row);
        ui::set_text(self.c.code, "");
        self.update_summary();
    }

    fn accept(&mut self) {
        let hk = hotkey_box::value();
        let inputs = self.checked_inputs();
        let monitor = self.selected_monitor().map(|m| m.monitor_id.clone());
        // Each problem gets a heading that describes it, rather than one heading for all of them.
        let problem: Option<(String, &str)> = if monitor.is_none() {
            Some(("Choose a monitor".into(), "Pick the monitor this hotkey should switch."))
        } else if hk.is_empty() {
            Some(("No key combination yet".into(), "Click the key combination box and press the keys to use."))
        } else if !hk.is_valid() {
            Some(("That combination can't be used".into(), "Use Ctrl, Alt or Win with the key (or one of F13–F24)."))
        } else if inputs.is_empty() {
            Some(("This hotkey isn't finished".into(), "Tick at least one input."))
        } else if self.taken.contains(&hk) {
            Some((format!("{} is already in use", hk.display()),
                "Another hotkey in this configuration uses it. Choose a different combination, or change the other hotkey."))
        } else {
            None
        };
        if let Some((heading, message)) = problem {
            ui::alert(self.hwnd, ui::Icon::Warning, &heading, message);
            return;
        }

        // Windows refuses combinations already owned by Windows or another app. (The tray app's
        // own hotkeys are paused while the hotkey editor is open, so they don't count.)
        if self.existing.as_ref().map(|b| b.hotkey) != Some(hk) && !can_register(self.hwnd, hk) {
            ui::alert(self.hwnd, ui::Icon::Warning, &format!("{} is already in use", hk.display()),
                "Windows or another app already uses this combination, so InputSwitch can't. Choose another one.");
            return;
        }
        // Some Windows shortcuts are handled before any app sees them, even when registering works.
        if let Some(what) = crate::reserved::windows_use(&hk) {
            let msg = format!("Windows uses {} for {what}, so it will probably never reach InputSwitch.", hk.display());
            if !ui::confirm(self.hwnd, &format!("{} is a Windows shortcut", hk.display()), &msg, "Use anyway") {
                return;
            }
        }

        self.result = Some(Some(Binding {
            id: self.existing.as_ref().map(|b| b.id.clone()).unwrap_or_else(crate::sys::new_id),
            monitor_id: monitor.unwrap_or_default(),
            hotkey: hk,
            inputs,
        }));
    }

    fn command(&mut self, id: i32, code: u16) {
        match (id, code) {
            (ID_MONITOR, CBN_SELCHANGE) => self.fill_inputs(),
            (ID_UP, BN_CLICKED) => self.move_selected(-1),
            (ID_DOWN, BN_CLICKED) => self.move_selected(1),
            (ID_ADD_CODE, BN_CLICKED) => self.add_code(),
            (IDOK, _) => self.accept(),
            (IDCANCEL, _) => self.result = Some(None),
            _ => {}
        }
    }
}

/// Whether Windows would let us register this hotkey: registers it briefly, then releases it.
fn can_register(h: HWND, hk: Hotkey) -> bool {
    const PROBE_ID: i32 = 0xBFFF;
    unsafe {
        if RegisterHotKey(h, PROBE_ID, hk.mods as u32 | MOD_NOREPEAT, hk.vk as u32) == 0 {
            return false;
        }
        UnregisterHotKey(h, PROBE_ID);
    }
    true
}
