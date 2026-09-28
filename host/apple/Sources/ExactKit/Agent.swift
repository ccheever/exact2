// The agent API's presenter half, the part both presenters share (LLP
// 1012). Requests arrive as JSON lines on a stream the platform opened —
// stdin on macOS, a Unix socket on iOS (a simulator app has no stdin) —
// and are answered in order on the main thread; the clock is the last
// `clock` value: no timer advances the runner, events carry the agent's
// time, the engine is seeked to it. `tree`, `state`, `logs`, and `settle`
// go to the library (`exact_agent`); `clock` moves both clocks here, and
// `prefer` sets the display preferences (LLP 1061 D5);
// `layout`, `tap`, `type`, and `screenshot` are the platform's
// (`AgentMac.swift`, `AgentIOS.swift`), being about what it renders and
// its input path. One `Agent` per session (LLP 1031 D9); the carrier
// routes a request to a session by its host-owned `session` label when
// there is more than one — routing, not a tenth operation.
import Foundation
import CoreFoundation

public final class Agent {
    #if os(iOS)
    // An agent-issued edit awaits its actual editor's native caret reveal.
    weak var pendingTextReveal: TextArea?
    /// The held contact on a `pan` node, recognized (LLP 1057 §10.6).
    var panContact: AgentPan?
    #endif
    let session: ExactSession
    init(session: ExactSession) {
        self.session = session
        // The system appearance is the agent's from its first operation:
        // light, as `prefer` would set it, never the machine's (LLP 1069.007
        // D2). Reduced motion and transparency start at no-preference
        // (`DisplayPreferences.agent`).
        if ExactEnv.agentMode { systemScheme(dark: false) }
    }

    /// The one contact the driver may hold across requests (LLP 1035.003
    /// D1): where it is, in the viewport's space, while the button is down.
    /// `nil` between contacts. AppKit holds it as a real mouse button; UIKit
    /// cannot hold one and says so.
    var contact: CGPoint? = nil
    weak var canvasContact: NodeView?
    var keyReleases: [String: () -> [String: Any]] = [:]

    /// Where replies go: the stream the requests came on.
    nonisolated(unsafe) static var out = FileHandle.standardOutput

    /// The sessions a carrier routes among, by label; the first is the default.
    nonisolated(unsafe) static var routes: [(String, ExactSession)] = []

    /// A session that joined after the carrier started (a document's own
    /// window, LLP 1069.010 D4) routes by its label from now on; `nil`
    /// forgets the label when its window closes.
    public static func route(_ label: String, _ session: ExactSession?) {
        routes.removeAll { $0.0 == label }
        if let session { routes.append((label, session)) }
    }

    /// What the adapter that owns the windows observes, added to every
    /// session's `state` (its windows and their sessions, Open Recent):
    /// observations, never a second model.
    nonisolated(unsafe) public static var hostState: (() -> [String: Any])?

    /// Serve requests from `fd` until it closes — on the calling thread:
    /// each line is answered on the main thread before the next is read.
    /// The stream closing ends the process, and says so on stderr: a driver
    /// that saw the hangup reads why there, not a silent exit.
    public static func serve(fd: Int32) {
        var pending = Data()
        var buf = [UInt8](repeating: 0, count: 65536)
        while true {
            let n = read(fd, &buf, buf.count)
            if n < 0 && errno == EINTR { continue }
            if n < 0 { fputs("exact agent: read failed (\(String(cString: strerror(errno)))); exiting\n", stderr); break }
            if n == 0 { fputs("exact agent: the driver closed the connection; exiting\n", stderr); break }
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
        DispatchQueue.main.async { exit(0) }
    }

    /// One line: the session it names (or the default), then its operation.
    static func handle(_ line: String) {
        guard let data = line.data(using: .utf8),
              let req = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let op = req["op"] as? String
        else { reply(["error": "unreadable request: \(line)"]); return }
        if op == "quit" { exit(0) }
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
        if Agent.worldRequest(req) { Agent.reply(tagged(world(req))); return }
        // `tap @t` / `type @t` answer a held device request in the library,
        // before any view is looked up (LLP 1069.007 D4).
        if (op == "tap" || op == "type"), req["ticket"] != nil {
            var forward = req
            forward.removeValue(forKey: "session")
            let json = (try? JSONSerialization.data(withJSONObject: forward)).map { String(decoding: $0, as: UTF8.self) } ?? line
            let data = Data(session.agent(json).utf8)
            let answer = (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? ["error": "unreadable reply"]
            // A picker's answer is delivered once the library took it (LLP 1069.002 D9).
            session.picker.answered(answer, request: req)
            session.picker.saveAnswered(answer, request: req)
            session.picker.documentAnswered(answer, request: req)
            Agent.reply(tagged(answer))
            return
        }
        switch op {
        case "tree": Agent.reply(session.canvases.decorate(req, accessibilityTree(session.natives.decorate(session.webviews.tree(line)))))
        case "layout": Agent.reply(tagged(layout(req)))
        // A call that moved something settles the canvases before it
        // replies (LLP 1012's fixed point; LLP 1014 D5 reads placements
        // after a frame, so the frame is rendered here, not left to the
        // display link to get to between two calls).
        case "tap":
            let r: [String: Any]
            if req["resize"] != nil {
                // LLP 1041 §8's opt-in diagnostic is an input variant, not a
                // ninth operation. Reject ambiguous input before touching UI.
                guard req.keys.allSatisfy({ ["op", "session", "resize"].contains($0) }),
                      let size = Agent.resizeSize(req["resize"]!) else {
                    Agent.reply(["error": Agent.resizeError]); return
                }
                #if os(macOS)
                r = resizeWindow(size)
                #else
                r = ["error": "unsupported: resize input requires a macOS window or Linux presenter"]
                #endif
            } else if let into = req["into"] as? [String: Any] {
                r = intoView(req, into)
            } else { r = session.canvases.releaseContact(req) ?? tap(req) }
            session.canvases.settle(now: session.now())
            Agent.reply(tagged(r))
        case "type": let r = releaseCanvasKey(req) ?? type(req); session.canvases.settle(now: session.now()); Agent.reply(tagged(r))
        case "clock": let r = clock(req); session.tellAgentOffset(); Agent.reply(tagged(r))
        case "prefer": Agent.reply(tagged(prefer(req)))
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
            var nativeSections = stateSections()
            for (key, value) in Agent.hostState?() ?? [:] { nativeSections[key] = value }
            nativeSections["presence"] = presenter.presenceObservation()
            nativeSections["media"] = presenter.views.compactMap { id, view in view.video.map { ["id": id, "state": $0.state()] as [String: Any] } }
            var raster = session.rasters.diagnostics
            raster["encodedResolverBytes"] = session.app.resolver.encodedCacheBytes
            raster["encodedHTTPCache"] = RasterInput.httpCacheUsage
            raster["animated"] = AnimatedRasters.shared.diagnostics
            nativeSections["raster"] = raster
            #if os(macOS)
            nativeSections["contentRegion"] = session.regions.diagnostics
            nativeSections["readerParagraphs"] = session.text.readerParagraphs.values.map(\.diagnostics)
            #endif
            let world = session.canvases.worlds(["op": "state"])
            nativeSections = session.canvases.restoreReply(nativeSections)
            if !world.isEmpty { nativeSections["world"] = world }
            if reply.hasSuffix("}"), !reply.hasPrefix("{\"error\""),
               let sections = try? JSONSerialization.data(withJSONObject: nativeSections) {
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

    // Whole logical points, with an area ceiling to keep this diagnostic
    // from asking the software painter for arbitrarily large allocations.
    static let resizeError = "tap resize needs exactly two integer dimensions in 64...4096, area <= 8388608, and no other input fields"
    static func resizeSize(_ value: Any) -> CGSize? {
        guard let pair = value as? [NSNumber], pair.count == 2 else { return nil }
        let values = pair.map { $0.doubleValue }
        guard pair.allSatisfy({ CFGetTypeID($0) != CFBooleanGetTypeID() }),
              values.allSatisfy({ $0.isFinite && $0.rounded() == $0 && $0 >= 64 && $0 <= 4096 }),
              values[0] * values[1] <= 8_388_608 else { return nil }
        return CGSize(width: values[0], height: values[1])
    }

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
    /// `prefer` (LLP 1061 D5; LLP 1069.000 D6): the device facts by their
    /// web names, grouped as LLP 1069.007 D2 groups them. `media`: reduced
    /// motion, transparency and contrast stand in for the accessibility
    /// settings, for this process; the colour scheme is the system's
    /// appearance, beneath the app's own `setScheme`. `page`: what
    /// `exactPage()` answers. A fact not named stays as it is; nothing
    /// applies unless every one is known.
    func prefer(_ req: [String: Any]) -> [String: Any] {
        let media = req["media"] as? [String: String], page = req["page"] as? [String: Any]
        guard media != nil || page != nil else { return ["error": "prefer needs media or page: {\"prefers-reduced-motion\": \"reduce\", …}"] }
        var (motion, transparency, contrast) = (DisplayPreferences.reducedMotion, DisplayPreferences.reducedTransparency, DisplayPreferences.contrast)
        var dark: Bool?
        for (name, value) in media ?? [:] {
            switch (name, value) {
            case ("prefers-reduced-motion", "reduce"), ("prefers-reduced-motion", "no-preference"): motion = value == "reduce"
            case ("prefers-reduced-transparency", "reduce"), ("prefers-reduced-transparency", "no-preference"): transparency = value == "reduce"
            case ("prefers-contrast", "more"), ("prefers-contrast", "less"), ("prefers-contrast", "custom"), ("prefers-contrast", "no-preference"): contrast = value
            case ("prefers-color-scheme", "light"), ("prefers-color-scheme", "dark"): dark = value == "dark"
            default: return ["error": "prefer: \(name): \(value) is not a preference this host sets"]
            }
        }
        var facts = PageFacts.agent
        for (name, raw) in page ?? [:] {
            let value = name == "root-font-size" ? "\(raw)" : (raw as? Bool).map { $0 ? "true" : "false" } ?? "\(raw)"
            switch (name, value) {
            case ("visibility-state", "visible"), ("visibility-state", "hidden"): facts.hidden = value == "hidden"
            case ("online", "true"), ("online", "false"): facts.onLine = value == "true"
            case ("can-share", "true"), ("can-share", "false"): facts.canShare = value == "true"
            case ("root-font-size", _) where (Double(value) ?? 0) > 0 && Double(value)!.isFinite: facts.rootFontSize = Double(value)!
            default: return ["error": "prefer: \(name): \(value) is not a page fact this host sets"]
            }
        }
        // The scheme first: the preferences' notification reads it.
        if let dark { systemScheme(dark: dark) }
        DisplayPreferences.agentContrast = contrast
        DisplayPreferences.agent = (motion, transparency)
        if page != nil { PageFacts.agent = facts }
        let keyword = { (on: Bool) in on ? "reduce" : "no-preference" }
        return ["media": ["prefers-reduced-motion": keyword(DisplayPreferences.reducedMotion),
                          "prefers-reduced-transparency": keyword(DisplayPreferences.reducedTransparency),
                          "prefers-contrast": DisplayPreferences.contrast,
                          "prefers-color-scheme": systemDark ? "dark" : "light"],
                "page": ["visibility-state": PageFacts.hidden ? "hidden" : "visible",
                         "online": PageFacts.onLine, "can-share": PageFacts.canShare, "root-font-size": PageFacts.rootFontSize]]
    }

    /// `tap <list> into <key>` (LLP 1070.000 §5): the runner's request on a
    /// mounted list, committed, then settled as `clock settle` settles it.
    func intoView(_ req: [String: Any], _ into: [String: Any]) -> [String: Any] {
        guard let v = view(req) else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        let key = into["key"] as? String ?? (into["key"].map { "\($0)" } ?? "")
        session.apply(session.runtime.intoView(v.id, key: key, block: into["block"] as? String ?? "start",
                                               inline: into["inline"] as? String ?? "nearest"))
        presenter.settlePump()
        return ["tapped": Int(v.id), "into": into]
    }

    func clock(_ req: [String: Any]) -> [String: Any] {
        let from = session.clock ?? 0
        let settle = req["settle"] as? Bool == true
        // A request in flight (LLP 1016) is waited for first: its reply
        // commits — and may start motion or ask for more — before the fixed
        // point is measured. The wake lands on the main queue, which the
        // run loop drains here. One bound for the whole call (LLP 1012 §2),
        // not one per round: a request that never answers ends the call at
        // twenty seconds, not sixteen times that.
        let deadline = Date(timeIntervalSinceNow: Agent.settleBound)
        if settle { waitForReplies(until: deadline) }
        #if os(iOS)
        // Settle ends motion: every leaf held mid-fling is made (LLP 1068 §5.1).
        if settle { presenter.leaves.settle() }
        #endif
        waitForImages()
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
            let batch = advanceStepped(to: to, deadline: deadline)
            let landed = batch.clock ?? to
            session.clock = landed
            session.apply(session.runtime.tick(now: landed))
            AnimatedRasters.shared.evaluate()
            if let e = batch.error { return ["error": "clock: \(e)", "clock": landed] }
            guard session.canvases.waitUntilReady() else { return ["error": "canvas creation is still in flight"] }
            session.canvases.settle(now: landed)
            // Every list reported and filled where it shows, nested ones
            // included, before the fixed point is read (LLP 1070 G3).
            if settle { presenter.settlePump() }
            world = session.canvases.clock(settle: settle)
            guard settle else { return reply(landed) }
            if pendingCount() > 0 {
                rounds += 1
                if rounds >= 16 || Date() >= deadline { return reply(landed, false, reason: "requests") }
                waitForReplies(until: deadline)
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
                // A held device request is never waited on: the fixed point
                // stands, and the agent hears what waits for it (LLP 1069.007 D3).
                if let d = session.agent("{\"op\":\"holds\"}").data(using: .utf8),
                   let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any],
                   let holds = o["holds"] as? [Any], !holds.isEmpty {
                    var out = reply(landed, false, reason: "device")
                    out["tickets"] = o["tickets"]
                    return out
                }
                return reply(landed, true)
            }
            rounds += 1
            if rounds >= 16 { return reply(landed, false) }
            to = next
        }
    }

    /// To `to`, and what is in flight lands before a timer fires — the
    /// runner keeps one request per target (LLP 1016 D5), so a tick's send
    /// would drop the reply of the one before it: the jump stops after each
    /// timer that sends, and its reply is waited for. Past the deadline, or
    /// 4096 stops, the rest is one advance. Each batch is applied; the last
    /// one is returned.
    func advanceStepped(to: Double, deadline: Date) -> Batch {
        var steps = 0
        while true {
            let waited = session.timerDue.map { $0 <= to } == true && waitForReplies(until: deadline)
            let held = waited && steps < 4096
            let batch = session.runtime.advance(now: to, untilRequest: held)
            session.apply(batch)
            session.clock = batch.clock ?? to
            // A stop at `to` may leave a timer due there: only a plain advance ends.
            if batch.error != nil || !held { return batch }
            steps += 1
        }
    }

    /// Images on screen land before the clock moves, for up to 3 s: an
    /// animated one starts on the clock it lands at (LLP 1011.000), and a
    /// decode is host I/O no clock waits for otherwise.
    func waitForImages() {
        let end = Date(timeIntervalSinceNow: 3)
        while session.rasters.loadingOnScreen > 0 && Date() < end { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01)) }
        AnimatedRasters.shared.evaluate()
    }

    /// How many requests the runner has in flight (`state.pending`).
    func pendingCount() -> Int {
        guard let d = session.agent("{\"op\":\"state\"}").data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else { return 0 }
        // A Canvas 2D image being decoded is a reply still to come (LLP 1056 D9).
        // A held device request is not I/O in flight (LLP 1069.007 D3).
        let inFlight = (o["pending"] as? [[String: Any]])?.filter { $0["device"] == nil }.count ?? 0
        #if os(iOS)
        // A filtered SVG picture being drawn off the main thread (LLP 1055.000 D14).
        let pictures = SvgFilterLive.inFlight
        #else
        let pictures = 0
        #endif
        return inFlight + session.presenter.canvas2d.loadingCount + pictures
    }

    /// `clock settle`'s bound on requests in flight: a network's worth.
    static let settleBound: TimeInterval = 20

    /// Pump the executor's queue until no request is in flight, or until
    /// the call's deadline (`settled: false` past it), false then. The wake's
    /// own pump is a main-queue block, and this runs inside one — so the
    /// queue is drained here directly, the run loop turning in between for
    /// the executor's thread to make progress.
    @discardableResult
    func waitForReplies(until deadline: Date) -> Bool {
        while pendingCount() > 0 {
            if Date() >= deadline { return false }
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02))
            session.apply(session.runtime.pump(now: session.now()))
        }
        return true
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
