//! InputSwitch: switches monitor inputs over DDC/CI with per-setup hotkeys.
//! A Rust rewrite with no dependencies beyond the Windows API.

#![windows_subsystem = "windows"]

mod config;
mod dialog;
mod editor;
mod ffi;
mod hotkey;
mod hotkey_box;
mod hotkey_list;
mod json;
mod matcher;
mod monitor_list;
mod monitors;
mod reserved;
mod settings;
mod sys;
mod theme;
mod tray;
mod ui;
mod widgets;

use ffi::*;
use std::fmt::Write as _;

fn main() {
    // A windowed app has nowhere to print a panic, so a crash would be silent: write it to
    // crash.txt in the settings folder instead.
    std::panic::set_hook(Box::new(|info| {
        let text = format!("{}\n\n{info}\n\n{}", sys::display_version(), std::backtrace::Backtrace::force_capture());
        let _ = std::fs::write(config::dir().join("crash.txt"), text);
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a.eq_ignore_ascii_case("--diagnose")) {
        diagnose(args.get(1).map(String::as_str));
        return;
    }

    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: core::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_LISTVIEW_CLASSES,
        };
        InitCommonControlsEx(&icc);
        theme::init_app();

        // One instance per config folder: a test copy with its own folder can run beside the real one.
        let name = wide(&format!(r"Local\InputSwitch-6d3f0b8e{}", config::instance_key()));
        CreateMutexW(core::ptr::null(), 1, name.as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            ui::alert(0, ui::Icon::Info, "InputSwitch is already running",
                "Look for its icon in the notification area. Click it to open the settings.");
            return;
        }
    }

    let open_settings = args.iter().any(|a| a.eq_ignore_ascii_case("--settings"));
    std::process::exit(tray::run(open_settings));
}

/// Read-only report of what the app sees. Never changes an input.
fn diagnose(output: Option<&str>) {
    let mut r = String::new();
    let _ = writeln!(r, "InputSwitch diagnostics — {}", sys::utc_now());
    let _ = writeln!(r, "Version:         {}", sys::full_version());
    let _ = writeln!(r);
    let found = monitors::enumerate();
    for m in &found {
        let _ = writeln!(r, "{}{}", m.display_name(), if m.internal { "  [built-in, ignored for setup matching]" } else { "" });
        let _ = writeln!(r, "  Id:            {}", m.id);
        let _ = writeln!(r, "  Model/serial:  {} / {}", m.model, m.serial.as_deref().unwrap_or("(none)"));
        let _ = writeln!(r, "  GDI device:    {} #{}", m.gdi, m.index);
        let current = monitors::current_input(m);
        let _ = writeln!(r, "  Current input: {}", current.map(monitors::long_name).unwrap_or_else(|| "no DDC/CI reply".into()));
        let caps = monitors::capabilities(m);
        let inputs = caps.as_deref().map(monitors::parse_capabilities).unwrap_or_default();
        let list = if inputs.is_empty() { "(none reported)".into() } else { inputs.iter().map(|&c| monitors::long_name(c)).collect::<Vec<_>>().join(", ") };
        let _ = writeln!(r, "  Inputs:        {list}");
        let _ = writeln!(r, "  Capabilities:  {}", caps.as_deref().unwrap_or("(no reply)"));
        let _ = writeln!(r);
    }
    let (cfg, _) = config::load();
    let matched = matcher::find(&cfg, &matcher::fingerprint(&found), None).map(|i| cfg.profiles[i].name.clone());
    let _ = writeln!(r, "Config file:     {}", config::file_path().display());
    let _ = writeln!(r, "Matching config: {}", matched.unwrap_or_else(|| "(none — a new one would be created)".into()));

    let path = output.map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("InputSwitch-diagnostics.txt"));
    let _ = std::fs::write(&path, r);
    if output.is_none() {
        let _ = std::process::Command::new("notepad.exe").arg(&path).spawn();
    }
}
