// The user's display preferences, as the platform's accessibility settings
// hold them (LLP 1061 D5; LLP 1069.000 D1): what a browser reports as
// `prefers-reduced-motion`, `prefers-reduced-transparency`,
// `prefers-contrast` and `prefers-color-scheme`. The runner answers them
// through `exactViewport()`; the app decides what they mean. Press feedback
// stays visible under reduced motion (LLP 1061's 2026-09-27 ruling).
// An agent's `prefer` stands in for the settings, for this process only.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

enum DisplayPreferences {
    /// What an agent's `prefer` set, in place of the platform's settings.
    /// Under the agent it starts at `no-preference` for both, never the
    /// machine's (LLP 1069.007 D2: facts are fixed at launch).
    nonisolated(unsafe) static var agent: (reducedMotion: Bool, reducedTransparency: Bool)? = ExactEnv.agentMode ? (false, false) : nil {
        didSet { changed() }
    }
    /// What an agent's `prefer` set for `prefers-contrast`, in CSS's words;
    /// `no-preference` from launch under the agent (LLP 1069.007 D2).
    nonisolated(unsafe) static var agentContrast: String? = ExactEnv.agentMode ? "no-preference" : nil {
        didSet { changed() }
    }
    /// What an agent's `prefer` set for `color-gamut`; `srgb` from launch
    /// under the agent (LLP 1069.007 D2).
    nonisolated(unsafe) static var agentGamut: String? = ExactEnv.agentMode ? "srgb" : nil {
        didSet { changed() }
    }
    private static let agentChanged = Notification.Name("ExactDisplayPreferencesChanged")
    private static var center: NotificationCenter {
        #if os(macOS)
        return NSWorkspace.shared.notificationCenter
        #else
        return NotificationCenter.default
        #endif
    }
    /// Something a reading depends on changed outside a platform
    /// notification (an agent's `prefer`, the agent's system scheme): every
    /// session tells its runner again.
    static func changed() { center.post(name: agentChanged, object: nil) }

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
    /// `prefers-contrast`: Increase Contrast (macOS) or Increase Contrast /
    /// Darker System Colors (iOS) is `more`; neither platform has `less`.
    static var contrast: String {
        if let agentContrast { return agentContrast }
        #if os(macOS)
        return NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast ? "more" : "no-preference"
        #else
        return UIAccessibility.isDarkerSystemColorsEnabled ? "more" : "no-preference"
        #endif
    }
    /// `color-gamut` (LLP 1100 D9). Neither platform reports a rec2020 panel.
    static var gamut: String {
        if let agentGamut { return agentGamut }
        #if os(macOS)
        return NSScreen.main?.canRepresent(.p3) == true ? "p3" : "srgb"
        #else
        return UIScreen.main.traitCollection.displayGamut == .P3 ? "p3" : "srgb"
        #endif
    }
    /// `dynamic-range: high` (LLP 1100 D9): the main display's potential headroom.
    static var highDynamicRange: Bool {
        if let pinned = DisplayRange.pinned { return pinned > 1 }
        #if os(macOS)
        return (NSScreen.main?.maximumPotentialExtendedDynamicRangeColorComponentValue ?? 1) > 1
        #else
        return UIScreen.main.potentialEDRHeadroom > 1
        #endif
    }
    /// `prefers-color-scheme: dark` — the *system's* appearance, beneath any
    /// `setScheme` the app chose (LLP 1034 D3 as amended by LLP 1069.000 D1).
    #if os(macOS)
    static var systemDark: Bool {
        if let agent = Agent.systemAppearance { return agent.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua }
        return UserDefaults.standard.string(forKey: "AppleInterfaceStyle")?.lowercased().contains("dark") == true
    }
    #endif
    /// The ABI's form (`exact_set_preferences`): bit 0 reduced motion, bit 1
    /// reduced transparency, bit 2 contrast more, bit 3 contrast less (both:
    /// custom), bit 4 a dark system, bit 5 pointer `coarse`, bit 6 pointer
    /// `none`, bit 7 hover `none` (zero: a mouse, `fine` and `hover`), bits
    /// 8–9 the gamut (1 P3, 2 rec2020), bit 10 a high dynamic range.
    static func bits(systemDark: Bool) -> UInt32 {
        let contrastBits: UInt32 = switch contrast { case "more": 4; case "less": 8; case "custom": 12; default: 0 }
        let gamutBits: UInt32 = switch gamut { case "p3": 256; case "rec2020": 512; default: 0 }
        return (reducedMotion ? 1 : 0) | (reducedTransparency ? 2 : 0) | contrastBits | (systemDark ? 16 : 0) | inputBits
            | gamutBits | (highDynamicRange ? 1024 : 0)
    }
    /// CSS's `pointer` and `hover` for the primary input: a Siri Remote
    /// points at nothing, a finger is coarse, and neither hovers.
    #if os(tvOS)
    static let inputBits: UInt32 = 64 | 128
    #elseif os(iOS)
    static let inputBits: UInt32 = 32 | 128
    #else
    static let inputBits: UInt32 = 0
    #endif

    /// Calls `changed` on the main queue when any setting changes; the
    /// caller holds the tokens and removes them.
    static func observe(_ changed: @escaping () -> Void) -> [NSObjectProtocol] {
        #if os(macOS)
        let names = [NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, agentChanged]
        var tokens = names.map { center.addObserver(forName: $0, object: nil, queue: .main) { _ in changed() } }
        // The system appearance arrives on the distributed centre; the
        // preference is written just before, so the reading drops the cached
        // argument domain first.
        tokens.append(DistributedNotificationCenter.default().addObserver(
            forName: Notification.Name("AppleInterfaceThemeChangedNotification"), object: nil, queue: .main
        ) { _ in
            UserDefaults.standard.removeVolatileDomain(forName: UserDefaults.argumentDomain)
            changed()
        })
        return tokens
        #else
        // A system appearance change reaches an app as a trait change (the
        // view reports it, ExactViewIOS) or, when made elsewhere, on return.
        let names = [UIAccessibility.reduceMotionStatusDidChangeNotification, UIAccessibility.reduceTransparencyStatusDidChangeNotification,
                     UIAccessibility.darkerSystemColorsStatusDidChangeNotification, UIApplication.didBecomeActiveNotification, agentChanged]
        return names.map { center.addObserver(forName: $0, object: nil, queue: .main) { _ in changed() } }
        #endif
    }
    static func forget(_ tokens: [NSObjectProtocol]) {
        #if os(macOS)
        tokens.forEach { token in
            center.removeObserver(token)
            DistributedNotificationCenter.default().removeObserver(token)
        }
        #else
        tokens.forEach(center.removeObserver)
        #endif
    }
}
