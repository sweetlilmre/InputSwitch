namespace InputSwitch.Monitors;

/// <summary>A monitor that is currently connected and active.</summary>
/// <param name="Id">Stable identity: manufacturer + product code + serial (falls back to the device instance).</param>
/// <param name="GdiDeviceName">GDI device (\\.\DISPLAYn) this monitor is attached to; used to reach it over DDC/CI.</param>
/// <param name="IndexOnDevice">Position among monitors sharing the same GDI device (clone mode).</param>
internal sealed record MonitorInfo(
    string Id,
    string FriendlyName,
    string Model,
    string? Serial,
    bool IsInternal,
    string GdiDeviceName,
    int IndexOnDevice)
{
    public string DisplayName => string.IsNullOrWhiteSpace(FriendlyName) ? Model : FriendlyName;
}
