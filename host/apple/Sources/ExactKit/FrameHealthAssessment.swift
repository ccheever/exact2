import Foundation

/// LLP 1079.000 §7.3's v1 predicates over one window's deltas. Each result is scoped to the
/// reply's source and its evidence level; missing evidence never yields a pass, and callback
/// evidence yields at most `proxy-pass`.
enum FrameHealthAssessment {
    static let policy = "exact-frame-health-policy/2"
    /// Whether a frame finishing after `targetTimestamp` is late. The iOS 27.1 simulator dispatched
    /// 119 of 120 display-link callbacks about 0.5 ms after their own `targetTimestamp`, so there
    /// the deadline carries no information and lateness is unknown (LLP 1079.000 §4.6).
    static var deadlinesObserved: Bool = {
        #if targetEnvironment(simulator)
        return false
        #else
        return true
        #endif
    }()

    struct Window {
        let callbacks, intervals, worked, late, unclassified: UInt64
        let requestedSlots, missedFrames, restrictedIntervals: UInt64
        /// Scheduled slots frame intervals skipped: refreshes beyond each interval's first
        let skippedRefreshes: UInt64
        /// The most recent platform-predicted interval; zero when the source has no schedule.
        let scheduledPeriodNs: UInt64
        /// Whether the source runs at the read: a window without callbacks is idle only when it does not
        let running: Bool
        /// Frames whose work is still in flight past their deadline, so certainly late
        let overdue: UInt64
        /// Callbacks rejected because their timestamp did not advance
        let outOfOrder: UInt64
        /// The source runs but has sent no callback for two publication intervals
        let stale: Bool

        /// Read a delta laid out as `FrameHealthMetrics.Sample.values`.
        init(_ delta: [UInt64], scheduledPeriodNs: UInt64, running: Bool = false, overdue: UInt64 = 0, stale: Bool = false) {
            callbacks = delta[0]; intervals = delta[2]; worked = delta[6]; late = delta[7]
            requestedSlots = delta[11]; missedFrames = delta[12]; restrictedIntervals = delta[13]; skippedRefreshes = delta[14]
            unclassified = delta[15]; outOfOrder = delta[16]
            self.stale = stale
            self.scheduledPeriodNs = scheduledPeriodNs
            self.running = running
            self.overdue = overdue
        }
    }

    static func reply(_ w: Window, health: FrameHealth) -> [String: Any] {
        let blocked = health.virtual ? "virtual-clock" : health.enabled ? nil : health.unavailableCode ?? "collection-disabled"
        let noSchedule = w.scheduledPeriodNs == 0 ? "schedule-unknown" : "no-frame-intervals"
        // Cadence compares frames with the platform's schedule
        let cadence: (String, String) =
            if let blocked { ("insufficient-evidence", blocked) }
            // An observed failure stands even where the rest of the window is unobserved.
            else if w.skippedRefreshes > 0 { ("fail", "skipped-refreshes") }
            else if deadlinesObserved && (w.late > 0 || w.overdue > 0) { ("fail", "late-work") }
            else if w.callbacks == 0 { w.running ? ("insufficient-evidence", "no-callbacks") : ("not-applicable", "idle") }
            else if w.stale { ("insufficient-evidence", "stale") }
            else if w.outOfOrder > 0 { ("insufficient-evidence", "out-of-order") }
            else if w.intervals == 0 { ("insufficient-evidence", noSchedule) }
            // An interval across a schedule change has no single period, so no refresh count; it is
            // counted where it finalizes, whichever window saw the change
            else if w.unclassified > 0 { ("insufficient-evidence", "schedule-changed") }
            else if !deadlinesObserved { ("insufficient-evidence", "simulator-deadlines") }
            else { ("proxy-pass", "presentation-unobserved") }
        // Target delivery compares frames with the request. A schedule slower than the request is
        // restricted whatever the cause; causes Exact cannot attribute stay unknown, and conditions
        // known at the read are context only.
        // An observed miss stands even in a window that saw no callback, as when a run's end
        // finalizes a held callback's slot
        let target: (String, String) =
            if let blocked { ("insufficient-evidence", blocked) }
            else if w.restrictedIntervals > 0 { ("restricted", "unknown") }
            else if w.missedFrames > 0 { ("fail", "missed-frames") }
            else if w.callbacks == 0 { w.running ? ("insufficient-evidence", "no-callbacks") : ("not-applicable", "idle") }
            else if w.intervals == 0 { ("insufficient-evidence", noSchedule) }
            else if w.requestedSlots == 0 { ("not-applicable", "no-rate-request") }
            else { ("insufficient-evidence", "presentation-unobserved") }
        let latency: (String, String) =
            if let blocked { ("insufficient-evidence", blocked) }
            else if w.callbacks == 0 && w.worked == 0 { ("not-applicable", "idle") }
            else { ("insufficient-evidence", "content-lateness-unobserved") }
        // Lateness is unknown where deadlines carry no information, not zero
        let late: Any = deadlinesObserved ? String(w.late) : NSNull(), overdue: Any = deadlinesObserved ? String(w.overdue) : NSNull()
        let cadencePredicates: [String: Any] = ["frameIntervals": String(w.intervals), "skippedRefreshes": String(w.skippedRefreshes),
            "lateFrames": late, "overdueFrames": overdue, "unclassifiedIntervals": String(w.unclassified)]
        let targetPredicates: [String: Any] = ["requestedSlots": String(w.requestedSlots), "missedFrames": String(w.missedFrames),
            "restrictedIntervals": String(w.restrictedIntervals)]
        let cadenceReply: [String: Any] = ["result": cadence.0, "reason": cadence.1, "evidence": "callback-and-owner-work-proxies",
            "scope": "frames", "predicates": cadencePredicates]
        let targetReply: [String: Any] = ["result": target.0, "reason": target.1, "evidence": "callback-proxy", "scope": "requested-rate",
            "predicates": targetPredicates, "context": context()]
        let latencyReply: [String: Any] = ["result": latency.0, "reason": latency.1, "evidence": "unavailable", "scope": "content"]
        return ["policy": policy, "cadence": cadenceReply, "target": targetReply, "contentLatency": latencyReply]
    }

    /// Conditions at the read that can limit the display rate.
    private static func context() -> [String] {
        let process = ProcessInfo.processInfo
        var known: [String] = []
        #if targetEnvironment(simulator)
        known.append("simulator")
        #endif
        if process.isLowPowerModeEnabled { known.append("low-power-mode") }
        switch process.thermalState {
        case .serious: known.append("thermal-serious")
        case .critical: known.append("thermal-critical")
        default: break
        }
        return known
    }
}
