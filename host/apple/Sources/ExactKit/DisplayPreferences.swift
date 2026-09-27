// The user's display preferences, as the platform's accessibility settings
// hold them (LLP 1061 D4): what a browser reports as `prefers-reduced-motion`
// and `prefers-reduced-transparency`. The runner answers them through
// `exactViewport()`; the app decides what they mean (`rules/NOT-DOING.md`
// §Motion). The host reads reduced motion itself only for the feedback it
// owns: press scale (LLP 1061 D2).
#if os(macOS)
import AppKit
#else
import UIKit
#endif

enum DisplayPreferences {
    static var reducedMotion: Bool {
        #if os(macOS)
        NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        #else
        UIAccessibility.isReduceMotionEnabled
        #endif
    }
    static var reducedTransparency: Bool {
        #if os(macOS)
        NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency
        #else
        UIAccessibility.isReduceTransparencyEnabled
        #endif
    }
    /// The ABI's form (`exact_set_preferences`): bit 0 reduced motion, bit 1
    /// reduced transparency.
    static var bits: UInt32 { (reducedMotion ? 1 : 0) | (reducedTransparency ? 2 : 0) }

    /// Calls `changed` on the main queue when either setting changes; the
    /// caller holds the tokens and removes them.
    static func observe(_ changed: @escaping () -> Void) -> [NSObjectProtocol] {
        #if os(macOS)
        let center = NSWorkspace.shared.notificationCenter
        let names = [NSWorkspace.accessibilityDisplayOptionsDidChangeNotification]
        #else
        let center = NotificationCenter.default
        let names = [UIAccessibility.reduceMotionStatusDidChangeNotification, UIAccessibility.reduceTransparencyStatusDidChangeNotification]
        #endif
        return names.map { center.addObserver(forName: $0, object: nil, queue: .main) { _ in changed() } }
    }
    static func forget(_ tokens: [NSObjectProtocol]) {
        #if os(macOS)
        let center = NSWorkspace.shared.notificationCenter
        #else
        let center = NotificationCenter.default
        #endif
        tokens.forEach(center.removeObserver)
    }
}
