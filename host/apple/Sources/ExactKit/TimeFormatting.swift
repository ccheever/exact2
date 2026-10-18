import Foundation
import CExact

// Formatter instances belong to the runner's owner thread, as the callback does.
enum TimeFormatting {
    private static var relative: [String: RelativeDateTimeFormatter] = [:]
    private static var durations: [String: DateComponentsFormatter] = [:]
    private static var dates: [String: DateFormatter] = [:]

    private static func relativeFormatter(_ locale: Locale, named: Bool) -> RelativeDateTimeFormatter {
        let key = "\(locale.identifier)/\(named)"
        if let value = relative[key] { return value }
        let value = RelativeDateTimeFormatter()
        value.locale = locale
        value.unitsStyle = .short
        value.dateTimeStyle = named ? .named : .numeric
        relative[key] = value
        return value
    }

    static let format: ExactTimeFormatFn = { _, pointer, out, cap in
        guard let q = pointer?.pointee else { return 0 }
        func string(_ p: UnsafePointer<UInt8>?, _ n: Int) -> String {
            guard let p else { return "" }
            return String(decoding: UnsafeBufferPointer(start: p, count: n), as: UTF8.self)
        }
        let name = string(q.format, q.format_len)
        let locale = Locale(identifier: string(q.locale, q.locale_len))
        let date = Date(timeIntervalSince1970: q.datetime / 1000)
        let now = Date(timeIntervalSince1970: q.now / 1000)
        let result: String
        switch name {
        case "relative-reservation":
            // RelativeDateTimeFormatter reduces components to one unit. The
            // kernel supplies its two-unit phrase for width measurement.
            return 0
        case "relative":
            let minutes = ((q.datetime - q.now) / 60000).rounded()
            let formatter = relativeFormatter(locale, named: minutes == 0)
            result = minutes == 0 ? formatter.localizedString(from: DateComponents(second: 0))
                : formatter.localizedString(for: Date(timeIntervalSince1970: q.now / 1000 + minutes * 60), relativeTo: now)
        case "timer", "countdown":
            let seconds = name == "countdown" ? ceil(max(0, q.datetime - q.now) / 1000) : floor(abs(q.datetime - q.now) / 1000)
            let formatter = durations[locale.identifier] ?? {
                let value = DateComponentsFormatter()
                value.calendar = Calendar(identifier: .gregorian)
                value.calendar?.locale = locale
                value.unitsStyle = .positional
                value.zeroFormattingBehavior = .pad
                durations[locale.identifier] = value
                return value
            }()
            formatter.allowedUnits = seconds >= 3600 ? [.hour, .minute, .second] : [.minute, .second]
            result = formatter.string(from: seconds) ?? ""
        default:
            let styles: [String: DateFormatter.Style] = ["short": .short, "medium": .medium, "long": .long, "full": .full]
            let parts = name.split(separator: "-", maxSplits: 1).map(String.init)
            guard parts.count == 2, let style = styles[parts[1]], ["date", "time"].contains(parts[0]) else { return 0 }
            let key = "\(name)/\(locale.identifier)"
            let formatter = dates[key] ?? {
                let value = DateFormatter()
                value.locale = locale
                if parts[0] == "date" { value.dateStyle = style } else { value.timeStyle = style }
                dates[key] = value
                return value
            }()
            formatter.timeZone = TimeZone(identifier: string(q.time_zone, q.time_zone_len))
            result = formatter.string(from: date)
        }
        let bytes = Array(result.utf8)
        if let out { for i in 0..<min(cap, bytes.count) { out[i] = bytes[i] } }
        return bytes.count
    }
}
