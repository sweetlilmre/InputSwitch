//! `--diagnose`: a read-only report of what the app sees, for troubleshooting.

use crate::{config, matcher, monitors, sys};
use std::fmt::Write as _;

/// Read-only report of what the app sees. Never changes an input.
pub fn run(output: Option<&str>) {
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
