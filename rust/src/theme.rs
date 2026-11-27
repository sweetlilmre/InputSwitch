//! Light/dark theming without any UI framework: a WinUI-like palette (following the Windows
//! "app mode" setting and accent colour), dark title bars, dark common controls, and the colour
//! each control's background should have.
//!
//! The dark title bar uses a documented DWM attribute. Dark *controls* and menus use uxtheme
//! functions exported only by ordinal (the ones Explorer and Notepad++ use); if they're missing,
//! controls simply stay light.

use crate::ffi::*;
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
    /// The status banner, whose colour changes with the state.
    Banner,
    /// A monitor's card inside the Monitors card (ControlFillColorDefault over the card).
    Row,
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub window: u32,
    pub layer: u32,
    pub card: u32,
    pub stroke: u32,
    pub control: u32,
    pub control_hot: u32,
    pub control_pressed: u32,
    pub control_stroke: u32,
    pub text: u32,
    pub text2: u32,
    pub text_disabled: u32,
    pub accent: u32,
    pub accent_hot: u32,
    pub on_accent: u32,
    pub success: u32,
    pub caution: u32,
    pub neutral: u32,
    pub listening: u32,
    pub selected: u32,
}

/// COLORREF (0x00BBGGRR).
pub const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |c: u32, shift: u32| ((c >> shift) & 0xFF) as f32;
    let m = |shift| ((ch(a, shift) * (1.0 - t) + ch(b, shift) * t).round() as u32) << shift;
    m(0) | m(8) | m(16)
}

fn make_palette(dark: bool, accent: u32) -> Palette {
    if dark {
        let accent = mix(accent, rgb(255, 255, 255), 0.45); // WinUI uses a lighter accent on dark
        Palette {
            dark, window: rgb(32, 32, 32), layer: rgb(39, 39, 39), card: rgb(45, 45, 45), stroke: rgb(60, 60, 60),
            control: rgb(55, 55, 55), control_hot: rgb(62, 62, 62), control_pressed: rgb(48, 48, 48), control_stroke: rgb(75, 75, 75),
            text: rgb(255, 255, 255), text2: rgb(200, 200, 200), text_disabled: rgb(120, 120, 120),
            accent, accent_hot: mix(accent, rgb(255, 255, 255), 0.15), on_accent: rgb(0, 0, 0),
            success: rgb(57, 61, 27), caution: rgb(67, 53, 25), neutral: rgb(50, 50, 50),
            listening: mix(rgb(55, 55, 55), accent, 0.25), selected: rgb(58, 58, 58),
        }
    } else {
        Palette {
            dark, window: rgb(243, 243, 243), layer: rgb(249, 249, 249), card: rgb(255, 255, 255), stroke: rgb(229, 229, 229),
            control: rgb(253, 253, 253), control_hot: rgb(246, 246, 246), control_pressed: rgb(240, 240, 240), control_stroke: rgb(208, 208, 208),
            text: rgb(26, 26, 26), text2: rgb(96, 96, 96), text_disabled: rgb(160, 160, 160),
            accent, accent_hot: mix(accent, rgb(0, 0, 0), 0.1), on_accent: rgb(255, 255, 255),
            success: rgb(223, 246, 221), caution: rgb(255, 244, 206), neutral: rgb(240, 240, 240),
            listening: mix(rgb(255, 255, 255), accent, 0.12), selected: rgb(234, 234, 234),
        }
    }
}

#[derive(Clone, Copy)]
struct Style {
    surface: Surface,
    secondary: bool,
}

thread_local! {
    static PALETTE: Cell<Palette> = Cell::new(make_palette(false, rgb(0, 95, 184)));
    /// The Theme setting: Some(dark) overrides Windows; None follows it.
    static OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
    static BANNER: Cell<u32> = const { Cell::new(0) };
    static BRUSHES: RefCell<HashMap<u32, isize>> = RefCell::new(HashMap::new());
    static STYLES: RefCell<HashMap<HWND, Style>> = RefCell::new(HashMap::new());
}

pub fn palette() -> Palette {
    PALETTE.with(|p| p.get())
}

/// A solid brush for a colour. Brushes are cached for the life of the app (there are only a few).
pub fn brush(color: u32) -> isize {
    BRUSHES.with(|b| *b.borrow_mut().entry(color).or_insert_with(|| unsafe { CreateSolidBrush(color) }))
}

fn read_dword(path: &str, name: &str) -> Option<u32> {
    unsafe {
        let mut key: HKEY = 0;
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(path).as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return None;
        }
        let (mut value, mut len, mut kind) = (0u32, 4u32, 0u32);
        let ok = RegQueryValueExW(key, wide(name).as_ptr(), core::ptr::null_mut(), &mut kind,
            &mut value as *mut u32 as *mut u8, &mut len) == 0 && kind == 4; // REG_DWORD
        RegCloseKey(key);
        ok.then_some(value)
    }
}

/// Windows' "Choose your app mode" setting.
fn system_dark() -> bool {
    read_dword(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "AppsUseLightTheme") == Some(0)
}

/// The accent colour chosen in Windows settings (stored as 0xAABBGGRR).
fn system_accent() -> u32 {
    read_dword(r"Software\Microsoft\Windows\DWM", "AccentColor").map(|c| c & 0x00FF_FFFF).unwrap_or(rgb(0, 95, 184))
}

/// Re-reads the Windows settings (and the user's light/dark choice). Returns true if anything changed.
pub fn reload() -> bool {
    let dark = OVERRIDE.with(|o| o.get()).unwrap_or_else(system_dark);
    let new = make_palette(dark, system_accent());
    let old = palette();
    PALETTE.with(|p| p.set(new));
    BANNER.with(|b| if b.get() == 0 || b.get() == old.neutral { b.set(new.neutral) });
    unsafe {
        // SetPreferredAppMode: AllowDark (1) follows Windows; ForceDark (2) / ForceLight (3) for the
        // user's choice, so menus and scroll bars match it too.
        let mode = match OVERRIDE.with(|o| o.get()) { None => 1, Some(true) => 2, Some(false) => 3 };
        if let Some(f) = uxtheme::<unsafe extern "system" fn(i32) -> i32>(135) {
            f(mode);
        }
        if let Some(f) = uxtheme::<unsafe extern "system" fn()>(104) {
            f(); // RefreshImmersiveColorPolicyState
        }
        if let Some(f) = uxtheme::<unsafe extern "system" fn()>(136) {
            f(); // FlushMenuThemes: menus pick up the new mode
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

/// Applies the Theme setting (System follows Windows). Returns true if the look changed.
pub fn set_choice(choice: crate::config::Theme) -> bool {
    use crate::config::Theme;
    OVERRIDE.with(|o| o.set(match choice { Theme::System => None, Theme::Light => Some(false), Theme::Dark => Some(true) }));
    reload()
}

/// Call once at startup: lets menus and controls follow the dark setting.
pub fn init_app() {
    reload();
}

/// Dark or light title bar (and dark-capable window) to match the palette.
pub fn apply_window(h: HWND) {
    let dark = palette().dark as BOOL;
    unsafe {
        if let Some(f) = uxtheme::<unsafe extern "system" fn(HWND, BOOL) -> BOOL>(133) {
            f(h, dark); // AllowDarkModeForWindow
        }
        DwmSetWindowAttribute(h, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const BOOL as *const _, 4);
    }
}

#[derive(Clone, Copy)]
pub enum ControlKind {
    ListView,
    ListBox,
    Edit,
    ComboBox,
}

/// Dark or light visual style for a common control, to match the palette.
pub fn apply_control(h: HWND, kind: ControlKind) {
    let p = palette();
    unsafe {
        if let Some(f) = uxtheme::<unsafe extern "system" fn(HWND, BOOL) -> BOOL>(133) {
            f(h, p.dark as BOOL);
        }
        let theme = match (kind, p.dark) {
            (ControlKind::Edit | ControlKind::ComboBox, true) => "DarkMode_CFD",
            (_, true) => "DarkMode_Explorer",
            (ControlKind::Edit | ControlKind::ComboBox, false) => "CFD",
            (_, false) => "Explorer",
        };
        SetWindowTheme(h, wide(theme).as_ptr(), core::ptr::null());
        if let ControlKind::ListView = kind {
            let bg = p.card;
            SendMessageW(h, LVM_SETBKCOLOR, 0, bg as LPARAM);
            SendMessageW(h, LVM_SETTEXTBKCOLOR, 0, bg as LPARAM);
            SendMessageW(h, LVM_SETTEXTCOLOR, 0, p.text as LPARAM);
            let header = SendMessageW(h, LVM_GETHEADER, 0, 0) as HWND;
            if header != 0 {
                let header_theme = if p.dark { "DarkMode_ItemsView" } else { "ItemsView" };
                SetWindowTheme(header, wide(header_theme).as_ptr(), core::ptr::null());
            }
            // The dark header style keeps dark text: colour it ourselves (header notifications go
            // to the list view, so it's subclassed).
            SetWindowSubclass(h, listview_proc, 7, 0);
        }
        SendMessageW(h, WM_THEMECHANGED, 0, 0);
    }
}

unsafe extern "system" fn listview_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM, _id: usize, _data: usize) -> LRESULT {
    if m == WM_NOTIFY {
        let hdr = &*(l as *const NMHDR);
        if hdr.code == NM_CUSTOMDRAW && hdr.hwndFrom == SendMessageW(h, LVM_GETHEADER, 0, 0) as HWND {
            let cd = &*(l as *const NMCUSTOMDRAW);
            match cd.dwDrawStage {
                CDDS_PREPAINT => return CDRF_NOTIFYITEMDRAW,
                CDDS_ITEMPREPAINT => {
                    SetTextColor(cd.hdc, palette().text2);
                    return 0; // CDRF_DEFAULT
                }
                _ => {}
            }
        }
    }
    if m == WM_NCDESTROY {
        RemoveWindowSubclass(h, listview_proc, 7);
    }
    DefSubclassProc(h, m, w, l)
}

/// Records what a control sits on, and whether its text is secondary (greyer).
pub fn set_surface(h: HWND, surface: Surface, secondary: bool) {
    STYLES.with(|s| s.borrow_mut().insert(h, Style { surface, secondary }));
}

pub fn forget(h: HWND) {
    STYLES.with(|s| s.borrow_mut().remove(&h));
}

pub fn surface_of(h: HWND) -> Surface {
    STYLES.with(|s| s.borrow().get(&h).map(|x| x.surface)).unwrap_or(Surface::Window)
}

fn is_secondary(h: HWND) -> bool {
    STYLES.with(|s| s.borrow().get(&h).map(|x| x.secondary)).unwrap_or(false)
}

pub fn color_of(surface: Surface) -> u32 {
    let p = palette();
    match surface {
        Surface::Window => p.window,
        Surface::Layer => p.layer,
        Surface::Card => p.card,
        Surface::Banner => BANNER.with(|b| b.get()),
        Surface::Row => if p.dark { mix(p.card, rgb(255, 255, 255), 0.06) } else { rgb(255, 255, 255) },
    }
}

/// SystemFillColorSuccess: the green of ticks and status dots.
pub fn success_text() -> u32 {
    if palette().dark { rgb(108, 203, 95) } else { rgb(15, 123, 15) }
}

/// SystemFillColorCaution: the amber of warning icons.
pub fn caution_text() -> u32 {
    if palette().dark { rgb(252, 225, 0) } else { rgb(157, 93, 0) }
}

pub fn set_banner(color: u32) {
    BANNER.with(|b| b.set(color));
}

/// Handles WM_CTLCOLORSTATIC / WM_CTLCOLOREDIT / WM_CTLCOLORLISTBOX for a window's children.
pub fn ctl_color(msg: u32, hdc: HDC, control: HWND) -> Option<LRESULT> {
    let p = palette();
    unsafe {
        match msg {
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                let bg = color_of(surface_of(control));
                SetTextColor(hdc, if is_secondary(control) { p.text2 } else { p.text });
                SetBkColor(hdc, bg);
                Some(brush(bg))
            }
            WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                // Inputs use the control colour, unless they sit flat on a surface (the configurations list).
                let registered = STYLES.with(|s| s.borrow().get(&control).map(|x| x.surface));
                let bg = registered.map(color_of).unwrap_or(p.control);
                SetTextColor(hdc, p.text);
                SetBkColor(hdc, bg);
                Some(brush(bg))
            }
            _ => None,
        }
    }
}

// ------------------------------------------------------------------------------------------ painting

/// Pass as the fill to draw only the border.
pub const NO_FILL: u32 = u32::MAX;

/// Fills a rounded rectangle, optionally with a 1-pixel border.
pub fn round_rect(hdc: HDC, r: RECT, radius: i32, fill: u32, border: Option<u32>) {
    unsafe {
        let pen = match border {
            Some(c) => CreatePen(PS_SOLID, 1, c),
            None => GetStockObject(NULL_PEN),
        };
        let old_pen = SelectObject(hdc, pen);
        let fill_brush = if fill == NO_FILL { GetStockObject(NULL_BRUSH) } else { brush(fill) };
        let old_brush = SelectObject(hdc, fill_brush);
        // RoundRect excludes the right/bottom edge when there's no pen; add one to compensate.
        let extra = if border.is_none() { 1 } else { 0 };
        RoundRect(hdc, r.left, r.top, r.right + extra, r.bottom + extra, radius * 2, radius * 2);
        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        if border.is_some() {
            DeleteObject(pen);
        }
    }
}

pub fn fill(hdc: HDC, r: RECT, color: u32) {
    unsafe { FillRect(hdc, &r, brush(color)) };
}

/// Draws text in a rectangle.
pub fn text(hdc: HDC, s: &str, mut r: RECT, font: HFONT, color: u32, format: u32) {
    if s.is_empty() {
        return;
    }
    let w: Vec<u16> = s.encode_utf16().collect();
    unsafe {
        let old = SelectObject(hdc, font);
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, color);
        DrawTextW(hdc, w.as_ptr(), w.len() as i32, &mut r, format | DT_NOPREFIX);
        SelectObject(hdc, old);
    }
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
    let s = unsafe {
        let p = l as *const u16;
        let mut n = 0;
        while *p.add(n) != 0 && n < 64 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
    };
    s == "ImmersiveColorSet"
}
