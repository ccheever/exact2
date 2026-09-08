// The agent API on AppKit (LLP 1012; the shared half is `Agent.swift`).
// Under EXACT_AGENT=1 the driver (`scripts/agent.mjs`) owns this process
// over stdio. `layout` reads the views as they sit in the viewport, scroll
// folded in (the web's getBoundingClientRect); `tap` sends mouse events
// through the window — hit-testing and the responder chain, the path a
// click takes — or a wheel to the hit view; `type` puts text through the
// field editor; `screenshot` draws the viewport to a PNG (`window: true`
// asks the window server instead, which sees Metal layers).
#if os(macOS)
import AppKit

extension Agent {
    /// Read requests off stdin on a thread; answer each on the main thread,
    /// in order, before reading the next. Stdin closing ends the process.
    /// `sessions` are what a request's `session` label routes among; the
    /// first is the default.
    public static func startStdio(sessions: [(String, ExactSession)]) {
        routes = sessions
        Thread { serve(fd: 0) }.start()
    }

    var presenter: Presenter { session.presenter }

    /// A view's box in the viewport: the clip view's space, less its scroll
    /// origin — every enclosing scroll node's offset folded in — with the
    /// presentation transform (translate/scale/rotate on the layer) applied,
    /// as the web's `getBoundingClientRect` includes CSS transforms.
    func box(_ v: NSView) -> NSRect {
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

    /// How far an offset lies outside `[0, max]` per axis, and zero on an
    /// axis within them — the geometry of an overscroll.
    ///
    /// Stated over numbers rather than over a view because `NSClipView`
    /// clamps an origin set through its own API while AppKit's rubber band
    /// puts one out of range, so a test cannot build the state it needs to
    /// check the arithmetic. `ExactKitTests` checks this; the view reads the
    /// three numbers and calls it.
    static func overscroll(origin: CGPoint, document: CGSize, visible: CGSize) -> (CGFloat, CGFloat) {
        let past = { (value: CGFloat, limit: CGFloat) -> CGFloat in
            // Half a point of slack: a fractional layout is not an overscroll.
            let end = max(0, limit)
            if value < -0.5 { return value }
            if value > end + 0.5 { return value - end }
            return 0
        }
        return (past(origin.x, document.width - visible.width), past(origin.y, document.height - visible.height))
    }

    /// The same, for a live scroller.
    static func overscroll(of sv: NSScrollView) -> (CGFloat, CGFloat) {
        overscroll(origin: sv.contentView.bounds.origin,
                   document: sv.documentView?.frame.size ?? .zero,
                   visible: sv.contentView.bounds.size)
    }

    func layout() -> [String: Any] {
        let clip = presenter.viewport.contentView
        var nodes: [[String: Any]] = []
        for (id, v) in presenter.views.sorted(by: { $0.key < $1.key }) where v.window != nil {
            let r = box(v)
            var n: [String: Any] = ["id": Int(id), "x": Agent.r2(r.origin.x), "y": Agent.r2(r.origin.y), "w": Agent.r2(r.width), "h": Agent.r2(r.height)]
            if let sv = v.scroll {
                let o = sv.contentView.bounds.origin
                n["sx"] = Agent.r2(o.x)
                n["sy"] = Agent.r2(o.y)
                // How far past its own ends this scroller currently sits.
                // A stretched rubber band is a real state a driver could not
                // otherwise see: the offset alone reads as an ordinary
                // number, and a band that never releases looks like a
                // scrolled pane. Absent when the offset is within its bounds.
                let (ox, oy) = Agent.overscroll(of: sv)
                if ox != 0 { n["ox"] = Agent.r2(ox) }
                if oy != 0 { n["oy"] = Agent.r2(oy) }
            }
            nodes.append(n)
        }
        // The page's environment (LLP 1012 §1): under `viewport-fit=cover`
        // the titlebar is the top inset; a software keyboard is never here.
        let i = presenter.insets
        let env: [String: Any] = ["safe-area-inset-top": Agent.r2(i.top), "safe-area-inset-right": Agent.r2(i.right), "safe-area-inset-bottom": Agent.r2(i.bottom), "safe-area-inset-left": Agent.r2(i.left), "keyboard-inset-height": 0]
        // The page scrolls too, and it is the one whose overscroll drags the
        // app's own chrome (LLP 1033 D4).
        let (px, py) = Agent.overscroll(of: presenter.viewport)
        var viewport: [String: Any] = ["w": Agent.r2(clip.bounds.width), "h": Agent.r2(clip.bounds.height)]
        if px != 0 { viewport["ox"] = Agent.r2(px) }
        if py != 0 { viewport["oy"] = Agent.r2(py) }
        return ["clock": session.now(), "viewport": viewport, "env": env, "nodes": nodes]
    }

    func view(_ req: [String: Any]) -> NodeView? {
        guard let id = req["id"] as? Int else { return nil }
        return presenter.views[UInt32(id)]?.paragraphOwner
    }

    func tap(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), let win = v.window else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        let b = box(v)
        // The middle of the box as seen — through a surface's placement when
        // there is one (LLP 1014 D5) — as a point in the window.
        let clip = presenter.viewport.contentView
        let p = clip.convert(NSPoint(x: (req["x"] as? Double ?? b.midX) + clip.bounds.origin.x, y: (req["y"] as? Double ?? b.midY) + clip.bounds.origin.y), to: nil)
        let at = [Agent.r2(b.midX), Agent.r2(b.midY)]
        if req["hover"] as? Bool == true {
            // The pointer moved onto the target: the node with a hover
            // handler at the hit point enters (and whatever was hovered
            // leaves), as a tracking area would report for a real move.
            var n: NSView? = win.contentView?.hitTest(p) ?? v
            while let cur = n, !((cur as? NodeView)?.handlers.contains("hover") ?? false) { n = cur.superview }
            if let node = n as? NodeView { presenter.hover(node, true) } else if let h = presenter.hovered { presenter.hover(h, false) }
            return ["tapped": Int(v.id), "hover": true, "at": at]
        }
        if let wheel = req["wheel"] as? [Double], wheel.count == 2 {
            // The web's sign (a positive dy scrolls down), pixel units. The
            // hit view gets it and the responder chain carries it up, as the
            // window routes a trackpad's. Deltas are whole pixels here
            // (rounded, bounded); a non-finite delta is refused, never a trap.
            //
            // `gesture` sends what a finger sends instead of a bare delta:
            // `.began`, `.changed`, and the **zero-delta `.ended`** that is a
            // lift. Elastic overscroll lives entirely in those phases — a
            // plain wheel scrolls a pane perfectly while a real trackpad
            // sticks — so the phases are the only way to drive that path
            // (LLP 1033 D4a). The whole sequence goes out inside this one
            // call: the original refusal here was that phases put the top
            // scroll view into a tracking loop a synchronous call cannot
            // feed, and a gesture delivered complete is never left waiting.
            // What AppKit then animates (a rubber band settling) is its own
            // and outside the session clock; `layout` reports the overscroll
            // it leaves behind.
            guard wheel.allSatisfy(\.isFinite) else { return ["error": "wheel deltas must be finite"] }
            let gesture = req["gesture"] as? Bool == true
            let whole = { (d: Double) -> Int32 in Int32(min(max(d.rounded(), -1_000_000), 1_000_000)) }
            let screen = win.convertPoint(toScreen: p)
            let location = CGPoint(x: screen.x, y: (NSScreen.screens.first?.frame.height ?? 0) - screen.y)
            let target = win.contentView?.hitTest(p) ?? v
            // CGScrollPhase: began 1, changed 2, ended 4.
            let halfX: Double = wheel[0] / 2
            let halfY: Double = wheel[1] / 2
            let restX: Double = wheel[0] - halfX
            let restY: Double = wheel[1] - halfY
            var steps: [(phase: Int64, dx: Double, dy: Double)] = [(0, wheel[0], wheel[1])]
            if gesture {
                steps = [(1, halfX, halfY), (2, restX, restY), (4, 0, 0)]
            }
            for (phase, dx, dy) in steps {
                guard let cg = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: -whole(dy), wheel2: -whole(dx), wheel3: 0) else { return ["error": "no wheel event"] }
                cg.location = location
                if gesture {
                    // A trackpad's deltas are continuous; without this the
                    // event reads as a wheel's notches and the phase is moot.
                    cg.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
                    cg.setIntegerValueField(.scrollWheelEventScrollPhase, value: phase)
                }
                guard let e = NSEvent(cgEvent: cg) else { return ["error": "no wheel event"] }
                target.scrollWheel(with: e)
            }
            return ["tapped": Int(v.id), "wheel": wheel, "gesture": gesture, "at": at]
        }
        if v.kind == "iframe" { return session.webviews.tap(v, request: req, at: at) }
        let t = ProcessInfo.processInfo.systemUptime
        guard let down = NSEvent.mouseEvent(with: .leftMouseDown, location: p, modifierFlags: [], timestamp: t, windowNumber: win.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1),
              let up = NSEvent.mouseEvent(with: .leftMouseUp, location: p, modifierFlags: [], timestamp: t, windowNumber: win.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 0)
        else { return ["error": "no mouse event"] }
        win.sendEvent(down)
        if let drag = req["drag"] as? [Double], drag.count == 2, drag.allSatisfy(\.isFinite) {
            let point = clip.convert(NSPoint(x: drag[0] + clip.bounds.origin.x, y: drag[1] + clip.bounds.origin.y), to: nil)
            if let move = NSEvent.mouseEvent(with: .leftMouseDragged, location: point, modifierFlags: [], timestamp: t + 0.01,
                windowNumber: win.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1) { win.sendEvent(move) }
        }
        win.sendEvent(up)
        return ["tapped": Int(v.id), "at": at]
    }

    /// Set an input's text as typing does: the field editor, all selected,
    /// the text inserted — the delegate hears one change with the new value.
    func type(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), let win = v.window else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        guard !v.disabled else { return ["error": "view \(v.id) is disabled"] }
        if v.props["editable"] == "false", req["key"] == nil { return ["error": "view \(v.id) is readonly"] }
        if v.kind == "iframe" { return session.webviews.type(v, request: req) }
        if let chord = req["key"] as? String {
            let parts = chord.split(separator: "+").map(String.init)
            let key = parts.last ?? chord
            var modifiers: NSEvent.ModifierFlags = []
            for modifier in parts.dropLast() {
                switch modifier { case "Meta": modifiers.insert(.command); case "Shift": modifiers.insert(.shift)
                case "Control": modifiers.insert(.control); case "Alt": modifiers.insert(.option)
                default: return ["error": "unknown key modifier \(modifier)"] }
            }
            // A key down at the target through the window — the field
            // editor's commands, or a focused node's keyDown — by the web's
            // name, as AppKit would deliver the keyboard's.
            if !win.isKeyWindow { win.makeKey() }
            // First responder only if it is not held already: re-making an
            // editing field first responder ends its editing (a blur the
            // app would see) and begins it again with no focus.
            if let f = v.textArea {
                if win.firstResponder !== f { win.makeFirstResponder(f) }
            } else if let f = v.field {
                let editing = f.currentEditor().map { win.firstResponder === $0 } ?? false
                if !editing { win.makeFirstResponder(f) }
            } else if v.acceptsFirstResponder {
                if win.firstResponder !== v { win.makeFirstResponder(v) }
            } else { return ["error": "view \(v.id) takes no key"] }
            let (chars, code): (String, UInt16) = {
                switch key {
                case "c": return (key, 8)
                case "o": return (key, 31)
                case "Enter": return ("\r", 36)
                case "Escape": return ("\u{1b}", 53)
                case "Tab": return ("\t", 48)
                case "Backspace": return ("\u{7f}", 51)
                case "ArrowUp": return ("\u{F700}", 126)
                case "ArrowDown": return ("\u{F701}", 125)
                case "ArrowLeft": return ("\u{F702}", 123)
                case "ArrowRight": return ("\u{F703}", 124)
                default: return (key, 0)
                }
            }()
            let t = ProcessInfo.processInfo.systemUptime
            guard let down = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers, timestamp: t, windowNumber: win.windowNumber, context: nil, characters: chars, charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code),
                  let up = NSEvent.keyEvent(with: .keyUp, location: .zero, modifierFlags: modifiers, timestamp: t, windowNumber: win.windowNumber, context: nil, characters: chars, charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code)
            else { return ["error": "no key event"] }
            // This driver sends directly to NSWindow, bypassing NSApplication's
            // local monitor. Use the same session command router first.
            if presenter.shortcuts.perform(down) {
                return ["typed": Int(v.id), "key": chord, "value": v.textArea?.string ?? v.field?.stringValue ?? ""]
            }
            // Accessory test windows may have a first responder before
            // NSApp has a keyWindow. Deliver to the named responder first.
            if modifiers.contains(.command), v.performKeyEquivalent(with: down) || NSApp.mainMenu?.performKeyEquivalent(with: down) == true {
                return ["typed": Int(v.id), "key": chord]
            }
            win.sendEvent(down)
            win.sendEvent(up)
            return ["typed": Int(v.id), "key": key, "value": v.textArea?.string ?? v.field?.stringValue ?? ""]
        }
        if let f = v.textArea {
            if !win.isKeyWindow { win.makeKey() }
            win.makeFirstResponder(f)
            f.selectAll(nil)
            f.insertText(req["text"] as? String ?? "", replacementRange: f.selectedRange())
            return ["typed": Int(v.id), "value": f.string]
        }
        guard let f = v.field else { return ["error": "view \(v.id) is not an input"] }
        let text = req["text"] as? String ?? ""
        // The field editor needs a key window; an accessory app's is not
        // one until asked (and asking does not activate the app).
        if !win.isKeyWindow { win.makeKey() }
        win.makeFirstResponder(f)
        guard let editor = f.currentEditor() as? NSTextView else { return ["error": "the field has no editor"] }
        editor.selectAll(nil)
        editor.insertText(text, replacementRange: editor.selectedRange())
        return ["typed": Int(v.id), "value": f.stringValue]
    }

    func screenshot(_ req: [String: Any]) -> [String: Any] {
        guard let path = req["path"] as? String else { return ["error": "screenshot needs a path"] }
        let v = presenter.viewport
        if req["window"] as? Bool == true {
            // The window server's picture of this window — Metal layers
            // included, which cacheDisplay cannot see. Needs screen-capture
            // permission.
            guard let window = v.window else { return ["error": "the session's view is not in a window"] }
            let p = Process()
            p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
            p.arguments = ["-x", "-o", "-l", String(window.windowNumber), path]
            do { try p.run() } catch { return ["error": "screencapture: \(error)"] }
            p.waitUntilExit()
            return p.terminationStatus == 0 ? ["screenshot": path, "window": true, "w": Agent.r2(v.bounds.width), "h": Agent.r2(v.bounds.height), "scale": Agent.r2(window.backingScaleFactor)] : ["error": "screencapture exited \(p.terminationStatus)"]
        }
        guard let rep = v.bitmapImageRepForCachingDisplay(in: v.bounds) else { return ["error": "no bitmap for the viewport"] }
        Capture.web = session.webviews.snapshots()
        let hidden = presenter.views.values.compactMap(\.web).map { ($0, $0.isHidden) }
        hidden.forEach { $0.0.isHidden = true }
        // As a capture: every canvas paints its picture, read back from the
        // module, and every iframe paints its arm snapshot at its node.
        Capture.capturing = true
        v.cacheDisplay(in: v.bounds, to: rep)
        Capture.capturing = false
        hidden.forEach { $0.0.isHidden = $0.1 }
        Capture.web = [:]
        guard let png = rep.representation(using: .png, properties: [:]) else { return ["error": "no PNG"] }
        do { try png.write(to: URL(fileURLWithPath: path)) } catch { return ["error": "write \(path): \(error)"] }
        return ["screenshot": path, "w": Agent.r2(v.bounds.width), "h": Agent.r2(v.bounds.height)]
    }
}
#endif
