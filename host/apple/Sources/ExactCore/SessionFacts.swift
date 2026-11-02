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
    /// The Region's format with a forced hour cycle (`currentFormatLocale`): `exactTime().formatLocale`.
    public let formatLocale: String
    public let timeZone: String
    public let seed: UInt64

    public init(displayScale: Double, preferences: Preferences,
                rootFontSize: Double = Double(defaultRootFontSize),
                date: Date = Date(), clock: Double = 0, timeZone: TimeZone = .current,
                locale: String = Locale.preferredLanguages.first ?? Locale.current.identifier(.bcp47),
                formatLocale: String = SessionFacts.currentFormatLocale,
                seed: UInt64 = UInt64(UInt32.random(in: .min ... .max)),
                textResidencyBytes: Int = 2 * 1024 * 1024) {
        self.textResidencyBytes = max(0, textResidencyBytes)
        self.rootFontSize = rootFontSize; self.displayScale = displayScale; self.preferences = preferences
        epochAtZero = date.timeIntervalSince1970 * 1000 - clock
        utcOffset = Double(timeZone.secondsFromGMT(for: date)) / 60
        self.locale = locale; self.formatLocale = formatLocale; self.timeZone = timeZone.identifier; self.seed = seed
    }

    package func prime(_ runtime: Runtime) throws {
        guard seed <= UInt64(UInt32.max) else { throw HeadlessSession.Failure(description: "launch seed must be a whole number below 2^32") }
        func accept(_ batch: Batch?) throws {
            if let error = batch?.error, error != "not booted" { throw HeadlessSession.Failure(description: error) }
        }
        try accept(runtime.setRootFontSize(rootFontSize))
        try accept(runtime.setDisplayScale(displayScale))
        runtime.setLaunchTime(epochAtZero: epochAtZero, utcOffset: utcOffset)
        try accept(runtime.setLaunchPlace(locale: locale, timeZone: timeZone, seed: seed, formatLocale: formatLocale))
        try accept(runtime.setPreferences(preferences.rawValue))
    }
}

extension SessionFacts {
    /// `exactTime().formatLocale` off the agent: the Region's format
    /// (`Locale.current`; English in Korea is `en-KR`) with the hour cycle the
    /// user forces (System Settings › General › Date & Time › 24-hour time on
    /// the Mac, Settings › General › Date & Time › 24-Hour Time on iOS) as
    /// `-u-hc-h23` or `-u-hc-h12`. Foundation's formatters honour that
    /// setting, but `Locale.current.identifier` leaves it out. It is read from
    /// the two defaults the setting writes, as T3 Code's desktop app reads them
    /// beside Electron's `app.getSystemLocale()`. `Locale.current.hourCycle`
    /// answers the global setting too, but not a launch argument
    /// (`-AppleICUForce24HourTime YES`), which the defaults hear: a test sets
    /// the cycle for one process that way, though Foundation's own
    /// formatters in that process keep the global one.
    public static var currentFormatLocale: String {
        let defaults = UserDefaults.standard
        let forced: Locale.HourCycle? = defaults.bool(forKey: "AppleICUForce24HourTime") ? .zeroToTwentyThree
            : defaults.bool(forKey: "AppleICUForce12HourTime") ? .oneToTwelve : nil
        return formatLocale(.current, hourCycle: forced)
    }

    /// `locale` as BCP 47, with `hourCycle` as its `hc` keyword when one is
    /// given and the tag stays within the runner's 64 characters (a longer
    /// one would refuse the whole place).
    package static func formatLocale(_ locale: Locale, hourCycle: Locale.HourCycle?) -> String {
        let tag = locale.identifier(.bcp47)
        guard let hourCycle else { return tag }
        var components = Locale.Components(identifier: tag)
        components.hourCycle = hourCycle
        let cycled = Locale(components: components).identifier(.bcp47)
        return cycled.count <= 64 ? cycled : tag
    }
}
