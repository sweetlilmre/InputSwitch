//! The hotkey editor: a modal window, laid out like the C# app's, to choose the monitor, record
//! the key combination ([`key_box`], Win key included) and pick the inputs ([`input_list`]) to
//! switch between.

mod input_list;
mod key_box;

use crate::config::{Binding, Profile, ProfileMonitor};
use crate::ffi::*;
use crate::hotkey::Hotkey;
use crate::monitors;
use crate::ui::dialog::{self, Icon};
use crate::ui::draw::{self, rect};
use crate::ui::theme::{self, Surface};
use crate::ui::{self, widgets, Fonts, Slot};
use std::sync::atomic::{AtomicIsize, Ordering};

const CLASS: &str = "InputSwitch.HotkeyEditor";
const ID_MONITOR: i32 = 200;
const ID_HOTKEY: i32 = 201;
const ID_INPUTS: i32 = 202;
const ID_UP: i32 = 203;
const ID_DOWN: i32 = 204;
const ID_CODE: i32 = 205;
const ID_ADD_CODE: i32 = 206;

// Layout at 96 DPI (the C# editor's, without its heading: the title bar says it).
const WIDTH: i32 = 520;
const PAD: i32 = 28;
const CONTENT_W: i32 = WIDTH - 2 * PAD;
const FIELD_H: i32 = 32;
const INPUTS_Y: i32 = 258;
const INPUTS_H: i32 = 186;
const CODE_Y: i32 = INPUTS_Y + INPUTS_H + 12;
const SUMMARY_Y: i32 = CODE_Y + FIELD_H + 14;
const SUMMARY_H: i32 = 40;
const FOOTER_Y: i32 = SUMMARY_Y + SUMMARY_H + 18;
const FOOTER_H: i32 = 60;
const HEIGHT: i32 = FOOTER_Y + FOOTER_H;

struct Controls {
    monitor: HWND,
    inputs: HWND,
    code: HWND,
    summary: HWND,
}

/// Areas painted behind the controls, in pixels.
struct Frames {
    hotkey: RECT,
    inputs: RECT,
    code: RECT,
    summary: RECT,
    footer: RECT,
}

struct Editor {
    hwnd: HWND,
    fonts: Fonts,
    c: Controls,
    frames: Frames,
    monitors: Vec<ProfileMonitor>,
    /// The monitor chosen (index into monitors).
    monitor: usize,
    /// Other hotkeys in this configuration, to refuse duplicates.
    taken: Vec<Hotkey>,
    existing: Option<Binding>,
    /// Set when the editor closes: Some(binding) saved, None cancelled.
    result: Option<Option<Binding>>,
    dpi: u32,
}

thread_local! {
    static STATE: Slot<Editor> = const { Slot::new() };
}

fn with<R>(f: impl FnOnce(&mut Editor) -> R) -> Option<R> {
    STATE.with(|s| s.with(f))
}

/// Shows the editor modally over `owner`. Returns the new or changed hotkey, or None if cancelled.
pub fn run(owner: HWND, dpi: u32, profile: &Profile, existing: Option<&Binding>) -> Option<Binding> {
    ui::register_class(CLASS, wndproc, ui::app_icon(), 0);

    let title = if existing.is_some() { "Edit hotkey" } else { "Add hotkey" };
    let hwnd = ui::popup_window(CLASS, title, owner, WIDTH, HEIGHT, dpi);
    LAST_FOCUS.store(0, Ordering::Relaxed);
    let fonts = Fonts::new(dpi);
    let (c, frames) = create_controls(hwnd, &fonts, dpi, existing.map(|b| b.hotkey).unwrap_or_default());

    let monitor = existing.and_then(|e| profile.monitors.iter().position(|m| m.monitor_id.eq_ignore_ascii_case(&e.monitor_id))).unwrap_or(0);
    ui::set_text(c.monitor, &profile.monitors.get(monitor).map(monitor_label).unwrap_or_default());

    let taken = profile.bindings.iter().filter(|b| Some(&b.id) != existing.map(|e| &e.id)).map(|b| b.hotkey).collect();
    STATE.with(|s| s.set(Editor {
        hwnd, fonts, c, frames, monitors: profile.monitors.clone(), monitor, taken, existing: existing.cloned(), result: None, dpi,
    }));
    with(|e| e.fill_inputs());

    ui::modal_loop(owner, hwnd, &|| STATE.with(|s| s.with(|e| e.result.is_some()).unwrap_or(true)), &key_box::is_raw_key);
    let result = STATE.with(|s| s.take()).and_then(|e| e.result.flatten());
    unsafe { DestroyWindow(hwnd) };
    result
}

fn create_controls(h: HWND, f: &Fonts, dpi: u32, initial: Hotkey) -> (Controls, Frames) {
    use Surface::{Card, Layer, Window};
    let s = |v: i32| ui::scale(v, dpi);
    let p = |hw: HWND, x: i32, y: i32, w: i32, ht: i32| ui::place(hw, s(x), s(y), s(w), s(ht));
    let r = |x: i32, y: i32, w: i32, ht: i32| rect(s(x), s(y), s(w), s(ht));
    let field_label = |text: &str, y: i32| p(ui::label(h, text, f.bold, Window, false), PAD, y, CONTENT_W, 20);
    let hint = |text: &str, y: i32, lines: i32| {
        let label = ui::child(h, "STATIC", text, SS_NOPREFIX, 0, 0, f.small);
        theme::set_surface(label, Window, true);
        p(label, PAD + 2, y, CONTENT_W - 2, 17 * lines);
        label
    };

    field_label("Monitor", 16);
    let monitor = ui::drop_down::create(h, "", ID_MONITOR, f, Window);
    p(monitor, PAD, 42, CONTENT_W, FIELD_H);

    field_label("Key combination", 90);
    // Says whether the box is recording; kept up to date by the key box.
    let key_hint = hint("", 154, 2);
    let hotkey_frame = r(PAD, 116, CONTENT_W, FIELD_H);
    let hotkey = key_box::create(h, ID_HOTKEY, f.normal, initial, key_hint);
    p(hotkey, PAD + 10, 122, CONTENT_W - 20, 20);

    field_label("Inputs", 196);
    hint("Tick one input to switch to it, two to toggle between them, or more to cycle through them in the order shown.", 220, 2);
    // A card holding the inputs, with the up and down buttons on its right.
    let inputs_frame = r(PAD, INPUTS_Y, CONTENT_W, INPUTS_H);
    let inputs = input_list::create(h, ID_INPUTS, dpi);
    p(inputs, PAD + 6, INPUTS_Y + 6, CONTENT_W - 12 - 34 - 10, INPUTS_H - 12);
    let arrow = |id, icon| widgets::button(h, "", id, Some(icon), f, Card);
    p(arrow(ID_UP, widgets::ICON_UP), PAD + CONTENT_W - 6 - 34, INPUTS_Y + 10, 34, 32);
    p(arrow(ID_DOWN, widgets::ICON_DOWN), PAD + CONTENT_W - 6 - 34, INPUTS_Y + 10 + 36, 34, 32);

    p(ui::label(h, "Other input code (hex)", f.normal, Window, true), PAD, CODE_Y + 6, 200, 20);
    let add_w = 64;
    let code_frame = r(PAD + CONTENT_W - add_w - 8 - 90, CODE_Y, 90, FIELD_H);
    let code = ui::edit(h, ID_CODE, f.normal);
    p(code, PAD + CONTENT_W - add_w - 8 - 90 + 8, CODE_Y + 6, 90 - 16, 20);
    p(widgets::button(h, "Add", ID_ADD_CODE, None, f, Window), PAD + CONTENT_W - add_w, CODE_Y, add_w, FIELD_H);

    // What the hotkey will do, on a banner with a keyboard icon.
    let summary_frame = r(PAD, SUMMARY_Y, CONTENT_W, SUMMARY_H);
    let summary = ui::label(h, "", f.normal, Card, false);
    p(summary, PAD + 12 + 26, SUMMARY_Y + 10, CONTENT_W - 24 - 26, 20);

    let footer = r(0, FOOTER_Y, WIDTH, FOOTER_H);
    let by = FOOTER_Y + (FOOTER_H - 32) / 2;
    p(widgets::accent_button(h, "Save", IDOK, None, f, Layer), WIDTH - PAD - 100 - 8 - 100, by, 100, 32);
    p(widgets::button(h, "Cancel", IDCANCEL, None, f, Layer), WIDTH - PAD - 100, by, 100, 32);

    let frames = Frames { hotkey: hotkey_frame, inputs: inputs_frame, code: code_frame, summary: summary_frame, footer };
    (Controls { monitor, inputs, code, summary }, frames)
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
                draw::fill(w as HDC, draw::client_rect(h), theme::palette().window);
            }
            return 1;
        }
        WM_DRAWITEM if widgets::draw(&*(l as *const DRAWITEMSTRUCT)) => return 1,
        WM_SETTINGCHANGE if theme::is_theme_change(m, l) => {
            theme::reload();
            with(|e| e.retheme());
        }
        WM_CLOSE => {
            with(|e| e.result = Some(None));
            return 0;
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
            if let Some(brush) = key_box::ctl_color(w as HDC, l as HWND).or_else(|| theme::ctl_color(m, w as HDC, l as HWND)) {
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

/// The control to focus when the editor becomes active: the last one used, else the Monitor list.
/// (Not the key box: it records only once the user clicks or tabs into it.)
static LAST_FOCUS: AtomicIsize = AtomicIsize::new(0);

fn restore_focus() {
    let target = match LAST_FOCUS.load(Ordering::Relaxed) {
        0 => with(|e| e.c.monitor).unwrap_or(0),
        f => f,
    };
    if target != 0 {
        unsafe { SetFocus(target) };
    }
}

impl Editor {
    fn paint_background(&self, hdc: HDC) {
        let p = theme::palette();
        let f = &self.frames;
        let s = |v| ui::scale(v, self.dpi);
        draw::fill(hdc, draw::client_rect(self.hwnd), p.window);
        widgets::text_box(hdc, f.hotkey, self.dpi, key_box::is_listening());
        draw::round_rect(hdc, f.inputs, s(8), p.card, Some(p.stroke));
        widgets::text_box(hdc, f.code, self.dpi, unsafe { GetFocus() } == self.c.code);
        draw::round_rect(hdc, f.summary, s(6), p.card, None);
        let icon = rect(f.summary.left + s(12), f.summary.top, s(20), f.summary.bottom - f.summary.top);
        draw::glyph(hdc, widgets::ICON_KEYBOARD, icon, self.fonts.icon_16, p.accent, DT_VCENTER);
        draw::fill(hdc, f.footer, p.layer);
        draw::fill(hdc, RECT { bottom: f.footer.top + 1, ..f.footer }, p.stroke);
    }

    fn retheme(&mut self) {
        theme::apply_window(self.hwnd);
        theme::apply_control(self.c.code, theme::ControlKind::Edit);
        ui::card_panel::retheme(self.c.inputs);
        theme::repaint(self.hwnd);
    }

    fn selected_monitor(&self) -> Option<&ProfileMonitor> {
        self.monitors.get(self.monitor)
    }

    /// The Monitor drop-down was clicked: offer the configuration's monitors.
    fn choose_monitor(&mut self) {
        let names: Vec<String> = self.monitors.iter().map(monitor_label).collect();
        if let Some(i) = ui::drop_down::choose(self.c.monitor, &names, Some(self.monitor)).filter(|&i| i != self.monitor) {
            self.monitor = i;
            ui::set_text(self.c.monitor, &names[i]);
            self.fill_inputs();
        }
    }

    /// The hotkey's inputs first (in its order), then the rest of what the monitor offers.
    fn fill_inputs(&mut self) {
        let Some(m) = self.selected_monitor() else { return };
        let selected: Vec<u8> = match &self.existing {
            Some(b) if b.monitor_id.eq_ignore_ascii_case(&m.monitor_id) => b.inputs.clone(),
            _ => Vec::new(),
        };
        let available = if m.supported_inputs.is_empty() { monitors::COMMON_INPUTS } else { &m.supported_inputs[..] };
        let rows: Vec<(u8, bool)> = selected.iter().map(|&c| (c, true))
            .chain(available.iter().filter(|c| !selected.contains(c)).map(|&c| (c, false)))
            .collect();
        input_list::set_inputs(self.c.inputs, &rows);
        self.update_summary();
    }

    fn update_summary(&self) {
        let Some(m) = self.selected_monitor() else { return ui::set_text(self.c.summary, "") };
        let mut text = monitors::describe_action(&input_list::checked(self.c.inputs));
        if m.supported_inputs.is_empty() {
            text.push_str("   (inputs not detected; common inputs listed)");
        }
        ui::set_text(self.c.summary, &text);
    }

    fn add_code(&mut self) {
        let text = ui::get_text(self.c.code);
        let t = text.trim();
        let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
        let Some(code) = u8::from_str_radix(t, 16).ok().filter(|&c| c != 0) else {
            dialog::alert(self.hwnd, Icon::Warning, "Not a valid input code", "Enter a hexadecimal code between 01 and FF, for example 1B.");
            return;
        };
        input_list::add(self.c.inputs, code);
        ui::set_text(self.c.code, "");
        self.update_summary();
    }

    /// Save: checks the hotkey can be used, and closes with it.
    fn accept(&mut self) {
        let hk = key_box::value();
        let inputs = input_list::checked(self.c.inputs);
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
            dialog::alert(self.hwnd, Icon::Warning, &heading, message);
            return;
        }

        // Windows refuses combinations already owned by Windows or another app. (The tray's own
        // hotkeys are paused while the editor is open, so they don't count.)
        if self.existing.as_ref().map(|b| b.hotkey) != Some(hk) && !can_register(self.hwnd, hk) {
            dialog::alert(self.hwnd, Icon::Warning, &format!("{} is already in use", hk.display()),
                "Windows or another app already uses this combination, so InputSwitch can't. Choose another one.");
            return;
        }
        // Some Windows shortcuts are handled before any app sees them, even when registering works.
        if let Some(what) = crate::reserved::windows_use(&hk) {
            let msg = format!("Windows uses {} for {what}, so it will probably never reach InputSwitch.", hk.display());
            if !dialog::confirm(self.hwnd, &format!("{} is a Windows shortcut", hk.display()), &msg, "Use anyway") {
                return;
            }
        }

        self.result = Some(Some(Binding {
            id: self.existing.as_ref().map_or_else(crate::sys::new_id, |b| b.id.clone()),
            monitor_id: monitor.unwrap_or_default(),
            hotkey: hk,
            inputs,
        }));
    }

    fn command(&mut self, id: i32, code: u16) {
        if input_list::is_check(id, code) {
            return self.update_summary();
        }
        match (id, code) {
            (ID_MONITOR, BN_CLICKED) => self.choose_monitor(),
            (ID_UP, BN_CLICKED) => input_list::move_selected(self.c.inputs, -1),
            (ID_DOWN, BN_CLICKED) => input_list::move_selected(self.c.inputs, 1),
            (ID_ADD_CODE, BN_CLICKED) => self.add_code(),
            (ID_CODE, EN_SETFOCUS | EN_KILLFOCUS) => self.repaint(self.frames.code),
            (IDOK, _) => self.accept(),
            (IDCANCEL, _) => self.result = Some(None),
            _ => return,
        }
        if matches!(id, ID_UP | ID_DOWN) {
            self.update_summary(); // the order matters for a cycle
        }
    }

    fn repaint(&self, r: RECT) {
        unsafe { RedrawWindow(self.hwnd, &r, 0, RDW_INVALIDATE | RDW_ERASE) };
    }
}

/// How a monitor is shown in the Monitor drop-down.
fn monitor_label(m: &ProfileMonitor) -> String {
    format!("{}  ·  {}", m.name, m.model)
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
