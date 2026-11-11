# InputSwitch: technical notes

This is the developer and power-user companion to the [user guide](../README.md). It covers building, how the app works inside, the settings file, command-line options and developer tools.

---

## Contents

- [Building and publishing](#building-and-publishing)
- [Versioning and releases](#versioning-and-releases)
- [Smoke tests and CI](#smoke-tests-and-ci)
- [Project layout](#project-layout)
- [How it works](#how-it-works)
- [The settings file](#the-settings-file)
- [Command-line options](#command-line-options)
- [Input codes reference](#input-codes-reference)
- [Developer tool: window screenshots](#developer-tool-window-screenshots)
- [Known limitations](#known-limitations)

---

## Building and publishing

You need the **.NET 10 SDK**; `global.json` pins the feature band. The app has no third-party dependencies.

```
dotnet build -c Release
bin\Release\net10.0-windows\InputSwitch.exe
```

A single executable that doesn't need .NET installed, as the release workflow builds it:

```
dotnet publish InputSwitch.csproj -c Release -r win-x64 --self-contained -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true
```

`IncludeNativeLibrariesForSelfExtract` is needed because WPF has native libraries. The result is large (about 165 MB for x64, 180 MB for Arm64), because it carries the whole .NET desktop runtime, and WPF and WinForms can't be trimmed.

A **slim** single exe of about 0.3 MB, using the installed **.NET 10 Desktop Runtime**:

```
dotnet publish InputSwitch.csproj -c Release -r win-x64 --self-contained false -p:PublishSingleFile=true
```

The running app locks `InputSwitch.exe`, so exit it from the tray before rebuilding Release. A Debug build (`dotnet build -c Debug`) is a quick way to check the code compiles while it's running.

## Versioning and releases

The version lives in one place: `<VersionPrefix>` in `InputSwitch.csproj`. It follows [semantic versioning](https://semver.org/): bump the patch number for fixes, the minor for new features, and the major for breaking changes. The SDK derives the assembly and file versions from it, and appends the git commit to the informational version, e.g. `0.1.0+d3e334f…`. The app shows the version in the tray menu, at the bottom of the settings window's left-hand pane, and in the diagnostic report (`AppInfo.cs`).

Releases are **built once**. Every merge to `main` packages the release files, and tagging that commit publishes exactly those files, with nothing rebuilt.

**To cut a release:**

1. In a pull request, bump `<VersionPrefix>` (e.g. `0.1.0` → `0.2.0`) and merge it once CI passes.
2. Wait for CI on `main` to pass for the merge commit.
3. Tag the merge commit and push the tag:
   ```
   git checkout main && git pull
   git tag v0.2.0
   git push origin v0.2.0
   ```

Tags must be plain `vMAJOR.MINOR.PATCH`. The release is a full release marked **Latest**. To make one a pre-release, edit it on GitHub afterwards and tick **Set as a pre-release**. Suffixed tags such as `v0.2.0-beta.1` are refused, because the version is built into the files when they land on `main`, before any tag exists.

**On every push to `main`**, CI (see [Smoke tests and CI](#smoke-tests-and-ci)) publishes a **full** and a **slim** single-file build for **win-x64** and **win-arm64**, and launch-tests both x64 builds. It then zips all four with `SHA256SUMS.txt`, and keeps them on the run as the `release-files` artifact for **90 days**.

**On a version tag**, the **Release** workflow (`.github/workflows/release.yml`):

1. checks the tag is `vMAJOR.MINOR.PATCH` and matches `VersionPrefix`;
2. finds the successful CI run on `main` for the tagged commit, and stops if there isn't one;
3. downloads that run's `release-files`;
4. checks all four zips are present, the checksums match, and each exe's version stamp is exactly `<version>+<tagged commit>`;
5. creates the GitHub release with generated release notes, the zips and `SHA256SUMS.txt`.

This takes about a minute. If it stops, the error says why. The usual causes are:

- **Tagging a commit that isn't on `main`:** tag the merge commit instead.
- **Tagging before CI on `main` has finished:** delete the tag and tag again once CI has passed.
- **Release files older than 90 days:** push a new commit to `main` and tag that.

| Release file | For |
|---|---|
| `InputSwitch-<version>-win-x64-full.zip` | Most PCs. .NET runtime built in, nothing else to install (about 68 MB zipped). |
| `InputSwitch-<version>-win-x64-slim.zip` | PCs that already have the .NET 10 Desktop Runtime. Tiny download. |
| `InputSwitch-<version>-win-arm64-full.zip` | Arm laptops (e.g. Snapdragon). Nothing else to install. |
| `InputSwitch-<version>-win-arm64-slim.zip` | Arm laptops with the .NET 10 Desktop Runtime. |
| `SHA256SUMS.txt` | Checksums for the zips. |

To redo a release, delete the release and its tag (`gh release delete v0.2.0 --cleanup-tag --yes` and `git tag -d v0.2.0`), then tag again.

Keep the repository private or internal to the company's GitHub organisation, so releases are only visible to colleagues. Check with the AI Engineering team before making it more widely available.

## Smoke tests and CI

There are two layers of smoke tests. Neither needs monitors, and neither changes anything on the machine.

| Test | What it checks | Run locally |
|---|---|---|
| **Logic** (`tests/InputSwitch.SmokeTests`) | Capabilities parsing (with real Dell strings), EDID parsing, setup fingerprinting and matching, hotkey rules and display, action descriptions, settings round-trip. A plain console runner with no test-framework packages. | `dotnet run -c Release --project tests/InputSwitch.SmokeTests` |
| **Launch** (`tests/launch-smoke.ps1`) | Starts the real exe. `--diagnose` writes a report; `--settings` opens the window and it renders (found through UI Automation); no error dialog appears; the app stays up. Saves the report, plus a screenshot if `-Capture` points at WindowCapture (CI doesn't use this). | `pwsh tests/launch-smoke.ps1 -Exe bin\Release\net10.0-windows\InputSwitch.exe` |

The launch test sets `INPUTSWITCH_CONFIG_DIR` to a throwaway folder. The app then uses that folder for its settings, and a separate single-instance lock, so the test never touches your real settings and runs fine while your own copy is in the tray. It never switches an input.

The **CI** workflow (`.github/workflows/ci.yml`) runs on every pull request to `main`, every push to `main`, and on demand. It's a single job, **Build and smoke test**, on `windows-latest`. Each binary is built once, and the launch tests run against the files that ship:

1. runs the logic smoke tests;
2. publishes the **x64 full** and **x64 slim** single-file builds and launch-tests both;
3. on pushes to `main` only: also publishes the Arm64 builds, zips all four with checksums, and keeps them as the `release-files` artifact (see [Versioning and releases](#versioning-and-releases)).

Pull requests skip step 3, which takes a while. Apart from the release files, nothing is uploaded; everything is in the run log. If a launch check fails, the test prints the diagnostic report into the log.

**Require it before merging.** In the repository's **Settings → Rules → Rulesets** (or **Branches → Branch protection rules**), add a rule for `main` that requires a pull request, and requires the status check **Build and smoke test** to pass. Optionally, also block direct pushes and force-pushes to `main`. CI can't enforce this itself; the rule has to be set in GitHub.

## Project layout

| Path | Contents |
|---|---|
| `Program.cs` | Entry point: single-instance check, `--diagnose`, `--settings`. |
| `AppInfo.cs` | Version and commit, read from the assembly attributes. |
| `TrayApp.cs` | The tray icon and menu, display-change handling, profile activation and hotkey registration. |
| `Startup.cs` | "Start with Windows" (per-user Run key). |
| `TrayIconFactory.cs` | Draws the tray and window icon at runtime (no `.ico` file). |
| `Config/` | `AppConfig` (data model), `ConfigStore` (JSON load/save), `ProfileMatcher` (which configuration fits the connected monitors). |
| `Hotkeys/` | `Hotkey` (key + modifiers) and `SystemWindow` (hidden window that owns the global hotkeys and receives display/device broadcasts). |
| `Monitors/` | `MonitorService` (enumeration and DDC/CI), `Edid`, `InputSources` (VCP 0x60 names and capabilities parsing), `InputSwitcher` (switch/toggle/cycle logic). |
| `Native/` | P/Invoke declarations: user32, dxva2, and the CCD display-configuration API. |
| `UI/` | WPF windows: settings (`ConfigWindow`), hotkey editor (`BindingEditorWindow`), themed dialog (`FluentDialog`), shared styles and view models, and the WinForms/WPF bridge (`WpfHost`, `SettingsWindow`). |
| `tests/` | Smoke tests: logic (`InputSwitch.SmokeTests`) and launch (`launch-smoke.ps1`). |
| `tools/WindowCapture/` | Developer screenshot tool (separate project, excluded from the app build). |
| `.github/workflows/` | CI (`ci.yml`) and release (`release.yml`). |
| `global.json` | Pins the .NET SDK feature band for local and CI builds. |

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

Stored at `%APPDATA%\InputSwitch\config.json`, or in the folder named by the `INPUTSWITCH_CONFIG_DIR` environment variable if it's set. An instance with its own folder also gets its own single-instance lock. It's written atomically (to a temporary file, then moved into place). If it can't be read, it's kept as `config.json.bad` and the app starts with fresh settings.

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
