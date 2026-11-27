//! Themed message boxes in the style of a Windows 11 content dialog (like the C# app's
//! FluentDialog): an icon badge, a heading, a sentence of detail, and equal-width buttons labelled
//! with what they do, on a footer band. `TaskDialog` can't follow dark mode, so these are drawn
//! with the app's own palette and widgets.

use crate::ffi::*;
use crate::theme::{self, Surface};
use crate::ui::{self, Fonts};
use crate::widgets;
use std::cell::RefCell;

const CLASS: &str = "InputSwitch.Dialog";
const ID_FIRST_BUTTON: i32 = 300;

/// Client width at 96 DPI; the height follows the text.
const WIDTH: i32 = 460;
const PAD: i32 = 24;
const BADGE: i32 = 40;
const FOOTER: i32 = 64;

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Info,
    Warning,
}

struct Dialog {
    icon: Icon,
    heading: String,
    message: String,
    buttons: Vec<HWND>,
    heading_font: HFONT,
    glyph_font: HFONT,
    _fonts: Fonts,
    dpi: u32,
    /// Index of the button chosen; Some(None) when dismissed with Esc or the close box.
    result: Option<Option<usize>>,
}

impl Drop for Dialog {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.heading_font);
            DeleteObject(self.glyph_font);
        }
    }
}

thread_local! {
    static STATE: RefCell<Option<Dialog>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Dialog) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

/// Shows a dialog modally over `owner` (0 for none: then it's centred on screen, on top, with a
/// taskbar button). `buttons[0]` is the main action (accent, Enter); the last button is what Esc
/// and the close box mean. Returns the index of the button chosen.
pub fn show(owner: HWND, icon: Icon, heading: &str, message: &str, buttons: &[&str]) -> usize {
    let cancel = buttons.len().saturating_sub(1);
    // One at a time: a second dialog while one is open (rare) falls back to the system one.
    if STATE.with(|s| s.try_borrow().map(|g| g.is_some()).unwrap_or(true)) {
        return fallback(owner, icon, heading, message, buttons);
    }
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| ui::register_class(CLASS, wndproc, ui::app_icon()));

    let dpi = unsafe { if owner != 0 { GetDpiForWindow(owner) } else { GetDpiForSystem() } }.max(96);
    let sc = |v: i32| ui::scale(v, dpi);
    let fonts = Fonts::new(dpi);
    let heading_font = ui::make_font(dpi, 20, 600);
    let glyph_font = ui::icon_font(dpi, 20);

    // Height: the text column, or the badge if taller, then the footer.
    let text_w = sc(WIDTH - PAD - BADGE - 16 - PAD);
    let heading_h = measure(heading, heading_font, text_w);
    let message_h = if message.is_empty() { 0 } else { sc(8) + measure(message, fonts.normal, text_w) };
    let body_h = (heading_h + message_h).max(sc(BADGE + 2));
    let (cw, ch) = (sc(WIDTH), sc(20) + body_h + sc(24) + sc(FOOTER));

    let style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
    let mut ex = WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT;
    if owner == 0 {
        ex |= WS_EX_TOPMOST | WS_EX_APPWINDOW;
    }
    let mut r = RECT { left: 0, top: 0, right: cw, bottom: ch };
    let mut area = RECT::default();
    unsafe {
        AdjustWindowRectExForDpi(&mut r, style, 0, ex, dpi);
        if owner != 0 {
            GetWindowRect(owner, &mut area);
        } else {
            area = RECT { left: 0, top: 0, right: GetSystemMetrics(0 /* SM_CXSCREEN */), bottom: GetSystemMetrics(1 /* SM_CYSCREEN */) };
        }
    }
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    let x = area.left + ((area.right - area.left) - w) / 2;
    let y = area.top + ((area.bottom - area.top) - h) / 2;
    let hwnd = unsafe {
        CreateWindowExW(ex, wide(CLASS).as_ptr(), wide(ui::TITLE).as_ptr(), style, x, y, w, h, owner, 0, hinstance(), core::ptr::null())
    };
    theme::apply_window(hwnd);

    // Equal-width buttons across the footer, the main action first.
    let n = buttons.len().max(1) as i32;
    let row_w = cw - sc(PAD) * 2;
    let gap = sc(8);
    let bw = (row_w - gap * (n - 1)) / n;
    let by = ch - sc(FOOTER) + sc(16);
    let handles: Vec<HWND> = buttons.iter().enumerate().map(|(i, text)| {
        let id = ID_FIRST_BUTTON + i as i32;
        let b = if i == 0 {
            widgets::accent_button(hwnd, text, id, None, &fonts, Surface::Layer)
        } else {
            widgets::button(hwnd, text, id, None, &fonts, Surface::Layer)
        };
        ui::place(b, sc(PAD) + i as i32 * (bw + gap), by, bw, sc(32));
        b
    }).collect();

    STATE.with(|s| {
        *s.borrow_mut() = Some(Dialog {
            icon, heading: heading.to_string(), message: message.to_string(), buttons: handles.clone(),
            heading_font, glyph_font, _fonts: fonts, dpi, result: None,
        })
    });

    unsafe {
        MessageBeep(if icon == Icon::Warning { MB_ICONEXCLAMATION } else { MB_ICONASTERISK });
        if let Some(&first) = handles.first() {
            SetFocus(first);
        }
    }
    ui::modal_loop(owner, hwnd, &|| {
        STATE.with(|s| s.try_borrow().map(|g| g.as_ref().is_none_or(|d| d.result.is_some())).unwrap_or(false))
    }, &|_| false);
    let chosen = STATE.with(|s| s.borrow_mut().take()).and_then(|d| d.result.flatten()).unwrap_or(cancel);
    unsafe {
        if owner != 0 {
            SetForegroundWindow(owner); // before destroying, so the owner doesn't drop behind other apps
        }
        DestroyWindow(hwnd);
    }
    chosen
}

/// The system dialog, for the rare nested case.
fn fallback(owner: HWND, icon: Icon, heading: &str, message: &str, buttons: &[&str]) -> usize {
    let icon = match icon {
        Icon::Info => TD_INFORMATION_ICON,
        Icon::Warning => TD_WARNING_ICON,
    };
    let flags = if buttons.len() > 1 { TDCBF_OK_BUTTON | TDCBF_CANCEL_BUTTON } else { TDCBF_OK_BUTTON };
    let mut pressed = 0;
    unsafe {
        TaskDialog(owner, hinstance(), wide(ui::TITLE).as_ptr(), wide(heading).as_ptr(), wide(message).as_ptr(), flags, icon, &mut pressed);
    }
    if pressed == IDOK { 0 } else { buttons.len().saturating_sub(1) }
}

/// Height of `text` wrapped to `width`.
fn measure(text: &str, font: HFONT, width: i32) -> i32 {
    let w: Vec<u16> = text.encode_utf16().collect();
    let mut r = RECT { left: 0, top: 0, right: width, bottom: 0 };
    unsafe {
        let dc = GetDC(0);
        let old = SelectObject(dc, font);
        DrawTextW(dc, w.as_ptr(), w.len() as i32, &mut r, DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX);
        SelectObject(dc, old);
        ReleaseDC(0, dc);
    }
    r.bottom - r.top
}

impl Dialog {
    fn paint(&self, h: HWND, hdc: HDC) {
        let p = theme::palette();
        let sc = |v: i32| ui::scale(v, self.dpi);
        let mut rc = RECT::default();
        unsafe { GetClientRect(h, &mut rc) };
        let footer_top = rc.bottom - sc(FOOTER);
        theme::fill(hdc, RECT { bottom: footer_top, ..rc }, p.window);
        theme::fill(hdc, RECT { top: footer_top, ..rc }, p.layer);
        theme::fill(hdc, RECT { top: footer_top, bottom: footer_top + 1.max(sc(1)), ..rc }, p.stroke);

        // Icon badge: a tinted circle with a Segoe Fluent Icons glyph.
        let (glyph, fg, bg) = match self.icon {
            Icon::Warning => ('\u{E7BA}', if p.dark { theme::rgb(252, 225, 0) } else { theme::rgb(157, 93, 0) }, p.caution),
            Icon::Info => ('\u{E946}', p.accent, theme::mix(p.window, p.accent, if p.dark { 0.25 } else { 0.15 })),
        };
        let badge = RECT { left: sc(PAD), top: sc(22), right: sc(PAD + BADGE), bottom: sc(22 + BADGE) };
        theme::round_rect(hdc, badge, sc(BADGE / 2), bg, None);
        theme::text(hdc, &glyph.to_string(), badge, self.glyph_font, fg, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

        let left = sc(PAD + BADGE + 16);
        let right = rc.right - sc(PAD);
        let heading_h = measure(&self.heading, self.heading_font, right - left);
        theme::text(hdc, &self.heading, RECT { left, top: sc(20), right, bottom: sc(20) + heading_h }, self.heading_font, p.text, DT_WORDBREAK);
        if !self.message.is_empty() {
            let top = sc(20) + heading_h + sc(8);
            theme::text(hdc, &self.message, RECT { left, top, right, bottom: footer_top }, self._fonts.normal, p.text2, DT_WORDBREAK);
        }
    }

    fn choose(&mut self, index: Option<usize>) {
        self.result = Some(index);
    }
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_ERASEBKGND => {
            if with(|d| d.paint(h, w as HDC)).is_none() {
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
        WM_COMMAND => {
            let id = loword(w) as i32;
            with(|d| {
                if id == IDOK {
                    // Enter: the focused button, else the main action.
                    let focused = GetFocus();
                    d.choose(Some(d.buttons.iter().position(|&b| b == focused).unwrap_or(0)));
                } else if id == IDCANCEL {
                    d.choose(None);
                } else if id >= ID_FIRST_BUTTON && hiword(w) == BN_CLICKED {
                    d.choose(Some((id - ID_FIRST_BUTTON) as usize));
                }
            });
            return 0;
        }
        WM_CLOSE => {
            with(|d| d.choose(None));
            return 0;
        }
        WM_SETTINGCHANGE if theme::is_theme_change(m, l) => {
            theme::reload();
            theme::apply_window(h);
            theme::repaint(h);
        }
        _ => {}
    }
    DefWindowProcW(h, m, w, l)
}
