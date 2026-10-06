#if os(macOS)
import AppKit

/// The main window's full-screen state as a native fact (task desktop-shell-details; reference
/// apps/desktop getWindowFullscreenState / onWindowFullscreenStateChange, 1e2ecbd975, MIT, see
/// LICENSE-T3; X27, #113). The initial value is the window's style mask; NSWindow's did-enter and
/// did-exit notifications update it and publish `t3.status`, whose presentation carries
/// `fullScreen`, so the title rows drop the traffic lights' 90 pt inset (AppSidebarLayout.tsx).
final class T3FullScreen {
    private weak var window: NSWindow?
    private var observations: [NSObjectProtocol] = []
    private(set) var fullScreen = false
    var changed: () -> Void = {}

    func attach(_ window: NSWindow) {
        guard self.window !== window else { return }
        detach()
        self.window = window
        fullScreen = window.styleMask.contains(.fullScreen)
        for (name, value) in [(NSWindow.didEnterFullScreenNotification, true), (NSWindow.didExitFullScreenNotification, false)] {
            observations.append(NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                self?.set(value)
            })
        }
        changed()
    }
    func set(_ value: Bool) {
        guard fullScreen != value else { return }
        fullScreen = value
        changed()
    }
    func detach() {
        observations.forEach(NotificationCenter.default.removeObserver)
        observations.removeAll()
        window = nil
    }
    deinit { detach() }
}
#endif
