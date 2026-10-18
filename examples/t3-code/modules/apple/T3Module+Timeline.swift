// T3Module's timeline ops (T3Module.swift routes them): the data module's sleep and image chip accents.
import Foundation
import AppKit

extension T3Module {
    /// The data module's sleep and image chip accents.
    func timelineOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "timelineSleep" { return Self.sleep(request) { reply.send($0) } }
        if request["op"] as? String == "imageAccent" { return T3ImageAccent.perform(request) { reply.send($0) } } // Image chip accents (T3ImageAccent.swift).
        next()
    }

    /// A short wait for TypeScript flows that watch the projection settle (Edit from here waits for its
    /// rollback's outcome, View source for its thread's rows), at most two seconds: data modules have no timers.
    static func sleep(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let delay = min(2.0, max(0.01, (request["ms"] as? Double ?? 200) / 1000))
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { reply(["ok": true, "generation": generation, "value": ["slept": delay]]) }
    }
}
