// The agent API's presenter half (LLP 1012). Under EXACT_AGENT=1 the driver
// (`scripts/agent.mjs`) owns this process: requests arrive as JSON lines on
// stdin, replies leave as JSON lines on stdout, and the clock is the last
// `clock` value — no timer advances the runner, events carry the agent's
// time, the engine is seeked to it. `tree`, `state`, `logs`, and `settle` go
// to the library (`exact_agent`); `layout` reads the views as they sit in
// the viewport, scroll folded in (the web's getBoundingClientRect); `tap`
// sends mouse events through the window — hit-testing and the responder
// chain, the path a click takes — or a wheel to the hit view; `type` puts
// text through the field editor; `screenshot` draws the viewport to a PNG
// (`window: true` asks the window server instead, which sees Metal layers).
import AppKit

enum Agent {
    /// Read requests off stdin on a thread; answer each on the main thread,
    /// in order, before reading the next. Stdin closing ends the process.
    static func start() {
        let t = Thread {
            while let line = readLine() {
                DispatchQueue.main.sync { handle(line) }
            }
            DispatchQueue.main.async { exit(0) }
        }
        t.start()
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
        default: print(Exact.agent(line))
        }
    }

    static func reply(_ obj: [String: Any]) {
        guard let d = try? JSONSerialization.data(withJSONObject: obj) else { print("{\"error\":\"unencodable reply\"}"); return }
        print(String(decoding: d, as: UTF8.self))
    }

    static func r2(_ x: CGFloat) -> Double { (Double(x) * 100).rounded() / 100 }

    /// A view's box in the viewport: the clip view's space, less its scroll
    /// origin — every enclosing scroll node's offset folded in — with the
    /// presentation transform (translate/scale/rotate on the layer) applied,
    /// as the web's `getBoundingClientRect` includes CSS transforms.
    static func box(_ v: NSView) -> NSRect {
        let clip = presenter.viewport.contentView
        // Under a child a canvas's surface has placed (LLP 1014 D5): the box
        // where it is seen, through the placement, not the kernel's.
        if let n = v as? NodeView, let placed = n.placedAncestor, let h = placed.placement, let overlay = placed.superview, let canvas = overlay.superview as? NodeView {
            let corners = [NSPoint(x: 0, y: 0), NSPoint(x: v.bounds.width, y: 0), NSPoint(x: v.bounds.width, y: v.bounds.height), NSPoint(x: 0, y: v.bounds.height)]
                .map { NodeView.map(h, placed.convert($0, from: v)) }
            let xs = corners.map { $0.x }, ys = corners.map { $0.y }
            let inCanvas = NSRect(x: xs.min()!, y: ys.min()!, width: xs.max()! - xs.min()!, height: ys.max()! - ys.min()!)
            let r = canvas.convert(inCanvas, to: clip)
            return NSRect(x: r.origin.x - clip.bounds.origin.x, y: r.origin.y - clip.bounds.origin.y, width: r.width, height: r.height)
        }
        let r = v.convert(v.bounds.applying(v.layer?.affineTransform() ?? .identity), to: clip)
        return NSRect(x: r.origin.x - clip.bounds.origin.x, y: r.origin.y - clip.bounds.origin.y, width: r.width, height: r.height)
    }

    static func layout() -> [String: Any] {
        let clip = presenter.viewport.contentView
        var nodes: [[String: Any]] = []
        for (id, v) in presenter.views.sorted(by: { $0.key < $1.key }) where v.window != nil {
            let r = box(v)
            var n: [String: Any] = ["id": Int(id), "x": r2(r.origin.x), "y": r2(r.origin.y), "w": r2(r.width), "h": r2(r.height)]
            if let sv = v.scroll { n["sx"] = r2(sv.contentView.bounds.origin.x); n["sy"] = r2(sv.contentView.bounds.origin.y) }
            nodes.append(n)
        }
        return ["clock": now(), "viewport": ["w": r2(clip.bounds.width), "h": r2(clip.bounds.height)], "nodes": nodes]
    }

    static func view(_ req: [String: Any]) -> NodeView? {
        guard let id = req["id"] as? Int else { return nil }
        return presenter.views[UInt32(id)]
    }

    static func tap(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), let win = v.window else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        let b = box(v)
        // The middle of the box as seen — through a surface's placement when
        // there is one (LLP 1014 D5) — as a point in the window.
        let clip = presenter.viewport.contentView
        let p = clip.convert(NSPoint(x: b.midX + clip.bounds.origin.x, y: b.midY + clip.bounds.origin.y), to: nil)
        let at = [r2(b.midX), r2(b.midY)]
        if let wheel = req["wheel"] as? [Double], wheel.count == 2 {
            // The web's sign (a positive dy scrolls down), pixel units, no
            // phase: a gesture's phases would put the top-level scroll view
            // into a tracking loop that a synchronous call cannot feed. The
            // hit view gets it and the responder chain carries it up, as the
            // window routes a trackpad's. Deltas are whole pixels here
            // (rounded, bounded); a non-finite delta is refused, never a trap.
            guard wheel.allSatisfy(\.isFinite) else { return ["error": "wheel deltas must be finite"] }
            let whole = { (d: Double) -> Int32 in Int32(min(max(d.rounded(), -1_000_000), 1_000_000)) }
            guard let cg = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: -whole(wheel[1]), wheel2: -whole(wheel[0]), wheel3: 0) else { return ["error": "no wheel event"] }
            let screen = win.convertPoint(toScreen: p)
            cg.location = CGPoint(x: screen.x, y: (NSScreen.screens.first?.frame.height ?? 0) - screen.y)
            guard let e = NSEvent(cgEvent: cg) else { return ["error": "no wheel event"] }
            (win.contentView?.hitTest(p) ?? v).scrollWheel(with: e)
            return ["tapped": Int(v.id), "wheel": wheel, "at": at]
        }
        let t = ProcessInfo.processInfo.systemUptime
        guard let down = NSEvent.mouseEvent(with: .leftMouseDown, location: p, modifierFlags: [], timestamp: t, windowNumber: win.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1),
              let up = NSEvent.mouseEvent(with: .leftMouseUp, location: p, modifierFlags: [], timestamp: t, windowNumber: win.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 0)
        else { return ["error": "no mouse event"] }
        win.sendEvent(down)
        win.sendEvent(up)
        return ["tapped": Int(v.id), "at": at]
    }

    /// Set an input's text as typing does: the field editor, all selected,
    /// the text inserted — the delegate hears one change with the new value.
    static func type(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), let win = v.window else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        guard let f = v.field else { return ["error": "view \(v.id) is not an input"] }
        let text = req["text"] as? String ?? ""
        win.makeFirstResponder(f)
        guard let editor = f.currentEditor() as? NSTextView else { return ["error": "the field has no editor"] }
        editor.selectAll(nil)
        editor.insertText(text, replacementRange: editor.selectedRange())
        return ["typed": Int(v.id), "value": f.stringValue]
    }

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

    static func screenshot(_ req: [String: Any]) -> [String: Any] {
        guard let path = req["path"] as? String else { return ["error": "screenshot needs a path"] }
        if req["window"] as? Bool == true {
            // The window server's picture of this window — Metal layers
            // included, which cacheDisplay cannot see. Needs screen-capture
            // permission.
            let p = Process()
            p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
            p.arguments = ["-x", "-o", "-l", String(window.windowNumber), path]
            do { try p.run() } catch { return ["error": "screencapture: \(error)"] }
            p.waitUntilExit()
            let v = presenter.viewport
            return p.terminationStatus == 0 ? ["screenshot": path, "window": true, "w": r2(v.bounds.width), "h": r2(v.bounds.height), "scale": r2(window.backingScaleFactor)] : ["error": "screencapture exited \(p.terminationStatus)"]
        }
        let v = presenter.viewport
        guard let rep = v.bitmapImageRepForCachingDisplay(in: v.bounds) else { return ["error": "no bitmap for the viewport"] }
        v.cacheDisplay(in: v.bounds, to: rep)
        guard let png = rep.representation(using: .png, properties: [:]) else { return ["error": "no PNG"] }
        do { try png.write(to: URL(fileURLWithPath: path)) } catch { return ["error": "write \(path): \(error)"] }
        return ["screenshot": path, "w": r2(v.bounds.width), "h": r2(v.bounds.height)]
    }
}
