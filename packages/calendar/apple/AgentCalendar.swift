// The agent's calendars (LLP 1133.018 §10): what @exact/calendar reads and
// writes under the agent, where it asks EventKit for nothing. It is this
// checkout's `fixture/calendar.json` (a drive runs on this Mac or its
// simulators), or another file there the agent's `store` names, changed for
// the session by the app's writes and the editor's saves. A recurring series
// is listed one occurrence at a time, each with its `recurrenceId`, so
// nothing here expands a rule.
//
// Time follows JSCalendar (RFC 8984): an event with a `timeZone` is fixed in
// it; one without, and an all-day one, is placed in the session's zone
// (`EXACT_AGENT_TIME_ZONE`, UTC unless the drive names one).
import Foundation

final class AgentCalendar {
    /// The fixtures, beside this file's directory in the checkout.
    private static let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("fixture")
    /// EventKit's predicate spans at most four years (D2).
    static let longest = 4 * 365.25 * 86_400_000.0

    private(set) var calendars: [[String: Any]] = []
    private(set) var events: [[String: Any]] = []
    private var made = 0
    var access = "none"
    let zone = TimeZone(identifier: ProcessInfo.processInfo.environment["EXACT_AGENT_TIME_ZONE"] ?? "UTC") ?? TimeZone(identifier: "UTC")!

    init() { _ = load("calendar.json") }

    /// Reads fixture file `name`; false where there is none.
    func load(_ name: String) -> Bool {
        guard !name.contains("/"), let data = try? Data(contentsOf: Self.fixtures.appendingPathComponent(name)),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
        calendars = json["calendars"] as? [[String: Any]] ?? []
        events = json["events"] as? [[String: Any]] ?? []
        return true
    }

    var defaultCalendar: String? { calendars.first { $0["isDefault"] as? Bool == true }?["id"] as? String }

    func calendar(_ id: String) -> [String: Any]? { calendars.first { $0["id"] as? String == id } }

    /// Whether `id` names a recurring series: its occurrences carry a `recurrenceId`.
    func isSeries(_ id: String) -> Bool {
        events.contains { $0["id"] as? String == id && $0["recurrenceId"] is String }
    }

    // MARK: time

    /// `2026-10-23T09:00:00` as its parts.
    static func parts(_ local: String) -> DateComponents? {
        let fields = local.split(whereSeparator: { $0 == "-" || $0 == "T" || $0 == ":" }).map { Int($0) }
        guard (3...6).contains(fields.count), fields.allSatisfy({ $0 != nil }) else { return nil }
        var c = DateComponents()
        c.year = fields[0]; c.month = fields[1]; c.day = fields[2]
        c.hour = fields.count > 3 ? fields[3] : 0; c.minute = fields.count > 4 ? fields[4] : 0; c.second = fields.count > 5 ? fields[5] : 0
        return c
    }

    /// An ISO 8601 duration (`P1D`, `PT1H30M`, `P1W`) as nominal days and exact seconds.
    static func duration(_ text: String) -> (days: Int, seconds: Int)? {
        guard text.hasPrefix("P") else { return nil }
        var days = 0, seconds = 0, number = "", inTime = false
        for ch in text.dropFirst() {
            if ch.isNumber { number.append(ch); continue }
            if ch == "T" { inTime = true; continue }
            guard let n = Int(number) else { return nil }
            number = ""
            switch (ch, inTime) {
            case ("W", false): days += 7 * n
            case ("D", false): days += n
            case ("H", true): seconds += 3600 * n
            case ("M", true): seconds += 60 * n
            case ("S", true): seconds += n
            default: return nil
            }
        }
        return number.isEmpty ? (days, seconds) : nil
    }

    /// The instants an event spans, ms since the epoch; nil for one whose start or duration is not well formed.
    func span(_ event: [String: Any]) -> (Double, Double)? {
        guard let start = event["start"] as? String, var c = Self.parts(start),
              let d = Self.duration(event["duration"] as? String ?? "PT0S") else { return nil }
        let allDay = event["showWithoutTime"] as? Bool == true
        if allDay { c.hour = 0; c.minute = 0; c.second = 0 }
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = (allDay ? nil : (event["timeZone"] as? String).flatMap(TimeZone.init(identifier:))) ?? zone
        guard let from = calendar.date(from: c), let to = calendar.date(byAdding: .day, value: d.days, to: from) else { return nil }
        return (from.timeIntervalSince1970 * 1000, (to.timeIntervalSince1970 + Double(d.seconds)) * 1000)
    }

    /// The event as answered: every field, its instants beside it.
    func answered(_ event: [String: Any]) -> [String: Any] {
        var out = event
        for field in ["id", "uid", "calendarId", "description", "timeZone", "location", "url", "recurrenceId", "apple"] where out[field] == nil {
            out[field] = NSNull()
        }
        if let (start, end) = span(event) { out["utcStart"] = start; out["utcEnd"] = end }
        return out
    }

    // MARK: writes

    /// A new or changed event, normalised as the store keeps it.
    func stored(_ event: [String: Any], id: String?, calendar: String) -> [String: Any] {
        var out = event
        if let id { out["id"] = id } else { made += 1; out["id"] = "n\(made)" }
        out["uid"] = (event["uid"] as? String) ?? "\(out["id"]!)@agent.exact"
        out["calendarId"] = calendar
        out["showWithoutTime"] = event["showWithoutTime"] as? Bool ?? false
        out["status"] = event["status"] as? String ?? "confirmed"
        out["freeBusyStatus"] = event["freeBusyStatus"] as? String ?? "busy"
        out["recurrenceId"] = NSNull()
        out["utcStart"] = nil; out["utcEnd"] = nil
        return out
    }

    func put(_ event: [String: Any]) {
        events.removeAll { $0["id"] as? String == event["id"] as? String }
        events.append(event)
    }

    func remove(_ id: String) -> Bool {
        let before = events.count
        events.removeAll { $0["id"] as? String == id }
        return events.count < before
    }
}
