//! Light and dark theming without a UI framework: a WinUI-like palette (following Windows' app
//! mode and accent colour, or the user's choice), dark title bars and common controls, and the
//! colour each control's background should have.
//!
//! The dark title bar uses a documented DWM attribute. Dark *controls* and menus use uxtheme
//! functions exported only by ordinal (the ones Explorer and Notepad++ use); if they're missing,
//! controls simply stay light.

use crate::ffi::*;
use crate::registry::RegKey;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// What a control sits on, so its background can match.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Surface {
    /// The window itself (the main window's navigation pane, dialogs).
    Window,
    /// The main window's content area, behind the cards.
    Layer,
    /// A card (Monitors, Hotkeys).
    Card,
    /// An info bar (a tinted strip with a message), by severity.
    InfoBar(Severity),
    /// A row inside a card.
    Row,
    /// A selected item in a list (an accent tint).
    Selection,
}

/// How an info bar is tinted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Informational,
    Success,
    Warning,
}

/// COLORREF (0x00BBGGRR).
pub const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

/// `a` blended towards `b` by `t` (0 = a, 1 = b).
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |c: u32, shift: u32| ((c >> shift) & 0xFF) as f32;
    let m = |shift| ((ch(a, shift) * (1.0 - t) + ch(b, shift) * t).round() as u32) << shift;
    m(0) | m(8) | m(16)
}

const WHITE: u32 = rgb(255, 255, 255);
const BLACK: u32 = rgb(0, 0, 0);
const DEFAULT_ACCENT: u32 = rgb(0, 95, 184);

/// The colours of the current theme, named after their WinUI roles.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    // Surfaces.
    pub window: u32,
    pub layer: u32,
    pub card: u32,
    pub stroke: u32,
    /// A row inside a card (ControlFillColorDefault over the card) and its border.
    pub row: u32,
    pub row_stroke: u32,
    // Controls.
    pub control: u32,
    pub control_hot: u32,
    pub control_pressed: u32,
    pub control_stroke: u32,
    /// A text box: fill (and while focused), border, and the brighter bottom line.
    pub field: u32,
    pub field_focused: u32,
    pub field_stroke: u32,
    pub field_underline: u32,
    /// A chip or key cap on a row (ControlFillColorSecondary) and its border.
    pub chip: u32,
    pub chip_stroke: u32,
    /// A selected item in a navigation list (subtle) and in a list box (an accent tint).
    pub selected: u32,
    pub selection: u32,
    /// A drop-down list's background.
    pub flyout: u32,
    // Text.
    pub text: u32,
    pub text2: u32,
    pub text_disabled: u32,
    pub accent: u32,
    pub accent_hot: u32,
    pub on_accent: u32,
    // Status: banner and pill backgrounds, and the green/amber of icons and dots.
    pub success: u32,
    pub caution: u32,
    pub neutral: u32,
    pub success_text: u32,
    pub caution_text: u32,
}

fn make_palette(dark: bool, accent: u32) -> Palette {
    if dark {
        let selection = mix(accent, BLACK, 0.25);
        let accent = mix(accent, WHITE, 0.45); // WinUI uses a lighter accent on dark
        let card = rgb(45, 45, 45);
        let on_card = |t| mix(card, WHITE, t); // the translucent white fills, over the card
        Palette {
            dark,
            window: rgb(32, 32, 32), layer: rgb(39, 39, 39), card, stroke: rgb(60, 60, 60),
            row: on_card(0.06), row_stroke: on_card(0.07),
            control: rgb(55, 55, 55), control_hot: rgb(62, 62, 62), control_pressed: rgb(48, 48, 48), control_stroke: rgb(75, 75, 75),
            field: on_card(0.09), field_focused: rgb(31, 31, 31), field_stroke: on_card(0.08), field_underline: on_card(0.55),
            chip: on_card(0.11), chip_stroke: on_card(0.13),
            selected: rgb(58, 58, 58), selection, flyout: rgb(44, 44, 44),
            text: WHITE, text2: rgb(200, 200, 200), text_disabled: rgb(120, 120, 120),
            accent, accent_hot: mix(accent, WHITE, 0.15), on_accent: BLACK,
            success: rgb(57, 61, 27), caution: rgb(67, 53, 25), neutral: rgb(50, 50, 50),
            success_text: rgb(108, 203, 95), caution_text: rgb(252, 225, 0),
        }
    } else {
        let card = WHITE;
        let on_card = |t| mix(card, BLACK, t); // the translucent black strokes, over the card
        Palette {
            dark,
            window: rgb(243, 243, 243), layer: rgb(249, 249, 249), card, stroke: rgb(229, 229, 229),
            row: WHITE, row_stroke: on_card(0.06),
            control: rgb(253, 253, 253), control_hot: rgb(246, 246, 246), control_pressed: rgb(240, 240, 240), control_stroke: rgb(208, 208, 208),
            field: WHITE, field_focused: WHITE, field_stroke: on_card(0.07), field_underline: on_card(0.45),
            chip: rgb(249, 249, 249), chip_stroke: on_card(0.07),
            selected: rgb(234, 234, 234), selection: mix(accent, WHITE, 0.75), flyout: rgb(249, 249, 249),
            text: rgb(26, 26, 26), text2: rgb(96, 96, 96), text_disabled: rgb(160, 160, 160),
            accent, accent_hot: mix(accent, BLACK, 0.1), on_accent: WHITE,
            success: rgb(223, 246, 221), caution: rgb(255, 244, 206), neutral: rgb(240, 240, 240),
            success_text: rgb(15, 123, 15), caution_text: rgb(157, 93, 0),
        }
    }
}

#[derive(Clone, Copy)]
struct Style {
    surface: Surface,
    secondary: bool,
}

thread_local! {
    static PALETTE: Cell<Palette> = Cell::new(make_palette(false, DEFAULT_ACCENT));
    /// The app's choice: Some(dark) overrides Windows; None follows it.
    static OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
    static BRUSHES: RefCell<HashMap<u32, isize>> = RefCell::new(HashMap::new());
    static STYLES: RefCell<HashMap<HWND, Style>> = RefCell::new(HashMap::new());
}

pub fn palette() -> Palette {
    PALETTE.with(|p| p.get())
}

/// A solid brush for a colour, cached for the life of the app (there are only a few).
pub fn brush(color: u32) -> isize {
    BRUSHES.with(|b| *b.borrow_mut().entry(color).or_insert_with(|| unsafe { CreateSolidBrush(color) }))
}

/// Windows' "Choose your app mode" setting.
fn system_dark() -> bool {
    RegKey::open(HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|k| k.dword("AppsUseLightTheme")) == Some(0)
}

/// The accent colour chosen in Windows settings (stored as 0xAABBGGRR).
fn system_accent() -> u32 {
    RegKey::open(HKEY_CURRENT_USER, r"Software\Microsoft\Windows\DWM")
        .and_then(|k| k.dword("AccentColor"))
        .map_or(DEFAULT_ACCENT, |c| c & 0x00FF_FFFF)
}

/// Re-reads the Windows settings (and the user's light/dark choice), and tells menus and
/// scroll bars. Returns true if the look changed.
pub fn reload() -> bool {
    let choice = OVERRIDE.with(|o| o.get());
    let new = make_palette(choice.unwrap_or_else(system_dark), system_accent());
    let old = palette();
    PALETTE.with(|p| p.set(new));
    unsafe {
        // SetPreferredAppMode: AllowDark (1) follows Windows; ForceDark (2) / ForceLight (3).
        let mode = match choice { None => 1, Some(true) => 2, Some(false) => 3 };
        if let Some(set_preferred_app_mode) = uxtheme::<unsafe extern "system" fn(i32) -> i32>(135) {
            set_preferred_app_mode(mode);
        }
        if let Some(refresh_immersive_color_policy_state) = uxtheme::<unsafe extern "system" fn()>(104) {
            refresh_immersive_color_policy_state();
        }
        if let Some(flush_menu_themes) = uxtheme::<unsafe extern "system" fn()>(136) {
            flush_menu_themes();
        }
    }
    new.dark != old.dark || new.accent != old.accent
}

/// A uxtheme function exported only by ordinal.
unsafe fn uxtheme<F: Copy>(ordinal: usize) -> Option<F> {
    let module = LoadLibraryW(wide("uxtheme.dll").as_ptr());
    if module == 0 {
        return None;
    }
    let p = GetProcAddress(module, ordinal as *const u8);
    (!p.is_null()).then(|| core::mem::transmute_copy::<*const core::ffi::c_void, F>(&p))
}

/// Chooses dark (`Some(true)`), light (`Some(false)`) or Windows' setting (`None`). Returns true
/// if the look changed.
pub fn set_dark(dark: Option<bool>) -> bool {
    OVERRIDE.with(|o| o.set(dark));
    reload()
}

unsafe fn allow_dark(h: HWND, dark: bool) {
    if let Some(allow_dark_mode_for_window) = uxtheme::<unsafe extern "system" fn(HWND, BOOL) -> BOOL>(133) {
        allow_dark_mode_for_window(h, dark as BOOL);
    }
}

/// Dark or light title bar (and dark-capable window) to match the palette.
pub fn apply_window(h: HWND) {
    let dark = palette().dark as BOOL;
    unsafe {
        allow_dark(h, dark != 0);
        DwmSetWindowAttribute(h, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const BOOL as *const _, 4);
    }
}

#[derive(Clone, Copy)]
pub enum ControlKind {
    /// Also used for anything that only needs dark scroll bars.
    ListBox,
    Edit,
}

/// Dark or light visual style for a common control, to match the palette.
pub fn apply_control(h: HWND, kind: ControlKind) {
    let p = palette();
    unsafe {
        allow_dark(h, p.dark);
        let theme = match (kind, p.dark) {
            (ControlKind::Edit, true) => "DarkMode_CFD",
            (_, true) => "DarkMode_Explorer",
            (ControlKind::Edit, false) => "CFD",
            (_, false) => "Explorer",
        };
        SetWindowTheme(h, wide(theme).as_ptr(), core::ptr::null());
        SendMessageW(h, WM_THEMECHANGED, 0, 0);
    }
}


/// Records what a control sits on, and whether its text is secondary (greyer).
pub fn set_surface(h: HWND, surface: Surface, secondary: bool) {
    STYLES.with(|s| s.borrow_mut().insert(h, Style { surface, secondary }));
}

/// Forgets a destroyed control's surface.
pub fn forget(h: HWND) {
    STYLES.with(|s| s.borrow_mut().remove(&h));
}

fn style_of(h: HWND) -> Option<Style> {
    STYLES.with(|s| s.borrow().get(&h).copied())
}

pub fn surface_of(h: HWND) -> Surface {
    style_of(h).map_or(Surface::Window, |s| s.surface)
}

pub fn color_of(surface: Surface) -> u32 {
    let p = palette();
    match surface {
        Surface::Window => p.window,
        Surface::Layer => p.layer,
        Surface::Card => p.card,
        Surface::InfoBar(Severity::Informational) => p.neutral,
        Surface::InfoBar(Severity::Success) => p.success,
        Surface::InfoBar(Severity::Warning) => p.caution,
        Surface::Row => p.row,
        Surface::Selection => p.selection,
    }
}

/// Handles WM_CTLCOLORSTATIC / BTN / EDIT / LISTBOX for a window's children: labels take their
/// surface's colour; inputs the control colour unless they sit flat on a surface.
pub fn ctl_color(msg: u32, hdc: HDC, control: HWND) -> Option<LRESULT> {
    let p = palette();
    let style = style_of(control);
    let (bg, fg) = match msg {
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            let secondary = style.is_some_and(|s| s.secondary);
            (color_of(style.map_or(Surface::Window, |s| s.surface)), if secondary { p.text2 } else { p.text })
        }
        WM_CTLCOLOREDIT => {
            let field = if unsafe { GetFocus() } == control { p.field_focused } else { p.field };
            (style.map_or(field, |s| color_of(s.surface)), p.text)
        }
        WM_CTLCOLORLISTBOX => (style.map_or(p.control, |s| color_of(s.surface)), p.text),
        _ => return None,
    };
    Some(set_control_colors(hdc, fg, bg))
}

/// Sets a control's text and background colour; returns the background brush for WM_CTLCOLOR*.
pub fn set_control_colors(hdc: HDC, text: u32, background: u32) -> LRESULT {
    unsafe {
        SetTextColor(hdc, text);
        SetBkColor(hdc, background);
    }
    brush(background)
}

/// Redraws a window and all its children (after the theme changes).
pub fn repaint(h: HWND) {
    unsafe { RedrawWindow(h, core::ptr::null(), 0, RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_FRAME) };
}

/// True for the WM_SETTINGCHANGE that Windows sends when the app mode or accent changes.
pub fn is_theme_change(msg: u32, l: LPARAM) -> bool {
    if msg != WM_SETTINGCHANGE || l == 0 {
        return false;
    }
    let area = unsafe {
        let p = l as *const u16;
        let n = (0..64).take_while(|&i| *p.add(i) != 0).count();
        String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
    };
    area == "ImmersiveColorSet"
}
