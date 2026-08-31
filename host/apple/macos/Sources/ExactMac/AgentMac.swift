// The agent API on AppKit (LLP 1012; the shared half is `Agent.swift`).
// Under EXACT_AGENT=1 the driver (`scripts/agent.mjs`) owns this process
// over stdio. `layout` reads the views as they sit in the viewport, scroll
// folded in (the web's getBoundingClientRect); `tap` sends mouse events
// through the window — hit-testing and the responder chain, the path a
// click takes — or a wheel to the hit view; `type` puts text through the
// field editor; `screenshot` draws the viewport to a PNG (`window: true`
// asks the window server instead, which sees Metal layers).
import AppKit

extension Agent {
    /// Read requests off stdin on a thread; answer each on the main thread,
    /// in order, before reading the next. Stdin closing ends the process.
    static func start() {
        Thread { serve(fd: 0) }.start()
    }

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
        // The page's environment (LLP 1012 §1): under `viewport-fit=cover`
        // the titlebar is the top inset; a software keyboard is never here.
        let i = presenter.insets
        let env: [String: Any] = ["safe-area-inset-top": r2(i.top), "safe-area-inset-right": r2(i.right), "safe-area-inset-bottom": r2(i.bottom), "safe-area-inset-left": r2(i.left), "keyboard-inset-height": 0]
        return ["clock": now(), "viewport": ["w": r2(clip.bounds.width), "h": r2(clip.bounds.height)], "env": env, "nodes": nodes]
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
        if v.kind == "iframe" { return webviews.tap(v, request: req, at: at) }
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
        if v.kind == "iframe" { return webviews.type(v, request: req) }
        if let key = req["key"] as? String {
            // A key down at the target through the window — the field
            // editor's commands, or a focused node's keyDown — by the web's
            // name, as AppKit would deliver the keyboard's.
            if !win.isKeyWindow { win.makeKey() }
            // First responder only if it is not held already: re-making an
            // editing field first responder ends its editing (a blur the
            // app would see) and begins it again with no focus.
            if let f = v.field {
                let editing = f.currentEditor().map { win.firstResponder === $0 } ?? false
                if !editing { win.makeFirstResponder(f) }
            } else if v.acceptsFirstResponder {
                if win.firstResponder !== v { win.makeFirstResponder(v) }
            } else { return ["error": "view \(v.id) takes no key"] }
            let (chars, code): (String, UInt16) = {
                switch key {
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
            guard let down = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: t, windowNumber: win.windowNumber, context: nil, characters: chars, charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code),
                  let up = NSEvent.keyEvent(with: .keyUp, location: .zero, modifierFlags: [], timestamp: t, windowNumber: win.windowNumber, context: nil, characters: chars, charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code)
            else { return ["error": "no key event"] }
            win.sendEvent(down)
            win.sendEvent(up)
            return ["typed": Int(v.id), "key": key, "value": v.field?.stringValue ?? ""]
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
        Capture.web = webviews.snapshots()
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
        return ["screenshot": path, "w": r2(v.bounds.width), "h": r2(v.bounds.height)]
    }
}
