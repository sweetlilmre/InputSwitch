using System.Windows.Controls;
using System.Windows.Input;
using InputSwitch.Hotkeys;
using Keys = System.Windows.Forms.Keys;

namespace InputSwitch.UI;

/// <summary>A text box that records a key combination instead of text.</summary>
internal sealed class HotkeyBox : TextBox
{
    private Hotkey _hotkey = new();

    public event EventHandler? HotkeyChanged;

    public HotkeyBox()
    {
        // Implicit (theme) styles only apply to the exact type, so borrow TextBox's Fluent style.
        SetResourceReference(StyleProperty, typeof(TextBox));
        IsReadOnly = true;
        IsReadOnlyCaretVisible = false;
        IsUndoEnabled = false;
        ContextMenu = null;
        UpdateText();
    }

    public Hotkey Hotkey
    {
        get => _hotkey;
        set
        {
            _hotkey = value ?? new Hotkey();
            UpdateText();
            HotkeyChanged?.Invoke(this, EventArgs.Empty);
        }
    }

    protected override void OnPreviewKeyDown(KeyEventArgs e)
    {
        var key = e.Key switch
        {
            Key.System => e.SystemKey,         // keys pressed with Alt
            Key.ImeProcessed => e.ImeProcessedKey,
            _ => e.Key,
        };
        var mods = Keyboard.Modifiers;

        // Plain Tab / Shift+Tab still move focus.
        if (key == Key.Tab && (mods & ~ModifierKeys.Shift) == 0) return;

        // Plain Enter / Escape still reach the dialog's Save / Cancel buttons.
        if (mods == ModifierKeys.None && key is Key.Enter or Key.Escape) return;

        e.Handled = true;

        // Backspace / Delete on their own clear the box.
        if (mods == ModifierKeys.None && key is Key.Back or Key.Delete)
        {
            Hotkey = new Hotkey();
            return;
        }

        var m = HotkeyModifiers.None;
        if (mods.HasFlag(ModifierKeys.Control)) m |= HotkeyModifiers.Control;
        if (mods.HasFlag(ModifierKeys.Alt)) m |= HotkeyModifiers.Alt;
        if (mods.HasFlag(ModifierKeys.Shift)) m |= HotkeyModifiers.Shift;
        if (mods.HasFlag(ModifierKeys.Windows)) m |= HotkeyModifiers.Win;

        if (key is Key.LeftCtrl or Key.RightCtrl or Key.LeftAlt or Key.RightAlt or
            Key.LeftShift or Key.RightShift or Key.LWin or Key.RWin or Key.None)
        {
            // Only modifiers held so far: show them as a hint.
            Text = m == HotkeyModifiers.None ? Describe() : PartialText(m);
            return;
        }

        Hotkey = new Hotkey { Modifiers = m, Key = (Keys)KeyInterop.VirtualKeyFromKey(key) };
    }

    protected override void OnPreviewKeyUp(KeyEventArgs e)
    {
        base.OnPreviewKeyUp(e);
        if (Keyboard.Modifiers == ModifierKeys.None) UpdateText();
    }

    protected override void OnLostKeyboardFocus(KeyboardFocusChangedEventArgs e)
    {
        base.OnLostKeyboardFocus(e);
        UpdateText();
    }

    private string Describe() => _hotkey.IsEmpty ? "Press a key combination…" : _hotkey.ToString();

    private void UpdateText() => Text = Describe();

    private static string PartialText(HotkeyModifiers m)
    {
        var parts = new List<string>();
        if (m.HasFlag(HotkeyModifiers.Control)) parts.Add("Ctrl");
        if (m.HasFlag(HotkeyModifiers.Alt)) parts.Add("Alt");
        if (m.HasFlag(HotkeyModifiers.Shift)) parts.Add("Shift");
        if (m.HasFlag(HotkeyModifiers.Win)) parts.Add("Win");
        return string.Join(" + ", parts) + " + …";
    }
}
