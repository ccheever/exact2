// What the main thread did, by named host phase (LLP 1142 D1): a late frame
// is charged each phase's overlap with its interval, as its hatch calls are
// (LLP 1075.003.000.001 §3.1). A join by window, not a causal trace: no
// stacks, no attribution to app code, and the rest stated as unattributed.
import Foundation
import QuartzCore

package final class HostPhases: @unchecked Sendable {
    package static let shared = HostPhases()
    package init() {}
    /// The media clock, seconds; a test gives its own.
    package var clock: () -> Double = CACurrentMediaTime

    /// The fixed phases: what the host does on the main thread to put a frame
    /// up. Each is charged only the time no phase inside it took.
    package enum Phase: String, CaseIterable {
        /// Waiting on the runtime's thread: the runner, the kernel and its layout.
        case runtime
        /// Applying a batch to the views.
        case apply
        /// A collection building and reusing its rows.
        case rows
        /// A collection keeping its anchored row in place.
        case anchor
        /// Text measured, drawn or rastered on the main thread.
        case text
        /// A scroll view's callback, less what nests inside it.
        case scroll
        /// A region reader drawing.
        case draw
    }


    /// Operation counts are process-wide: the owner and host phases are shared
    /// by sessions. Node ids are session-local, so these counters claim no site.
    package enum Operation: String, CaseIterable {
        case ownerSync, intrinsicReports, intrinsicLayoutRequests, nativeControlMeasurements
    }
    private var counts: [Operation: Int] = [:]
    private var countSince: Double?
    package func count(_ operation: Operation, by amount: Int = 1) {
        guard on, Thread.isMainThread else { return }
        if countSince == nil { countSince = clock() }
        counts[operation, default: 0] += amount
    }
    package func work() -> [String: Any] {
        ["scope": "process", "site": "unknown", "measured": on,
         "since": countSince.map { $0 * 1000 } ?? NSNull(), "timebase": "media-clock-ms",
         "counts": Dictionary(uniqueKeysWithValues: Operation.allCases.map { ($0.rawValue, counts[$0, default: 0]) }),
         "coverage": ["ownerSync": "main-thread calls submitted to the runtime owner",
                      "intrinsicReports": "sizes submitted through Runtime.intrinsics",
                      "intrinsicLayoutRequests": "Runtime.intrinsics calls, including unchanged sizes",
                      "nativeControlMeasurements": "button, field chrome and mounted value-control measurement calls"]]
    }

    private struct Segment { let phase: Phase; let start: Double; let end: Double }

    /// Segments kept: about a second of a busy fling's.
    package static let capacity = 2048
    /// Whether anything records: the host turns it on where its frame sampler
    /// may run (not a production bake). Main thread only, as every phase is.
    package var on = false {
        didSet { if on && !oldValue { since = clock(); if countSince == nil { countSince = since } } }
    }
    private var ring: [Segment] = []
    private var next = 0
    /// Recording covers windows from here: when it was turned on, or the end
    /// of the newest segment the ring has let go.
    private var since = Double.infinity
    private var open: [(phase: Phase, from: Double)] = []

    /// Open `phase` on the main thread; `end` with what it returned. The phase
    /// it nests in stops being charged until then. Elsewhere, nothing.
    package func begin(_ phase: Phase) -> Bool {
        guard on, Thread.isMainThread else { return false }
        let now = clock()
        if let top = open.last { keep(top.phase, top.from, now) }
        open.append((phase, now))
        return true
    }

    package func end(_ began: Bool) {
        guard began, let top = open.popLast() else { return }
        let now = clock()
        keep(top.phase, top.from, now)
        if !open.isEmpty { open[open.count - 1].from = now }
    }

    package func measure<T>(_ phase: Phase, _ body: () throws -> T) rethrows -> T {
        let began = begin(phase)
        defer { end(began) }
        return try body()
    }

    private func keep(_ phase: Phase, _ start: Double, _ end: Double) {
        guard end > start else { return }
        let segment = Segment(phase: phase, start: start, end: end)
        if ring.count < Self.capacity {
            ring.append(segment)
        } else {
            since = max(since, ring[next].end)
            ring[next] = segment
            next = (next + 1) % Self.capacity
        }
    }

    /// The phases' time inside `(from, to]`, seconds on the media clock:
    /// `phases` (each one's ms and segments), `unattributed` (the window's
    /// ms no phase took: idle time, the platform's own work, anything not
    /// wrapped), and `coverage: partial` when recording does not reach back
    /// to `from`. Nil while not recording. A phase still open is charged
    /// when it closes, in the window it closes in.
    package func window(from: Double, to: Double) -> [String: Any]? {
        guard on, to > from else { return nil }
        var ms: [Phase: (ms: Double, n: Int)] = [:]
        for s in ring where s.end > from && s.start <= to {
            ms[s.phase, default: (0, 0)].ms += (min(s.end, to) - max(s.start, from)) * 1000
            ms[s.phase, default: (0, 0)].n += 1
        }
        let r2 = { (x: Double) in (x * 100).rounded() / 100 }
        let charged = ms.values.reduce(0) { $0 + $1.ms }
        var out: [String: Any] = [
            "phases": Phase.allCases.compactMap { p in ms[p].map { ["phase": p.rawValue, "ms": r2($0.ms), "segments": $0.n] as [String: Any] } },
            "unattributed": r2(max(0, (to - from) * 1000 - charged)),
        ]
        if since > from { out["coverage"] = "partial" }
        return out
    }
}
