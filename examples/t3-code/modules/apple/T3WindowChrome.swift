#if os(macOS)
import AppKit

/// T3's 52-point title row, using the window's real AppKit controls.
/// Contract's viewport-fit=cover owns the transparent, full-size content.
final class T3WindowChrome {
    private var appearanceMode = "system"
    private weak var window: NSWindow?
    private var toolbar: NSToolbar?
    private var observations: [NSObjectProtocol] = []
    /// r8-pointer (D14): the window's own frame record, outside agent runs (R8PointerWindowFrame.swift).
    var frame: R8WindowFrame?
    /// desktop-shell-details: the full-screen fact (T3FullScreen.swift), published as `t3.status`.
    let fullScreen = T3FullScreen()
    var changed: (String) -> Void = { _ in } { didSet { fullScreen.changed = { [weak self] in self?.changed("t3.status") } } }
    /// Merged into the status read's presentation: full screen and the Mac's locale (T3Locale.swift).
    var status: [String: Any] { ["fullScreen": fullScreen.fullScreen, "systemLocale": T3Locale.systemLocale()] }

    func install(_ element: ExactElement) {
        guard element.hook == .t3Composer, let window = element.view?.window else { return }
        if self.window !== window {
            destroy()
            self.window = window
            applyAppearance()
            // AppKit's unified toolbar centers the real traffic lights at y=26.
            // It remains empty: the Contract draws the entire header underneath.
            let toolbar = NSToolbar(identifier: "com.exact.t3code.titlebar")
            toolbar.displayMode = .iconOnly
            toolbar.allowsUserCustomization = false
            self.toolbar = toolbar
            window.toolbar = toolbar
            window.toolbarStyle = .unified
            window.titlebarAppearsTransparent = true
            window.titleVisibility = .hidden
            window.titlebarSeparatorStyle = .none
            for name in [NSWindow.didResizeNotification, NSWindow.didExitFullScreenNotification] {
                observations.append(NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                    self?.positionControls()
                })
            }
            frame?.attach(window)
            fullScreen.attach(window)
        }
        positionControls()
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
        frame?.detach()
        fullScreen.detach()
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
