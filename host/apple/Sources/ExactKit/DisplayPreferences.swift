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
    /// What an agent's `prefer` (or a test) set for Bold Text; off from
    /// launch under the agent (LLP 1069.007 D2).
    nonisolated(unsafe) static var agentBoldText: Bool? = ExactEnv.agentMode ? false : nil {
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
    /// Bold Text (LLP 1115.000 D12.3): the person's setting, which the
    /// kernel weights the system font by. The Mac has none.
    static var boldText: Bool {
        if let agentBoldText { return agentBoldText }
        #if os(macOS)
        return false
        #else
        return UIAccessibility.isBoldTextEnabled
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
    /// Encode this window's readings with the core's shared wire layout.
    static func bits(systemDark: Bool, view: PlatformView? = nil) -> UInt32 {
        #if os(macOS)
        let displayGamut = agentGamut ?? ((view?.window?.screen ?? NSScreen.main)?.canRepresent(.p3) == true ? "p3" : "srgb")
        #else
        let displayGamut = agentGamut ?? ((view?.window?.screen ?? UIScreen.main).traitCollection.displayGamut == .P3 ? "p3" : "srgb")
        #endif
        let high = view.map { DisplayRange.headroom($0) > 1 } ?? highDynamicRange
        return Preferences(reducedMotion: reducedMotion, reducedTransparency: reducedTransparency,
            contrast: contrast, dark: systemDark, gamut: displayGamut, highDynamicRange: high,
            displayMode: Preferences.DisplayMode(rawValue: displayMode(view)) ?? .standalone, boldText: boldText).rawValue
    }

    // Same order as runner/src/viewport.rs's DisplayMode (media bits 11–13).
    static let displayModes = ["standalone", "browser", "fullscreen", "minimal-ui", "window-controls-overlay", "picture-in-picture", "tabbed"]
    static func displayMode(_ view: PlatformView?) -> UInt32 {
        #if os(macOS)
        return view?.window?.styleMask.contains(.fullScreen) == true ? 2 : 0
        #else
        return 0
        #endif
    }

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
                     UIAccessibility.darkerSystemColorsStatusDidChangeNotification, UIAccessibility.boldTextStatusDidChangeNotification,
                     UIApplication.didBecomeActiveNotification, agentChanged]
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

extension TextEngine {
    /// Whether Bold Text weights a run in stack `family` (LLP 1115.000
    /// D12.3), as the kernel weights its runs: the setting on, and the
    /// stack the system's, a generic one or a list whose first member is a
    /// generic family (the catalog's first face, as a compiled or runtime
    /// list registered it). A Markdown piece's own weight takes the step its
    /// run's took.
    package func boldTextSteps(_ family: Int) -> Bool {
        guard DisplayPreferences.boldText else { return false }
        guard let faces = catalog[family] else { return family < 8 }
        return faces.first?.generic != nil
    }

    /// The kernel's step (`exact_kernel::bolder`), UIKit's as measured: 100
    /// to 200, 200 to 300, 300 to 400, 400 to 600, 500 to 700, 600 to 800,
    /// 700 and on to 900; between two hundreds, between their steps.
    package static func bolder(_ weight: Int) -> Int {
        let steps = [100, 200, 300, 400, 600, 700, 800, 900, 900, 900, 1000]
        let w = min(max(weight, 1), 1000), at = w / 100, rest = w % 100
        if at >= 10 { return 1000 }
        return steps[at] + (steps[at + 1] - steps[at]) * rest / 100
    }
}
