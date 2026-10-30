// The WinUI reference's process (LLP 1115.004 D13): draws each specimen of
// specimens.json alone in its window, in the theme it is given, and writes
// `<out>/<scheme>/<id>.png` and `<out>/<scheme>/geometry.json`, then
// `done.txt` (or `error.txt`) and exits.
//
//   Reference.exe --scheme light|dark --specimens <specimens.json> --out <dir>
//
// It must run in the signed-in desktop session: launched from an SSH session,
// WinUI fails before its first frame. draw.mjs starts it there.
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices.WindowsRuntime;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Threading.Tasks;
using Windows.Graphics.Imaging;

namespace Reference;

public partial class App : Application
{
    /// Around a specimen, room for its shadow, as Android's oracle leaves.
    const double Margin = 16;

    readonly Dictionary<string, string> options;

    App(Dictionary<string, string> options)
    {
        this.options = options;
        // The theme a process's resources resolve in is set before they load.
        RequestedTheme = options["--scheme"] == "dark" ? ApplicationTheme.Dark : ApplicationTheme.Light;
        InitializeComponent();
    }

    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        var dir = Path.Combine(options["--out"], options["--scheme"]);
        try
        {
            Directory.CreateDirectory(dir);
            var manifest = JsonNode.Parse(File.ReadAllText(options["--specimens"]))!;
            var window = new Window { Title = "Exact WinUI reference" };
            window.AppWindow.Resize(new Windows.Graphics.SizeInt32(1200, 800));
            window.Activate();
            var geometry = new JsonObject();
            foreach (var specimen in manifest["specimens"]!.AsArray())
            {
                var id = (string)specimen!["id"]!;
                geometry[id] = await Draw(window, id, specimen["size"]?.AsArray(), dir);
            }
            var root = new JsonObject
            {
                ["scheme"] = options["--scheme"],
                ["scale"] = window.Content.XamlRoot.RasterizationScale,
                ["windowsAppSdk"] = "1.8.260921001",
                ["specimens"] = geometry,
            };
            File.WriteAllText(Path.Combine(dir, "geometry.json"),
                root.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));
            File.WriteAllText(Path.Combine(dir, "done.txt"), "done");
        }
        catch (Exception e)
        {
            Directory.CreateDirectory(dir);
            File.WriteAllText(Path.Combine(dir, "error.txt"), e.ToString());
        }
        // Application.Exit can leave the process waiting on its window; nothing is left to do.
        Environment.Exit(0);
    }

    /// One specimen alone in the window, on Fluent's base background, its
    /// image and the control's box within it (effective pixels).
    static async Task<JsonObject> Draw(Window window, string id, JsonArray? size, string dir)
    {
        var control = Specimens.Build(id);
        if (size?[0] is JsonNode w) control.Width = (double)w;
        if (size?[1] is JsonNode h) control.Height = (double)h;
        var frame = new Border
        {
            Padding = new Thickness(Margin),
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
            Background = (Brush)Current.Resources["SolidBackgroundFillColorBaseBrush"],
            Child = control,
        };
        var loaded = new TaskCompletionSource();
        frame.Loaded += (_, _) => loaded.TrySetResult();
        window.Content = new Grid { Children = { frame } };
        await loaded.Task;
        if (id == "text-field-focused") control.Focus(FocusState.Keyboard);
        // Two frames after loading: templates applied, theme transitions settled.
        for (var i = 0; i < 2; i++) await NextFrame();
        await Task.Delay(400);
        var bitmap = new RenderTargetBitmap();
        await bitmap.RenderAsync(frame);
        var pixels = (await bitmap.GetPixelsAsync()).ToArray();
        using (var file = File.Create(Path.Combine(dir, $"{id}.png")))
        {
            var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, file.AsRandomAccessStream());
            var dpi = 96 * window.Content.XamlRoot.RasterizationScale;
            encoder.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied,
                (uint)bitmap.PixelWidth, (uint)bitmap.PixelHeight, dpi, dpi, pixels);
            await encoder.FlushAsync();
        }
        var at = control.TransformToVisual(frame).TransformPoint(new Windows.Foundation.Point(0, 0));
        return new JsonObject
        {
            ["frame"] = new JsonArray(frame.ActualWidth, frame.ActualHeight),
            ["control"] = new JsonArray(at.X, at.Y, control.ActualWidth, control.ActualHeight),
            ["pixels"] = new JsonArray(bitmap.PixelWidth, bitmap.PixelHeight),
        };
    }

    static Task NextFrame()
    {
        var next = new TaskCompletionSource();
        EventHandler<object>? handler = null;
        handler = (_, _) =>
        {
            CompositionTarget.Rendering -= handler;
            next.TrySetResult();
        };
        CompositionTarget.Rendering += handler;
        return next.Task;
    }

    [STAThread]
    static void Main(string[] args)
    {
        // Match the monitor's DPI so these captures can be compared with Shelf.
        SetProcessDpiAwarenessContext(new IntPtr(-4));
        var options = new Dictionary<string, string>();
        for (var i = 0; i + 1 < args.Length; i += 2) options[args[i]] = args[i + 1];
        foreach (var name in new[] { "--scheme", "--specimens", "--out" })
        {
            if (!options.ContainsKey(name)) throw new ArgumentException($"{name} is required");
        }
        WinRT.ComWrappersSupport.InitializeComWrappers();
        Start(_ =>
        {
            // As WinUI's generated Main does: an await resumes on the UI thread.
            System.Threading.SynchronizationContext.SetSynchronizationContext(
                new Microsoft.UI.Dispatching.DispatcherQueueSynchronizationContext(
                    Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread()));
            new App(options);
        });
    }

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    static extern bool SetProcessDpiAwarenessContext(IntPtr context);
}
