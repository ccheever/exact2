// The user's display preferences, as the platform's accessibility settings
// hold them (LLP 1061 D5): what a browser reports as `prefers-reduced-motion`
// and `prefers-reduced-transparency`. The runner answers them through
// `exactViewport()`; the app decides what they mean. The host reads reduced
// motion itself only for the feedback it owns: press scale (LLP 1061 D2).
// An agent's `prefer` stands in for the settings, for this process only.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

enum DisplayPreferences {
    /// What an agent's `prefer` set, in place of the platform's settings.
    nonisolated(unsafe) static var agent: (reducedMotion: Bool, reducedTransparency: Bool)? {
        didSet { center.post(name: agentChanged, object: nil) }
    }
    private static let agentChanged = Notification.Name("ExactDisplayPreferencesChanged")
    private static var center: NotificationCenter {
        #if os(macOS)
        return NSWorkspace.shared.notificationCenter
        #else
        return NotificationCenter.default
        #endif
    }
    static var reducedMotion: Bool {
        if let agent { return agent.reducedMotion }
        #if os(macOS)
        return NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        #else
        return UIAccessibility.isReduceMotionEnabled
        #endif
    }
    static var reducedTransparency: Bool {
        if let agent { return agent.reducedTransparency }
        #if os(macOS)
        return NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency
        #else
        return UIAccessibility.isReduceTransparencyEnabled
        #endif
    }
    /// The ABI's form (`exact_set_preferences`): bit 0 reduced motion, bit 1
    /// reduced transparency.
    static var bits: UInt32 { (reducedMotion ? 1 : 0) | (reducedTransparency ? 2 : 0) }

    /// Calls `changed` on the main queue when either setting changes; the
    /// caller holds the tokens and removes them.
    static func observe(_ changed: @escaping () -> Void) -> [NSObjectProtocol] {
        #if os(macOS)
        let names = [NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, agentChanged]
        #else
        let names = [UIAccessibility.reduceMotionStatusDidChangeNotification, UIAccessibility.reduceTransparencyStatusDidChangeNotification, agentChanged]
        #endif
        return names.map { center.addObserver(forName: $0, object: nil, queue: .main) { _ in changed() } }
    }
    static func forget(_ tokens: [NSObjectProtocol]) {
        tokens.forEach(center.removeObserver)
    }
}
