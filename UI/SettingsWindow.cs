using InputSwitch.Config;
using InputSwitch.Monitors;

namespace InputSwitch.UI;

/// <summary>
/// What the tray app needs from the main window. It contains no WPF types, so the tray code can
/// hold one without the runtime loading WPF until a window is actually created.
/// </summary>
internal interface ISettingsWindow
{
    /// <summary>Raised with a copy of the configuration after every change; the tray saves and applies it.</summary>
    event Action<AppConfig>? Changed;
    /// <summary>True while the hotkey editor is open, so hotkeys should pause.</summary>
    event Action<bool>? CapturingHotkey;
    event EventHandler Closed;
    void UpdateConnection(List<MonitorInfo> connected, string? activeId);
    void SelectProfile(string? id);
    void BringToFront();
    void Show();
    bool Activate();
    void Close();
}

internal static class SettingsWindow
{
    public static ISettingsWindow Create(AppConfig config, List<MonitorInfo> connected, string? activeId, string? selectId, InputSwitcher switcher)
    {
        WpfHost.EnsureInitialized();
        var window = new ConfigWindow(config, connected, activeId, selectId, switcher);
        // A modeless WPF window on the WinForms message loop needs this for keyboard input to work.
        System.Windows.Forms.Integration.ElementHost.EnableModelessKeyboardInterop(window);
        return window;
    }
}
