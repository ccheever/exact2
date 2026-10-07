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
#if os(macOS)
import AppKit
#endif

public final class Agent {
    #if os(iOS) || os(tvOS)
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

    /// Inline IDs name shaped fragments, not their paragraph owner's box.
    /// Pick a visible fragment's midpoint; a wrapped link can have an earlier
    /// fragment offscreen while a later one is reachable. `box` carries canvas
    /// placement and presentation transforms into the viewport coordinates.
    func tapPoint(_ request: [String: Any], node: NodeView) -> CGPoint? {
        let bounds: CGRect
        if let id = request["id"] as? UInt32, let run = presenter.inlineText(id) {
            let fragments = node.inlineRects(run).filter { !$0.isEmpty }.map { box(node, region: $0) }
            let viewport = CGRect(origin: .zero, size: presenter.viewport.bounds.size)
            guard let fragment = fragments.first(where: { viewport.contains(CGPoint(x: $0.midX, y: $0.midY)) }) ?? fragments.first else { return nil }
            bounds = fragment
        } else { bounds = box(node) }
        // `at` is target-relative (a mouse click, a context menu). `x`/`y` are viewport points.
        if let at = request["at"] as? [Double], at.count == 2, at.allSatisfy(\.isFinite) {
            return CGPoint(x: bounds.minX + at[0], y: bounds.minY + at[1])
        }
        return CGPoint(x: request["x"] as? Double ?? bounds.midX, y: request["y"] as? Double ?? bounds.midY)
    }

    /// The one contact the driver may hold across requests (LLP 1035.003
    /// D1): where it is, in the viewport's space, while the button is down.
    /// `nil` between contacts. AppKit holds it as a real mouse button; UIKit
    /// cannot hold one and says so.
    var contact: CGPoint? = nil
    #if os(macOS)
    /// The held contact's event time, seconds on `systemUptime`'s clock.
    var contactClock: Double = 0
    /// The modifiers held through the contact: its `down`'s, until a
    /// `move` or `up` names others.
    var contactFlags: NSEvent.ModifierFlags = []
    #endif
    weak var canvasContact: NodeView?
    /// The last point the agent's pointer sent its canvas (iOS), for its motion.
    var canvasPoint: CGPoint?
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
        DispatchQueue.main.async { exitAfterStorage() }
    }

    /// The drive ended: storage an answer started lands first, as a quit's
    /// does (LLP 1097 D10), within the driver's patience (it kills at 2 s).
    nonisolated(unsafe) static var exiting: StorageHold?
    static func exitAfterStorage() {
        let hold = StorageHold(bound: 1.5, pending: { routes.contains { $0.1.storageOperations > 0 } }, begin: { _ in }, end: { exit(0) })
        if hold.hold() { exiting = hold } else { exit(0) }
    }

    /// One line: the session it names (or the default), then its operation.
    static func handle(_ line: String) {
        guard let data = line.data(using: .utf8),
              let req = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let op = req["op"] as? String
        else { reply(["error": "unreadable request: \(line)"]); return }
        if op == "quit" { exitAfterStorage(); return }
        // A slice building off main lands before the agent reads or acts
        // (LLP 1072 T9); under the agent slices are synchronous, so this is
        // for a carrier attached to an ordinary run.
        ExactSession.drainAll()
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
        case "tree" where req["ax"] as? Bool == true: Agent.reply(accessibilityElementsTree(req)) // LLP 1080.002
        case "tree": Agent.reply(session.canvases.decorate(req, decorateTree(session.natives.decorate(session.webviews.tree(line)))))
        case "layout": Agent.reply(tagged(inspectLayout(req) ?? layout(req))) // LLP 1080.001: `native`, `agree`
        // A call that moved something settles the canvases before it
        // replies (LLP 1012's fixed point; LLP 1014 D5 reads placements
        // after a frame, so the frame is rendered here, not left to the
        // display link to get to between two calls).
        case "tap":
            var r: [String: Any]
            #if os(macOS)
            let wasOpen = presenter.viewport.window?.isVisible == true
            #endif
            if req["close"] != nil {
                // The window's close button, as ⌘W and File ▸ Close Window
                // press it: an input, as `resize` is, so a `beforeunload`
                // flow can be driven (the agent's window is never key, so a
                // ⌘W it typed would go nowhere).
                guard req.keys.allSatisfy({ ["op", "session", "close"].contains($0) }), req["close"] as? Bool == true else {
                    Agent.reply(["error": "tap close takes no other input fields"]); return
                }
                #if os(macOS)
                r = closeWindow()
                #else
                r = ["error": "unsupported: an iOS app closes no window; close drives a macOS window or the browser's page (`beforeunload`)"]
                #endif
            } else if req["resize"] != nil {
                // LLP 1041 §8's opt-in diagnostic is an input variant, not a
                // ninth operation. Reject ambiguous input before touching UI.
                guard req.keys.allSatisfy({ ["op", "session", "resize"].contains($0) }),
                      let size = Agent.resizeSize(req["resize"]!) else {
                    Agent.reply(["error": Agent.resizeError]); return
                }
                #if os(macOS)
                r = resizeWindow(size)
                #else
                // The device sets an iOS app's viewport: there is no window to resize.
                r = ["error": "unsupported: an iOS app's viewport is the device's screen; resize drives a macOS window, the Linux presenter or the browser"]
                #endif
            } else if let action = req["mediaSession"] as? String {
                r = mediaSessionTap(req, action: action) // LLP 1098 D10, never a press
            } else if let into = req["into"] as? [String: Any] {
                r = intoView(req, into)
            } else { r = session.canvases.releaseContact(req) ?? tap(req) }
            #if os(macOS)
            // A press the app answered with `close()` (a "Don't Save") took
            // the window, and its session with it: the reply says so, as
            // `close`'s does, since nothing is left to read after it.
            if wasOpen, r["error"] == nil, req["close"] == nil, presenter.viewport.window?.isVisible != true { r["closed"] = true }
            #endif
            session.canvases.settle(now: session.now())
            Agent.reply(tagged(r))
        case "type": let r = releaseCanvasKey(req) ?? type(req); session.canvases.settle(now: session.now()); Agent.reply(tagged(r))
        case "reveal": Agent.reply(tagged(reveal(req))) // before a tap or a type: a target out of view, scrolled into it
        case "clock": let r = clock(req); session.tellAgentOffset(); Agent.reply(tagged(r))
        // A fetch fault (LLP 1103) is the runner's, below; the device facts are this host's.
        case "prefer" where req["faults"] == nil: Agent.reply(tagged(prefer(req)))
        case "screenshot": Agent.reply(tagged(screenshot(req)))
        case "sample": Agent.reply(tagged(sample(req)))
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
            nativeSections["mediaSession"] = mediaSessionState() // LLP 1098 D10
            // The drive's app storage (trivia F7): none unless it names a scratch store.
            nativeSections["storage"] = ExactEnv.environment["EXACT_AGENT_STORAGE"].map { ["available": true, "store": $0] as [String: Any] }
                ?? ["available": false, "code": "agent", "message": "storage is unavailable in agent mode unless the drive names a scratch store (--storage <name>)"]
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
            // A session that plays fills in its output (LLP 1096 D10); under the agent none plays.
            if !ExactEnv.agentMode, let output = try? JSONSerialization.data(withJSONObject: session.sound.state) { reply = reply.replacingOccurrences(of: "\"output\":\"agent\"", with: "\"output\":" + String(decoding: output, as: UTF8.self)) }
            if reply.hasSuffix("}"), !reply.hasPrefix("{\"error\""),
               let sections = try? JSONSerialization.data(withJSONObject: nativeSections) {
                reply.removeLast()
                let tail = String(decoding: sections, as: UTF8.self)
                reply += "," + tail.dropFirst()
            }
            Agent.raw(reply)
        // `perf frames` is this host's (LLP 1079 D4); `perf <target>` is the runner's, below.
        case "perf" where req["frames"] as? Bool == true:
            Agent.reply(session.clock != nil ? ["virtual": true] : session.sampler?.reply(late: req["late"] as? Int ?? 20) ?? ["unavailable": true])
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

    /// A field's value as a reply shows it (#134): a password's, when not
    /// empty, is a fixed mark whatever its length — the runner's
    /// `agent::MASKED`, which its tree already shows.
    static func shownValue(_ value: String, of v: NodeView) -> String {
        v.props["type"] == "password" && !value.isEmpty ? "•••" : value
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
    /// where the runner says (`batch.clock`), never behind where it stood
    /// (LLP 1080.000 §12): a timer's refusal stops it there and is the
    /// reply's error; the timer-fire limit is progress. `settle` is a fixed
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
    /// `fold` (LLP 1078 D7): the posture and the segment grid a host without
    /// a fold makes by splitting its viewport evenly; a device with a fold
    /// refuses them — it decides.
    func prefer(_ req: [String: Any]) -> [String: Any] {
        let media = req["media"] as? [String: String], page = req["page"] as? [String: Any], fold = req["fold"] as? [String: Any]
        guard media != nil || page != nil || fold != nil else { return ["error": "prefer needs media, page or fold: {\"prefers-reduced-motion\": \"reduce\", …}"] }
        if let fold, let refused = preferFold(fold) { return ["error": refused] }
        var (motion, transparency, contrast) = (DisplayPreferences.reducedMotion, DisplayPreferences.reducedTransparency, DisplayPreferences.contrast)
        var dark: Bool?
        var (gamut, high) = (DisplayPreferences.gamut, DisplayPreferences.highDynamicRange)
        for (name, value) in media ?? [:] {
            switch (name, value) {
            case ("prefers-reduced-motion", "reduce"), ("prefers-reduced-motion", "no-preference"): motion = value == "reduce"
            case ("prefers-reduced-transparency", "reduce"), ("prefers-reduced-transparency", "no-preference"): transparency = value == "reduce"
            case ("prefers-contrast", "more"), ("prefers-contrast", "less"), ("prefers-contrast", "custom"), ("prefers-contrast", "no-preference"): contrast = value
            case ("prefers-color-scheme", "light"), ("prefers-color-scheme", "dark"): dark = value == "dark"
            // @ref LLP 1100 D9
            case ("color-gamut", "srgb"), ("color-gamut", "p3"), ("color-gamut", "rec2020"): gamut = value
            case ("dynamic-range", "standard"), ("dynamic-range", "high"): high = value == "high"
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
            case ("can-open-files", "true"), ("can-open-files", "false"): facts.canOpenFiles = value == "true"
            case ("root-font-size", _) where (Double(value) ?? 0) > 0 && Double(value)!.isFinite: facts.rootFontSize = Double(value)!
            default: return ["error": "prefer: \(name): \(value) is not a page fact this host sets"]
            }
        }
        // The scheme first: the preferences' notification reads it.
        if let dark { systemScheme(dark: dark) }
        DisplayPreferences.agentContrast = contrast
        if DisplayPreferences.gamut != gamut || DisplayPreferences.highDynamicRange != high {
            DisplayRange.pinned = high ? 4 : 1
            DisplayPreferences.agentGamut = gamut
            session.rasters.displayChanged()
        }
        systemContrast(more: DisplayPreferences.contrast == "more")
        DisplayPreferences.agent = (motion, transparency)
        if page != nil { PageFacts.agent = facts }
        let keyword = { (on: Bool) in on ? "reduce" : "no-preference" }
        return ["media": ["prefers-reduced-motion": keyword(DisplayPreferences.reducedMotion),
                          "prefers-reduced-transparency": keyword(DisplayPreferences.reducedTransparency),
                          "prefers-contrast": DisplayPreferences.contrast,
                          "prefers-color-scheme": systemDark ? "dark" : "light",
                          "color-gamut": DisplayPreferences.gamut,
                          "dynamic-range": DisplayPreferences.highDynamicRange ? "high" : "standard"],
                "page": ["visibility-state": PageFacts.hidden ? "hidden" : "visible",
                         "online": PageFacts.onLine, "can-share": PageFacts.canShare, "can-open-files": PageFacts.canOpenFiles, "root-font-size": PageFacts.rootFontSize],
                "fold": presenter.fold.env]
    }

    /// The `fold` group: `posture` (`folded` | `continuous`), `cols` and
    /// `rows` (each at least 1), `gap` (points, 0 by default). Each refusal
    /// names its fact (LLP 1078 D10); nothing applies unless all are known.
    private func preferFold(_ fold: [String: Any]) -> String? {
        #if os(iOS) || os(tvOS)
        // The hinge interaction reports after the view attaches, a turn or two
        // after boot; a drive's first `prefer` can arrive before it. Give it a
        // bounded moment on a 27.1 device so the answer is the device's.
        let deadline = Date(timeIntervalSinceNow: 0.3)
        while ReservedRegions.available, !presenter.hingeReported, Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
        if presenter.hasFold { return "prefer: posture and segments: the device decides (it has a fold)" }
        #endif
        var next = presenter.fold
        var gap: CGFloat = 0, grid = false
        for (name, raw) in fold {
            let number = (raw as? NSNumber).map(\.doubleValue) ?? Double("\(raw)")
            switch name {
            case "posture":
                guard let p = raw as? String, p == "folded" || p == "continuous" else { return "prefer: posture: \(raw) is folded or continuous" }
                next.posture = p
            case "cols", "rows":
                guard let n = number, n.isFinite, n >= 1, n == n.rounded() else { return "prefer: segments: \(raw) \(name == "cols" ? "columns" : "rows") is not a count" }
                if name == "cols" { next.cols = Int(n) } else { next.rows = Int(n) }
                grid = true
            case "gap":
                guard let g = number, g.isFinite, g >= 0 else { return "prefer: segments: gap \(raw) is not a length" }
                gap = CGFloat(g); grid = true
            default: return "prefer: \(name) is not a fold fact this host sets"
            }
        }
        if grid {
            do { next.rects = try Segments.even(viewport: presenter.viewportSize, cols: next.cols, rows: next.rows, gap: gap) } catch { return "prefer: \(error)" }
        }
        return session.segments(next).map { "prefer: \($0)" }
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
        // @ref LLP 1080.000 §12 — platform timing, before any `clock`: the
        // host has run on the wall's time (motion and holds included) while
        // the runner's clock stood behind it. The clock is taken over at the
        // wall, frozen there before anything waits, and never set behind it:
        // a hold begun behind the motion engine's time is refused
        // (ClockWentBackwards). The runner catches up in the next seek. A
        // runner already ahead of the wall (no known path; the safe floor) sets the floor
        // instead. `take` is the takeover alone: it moves nothing and
        // reports where the clock stands.
        if session.clock == nil, !ExactEnv.agentFreezes {
            let runner = session.agent("{\"op\":\"tags\"}").data(using: .utf8)
                .flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }?["clock"] as? Double
            session.clock = max(session.now(), runner ?? 0)
            // The display's cadence means nothing under the agent's clock.
            session.sampler?.stop()
        }
        if req["take"] as? Bool == true { return ["clock": session.clock ?? 0] }
        let from = session.clock ?? 0
        // The end of an input (LLP 1012 §2): the `then`s of the answers it
        // settled land, the clock unmoved and no timer fired (Runner::land_then).
        if req["land"] as? Bool == true {
            let batch = session.runtime.landThen()
            session.apply(batch)
            session.apply(session.runtime.tick(now: from))
            if let e = batch.error { return ["error": "clock: \(e)", "clock": from] }
            return ["clock": from]
        }
        if req["data"] as? Bool == true { return landData(at: from) }
        let settle = req["settle"] as? Bool == true
        // A request in flight (LLP 1016) is waited for first: its reply
        // commits — and may start motion or ask for more — before the fixed
        // point is measured. The wake lands on the main queue, which the
        // run loop drains here. One bound for the whole call (LLP 1012 §2),
        // not one per round: a request that never answers ends the call at
        // twenty seconds, not sixteen times that.
        let deadline = Date(timeIntervalSinceNow: Agent.settleBound)
        if settle { waitForReplies(until: deadline) }
        // Settle ends motion: every leaf held mid-fling is made (LLP 1068 §5.1).
        if settle { presenter.leaves.settle() }
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
            let batch = advanceStepped(to: to, deadline: deadline, floor: from)
            let landed = max(from, batch.clock ?? to)
            session.clock = landed
            session.apply(session.runtime.tick(now: landed))
            AnimatedRasters.shared.evaluate()
            // A long catch-up (a takeover after minutes of uptime) can pass the
            // runner's timer-fire limit: it is progress, so go on from there.
            if let e = batch.error, "\(e)".contains("TimerFireLimit"), (batch.clock ?? to) < to, Date() < deadline { continue }
            if let e = batch.error { return ["error": "clock: \(e)", "clock": landed] }
            guard session.canvases.waitUntilReady() else { return ["error": "canvas creation is still in flight"] }
            session.canvases.settle(now: landed)
            // Every list reported and filled where it shows, nested ones
            // included, before the fixed point is read (LLP 1070 G3).
            if settle { presenter.settlePump() }
            world = session.canvases.clock(settle: settle)
            // A jump does not wait for what is still in flight on real time
            // (a store's, a worker's, the network's): the reply names how much,
            // as the web hosts' do (calendar F10, workout F6).
            guard settle else {
                var out = reply(landed)
                let inflight = pendingCount()
                if inflight > 0 { out["inflight"] = inflight }
                // The module's background storage still to land (LLP 1097 D9).
                let background = backgroundCount()
                if background > 0 { out["background"] = background }
                return out
            }
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
                    #if !(os(iOS) || os(tvOS))
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
    /// `floor`: the host's clock is never set behind it while the runner catches up (LLP 1080.000 §12).
    func advanceStepped(to: Double, deadline: Date, floor: Double = -.infinity) -> Batch {
        var steps = 0
        // The agent's clock is a seek: frame tasks fire virtual frames (LLP 1073 D3).
        session.runtime.presentFrames(false)
        while true {
            let waited = session.timerDue.map { $0 <= to } == true && waitForReplies(until: deadline)
            let held = waited && steps < 4096
            let batch = session.runtime.advance(now: to, untilRequest: held)
            session.apply(batch)
            session.clock = max(floor, batch.clock ?? to)
            // A stop at `to` may leave a timer due there: only a plain advance ends.
            if batch.error != nil || !held { return batch }
            steps += 1
        }
    }

    /// Images on screen land before the clock moves or a screenshot is
    /// taken, for up to 3 s: an animated one starts on the clock it lands at
    /// (LLP 1011.000), and a decode is host I/O no clock waits for otherwise.
    /// How many were still loading at the bound (`screenshot`'s `imagesPending`,
    /// as on the web, LLP 1054.000 R9).
    @discardableResult
    func waitForImages(until end: Date = Date(timeIntervalSinceNow: 3)) -> Int {
        var loading = session.rasters.loadingOnScreen
        while loading > 0 && Date() < end {
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
            loading = session.rasters.loadingOnScreen
        }
        AnimatedRasters.shared.evaluate()
        return loading
    }

    /// What a screenshot shows, settled: the pump and the images on screen
    /// in turn under one 3 s bound, since an image landing can relayout and
    /// mount rows with images of their own. How many were still loading
    /// after the last settle (`imagesPending`).
    func settleForPicture() -> Int {
        let end = Date(timeIntervalSinceNow: 3)
        while true {
            presenter.settlePump()
            let loading = session.rasters.loadingOnScreen
            if loading == 0 || Date() >= end { return loading }
            waitForImages(until: end)
        }
    }

    /// `clock data`: the app's data lands — its deferred module activated
    /// (the turn after first draw) and every request in flight answered,
    /// each answer's `then` landed — at the clock as it stands, no timer
    /// fired. A test's first step waits for it (habits, pomodoro, kanban:
    /// storage opened after the first step, which then read the placeholder).
    func landData(at from: Double) -> [String: Any] {
        let deadline = Date(timeIntervalSinceNow: Agent.settleBound)
        for _ in 0..<16 {
            while !session.dataActivated && Date() < deadline { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02)) }
            if !session.dataActivated { return ["clock": from, "settled": false, "reason": "data"] }
            if !waitForReplies(until: deadline) { return ["clock": from, "settled": false, "reason": "requests"] }
            let batch = session.runtime.landThen()
            session.apply(batch)
            session.apply(session.runtime.tick(now: from))
            if let e = batch.error { return ["error": "clock: \(e)", "clock": from] }
            // A `then` that sent asks again; what it sends lands in the next round.
            if pendingCount() == 0 { return ["clock": from, "settled": true] }
        }
        return ["clock": from, "settled": false, "reason": "requests"]
    }

    /// The background's storage operations queued or in flight
    /// (`state.background`, LLP 1097 D8).
    func backgroundCount() -> Int {
        guard let d = session.agent("{\"op\":\"state\"}").data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any],
              let b = o["background"] as? [String: Any] else { return 0 }
        return (b["queued"] as? Int ?? 0) + (b["inFlight"] as? Int ?? 0)
    }

    /// How many requests the runner has in flight (`state.pending`).
    func pendingCount() -> Int {
        guard let d = session.agent("{\"op\":\"state\"}").data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else { return 0 }
        // A Canvas 2D image being decoded is a reply still to come (LLP 1056 D9).
        // A held device request is not I/O in flight (LLP 1069.007 D3).
        let inFlight = (o["pending"] as? [[String: Any]])?.filter { $0["device"] == nil }.count ?? 0
        #if os(iOS) || os(tvOS)
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

public extension ExactEnv {
    /// `EXACT_AGENT_CHROME=platform` (opt-in; splitter rough 4, 11): under
    /// the agent, the native navigation bar and tab bar show as a person
    /// sees them, their items pressing the authored controls they stand for.
    /// The default paints the authored header and tablist in their place.
    static let agentChrome = environment["EXACT_AGENT_CHROME"] ?? "agent"
    /// Whether the authored header and tablist paint instead of UIKit's
    /// bars: the agent's default chrome.
    static let authoredChrome = agentMode && agentChrome != "platform"
}
