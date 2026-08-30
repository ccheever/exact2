// The agent API's presenter half, the part both presenters share (LLP
// 1012). Requests arrive as JSON lines on a stream the platform opened —
// stdin on macOS, a Unix socket on iOS (a simulator app has no stdin) —
// and are answered in order on the main thread; the clock is the last
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
    /// each line is answered on the main thread before the next is read.
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
                DispatchQueue.main.sync { handle(line) }
            }
        }
        DispatchQueue.main.async { exit(0) }
    }

    static func handle(_ line: String) {
        guard let data = line.data(using: .utf8),
              let req = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let op = req["op"] as? String
        else { reply(["error": "unreadable request: \(line)"]); return }
        switch op {
        case "quit": exit(0)
        case "layout": reply(layout())
        // A call that moved something settles the canvases before it
        // replies (LLP 1012's fixed point; LLP 1014 D5 reads placements
        // after a frame, so the frame is rendered here, not left to the
        // display link to get to between two calls).
        case "tap": let r = tap(req); canvases.settle(now: now()); reply(r)
        case "type": let r = type(req); canvases.settle(now: now()); reply(r)
        case "clock": let r = clock(req); canvases.settle(now: now()); reply(r)
        case "screenshot": reply(screenshot(req))
        default: raw(Exact.agent(line))
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

    static func settle() -> Double? {
        guard let d = Exact.agent("{\"op\":\"settle\"}").data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else { return nil }
        return o["settle"] as? Double
    }

    /// Move both clocks to one instant: the runner's (timers, each fired at
    /// its own due time) and the motion engine's (a seek). The clock lands
    /// where the runner says (`batch.clock`): a timer's refusal stops it at
    /// that timer's due time and is the reply's error. `settle` is a fixed
    /// point: advance to when the last transition in flight ends, and if
    /// the timers crossed on the way started more, again — bounded, and
    /// `settled: false` when the bound is hit.
    static func clock(_ req: [String: Any]) -> [String: Any] {
        let from = agentClock ?? 0
        let settle = req["settle"] as? Bool == true
        var target = req["to"] as? Double
        if settle { target = max(from, Agent.settle() ?? from) }
        guard var to = target, to.isFinite else { return ["error": "clock needs \"to\" (ms) or \"settle\": true"] }
        guard to >= from else { return ["error": "the clock cannot go backwards (\(from) → \(to))"] }
        var rounds = 0
        while true {
            let batch = Exact.advance(now: to)
            apply(batch)
            let landed = batch.clock ?? to
            agentClock = landed
            apply(Exact.tick(now: landed))
            if let e = batch.error { return ["error": "clock: \(e)", "clock": landed] }
            guard settle else { return ["clock": landed] }
            let next = max(landed, Agent.settle() ?? landed)
            if next <= landed { return ["clock": landed, "settled": true] }
            rounds += 1
            if rounds >= 16 { return ["clock": landed, "settled": false] }
            to = next
        }
    }
}
