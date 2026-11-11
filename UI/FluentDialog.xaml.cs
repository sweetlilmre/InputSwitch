using System.Media;
using System.Windows;
using System.Windows.Controls;

namespace InputSwitch.UI;

internal enum DialogIcon { Info, Warning, Question, Error }

/// <summary>Which button closed a <see cref="FluentDialog"/>.</summary>
internal enum DialogChoice { Primary, Secondary, Cancel }

/// <summary>
/// A themed replacement for MessageBox, in the style of a Windows 11 content dialog: a heading,
/// a sentence of detail, and buttons labelled with what they do.
/// </summary>
internal sealed partial class FluentDialog : Window
{
    private DialogChoice _choice = DialogChoice.Cancel;

    private FluentDialog() => InitializeComponent();

    /// <param name="primary">The main action (accent coloured, triggered by Enter).</param>
    /// <param name="secondary">An optional alternative, e.g. "Don't save".</param>
    /// <param name="cancel">The dismiss button (triggered by Esc or the close box); omit for a single-button dialog.</param>
    public static DialogChoice Show(Window? owner, DialogIcon icon, string heading, string message,
        string primary = "OK", string? secondary = null, string? cancel = null)
    {
        var dialog = new FluentDialog();
        dialog.Icon = ConfigWindow.CreateWindowIcon();
        dialog.HeadingText.Text = heading;
        dialog.MessageText.Text = message;
        dialog.MessageText.Visibility = string.IsNullOrEmpty(message) ? Visibility.Collapsed : Visibility.Visible;
        dialog.SetIcon(icon);

        dialog.AddButton(primary, DialogChoice.Primary, isDefault: true, isCancel: cancel is null && secondary is null);
        if (secondary is not null) dialog.AddButton(secondary, DialogChoice.Secondary, isDefault: false, isCancel: false);
        if (cancel is not null) dialog.AddButton(cancel, DialogChoice.Cancel, isDefault: false, isCancel: true);

        if (owner is { IsLoaded: true })
        {
            dialog.Owner = owner;
        }
        else
        {
            // No window to sit on (e.g. at startup): show it on its own, above everything.
            dialog.WindowStartupLocation = WindowStartupLocation.CenterScreen;
            dialog.ShowInTaskbar = true;
            dialog.Topmost = true;
        }

        Sound(icon)?.Play();
        dialog.ShowDialog();
        return dialog._choice;
    }

    /// <summary>A two-button confirmation. Returns true if the user chose <paramref name="confirm"/>.</summary>
    public static bool Confirm(Window? owner, DialogIcon icon, string heading, string message, string confirm, string cancel = "Cancel") =>
        Show(owner, icon, heading, message, confirm, cancel: cancel) == DialogChoice.Primary;

    private void AddButton(string text, DialogChoice choice, bool isDefault, bool isCancel)
    {
        var button = new Button
        {
            Content = text,
            IsDefault = isDefault,
            IsCancel = isCancel,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Margin = new Thickness(ButtonRow.Children.Count == 0 ? 0 : 8, 0, 0, 0),
            MinWidth = 96,
        };
        if (isDefault) button.SetResourceReference(StyleProperty, "AccentButtonStyle");
        button.Click += (_, _) =>
        {
            _choice = choice;
            DialogResult = choice != DialogChoice.Cancel;
        };
        ButtonRow.Children.Add(button);
        ButtonRow.Columns = ButtonRow.Children.Count;
    }

    private void SetIcon(DialogIcon icon)
    {
        var (glyph, foreground, background) = icon switch
        {
            DialogIcon.Warning => ("", "SystemFillColorCautionBrush", "SystemFillColorCautionBackgroundBrush"),
            DialogIcon.Error => ("", "SystemFillColorCriticalBrush", "SystemFillColorCriticalBackgroundBrush"),
            DialogIcon.Question => ("", "AccentTextFillColorPrimaryBrush", "SystemFillColorAttentionBackgroundBrush"),
            _ => ("", "AccentTextFillColorPrimaryBrush", "SystemFillColorAttentionBackgroundBrush"),
        };
        IconGlyph.Text = glyph;
        IconGlyph.SetResourceReference(TextBlock.ForegroundProperty, foreground);
        IconBadge.SetResourceReference(System.Windows.Controls.Border.BackgroundProperty, background);
    }

    private static SystemSound? Sound(DialogIcon icon) => icon switch
    {
        DialogIcon.Warning => SystemSounds.Exclamation,
        DialogIcon.Error => SystemSounds.Hand,
        DialogIcon.Info => SystemSounds.Asterisk,
        _ => null,
    };
}

/// <summary>
/// Dialogs for code outside the WPF windows (startup, config loading). Its signatures contain no
/// WPF types, so calling code doesn't pull WPF in unless a dialog is actually shown.
/// </summary>
internal static class AppDialogs
{
    public static void Alert(DialogIcon icon, string heading, string message)
    {
        WpfHost.EnsureInitialized();
        FluentDialog.Show(null, icon, heading, message);
    }
}
