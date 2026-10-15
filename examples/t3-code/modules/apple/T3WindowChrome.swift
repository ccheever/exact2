#if os(macOS)
import AppKit

/// T3's 52-point title row, using the window's real AppKit controls.
/// Contract's viewport-fit=cover owns the transparent, full-size content.
final class T3WindowChrome {
    private var appearanceMode = "system"
    private weak var window: NSWindow?
    private var toolbar: NSToolbar?
    private var observations: [NSObjectProtocol] = []
    /// Merged into the status read's presentation: the Mac's locale (T3Locale.swift). Full screen is
    /// Contract's `exactViewport().displayMode` (timeline-and-window-hooks KW-12c, presentation.ts).
    var status: [String: Any] { ["systemLocale": T3Locale.systemLocale()] }

    func install(_ element: ExactElement) {
        guard element.hatch == .t3Composer, let window = element.view?.window else { return }
        attach(window)
        positionControls()
    }

    /// Styles `window` once and follows its resizes and full-screen changes.
    func attach(_ window: NSWindow) {
        guard self.window !== window else { return }
        destroy()
        self.window = window
        applyAppearance()
        toolbar = Self.titleRow(window)
        fitToolbar()
        func observe(_ name: Notification.Name, _ apply: @escaping (T3WindowChrome) -> Void) {
            observations.append(NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                if let self { apply(self) }
            })
        }
        observe(NSWindow.didResizeNotification) { $0.positionControls() }
        // In full screen AppKit draws the toolbar as an opaque 52-pt bar over the full-size content, the band
        // that covered the title row (realinput-1015). T3 Code's window keeps its title row at the top there,
        // with no traffic lights to inset for: the toolbar leaves as the window enters full screen and comes
        // back once it is windowed again, and the system titlebar only reveals at the top edge.
        observe(NSWindow.willEnterFullScreenNotification) { $0.fitToolbar(fullScreen: true) }
        observe(NSWindow.didExitFullScreenNotification) { $0.fitToolbar(fullScreen: false); $0.positionControls() }
    }

    /// The title row's toolbar is on the window only while it is windowed.
    private func fitToolbar(fullScreen: Bool? = nil) {
        guard let window, let toolbar else { return }
        let wanted: NSToolbar? = (fullScreen ?? window.styleMask.contains(.fullScreen)) ? nil : toolbar
        if window.toolbar !== wanted { window.toolbar = wanted }
    }

    /// AppKit's unified toolbar centers the real traffic lights at y=26.
    /// It remains empty: the Contract draws the entire header underneath. With the
    /// full-size content `viewport-fit=cover` gives, it leaves the window's frame as it
    /// is, so the frame the host restored at launch (its autosave, #113) stays.
    @discardableResult
    static func titleRow(_ window: NSWindow) -> NSToolbar {
        let toolbar = NSToolbar(identifier: "com.exact.t3code.titlebar")
        toolbar.displayMode = .iconOnly
        toolbar.allowsUserCustomization = false
        window.toolbar = toolbar
        window.toolbarStyle = .unified
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.titlebarSeparatorStyle = .none
        return toolbar
    }

    func setAppearance(_ mode: String) {
        guard ["system", "light", "dark"].contains(mode) else { return }
        appearanceMode = mode
        applyAppearance()
    }

    private func applyAppearance() {
        let appearance: NSAppearance? = appearanceMode == "system" ? nil : NSAppearance(named: appearanceMode == "dark" ? .darkAqua : .aqua)
        if window?.appearance?.name != appearance?.name { window?.appearance = appearance }
    }

    func destroy() {
        observations.forEach(NotificationCenter.default.removeObserver)
        observations.removeAll()
        if let window, window.toolbar === toolbar { window.toolbar = nil }
        toolbar = nil
        window = nil
    }

    private func positionControls() {
        guard let window, !window.styleMask.contains(.fullScreen) else { return }
        Self.positionControls(in: window)
    }

    static func positionControls(in window: NSWindow) {
        guard window.styleMask.contains(.fullSizeContentView),
              let close = window.standardWindowButton(.closeButton) else { return }
        let shift = 16 - close.frame.minX
        for kind: NSWindow.ButtonType in [.closeButton, .miniaturizeButton, .zoomButton] {
            guard let button = window.standardWindowButton(kind) else { continue }
            button.setFrameOrigin(NSPoint(x: button.frame.minX + shift, y: button.frame.minY))
        }
    }

    deinit { destroy() }
}
#endif
