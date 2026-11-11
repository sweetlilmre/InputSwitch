using InputSwitch.Native;

namespace InputSwitch.Hotkeys;

/// <summary>
/// Hidden top-level window that owns the global hotkeys and receives display and device
/// change broadcasts. (A message-only window would not receive those broadcasts.)
/// </summary>
internal sealed class SystemWindow : NativeWindow, IDisposable
{
    private readonly Dictionary<int, string> _hotkeys = new();
    private int _nextId = 1;

    /// <summary>Raised with the binding id whose hotkey was pressed.</summary>
    public event Action<string>? HotkeyPressed;

    /// <summary>Raised when monitors may have been added, removed or reconfigured.</summary>
    public event Action? DisplaysMayHaveChanged;

    public SystemWindow()
    {
        CreateHandle(new CreateParams { Caption = "InputSwitch.SystemWindow" });
    }

    /// <summary>Registers a hotkey. Returns false if it is invalid or another app already owns it.</summary>
    public bool Register(Hotkey hotkey, string bindingId)
    {
        if (!hotkey.IsValid) return false;
        int id = _nextId++;
        if (!NativeMethods.RegisterHotKey(Handle, id, hotkey.NativeModifiers | NativeMethods.MOD_NOREPEAT, (uint)hotkey.Key))
            return false;
        _hotkeys[id] = bindingId;
        return true;
    }

    public void UnregisterAll()
    {
        foreach (var id in _hotkeys.Keys)
            NativeMethods.UnregisterHotKey(Handle, id);
        _hotkeys.Clear();
    }

    protected override void WndProc(ref Message m)
    {
        switch (m.Msg)
        {
            case NativeMethods.WM_HOTKEY:
                if (_hotkeys.TryGetValue((int)m.WParam, out var bindingId))
                    HotkeyPressed?.Invoke(bindingId);
                break;
            case NativeMethods.WM_DISPLAYCHANGE:
            case NativeMethods.WM_DEVICECHANGE:
                DisplaysMayHaveChanged?.Invoke();
                break;
        }
        base.WndProc(ref m);
    }

    public void Dispose()
    {
        UnregisterAll();
        DestroyHandle();
    }
}
