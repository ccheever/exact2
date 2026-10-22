// T3Module's timeline op (T3Module.swift routes it): the data module's sleep.
import Foundation

extension T3Module {
    /// The data module's sleep.
    func timelineOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if request["op"] as? String == "timelineSleep" { return Self.sleep(request) { reply.send($0) } }
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
