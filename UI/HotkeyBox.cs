using System.Runtime.InteropServices;
using InputSwitch.Hotkeys;

namespace InputSwitch.UI;

/// <summary>A text box that records a key combination instead of text.</summary>
internal sealed class HotkeyBox : TextBox
{
    private Hotkey _hotkey = new();

    [DllImport("user32.dll")]
    private static extern short GetKeyState(int vk);
    private const int VK_LWIN = 0x5B, VK_RWIN = 0x5C;

    public event EventHandler? HotkeyChanged;

    public HotkeyBox()
    {
        ShortcutsEnabled = false;
        Cursor = Cursors.Default;
        UpdateText();
    }

    [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
    public Hotkey Hotkey
    {
        get => _hotkey;
        set { _hotkey = value ?? new Hotkey(); UpdateText(); HotkeyChanged?.Invoke(this, EventArgs.Empty); }
    }

    protected override bool ProcessCmdKey(ref Message msg, Keys keyData)
    {
        var key = keyData & Keys.KeyCode;
        var mods = keyData & Keys.Modifiers;

        // Plain Tab / Shift+Tab still move focus.
        if (key == Keys.Tab && (mods & ~Keys.Shift) == 0) return base.ProcessCmdKey(ref msg, keyData);

        // Backspace / Delete / Escape on their own clear the box.
        if (mods == Keys.None && key is Keys.Back or Keys.Delete or Keys.Escape)
        {
            Hotkey = new Hotkey();
            return true;
        }

        var m = HotkeyModifiers.None;
        if ((mods & Keys.Control) != 0) m |= HotkeyModifiers.Control;
        if ((mods & Keys.Alt) != 0) m |= HotkeyModifiers.Alt;
        if ((mods & Keys.Shift) != 0) m |= HotkeyModifiers.Shift;
        if (GetKeyState(VK_LWIN) < 0 || GetKeyState(VK_RWIN) < 0) m |= HotkeyModifiers.Win;

        if (key is Keys.ControlKey or Keys.ShiftKey or Keys.Menu or Keys.LWin or Keys.RWin or Keys.None)
        {
            // Only modifiers held so far: show them as a hint.
            Text = m != HotkeyModifiers.None ? PartialText(m) : _hotkey.ToString();
            return true;
        }

        Hotkey = new Hotkey { Modifiers = m, Key = key };
        return true;
    }

    protected override void OnKeyPress(KeyPressEventArgs e) => e.Handled = true;

    protected override void OnKeyUp(KeyEventArgs e)
    {
        base.OnKeyUp(e);
        UpdateText();
    }

    protected override void OnLostFocus(EventArgs e)
    {
        base.OnLostFocus(e);
        UpdateText();
    }

    private void UpdateText() => Text = _hotkey.IsEmpty ? "Press a key combination…" : _hotkey.ToString();

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
