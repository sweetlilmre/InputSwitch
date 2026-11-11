using System.Drawing;
using System.Windows.Forms;
using System.Text.Json.Serialization;
using InputSwitch.Native;

namespace InputSwitch.Hotkeys;

[Flags]
internal enum HotkeyModifiers
{
    None = 0,
    Alt = 1,
    Control = 2,
    Shift = 4,
    Win = 8,
}

internal sealed record Hotkey
{
    public HotkeyModifiers Modifiers { get; init; }
    public Keys Key { get; init; } = Keys.None;

    [JsonIgnore]
    public bool IsEmpty => Key == Keys.None;

    /// <summary>
    /// A global hotkey needs a modifier, except F13–F24 which nothing else uses.
    /// Shift alone is not enough: it would swallow ordinary typing.
    /// </summary>
    [JsonIgnore]
    public bool IsValid =>
        !IsEmpty &&
        ((Modifiers & ~HotkeyModifiers.Shift) != 0 || Key is >= Keys.F13 and <= Keys.F24);

    [JsonIgnore]
    public uint NativeModifiers =>
        ((Modifiers & HotkeyModifiers.Alt) != 0 ? NativeMethods.MOD_ALT : 0) |
        ((Modifiers & HotkeyModifiers.Control) != 0 ? NativeMethods.MOD_CONTROL : 0) |
        ((Modifiers & HotkeyModifiers.Shift) != 0 ? NativeMethods.MOD_SHIFT : 0) |
        ((Modifiers & HotkeyModifiers.Win) != 0 ? NativeMethods.MOD_WIN : 0);

    public override string ToString()
    {
        if (IsEmpty) return "(none)";
        var parts = new List<string>();
        if (Modifiers.HasFlag(HotkeyModifiers.Control)) parts.Add("Ctrl");
        if (Modifiers.HasFlag(HotkeyModifiers.Alt)) parts.Add("Alt");
        if (Modifiers.HasFlag(HotkeyModifiers.Shift)) parts.Add("Shift");
        if (Modifiers.HasFlag(HotkeyModifiers.Win)) parts.Add("Win");
        parts.Add(KeyName(Key));
        return string.Join(" + ", parts);
    }

    private static string KeyName(Keys key) => key switch
    {
        >= Keys.D0 and <= Keys.D9 => ((char)('0' + (key - Keys.D0))).ToString(),
        >= Keys.NumPad0 and <= Keys.NumPad9 => $"Num {(int)(key - Keys.NumPad0)}",
        Keys.Oemcomma => ",",
        Keys.OemPeriod => ".",
        Keys.OemMinus => "-",
        Keys.Oemplus => "=",
        Keys.OemQuestion => "/",
        Keys.OemSemicolon => ";",
        Keys.OemQuotes => "'",
        Keys.OemOpenBrackets => "[",
        Keys.OemCloseBrackets => "]",
        Keys.OemPipe => "\\",
        Keys.Oemtilde => "`",
        Keys.Next => "PageDown",
        Keys.Prior => "PageUp",
        _ => key.ToString(),
    };
}
