using InputSwitch.Config;

namespace InputSwitch.UI;

/// <summary>
/// Sets WPF up the first time a window is needed, so an idle tray app never loads it.
/// The WinForms loop keeps driving the app; the WPF Application object only supplies the
/// Fluent theme (Windows 11 styling, light or dark per <see cref="UiTheme"/>) to the windows.
/// </summary>
internal static class WpfHost
{
    private static System.Windows.Application? _app;

    public static void EnsureInitialized()
    {
        if (_app is not null) return;
        _app = System.Windows.Application.Current ?? new System.Windows.Application
        {
            ShutdownMode = System.Windows.ShutdownMode.OnExplicitShutdown,
        };
        Apply();
        UiTheme.Changed += Apply;
    }

    private static void Apply()
    {
        if (_app is null) return;
        _app.ThemeMode = UiTheme.Current switch
        {
            AppTheme.Light => System.Windows.ThemeMode.Light,
            AppTheme.Dark => System.Windows.ThemeMode.Dark,
            _ => System.Windows.ThemeMode.System,
        };
    }

    public static void Shutdown() => _app?.Shutdown();
}
