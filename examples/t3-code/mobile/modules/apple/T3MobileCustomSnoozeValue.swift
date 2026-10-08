// @ref llp/1107.004-home-projection.decision.md#custom-snooze-input
// Pinned threadSettled.ts resolveCustomSnooze; native input validation only.
import Foundation

enum T3CustomSnoozeValue {
    static func resolve(mode: String, date: String, time: String, amount: Int, unit: String,
                        now: Date, zone: TimeZone) -> Date? {
        let wake: Date
        if mode == "duration" {
            guard (1...99).contains(amount), let seconds = ["minutes": 60.0, "hours": 3600, "days": 86400][unit] else { return nil }
            wake = now.addingTimeInterval(Double(amount) * seconds)
        } else {
            guard mode == "date", date.range(of: "^\\d{4}-\\d{2}-\\d{2}$", options: .regularExpression) != nil,
                  time.range(of: "^\\d{2}:\\d{2}$", options: .regularExpression) != nil else { return nil }
            let d = date.split(separator: "-").compactMap { Int($0) }, t = time.split(separator: ":").compactMap { Int($0) }
            var calendar = Calendar(identifier: .gregorian); calendar.timeZone = zone
            let components = DateComponents(year: d[0], month: d[1], day: d[2], hour: t[0], minute: t[1], second: 0)
            guard let provisional = calendar.date(from: components) else { return nil }
            let seen = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: provisional)
            guard seen.year == d[0], seen.month == d[1], seen.day == d[2], seen.hour == t[0], seen.minute == t[1], seen.second == 0,
                  let first = calendar.nextDate(after: calendar.startOfDay(for: provisional).addingTimeInterval(-1), matching: components,
                                                matchingPolicy: .strict, repeatedTimePolicy: .first, direction: .forward) else { return nil }
            wake = first
        }
        return wake.timeIntervalSince1970.isFinite && wake > now ? wake : nil
    }
    static func local(_ value: Date, zone: TimeZone) -> (date: String, time: String) {
        var calendar = Calendar(identifier: .gregorian); calendar.timeZone = zone
        let c = calendar.dateComponents([.year, .month, .day, .hour, .minute], from: value)
        return (String(format: "%04d-%02d-%02d", c.year ?? 0, c.month ?? 0, c.day ?? 0),
                String(format: "%02d:%02d", c.hour ?? 0, c.minute ?? 0))
    }
    static func milliseconds(_ value: Date) -> Date { Date(timeIntervalSince1970: floor(value.timeIntervalSince1970 * 1000) / 1000) }
    static func iso(_ value: Date) -> String {
        let format = ISO8601DateFormatter(); format.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return format.string(from: value)
    }
}

struct T3CustomSnoozeContext: Decodable, Equatable {
    let identity: String, requestRoute: String, environmentId: String, threadId: String, origin: String
    let generation: Int, connected: Bool, homeVisible: Bool, sidebarVisible: Bool
    let swipeSnoozable: Bool
    let swipeResetKey: String
    let items: [T3HomeMenuConfiguration.Item]
    func eligible(invocation: String) -> Bool {
        guard connected else { return false }
        if invocation == "swipe" { return swipeSnoozable }
        return invocation == "menu" && items.contains { $0.id == "snooze:custom" && !$0.disabled }
    }
    func sameOwner(as other: Self, invocation: String) -> Bool {
        identity == other.identity && requestRoute == other.requestRoute && environmentId == other.environmentId
            && threadId == other.threadId && origin == other.origin && generation == other.generation
            && (invocation != "swipe" || swipeResetKey == other.swipeResetKey)
    }
    func matches(_ request: [String: Any]) -> Bool {
        requestRoute == request["requestRoute"] as? String && environmentId == request["environmentId"] as? String
            && threadId == request["threadId"] as? String && origin == request["origin"] as? String && generation == request["generation"] as? Int
    }
}
