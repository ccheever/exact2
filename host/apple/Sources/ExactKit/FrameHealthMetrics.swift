import Foundation
import QuartzCore
import CExact

/// Compute deltas and percentiles in the reader; hooks update counters and duration histograms.
enum FrameHealthMetrics {
    static let format = "exact.frame-health.metrics/2"
    static let histogramVersion = "frame-health-bins/2"

    /// A histogram layout (LLP 1079.000 §5.3): 0.5 ms bins below `linear`, doubling tails above,
    /// and an open last bin.
    enum Layout {
        case duration(linearNs: UInt64, count: Int), refreshes
        static let frames = Layout.duration(linearNs: 24_000_000, count: 64), late = Layout.duration(linearNs: 12_000_000, count: 32)
        var count: Int { if case .duration(_, let count) = self { count } else { 32 } }
        /// The bin's lower bound and its upper bound, nil for the open last bin.
        func bounds(_ index: Int) -> (UInt64, UInt64?) {
            switch self {
            case .refreshes: return (UInt64(index + 1), index == 31 ? nil : UInt64(index + 2))
            case .duration(let linear, _):
                let steps = Int(linear / 500_000)
                if index < steps { return (UInt64(index) * 500_000, UInt64(index + 1) * 500_000) }
                let lower = linear << (index - steps)
                return (lower, index == count - 1 ? nil : lower * 2)
            }
        }
    }

    struct Sample {
        let producer: UInt32
        let snapshot: ExactFrameHealthSnapshot
        let intervals: [UInt32]
        let work: [UInt32]
        let refreshes: [UInt32]
        let lateBy: [UInt32]
        let read: UInt64
        /// Cumulative values a cursor carries; 8 and 9 are watermarks.
        var values: [UInt64] {
            let s = snapshot
            return [s.callbacks, s.held, s.intervals, s.active_ns, s.cadence_changes, s.pacing_changes, s.worked, s.late,
                s.interval_through_ns, s.work_through_ns, s.histogram_overflows, s.requested_slots, s.missed_frames, s.restricted_intervals,
                s.skipped_refreshes, s.unclassified_intervals, s.out_of_order]
        }
        /// The cursor carries the two 64-bin populations, so a dense one still fits the 4000-byte
        /// envelope; refresh and lateness bins are the card's.
        var cursor: [String: Any] {
            ["version": 3, "producer": String(producer), "generation": String(snapshot.generation),
                "values": values.map(String.init), "intervals": intervals, "work": work, "readNs": String(read)]
        }
    }

    static func read(_ health: FrameHealth, now: Double = CACurrentMediaTime()) -> Sample {
        func bins(_ kind: UInt32, _ count: Int) -> [UInt32] {
            guard health.enabled else { return Array(repeating: 0, count: count) }
            var distribution = exact_frame_health_distribution(health.handle, kind)
            return withUnsafeBytes(of: &distribution.bins) { Array($0.bindMemory(to: UInt32.self).prefix(count)) }
        }
        return Sample(producer: health.handle, snapshot: health.snapshot(), intervals: bins(0, 64), work: bins(1, 64),
            refreshes: bins(2, 32), lateBy: bins(3, 32), read: FrameHealth.nanoseconds(now))
    }

    static func reply(_ sample: Sample, health: FrameHealth, cursor: [String: Any]? = nil) -> [String: Any] {
        let s = sample.snapshot
        let current: [[UInt32]] = [sample.intervals, sample.work]
        var baseline = Array(repeating: UInt64(0), count: 17)
        var prior = current.map { Array(repeating: UInt32(0), count: $0.count) }
        var previousRead: UInt64 = 0
        if let cursor {
            let bins = zip(["intervals", "work"], current).compactMap { self.bins(cursor[$0], count: $1.count) }
            guard cursor["version"] as? Int == 3,
                  cursor["producer"] as? String == String(sample.producer),
                  cursor["generation"] as? String == String(s.generation),
                  let values = integers(cursor["values"], count: 17), bins.count == current.count,
                  let read = (cursor["readNs"] as? String).flatMap(UInt64.init), read <= sample.read,
                  zip(values, sample.values).allSatisfy({ $0 <= $1 }),
                  zip(bins, current).allSatisfy({ zip($0, $1).allSatisfy { $0 <= $1 } }) else {
                return ["format": format, "available": false, "error": "baseline-required", "reason": "The cursor is malformed or belongs to another producer/generation. Read again without a cursor."]
            }
            baseline = values; prior = bins; previousRead = read
        }
        let delta = zip(sample.values, baseline).map(-)
        let d = zip(current, prior).map { zip($0, $1).map(-) }
        let intervals = d[0], work = d[1]
        func sum(_ bins: [UInt32]) -> UInt64 { bins.reduce(0) { $0 + UInt64($1) } }
        let distributionsValid = s.histogram_overflows == 0 && sum(intervals) == delta[2] && sum(work) == delta[6]
        if !distributionsValid, cursor != nil, s.histogram_overflows == 0 {
            return ["format": format, "available": false, "error": "baseline-required", "reason": "Cursor bins do not match their populations. Read again without a cursor."]
        }
        func bounds(_ bins: [UInt32], _ layout: Layout, _ fraction: Double) -> Any {
            distributionsValid ? quantile(bins, fraction: fraction, layout: layout) : NSNull()
        }
        let frames: [String: Any] = ["callbacks": String(delta[0]), "held": String(delta[1]), "intervals": String(delta[2]),
            "observedNs": String(delta[3]), "perSecond": delta[3] == 0 ? NSNull() : Double(delta[2]) * 1e9 / Double(delta[3]),
            "lifetimeMaximumIntervalNs": String(s.max_interval_ns), "medianBoundsNs": bounds(intervals, .frames, 0.5),
            "p99BoundsNs": bounds(intervals, .frames, 0.99), "maximumBoundsNs": bounds(intervals, .frames, 1),
            "cadenceChanges": String(delta[4]), "pacingChanges": String(delta[5]), "unclassifiedIntervals": String(delta[15]),
            "outOfOrder": String(delta[16]),
            "requestedSlots": String(delta[11]), "missedFrames": String(delta[12]),
            "restrictedIntervals": String(delta[13])]
        // Where the deadline carries no information, lateness is unknown, not a count
        let lateness = FrameHealthAssessment.deadlinesObserved
        let (pending, overdue) = health.pendingFrames(at: Double(sample.read) / 1e9)
        let workReply: [String: Any] = ["records": String(delta[6]),
            "late": lateness ? String(delta[7]) : NSNull(), "latenessUnknown": lateness ? NSNull() : "simulator-deadlines",
            "pending": pending, "overdue": lateness ? overdue : NSNull(),
            "medianBoundsNs": bounds(work, .frames, 0.5), "p99BoundsNs": bounds(work, .frames, 0.99),
            "maximumBoundsNs": bounds(work, .frames, 1)]
        // A running source that has sent no callback for two publication intervals, as through a
        // blocked main thread, has stale evidence
        let stale = s.running != 0 && s.last_arrival_ns != 0 && sample.read > s.last_arrival_ns + 1_000_000_000
        let window = FrameHealthAssessment.Window(delta, scheduledPeriodNs: s.period_ns, running: s.running != 0,
            overdue: UInt64(overdue), stale: stale)
        let result: [String: Any] = ["format": format, "available": health.enabled && !health.virtual,
            "source": health.source, "evidence": "callback-and-owner-work-proxies", "qualification": "unqualified",
            "status": health.virtual ? "virtual" : !health.enabled ? "unavailable" : s.running == 0 ? "inactive" : stale ? "stale" : "observing",
            "unavailableReason": health.unavailableCode as Any? ?? (health.virtual ? "virtual-clock" : NSNull()),
            "window": ["generation": String(s.generation), "kind": cursor == nil ? "since-reset" : "delta",
                "readFromNs": String(previousRead), "readThroughNs": String(sample.read),
                "intervalFromNs": String(baseline[8]), "intervalThroughNs": String(s.interval_through_ns),
                "workFromNs": String(baseline[9]), "workThroughNs": String(s.work_through_ns)],
            "frames": frames, "work": workReply, "assessments": FrameHealthAssessment.reply(window, health: health),
            "histogram": ["version": histogramVersion, "valid": distributionsValid,
                "overflows": String(s.histogram_overflows), "quantiles": "half-open bounds; null upper is overflow"],
            "scheduledPeriodNs": String(s.period_ns), "requestedPeriodNs": String(health.requestedPeriodNs), "observationAgeNs": s.last_arrival_ns == 0 ? NSNull() : String(sample.read >= s.last_arrival_ns ? sample.read - s.last_arrival_ns : 0),
            "coverage": ["presentation": "unobserved", "coreAnimationCommit": "unobserved", "inputLatency": "unobserved", "GPU": "unobserved",
                "nativeScrolling": health.nativeCoverage, "nativeUpdateSamplesLost": String(health.nativeSampleLoss), "liveAgent": ExactEnv.agentMode ? "visual-scheduling-audited; other agent overrides remain" : "ordinary-app"],
            "capture": ["agentLeaseRemainingMs": max(0, ((health.agentCaptureUntil - CACurrentMediaTime()) * 1000).rounded()),
                "hud": health.hudVisible, "inspector": health.inspectorCapture],
            "cursor": sample.cursor]
        guard let data = try? JSONSerialization.data(withJSONObject: result), data.count < 4000 else {
            return ["format": format, "available": false, "error": "metrics-budget", "reason": "The metrics exceeded the 4000-byte carrier budget. Use a matching Exact reader."]
        }
        return result
    }

    /// The bounds of the bin holding `fraction` of the values, by nearest rank, or null without values.
    static func quantile(_ bins: [UInt32], fraction: Double, layout: Layout = .frames) -> Any {
        guard let index = bin(bins, fraction: fraction) else { return NSNull() }
        let (lower, upper) = layout.bounds(index)
        return [lower, upper.map { $0 as Any } ?? NSNull()]
    }

    /// The index of the bin holding `fraction` of the values, by nearest rank.
    static func bin(_ bins: [UInt32], fraction: Double) -> Int? {
        let total = bins.reduce(UInt64(0)) { $0 + UInt64($1) }
        guard total > 0 else { return nil }
        let rank = max(1, UInt64(ceil(Double(total) * fraction)))
        var count: UInt64 = 0
        for (index, value) in bins.enumerated() {
            count += UInt64(value)
            if count >= rank { return index }
        }
        return nil
    }

    private static func integers(_ value: Any?, count: Int) -> [UInt64]? {
        guard let strings = value as? [String], strings.count == count else { return nil }
        let result = strings.compactMap { text -> UInt64? in
            guard text.count <= 20, !text.isEmpty, text.utf8.allSatisfy({ $0 >= 48 && $0 <= 57 }) else { return nil }
            return UInt64(text)
        }
        return result.count == count ? result : nil
    }
    private static func bins(_ value: Any?, count: Int) -> [UInt32]? {
        guard let numbers = value as? [NSNumber], numbers.count == count,
              numbers.allSatisfy({ $0.doubleValue >= 0 && $0.doubleValue <= Double(UInt32.max) && $0.doubleValue.rounded() == $0.doubleValue }) else { return nil }
        return numbers.map(\.uint32Value)
    }
}

/// Retain at most eleven snapshots for a roughly five-second view, without a polling timer.
/// Readers that keep history in the process: cards and Details inspectors. At most four keep it at
/// once, which holds them, with the collectors, within LLP 1079.000 §5.2's storage budget.
enum FrameHealthReaders {
    private static var open = 0
    static let full = "Four other frame health readers are open; close one to read here"
    static func claim() -> Bool {
        guard open < 4 else { return false }
        open += 1
        return true
    }
    static func release() { open -= 1 }
}

/// The Details inspector's rolling window: up to eleven snapshots over five seconds, kept while an
/// inspector shows this source and freed when it closes.
final class FrameHealthWindow {
    private var samples: [FrameHealthMetrics.Sample] = []
    private var cached: FrameHealthMetrics.Sample?
    private var claimed = false
    func invalidate() { cached = nil }
    /// Free the history and its place among the readers.
    func close() {
        if claimed { FrameHealthReaders.release() }
        claimed = false
        samples = []
        cached = nil
    }
    deinit { if claimed { FrameHealthReaders.release() } }
    /// Nil when four other readers keep history.
    func reply(_ health: FrameHealth, now: Double = CACurrentMediaTime()) -> [String: Any]? {
        if !claimed { claimed = FrameHealthReaders.claim() }
        guard claimed else { return nil }
        let instant = FrameHealth.nanoseconds(now)
        let current: FrameHealthMetrics.Sample
        if let cached, instant >= cached.read, instant - cached.read < 500_000_000 { current = cached }
        else { current = FrameHealthMetrics.read(health, now: now); cached = current }
        if samples.last?.snapshot.generation != current.snapshot.generation { samples.removeAll(keepingCapacity: true) }
        let baseline = samples.first
        if samples.last.map({ current.read >= $0.read && current.read - $0.read >= 500_000_000 }) ?? true {
            samples.append(current)
            if samples.count > 11 { samples.removeFirst() }
            while samples.count > 1, current.read - samples[1].read > 5_000_000_000 { samples.removeFirst() }
        }
        return FrameHealthMetrics.reply(current, health: health, cursor: baseline?.cursor)
    }
}

extension FrameHealth {
    var liveCapture: Bool { agentCaptureUntil > CACurrentMediaTime() && !virtual }
    var reportSource: FrameHealth {
        guard let nativeHealth else { return self }
        let own = historyStatus(), native = nativeHealth.historyStatus()
        let ownTime = own.retained == 0 ? 0 : hitch(UInt32(own.retained - 1)).at_ns
        let nativeTime = native.retained == 0 ? 0 : nativeHealth.hitch(UInt32(native.retained - 1)).at_ns
        return nativeTime > ownTime ? nativeHealth : self
    }
    func metrics(cursor: [String: Any]? = nil) -> [String: Any] {
        expireCapture(at: CACurrentMediaTime())
        return FrameHealthMetrics.reply(FrameHealthMetrics.read(self), health: self, cursor: cursor)
    }
    func rollingMetrics() -> [String: Any]? { window.reply(self) }
}
