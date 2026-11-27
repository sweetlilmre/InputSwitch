//! Themed message boxes in the style of a Windows 11 content dialog (like the C# app's
//! FluentDialog): an icon badge, a heading, a sentence of detail, and equal-width buttons named
//! after what they do, on a footer band. (`TaskDialog` can't follow dark mode.)

use super::draw::{self, rect};
use super::theme::{self, Surface};
use super::{widgets, Fonts, Slot};
use crate::ffi::*;

const CLASS: &str = "InputSwitch.Dialog";
const FIRST_BUTTON_ID: i32 = 300;

// Layout at 96 DPI. The height follows the text.
const WIDTH: i32 = 460;
const PAD: i32 = 24;
const TOP: i32 = 20;
const BADGE: i32 = 40;
const BADGE_GAP: i32 = 16;
const FOOTER: i32 = 64;
const BUTTON_H: i32 = 32;
const BUTTON_GAP: i32 = 8;

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Info,
    Warning,
}

/// A message with an OK button.
pub fn alert(owner: HWND, icon: Icon, heading: &str, message: &str) {
    show(owner, icon, heading, message, &["OK"]);
}

/// A confirmation with a warning icon: `action` (e.g. "Delete") or Cancel. True for `action`.
pub fn confirm(owner: HWND, heading: &str, message: &str, action: &str) -> bool {
    show(owner, Icon::Warning, heading, message, &[action, "Cancel"]) == 0
}

struct Dialog {
    icon: Icon,
    heading: String,
    message: String,
    buttons: Vec<HWND>,
    fonts: Fonts,
    dpi: u32,
    /// The button chosen; Some(None) when dismissed with Esc or the close box.
    result: Option<Option<usize>>,
}

thread_local! {
    static STATE: Slot<Dialog> = const { Slot::new() };
}

fn with<R>(f: impl FnOnce(&mut Dialog) -> R) -> Option<R> {
    STATE.with(|s| s.with(f))
}

/// Shows a dialog modally over `owner` (0 for none). `buttons[0]` is the main action (accent
/// coloured, Enter); the last button is what Esc and the close box mean. Returns the index of
/// the button chosen.
pub fn show(owner: HWND, icon: Icon, heading: &str, message: &str, buttons: &[&str]) -> usize {
    let cancel = buttons.len().saturating_sub(1);
    if STATE.with(|s| s.occupied()) {
        return show_system(owner, icon, heading, message, buttons); // one at a time (a dialog over a dialog)
    }
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| super::register_class(CLASS, wndproc, super::app_icon()));

    let dpi = unsafe { if owner != 0 { GetDpiForWindow(owner) } else { GetDpiForSystem() } }.max(96);
    let fonts = Fonts::new(dpi);
    let s = |v| super::scale(v, dpi);
    let text_w = s(WIDTH - PAD - BADGE - BADGE_GAP - PAD);
    let text_h = draw::text_height(heading, fonts.heading, text_w)
        + if message.is_empty() { 0 } else { s(8) + draw::text_height(message, fonts.normal, text_w) };
    let body_h = text_h.max(s(BADGE + 2)) * 96 / dpi as i32;
    let height = TOP + body_h + 24 + FOOTER;
    let hwnd = super::popup_window(CLASS, super::TITLE, owner, WIDTH, height, dpi);

    // Equal-width buttons across the footer, the main action first.
    let n = buttons.len().max(1) as i32;
    let bw = (WIDTH - 2 * PAD - BUTTON_GAP * (n - 1)) / n;
    let by = height - FOOTER + (FOOTER - BUTTON_H) / 2;
    let handles: Vec<HWND> = buttons.iter().enumerate().map(|(i, text)| {
        let (id, x) = (FIRST_BUTTON_ID + i as i32, PAD + i as i32 * (bw + BUTTON_GAP));
        let b = if i == 0 { widgets::accent_button(hwnd, text, id, None, &fonts, Surface::Layer) } else { widgets::button(hwnd, text, id, None, &fonts, Surface::Layer) };
        super::place(b, s(x), s(by), s(bw), s(BUTTON_H));
        b
    }).collect();

    STATE.with(|st| st.set(Dialog { icon, heading: heading.into(), message: message.into(), buttons: handles.clone(), fonts, dpi, result: None }));
    unsafe {
        MessageBeep(if icon == Icon::Warning { MB_ICONEXCLAMATION } else { MB_ICONASTERISK });
        if let Some(&main_action) = handles.first() {
            SetFocus(main_action);
        }
    }
    super::modal_loop(owner, hwnd, &|| STATE.with(|s| s.with(|d| d.result.is_some()).unwrap_or(true)), &|_| false);
    let chosen = STATE.with(|s| s.take()).and_then(|d| d.result.flatten()).unwrap_or(cancel);
    unsafe { DestroyWindow(hwnd) };
    chosen
}

/// The system dialog, for the rare dialog-over-a-dialog.
fn show_system(owner: HWND, icon: Icon, heading: &str, message: &str, buttons: &[&str]) -> usize {
    let icon = if icon == Icon::Warning { TD_WARNING_ICON } else { TD_INFORMATION_ICON };
    let flags = if buttons.len() > 1 { TDCBF_OK_BUTTON | TDCBF_CANCEL_BUTTON } else { TDCBF_OK_BUTTON };
    let mut pressed = 0;
    unsafe {
        TaskDialog(owner, hinstance(), wide(super::TITLE).as_ptr(), wide(heading).as_ptr(), wide(message).as_ptr(), flags, icon, &mut pressed);
    }
    if pressed == IDOK { 0 } else { buttons.len().saturating_sub(1) }
}

impl Dialog {
    fn paint(&self, hdc: HDC, rc: RECT) {
        let p = theme::palette();
        let s = |v: i32| super::scale(v, self.dpi);
        let footer_top = rc.bottom - s(FOOTER);
        draw::fill(hdc, RECT { bottom: footer_top, ..rc }, p.window);
        draw::fill(hdc, RECT { top: footer_top, ..rc }, p.layer);
        draw::fill(hdc, RECT { top: footer_top, bottom: footer_top + s(1).max(1), ..rc }, p.stroke);

        // The icon badge: a tinted circle with a glyph.
        let (glyph, fg, bg) = match self.icon {
            Icon::Warning => (widgets::ICON_WARNING, p.caution_text, p.caution),
            Icon::Info => (widgets::ICON_INFO, p.accent, theme::mix(p.window, p.accent, if p.dark { 0.25 } else { 0.15 })),
        };
        let badge = rect(s(PAD), s(TOP + 2), s(BADGE), s(BADGE));
        draw::round_rect(hdc, badge, s(BADGE / 2), bg, None);
        draw::glyph(hdc, glyph, badge, self.fonts.icon_20, fg, DT_CENTER | DT_VCENTER);

        let left = s(PAD + BADGE + BADGE_GAP);
        let right = rc.right - s(PAD);
        let heading_h = draw::text_height(&self.heading, self.fonts.heading, right - left);
        let heading = RECT { left, top: s(TOP), right, bottom: s(TOP) + heading_h };
        draw::text(hdc, &self.heading, heading, self.fonts.heading, p.text, DT_WORDBREAK);
        let message = RECT { top: heading.bottom + s(8), bottom: footer_top, ..heading };
        draw::text(hdc, &self.message, message, self.fonts.normal, p.text2, DT_WORDBREAK);
    }

    fn command(&mut self, id: i32, code: u16) {
        self.result = match id {
            // Enter: the focused button, else the main action.
            IDOK => Some(Some(self.buttons.iter().position(|&b| b == unsafe { GetFocus() }).unwrap_or(0))),
            IDCANCEL => Some(None),
            _ if code == BN_CLICKED && id >= FIRST_BUTTON_ID => Some(Some((id - FIRST_BUTTON_ID) as usize)),
            _ => return,
        };
    }
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            draw::paint_buffered(h, |hdc, rc| {
                if with(|d| d.paint(hdc, rc)).is_none() {
                    draw::fill(hdc, rc, theme::palette().window);
                }
            });
            return 0;
        }
        WM_DRAWITEM if widgets::draw(&*(l as *const DRAWITEMSTRUCT)) => return 1,
        WM_COMMAND => {
            with(|d| d.command(loword(w) as i32, hiword(w)));
            return 0;
        }
        WM_CLOSE => {
            with(|d| d.result = Some(None));
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

