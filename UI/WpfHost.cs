namespace InputSwitch.UI;

/// <summary>
/// Sets WPF up the first time a window is needed, so an idle tray app never loads it.
/// The WinForms loop keeps driving the app; the WPF Application object only supplies the
/// Fluent theme (Windows 11 styling, light/dark following Windows) to the windows.
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
        _app.ThemeMode = System.Windows.ThemeMode.System;
    }

    public static void Shutdown() => _app?.Shutdown();
}
