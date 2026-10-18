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
        #elseif os(watchOS)
        return 16 // HIG Typography: watchOS Body at the default size.
        #else // iOS and visionOS
        return 17
        #endif
    }()
    public let rootFontSize: Double
    public let displayScale: Double
    public let preferences: Preferences
    /// Cold shaped text only; 2 MiB leaves room for the runner and archive under
    /// a widget's roughly 30 MiB ceiling. This is not a whole-process limit.
    public let textResidencyBytes: Int
    public let epochAtZero: Double
    public let utcOffset: Double
    public let locale: String
    public let timeZone: String
    public let seed: UInt64

    public init(displayScale: Double, preferences: Preferences,
                rootFontSize: Double = Double(defaultRootFontSize),
                date: Date = Date(), clock: Double = 0, timeZone: TimeZone = .current,
                locale: String = Locale.preferredLanguages.first ?? Locale.current.identifier(.bcp47),
                seed: UInt64 = UInt64(UInt32.random(in: .min ... .max)),
                textResidencyBytes: Int = 2 * 1024 * 1024) {
        self.textResidencyBytes = max(0, textResidencyBytes)
        self.rootFontSize = rootFontSize; self.displayScale = displayScale; self.preferences = preferences
        epochAtZero = date.timeIntervalSince1970 * 1000 - clock
        utcOffset = Double(timeZone.secondsFromGMT(for: date)) / 60
        self.locale = locale; self.timeZone = timeZone.identifier; self.seed = seed
    }

    package func prime(_ runtime: Runtime) throws {
        guard seed <= UInt64(UInt32.max) else { throw HeadlessSession.Failure(description: "launch seed must be a whole number below 2^32") }
        func accept(_ batch: Batch?) throws {
            if let error = batch?.error, error != "not booted" { throw HeadlessSession.Failure(description: error) }
        }
        try accept(runtime.setRootFontSize(rootFontSize))
        try accept(runtime.setDisplayScale(displayScale))
        runtime.setLaunchTime(epochAtZero: epochAtZero, utcOffset: utcOffset)
        try accept(runtime.setLaunchPlace(locale: locale, timeZone: timeZone, seed: seed))
        try accept(runtime.setPreferences(preferences.rawValue))
    }
}
