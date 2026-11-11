using System.Text;
using InputSwitch.Native;
using static InputSwitch.Native.NativeMethods;

namespace InputSwitch.Monitors;

/// <summary>Finds connected monitors and talks to them over DDC/CI.</summary>
internal static class MonitorService
{
    // The I2C bus is slow and some monitors misbehave under concurrent requests; serialise all DDC traffic.
    private static readonly object DdcLock = new();

    public static List<MonitorInfo> Enumerate()
    {
        var result = new List<MonitorInfo>();
        var perDevice = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);

        foreach (var t in DisplayConfig.GetActiveTargets())
        {
            var edid = EdidInfo.Parse(EdidInfo.ReadFromRegistry(t.DevicePath));

            string mfg = edid?.ManufacturerId ?? DecodePnpId(t.EdidManufacturerId);
            ushort product = edid?.ProductCode ?? t.EdidProductCode;
            string model = $"{mfg}{product:X4}";
            string? serial = edid?.Serial;

            // A serial number makes the id follow the monitor across docks and ports.
            // Without one, fall back to the device instance (stable per port on this machine).
            string id = serial is not null
                ? $"{model}#{serial}"
                : $"{model}@{EdidInfo.DevicePathToInstanceId(t.DevicePath) ?? t.GdiDeviceName}";

            string friendly = !string.IsNullOrWhiteSpace(t.FriendlyName) ? t.FriendlyName
                : edid?.ModelName ?? (t.IsInternal ? "Built-in display" : model);

            perDevice.TryGetValue(t.GdiDeviceName, out int index);
            perDevice[t.GdiDeviceName] = index + 1;

            result.Add(new MonitorInfo(id, friendly, model, serial, t.IsInternal, t.GdiDeviceName, index));
        }

        return result;
    }

    /// <summary>Reads the current input (VCP 0x60). Returns null if the monitor did not answer.</summary>
    public static int? GetCurrentInput(MonitorInfo monitor) =>
        WithPhysicalMonitor(monitor, h =>
        {
            for (int attempt = 0; attempt < 3; attempt++)
            {
                if (GetVCPFeatureAndVCPFeatureReply(h, InputSources.VcpInputSource, out _, out uint current, out _))
                    return (int?)(current & 0xFF);
                Thread.Sleep(60);
            }
            return null;
        });

    public static bool SetInput(MonitorInfo monitor, int code) =>
        WithPhysicalMonitor(monitor, h =>
        {
            for (int attempt = 0; attempt < 3; attempt++)
            {
                if (SetVCPFeature(h, InputSources.VcpInputSource, (uint)code))
                    return true;
                Thread.Sleep(60);
            }
            return false;
        });

    /// <summary>Requests the MCCS capabilities string. This can take a second or two.</summary>
    public static string? GetCapabilities(MonitorInfo monitor) =>
        WithPhysicalMonitor(monitor, h =>
        {
            for (int attempt = 0; attempt < 2; attempt++)
            {
                if (GetCapabilitiesStringLength(h, out uint len) && len > 0)
                {
                    var buffer = new byte[len];
                    if (CapabilitiesRequestAndCapabilitiesReply(h, buffer, len))
                        return Encoding.ASCII.GetString(buffer).TrimEnd('\0');
                }
                Thread.Sleep(100);
            }
            return null;
        });

    public static List<int> GetSupportedInputs(MonitorInfo monitor) =>
        InputSources.ParseFromCapabilities(GetCapabilities(monitor));

    private static T? WithPhysicalMonitor<T>(MonitorInfo monitor, Func<IntPtr, T> action)
    {
        var hMonitor = FindHMonitor(monitor.GdiDeviceName);
        if (hMonitor == IntPtr.Zero) return default;

        lock (DdcLock)
        {
            if (!GetNumberOfPhysicalMonitorsFromHMONITOR(hMonitor, out uint count) || count == 0)
                return default;

            var physical = new PHYSICAL_MONITOR[count];
            if (!GetPhysicalMonitorsFromHMONITOR(hMonitor, count, physical))
                return default;

            try
            {
                int index = Math.Min(monitor.IndexOnDevice, (int)count - 1);
                return action(physical[index].hPhysicalMonitor);
            }
            finally
            {
                DestroyPhysicalMonitors(count, physical);
            }
        }
    }

    private static IntPtr FindHMonitor(string gdiDeviceName)
    {
        IntPtr found = IntPtr.Zero;
        EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, (IntPtr h, IntPtr _, ref RECT _, IntPtr _) =>
        {
            var info = new MONITORINFOEX { cbSize = System.Runtime.InteropServices.Marshal.SizeOf<MONITORINFOEX>() };
            if (GetMonitorInfo(h, ref info) && string.Equals(info.szDevice, gdiDeviceName, StringComparison.OrdinalIgnoreCase))
            {
                found = h;
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    private static string DecodePnpId(ushort raw)
    {
        // CCD returns the EDID bytes 8-9 as a little-endian ushort; swap back to big-endian.
        int m = ((raw & 0xFF) << 8) | (raw >> 8);
        return new string([
            (char)('A' - 1 + ((m >> 10) & 0x1F)),
            (char)('A' - 1 + ((m >> 5) & 0x1F)),
            (char)('A' - 1 + (m & 0x1F))]);
    }
}
