// The page's facts (LLP 1069.000 D2, D3): whether any of the app can be
// seen (`visibilityState`), whether the device is online (`onLine`), whether
// a share sheet exists (`canShare`, LLP 1069.003 D5) and takes files and
// pictures (`canShareFiles`, `canSharePictures`, LLP 1125 D12), whether
// the document pickers do (`canOpenFiles`), whether the session's window has
// the focus (`hasFocus`, `document.hasFocus()`: #114), and the root font
// size `rem` lengths follow (the platform's body size, LLP 1115 D3: Dynamic
// Type's body on iOS, 17 at the default; `NSFont.systemFontSize`, 13, on the
// Mac). The runner answers all but the last through `exactPage()`; the
// kernel lays out `rem` against the last. Under the agent the machine's facts are
// not read: the drive's values stand in (`prefer`'s `page` group), visible,
// online, a share sheet, the pickers, focus, until it says otherwise (LLP
// 1069.000 D6), unless `EXACT_DEVICE=real` asks for the machine's (LLP
// 1069.007 D8: a person looking at the real thing). The text size is the
// device's, as UIKit's controls have it, until `prefer root-font-size` names
// one (LLP 1116.019 D2): a screenshot at an accessibility size is what a
// person sees, not Exact's text at 17 points among controls at 53.
#if os(macOS)
import AppKit
typealias FocusWindow = NSWindow
#else
import UIKit
typealias FocusWindow = UIWindow
#endif
import Network

package enum PageFacts {
    /// What an agent's `prefer` set, in place of the platform's readings.
    nonisolated(unsafe) static var agent = (hidden: false, onLine: true, canShare: true, canShareFiles: true, canSharePictures: true, canOpenFiles: true, hasFocus: true, rootFontSize: Double?.none) {
        didSet { changed() }
    }
    /// What the build channel's `pin` set (LLP 1122 D4), over the platform's
    /// and the agent's; nil hands it back.
    nonisolated(unsafe) static var pinnedRootFontSize: Double?
    /// The drive's values stand in for the machine's (LLP 1069.007 D2, D8).
    static let substituted = ExactEnv.agentMode && ExactEnv.environment["EXACT_DEVICE"] != "real"
    private static let agentChanged = Notification.Name("ExactPageFactsChanged")
    private static func changed() { NotificationCenter.default.post(name: agentChanged, object: nil) }

    /// One path monitor for the process, started when a session's plan
    /// reads `onLine`: it loads Network and keeps a queue, which an app that
    /// never asks should not pay. Until its first update the device is taken
    /// to be online, as a browser starts.
    nonisolated(unsafe) private static var monitor: NWPathMonitor?
    nonisolated(unsafe) private static var satisfied = true
    private static func startMonitor() {
        guard monitor == nil else { return }
        let m = NWPathMonitor()
        m.pathUpdateHandler = { path in
            let online = path.status == .satisfied
            DispatchQueue.main.async {
                guard online != satisfied else { return }
                satisfied = online
                changed()
            }
        }
        m.start(queue: DispatchQueue(label: "exact.page.path"))
        monitor = m
    }

    /// Nothing of the app can be seen: iOS in the background; macOS hidden,
    /// or every window on screen or in the Dock minimized or fully occluded
    /// (Chrome reads a window minimized with ⌘M as hidden; AppKit's
    /// `isVisible` is false for it). A window behind another is visible.
    static var hidden: Bool {
        if substituted { return agent.hidden }
        #if os(macOS)
        if NSApp.isHidden { return true }
        let windows = NSApp.windows.filter { $0.isVisible || $0.isMiniaturized }
        return !windows.isEmpty && windows.allSatisfy { $0.isMiniaturized || !$0.occlusionState.contains(.visible) }
        #else
        return AppBackground.now
        #endif
    }
    static var onLine: Bool {
        if substituted { return agent.onLine }
        startMonitor()
        return satisfied
    }
    /// Both Apple platforms have a share sheet.
    static var canShare: Bool { substituted ? agent.canShare : true }
    /// iOS's and the Mac's sheets take file URLs and a node's picture
    /// (LLP 1125 D12); tvOS has no sheet to take them.
    static var canShareFiles: Bool {
        if substituted { return agent.canShareFiles }
        #if os(tvOS)
        return false
        #else
        return true
        #endif
    }
    static var canSharePictures: Bool { substituted ? agent.canSharePictures : canShareFiles }
    /// The document pickers (LLP 1069.010 D2): the Mac's panels, iOS's
    /// document picker; tvOS has none (studio diary R31).
    static var canOpenFiles: Bool {
        if substituted { return agent.canOpenFiles }
        #if os(tvOS)
        return false
        #else
        return true
        #endif
    }

    /// `document.hasFocus()` for a session in `window` (#114): the window the
    /// system's keyboard focus is in. macOS: the app is active and the window
    /// is its key window, so another app in front, another of its windows, or
    /// a sheet over it (an open panel, as a browser's file chooser blurs its
    /// window) takes the focus. iOS and tvOS: the window's scene is in front
    /// and active, so Control Centre, the app switcher or a system alert over
    /// it takes it. Before the view has a window, the app's own state.
    static func hasFocus(_ window: FocusWindow?) -> Bool {
        if substituted { return agent.hasFocus }
        #if os(macOS)
        return NSApp.isActive && (window?.isKeyWindow ?? true)
        #else
        if let scene = window?.windowScene { return scene.activationState == .foregroundActive }
        return UIApplication.shared.applicationState == .active
        #endif
    }

    /// The root font size in points: the platform's body size (LLP 1115
    /// D3). iOS: Dynamic Type's body, 17 at the default size and scaled by
    /// the preferred content size category. macOS: `NSFont.systemFontSize`,
    /// 13. tvOS keeps CSS's 16 scaled by the category until its own pass
    /// (its body style is 29 points, read at ten feet).
    package static var rootFontSize: Double {
        if let pinnedRootFontSize { return pinnedRootFontSize }
        if substituted, let size = agent.rootFontSize { return size }
        #if os(macOS)
        return Double(NSFont.systemFontSize)
        #elseif os(tvOS)
        return Double(UIFontMetrics.default.scaledValue(for: 16))
        #else
        return Double(UIFont.preferredFont(forTextStyle: .body).pointSize)
        #endif
    }

    /// The body size at the platform's default settings: a text's size when
    /// its style names none.
    package static let defaultRootFontSize = SessionFacts.defaultRootFontSize

    /// What an agent's `prefer display-scale` set in place of the display's
    /// own; nil reads the display.
    nonisolated(unsafe) static var agentDisplayScale: Double? {
        didSet { changed() }
    }

    /// The scale of the display `view` is drawn on, device pixels a point
    /// (LLP 1115.000 D10.3): UIKit's `displayScale` (3 or 2 on a phone), the
    /// window's `backingScaleFactor` on the Mac (2 on a Retina display, 1
    /// where there is none or no window yet). A hairline (`-exact-hairline`)
    /// is one device pixel of it, so it is told to the kernel as the root
    /// font size is. Under the agent it is still the display's, which is the
    /// simulator's or the Mac's own, unless the drive names one.
    package static func displayScale(_ view: PlatformView?) -> Double {
        if let agentDisplayScale { return agentDisplayScale }
        #if os(macOS)
        return Double(max(1, view?.window?.backingScaleFactor ?? 1))
        #else
        return Double(max(1, view?.traitCollection.displayScale ?? 1))
        #endif
    }

    /// The system is about to suspend the app (LLP 1125 D4): iOS and tvOS
    /// in the background, unless a `playback` session keeps it running. A
    /// Mac app is never suspended for being hidden.
    static var suspended: Bool {
        #if os(macOS)
        return false
        #else
        return hidden && !AudioSession.playsInBackground
        #endif
    }

    /// The ABI's form (`exact_set_page`) for a session in `window`: bit 0
    /// hidden, bit 1 offline, bit 2 a share sheet, bit 3 the document
    /// pickers, bit 4 without focus, bit 7 about to be suspended, bits 8 and
    /// 9 a share takes files and pictures. A session whose plan does not read
    /// `onLine` starts no monitor for it.
    static func bits(_ window: FocusWindow?, readsOnLine: Bool = true) -> UInt32 {
        let online = readsOnLine ? onLine : substituted ? agent.onLine : satisfied
        return (hidden ? 1 : 0) | (online ? 0 : 2) | (canShare ? 4 : 0) | (canOpenFiles ? 8 : 0) | (hasFocus(window) ? 0 : 16) | (suspended ? 128 : 0) | (canShareFiles ? 256 : 0) | (canSharePictures ? 512 : 0)
    }

    /// Calls `changed` on the main queue when any reading may have changed;
    /// the caller holds the tokens and removes them.
    static func observe(_ changed: @escaping () -> Void) -> [NSObjectProtocol] {
        #if os(macOS)
        // Visibility: hiding, occlusion, and a minimize or restore, which
        // need not change the occlusion. Focus (#114): any window's key change
        // or the app's activation; each session reads its own window, and the
        // same facts again commit nothing.
        let workspace = [NSApplication.didHideNotification, NSApplication.didUnhideNotification,
                         NSWindow.didChangeOcclusionStateNotification, NSWindow.didMiniaturizeNotification,
                         NSWindow.didDeminiaturizeNotification, NSWindow.didBecomeKeyNotification,
                         NSWindow.didResignKeyNotification, NSApplication.didBecomeActiveNotification,
                         NSApplication.didResignActiveNotification, agentChanged]
        #else
        // A prewarmed launch (iOS starts the process in the background ahead
        // of a tap, often after an update) never posts willEnterForeground:
        // becoming active is the only word that the app can be seen. Focus
        // (#114) follows the scene's activation; a "will" precedes the
        // state's update, so each is read again a main-queue turn later.
        let workspace = [UIApplication.didEnterBackgroundNotification, UIApplication.willEnterForegroundNotification,
                         UIApplication.didBecomeActiveNotification, UIApplication.willResignActiveNotification,
                         UIScene.didActivateNotification, UIScene.willDeactivateNotification,
                         UIContentSizeCategory.didChangeNotification, agentChanged]
        #endif
        return workspace.map {
            NotificationCenter.default.addObserver(forName: $0, object: nil, queue: .main) { _ in
                #if !os(macOS)
                AppBackground.invalidate()
                DispatchQueue.main.async { changed() }
                #endif
                changed()
            }
        }
    }
    static func forget(_ tokens: [NSObjectProtocol]) {
        tokens.forEach(NotificationCenter.default.removeObserver)
    }
}

extension ExactSession {
    /// @ref LLP 1069.000 D2 — told after every boot and on each change; a
    /// change while iOS suspends the process lands with the foreground
    /// notification, in the same turn.
    func tellPage() {
        guard booted, state != .destroyed else { return }
        #if canImport(UIKit)
        // Out of the foreground, a press hands nothing on (PressHandoffIOS).
        if view?.window?.windowScene?.activationState != .foregroundActive { presenter.supersedePress() }
        #endif
        apply(runtime.setPage(PageFacts.bits(pageWindow, readsOnLine: runtime.readsOnLine())))
        apply(runtime.setRootFontSize(PageFacts.rootFontSize))
        tellDisplayScale()
    }
    /// LLP 1115.000 D10.3: the display's scale, told with the page's facts
    /// after every boot and again when the view's display changes
    /// (`ExactViewIOS`, `ExactViewMac`); the same scale again commits
    /// nothing, and a new one only where a row is a hairline.
    func tellDisplayScale() {
        guard booted, state != .destroyed else { return }
        apply(runtime.setDisplayScale(PageFacts.displayScale(presenter.viewport)))
    }
}

extension ExactSession {
    /// The window `document.hasFocus()` is read of: the session's view's. On
    /// macOS, the one of the session's windows that is key when one is: a
    /// `dialog`'s sheet holds the keyboard for its page (LLP 1115.002 D7.2),
    /// which another app's sheet (an open panel) does not.
    var pageWindow: FocusWindow? {
        #if os(macOS)
        if let key = NSApp.keyWindow, presenter.hosts(key) { return key }
        #endif
        return view?.window
    }
}

#if !os(macOS)
/// Whether the app is in the background, read from UIKit once per change.
/// `applicationState` crosses into the scene machinery on every read — about
/// 5 ms/s of the iPhone's main thread when the canvases asked each frame of
/// the Extra Heavy feed at rest — so it is read once and read again after a
/// lifecycle notification: at it, and a main-queue turn later, since UIKit's
/// "will" notifications precede the state's update.
package enum AppBackground {
    nonisolated(unsafe) private static var cached: Bool?
    nonisolated(unsafe) private static var observing = false
    package static var now: Bool {
        if !observing { observe() }
        if let cached { return cached }
        let value = UIApplication.shared.applicationState == .background
        cached = value
        return value
    }
    package static func invalidate() { cached = nil }
    private static func observe() {
        observing = true
        for name in [UIApplication.willResignActiveNotification, UIApplication.didEnterBackgroundNotification,
                     UIApplication.willEnterForegroundNotification, UIApplication.didBecomeActiveNotification] {
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: nil) { _ in
                invalidate()
                DispatchQueue.main.async { invalidate() }
            }
        }
    }
}
#endif
