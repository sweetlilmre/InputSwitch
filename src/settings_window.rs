//! The Settings dialog: the app-wide options (language, theme, Start with Windows,
//! notifications). Like the main window it has no Save button: each change is reported to the
//! caller as it's made, and takes effect straight away. A new language shows at once, here too.

use crate::config::{AppConfig, Theme};
use crate::ffi::*;
use crate::i18n::{self, t, LANGUAGES};
use crate::sys;
use crate::ui::dialog::{self, Icon};
use crate::ui::draw::{self, rect};
use crate::ui::theme::{self, Surface};
use crate::ui::{self, widgets, Fonts, Slot};
use std::cell::RefCell;

/// A setting the user changed, for the caller to apply and save.
pub enum Change {
    Language(&'static str),
    Theme(Theme),
    Notify(bool),
}

const CLASS: &str = "InputSwitch.Settings";
const ID_LANGUAGE: i32 = 300;
const ID_THEME: i32 = 301;
const ID_STARTUP: i32 = 302;
const ID_NOTIFY: i32 = 303;

// Layout at 96 DPI.
const WIDTH: i32 = 440;
const PAD: i32 = 28;
const CONTENT_W: i32 = WIDTH - 2 * PAD;
const FIELD_H: i32 = 32;
const CHECK_H: i32 = 24;
const FOOTER_Y: i32 = 276;
const FOOTER_H: i32 = 60;
const HEIGHT: i32 = FOOTER_Y + FOOTER_H;
const BUTTON_W: i32 = 100;
const BUTTON_H: i32 = 32;

/// The themes, in the order the Theme drop-down lists them, with their text keys.
const THEMES: [(Theme, &str); 3] = [(Theme::System, "settings.theme_system"), (Theme::Light, "settings.theme_light"), (Theme::Dark, "settings.theme_dark")];

struct Controls {
    language_label: HWND,
    language: HWND,
    theme_label: HWND,
    theme: HWND,
    startup: HWND,
    notify: HWND,
    notify_hint: HWND,
    close: HWND,
}

struct Settings {
    hwnd: HWND,
    c: Controls,
    language: &'static str,
    theme: Theme,
    /// Changes not yet handed to the caller.
    changes: Vec<Change>,
    done: bool,
}

thread_local! {
    static STATE: Slot<Settings> = const { Slot::new() };
}

fn with<R>(f: impl FnOnce(&mut Settings) -> R) -> Option<R> {
    STATE.with(|s| s.with(f))
}

/// Shows the dialog modally over `owner`, for the settings in `config`. `on_change` is called
/// with each change as it's made.
pub fn run(owner: HWND, dpi: u32, config: &AppConfig, on_change: &mut dyn FnMut(Change)) {
    ui::register_class(CLASS, wndproc, true, 0);
    let hwnd = ui::popup_window(CLASS, t("settings.title"), owner, WIDTH, HEIGHT, dpi);
    let fonts = Fonts::new(dpi);
    let c = create_controls(hwnd, &fonts);
    widgets::set_checked(c.startup, sys::startup_enabled());
    widgets::set_checked(c.notify, config.show_notifications);
    let language = LANGUAGES.iter().find(|l| l.code == i18n::language()).map_or(i18n::DEFAULT, |l| l.code);
    STATE.with(|s| s.set(Settings { hwnd, c, language, theme: config.theme, changes: Vec::new(), done: false }));
    with(|s| s.show_texts());

    // Changes are handed over between messages, while the dialog's state isn't lent out.
    let on_change = RefCell::new(on_change);
    let done = || {
        for change in with(|s| std::mem::take(&mut s.changes)).unwrap_or_default() {
            (on_change.borrow_mut())(change);
        }
        with(|s| s.done).unwrap_or(true)
    };
    ui::modal_loop(owner, hwnd, &done, &|_| false);
    STATE.with(|s| s.take());
    unsafe { DestroyWindow(hwnd) };
    // Keep the fonts alive until the window that uses them is gone.
    drop(fonts);
}

/// Creates and places the controls, at 96 DPI scaled. Their text is set by `show_texts`.
fn create_controls(h: HWND, f: &Fonts) -> Controls {
    let s = |v: i32| ui::scale(v, f.dpi);
    let p = |hw: HWND, x: i32, y: i32, w: i32, ht: i32| {
        ui::place(hw, s(x), s(y), s(w), s(ht));
        hw
    };
    let field_label = |y: i32| p(ui::label(h, "", f.bold, Surface::Window, false), PAD, y, CONTENT_W, 20);
    let switch = |id: i32, y: i32| p(widgets::switch(h, "", id, f, Surface::Window), PAD, y, CONTENT_W, CHECK_H);
    Controls {
        language_label: field_label(16),
        language: p(ui::drop_down::create(h, "", ID_LANGUAGE, f, Surface::Window), PAD, 42, CONTENT_W, FIELD_H),
        theme_label: field_label(90),
        theme: p(ui::drop_down::create(h, "", ID_THEME, f, Surface::Window), PAD, 116, CONTENT_W, FIELD_H),
        startup: switch(ID_STARTUP, 172),
        notify: switch(ID_NOTIFY, 206),
        notify_hint: {
            let hint = ui::child(h, "STATIC", "", SS_NOPREFIX, 0, 0, f.small);
            theme::set_surface(hint, Surface::Window, true);
            p(hint, PAD + 53, 232, CONTENT_W - 53, 34)
        },
        close: {
            let by = FOOTER_Y + (FOOTER_H - BUTTON_H) / 2;
            p(widgets::button(h, "", IDCANCEL, None, f, Surface::Layer), WIDTH - PAD - BUTTON_W, by, BUTTON_W, BUTTON_H)
        },
    }
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_COMMAND => {
            if hiword(w) == BN_CLICKED && l != 0 {
                widgets::clicked(l as HWND);
            }
            if loword(w) as i32 == IDOK && ui::click_focused_button(h) {
                return 0;
            }
            with(|s| s.command(loword(w) as i32, hiword(w)));
            return 0;
        }
        WM_ERASEBKGND => {
            let hdc = w as HDC;
            let p = theme::palette();
            let dpi = ui::dpi_of(h);
            let rc = draw::client_rect(h);
            let footer = rect(0, ui::scale(FOOTER_Y, dpi), rc.right, rc.bottom);
            draw::fill(hdc, rc, p.window);
            draw::fill(hdc, footer, p.layer);
            draw::fill(hdc, RECT { bottom: footer.top + 1, ..footer }, p.stroke);
            return 1;
        }
        WM_DRAWITEM if widgets::draw(&*(l as *const DRAWITEMSTRUCT)) => return 1,
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            if let Some(brush) = theme::ctl_color(m, w as HDC, l as HWND) {
                return brush;
            }
        }
        WM_SETTINGCHANGE if theme::is_theme_change(m, l) => {
            theme::reload();
            with(|s| s.retheme());
        }
        WM_CLOSE => {
            with(|s| s.done = true);
            return 0;
        }
        WM_ACTIVATE => {
            ui::keep_focus(h, w, with(|s| s.c.language).unwrap_or(0));
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}

impl Settings {
    fn command(&mut self, id: i32, code: u16) {
        match (id, code) {
            (ID_LANGUAGE, BN_CLICKED) => self.choose_language(),
            (ID_THEME, BN_CLICKED) => self.choose_theme(),
            (ID_STARTUP, BN_CLICKED) => self.set_startup(),
            (ID_NOTIFY, BN_CLICKED) => self.changes.push(Change::Notify(widgets::checked(self.c.notify))),
            (IDCANCEL | IDOK, _) => self.done = true,
            _ => {}
        }
    }

    /// Every label, in the current language.
    fn show_texts(&self) {
        let c = &self.c;
        ui::set_text(self.hwnd, t("settings.title"));
        ui::set_text(c.language_label, t("settings.language"));
        ui::set_text(c.language, LANGUAGES.iter().find(|l| l.code == self.language).map_or("", |l| l.name));
        ui::set_text(c.theme_label, t("settings.theme"));
        ui::set_text(c.theme, THEMES.iter().find(|(th, _)| *th == self.theme).map_or("", |(_, key)| t(key)));
        ui::set_text(c.startup, t("settings.start_with_windows"));
        ui::set_text(c.notify, t("settings.notify"));
        ui::set_text(c.notify_hint, t("settings.notify_hint"));
        ui::set_text(c.close, t("common.close"));
        theme::repaint(self.hwnd);
    }

    fn choose_language(&mut self) {
        let names: Vec<String> = LANGUAGES.iter().map(|l| l.name.to_string()).collect();
        let current = LANGUAGES.iter().position(|l| l.code == self.language);
        let Some(i) = ui::drop_down::choose(self.c.language, &names, current).filter(|&i| Some(i) != current) else { return };
        self.language = LANGUAGES[i].code;
        i18n::set_language(self.language);
        self.show_texts();
        self.changes.push(Change::Language(self.language));
    }

    fn choose_theme(&mut self) {
        let names: Vec<String> = THEMES.iter().map(|(_, key)| t(key).to_string()).collect();
        let current = THEMES.iter().position(|(th, _)| *th == self.theme);
        let Some(i) = ui::drop_down::choose(self.c.theme, &names, current).filter(|&i| Some(i) != current) else { return };
        self.theme = THEMES[i].0;
        theme::set_dark(self.theme.dark());
        self.retheme();
        self.show_texts();
        self.changes.push(Change::Theme(self.theme));
    }

    fn set_startup(&mut self) {
        if !sys::set_startup(widgets::checked(self.c.startup)) {
            dialog::alert(self.hwnd, Icon::Warning, t("notify.startup_failed"), t("notify.startup_failed_detail"));
            widgets::set_checked(self.c.startup, sys::startup_enabled());
        }
    }

    fn retheme(&self) {
        theme::apply_window(self.hwnd);
        theme::repaint(self.hwnd);
    }
}
