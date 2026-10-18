import Foundation

/// Facts known by the host before a plan's first layout. A headless session has
/// no display to query: its caller supplies the destination's scale and preferences.
public struct SessionFacts {
    /// Body size at the platform's default settings, in points. Hosts may supply
    /// the user's preferred size (including Dynamic Type) through `rootFontSize`.
    public static let defaultRootFontSize: CGFloat = {
        #if os(macOS)
        return 13
        #elseif os(tvOS)
        return 16
        #else
        return 17
        #endif
    }()
    public let rootFontSize: Double
    public let displayScale: Double
    /// `exact_set_preferences` bits: motion, transparency, contrast, appearance,
    /// pointer/hover, gamut, dynamic range, display mode and Bold Text.
    public let preferences: UInt32
    public let epochAtZero: Double
    public let utcOffset: Double
    public let locale: String
    public let timeZone: String
    public let seed: UInt64

    public init(displayScale: Double, preferences: UInt32,
                rootFontSize: Double = Double(defaultRootFontSize),
                date: Date = Date(), timeZone: TimeZone = .current,
                locale: String = Locale.preferredLanguages.first ?? Locale.current.identifier(.bcp47),
                seed: UInt64 = UInt64(UInt32.random(in: .min ... .max))) {
        self.rootFontSize = rootFontSize; self.displayScale = displayScale; self.preferences = preferences
        epochAtZero = date.timeIntervalSince1970 * 1000
        utcOffset = Double(timeZone.secondsFromGMT(for: date)) / 60
        self.locale = locale; self.timeZone = timeZone.identifier; self.seed = seed
    }

    package func prime(_ runtime: Runtime) {
        _ = runtime.setRootFontSize(rootFontSize)
        _ = runtime.setDisplayScale(displayScale)
        runtime.setLaunchTime(epochAtZero: epochAtZero, utcOffset: utcOffset)
        runtime.setLaunchPlace(locale: locale, timeZone: timeZone, seed: seed)
        _ = runtime.setPreferences(preferences)
    }
}
