import Foundation

/// Shared reader text for the native cards and detail panels; never called from a frame hook.
enum FrameHealthReadout {
    static func compact(_ health: FrameHealth) -> String {
        guard health.enabled else { return health.unavailableReason ?? "Collection unavailable" }
        let metrics = health.rollingMetrics()
        let callbacks = metrics["callbacks"] as? [String: Any] ?? [:]
        let period = UInt64(metrics["scheduledPeriodNs"] as? String ?? "0") ?? 0
        let inactive = health.virtual || metrics["status"] as? String != "observing"
        let rate = inactive ? (health.virtual ? "Virtual" : "Idle") : period == 0 ? "Unknown" : (callbacks["perSecond"] as? Double).map { String(format: "%.1f", min($0, 9999)) } ?? "Collecting…"
        return "FPS avg: \(rate) · callbacks\nPacing p50: \(bounds(callbacks["p50BoundsNs"]))\nPacing p95: \(bounds(callbacks["p95BoundsNs"]))\nPacing p99: \(bounds(callbacks["p99BoundsNs"]))"
    }

    static func detail(_ health: FrameHealth) -> String {
        let m = health.rollingMetrics(), c = m["callbacks"] as? [String: Any] ?? [:], a = m["apply"] as? [String: Any] ?? [:]
        let w = m["window"] as? [String: Any] ?? [:]
        return """
        \(health.source)
        Status: \(m["status"] ?? "unknown") · unqualified proxy
        Generation: \(w["generation"] ?? "unknown")
        Observed callbacks: \(c["count"] ?? "0")
        Scheduled period: \(m["scheduledPeriodNs"] ?? "0") ns (0: unknown)
        Requested period: \(m["requestedPeriodNs"] ?? "0") ns (0: default/unknown)
        \(compact(health))
        Callback overruns: \(c["synchronousTargetOverruns"] ?? "0") · apply overruns: \(a["targetOverruns"] ?? "0")
        Apply p50: \(bounds(a["p50BoundsNs"]))
        Apply p95: \(bounds(a["p95BoundsNs"]))
        Apply p99: \(bounds(a["p99BoundsNs"]))
        Targeted applies: \(a["withTarget"] ?? "0")
        Interval finalized through: \(w["intervalThroughNs"] ?? "0") ns
        Apply finalized through: \(w["applyThroughNs"] ?? "0") ns
        Native scrolling: \(health.nativeCoverage)
        Percentile bounds include the lower value and exclude the upper value.
        Display presentation, GPU execution and input latency are unmeasured.
        """
    }

    static func bounds(_ value: Any?) -> String {
        guard let pair = value as? [Any], pair.count == 2, let lower = pair[0] as? NSNumber else { return "—" }
        let upper = (pair[1] as? NSNumber).map { String(format: "%.1f", $0.doubleValue / 1e6) } ?? "∞"
        return String(format: "%.1f–%@ ms", lower.doubleValue / 1e6, upper)
    }

}
