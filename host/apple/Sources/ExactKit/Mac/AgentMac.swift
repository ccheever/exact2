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

    /// AppKit animates nothing here that a seek does not move.
    func nativeInFlight() -> Bool { false }

    /// Diagnostic tap {resize:[w,h]} (LLP 1041 §8). Resize the containing
    /// NSWindow, allowing ExactView's ordinary fit/inset path to follow.
    /// Never assign the viewport frame or subtract titlebar/toolbar heights:
    /// AppKit owns that geometry. This is repeated programmatic window resize,
    /// not a simulated titlebar drag or a physical frame-presentation receipt.
    func resizeWindow(_ size: CGSize) -> [String: Any] {
        guard contact == nil else { return ["error": "release the held contact before resizing"] }
        guard let window = presenter.viewport.window, let content = window.contentView else {
            return ["error": "no window to resize"]
        }
        window.setContentSize(size)
        content.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        let actual = window.contentRect(forFrameRect: window.frame).size
        let viewport = presenter.viewport.contentView.bounds.size
        let dimensions = { (s: CGSize) -> [Double] in [Agent.r2(s.width), Agent.r2(s.height)] }
        return ["resized": dimensions(actual), "viewport": dimensions(viewport),
                "contentView": dimensions(content.bounds.size),
                "contentLayout": dimensions(window.contentLayoutRect.size),
                "windowFrame": dimensions(window.frame.size),
                "backingScale": window.backingScaleFactor,
                "toolbar": window.toolbar != nil, "delivery": "platform-window",
                "native": "NSWindow.setContentSize", "paint": "displayIfNeeded; presentation unobserved"]
    }

    /// What AppKit knows for `state` (LLP 1035.002 D2): the node holding
    /// the focus (a field through its field editor), no software keyboard,
    /// and the routes as the props declare them — macOS projects nothing
    /// natively, so the stack is the rule's prefix and the phase is idle.
    func stateSections() -> [String: Any] {
        var focus: [String: Any] = ["logical": NSNull(), "editor": NSNull(), "responder": NSNull(), "pending": NSNull()]
        let responder = presenter.viewport.window?.firstResponder
        if let node = presenter.views.values.filter({ n in
            responder === n || responder === n.textArea || (n.field.flatMap { f in f.currentEditor().map { responder === $0 } } ?? false)
        }).min(by: { $0.id < $1.id }) {
            focus["logical"] = Int(node.id)
            if node.field != nil || node.textArea != nil { focus["editor"] = Int(node.id) }
            focus["responder"] = responder.map { String(describing: Swift.type(of: $0)) } ?? NSNull()
        }
        let keyboard: [String: Any] = ["visible": false, "overlap": 0, "policy": "resizes-visual", "interactive": false]
        var navigation: [String: Any] = ["route": NSNull(), "stack": [] as [String], "presentation": NSNull(), "closedby": NSNull(),
                                         "transition": ["interactive": false, "phase": "idle"]]
        if let container = presenter.views.values.filter({ $0.props["navigationBack"] != nil }).min(by: { $0.id < $1.id }) {
            let key = container.props["navigationKey"] ?? ""
            let routes = container.container.subviews.compactMap { $0 as? NodeView }.filter { $0.props["navigationKey"] != nil }
            let keys = routes.map { $0.props["navigationKey"] ?? "" }
            navigation["route"] = key
            if let range = NavigationRules.stack(routeKeys: keys, selected: key) {
                navigation["stack"] = Array(keys[range])
                let selected = routes[range.upperBound - 1]
                navigation["presentation"] = selected.props["navigationPresentation"] == "modal" ? "modal" : NSNull()
                navigation["closedby"] = selected.props["closedby"] ?? NSNull()
            }
        }
        // @ref LLP 1038 D11 — last op, never inferred from route props.
        navigation["url"] = session.routerOp?["url"] ?? NSNull()
        return ["focus": focus, "keyboard": keyboard, "navigation": navigation]
    }

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
            if let toolbar = presenter.toolbar.observation(v) {
                n = ["id": Int(id), "native": toolbar]
            }
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

    /// `layout <node>` (LLP 1035.002 D1): the runner's rows and their sources
    /// for one node, then what AppKit knows about it — its box in the
    /// viewport, the window and the screen (both reported y-down from the
    /// top, as every space here is), the scroll and clip chains above it,
    /// whether it is hidden, in the viewport or clipped away, and what was
    /// mounted for it. Hidden and inert ancestors are observed, never
    /// guessed. A stale id is refused by name.
    func layout(_ req: [String: Any]) -> [String: Any] {
        var reply = layout()
        guard let id = req["id"] as? Int else { return reply }
        guard let v = presenter.views[UInt32(id)] else { return ["error": "stale node #\(id)"] }
        guard let d = session.agent("{\"op\":\"node\",\"id\":\(id)}").data(using: .utf8),
              var node = (try? JSONSerialization.jsonObject(with: d)) as? [String: Any] else { return ["error": "node #\(id): unreadable"] }
        if let e = node["error"] { return ["error": e] }
        let host = v.paragraphOwner
        let clipView = presenter.viewport.contentView
        let rect = { (r: NSRect) -> [String: Any] in ["x": Agent.r2(r.origin.x), "y": Agent.r2(r.origin.y), "w": Agent.r2(r.width), "h": Agent.r2(r.height)] }
        let b = box(host)
        var space: [String: Any] = ["viewport": rect(b), "local": ["w": Agent.r2(host.bounds.width), "h": Agent.r2(host.bounds.height)],
                                    "capture": ["scale": Agent.r2(host.window?.backingScaleFactor ?? 1)]]
        if let w = host.window, let content = w.contentView {
            let inWindow = host.convert(host.bounds, to: nil)
            space["window"] = rect(NSRect(x: inWindow.origin.x, y: content.frame.height - inWindow.maxY, width: inWindow.width, height: inWindow.height))
            let onScreen = w.convertToScreen(inWindow)
            let top = NSScreen.screens.first?.frame.maxY ?? onScreen.maxY
            space["screen"] = rect(NSRect(x: onScreen.origin.x, y: top - onScreen.maxY, width: onScreen.width, height: onScreen.height))
        }
        node["space"] = space
        var clipped = b.isEmpty
        var chain: [[String: Any]] = []
        var clippers: [(NodeView, String)] = []
        var above = host.superview
        while let s = above {
            if let n = s as? NodeView {
                if let sv = n.scroll { let o = sv.contentView.bounds.origin; chain.append(["id": Int(n.id), "sx": Agent.r2(o.x), "sy": Agent.r2(o.y)]) }
                if n.clipsToBounds { clippers.append((n, "overflow")) }
                if n.clipPath != nil { clippers.append((n, "clip-path")) }
            }
            above = s.superview
        }
        let page = clipView.bounds.origin
        var scroll: [[String: Any]] = [["viewport": true, "sx": Agent.r2(page.x), "sy": Agent.r2(page.y)]]
        scroll.append(contentsOf: chain.reversed())
        var clip: [[String: Any]] = []
        for (n, kind) in clippers.reversed() {
            clip.append(["id": Int(n.id), "kind": kind])
            if host.window != nil, !host.convert(host.bounds, to: nil).intersects(n.convert(n.bounds, to: nil)) { clipped = true }
        }
        node["scroll"] = scroll
        node["clip"] = clip
        var visible: [String: Any] = ["hidden": host.isHiddenOrHasHiddenAncestor, "inert": host.inert, "inViewport": b.intersects(NSRect(origin: .zero, size: clipView.bounds.size)), "clipped": clipped]
        if host.isHiddenOrHasHiddenAncestor {
            // Name the ancestor that hides it, never leave a reader guessing.
            var s: NSView? = host
            while let v = s, !v.isHidden { s = v.superview }
            if let v = s { visible["hiddenBy"] = (v as? NodeView).map { "#\($0.id)" } ?? String(describing: Swift.type(of: v)) }
        }
        node["visible"] = visible
        var native: [String: Any] = ["view": String(describing: Swift.type(of: v)), "sheet": false]
        if v !== host { native["inline"] = true }
        if let f = host.field { native["editor"] = String(describing: Swift.type(of: f)); native["firstResponder"] = f.currentEditor() != nil }
        if let t = host.textArea { native["editor"] = String(describing: Swift.type(of: t)); native["firstResponder"] = host.window?.firstResponder === t }
        if let segment = presenter.segments.observation(host) { native["segmentedControl"] = segment }
        if let toolbar = presenter.toolbar.observation(host) {
            native["windowToolbar"] = toolbar
            // The kernel frame is authored fallback geometry, not the native
            // titlebar item's bounds. AppKit exposes no public item frame.
            node["space"] = ["placement": "window-toolbar", "geometry": "system-owned"]
            node["scroll"] = [] as [Int]; node["clip"] = [] as [Int]
            node["visible"] = ["hidden": !presenter.toolbar.visible(host), "inert": host.inert,
                               "inViewport": false, "clipped": false]
        }
        if let leaf = host.symbolView {
            let size = leaf.image?.size ?? .zero
            native["symbol"] = ["renderer": String(describing: Swift.type(of: leaf)), "name": host.props["symbolName"] ?? "", "intrinsic": [Agent.r2(size.width), Agent.r2(size.height)], "frame": rect(box(leaf))]
        }
        if v.props["backgroundMaterial"] != nil {
            native["effect"] = v.appliedMaterial
        }
        node["native"] = native
        node["observed"] = ["clock": session.now(), "wall": Date().timeIntervalSince1970 * 1000]
        reply["node"] = node
        return reply
    }

    func view(_ req: [String: Any]) -> NodeView? {
        guard let id = req["id"] as? Int else { return nil }
        return presenter.views[UInt32(id)]?.paragraphOwner
    }

    /// A contact held across requests (LLP 1035.003 D1): the mouse button
    /// down at a point in the viewport, dragged along a declared path,
    /// held, released — each phase a real `NSEvent` through `sendEvent`,
    /// the path a click takes, with the run loop turning between the steps
    /// of a timed move so AppKit tracks them as it would a hand's. Every
    /// other operation answers while the button is down. AppKit has no
    /// cancel for a mouse: `cancel` is reported unsupported and the contact
    /// stays down, never faked as a release.
    func contact(_ phase: String, _ req: [String: Any]) -> [String: Any] {
        guard let win = presenter.viewport.window else { return ["error": "no window"] }
        let clip = presenter.viewport.contentView
        let toWindow = { (p: CGPoint) -> NSPoint in clip.convert(NSPoint(x: p.x + clip.bounds.origin.x, y: p.y + clip.bounds.origin.y), to: nil) }
        let send = { (type: NSEvent.EventType, p: CGPoint) in
            let t = ProcessInfo.processInfo.systemUptime
            if let e = NSEvent.mouseEvent(with: type, location: toWindow(p), modifierFlags: [], timestamp: t, windowNumber: win.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: type == .leftMouseUp ? 0 : 1) {
                win.sendEvent(e)
            }
        }
        let at = { (p: CGPoint) -> [Double] in [Agent.r2(p.x), Agent.r2(p.y)] }
        switch phase {
        case "down":
            guard contact == nil else { return ["error": "a contact is already down; up it first"] }
            guard let v = view(req), v.window != nil else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
            if presenter.toolbar.suppresses(v) { return ["error": "native toolbar geometry is system-owned; use tap host activation"] }
            let b = box(v)
            let p = CGPoint(x: req["x"] as? Double ?? b.midX, y: req["y"] as? Double ?? b.midY)
            send(.leftMouseDown, p)
            contact = p
            return ["contact": Int(v.id), "phase": "down", "at": at(p), "delivery": "platform"]
        case "move":
            guard let from = contact else { return ["error": "no contact is down"] }
            let to = CGPoint(x: req["x"] as? Double ?? from.x + (req["dx"] as? Double ?? 0), y: req["y"] as? Double ?? from.y + (req["dy"] as? Double ?? 0))
            guard to.x.isFinite, to.y.isFinite else { return ["error": "move needs finite coordinates"] }
            let ms = max(0, req["ms"] as? Double ?? 0)
            let steps = max(1, Int(ms / 16))
            for i in 1...steps {
                let t = CGFloat(i) / CGFloat(steps)
                send(.leftMouseDragged, CGPoint(x: from.x + (to.x - from.x) * t, y: from.y + (to.y - from.y) * t))
                if ms > 0 { RunLoop.main.run(until: Date(timeIntervalSinceNow: ms / 1000 / Double(steps))) }
            }
            contact = to
            return ["phase": "move", "at": at(to), "delivery": "platform"]
        case "hold":
            guard let p = contact else { return ["error": "no contact is down"] }
            let ms = max(0, req["ms"] as? Double ?? 0)
            if ms > 0 { RunLoop.main.run(until: Date(timeIntervalSinceNow: ms / 1000)) }
            return ["phase": "hold", "at": at(p), "delivery": "platform"]
        case "up":
            guard let p = contact else { return ["error": "no contact is down"] }
            send(.leftMouseUp, p)
            contact = nil
            return ["phase": "up", "at": at(p), "delivery": "platform"]
        case "cancel":
            guard let p = contact else { return ["error": "no contact is down"] }
            return ["phase": "cancel", "at": at(p), "delivery": "unsupported", "reason": "AppKit has no cancel for a mouse; the contact is still down — send up"]
        default:
            return ["error": "unknown phase \(phase) (down, move, hold, up, cancel)"]
        }
    }

    func tap(_ req: [String: Any]) -> [String: Any] {
        if let phase = req["phase"] as? String { return contact(phase, req) }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let activated = presenter.toolbar.activate(node) {
            return activated ? ["tapped": id, "delivery": "host-activation", "native": "NSToolbarItem"]
                : ["error": "native toolbar item #\(id) is unavailable"]
        }
        if let node = view(req), presenter.toolbar.suppresses(node) {
            return ["error": "native toolbar geometry is system-owned; only button host activation is supported"]
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let activated = presenter.segments.activate(node) {
            return activated ? ["tapped": id, "delivery": "host-activation", "native": "segmented-control"]
                : ["error": "native segment #\(id) is unavailable"]
        }
        guard let v = view(req), let win = v.window else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        guard presenter.toolbar.visible(v), !v.inert else { return ["error": "view \(v.id) is hidden or inert"] }
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
        win.sendEvent(up)
        return ["tapped": Int(v.id), "at": at, "delivery": "platform"]
    }

    /// Set an input's text as typing does: the field editor, all selected,
    /// the text inserted — the delegate hears one change with the new value.
    func type(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), let win = v.window else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        guard presenter.toolbar.visible(v), !v.inert else { return ["error": "view \(v.id) is hidden or inert"] }
        guard !v.disabled else { return ["error": "view \(v.id) is disabled"] }
        if v.props["editable"] == "false", req["key"] == nil { return ["error": "view \(v.id) is readonly"] }
        // @ref LLP 1038 D11 — type on the root delivers a location.
        if v.props["navigationBack"] != nil, req["key"] == nil {
            let location = req["text"] as? String ?? ""
            return session.navigate(location) ? ["typed": Int(v.id), "value": location, "delivery": "recognized"] : ["error": "navigate refused"]
        }
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
            if presenter.toolbar.contains(v) {
                if let view = session.view { win.makeFirstResponder(view) }
            } else if let f = v.textArea {
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
