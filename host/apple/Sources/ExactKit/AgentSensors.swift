// A clock advance across which a canvas has a sensor input (LLP 1133.013
// D6): the worlds advance a display frame at a time, each frame's samples
// handed to their surfaces before the frame, as a live frame does. One jump
// would queue a whole span at once, past a world's 1,024-event bound. The
// samples are the runner's substitute's (`sensorEvents`), so this runs only
// under the agent; a package feeding a canvas on a device is not built yet.
import Foundation

extension Agent {
    /// The clock stepped to `to` a frame at a time while the substitute
    /// feeds a canvas: the last step's batch, or nil when none feeds one.
    func sensorFrames(to: Double, deadline: Date, floor: Double) -> Batch? {
        var last: Batch?
        while let from = session.clock, from < to {
            let step = min(from + 1000 / 60, to)
            guard let events = sensorEvents(step) else { return last }
            for (surface, event) in events where !session.canvases.sensor(surface, event) {
                session.runtime.log("sensor input \(surface): dropped, no live canvas")
            }
            let batch = advanceStepped(to: step, deadline: deadline, floor: floor)
            last = batch
            let landed = max(floor, batch.clock ?? step)
            session.clock = landed
            session.apply(session.runtime.tick(now: landed))
            if batch.error != nil || landed < step { return batch }
            guard session.canvases.waitUntilReady() else { return batch }
            session.canvases.settle(now: landed)
            _ = session.canvases.clock(settle: false)
        }
        return last
    }

    /// What the substitute's canvas inputs owe their surfaces up to `to`, or
    /// nil when no input runs or ended.
    private func sensorEvents(_ to: Double) -> [(String, [String: Any])]? {
        guard let d = session.agent("{\"op\":\"sensorEvents\",\"to\":\(to)}").data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any],
              o["inputs"] as? Bool == true else { return nil }
        return (o["events"] as? [[String: Any]] ?? []).compactMap { e in
            guard let surface = e["surface"] as? String, let event = e["event"] as? [String: Any] else { return nil }
            return (surface, event)
        }
    }
}
