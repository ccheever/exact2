#if os(macOS)
import AppKit

/// The title-bar area beside the window's controls, what
/// `env(titlebar-area-x|y|width|height)` reads: Window Controls Overlay's
/// "available area of the title bar" (https://wicg.github.io/window-controls-overlay/),
/// which excludes the area under the window's controls. AppKit's own
/// geometry: the title bar's band where the page's view lies under it
/// (`viewport-fit=cover`), less the traffic lights with the margin AppKit
/// keeps before the first kept after the last, which is where macOS 26's
/// horizontally corner-adapted safe area begins.
enum TitleBarAreaMac {
    /// x, y, width, height from the view's top left; all zero while the
    /// title bar is hidden (full screen) or a route's toolbar fills it (LLP
    /// 1115.002 D2); `nil` where the page does not underlap the title bar, so
    /// every variable is undefined and its fallback applies, as in a browser
    /// tab. `overlay`: the toolbar is `window-controls-overlay`'s empty one,
    /// whose band is the title bar's (LLP 1039 D8).
    static func rect(of view: NSView, cover: Bool, overlay: Bool = false) -> [CGFloat]? {
        guard cover, let window = view.window, window.styleMask.contains(.titled),
              window.styleMask.contains(.fullSizeContentView) else { return nil }
        if window.styleMask.contains(.fullScreen) { return [0, 0, 0, 0] }
        // The standard buttons' superview is the title bar's own view.
        let buttons = [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton].compactMap { window.standardWindowButton($0) }
        guard let bar = buttons.first?.superview else { return nil }
        let bounds = view.bounds
        let band = view.convert(bar.bounds, from: bar).intersection(bounds)
        // A view below the title bar, embedded lower in the window, does not underlap it.
        guard !band.isNull, band.width > 0, band.height > 0 else { return nil }
        if window.toolbar?.isVisible == true, !overlay { return [0, 0, 0, 0] }
        var left = band.minX
        let shown = buttons.filter { !$0.isHidden && $0.superview === bar }
        if let first = shown.map(\.frame.minX).min(), let last = shown.map(\.frame.maxX).max() {
            let end = NSRect(x: last + first - bar.bounds.minX, y: bar.bounds.minY, width: 0, height: bar.bounds.height)
            left = max(left, view.convert(end, from: bar).maxX)
        }
        let top = view.isFlipped ? band.minY - bounds.minY : bounds.maxY - band.maxY
        return [left - bounds.minX, top, max(0, band.maxX - left), band.height]
    }
}

extension ExactSession {
    /// The title-bar area (`TitleBarAreaMac`): x, y, width, height, or none.
    public func titlebarArea(_ rect: [CGFloat]?) {
        guard booted, state != .destroyed else { return }
        apply(runtime.titlebarArea(rect))
    }
}
#endif
