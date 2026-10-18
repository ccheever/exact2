import Foundation
import CExact

// Formatter instances belong to the runner's owner thread, as the callback does.
enum TimeFormatting {
    private static var relative: [String: RelativeDateTimeFormatter] = [:]
    private static var durations: [String: DateComponentsFormatter] = [:]
    private static var dates: [String: DateFormatter] = [:]

    private static func relativeFormatter(_ locale: Locale, named: Bool, wide: Bool = false) -> RelativeDateTimeFormatter {
        let key = "\(locale.identifier)/\(named)/\(wide)"
        if let value = relative[key] { return value }
        let value = RelativeDateTimeFormatter()
        value.locale = locale
        value.unitsStyle = wide ? .full : .short
        value.dateTimeStyle = named ? .named : .numeric
        relative[key] = value
        return value
    }

    private static func componentsFormatter(_ locale: Locale, kind: String) -> DateComponentsFormatter {
        let key = "\(kind)/\(locale.identifier)"
        if let value = durations[key] { return value }
        let value = DateComponentsFormatter()
        value.calendar = Calendar(identifier: .gregorian)
        value.calendar?.locale = locale
        value.unitsStyle = kind == "sample" ? .abbreviated : kind == "unit" ? .full : .positional
        // Keep minute/second zeros; .default drops the entire zero minute.
        value.zeroFormattingBehavior = kind == "countdown" ? .pad : kind == "timer" ? [] : .default
        value.maximumUnitCount = kind == "sample" ? 2 : 0
        durations[key] = value
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
        let result: String
        switch name {
        case "relative-reservation":
            let sample = componentsFormatter(locale, kind: "sample")
            sample.allowedUnits = [.minute, .second]
            let unit = componentsFormatter(locale, kind: "unit")
            unit.allowedUnits = [.minute]
            guard let phrase = sample.string(from: 3599), let minute = unit.string(from: 3540) else { return 0 }
            let wrapper = relativeFormatter(locale, named: false, wide: true)
                .localizedString(from: DateComponents(minute: q.now > q.datetime ? -59 : 59))
            guard let range = wrapper.range(of: minute) else { return 0 }
            result = wrapper.replacingCharacters(in: range, with: phrase)
        case "relative":
            let n = Int(q.relative_value)
            let formatter = relativeFormatter(locale, named: n == 0)
            if n == 0 {
                result = formatter.localizedString(for: date, relativeTo: date)
            } else {
                let units: [Calendar.Component] = [.minute, .hour, .day, .month, .year]
                guard Int(q.relative_unit) < units.count else { return 0 }
                var components = DateComponents()
                components.setValue(n, for: units[Int(q.relative_unit)])
                result = formatter.localizedString(from: components)
            }
        case "timer", "countdown":
            let seconds = name == "countdown" ? ceil(max(0, q.datetime - q.now) / 1000) : floor(abs(q.datetime - q.now) / 1000)
            let formatter = componentsFormatter(locale, kind: name)
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
