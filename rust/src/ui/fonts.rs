//! The fonts a window uses, by role, sized like the C# app's Fluent theme. Sizes are in pixels
//! at 96 DPI, as WPF measures them, and scaled for the window's DPI.

use super::scale;
use crate::ffi::*;

pub struct Fonts {
    /// Body text and controls (14).
    pub normal: HFONT,
    /// Field labels and emphasis (14 semibold).
    pub bold: HFONT,
    /// Secondary text: status lines, hints, subtitles, the version (12).
    pub small: HFONT,
    /// Key caps (12 semibold).
    pub key_cap: HFONT,
    /// Section titles: "Configurations", "Monitors", "Hotkeys" (16 semibold).
    pub section: HFONT,
    /// Message headings (20 semibold).
    pub heading: HFONT,
    /// The configuration name box (24 semibold).
    pub title: HFONT,
    /// Segoe Fluent Icons glyphs: in buttons (14), the status banner (16), the configurations
    /// list (18), message badges (20) and monitor cards (28).
    pub icon: HFONT,
    pub icon_16: HFONT,
    pub icon_18: HFONT,
    pub icon_20: HFONT,
    pub icon_28: HFONT,
}

impl Fonts {
    pub fn new(dpi: u32) -> Fonts {
        let text = |px, weight| font("Segoe UI", dpi, px, weight);
        let icons = icon_face();
        let icon = |px| font(icons, dpi, px, 400);
        Fonts {
            normal: text(14, 400),
            bold: text(14, 600),
            small: text(12, 400),
            key_cap: text(12, 600),
            section: text(16, 600),
            heading: text(20, 600),
            title: text(24, 600),
            icon: icon(14),
            icon_16: icon(16),
            icon_18: icon(18),
            icon_20: icon(20),
            icon_28: icon(28),
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        let all = [self.normal, self.bold, self.small, self.key_cap, self.section, self.heading, self.title,
            self.icon, self.icon_16, self.icon_18, self.icon_20, self.icon_28];
        for f in all {
            unsafe { DeleteObject(f) };
        }
    }
}

fn font(face: &str, dpi: u32, px: i32, weight: i32) -> HFONT {
    const DEFAULT_CHARSET: u32 = 1;
    const CLEARTYPE_QUALITY: u32 = 5;
    unsafe { CreateFontW(-scale(px, dpi), 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, 0, wide(face).as_ptr()) }
}

/// Segoe Fluent Icons on Windows 11; Segoe MDL2 Assets (mostly the same glyphs) on Windows 10.
fn icon_face() -> &'static str {
    static FACE: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    FACE.get_or_init(|| {
        let fonts = crate::registry::RegKey::open(HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts");
        let installed = fonts.is_some_and(|k| k.exists("Segoe Fluent Icons (TrueType)"));
        if installed { "Segoe Fluent Icons" } else { "Segoe MDL2 Assets" }
    })
}
