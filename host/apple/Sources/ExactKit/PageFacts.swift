// The page's facts (LLP 1069.000 D2, D3): whether any of the app can be
// seen (`visibilityState`), whether the device is online (`onLine`), whether
// a share sheet exists (`canShare`, LLP 1069.003 D5), and the root font size
// `rem` lengths follow (Dynamic Type on iOS; 16 on the Mac, as a browser's
// `medium`). The runner answers the first three through `exactPage()`; the
// kernel lays out `rem` against the fourth. Under the agent the machine is
// never read: the drive's values stand in (`prefer`'s `page` group), visible,
// online, a share sheet, 16 until it says otherwise (LLP 1069.000 D6).
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import Network

enum PageFacts {
    /// What an agent's `prefer` set, in place of the platform's readings.
    nonisolated(unsafe) static var agent = (hidden: false, onLine: true, canShare: true, rootFontSize: 16.0) {
        didSet { changed() }
    }
    private static let agentChanged = Notification.Name("ExactPageFactsChanged")
    private static func changed() { NotificationCenter.default.post(name: agentChanged, object: nil) }

    /// One path monitor for the process, started on first use. Until its
    /// first update the device is taken to be online, as a browser starts.
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
    /// or every window fully occluded. A window behind another is visible.
    static var hidden: Bool {
        if ExactEnv.agentMode { return agent.hidden }
        #if os(macOS)
        if NSApp.isHidden { return true }
        let windows = NSApp.windows.filter { $0.isVisible }
        return !windows.isEmpty && windows.allSatisfy { !$0.occlusionState.contains(.visible) }
        #else
        return AppBackground.now
        #endif
    }
    static var onLine: Bool {
        if ExactEnv.agentMode { return agent.onLine }
        startMonitor()
        return satisfied
    }
    /// Both Apple platforms have a share sheet.
    static var canShare: Bool { ExactEnv.agentMode ? agent.canShare : true }

    /// The root font size in points: iOS scales CSS's 16 by the preferred
    /// content size category, as `UIFontMetrics` scales body text.
    static var rootFontSize: Double {
        if ExactEnv.agentMode { return agent.rootFontSize }
        #if os(macOS)
        return 16
        #else
        return Double(UIFontMetrics.default.scaledValue(for: 16))
        #endif
    }

    /// The ABI's form (`exact_set_page`): bit 0 hidden, bit 1 offline, bit 2
    /// a share sheet.
    static var bits: UInt32 { (hidden ? 1 : 0) | (onLine ? 0 : 2) | (canShare ? 4 : 0) }

    /// Calls `changed` on the main queue when any reading may have changed;
    /// the caller holds the tokens and removes them.
    static func observe(_ changed: @escaping () -> Void) -> [NSObjectProtocol] {
        if !ExactEnv.agentMode { startMonitor() }
        #if os(macOS)
        let workspace = [NSApplication.didHideNotification, NSApplication.didUnhideNotification,
                         NSWindow.didChangeOcclusionStateNotification, agentChanged]
        #else
        let workspace = [UIApplication.didEnterBackgroundNotification, UIApplication.willEnterForegroundNotification,
                         UIContentSizeCategory.didChangeNotification, agentChanged]
        #endif
        return workspace.map {
            NotificationCenter.default.addObserver(forName: $0, object: nil, queue: .main) { _ in
                #if !os(macOS)
                AppBackground.invalidate()
                #endif
                changed()
            }
        }
    }
    static func forget(_ tokens: [NSObjectProtocol]) {
        tokens.forEach(NotificationCenter.default.removeObserver)
    }
}

#if !os(macOS)
/// Whether the app is in the background, read from UIKit once per change.
/// `applicationState` crosses into the scene machinery on every read — about
/// 5 ms/s of the iPhone's main thread when the canvases asked each frame of
/// the Extra Heavy feed at rest — so it is read once and read again after a
/// lifecycle notification: at it, and a main-queue turn later, since UIKit's
/// "will" notifications precede the state's update.
enum AppBackground {
    nonisolated(unsafe) private static var cached: Bool?
    nonisolated(unsafe) private static var observing = false
    static var now: Bool {
        if !observing { observe() }
        if let cached { return cached }
        let value = UIApplication.shared.applicationState == .background
        cached = value
        return value
    }
    static func invalidate() { cached = nil }
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
