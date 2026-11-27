//! InputSwitch: switches monitor inputs over DDC/CI with per-setup hotkeys.
//! A Rust rewrite with no dependencies beyond the Windows API.

#![windows_subsystem = "windows"]

// The app's logic, with no UI.
mod config;
mod hotkey;
mod json;
mod matcher;
mod monitors;
mod registry;
mod reserved;
mod sys;
mod tray;

// Windows: a small UI toolkit, and the app's two windows.
mod hotkey_editor;
mod main_window;
mod ui;

mod diagnose;
mod ffi;

use ffi::*;

fn main() {
    // A windowed app has nowhere to print a panic, so a crash would be silent: write it to
    // crash.txt in the settings folder instead.
    std::panic::set_hook(Box::new(|info| {
        let text = format!("{}\n\n{info}\n\n{}", sys::display_version(), std::backtrace::Backtrace::force_capture());
        let _ = std::fs::create_dir_all(config::dir());
        let _ = std::fs::write(config::dir().join("crash.txt"), text);
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a.eq_ignore_ascii_case("--diagnose")) {
        diagnose::run(args.get(1).map(String::as_str));
        return;
    }

    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: core::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_LISTVIEW_CLASSES,
        };
        InitCommonControlsEx(&icc);
        ui::theme::reload();

        // One instance per config folder: a test copy with its own folder can run beside the real one.
        let name = wide(&format!(r"Local\InputSwitch-6d3f0b8e{}", config::instance_key()));
        // Never closed: holding it for the app's lifetime is what keeps a second copy out.
        CreateMutexW(core::ptr::null(), 1, name.as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            ui::dialog::alert(0, ui::dialog::Icon::Info, "InputSwitch is already running",
                "Look for its icon in the notification area. Click it to open the settings.");
            return;
        }
    }

    let open_settings = args.iter().any(|a| a.eq_ignore_ascii_case("--settings"));
    std::process::exit(tray::run(open_settings));
}
