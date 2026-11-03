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
    /// setting, but `Locale.current.identifier` leaves it out. It is the
    /// hour of `DateFormatter.dateFormat(fromTemplate: "j")`, the locale's
    /// preferred hour, for `Locale.current` against the same identifier
    /// without the person's settings: a cycle only where the two differ.
    public static var currentFormatLocale: String {
        let hours = { (locale: Locale) in DateFormatter.dateFormat(fromTemplate: "j", options: 0, locale: locale) ?? "" }
        return formatLocale(.current, hourCycle: forcedHourCycle(hourPatternOverride ?? hours(.current), regional: hours(Locale(identifier: Locale.current.identifier))))
    }
    /// What a test stands in for `Locale.current`'s preferred hour, which no
    /// process can set for itself.
    nonisolated(unsafe) package static var hourPatternOverride: String?

    /// The cycle `pattern` (a date format's preferred hour) forces where it
    /// differs from the Region's own `regional` one: 12-hour for `h` or `K`
    /// outside a quoted literal (`fr-CA`'s `HH 'h'` is 24-hour), else 24.
    package static func forcedHourCycle(_ pattern: String, regional: String) -> Locale.HourCycle? {
        let twelve = { (pattern: String) in pattern.split(separator: "'", omittingEmptySubsequences: false).enumerated().contains { $0.offset % 2 == 0 && $0.element.contains { "hK".contains($0) } } }
        guard !pattern.isEmpty, twelve(pattern) != twelve(regional) else { return nil }
        return twelve(pattern) ? .oneToTwelve : .zeroToTwentyThree
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
