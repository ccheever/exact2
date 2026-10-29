import Foundation
import QuartzCore
import CExact

/// Compute deltas and percentiles in the reader; hooks update counters and duration histograms.
enum FrameHealthMetrics {
    static let format = "exact.frame-health.metrics/1"
    static let histogramVersion = "64x500us-tail/1"

    struct Sample {
        let producer: UInt32
        let snapshot: ExactFrameHealthSnapshot
        let intervals: [UInt32]
        let applies: [UInt32]
        let read: UInt64
        var values: [UInt64] {
            let s = snapshot
            return [s.callbacks, s.intervals, s.active_ns, s.work_overruns, s.cadence_changes,
                s.applies, s.target_applies, s.apply_overruns, s.interval_through_ns, s.apply_through_ns, s.histogram_overflows]
        }
        var cursor: [String: Any] {
            ["version": 1, "producer": String(producer), "generation": String(snapshot.generation),
                "values": values.map(String.init), "intervals": intervals, "applies": applies, "readNs": String(read)]
        }
    }

    static func read(_ health: FrameHealth, now: Double = CACurrentMediaTime()) -> Sample {
        func bins(_ kind: UInt32) -> [UInt32] {
            guard health.enabled else { return Array(repeating: 0, count: 64) }
            var distribution = exact_frame_health_distribution(health.handle, kind)
            return withUnsafeBytes(of: &distribution.bins) { Array($0.bindMemory(to: UInt32.self)) }
        }
        return Sample(producer: health.handle, snapshot: health.snapshot(), intervals: bins(0), applies: bins(1), read: FrameHealth.nanoseconds(now))
    }

    static func reply(_ sample: Sample, health: FrameHealth, cursor: [String: Any]? = nil) -> [String: Any] {
        let s = sample.snapshot
        var baseline = Array(repeating: UInt64(0), count: 11)
        var intervals = Array(repeating: UInt32(0), count: 64), applies = intervals
        var previousRead: UInt64 = 0
        if let cursor {
            guard cursor["version"] as? Int == 1,
                  cursor["producer"] as? String == String(sample.producer),
                  cursor["generation"] as? String == String(s.generation),
                  let values = integers(cursor["values"], count: 11),
                  let priorIntervals = bins(cursor["intervals"]), let priorApplies = bins(cursor["applies"]),
                  let read = (cursor["readNs"] as? String).flatMap(UInt64.init), read <= sample.read,
                  zip(values, sample.values).allSatisfy({ $0 <= $1 }),
                  zip(priorIntervals, sample.intervals).allSatisfy({ $0 <= $1 }),
                  zip(priorApplies, sample.applies).allSatisfy({ $0 <= $1 }) else {
                return ["format": format, "available": false, "error": "baseline-required", "reason": "The cursor is malformed or belongs to another producer/generation. Read again without a cursor."]
            }
            baseline = values; intervals = priorIntervals; applies = priorApplies; previousRead = read
        }
        let delta = zip(sample.values, baseline).map(-)
        let intervalBins = zip(sample.intervals, intervals).map(-), applyBins = zip(sample.applies, applies).map(-)
        let distributionsValid = s.histogram_overflows == 0
            && intervalBins.reduce(UInt64(0), { $0 + UInt64($1) }) == delta[1]
            && applyBins.reduce(UInt64(0), { $0 + UInt64($1) }) == delta[5]
        if !distributionsValid, cursor != nil, s.histogram_overflows == 0 {
            return ["format": format, "available": false, "error": "baseline-required", "reason": "Cursor bins do not match their populations. Read again without a cursor."]
        }
        var callbacks: [String: Any] = ["count": String(delta[0]), "comparableIntervals": String(delta[1]),
            "observedNs": String(delta[2]), "synchronousTargetOverruns": String(delta[3]),
            "cadenceChanges": String(delta[4]), "perSecond": delta[2] == 0 ? NSNull() : Double(delta[1]) * 1e9 / Double(delta[2]),
            "lifetimeMaximumGapNs": String(s.max_gap_ns)]
        var apply: [String: Any] = ["count": String(delta[5]), "withTarget": String(delta[6]), "targetOverruns": String(delta[7])]
        for (name, fraction) in [("p50BoundsNs", 0.5), ("p95BoundsNs", 0.95), ("p99BoundsNs", 0.99)] {
            callbacks[name] = distributionsValid ? quantile(intervalBins, fraction: fraction) : NSNull()
            apply[name] = distributionsValid ? quantile(applyBins, fraction: fraction) : NSNull()
        }
        let result: [String: Any] = ["format": format, "available": health.enabled && !health.virtual,
            "source": health.source, "evidence": "callback-and-apply-proxies", "qualification": "unqualified",
            "status": health.virtual ? "virtual" : !health.enabled ? "unavailable" : s.running == 0 ? "inactive" : "observing",
            "unavailableReason": health.unavailableCode as Any? ?? (health.virtual ? "virtual-clock" : NSNull()),
            "window": ["generation": String(s.generation), "kind": cursor == nil ? "since-reset" : "delta",
                "readFromNs": String(previousRead), "readThroughNs": String(sample.read),
                "intervalFromNs": String(baseline[8]), "intervalThroughNs": String(s.interval_through_ns),
                "applyFromNs": String(baseline[9]), "applyThroughNs": String(s.apply_through_ns)],
            "callbacks": callbacks, "apply": apply, "histogram": ["version": histogramVersion, "valid": distributionsValid,
                "overflows": String(s.histogram_overflows), "quantiles": "half-open bounds; null upper is overflow"],
            "scheduledPeriodNs": String(s.period_ns), "requestedPeriodNs": String(health.requestedPeriodNs), "observationAgeNs": s.last_arrival_ns == 0 ? NSNull() : String(sample.read >= s.last_arrival_ns ? sample.read - s.last_arrival_ns : 0),
            "coverage": ["presentation": "unobserved", "inputLatency": "unobserved", "GPU": "unobserved",
                "nativeScrolling": health.nativeCoverage, "nativeUpdateSamplesLost": String(health.nativeSampleLoss), "liveAgent": ExactEnv.agentMode ? "visual-scheduling-audited; other agent overrides remain" : "ordinary-app"],
            "capture": ["agentLeaseRemainingMs": max(0, ((health.agentCaptureUntil - CACurrentMediaTime()) * 1000).rounded()),
                "hud": health.hudVisible, "inspector": health.inspectorCapture],
            "cursor": sample.cursor]
        guard let data = try? JSONSerialization.data(withJSONObject: result), data.count < 4000 else {
            return ["format": format, "available": false, "error": "metrics-budget", "reason": "The metrics exceeded the 4000-byte carrier budget. Use a matching Exact reader."]
        }
        return result
    }

    static func quantile(_ bins: [UInt32], fraction: Double) -> Any {
        let total = bins.reduce(UInt64(0)) { $0 + UInt64($1) }
        guard total > 0 else { return NSNull() }
        let rank = UInt64(ceil(Double(total) * fraction))
        var count: UInt64 = 0
        for (index, value) in bins.enumerated() {
            count += UInt64(value)
            if count >= rank {
                let lower: UInt64 = index < 48 ? UInt64(index) * 500_000 : 24_000_000 << (index - 48)
                let upper: Any = index == 63 ? NSNull() : index < 48 ? (UInt64(index) + 1) * 500_000 : lower * 2
                return [lower, upper]
            }
        }
        return NSNull()
    }

    private static func integers(_ value: Any?, count: Int) -> [UInt64]? {
        guard let strings = value as? [String], strings.count == count else { return nil }
        let result = strings.compactMap { text -> UInt64? in
            guard text.count <= 20, !text.isEmpty, text.utf8.allSatisfy({ $0 >= 48 && $0 <= 57 }) else { return nil }
            return UInt64(text)
        }
        return result.count == count ? result : nil
    }
    private static func bins(_ value: Any?) -> [UInt32]? {
        guard let numbers = value as? [NSNumber], numbers.count == 64,
              numbers.allSatisfy({ $0.doubleValue >= 0 && $0.doubleValue <= Double(UInt32.max) && $0.doubleValue.rounded() == $0.doubleValue }) else { return nil }
        return numbers.map(\.uint32Value)
    }
}

/// Retain at most eleven snapshots for a roughly five-second view, without a polling timer.
final class FrameHealthWindow {
    private var samples: [FrameHealthMetrics.Sample] = []
    private var cached: FrameHealthMetrics.Sample?
    let trend = FrameHealthTrend()
    func invalidate() { cached = nil; trend.breakBaseline() }
    func reply(_ health: FrameHealth, now: Double = CACurrentMediaTime()) -> [String: Any] {
        let instant = FrameHealth.nanoseconds(now)
        let current: FrameHealthMetrics.Sample
        if let cached, instant >= cached.read, instant - cached.read < 500_000_000 { current = cached }
        else { current = FrameHealthMetrics.read(health, now: now); cached = current }
        if samples.last?.snapshot.generation != current.snapshot.generation { samples.removeAll(keepingCapacity: true) }
        let baseline = samples.first
        if samples.last.map({ current.read >= $0.read && current.read - $0.read >= 500_000_000 }) ?? true {
            trend.record(current)
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
    func rollingMetrics() -> [String: Any] { window.reply(self) }
}
