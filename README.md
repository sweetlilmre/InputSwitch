# InputSwitch

**Switch your monitors between inputs with a keyboard shortcut, and have the right shortcuts ready wherever you plug your laptop in.**

Press a hotkey and your monitor jumps from DisplayPort to HDMI, or to USB-C, or to whichever input you like. No more fiddling with the buttons under the screen.

InputSwitch also remembers each place you work. Your desk at work and your desk at home each get their own set of hotkeys, and InputSwitch picks the right set by itself when you plug in.

> Developers and the curious: building, internals and the settings file are covered in the [technical notes](docs/TECHNICAL.md).

---

## Contents

- [Before you start](#before-you-start)
- [Getting started](#getting-started)
- [Work, home and anywhere else](#work-home-and-anywhere-else)
- [The main window](#the-main-window)
- [Making a hotkey](#making-a-hotkey)
- [The tray icon](#the-tray-icon)
- [Notifications](#notifications)
- [Something not working?](#something-not-working)
- [Updating](#updating)
- [Your settings and uninstalling](#your-settings-and-uninstalling)

---

## Before you start

You'll need:

- **Windows 10 or 11.** It looks its best on Windows 11.
- **Monitors that accept commands from your computer.** Most Dell, LG, Samsung, HP, Lenovo, BenQ and ASUS monitors do. The feature is called **DDC/CI** and is usually on already. If not, look for it in the monitor's on-screen menu (on Dell monitors it's under **Others**).

It usually won't work with TVs, or through some docks, KVM switches and adapters. Plugging the monitor in directly often fixes that.

## Getting started

1. **Download and run it.** From the project's **Releases** page, download the latest zip:
   - **`win-x64-full`** for most PCs. Everything is included, so there's nothing else to install.
   - **`win-x64-slim`** is a much smaller download, if your PC already has the *.NET 10 Desktop Runtime* (if you're not sure, pick *full*). If the runtime is missing, the slim version says so when you start it and offers to download it. Installing it needs administrator rights, so on a company PC you may need IT's help.
   - **`win-arm64-full`** or **`win-arm64-slim`** for Arm laptops, such as Snapdragon models.

   Unzip it to a folder of your choice and run `InputSwitch.exe`. It finds your monitors, makes a configuration for them called *Setup 1*, and opens the main window.
2. **Give it a name.** Click *Setup 1* at the top and type something like `Work`.
3. **Add a hotkey.** Click **Add hotkey**, choose a monitor, then click in the *Key combination* box and press the keys you want, for example **Ctrl + Alt + M**. Then tick the inputs, for example **DisplayPort 1** and **HDMI 1**, and click **Save**. The hotkey works straight away.
4. **Try it.** Press your hotkey, or click the ▷ button next to it.

There's no Save button in the main window: every change is saved as you make it. Close the window when you're done.

> **Tip:** before you try a hotkey, make sure something is plugged into the input you're switching to. Otherwise the screen shows "No signal" until you switch back.

To have InputSwitch start every time you sign in, tick **Start with Windows**.

## Work, home and anywhere else

InputSwitch recognises your monitors individually, even two of the same model, so it always knows which desk you're at.

- **The first time you plug in somewhere new,** InputSwitch notices and makes a new configuration. Click the notification, give it a name like `Home`, and add your hotkeys. You can use the same key combinations as at work.
- **From then on,** just plug in. The right hotkeys switch on by themselves.
- **Closing your laptop lid** doesn't confuse it. Neither does switching a monitor off, or switching it over to another computer.
- **Away from a desk,** with no monitors plugged in, nothing is active.

## The main window

**Click the tray icon** to open it.

- **On the left** are your configurations, such as *Work* and *Home*. The one you're plugged into now is highlighted.
- **At the top** is the configuration's name (click it to rename it), and a coloured banner:
  - **Green** means you're plugged into this setup and its hotkeys are working.
  - **Amber** means you're plugged in but **Hotkeys on** is unticked.
  - **Grey** means this setup isn't plugged in right now.
- **Monitors** shows each monitor with the inputs it has. The highlighted one is the input it's showing now. You can rename a monitor, for example to `Left screen`.
  - **Detect inputs** checks again which inputs your monitors have.
  - **Add connected** adds a newly plugged-in monitor to this configuration.
- **Hotkeys** lists your hotkeys. Use ▷ to try one, ✎ to change it, or 🗑 to remove it (you're asked first).
- **At the bottom** are **Start with Windows**, **Notify when the setup changes**, and **Close**.

Changes are saved as you make them, and take effect straight away. Your hotkeys keep working while the window is open; they only pause while the hotkey editor is open, so a combination can't fire while you're recording it.

## Making a hotkey

Each hotkey controls one monitor. What it does depends on how many inputs you tick:

| Tick | Pressing the hotkey… |
|---|---|
| One input | switches to it |
| Two inputs | flips between them |
| Three or more | steps through them in order |

When choosing keys:

- **Click in the *Key combination* box** to start recording. It says *Press the keys now…* while it's listening; then press the combination.
- Use **Ctrl**, **Alt** or the **Windows key** with a letter or number, for example **Ctrl + Alt + M**.
- **Backspace** clears the box if you change your mind.
- Each combination can only be used once per configuration. If another hotkey already uses it, you're told when you click **Save**.
- Some **Windows key** combinations are taken by Windows itself. If yours doesn't work, pick another.

Use the ▲ ▼ buttons to change the order when stepping through three or more inputs.

## The tray icon

InputSwitch sits in the notification area as a small blue monitor with two arrows. If you can't see it, click the **^** arrow next to the clock.

- **Click it** to open the main window.
- **Right-click it** for a menu where you can:
  - run any of your hotkeys,
  - switch a monitor to any input as a one-off,
  - turn **Start with Windows** on or off,
  - see which version you have,
  - **Exit**.

## Notifications

InputSwitch pops up a short message when:

- you plug into a **new setup**. Click it to set the setup up.
- you plug into a **known setup**, for example "Display setup: Home". You can turn these off with **Notify when the setup changes**.
- something needs your attention, for example a hotkey another app is already using, or a monitor that didn't respond.

## Something not working?

**The monitor didn't switch**

- Check **DDC/CI** is on in the monitor's on-screen menu.
- If you use a dock or adapter, try plugging the monitor in directly.
- Open the main window and click **Detect inputs**. If no inputs appear, the monitor isn't listening to the computer.

**My hotkey does nothing**

- Right-click the tray icon. Does it show the right setup? If it says *(disabled)*, open the main window and tick **Hotkeys on**.
- Another app might already use that key combination. Try a different one.
- Hotkeys pause while the hotkey editor is open.

**It can't switch back**

Some monitors stop listening to your computer while they're showing a different input. Use the monitor's own buttons to switch back.

**An input I need isn't listed**

Some monitors don't list every input. The [technical notes](docs/TECHNICAL.md#input-codes-reference) explain how to add one by its code.

**It made a new configuration I didn't expect**

That happens when you plug in a monitor it hasn't seen before. Either set up the new configuration, or delete it and use **Add connected** in your usual one.

Still stuck? The [technical notes](docs/TECHNICAL.md#command-line-options) describe a diagnostic report you can send to whoever supports you.

## Updating

To move to a new version, right-click the tray icon and choose **Exit**. Then download the new zip from the **Releases** page and unzip it over your InputSwitch folder, replacing the old files, and start it again. Your configurations and hotkeys are kept: they're stored separately from the program. The tray menu shows which version you're running.

## Your settings and uninstalling

Your settings are saved automatically. Right-click the tray icon and choose **Open config folder** to see where. Copy that folder to keep a backup.

InputSwitch doesn't connect to the internet or send anything anywhere.

**To uninstall:**

1. Untick **Start with Windows**.
2. Right-click the tray icon and choose **Exit**.
3. Delete the InputSwitch folder, and the settings folder too if you don't want to keep your settings.
