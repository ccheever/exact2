// A package feeds a canvas (LLP 1133.013 D6): what its implementation
// writes into a surface's input (`ExactSurfaceInput`, through the host
// table's `surface_sensor`) waits here until the next display frame, which
// delivers every sample since the last, in order, to the live canvas of
// that surface with the lowest view id, before the frame's render ticks
// its world. A sample with no live canvas to take it is gone, as a key
// pressed over nothing is.
//
// The implementation writes on the main thread (Core Motion's handler runs
// on the main queue there), so the queue needs no lock.
import Foundation

final class SensorRings {
    /// About a second at the fastest rate a phone samples (Android's 200
    /// Hz, Core Motion's 100), a surface: a world queues at most 1,024
    /// events a frame (`game/engine/src/sim/queue.rs`).
    static let bound = 256

    struct Sample {
        let sensor: String
        let values: [Double]
        let dropped: Int
        let at: Double
        let ended: Bool
    }

    private var waiting: [String: [Sample]] = [:]
    /// Samples the bound let go, by surface and sensor: counted into the
    /// next of that sensor delivered.
    private var lost: [String: [String: Int]] = [:]

    var isEmpty: Bool { waiting.isEmpty }

    func write(_ surface: String, _ sample: Sample) {
        var queue = waiting[surface] ?? []
        if queue.count >= Self.bound, let i = queue.firstIndex(where: { !$0.ended }) {
            let gone = queue.remove(at: i)
            lost[surface, default: [:]][gone.sensor, default: 0] += 1 + gone.dropped
        }
        queue.append(sample)
        waiting[surface] = queue
    }

    /// Every sample written since the last drain, by surface, as the input
    /// events the hosts write (`{"t":"sensor"|"sensorEnded", …}`), oldest first.
    func drain() -> [(surface: String, event: [String: Any])] {
        var out: [(String, [String: Any])] = []
        for (surface, queue) in waiting.sorted(by: { $0.key < $1.key }) {
            for s in queue {
                if s.ended {
                    out.append((surface, ["t": "sensorEnded", "sensor": s.sensor, "at": s.at]))
                    continue
                }
                let dropped = s.dropped + (lost[surface]?.removeValue(forKey: s.sensor) ?? 0)
                let values = (s.values + [0, 0, 0, 0]).prefix(4).map { $0.isFinite ? $0 : 0 }
                out.append((surface, ["t": "sensor", "sensor": s.sensor, "values": Array(values), "dropped": dropped, "at": s.at]))
            }
        }
        waiting.removeAll()
        return out
    }
}

extension CanvasesHost {
    func sensorWrite(_ surface: String, _ sensor: String, _ values: [Double], dropped: Int, at: Double, ended: Bool) {
        sensorRings.write(surface, SensorRings.Sample(sensor: sensor, values: values, dropped: dropped, at: at, ended: ended))
        session?.frames.requestCanvas()
    }

    /// At a display frame, before its render: what packages wrote into the
    /// canvases' inputs since the last.
    func drainSensors() {
        guard !sensorRings.isEmpty else { return }
        var gone: Set<String> = []
        for (surface, event) in sensorRings.drain() where !sensor(surface, event) { gone.insert(surface) }
        for surface in gone.sorted() { session?.log("sensor input \(surface): no live canvas took its samples") }
    }
}
