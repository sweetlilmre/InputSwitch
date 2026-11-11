# InputSwitch

A small Windows tray app that switches monitor inputs (DisplayPort, HDMI, USB-C, DVI, …) with global hotkeys. It sends the VESA MCCS "input source" command (VCP 0x60) over DDC/CI, the mechanism monitor utilities such as Dell Display Manager use, so it works with most monitors from Dell, LG, Samsung, HP, Lenovo, BenQ, ASUS and others. It has no dependencies beyond .NET 10.

Not every monitor plays along: some models (notably some LGs) use non-standard input codes or ignore the command, there is no standard code for USB-C (Dell uses 0x1B), and most TVs and some docks/KVMs don't pass DDC/CI through.

## Build and run

```
dotnet build -c Release
bin\Release\net10.0-windows\InputSwitch.exe
```

For a single self-contained executable: `dotnet publish -c Release -r win-x64 --self-contained -p:PublishSingleFile=true`.

## How it works

- **Setups are recognised by their monitors.** Each monitor is identified by its EDID manufacturer, model and serial number, so it is recognised whichever dock or port it is plugged into. The laptop's built-in panel is ignored, so opening or closing the lid does not count as a different setup.
- **First run / new place.** When the connected external monitors don't match any configuration, a new one is created automatically and starts **disabled**. On the very first run the configuration window opens; after that you get a notification you can click.
- **Hardware changes** (docking, undocking, plugging in a monitor, resume from sleep, unlock) are detected, and after a short settling delay the matching configuration is activated and its hotkeys are registered.
- **A monitor that disappears** after being switched to another input (or turned off) doesn't change the setup: a configuration still matches when some of its monitors are missing.
- **Hotkeys.** Each hotkey belongs to one monitor and lists one or more inputs. With one input it switches to that input. With two it toggles between them. With more it cycles through them in order. The current input is read from the monitor to decide which input comes next.

## Configuration window

Open it by left-clicking the tray icon, or by right-clicking the icon and choosing **Configure…**.

- **Configurations** (left): one per setup, e.g. rename "Setup 1" to "Work". **New from current setup** creates another one by hand.
- **Enabled**: hotkeys are only registered while the configuration is enabled and its setup is connected. Adding the first hotkey enables it.
- **Monitors**: rename monitors (e.g. "Left Dell"), **Detect inputs** reads the inputs each monitor supports, **Add connected monitors** / **Remove** changes which monitors make up the setup.
- **Hotkeys**: **Add…** picks a monitor, a key combination (it needs Ctrl, Alt or Win, or one of F13–F24) and the inputs. **Test** runs the action straight away.

Hotkeys are released while the window is open, so you can record combinations that are already assigned.

The right-click menu also lists the active hotkeys and has a **Switch … to** submenu for each monitor for one-off switches, plus **Start with Windows**.

## Files

- Configuration: `%APPDATA%\InputSwitch\config.json`
- Diagnostics: `InputSwitch.exe --diagnose` writes a read-only report (monitors, ids, current input, DDC/CI capabilities, matching configuration) and opens it in Notepad. It never changes an input.

## Troubleshooting

- **"did not accept the switch" / "no DDC/CI reply"**: enable DDC/CI in the monitor's on-screen menu (on Dell monitors it's under Others → DDC/CI; other brands have a similar setting). Some docks and KVMs don't pass DDC/CI through.
- **An input isn't listed**: some monitors report an incomplete capabilities list. In the hotkey editor, enter the code under "Other input code" (MCCS VCP 0x60 values: 0x03 DVI 1, 0x0F DisplayPort 1, 0x11 HDMI 1, 0x12 HDMI 2; Dell uses 0x1B for USB-C).
- **Switching back from the other input does nothing**: once a monitor shows another input, whether it still answers DDC/CI on the inactive cable depends on the model. Most Dells do; try it with yours.
