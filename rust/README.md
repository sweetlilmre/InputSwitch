# InputSwitch: Rust spike

A rewrite of InputSwitch in Rust, aiming to be as small, fast and dependency-free as possible. It's a spike: it does everything the C# app does, but hasn't had the same polish or real-world use yet.

## Results

Measured on the same machine with the same monitors. Both builds were isolated with `INPUTSWITCH_CONFIG_DIR`.

| | C# (WPF + WinForms) | Rust (Win32) |
|---|---|---|
| Download, nothing else to install | ~165 MB exe (68 MB zipped) | **0.4 MB** exe |
| Download, runtime installed separately | 0.3 MB + .NET 10 Desktop Runtime | n/a (no runtime needed) |
| Memory while idle in the tray | ~49 MB | **~11 MB** |
| Time to show the settings window | ~0.9 s | **~0.2 s** |
| Third-party dependencies | none | **none** (not even binding crates) |

## What's the same

- **Behaviour:** the same setup detection (external monitors identified by EDID serial, laptop panel ignored, missing monitors tolerated), hotkeys, tray menu and notifications.
- **Settings file:** the same `%APPDATA%\InputSwitch\config.json`, in the same format. Existing Work and Home configurations load as they are.
- **Command-line options:** the same `--settings`, `--diagnose [file]` and `INPUTSWITCH_CONFIG_DIR`.
- **Tests:** the existing launch test (`tests/launch-smoke.ps1`) passes against it unchanged.
- **Other details:** the same single-instance rule, and the same "Start with Windows" registry value.

## What's different

- **Look:** native Win32 controls with the Windows visual style, Segoe UI and per-monitor DPI scaling, instead of WPF's Fluent theme. There's no dark mode or Mica background.
- **Dialogs:** Windows' own `TaskDialog`. Questions offer Yes / No rather than buttons labelled "Delete" or "Don't save".
- **Recording a hotkey:** click the box to start recording (it turns blue while it's listening). Win-key combinations are recorded too: while the box is listening, a short-lived keyboard hook stops Windows acting on the Win key. On Save, the editor refuses combinations Windows or another app already owns, and warns about known Windows shortcuts (such as Ctrl + Win + M for Magnifier), which Windows handles before any app sees them.
- **Renaming a monitor:** done in place in the list (select it and press **Rename**, or F2).

## How it's built

- `src/ffi.rs` declares, by hand, only the Windows API functions, structures and constants that are used. The libraries are user32, gdi32, shell32, dxva2, advapi32, comctl32, uxtheme, wtsapi32 and bcrypt, all part of Windows.
- `src/json.rs` is a small JSON reader and writer for the settings file.
- `build.rs` does three things, without any build dependencies or a resource compiler:
  - embeds `app.manifest` (Common Controls 6 and per-monitor DPI awareness) through the MSVC linker;
  - writes a Windows resource file holding the icon and version information, so the exe shows as **InputSwitch** with its icon in notifications, Explorer and Task Manager;
  - records the git commit.
- `assets/InputSwitch.ico` is the app icon: the same artwork as the C# app, at 16–64 px. The tray and window icons are loaded from the exe's resources.
- The release profile optimises for size: `opt-level = "z"`, LTO, one codegen unit, abort on panic, symbols stripped.

| File | Contents |
|---|---|
| `main.rs` | Entry point, single instance, `--diagnose` |
| `tray.rs` | Tray icon and menu, display-change handling, configuration matching, hotkeys |
| `settings.rs` | Settings window |
| `editor.rs` | Hotkey editor |
| `hotkey_box.rs` | Key-capture box for the hotkey editor (Win key included, via a short-lived keyboard hook) |
| `reserved.rs` | Known Windows shortcuts, warned about when saving a hotkey |
| `monitors.rs` | Enumeration (CCD + EDID), DDC/CI, capabilities parsing, switching |
| `config.rs`, `matcher.rs`, `hotkey.rs` | Settings model and file, setup matching, hotkeys |
| `ui.rs`, `sys.rs` | Win32 UI helpers, IDs, time, version, startup |

## Building and testing

You need Rust (stable, MSVC toolchain).

```
cd rust
cargo build --release          # target\release\InputSwitch.exe
cargo test --release           # 21 unit tests
pwsh ..\tests\launch-smoke.ps1 -Exe target\release\InputSwitch.exe
```

## Not done yet

- CI and release workflows for the Rust build. The existing ones still build the C# app.
- An Arm64 build. It needs the `aarch64-pc-windows-msvc` target; the code has no x64-specific parts.
- Dark mode.
