import Foundation

/// Reader text for the native detail panels; never called from a frame hook.
enum FrameHealthReadout {
    static func detail(_ health: FrameHealth) -> String {
        guard let m = health.rollingMetrics() else { return "\(health.source)\n\(FrameHealthReaders.full)." }
        let f = m["frames"] as? [String: Any] ?? [:], w = m["work"] as? [String: Any] ?? [:]
        let window = m["window"] as? [String: Any] ?? [:]
        let rate = (f["perSecond"] as? Double).map { String(format: "%.1f FPS", min($0, 9999)) } ?? "—"
        // Age of the newest callback at the read; unknown before the first
        let age = (m["observationAgeNs"] as? String).flatMap(Double.init).map { String(format: "%.1f ms", $0 / 1e6) } ?? "unknown"
        return """
        \(health.source)
        Status: \(m["status"] ?? "unknown") · observation age: \(age) · unqualified proxy
        \(results(m))
        Generation: \(window["generation"] ?? "unknown")
        Callbacks: \(f["callbacks"] ?? "0") · held: \(f["held"] ?? "0")
        Frame rate: \(rate) over \(f["intervals"] ?? "0") intervals
        Missed frames: \(f["missedFrames"] ?? "0") of \(f["requestedSlots"] ?? "0") requested slots
        Scheduled period: \(m["scheduledPeriodNs"] ?? "0") ns (0: unknown)
        Requested period: \(m["requestedPeriodNs"] ?? "0") ns (0: default/unknown)
        Frame time median: \(bounds(f["medianBoundsNs"])) · p99: \(bounds(f["p99BoundsNs"]))
        Pacing changes: \(f["pacingChanges"] ?? "0") · schedule changes: \(f["cadenceChanges"] ?? "0")
        Work records: \(w["records"] ?? "0") · late: \(w["late"] ?? "0")
        Work median: \(bounds(w["medianBoundsNs"])) · p99: \(bounds(w["p99BoundsNs"]))
        Intervals finalized through: \(window["intervalThroughNs"] ?? "0") ns
        Work finalized through: \(window["workThroughNs"] ?? "0") ns
        Native scrolling: \(health.nativeCoverage)
        Bounds include the lower value and exclude the upper value.
        Presentation, Core Animation commits, GPU execution and input latency are unmeasured.
        """
    }

    /// Each scoped result with its reason, as `perf frames health` reports it.
    static func results(_ metrics: [String: Any]) -> String {
        let assessments = metrics["assessments"] as? [String: Any] ?? [:]
        return [("Cadence", "cadence"), ("Target rate", "target"), ("Content latency", "contentLatency")].map { label, key in
            let result = assessments[key] as? [String: Any] ?? [:]
            var line = "\(label): \(result["result"] as? String ?? "unknown") (\(result["reason"] as? String ?? "unknown"))"
            if let p = result["predicates"] as? [String: Any] {
                line += " · " + p.keys.sorted().map { "\($0) \(p[$0] ?? "0")" }.joined(separator: " · ")
            }
            if let context = result["context"] as? [String], !context.isEmpty { line += " · \(context.joined(separator: ", "))" }
            return line
        }.joined(separator: "\n")
    }

    static func bounds(_ value: Any?) -> String {
        guard let pair = value as? [Any], pair.count == 2, let lower = pair[0] as? NSNumber else { return "—" }
        let upper = (pair[1] as? NSNumber).map { String(format: "%.1f", $0.doubleValue / 1e6) } ?? "∞"
        return String(format: "%.1f–%@ ms", lower.doubleValue / 1e6, upper)
    }
}
