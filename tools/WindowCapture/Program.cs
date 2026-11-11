// WindowCapture: saves a window to PNG using Windows.Graphics.Capture.
//
// Unlike GDI screen copies (Graphics.CopyFromScreen / BitBlt), this reads the window's image from
// the compositor, so GPU-composed windows (WPF dialogs, Mica backdrops) come out correctly, and
// the window is captured even when other windows cover it. Nothing is brought to the front.
//
// Usage:
//   WindowCapture --pid <id> [--title <exact title>] --out <file.png>
//   WindowCapture --pid <id> --list
//
// Windows are searched in z-order (front to back), so when a dialog and its owner share a title,
// the dialog, which is in front, is the one captured.

using System.Runtime.InteropServices;
using System.Text;
using Windows.Graphics.Capture;
using Windows.Graphics.DirectX;
using Windows.Graphics.DirectX.Direct3D11;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;
using WinRT;

int? pid = null;
string? title = null, output = null;
bool list = false;
for (int i = 0; i < args.Length; i++)
{
    switch (args[i])
    {
        case "--pid": pid = int.Parse(args[++i]); break;
        case "--title": title = args[++i]; break;
        case "--out": output = args[++i]; break;
        case "--list": list = true; break;
        default: return Fail($"Unknown argument: {args[i]}");
    }
}
if (pid is null || (!list && output is null))
    return Fail("Usage: WindowCapture --pid <id> [--title <title>] --out <file.png>  |  --pid <id> --list");

Native.SetProcessDpiAwarenessContext(-4); // per-monitor v2, so sizes are real pixels

var windows = Native.TopLevelWindows(pid.Value);
if (list)
{
    foreach (var w in windows) Console.WriteLine($"0x{w.Handle:X}  {w.Width}x{w.Height}  \"{w.Title}\"");
    return 0;
}

var target = windows.FirstOrDefault(w => title is null ? w.Title.Length > 0 : w.Title == title);
if (target is null) return Fail($"No visible window{(title is null ? "" : $" titled \"{title}\"")} in process {pid}.");
if (Native.IsIconic(target.Handle)) return Fail("The window is minimised; it can't be captured until it's restored.");

if (!GraphicsCaptureSession.IsSupported()) return Fail("Windows.Graphics.Capture isn't supported on this system.");

var item = CaptureInterop.CreateItemForWindow(target.Handle);
using var device = CaptureInterop.CreateDevice();
using var pool = Direct3D11CaptureFramePool.CreateFreeThreaded(device, DirectXPixelFormat.B8G8R8A8UIntNormalized, 1, item.Size);
using var session = pool.CreateCaptureSession(item);
session.IsCursorCaptureEnabled = false;
try { session.IsBorderRequired = false; } catch { /* older Windows: a yellow border is drawn, but not into the image */ }

var firstFrame = new TaskCompletionSource<Direct3D11CaptureFrame>(TaskCreationOptions.RunContinuationsAsynchronously);
pool.FrameArrived += (p, _) =>
{
    var frame = p.TryGetNextFrame();
    if (frame is not null && !firstFrame.TrySetResult(frame)) frame.Dispose();
};
session.StartCapture();

if (await Task.WhenAny(firstFrame.Task, Task.Delay(5000)) != firstFrame.Task)
    return Fail("Timed out waiting for a frame.");

using (var frame = await firstFrame.Task)
{
    using var bitmap = await SoftwareBitmap.CreateCopyFromSurfaceAsync(frame.Surface, BitmapAlphaMode.Premultiplied);
    using var stream = new InMemoryRandomAccessStream();
    var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, stream);
    encoder.SetSoftwareBitmap(bitmap);
    await encoder.FlushAsync();

    var bytes = new byte[stream.Size];
    using (var reader = new DataReader(stream.GetInputStreamAt(0)))
    {
        await reader.LoadAsync((uint)stream.Size);
        reader.ReadBytes(bytes);
    }
    await File.WriteAllBytesAsync(output!, bytes);
    Console.WriteLine($"saved {bitmap.PixelWidth}x{bitmap.PixelHeight} \"{target.Title}\" -> {output}");
}
return 0;

static int Fail(string message)
{
    Console.Error.WriteLine(message);
    return 1;
}

internal sealed record WindowInfo(IntPtr Handle, string Title, int Width, int Height);

internal static class Native
{
    private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int max);
    [DllImport("user32.dll")] private static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(nint value);

    /// <summary>Visible top-level windows of a process, front to back.</summary>
    public static List<WindowInfo> TopLevelWindows(int processId)
    {
        var result = new List<WindowInfo>();
        EnumWindows((h, _) =>
        {
            GetWindowThreadProcessId(h, out uint owner);
            if (owner == processId && IsWindowVisible(h))
            {
                var sb = new StringBuilder(256);
                GetWindowText(h, sb, sb.Capacity);
                GetWindowRect(h, out var r);
                result.Add(new WindowInfo(h, sb.ToString(), r.Right - r.Left, r.Bottom - r.Top));
            }
            return true;
        }, IntPtr.Zero);
        return result;
    }
}

internal static class CaptureInterop
{
    private static readonly Guid IGraphicsCaptureItemIid = new("79C3F95B-31F7-4EC2-A464-632EF5D30760");
    private static readonly Guid IDXGIDeviceIid = new("54EC77FA-1377-44E6-8C32-88FD5F44C84C");

    [ComImport, Guid("3628E81B-3CAC-4C60-B7F4-23CE0E0C3356"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IGraphicsCaptureItemInterop
    {
        IntPtr CreateForWindow([In] IntPtr window, [In] ref Guid iid);
        IntPtr CreateForMonitor([In] IntPtr monitor, [In] ref Guid iid);
    }

    [DllImport("d3d11.dll")]
    private static extern int D3D11CreateDevice(IntPtr adapter, int driverType, IntPtr software, uint flags,
        IntPtr featureLevels, uint featureLevelCount, uint sdkVersion, out IntPtr device, out int featureLevel, out IntPtr context);

    [DllImport("d3d11.dll")]
    private static extern int CreateDirect3D11DeviceFromDXGIDevice(IntPtr dxgiDevice, out IntPtr graphicsDevice);

    private const int D3D_DRIVER_TYPE_HARDWARE = 1;
    private const int D3D_DRIVER_TYPE_WARP = 5;
    private const uint D3D11_CREATE_DEVICE_BGRA_SUPPORT = 0x20;
    private const uint D3D11_SDK_VERSION = 7;

    public static GraphicsCaptureItem CreateItemForWindow(IntPtr hwnd)
    {
        var interop = GraphicsCaptureItem.As<IGraphicsCaptureItemInterop>();
        var iid = IGraphicsCaptureItemIid;
        var pointer = interop.CreateForWindow(hwnd, ref iid);
        try { return GraphicsCaptureItem.FromAbi(pointer); }
        finally { Marshal.Release(pointer); }
    }

    /// <summary>A WinRT Direct3D device wrapping a D3D11 device (hardware, falling back to WARP).</summary>
    public static IDirect3DDevice CreateDevice()
    {
        int hr = D3D11CreateDevice(IntPtr.Zero, D3D_DRIVER_TYPE_HARDWARE, IntPtr.Zero, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            IntPtr.Zero, 0, D3D11_SDK_VERSION, out var d3dDevice, out _, out var context);
        if (hr < 0)
            hr = D3D11CreateDevice(IntPtr.Zero, D3D_DRIVER_TYPE_WARP, IntPtr.Zero, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                IntPtr.Zero, 0, D3D11_SDK_VERSION, out d3dDevice, out _, out context);
        Marshal.ThrowExceptionForHR(hr);

        try
        {
            var iid = IDXGIDeviceIid;
            Marshal.ThrowExceptionForHR(Marshal.QueryInterface(d3dDevice, in iid, out var dxgiDevice));
            try
            {
                Marshal.ThrowExceptionForHR(CreateDirect3D11DeviceFromDXGIDevice(dxgiDevice, out var inspectable));
                try { return MarshalInterface<IDirect3DDevice>.FromAbi(inspectable); }
                finally { Marshal.Release(inspectable); }
            }
            finally { Marshal.Release(dxgiDevice); }
        }
        finally
        {
            Marshal.Release(d3dDevice);
            if (context != IntPtr.Zero) Marshal.Release(context);
        }
    }
}
