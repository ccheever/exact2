import Foundation

/// Shared reader text for the native cards and detail panels; never called from a frame hook.
enum FrameHealthReadout {
    static func compact(_ health: FrameHealth) -> String {
        guard health.enabled else { return health.unavailableReason ?? "Collection unavailable" }
        let metrics = health.rollingMetrics()
        let callbacks = metrics["callbacks"] as? [String: Any] ?? [:]
        let apply = metrics["apply"] as? [String: Any] ?? [:]
        let period = UInt64(metrics["scheduledPeriodNs"] as? String ?? "0") ?? 0
        let inactive = health.virtual || metrics["status"] as? String != "observing"
        let rate = inactive ? (health.virtual ? "Virtual" : "Idle") : period == 0 ? "Cadence unknown" : (callbacks["perSecond"] as? Double).map { String(format: "%.0f/s", min($0, 9999)) } ?? "Collecting…"
        let scheduled = period == 0 ? "—" : String(format: "%.0f", 1e9 / Double(period))
        let requested = UInt64(metrics["requestedPeriodNs"] as? String ?? "0") ?? 0
        let request = requested == 0 ? "—" : String(format: "%.0f", 1e9 / Double(requested))
        let p99 = bounds(callbacks["p99BoundsNs"])
        return "Callbacks: \(rate)\nSchedule/request: \(scheduled)/\(request) Hz\nP99 gap: \(p99)\nLate: cb \(count(callbacks["synchronousTargetOverruns"])) · apply \(count(apply["targetOverruns"]))"
    }

    static func detail(_ health: FrameHealth) -> String {
        let m = health.rollingMetrics(), c = m["callbacks"] as? [String: Any] ?? [:], a = m["apply"] as? [String: Any] ?? [:]
        let w = m["window"] as? [String: Any] ?? [:]
        return """
        \(health.source)
        Status: \(m["status"] ?? "unknown") · unqualified proxy
        Generation: \(w["generation"] ?? "unknown")
        Observed callbacks: \(c["count"] ?? "0")
        Requested period: \(m["requestedPeriodNs"] ?? "0") ns (0: default/unknown)
        \(compact(health))
        Callback gap p50: \(bounds(c["p50BoundsNs"]))
        Callback gap p95: \(bounds(c["p95BoundsNs"]))
        Apply p50: \(bounds(a["p50BoundsNs"]))
        Apply p95: \(bounds(a["p95BoundsNs"]))
        Apply p99: \(bounds(a["p99BoundsNs"]))
        Targeted applies: \(a["withTarget"] ?? "0")
        Interval finalized through: \(w["intervalThroughNs"] ?? "0") ns
        Apply finalized through: \(w["applyThroughNs"] ?? "0") ns
        Native scrolling: \(health.nativeCoverage)
        Quantiles are half-open bounds over observed work.
        Presentation, GPU execution and input latency are unobserved.
        """
    }

    static func bounds(_ value: Any?) -> String {
        guard let pair = value as? [Any], pair.count == 2, let lower = pair[0] as? NSNumber else { return "—" }
        let upper = (pair[1] as? NSNumber).map { String(format: "%.1f", $0.doubleValue / 1e6) } ?? "∞"
        return String(format: "%.1f–%@ ms", lower.doubleValue / 1e6, upper)
    }
    private static func count(_ value: Any?) -> String {
        guard let text = value as? String, let count = UInt64(text) else { return "—" }
        return count < 1_000_000 ? text : "999k+"
    }
}
