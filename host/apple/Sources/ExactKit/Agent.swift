// The agent API's presenter half, the part both presenters share (LLP
// 1012). Requests arrive as JSON lines on a stream the platform opened —
// stdin on macOS, a Unix socket on iOS (a simulator app has no stdin) —
// and are answered in order on the main thread. Explicit ownership chooses
// live time or a controlled clock; read-only requests never acquire it.
// `tree`, `state`, `logs`, and `settle`
// go to the library (`exact_agent`); `clock` moves both clocks here;
// `layout`, `tap`, `type`, and `screenshot` are the platform's
// (`AgentMac.swift`, `AgentIOS.swift`), being about what it renders and
// its input path. One `Agent` per session (LLP 1031 D9); the carrier
// routes a request to a session by its host-owned `session` label when
// there is more than one — routing, not a ninth operation.
import Foundation
import CoreFoundation
#if os(iOS)
import UIKit
#else
import AppKit
#endif

public final class Agent {
    #if os(iOS)
    // An agent-issued edit awaits its actual editor's native caret reveal.
    weak var pendingTextReveal: TextArea?
    #endif
    let session: ExactSession
    init(session: ExactSession) { self.session = session }

    /// The one contact the driver may hold across requests (LLP 1035.003
    /// D1): where it is, in the viewport's space, while the button is down.
    /// `nil` between contacts. AppKit holds it as a real mouse button; UIKit
    /// cannot hold one and says so.
    var contact: CGPoint? = nil
    weak var canvasContact: NodeView?
    var keyReleases: [String: () -> [String: Any]] = [:]

    /// Where replies go: the stream the requests came on.
    nonisolated(unsafe) static var out = FileHandle.standardOutput

    nonisolated(unsafe) static var keepAliveOnEOF = false
    nonisolated(unsafe) static var carrierConnected = false

    /// The sessions a carrier routes among, by label; the first is the default.
    nonisolated(unsafe) static var routes: [(String, ExactSession)] = []

    /// Serve requests from `fd` until it closes — on the calling thread:
    /// each line is answered on the main thread before the next is read.
    /// EOF exits an isolated launch unless an acknowledged handoff requested live play.
    public static func serve(fd: Int32) {
        var pending = Data()
        var buf = [UInt8](repeating: 0, count: 65536)
        while true {
            let n = read(fd, &buf, buf.count)
            if n <= 0 { break }
            pending.append(buf, count: n)
            while let i = pending.firstIndex(of: UInt8(ascii: "\n")) {
                let line = String(decoding: pending[pending.startIndex..<i], as: UTF8.self)
                pending.removeSubrange(pending.startIndex...i)
                // A synchronous main-queue block prevents nested run-loop waits
                // from servicing main-queue completions (notably WK snapshots on
                // a device). Common modes also service requests while UIKit or
                // AppKit tracks a held gesture; default-only waits for its release.
                // Still one request at a time, including inside nested run loops.
                let completed = DispatchSemaphore(value: 0)
                CFRunLoopPerformBlock(CFRunLoopGetMain(), CFRunLoopMode.commonModes.rawValue) {
                    handle(line)
                    completed.signal()
                }
                CFRunLoopWakeUp(CFRunLoopGetMain())
                completed.wait()
            }
        }
        DispatchQueue.main.async {
            let keepPlaying = disconnected()
            if !keepPlaying { exit(0) }
            close(fd)
        }
    }

    /// One line: the session it names (or the default), then its operation.
    static func handle(_ line: String) {
        carrierConnected = true
        guard let data = line.data(using: .utf8),
              let req = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let op = req["op"] as? String
        else { reply(["error": "unreadable request: \(line)"]); return }
        if op == "quit" { _ = disconnected(); exit(0) }
        let wanted = req["session"] as? String
        var target: ExactSession? = nil
        if let wanted {
            for (label, s) in routes where label == wanted { target = s }
        } else {
            target = routes.first?.1
        }
        guard let target else {
            reply(["error": wanted.map { "no session \($0)" } ?? "no session"])
            return
        }
        target.agentInstance.handle(op: op, req, line: line)
    }

    func handle(op: String, _ req: [String: Any], line: String) {
        // A destroyed session's handle is a stranger to the library (LLP
        // 1031 D2): every operation is refused by name, in the API's shape.
        if session.state == .destroyed {
            Agent.reply(["error": "session \(session.label): destroyed (runtime \(session.runtime.rt): no such runtime)"])
            return
        }
        guard session.canvases.waitUntilReady() else {
            Agent.reply(["error": "canvas creation is still in flight"])
            return
        }
        if ["tap", "type", "focus"].contains(op), session.clock == nil {
            Agent.reply(["error": "human owns input: explicitly acquire with clock owner:agent before driving", "ownership": session.ownership])
            return
        }
        // Ownership belongs to the session, including every world, even if a driver
        // retained an id/world selector from its preceding inspection.
        if op == "clock", req["owner"] != nil { Agent.reply(tagged(clock(req))); return }
        if Agent.worldRequest(req) { Agent.reply(tagged(world(req))); return }
        switch op {
        case "tree": Agent.reply(session.canvases.decorate(req, accessibilityTree(session.webviews.tree())))
        case "layout": Agent.reply(tagged(layout(req)))
        // A call that moved something settles the canvases before it
        // replies (LLP 1012's fixed point; LLP 1014 D5 reads placements
        // after a frame, so the frame is rendered here, not left to the
        // display link to get to between two calls).
        case "tap": let r = tap(req); session.canvases.settle(now: session.now()); Agent.reply(tagged(r))
        case "type": let r = releaseCanvasKey(req) ?? type(req); session.canvases.settle(now: session.now()); Agent.reply(tagged(r))
        case "clock": Agent.reply(tagged(clock(req)))
        case "screenshot": Agent.reply(tagged(screenshot(req)))
        case "logs":
            var forward = req
            forward.removeValue(forKey: "session")
            let json = (try? JSONSerialization.data(withJSONObject: forward)).map { String(decoding: $0, as: UTF8.self) } ?? line
            let data = Data(session.agent(json).utf8)
            let reply = (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? ["error": "unreadable logs"]
            Agent.reply(session.canvases.decorate(req, reply))
        case "state":
            // The runner's state, in its own order, then what this host
            // observes for the session (LLP 1035.002 D2): the focus, the
            // keyboard and the navigation — observations, never a second
            // model, appended by text so the runner's order is kept.
            var forward = req
            forward.removeValue(forKey: "session")
            let json = (try? JSONSerialization.data(withJSONObject: forward)).map { String(decoding: $0, as: UTF8.self) } ?? line
            var reply = session.agent(json)
            let world = session.canvases.worlds(["op": "state"])
            var extra = session.canvases.restoreReply(stateSections())
            extra["ownership"] = session.ownership
            extra["capabilities"] = capabilities
            if !world.isEmpty { extra["world"] = world }
            if reply.hasSuffix("}"), !reply.hasPrefix("{\"error\""),
               let sections = try? JSONSerialization.data(withJSONObject: extra) {
                reply.removeLast()
                let tail = String(decoding: sections, as: UTF8.self)
                reply += "," + tail.dropFirst()
            }
            Agent.raw(reply)
        default:
            // The library's operations take the request without the
            // carrier's routing field.
            var forward = req
            forward.removeValue(forKey: "session")
            let json = (try? JSONSerialization.data(withJSONObject: forward)).map { String(decoding: $0, as: UTF8.self) } ?? line
            Agent.raw(session.agent(json))
        }
    }

    /// Every reply carries the runner's `epoch`, `incarnation` and `clock`
    /// (LLP 1035.002 D3) — read after the operation, so a reply's tags name
    /// the world it left behind; a reply's own `clock` (where a `clock` call
    /// landed) is kept. An error is left alone.
    func tagged(_ r: [String: Any]) -> [String: Any] {
        let r = session.canvases.restoreReply(r)
        guard r["error"] == nil,
              let d = session.agent("{\"op\":\"tags\"}").data(using: .utf8),
              let tags = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else { return r }
        var out = r
        out["ownership"] = session.ownership
        for (key, value) in tags where out[key] == nil { out[key] = value }
        return out
    }

    public static func reply(_ obj: [String: Any]) {
        guard let d = try? JSONSerialization.data(withJSONObject: obj) else { raw("{\"error\":\"unencodable reply\"}"); return }
        raw(String(decoding: d, as: UTF8.self))
    }

    /// One JSON line out.
    static func raw(_ json: String) {
        out.write(Data((json + "\n").utf8))
    }

    static func r2(_ x: CGFloat) -> Double { (Double(x) * 100).rounded() / 100 }

    func settle() -> Double? {
        guard let d = session.agent("{\"op\":\"settle\"}").data(using: .utf8),
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
    func clock(_ req: [String: Any]) -> [String: Any] {
        if let owner = req["owner"] as? String {
            guard owner == "human" || owner == "agent" else { return ["error": "clock owner must be human or agent"] }
            guard req["detach"] as? Bool != true || owner == "human" else { return ["error": "detach requires owner human"] }
            if req["detach"] as? Bool == true {
                // The transport closes for all routed sessions. Acknowledge only
                // after every controlled session has released its inputs and time.
                var outcomes: [[String: Any]] = []
                for (label, target) in Agent.routes where target.state != .destroyed {
                    var outcome = target.agentInstance.handoff(owner: "human")
                    outcome["session"] = label; outcomes.append(outcome)
                }
                if Agent.routes.isEmpty { outcomes = [handoff(owner: "human")] }
                guard !outcomes.contains(where: { $0["worldHandoff"] != nil }) else {
                    return ["error": "detach: a loaded world did not acknowledge live ownership; close the isolated driver and relaunch normally",
                            "detached": false, "sessions": outcomes, "ownership": session.ownership]
                }
                Agent.keepAliveOnEOF = true
                return ["detached": true, "ownership": session.ownership, "sessions": outcomes,
                        "clock": session.now(), "reconnect": false]
            }
            return handoff(owner: owner)
        }
        guard let from = session.clock else {
            return ["error": "live clock: explicitly acquire with clock owner:agent before seeking", "ownership": session.ownership]
        }
        let settle = req["settle"] as? Bool == true
        // A request in flight (LLP 1016) is waited for first: its reply
        // commits — and may start motion or ask for more — before the fixed
        // point is measured. The wake lands on the main queue, which the
        // run loop drains here.
        if settle { waitForReplies() }
        var target = req["to"] as? Double
        if settle { target = max(from, self.settle() ?? from) }
        guard var to = target, to.isFinite else { return ["error": "clock needs \"to\" (ms) or \"settle\": true"] }
        guard to >= from else { return ["error": "the clock cannot go backwards (\(from) → \(to))"] }
        var rounds = 0
        var world = Canvases.WorldClock()
        func reply(_ landed: Double, _ settled: Bool? = nil, reason: String? = nil) -> [String: Any] {
            var out = world.reply
            out["clock"] = landed
            if let settled { out["settled"] = settled }
            if settled == false, let reason = world.pending ? "world" : reason { out["reason"] = reason }
            return out
        }
        while true {
            let batch = session.runtime.advance(now: to)
            session.apply(batch)
            let landed = batch.clock ?? to
            session.clock = landed
            session.apply(session.runtime.tick(now: landed))
            if let e = batch.error { return ["error": "clock: \(e)", "clock": landed] }
            guard session.canvases.waitUntilReady() else { return ["error": "canvas creation is still in flight"] }
            session.canvases.settle(now: landed)
            world = session.canvases.clock(settle: settle)
            guard settle else { return reply(landed) }
            if pendingCount() > 0 {
                rounds += 1
                if rounds >= 16 { return reply(landed, false) }
                waitForReplies()
                continue
            }
            let next = max(landed, self.settle() ?? landed, world.settleAt ?? landed)
            if next <= landed && !world.pending {
                // A responder or presentation completion can enqueue a keyboard
                // resize before its animation exists. Require an idle native turn
                // after work finishes, including work created by that completion.
                let deadline = Date(timeIntervalSinceNow: 2)
                var wasBusy = nativeInFlight()
                while true {
                    #if !os(iOS)
                    if !wasBusy { break }
                    #endif
                    RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02))
                    let busy = nativeInFlight()
                    if !wasBusy && !busy { break }
                    if Date() >= deadline { return reply(landed, false, reason: "transition") }
                    wasBusy = busy
                }
                return reply(landed, true)
            }
            rounds += 1
            if rounds >= 16 { return reply(landed, false) }
            to = next
        }
    }

    /// Release host bookkeeping before the engine discards held and queued input.
    func cancelOwnedInput() {
        let releases = keyReleases.values
        keyReleases.removeAll()
        for release in releases { _ = release() }
        for view in session.presenter.views.values {
            view.canvasInput?.blur()
            #if os(macOS)
            view.pressed = false
            #endif
        }
        #if os(macOS)
        // Clear the host's press latch first: lifting the platform mouse must not
        // activate a Save/Restart button that happened to be held at detachment.
        if contact != nil { _ = contact("up", [:]) }
        #endif
        contact = nil; canvasContact = nil
    }

    @discardableResult
    func handoff(owner: String) -> [String: Any] {
        let controlled = owner == "agent"
        let boundary = session.now()
        // Freeze cancellation callbacks at the boundary, then rebase live time.
        session.clock = boundary
        cancelOwnedInput()
        let worlds = session.canvases.handoff(owner: owner)
        if !controlled { session.clock = nil }
        session.frames.run(session.frames.motion || session.canvases.wantsFrames)
        #if os(iOS)
        if !controlled { UIApplication.shared.isIdleTimerDisabled = false }
        #endif
        var reply: [String: Any] = ["ownership": session.ownership, "clock": session.now(),
            "releasedInput": true, "rebased": true, "world": worlds]
        let refused = worlds.filter { $0["error"] != nil || $0["ownership"] == nil }
        if !refused.isEmpty { reply["worldHandoff"] = ["unavailable": true, "reason": "loaded surface did not acknowledge clock ownership", "world": refused] }
        return reply
    }

    /// Called on the main thread at EOF; safe even after a prior acknowledged handoff.
    @discardableResult
    static func disconnected() -> Bool {
        carrierConnected = false
        for (_, session) in routes where session.state != .destroyed {
            if session.clock != nil { _ = session.agentInstance.handoff(owner: "human") }
            else { session.agentBox?.cancelOwnedInput() }
        }
        return keepAliveOnEOF
    }

    var capabilities: [String: Any] {
        #if os(iOS)
        let host = "ios"
        let active = ExactEnv.environment["EXACT_AGENT_CONNECT"] == nil ? "unix-socket" : "outbound-tcp"
        let carriers = ["unix-socket", "outbound-tcp"]
        let delivery: [String: Any] = ["tap": "recognized", "canvasKey": "recognized", "physicalTouch": false, "heldContact": false]
        #else
        let host = "macos"
        let active = "stdio"
        let carriers = ["stdio"]
        let delivery: [String: Any] = ["tap": "platform", "canvasKey": "recognized", "heldContact": "platform"]
        #endif
        let compat = GpuModule.bakedCompatibility
        let gpu = (compat["embedded"] as? [String: Any])?["gpu"] ?? NSNull()
        return ["host": host, "carrier": ["active": Agent.carrierConnected ? active : "none", "available": carriers,
                    "scope": "launch", "reconnect": false, "eof": Agent.keepAliveOnEOF ? "live" : "exit", "delivery": delivery],
                "clockOwnership": true, "handoff": true, "detach": true,
                "inputProvenance": ["unavailable": true, "reason": "GPU input ABI does not carry source attestation"],
                "reload": ["ui": "restart-with-compatible-slot-carry", "worldOnUIReload": "reset",
                    "game": "rebuild-relaunch", "liveGameReplacement": false, "worldCarry": "explicit-save-restore"],
                "build": ["host": compat["id"] ?? NSNull(), "requestedGame": gpu,
                    "loadedGame": session.canvases.module == nil ? NSNull() : gpu,
                    "planDigest": ["unavailable": true], "lastSuccessfulSwap": NSNull(), "phase": "launch"]]
    }

    /// How many requests the runner has in flight (`state.pending`).
    func pendingCount() -> Int {
        guard let d = session.agent("{\"op\":\"state\"}").data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else { return 0 }
        return (o["pending"] as? [Any])?.count ?? 0
    }

    /// Pump the executor's queue until no request is in flight, or for at
    /// most twenty seconds (a network's worth; `settled: false` past it).
    /// The wake's own pump is a main-queue block, and this runs inside one
    /// — so the queue is drained here directly, the run loop turning in
    /// between for the executor's thread to make progress.
    func waitForReplies() {
        let deadline = Date(timeIntervalSinceNow: 20)
        while pendingCount() > 0 && Date() < deadline {
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02))
            session.apply(session.runtime.pump(now: session.now()))
        }
    }
}

extension ExactSession {
    /// This session's agent (made on first use).
    var agentInstance: Agent {
        if let a = agentBox { return a }
        let a = Agent(session: self)
        agentBox = a
        return a
    }
}
