#if os(macOS)
import AppKit

/// Lane r8-pointer (D14): the main window comes back where and as large as it
/// was left. The host restores its frame autosave and only then turns on
/// `viewport-fit=cover`'s full-size content; AppKit keeps the content rect when
/// that style is inserted, so every launch lost the 32 pt titlebar (818 → 786 →
/// 754, the top edge moving down). The app keeps its own frame record and puts
/// it back once the window is in its final style (the framework is not changed).
/// Agent runs never read or write it: their windows are placed by the driver.
final class R8WindowFrame {
    static let key = "t3.window.frame"
    private let load: () -> String?
    private let store: (String) -> Void
    private let screens: () -> [NSRect]
    private weak var window: NSWindow?
    private var observations: [NSObjectProtocol] = []
    private(set) var tracking = false

    init(load: @escaping () -> String? = { UserDefaults.standard.string(forKey: R8WindowFrame.key) },
         store: @escaping (String) -> Void = { UserDefaults.standard.set($0, forKey: R8WindowFrame.key) },
         screens: @escaping () -> [NSRect] = { NSScreen.screens.map(\.visibleFrame) }) {
        self.load = load; self.store = store; self.screens = screens
    }

    /// A saved frame worth restoring: finite, at least 200×200, and with a
    /// 100×60 piece of its titlebar strip on a screen (else the window could not
    /// be reached and the host's own frame stands).
    static func usable(_ text: String?, screens: [NSRect]) -> NSRect? {
        guard let text, !text.isEmpty else { return nil }
        let frame = NSRectFromString(text)
        guard frame.width >= 200, frame.height >= 200, frame.width.isFinite, frame.height.isFinite,
              frame.minX.isFinite, frame.minY.isFinite else { return nil }
        let strip = NSRect(x: frame.minX, y: frame.maxY - 60, width: frame.width, height: 60)
        return screens.contains(where: { let hit = $0.intersection(strip); return hit.width >= 100 && hit.height >= 30 }) ? frame : nil
    }

    /// Once per window: restore now and again after the launch finishes its
    /// style changes (the next main-queue turn), then record every move and resize.
    func attach(_ window: NSWindow, settle: @escaping (@escaping () -> Void) -> Void = { DispatchQueue.main.async(execute: $0) }) {
        guard self.window !== window else { return }
        detach()
        self.window = window
        restore()
        settle { [weak self] in
            guard let self, self.window === window else { return }
            self.restore()
            self.tracking = true
            self.save()
            for name in [NSWindow.didMoveNotification, NSWindow.didResizeNotification, NSWindow.didEndLiveResizeNotification, NSWindow.willCloseNotification] {
                self.observations.append(NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in self?.save() })
            }
        }
    }

    func restore() {
        guard let window, !window.styleMask.contains(.fullScreen),
              let frame = Self.usable(load(), screens: screens()), frame != window.frame else { return }
        window.setFrame(frame, display: true)
    }

    func save() {
        guard tracking, let window, !window.styleMask.contains(.fullScreen), window.frame.width >= 200, window.frame.height >= 200 else { return }
        store(NSStringFromRect(window.frame))
    }

    func detach() {
        observations.forEach(NotificationCenter.default.removeObserver)
        observations.removeAll()
        tracking = false
        window = nil
    }

    deinit { detach() }
}
#endif
