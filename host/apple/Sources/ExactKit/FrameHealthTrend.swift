import Foundation

/// Keep up to 60 reader samples over 30 seconds; frame hooks do not update this history.
final class FrameHealthTrend {
    struct Point {
        let time: UInt64
        let average: Double?
        /// Lower rate bound computed from the longest interval bin's upper bound.
        let slowest: Double?
    }
    private(set) var points: [Point] = []
    private var generation: UInt64?
    private var previous: FrameHealthMetrics.Sample?
    func breakBaseline() { previous = nil }
    func record(_ current: FrameHealthMetrics.Sample) {
        defer { previous = current }
        if generation != current.snapshot.generation { points.removeAll(keepingCapacity: true) }
        generation = current.snapshot.generation
        var average: Double?, slowest: Double?
        if let previous, current.snapshot.running != 0, current.snapshot.period_ns != 0,
           current.snapshot.generation == previous.snapshot.generation,
           current.snapshot.intervals >= previous.snapshot.intervals, current.snapshot.active_ns > previous.snapshot.active_ns,
           current.snapshot.histogram_overflows == 0,
           zip(previous.intervals, current.intervals).allSatisfy({ $0 <= $1 }) {
            let count = current.snapshot.intervals - previous.snapshot.intervals
            let elapsed = current.snapshot.active_ns - previous.snapshot.active_ns
            let bins = zip(current.intervals, previous.intervals).map(-)
            if count > 0, bins.reduce(UInt64(0), { $0 + UInt64($1) }) == count {
                average = Double(count) * 1e9 / Double(elapsed)
                if let range = FrameHealthMetrics.quantile(bins, fraction: 1) as? [Any],
                   let upper = range[1] as? NSNumber, upper.doubleValue > 0 {
                    slowest = 1e9 / upper.doubleValue
                }
            }
        }
        points.append(Point(time: current.read, average: average, slowest: slowest))
        if points.count > 60 { points.removeFirst(points.count - 60) }
        while let first = points.first, current.read >= first.time, current.read - first.time > 30_000_000_000 { points.removeFirst() }
    }
}
