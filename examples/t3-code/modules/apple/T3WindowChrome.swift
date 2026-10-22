#if os(macOS)
import AppKit

/// The window's traffic lights at x = 16, where the reference puts them (DesktopWindow.ts
/// syncMacosWindowButtons, 74541bab17: `setWindowButtonPosition({ x: 16, y: 52 / 2 - 7 })`). The tall title
/// row itself is the host's (app.json `display_override: ["window-controls-overlay"]`, LLP 1039 D8): AppKit's
/// unified toolbar row, which puts the lights at its own x (19 on macOS 26.6) and moves none; the y (19) matches.
/// Also the window's appearance mode.
final class T3WindowChrome {
    private var appearanceMode = "system"
    private weak var window: NSWindow?
    private var observations: [NSObjectProtocol] = []

    func install(_ element: ExactElement) {
        guard element.hatch == .t3Composer, let window = element.view?.window else { return }
        attach(window)
        positionControls()
    }

    /// Follows `window`'s resizes and its returns from full screen, after which AppKit lays the lights out again.
    func attach(_ window: NSWindow) {
        guard self.window !== window else { return }
        destroy()
        self.window = window
        applyAppearance()
        func observe(_ name: Notification.Name) {
            observations.append(NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                self?.positionControls()
            })
        }
        observe(NSWindow.didResizeNotification)
        observe(NSWindow.didExitFullScreenNotification)
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
