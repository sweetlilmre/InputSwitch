# InputSwitch: technical notes

This is the developer and power-user companion to the [user guide](../README.md). It covers building, how the app works inside, the settings file, command-line options and developer tools.

---

## Contents

- [Building and publishing](#building-and-publishing)
- [Project layout](#project-layout)
- [How it works](#how-it-works)
- [The settings file](#the-settings-file)
- [Command-line options](#command-line-options)
- [Input codes reference](#input-codes-reference)
- [Developer tool: window screenshots](#developer-tool-window-screenshots)
- [Known limitations](#known-limitations)

---

## Building and publishing

You need the **.NET 10 SDK**. The app has no third-party dependencies.

```
dotnet build -c Release
bin\Release\net10.0-windows\InputSwitch.exe
```

A single executable that doesn't need .NET installed:

```
dotnet publish -c Release -r win-x64 --self-contained -p:PublishSingleFile=true
```

A framework-dependent build needs the **.NET 10 Desktop Runtime** on the target machine.

The running app locks `InputSwitch.exe`, so exit it from the tray before rebuilding Release. A Debug build (`dotnet build -c Debug`) is a quick way to check the code compiles while it's running.

## Project layout

| Path | Contents |
|---|---|
| `Program.cs` | Entry point: single-instance check, `--diagnose`, `--settings`. |
| `TrayApp.cs` | The tray icon and menu, display-change handling, profile activation and hotkey registration. |
| `Startup.cs` | "Start with Windows" (per-user Run key). |
| `TrayIconFactory.cs` | Draws the tray and window icon at runtime (no `.ico` file). |
| `Config/` | `AppConfig` (data model), `ConfigStore` (JSON load/save), `ProfileMatcher` (which configuration fits the connected monitors). |
| `Hotkeys/` | `Hotkey` (key + modifiers) and `SystemWindow` (hidden window that owns the global hotkeys and receives display/device broadcasts). |
| `Monitors/` | `MonitorService` (enumeration and DDC/CI), `Edid`, `InputSources` (VCP 0x60 names and capabilities parsing), `InputSwitcher` (switch/toggle/cycle logic). |
| `Native/` | P/Invoke declarations: user32, dxva2, and the CCD display-configuration API. |
| `UI/` | WPF windows: settings (`ConfigWindow`), hotkey editor (`BindingEditorWindow`), themed dialog (`FluentDialog`), shared styles and view models, and the WinForms/WPF bridge (`WpfHost`, `SettingsWindow`). |
| `tools/WindowCapture/` | Developer screenshot tool (separate project, excluded from the app build). |

## How it works

### Switching inputs

InputSwitch sends the VESA MCCS **Input Select** command, VCP code `0x60`, over **DDC/CI**. It uses the Windows monitor-configuration API in `dxva2.dll`:

- `GetPhysicalMonitorsFromHMONITOR`
- `SetVCPFeature`
- `GetVCPFeatureAndVCPFeatureReply`, which reads the current input
- `CapabilitiesRequestAndCapabilitiesReply`, which lists the supported inputs, parsed from the `vcp(… 60(…) …)` section of the capabilities string

All DDC/CI traffic is serialised behind a single lock, because the I²C bus is slow and some monitors misbehave under concurrent requests. Reads and writes are retried up to three times. Capabilities requests can take a second or two, so the UI makes them off the UI thread.

### Identifying monitors

The CCD API (`QueryDisplayConfig` / `DisplayConfigGetDeviceInfo`) gives each active display path's:

- GDI device name (`\\.\DISPLAYn`), used to find its `HMONITOR` for DDC/CI;
- monitor device path, used to find its cached EDID under `HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY\<model>\<instance>\Device Parameters\EDID`;
- output technology, used to recognise the laptop's built-in panel (internal, LVDS, embedded DisplayPort/UDI).

A monitor's ID is `<manufacturer><product code>#<serial>`, e.g. `DELD0D9#BPG5ZT2`, using the EDID serial descriptor if present, otherwise the numeric serial. That way it follows the monitor across docks and ports, and tells apart identical models. A monitor with no serial falls back to `<model>@<device instance>`, which is stable per port.

### Choosing a configuration

A setup's **fingerprint** is the set of IDs of the connected *external* monitors. The built-in panel is excluded, so lid open/closed doesn't matter. `ProfileMatcher.Match`:

1. returns a configuration whose monitor set **equals** the fingerprint (the first in list order if several);
2. otherwise considers configurations whose monitor set is a **superset** of the fingerprint. This handles a monitor that's switched off, or one that drops off after being switched to another source. It prefers the currently active configuration, then the one with the fewest monitors, then the oldest;
3. otherwise reports an unknown setup. `TrayApp` then creates a configuration for it with hotkeys disabled. On the very first run (no configurations at all) it opens the settings window; otherwise it shows a notification.

An empty fingerprint (no external monitors) matches nothing and creates nothing.

### Detecting changes

Re-evaluation is triggered by `WM_DISPLAYCHANGE` and `WM_DEVICECHANGE`, received on a hidden top-level window (message-only windows don't receive these broadcasts). It's also triggered by `SystemEvents.DisplaySettingsChanged`, by resume from sleep, and by session unlock. Events are debounced for 2.5 seconds, because docking produces a burst of them as monitors come up one at a time.

### Hotkeys

Hotkeys are registered with `RegisterHotKey` (with `MOD_NOREPEAT`) on the hidden window, and only for the active configuration when it's enabled. A combination must include Ctrl, Alt or Win, or be F13–F24. All hotkeys are unregistered while the settings window is open, so combinations can be recorded and an accidental press won't switch a screen.

A binding with one input sets that input. With several inputs, it reads the current input and moves to the next in the list, wrapping around. If the monitor doesn't answer, as some don't right after a switch, it uses the input it last set. If the current input isn't in the list, it goes to the first.

### User interface

- **Tray, hotkeys and display detection** run on WinForms, and the WinForms message loop drives the app.
- **Windows and dialogs** are WPF with the built-in **Fluent** theme (`Application.ThemeMode = System`), which follows the Windows light/dark setting and uses Mica on Windows 11. `ThemeMode` is marked experimental, so `WPF0001` is suppressed in the project file.
- **WPF is loaded lazily.** `TrayApp` only talks to the settings window through `ISettingsWindow`, which contains no WPF types. That way the JIT doesn't load WPF assemblies until a window is first created (`WpfHost.EnsureInitialized`). `AppDialogs` does the same for dialogs shown outside a window, such as at startup.
- **Keyboard input in the modeless settings window** relies on `ElementHost.EnableModelessKeyboardInterop`.
- **The Cancel button in the modeless window closes it explicitly**, because `IsCancel` only closes modal dialogs.
- **WinForms and WPF share many type names**, so the project removes the global `System.Windows.Forms` and `System.Drawing` usings, and each file imports what it needs.

## The settings file

Stored at `%APPDATA%\InputSwitch\config.json`. It's written atomically (to a temporary file, then moved into place). If it can't be read, it's kept as `config.json.bad` and the app starts with fresh settings.

```json
{
  "Profiles": [
    {
      "Id": "55daf5ea8adb41cbae76cdfb224b8d9c",
      "Name": "Work",
      "Enabled": true,
      "CreatedUtc": "2026-11-11T12:47:11Z",
      "Monitors": [
        {
          "MonitorId": "DELD0D9#BPG5ZT2",
          "Name": "DELL P2419H",
          "Model": "DELD0D9",
          "Serial": "BPG5ZT2",
          "SupportedInputs": [ 1, 15, 17 ]
        }
      ],
      "Bindings": [
        {
          "Id": "ad46d3cbe74d4227a47f209b3a8b712b",
          "MonitorId": "DELD0D9#BPG5ZT2",
          "Hotkey": { "Modifiers": "Alt, Control", "Key": "M" },
          "Inputs": [ 15, 17 ]
        }
      ]
    }
  ],
  "ShowNotifications": true
}
```

- `SupportedInputs` and `Inputs` are VCP 0x60 codes in decimal (15 = `0x0F` DisplayPort 1, 17 = `0x11` HDMI 1).
- `Key` is a `System.Windows.Forms.Keys` name. `Modifiers` is any combination of `Alt`, `Control`, `Shift` and `Win`.

**Start with Windows** writes a `InputSwitch` value under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`. No admin rights are needed.

## Command-line options

| Option | What it does |
|---|---|
| `--settings` | Starts with the settings window open. |
| `--diagnose [file]` | Writes a read-only report for each monitor: ID, model and serial, GDI device, current input, supported inputs and the raw capabilities string. It also shows the configuration that would match. With no file it opens in Notepad. It never changes an input. |

## Input codes reference

Standard MCCS VCP `0x60` values. Monitors implement a subset, and some manufacturers use their own codes.

| Code | Input | | Code | Input |
|---|---|---|---|---|
| `01` | VGA 1 | | `0C` | Component 1 |
| `02` | VGA 2 | | `0D` | Component 2 |
| `03` | DVI 1 | | `0E` | Component 3 |
| `04` | DVI 2 | | `0F` | DisplayPort 1 |
| `05` | Composite 1 | | `10` | DisplayPort 2 |
| `06` | Composite 2 | | `11` | HDMI 1 |
| `07` | S-Video 1 | | `12` | HDMI 2 |
| `08` | S-Video 2 | | `19` | USB-C (seen on Dell U2725QE) |
| `09`–`0B` | Tuner 1–3 | | `1B` | USB-C (seen on Dell U2723QE) |

To use an input a monitor doesn't list, open the hotkey editor, type its code in **Other input code (hex)** and click **Add**. Monitor manuals and forums often give the exact codes for a model.

USB-C has no standard code. The two listed are what those Dell models report. New names go in `Monitors/InputSources.cs`.

## Developer tool: window screenshots

`tools/WindowCapture` saves a window to PNG using **Windows.Graphics.Capture**. Use it rather than GDI screen copies (`Graphics.CopyFromScreen` / `BitBlt`), which return blank white images for WPF dialogs. It captures windows even when they're covered, and never brings them to the front.

```
dotnet build -c Release tools\WindowCapture
tools\WindowCapture\bin\Release\net10.0-windows10.0.22621.0\WindowCapture.exe --pid <id> --list
tools\WindowCapture\bin\Release\net10.0-windows10.0.22621.0\WindowCapture.exe --pid <id> [--title <title>] --out shot.png
```

Windows are listed and searched front to back, so when a dialog shares its owner's title ("InputSwitch"), the dialog is the one captured. It targets `net10.0-windows10.0.22621.0`, which uses Microsoft's Windows SDK projection, to get the WinRT capture APIs.

When driving the UI with UI Automation for tests, list the open windows first. `InvokePattern` clicks buttons even behind an open modal dialog.

## Known limitations

- Monitors that don't implement DDC/CI, or docks, KVMs and adapters that don't pass it through, can't be controlled.
- Whether a monitor still accepts commands on an input it isn't currently showing depends on the model.
- In clone (duplicate) mode, mapping a physical monitor to its handle relies on the order Windows reports them.
- Many `Win +` combinations are reserved by Windows and can't be registered.
