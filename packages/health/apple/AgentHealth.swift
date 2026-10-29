// The agent's Health store (LLP 1133.026 §11): what @exact/health reads
// under the agent, where it makes no `HKHealthStore`. It is this checkout's
// `fixture/health.json` (a drive runs on this Mac or its simulators), or
// another file there the agent's `store` names: hourly samples per type and
// a list of workouts, from one fixture source.
//
// It answers as HealthKit answers this app (D5): every answer's `complete`
// is false; a type the person did not share (the runner's `shared` fact)
// answers what HealthKit answers for it, the samples the app wrote itself,
// which are none; a limited window (`since`) clamps every read. Buckets are
// calendar intervals in the session's zone (`EXACT_AGENT_TIME_ZONE`, UTC
// unless the drive names one), anchored at `start`.
import Foundation

final class AgentHealth {
    /// The fixtures, beside this file's directory in the checkout.
    private static let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("fixture")
    private static let hour = 3_600_000.0

    /// Each type's samples, `(start ms, value)`, ascending.
    private var samples: [String: [(Double, Double)]] = [:]
    private(set) var units: [String: String] = [:]
    private(set) var workouts: [[String: Any]] = []
    /// The fixture's today (`now`), past which no sample is seen.
    var now: Double?
    let zone = TimeZone(identifier: ProcessInfo.processInfo.environment["EXACT_AGENT_TIME_ZONE"] ?? "UTC") ?? TimeZone(identifier: "UTC")!

    init() { _ = load("health.json") }

    /// Reads fixture file `name`; false where there is none.
    func load(_ name: String) -> Bool {
        guard !name.contains("/"), let data = try? Data(contentsOf: Self.fixtures.appendingPathComponent(name)),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
        units = json["units"] as? [String: String] ?? [:]
        workouts = (json["workouts"] as? [[String: Any]] ?? []).sorted(by: Self.ordered)
        samples = [:]
        var utc = Calendar(identifier: .gregorian)
        utc.timeZone = TimeZone(identifier: "UTC")!
        for (type, days) in json["hours"] as? [String: [String: [Double]]] ?? [:] {
            var list: [(Double, Double)] = []
            for (day, hours) in days {
                let parts = day.split(separator: "-").compactMap { Int($0) }
                guard parts.count == 3, let midnight = utc.date(from: DateComponents(year: parts[0], month: parts[1], day: parts[2])) else { continue }
                let at = midnight.timeIntervalSince1970 * 1000
                list += hours.enumerated().map { (at + Double($0.offset) * Self.hour, $0.element) }
            }
            samples[type] = list.sorted { $0.0 < $1.0 }
        }
        return true
    }

    /// D8's order: ascending by `start`, then `id`.
    static func ordered(_ a: [String: Any], _ b: [String: Any]) -> Bool {
        let (sa, sb) = (a["start"] as? Double ?? 0, b["start"] as? Double ?? 0)
        return sa != sb ? sa < sb : (a["id"] as? String ?? "") < (b["id"] as? String ?? "")
    }

    /// The bucket edges from `start` to `end` by `interval`, at most 400.
    static func edges(_ start: Double, _ end: Double, _ interval: String, _ zone: TimeZone) -> [Double]? {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = zone
        let step: (Date, Int) -> Date? = { from, n in
            switch interval {
            case "PT1H": return from.addingTimeInterval(Double(n) * 3600)
            case "P1D": return calendar.date(byAdding: .day, value: n, to: from)
            case "P1W": return calendar.date(byAdding: .day, value: 7 * n, to: from)
            case "P1M": return calendar.date(byAdding: .month, value: n, to: from)
            default: return nil
            }
        }
        let from = Date(timeIntervalSince1970: start / 1000)
        var out = [start]
        while let last = out.last, last < end {
            guard out.count <= 400, let next = step(from, out.count) else { return nil }
            out.append(min(end, next.timeIntervalSince1970 * 1000))
        }
        return out
    }

    /// `type`'s buckets: `none` where no sample is seen, a sum from `since`.
    func buckets(_ type: String, _ edges: [Double], since: Double?, shared: Bool) -> [[String: Any]] {
        let list = samples[type] ?? []
        return zip(edges, edges.dropFirst()).map { start, end in
            let from = max(start, since ?? start), to = min(end, now ?? end)
            let seen = shared ? list.filter { $0.0 >= from && $0.0 < to } : []
            return ["start": start, "end": end, "value": seen.isEmpty ? NSNull() : seen.reduce(0) { $0 + $1.1 } as Any]
        }
    }

    /// The workouts starting in `[start, end)` and ending at or after `since`.
    func workouts(_ start: Double, _ end: Double, since: Double?) -> [[String: Any]] {
        workouts.filter {
            let s = $0["start"] as? Double ?? 0, e = $0["end"] as? Double ?? s
            return s >= start && s < end && e >= (since ?? -Double.infinity) && s < (now ?? .infinity)
        }
    }
}
