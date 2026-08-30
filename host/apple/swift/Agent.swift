// The agent API's presenter half, the part both presenters share (LLP
// 1012). Requests arrive as JSON lines on a stream the platform opened —
// stdin on macOS, a Unix socket on iOS (a simulator app has no stdin) —
// and are admitted in order on main, while runner/kernel work stays on the
// dedicated runtime thread; the clock is the last
// `clock` value: no timer advances the runner, events carry the agent's
// time, the engine is seeked to it. `tree`, `state`, `logs`, and `settle`
// go to the library (`exact_agent`); `clock` moves both clocks here;
// `layout`, `tap`, `type`, and `screenshot` are the platform's
// (`AgentMac.swift`, `AgentIOS.swift`), being about what it renders and
// its input path.
import Foundation

enum Agent {
    /// Where replies go: the stream the requests came on.
    nonisolated(unsafe) static var out = FileHandle.standardOutput

    /// Serve requests from `fd` until it closes — on the calling thread:
    /// each line is admitted on main and answered before the next is read.
    /// The stream closing ends the process.
    static func serve(fd: Int32) {
        var pending = Data()
        var buf = [UInt8](repeating: 0, count: 65536)
        while true {
            let n = read(fd, &buf, buf.count)
            if n <= 0 { break }
            pending.append(buf, count: n)
            while let i = pending.firstIndex(of: UInt8(ascii: "\n")) {
                let line = String(decoding: pending[pending.startIndex..<i], as: UTF8.self)
                pending.removeSubrange(pending.startIndex...i)
                let answered = DispatchSemaphore(value: 0)
                DispatchQueue.main.async { handle(line) { answered.signal() } }
                answered.wait()
            }
        }
        DispatchQueue.main.async { exit(0) }
    }

    static func handle(_ line: String, done: @escaping () -> Void) {
        guard let data = line.data(using: .utf8),
              let req = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let op = req["op"] as? String
        else { reply(["error": "unreadable request: \(line)"]); done(); return }
        let finish: ([String: Any]) -> Void = { response in reply(response); done() }
        switch op {
        case "quit": exit(0)
        case "layout": finish(layout())
        // A call that moved something settles the canvases before it
        // replies (LLP 1012's fixed point; LLP 1014 D5 reads placements
        // after a frame, so the frame is rendered here, not left to the
        // display link to get to between two calls).
        case "tap":
            let response = tap(req)
            runtime.barrier { canvases.settle(now: now()); finish(response) }
        case "type":
            let response = type(req)
            runtime.barrier { canvases.settle(now: now()); finish(response) }
        case "clock":
            clock(req) { response in canvases.settle(now: now()); finish(response) }
        case "screenshot": finish(screenshot(req))
        default: runtime.agent(line) { response in raw(response); done() }
        }
    }

    static func reply(_ obj: [String: Any]) {
        guard let d = try? JSONSerialization.data(withJSONObject: obj) else { raw("{\"error\":\"unencodable reply\"}"); return }
        raw(String(decoding: d, as: UTF8.self))
    }

    /// One JSON line out.
    static func raw(_ json: String) {
        out.write(Data((json + "\n").utf8))
    }

    static func r2(_ x: CGFloat) -> Double { (Double(x) * 100).rounded() / 100 }

    static func settle(_ completion: @escaping (Double?) -> Void) {
        runtime.agent("{\"op\":\"settle\"}") { json in
            guard let d = json.data(using: .utf8),
                  let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any]
            else { completion(nil); return }
            completion(o["settle"] as? Double)
        }
    }

    /// Move both clocks to one instant: the runner's (timers, each fired at
    /// its own due time) and the motion engine's (a seek). The clock lands
    /// where the runner says (`batch.clock`): a timer's refusal stops it at
    /// that timer's due time and is the reply's error. `settle` is a fixed
    /// point: advance to when the last transition in flight ends, and if
    /// the timers crossed on the way started more, again — bounded, and
    /// `settled: false` when the bound is hit.
    static func clock(_ req: [String: Any], completion: @escaping ([String: Any]) -> Void) {
        let from = agentClock ?? 0
        let shouldSettle = req["settle"] as? Bool == true
        if !shouldSettle {
            guard let to = req["to"] as? Double, to.isFinite else { completion(["error": "clock needs \"to\" (ms) or \"settle\": true"]); return }
            guard to >= from else { completion(["error": "the clock cannot go backwards (\(from) → \(to))"]); return }
            advanceClock(to: to, settle: false, rounds: 0, deadline: Date(), completion: completion)
            return
        }
        let deadline = Date(timeIntervalSinceNow: 20)
        waitForReplies(until: deadline) { settled in
            guard settled else { completion(["clock": from, "settled": false]); return }
            Agent.settle { candidate in
                advanceClock(to: max(from, candidate ?? from), settle: true, rounds: 0, deadline: deadline, completion: completion)
            }
        }
    }

    private static func advanceClock(to: Double, settle shouldSettle: Bool, rounds: Int, deadline: Date, completion: @escaping ([String: Any]) -> Void) {
        runtime.advance(now: to) { batch in
            apply(batch)
            let landed = batch.clock ?? to
            agentClock = landed
            runtime.tick(now: landed) { motionBatch in
                apply(motionBatch)
                if let e = batch.error { completion(["error": "clock: \(e)", "clock": landed]); return }
                guard shouldSettle else { completion(["clock": landed]); return }
                guard rounds < 16 else { completion(["clock": landed, "settled": false]); return }
                pendingCount { count in
                    let continueAtSettle = {
                        Agent.settle { candidate in
                            let next = max(landed, candidate ?? landed)
                            if next <= landed { completion(["clock": landed, "settled": true]) }
                            else { advanceClock(to: next, settle: true, rounds: rounds + 1, deadline: deadline, completion: completion) }
                        }
                    }
                    if count == 0 { continueAtSettle(); return }
                    waitForReplies(until: deadline) { allIn in
                        if allIn { continueAtSettle() }
                        else { completion(["clock": landed, "settled": false]) }
                    }
                }
            }
        }
    }

    /// How many requests the runner has in flight (`state.pending`).
    static func pendingCount(_ completion: @escaping (Int) -> Void) {
        runtime.agent("{\"op\":\"state\"}") { json in
            guard let d = json.data(using: .utf8),
                  let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any]
            else { completion(0); return }
            completion((o["pending"] as? [Any])?.count ?? 0)
        }
    }

    /// Wait without blocking main. The executor's wake queues `pump` on the
    /// runtime owner; this polls only the runner's pending set.
    static func waitForReplies(until deadline: Date, completion: @escaping (Bool) -> Void) {
        pendingCount { count in
            if count == 0 { completion(true); return }
            if Date() >= deadline { completion(false); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.02) {
                waitForReplies(until: deadline, completion: completion)
            }
        }
    }
}
