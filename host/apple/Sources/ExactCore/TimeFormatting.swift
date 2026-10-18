import Foundation
import CExact

// Foundation's date formatters, with the runner's reference date rather than Date.now.
enum TimeFormatting {
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
        case "relative", "relative-reservation":
            let formatter = RelativeDateTimeFormatter()
            formatter.locale = locale
            formatter.unitsStyle = .short
            formatter.dateTimeStyle = .numeric
            if name == "relative-reservation" {
                let sign = q.datetime < q.now ? -1 : 1
                result = formatter.localizedString(from: DateComponents(minute: sign * 59)) + ", " + formatter.localizedString(from: DateComponents(second: sign * 59))
            } else {
                let minutes = ((q.datetime - q.now) / 60000).rounded()
                if minutes == 0 { result = formatter.localizedString(from: DateComponents(minute: 0)) }
                else { result = formatter.localizedString(for: Date(timeIntervalSince1970: q.now / 1000 + minutes * 60), relativeTo: now) }
            }
        case "timer", "countdown":
            let seconds = name == "countdown" ? ceil(max(0, q.datetime - q.now) / 1000) : floor(abs(q.datetime - q.now) / 1000)
            let formatter = DateComponentsFormatter()
            formatter.calendar = Calendar(identifier: .gregorian)
            formatter.calendar?.locale = locale
            formatter.unitsStyle = .positional
            formatter.zeroFormattingBehavior = .pad
            formatter.allowedUnits = seconds >= 3600 ? [.hour, .minute, .second] : [.minute, .second]
            result = formatter.string(from: seconds) ?? ""
        default:
            let formatter = DateFormatter()
            formatter.locale = locale
            formatter.timeZone = TimeZone(identifier: string(q.time_zone, q.time_zone_len))
            let styles: [String: DateFormatter.Style] = ["short": .short, "medium": .medium, "long": .long, "full": .full]
            let parts = name.split(separator: "-", maxSplits: 1).map(String.init)
            guard parts.count == 2, let style = styles[parts[1]] else { return 0 }
            if parts[0] == "date" { formatter.dateStyle = style }
            else if parts[0] == "time" { formatter.timeStyle = style }
            else { return 0 }
            result = formatter.string(from: date)
        }
        let bytes = Array(result.utf8)
        if let out { for i in 0..<min(cap, bytes.count) { out[i] = bytes[i] } }
        return bytes.count
    }
}
