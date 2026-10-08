// The agent API on UIKit (LLP 1012; the shared half is `Agent.swift`).
// Under EXACT_AGENT=1 the driver (`scripts/agent.mjs`) owns this process
// over a Unix socket named by EXACT_AGENT_SOCKET — a simulator app has no
// stdin. `layout` reads the views as they sit in the viewport, scroll
// folded in (the web's getBoundingClientRect); `tap` hit-tests through the
// window (UIKit's own, placements included) and delivers the press by the
// responder-chain rule a touch gets — UIKit offers no public touch
// synthesis, the one declared deviation from LLP 1012's contract — or
// applies a wheel to the first scroll container up the chain that can take
// it (the web's chaining rule); `type` puts text through the field's own
// `insertText`; `screenshot` draws the viewport's hierarchy to a PNG (Metal
// layers included, so `window: true` is the same picture).
#if os(iOS) || os(tvOS)
import UIKit

extension Agent {
    /// Listen on the socket; when the driver connects, `ready` goes out and
    /// requests are answered until it hangs up (which ends the process).
    /// `sessions` are what a request's `session` label routes among.
    public static func startSocket(ready: [String: Any], sessions: [(String, ExactSession)]) {
        routes = sessions
        // Agent input does not reset UIKit's user-idle timer. Keep an explicitly
        // driven test launch awake; normal launches never enter this carrier.
        UIApplication.shared.isIdleTimerDisabled = true
        // Physical devices have no readable stdin, even with devicectl --console.
        // Connect outward to this launch's driver, after first pixel. No listener.
        if let endpoint = ExactEnv.environment["EXACT_AGENT_CONNECT"],
           let token = ExactEnv.environment["EXACT_AGENT_TOKEN"], !token.isEmpty {
            let parts = endpoint.split(separator: ":")
            guard parts.count == 2, let port = UInt16(parts[1]), port > 0 else { return }
            var addr = sockaddr_in()
            addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
            addr.sin_family = sa_family_t(AF_INET)
            addr.sin_port = port.bigEndian
            guard String(parts[0]).withCString({ inet_pton(AF_INET, $0, &addr.sin_addr) }) == 1 else { return }
            var announcement = ready
            announcement["container"] = NSHomeDirectory()
            announcement["token"] = token
            Thread {
                let fd = socket(AF_INET, SOCK_STREAM, 0)
                guard fd >= 0 else { return }
                let connected = withUnsafePointer(to: &addr) {
                    $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                        connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
                    }
                }
                guard connected == 0 else {
                    FileHandle.standardError.write(Data("exact: agent connect: \(String(cString: strerror(errno)))\n".utf8))
                    close(fd)
                    return
                }
                out = FileHandle(fileDescriptor: fd, closeOnDealloc: false)
                reply(announcement)
                serve(fd: fd)
            }.start()
            return
        }
        guard let path = ExactEnv.environment["EXACT_AGENT_SOCKET"] else {
            FileHandle.standardError.write(Data("exact: EXACT_AGENT=1 needs EXACT_AGENT_SOCKET=<path> on iOS\n".utf8))
            return
        }
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { FileHandle.standardError.write(Data("exact: socket: \(String(cString: strerror(errno)))\n".utf8)); return }
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        let capacity = MemoryLayout.size(ofValue: addr.sun_path)
        guard bytes.count < capacity else { FileHandle.standardError.write(Data("exact: socket path \(path) is longer than \(capacity - 1) bytes\n".utf8)); return }
        withUnsafeMutablePointer(to: &addr.sun_path) { p in
            p.withMemoryRebound(to: UInt8.self, capacity: capacity) { dst in
                for (i, b) in bytes.enumerated() { dst[i] = b }
                dst[bytes.count] = 0
            }
        }
        unlink(path)
        let len = socklen_t(MemoryLayout<sockaddr_un>.size)
        let bound = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, len) } }
        guard bound == 0, listen(fd, 1) == 0 else {
            FileHandle.standardError.write(Data("exact: \(path): \(String(cString: strerror(errno)))\n".utf8))
            return
        }
        Thread {
            let client = accept(fd, nil, nil)
            guard client >= 0 else { FileHandle.standardError.write(Data("exact: accept: \(String(cString: strerror(errno)))\n".utf8)); return }
            out = FileHandle(fileDescriptor: client, closeOnDealloc: false)
            reply(ready)
            serve(fd: client)
        }.start()
    }

    var presenter: Presenter { session.presenter }

    /// Native transitions, keyboard work and an agent-issued caret reveal
    /// must finish before `clock settle` returns (LLP 1035.003 D5).
    func nativeInFlight() -> Bool {
        if presenter.navigation.inTransition || presenter.modals.inTransition || presenter.menus.inTransition || presenter.hasPendingKeyboardResize || nativeGeometryInFlight() { return true }
        // A smooth correction (a list following its end, a smooth
        // `scrollIntoView`) is UIKit's scroll animation under platform timing
        // (LLP 1070.000 §11): the fixed point is where it lands, within
        // settle's bound (LLP 1035.003 D5).
        if !presenter.collections.animating.isEmpty || session.natives.activationQueued || presenter.launchAutofocusPending { return true }
        guard let editor = pendingTextReveal else { return false }
        guard let node = editor.owner, presenter.views[node.id] === node,
              node.textArea === editor, !presenter.navigation.isInactiveRoute(containing: node),
              editor.window != nil, editor.isFirstResponder,
              !editor.isTracking, !editor.isDragging, !editor.isDecelerating,
              let selection = editor.selectedTextRange else {
            pendingTextReveal = nil
            return false
        }
        // TextKit's caret can exceed the authored line box. Clamp both
        // alignments to UIKit's scrollable range; require only reachable
        // visibility, with one physical pixel for native quantization.
        let caret = editor.caretRect(for: selection.end)
        let inset = editor.adjustedContentInset
        let minimum = -inset.top
        let maximum = max(minimum, editor.contentSize.height + inset.bottom - editor.bounds.height)
        let top = min(max(caret.minY - inset.top, minimum), maximum)
        let bottom = min(max(caret.maxY + inset.bottom - editor.bounds.height, minimum), maximum)
        let tolerance = 1 / max(editor.traitCollection.displayScale, 1)
        let offset = editor.contentOffset.y
        if editor.bounds.height <= 0 || (offset >= min(top, bottom) - tolerance && offset <= max(top, bottom) + tolerance) {
            pendingTextReveal = nil
            return false
        }
        return true
    }

    /// UIKit can animate keyboard-driven geometry even when the agent applies
    /// its own resize synchronously. Observe only this session's mounted views;
    /// inspecting geometry animations avoids waiting for a caret's opacity blink.
    func nativeGeometryInFlight() -> Bool {
        guard presenter.viewport.window != nil else { return false }
        func movesGeometry(_ animation: CAAnimation) -> Bool {
            if let group = animation as? CAAnimationGroup {
                return group.animations?.contains(where: movesGeometry) == true
            }
            guard let path = (animation as? CAPropertyAnimation)?.keyPath,
                  let property = path.split(separator: ".").first else { return false }
            return ["bounds", "position", "transform", "sublayerTransform", "anchorPoint", "zPosition"].contains(String(property))
        }
        var pending: [UIView] = [presenter.viewport]
        while let view = pending.popLast() {
            guard !view.isHidden, view.alpha > 0 else { continue }
            if view.layer.animationKeys()?.contains(where: { key in
                view.layer.animation(forKey: key).map(movesGeometry) == true
            }) == true { return true }
            pending.append(contentsOf: view.subviews)
        }
        return false
    }

    /// What UIKit knows for `state` (LLP 1035.002 D2): which node holds the
    /// focus and through which responder, a focus still waiting on a
    /// presentation; the keyboard's visibility, overlap, top edge and guide
    /// in the viewport's space, the resize policy, whether a drag is
    /// dismissing it; the route the root names, UIKit's stack by key, the
    /// presentation and its close policy, and the transition's phase.
    func stateSections() -> [String: Any] {
        let vp = presenter.viewport
        var focus: [String: Any] = ["logical": NSNull(), "editor": NSNull(), "responder": NSNull(), "pending": NSNull()]
        let responding = presenter.views.values
            .filter { $0.isFirstResponder || $0.field?.isFirstResponder == true || $0.textArea?.isFirstResponder == true }
            .min { $0.id < $1.id }
        if let node = responding ?? presenter.editing {
            focus["logical"] = Int(node.id)
            if node.field != nil || node.textArea != nil { focus["editor"] = Int(node.id) }
            let responder: UIResponder = node.textArea ?? node.field ?? node
            focus["responder"] = String(describing: Swift.type(of: responder))
        }
        if let pending = presenter.pendingFocusObservation {
            focus["pending"] = pending
        }
        let keyboardContainer = presenter.modals.coordinateView ?? presenter.session?.view
        let keyboardTop = keyboardContainer.flatMap { presenter.keyboardGuideTop(in: $0) }
        var keyboard: [String: Any] = ["visible": keyboardTop != nil, "overlap": Agent.r2(presenter.keyboardInset),
                                       "policy": presenter.interactiveWidget ?? "resizes-visual", "interactive": presenter.interactiveKeyboardDrag]
        if let top = keyboardTop, let container = keyboardContainer {
            keyboard["top"] = Agent.r2(vp.convert(CGPoint(x: 0, y: top), from: container).y - vp.contentOffset.y)
        }
        #if !os(tvOS)
        if let view = presenter.session?.view { keyboard["guide"] = Agent.r2(view.keyboardLayoutGuide.layoutFrame.minY) }
        #endif
        var navigation = presenter.navigation.observation()
        navigation["presentation"] = presenter.modals.presentation ?? NSNull()
        navigation["closedby"] = presenter.modals.closedby ?? NSNull()
        navigation["owners"] = presenter.modals.routes.map { ["route": $0.node.props["navigationKey"] ?? "", "presentation": $0.kind] }
        navigation["source"] = presenter.modals.routes.last?.node.props["navigationSource"] ?? NSNull()
        navigation["popover"] = presenter.menus.observation() ?? NSNull()
        // @ref LLP 1038 D11 — last op, never inferred from route props.
        navigation["url"] = session.routerOp?["url"] ?? NSNull()
        // The scene's title as UIKit holds it (LLP 1048.003 D1; the app sets it).
        let window: [String: Any] = ["title": presenter.session?.view?.window?.windowScene?.title ?? NSNull()]
        // The list pool and the leaves it holds mid-fling (LLP 1068 §6, §5.1).
        var pool = presenter.pool.observation.merging(presenter.leaves.observation) { a, _ in a }.merging(presenter.flats.observation) { a, _ in a }
        pool["native"] = session.natives.observation
        let kernelState = (try? JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"state\"}").utf8))) as? [String: Any]
        let kernelLayout = kernelState?["kernelLayout"] as? [String: Any] ?? [:]
        let layout = kernelLayout.merging(["provisional": session.fieldChrome.presentedProvisional]) { _, host in host }
        return ["layout": layout, "focus": focus, "keyboard": keyboard, "navigation": navigation, "window": window, "pool": pool, "hatches": presenter.elements.observation(presenter.session?.hatchDiagnostics)]
    }

    /// A view's box in the viewport: the viewport's content space less its
    /// offset — every enclosing scroll node's offset folded in — with the
    /// presentation transform applied (UIKit's conversion carries `transform`),
    /// as the web's `getBoundingClientRect` includes CSS transforms.
    package func box(_ v: UIView, region: CGRect? = nil) -> CGRect {
        let bounds = region ?? v.bounds
        if (v as? NodeView)?.placedAncestor?.placementHidden == true { return .zero }
        let vp = presenter.viewport
        let o = vp.contentOffset
        // Under a child a canvas's surface has placed (LLP 1014 D5): the box
        // where it is seen, through the placement, not the kernel's frame,
        // each corner carried out on its own (`drawnRect`).
        if let n = v as? NodeView, n.placedAncestor?.placement != nil {
            let r = n.drawnRect(bounds, in: vp)
            return CGRect(x: r.origin.x - o.x, y: r.origin.y - o.y, width: r.width, height: r.height)
        }
        let r = v.convert(bounds, to: vp)
        return CGRect(x: r.origin.x - o.x, y: r.origin.y - o.y, width: r.width, height: r.height)
    }

    func layout() -> [String: Any] {
        let vp = presenter.viewport
        var nodes: [[String: Any]] = []
        for (id, v) in presenter.views.sorted(by: { $0.key < $1.key }) where v.window != nil {
            if presenter.menus.ownsConfirmationNode(v) {
                nodes.append(["id": Int(id), "presentation": "native-confirmation", "geometry": "unavailable"])
                continue
            }
            let nativeAction = presenter.swipeActions.ownsAction(id)
            let actionView = presenter.swipeActions.actionView(id)
            let r = nativeAction ? actionView.map { box($0) } ?? .zero : box(v)
            var n: [String: Any] = ["id": Int(id), "x": Agent.r2(r.origin.x), "y": Agent.r2(r.origin.y), "w": Agent.r2(r.width), "h": Agent.r2(r.height)]
            if nativeAction { n["presentation"] = "native-swipe-action"; n["visible"] = actionView != nil }
            // A waiting scroll (a closed swipe row's) is at its start.
            if v.scrollDormant { n["sx"] = 0.0; n["sy"] = 0.0 }
            if let sv = v.scrollView {
                n["sx"] = Agent.r2(sv.contentOffset.x)
                n["sy"] = Agent.r2(sv.contentOffset.y + v.scrollTopInset(sv))
                // How far past its own ends it sits: a stretched bounce is a
                // state a driver cannot read from the offset alone.
                let past = { (value: CGFloat, start: CGFloat, end: CGFloat) -> CGFloat in
                    if value < start - 0.5 { return value - start }
                    if value > end + 0.5 { return value - end }
                    return 0
                }
                let i = sv.adjustedContentInset
                let ox = past(sv.contentOffset.x, -i.left, max(-i.left, sv.contentSize.width + i.right - sv.bounds.width))
                let oy = past(sv.contentOffset.y, -i.top, max(-i.top, sv.contentSize.height + i.bottom - sv.bounds.height))
                if ox != 0 { n["ox"] = Agent.r2(ox) }
                if oy != 0 { n["oy"] = Agent.r2(oy) }
            }
            nodes.append(n)
        }
        // The page's environment (LLP 1012 §1): the insets the kernel was
        // given, and the keyboard's inset on the viewport, by the web's
        // `env()` names.
        let i = presenter.insets
        let env: [String: Any] = ["safe-area-inset-top": Agent.r2(i.top), "safe-area-inset-right": Agent.r2(i.right), "safe-area-inset-bottom": Agent.r2(i.bottom), "safe-area-inset-left": Agent.r2(i.left), "keyboard-inset-height": Agent.r2(presenter.keyboardInset)].merging(presenter.fold.env) { a, _ in a }
        var reply: [String: Any] = ["clock": session.now(), "viewport": ["w": Agent.r2(vp.bounds.width), "h": Agent.r2(vp.bounds.height)], "env": env, "nodes": nodes]
        #if os(iOS)
        // The status bar's style Exact asks for (LLP 1105 D7), and the node that decided it.
        let bar = presenter.statusBar
        reply["statusBar"] = ["style": bar.style == .lightContent ? "light-content" : bar.style == .darkContent ? "dark-content" : "default",
                              "source": bar.source.map { Int($0) } ?? NSNull()] as [String: Any]
        #endif
        // The device's screen, where the viewport sits on it (LLP 1035.002
        // D4's `screen` space) and the scene's interface orientation: what a
        // real touch's aim is checked against (LLP 1080.000 D4).
        if let w = vp.window, let container = vp.superview {
            let screen = w.screen
            let origin = container.convert(vp.frame.origin, to: screen.coordinateSpace)
            reply["screen"] = ["w": Agent.r2(screen.bounds.width), "h": Agent.r2(screen.bounds.height), "scale": Agent.r2(screen.scale),
                               "x": Agent.r2(origin.x), "y": Agent.r2(origin.y), "orientation": Agent.orientation(w.windowScene)]
        }
        return reply
    }

    /// `layout <node>` (LLP 1035.002 D1): the runner's rows and their sources
    /// for one node, then what UIKit knows about it — its box in the viewport,
    /// the window and the screen, the scroll and clip chains above it, whether
    /// it is hidden, inert, in the viewport or clipped away, and what was
    /// mounted for it. Observations, never a second model; a stale id is
    /// refused by name. An inline run's geometry is its paragraph's.
    func layout(_ req: [String: Any]) -> [String: Any] {
        var reply = layout()
        guard let id = req["id"] as? Int else { return reply }
        guard let v = presenter.textHost(UInt32(id)) else { return ["error": "stale node #\(id)"] }
        let includePlan = req["plan"] as? Bool == true
        guard let d = session.agent("{\"op\":\"node\",\"id\":\(id),\"plan\":\(includePlan)}").data(using: .utf8),
              var node = (try? JSONSerialization.jsonObject(with: d)) as? [String: Any] else { return ["error": "node #\(id): unreadable"] }
        if let e = node["error"] { return ["error": e] }
        if presenter.menus.ownsConfirmationNode(v) {
            node["space"] = ["unavailable": "UIAlertAction exposes no public action geometry"]
            node["native"] = ["presentation": "confirmation"]
            node["visible"] = ["unavailable": "use the native presentation observation and a window capture"]
            reply["node"] = node
            return reply
        }
        if presenter.swipeActions.ownsAction(UInt32(id)) {
            let button = presenter.swipeActions.actionView(UInt32(id))
            func rect(_ r: CGRect) -> [String: Any] { ["x": Agent.r2(r.minX), "y": Agent.r2(r.minY), "w": Agent.r2(r.width), "h": Agent.r2(r.height)] }
            var space: [String: Any] = ["capture": ["scale": Agent.r2(presenter.viewport.traitCollection.displayScale)]]
            if let button, let window = button.window {
                let frame = button.convert(button.bounds, to: window)
                space["viewport"] = rect(box(button)); space["window"] = rect(frame)
                space["screen"] = rect(window.convert(frame, to: window.screen.coordinateSpace))
                space["local"] = ["w": Agent.r2(button.bounds.width), "h": Agent.r2(button.bounds.height)]
            } else { space["unavailable"] = "native swipe action is not revealed or cannot be uniquely resolved" }
            node["space"] = space
            node["visible"] = ["hidden": button == nil, "inert": v.disabled, "inViewport": button != nil]
            node["native"] = ["view": "UIKit swipe action", "accessibilityLabel": button?.accessibilityLabel ?? "", "geometryAvailable": button != nil, "activationEvents": button?.allControlEvents.rawValue ?? 0]
            node["observed"] = ["clock": session.now(), "wall": Date().timeIntervalSince1970 * 1000]
            reply["node"] = node
            return reply
        }
        let host = v.paragraphOwner
        // @ref LLP 1043.000 §3 D7 — diagnose actual painted fragments, not a replay.
        if !host.flowShapes.isEmpty, let paragraph = host.paragraphLayout() {
            node["flow"] = paragraph.flowFacts
        }
        let vp = presenter.viewport
        let rect = { (r: CGRect) -> [String: Any] in ["x": Agent.r2(r.origin.x), "y": Agent.r2(r.origin.y), "w": Agent.r2(r.width), "h": Agent.r2(r.height)] }
        let b = box(host)
        var space: [String: Any] = ["viewport": rect(b), "local": ["w": Agent.r2(host.bounds.width), "h": Agent.r2(host.bounds.height)],
                                    "capture": ["scale": Agent.r2(vp.window?.screen.scale ?? vp.traitCollection.displayScale)]]
        if let w = host.window {
            let inWindow = host.convert(host.bounds, to: w)
            space["window"] = rect(inWindow)
            space["screen"] = rect(w.convert(inWindow, to: w.screen.coordinateSpace))
        }
        // While the node's own layer animates, the frame on screen is the
        // presentation layer's, reported beside the model's (LLP 1035.002
        // D4) and never fabricated: absent when nothing is in flight.
        if let shown = host.layer.presentation()?.frame, shown != host.layer.frame {
            let d = CGPoint(x: shown.minX - host.layer.frame.minX, y: shown.minY - host.layer.frame.minY)
            space["presented"] = rect(CGRect(x: b.minX + d.x, y: b.minY + d.y, width: shown.width, height: shown.height))
        }
        node["space"] = space
        // Who hides or inerts it is named: a reader must not guess which
        // ancestor did.
        let describe = { (view: UIView) -> String in (view as? NodeView).map { "#\($0.id)" } ?? String(describing: Swift.type(of: view)) }
        var hiddenBy: String? = host.isHidden || (host.alpha == 0 && host.placement == nil) ? describe(host) : nil
        var inertBy: String? = host.isUserInteractionEnabled ? nil : describe(host)
        var clipped = b.isEmpty
        var chain: [[String: Any]] = []
        var clippers: [(NodeView, String)] = []
        var canvas: NodeView? = nil
        var above = host.superview
        while let s = above {
            // A canvas child's overlay is transparent on purpose: the child
            // is painted through the canvas's capture (LLP 1014 D5) and only
            // hit here. That is not hidden; it is named.
            if let o = s as? PlainView, let c = o.superview as? NodeView, c.overlay === o {
                canvas = c
                above = s.superview
                continue
            }
            if hiddenBy == nil, s.isHidden || (s.alpha == 0 && (s as? NodeView)?.placement == nil) { hiddenBy = describe(s) }
            if inertBy == nil, !s.isUserInteractionEnabled { inertBy = describe(s) }
            if let n = s as? NodeView {
                if let sv = n.scrollView { chain.append(["id": Int(n.id), "sx": Agent.r2(sv.contentOffset.x), "sy": Agent.r2(sv.contentOffset.y)]) }
                else if n.scrollDormant { chain.append(["id": Int(n.id), "sx": 0.0, "sy": 0.0]) }
                if n.clipsToBounds || n.clipBox != nil { clippers.append((n, "overflow")) }
                if n.clipPath != nil { clippers.append((n, "clip-path")) }
            }
            above = s.superview
        }
        var scroll: [[String: Any]] = [["viewport": true, "sx": Agent.r2(vp.contentOffset.x), "sy": Agent.r2(vp.contentOffset.y)]]
        scroll.append(contentsOf: chain.reversed())
        var clip: [[String: Any]] = []
        for (n, kind) in clippers.reversed() {
            clip.append(["id": Int(n.id), "kind": kind])
            if let w = host.window, !host.convert(host.bounds, to: w).intersects(n.convert(n.bounds, to: w)) { clipped = true }
        }
        node["scroll"] = scroll
        node["clip"] = clip
        // CSS `visibility: hidden` with nothing of it showing, or a hidden run (e28279b3b keeps the view).
        if hiddenBy == nil, !host.accessibilityExposed || presenter.inlineText(UInt32(id))?.hidden == true { hiddenBy = "visibility" }
        var visible: [String: Any] = ["hidden": hiddenBy != nil, "inert": inertBy != nil, "inViewport": b.intersects(CGRect(origin: .zero, size: vp.bounds.size)), "clipped": clipped]
        if let hiddenBy { visible["hiddenBy"] = hiddenBy }
        if let inertBy { visible["inertBy"] = inertBy }
        node["visible"] = visible
        var native: [String: Any] = ["view": String(describing: Swift.type(of: v)), "sheet": presenter.modals.active]
        if presenter.inlineText(UInt32(id)) != nil { native["inline"] = true }
        if let canvas { native["canvas"] = "#\(canvas.id)" }
        if let f = host.field { native["editor"] = String(describing: Swift.type(of: f)); native["firstResponder"] = f.isFirstResponder }
        if let t = host.textArea { native["editor"] = String(describing: Swift.type(of: t)); native["firstResponder"] = t.isFirstResponder }
        if let m = host.materialKind { native["effect"] = m; if m != "backdrop" { native["material"] = Materials.agentMaterial(m) } }
        host.glassAgentFields(&native)
        if presenter.leaves.isPending(host) { native["pending"] = true }
        if let segment = presenter.segments.observation(host) { native["segmentedControl"] = segment }
        if let grouped = presenter.groupedLists?.observation(host) { native["groupedList"] = grouped }
        if let control = presenter.controls.observation(host) { native["control"] = control }
        var responder: UIResponder? = host
        while let current = responder {
            if let vc = current as? UIViewController { native["controller"] = String(describing: Swift.type(of: vc)); break }
            responder = current.next
        }
        if let key = presenter.navigation.routeKey(containing: host) { native["route"] = key }
        if let sheet = presenter.modals.coordinateView, host.isDescendant(of: sheet) { native["presentation"] = presenter.modals.presentation == "fullscreen" ? "fullscreen" : "sheet" }
        if let leaf = host.symbolView {
            let source = host.imageSource ?? "", name = host.props["symbolName"] ?? ""
            let points = max(0, host.number("font_size", 16))
            let size = leaf.image?.size ?? CGSize(width: points, height: points)
            var symbol: [String: Any] = ["renderer": String(describing: Swift.type(of: leaf)), "source": source, "name": name, "found": host.symbolFound, "intrinsic": [Agent.r2(size.width), Agent.r2(size.height)], "frame": rect(box(leaf))]
            if !host.symbolFound { symbol["reason"] = source == "symbol:sf/" ? "empty" : name.isEmpty ? "role" : "os" }
            native["symbol"] = symbol
        }
        node["native"] = native
        node["observed"] = ["clock": session.now(), "wall": Date().timeIntervalSince1970 * 1000]
        reply["node"] = node
        return reply
    }

    func view(_ req: [String: Any]) -> NodeView? {
        guard let id = req["id"] as? Int else { return nil }
        return presenter.textHost(UInt32(id))
    }

    /// The agent's `reveal` (ledger F7, shop F11): before a tap or a type, a
    /// view whose middle is out of view is scrolled to the middle of each
    /// enclosing scroll view it is outside of, innermost first, then the
    /// page's — as the web's `scrollIntoView` does there (block centre,
    /// inline only as far as it takes) — by `scrollRectToVisible`, unanimated,
    /// as far as each one's range allows; their delegates tell the app, as a
    /// finger's scroll does.
    func reveal(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), v.window != nil else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        let from = box(v)
        var scrolled = false
        for case let sv as UIScrollView in sequence(first: v.superview, next: { $0?.superview }).compactMap({ $0 }) where sv.isScrollEnabled {
            let frame = v.convert(v.bounds, to: sv), port = sv.bounds, mid = CGPoint(x: frame.midX, y: frame.midY)
            if port.contains(mid) { continue }
            var rect = port
            if mid.y < port.minY || mid.y >= port.maxY { rect.origin.y = mid.y - port.height / 2 }
            if mid.x < port.minX || mid.x >= port.maxX { rect.origin.x = frame.minX; rect.size.width = frame.width }
            sv.scrollRectToVisible(rect, animated: false)
            scrolled = true
        }
        guard scrolled else { return ["revealed": Int(v.id), "scrolled": false] }
        presenter.settlePump()
        let to = box(v)
        return ["revealed": Int(v.id), "scrolled": to.origin != from.origin,
                "from": [Agent.r2(from.midX), Agent.r2(from.midY)], "to": [Agent.r2(to.midX), Agent.r2(to.midY)]]
    }

    func tap(_ req: [String: Any]) -> [String: Any] {
        if let reply = touchForm(req) { return reply }
        if view(req)?.placedAncestor?.placementHidden == true { return ["error": "placed child is hidden"] }
        if req["phase"] == nil, req["wheel"] == nil,
           let node = view(req), node.isDescendant(of: presenter.viewport), presenter.groupedLists?.draws(node.id) != true,
           // A swipe action's control sits past its row's edge until a swipe
           // reveals it; its tap is the action's (below), wherever it sits.
           !presenter.swipeActions.ownsAction(node.id) {
            guard let point = tapPoint(req, node: node) else {
                return ["error": "tap #\(req["id"] ?? node.id): no visible text fragment; scroll it into view first"]
            }
            if !CGRect(origin: .zero, size: presenter.viewport.bounds.size).contains(point) {
                return ["error": "tap #\(node.id): its middle is outside the viewport; scroll it into view first"]
            }
        }
        if let reply = canvasTap(req) { return reply }
        if req["phase"] == nil, req["wheel"] == nil, req["x"] == nil, req["y"] == nil,
           let id = req["id"] as? UInt32, let run = presenter.inlineText(id), let node = presenter.textHost(id) {
            guard node.window != nil, !node.inert, !node.disabled, !run.hidden else { return ["error": "inline node #\(id) is unavailable"] }
            if req["hover"] as? Bool == true {
                presenter.hoverInline(run.handlers.contains("hover") ? id : nil)
                return ["tapped": Int(id), "hover": true]
            }
            if node.activateInline(id) { return ["tapped": Int(id), "delivery": "host-activation", "native": "inline-text"] }
        }

        // A held contact (LLP 1035.003 D1) needs a touch that stays down
        // across requests, which XCTest's touches do not (LLP 1080.000 P3):
        // the iOS carrier says so rather than activating a node and calling it
        // a finger (D3) — except on a `pan` node, where it delivers the
        // recognized pan (`AgentPanIOS.swift`, LLP 1057 §10.6).
        if let phase = req["phase"] as? String {
            if let reply = recognizedPan(phase, req) { return reply }
            return ["phase": phase, "delivery": "unsupported", "reason": "no held contact across requests on iOS (LLP 1080.000 P3)"]
        }
        // The painted popovers under the agent (LLP 1021 D4): a tap opens,
        // closes or dismisses them first, then is delivered as any tap.
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil {
            presenter.menus.agentTap(node)
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let activated = presenter.menus.activate(node) {
            return activated ? ["tapped": id, "delivery": "host-activation", "native": "confirmation"]
                : ["error": "confirmation #\(id) is unavailable, transitioning, or its source is no longer active"]
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let reply = presenter.groupedLists?.activate(node) {
            return reply
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let activated = presenter.controls.activate(node) {
            if let unsupported = presenter.controls.unopened(node) { return unsupported }
            return activated ? ["tapped": id, "delivery": "host-activation", "native": "control"]
                : ["error": "control #\(id) is disabled, inert or not shown"]
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let activated = presenter.segments.activate(node) {
            return activated ? ["tapped": id, "delivery": "host-activation", "native": "segmented-control"]
                : ["error": "native segment #\(id) is unavailable"]
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let reply = presenter.navigation.activateChrome(node) {
            return reply
        }
        if let id = req["id"] as? Int, presenter.swipeActions.ownsAction(UInt32(id)),
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil {
            guard let button = presenter.swipeActions.actionView(UInt32(id)), let window = button.window,
                  let source = presenter.views[UInt32(id)], !source.disabled else {
                // Not revealed: the action as assistive technology performs
                // it, as the web's tap scrolls the row to it (splitter rough
                // 12). A real swipe is `tap <row> drag …` (`--touch platform`).
                if presenter.views[UInt32(id)]?.disabled == false, presenter.swipeActions.perform(UInt32(id)) == true {
                    return ["tapped": id, "pressed": id, "delivery": "host-activation", "native": "swipe-action", "revealed": false]
                }
                return ["error": "native swipe action #\(id) is not offered: disabled, hidden or inert, or its name is not unique"]
            }
            let b = box(button), point = button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.midY), to: window)
            guard let hit = window.hitTest(point, with: nil), hit === button || hit.isDescendant(of: button) else { return ["error": "native swipe action #\(id) is occluded"] }
            let event: UIControl.Event = button.allControlEvents.contains(.primaryActionTriggered) ? .primaryActionTriggered : .touchUpInside
            button.sendActions(for: event)
            return ["tapped": id, "at": [Agent.r2(b.midX), Agent.r2(b.midY)], "delivery": "host-activation", "native": "swipe-action"]
        }
        // A wheel on what a grouped list draws scrolls that list, even when
        // the node is a custom row's view its cell's reuse took off screen.
        if let wheel = req["wheel"] as? [Double], wheel.count == 2, wheel.allSatisfy(\.isFinite),
           let id = req["id"] as? Int, let list = presenter.groupedLists?.scroller(for: UInt32(id)), presenter.views[UInt32(id)]?.window == nil {
            Agent.scroll(from: list, dx: CGFloat(wheel[0]), dy: CGFloat(wheel[1]))
            return ["tapped": id, "wheel": wheel]
        }
        guard let v = view(req), let win = v.window else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        let b = v.tapBox(box(v))
        // The middle of the box as seen — through a surface's placement when
        // there is one (LLP 1014 D5) — as a point in the window.
        let vp = presenter.viewport
        let p = vp.convert(CGPoint(x: b.midX + vp.contentOffset.x, y: b.midY + vp.contentOffset.y), to: nil)
        let at = [Agent.r2(b.midX), Agent.r2(b.midY)]
        let seen = win.hitTest(p, with: nil)
        // A finger lands only where the target is seen (LLP 1035.003: action
        // dispatch is never substituted for a contact), so a press, a menu or
        // a double click is refused, having changed nothing, when the software
        // keyboard or another view is over it. Off-viewport targets must be
        // scrolled into view first, as on the web carrier.
        if req["wheel"] == nil, let why = offscreen(v, box: b, hit: seen) {
            return ["error": "tap #\(v.id): \(why)"]
        }
        let hit = seen ?? v
        if req["wheel"] == nil, req["hover"] == nil, let why = obscured(v, at: p, hit: hit) {
            return ["error": "tap #\(v.id) at (\(at[0]), \(at[1])): \(why)"]
        }
        if req["contextmenu"] as? Bool == true || req["dblclick"] as? Bool == true {
            let event = req["contextmenu"] as? Bool == true ? "contextmenu" : "dblclick"
            var next: UIView? = hit
            while let node = next {
                // A context menu's popover (LLP 1021 §5.1) answers it too: the
                // node's own action first, then the popover opens painted.
                if let target = node as? NodeView, !target.disabled, target.handlers.contains(event)
                    || (event == "contextmenu" && target.props["contextPopover"]?.isEmpty == false) {
                    if event == "contextmenu" {
                        let handled = target.handlers.contains(event)
                        if handled { presenter.contextmenu(target.id) }
                        // A node with only a popover that cannot open it is not the target.
                        if !presenter.menus.agentContext(target) && !handled { next = node.superview; continue }
                    } else { presenter.dblclick(target.id) }
                    return ["tapped": Int(target.id), "event": event, "injected": true, "at": at]
                }
                next = node.superview
            }
            return ["error": "no \(event) handler at view \(v.id)"]
        }
        if let scale = req["pinch"] as? Double {
            // UIKit synthesizes no pinch: the recognized scale (LLP 1057.001 §5).
            var next: UIView? = v
            while let view = next, !((view as? NodeView).map { presenter.transformBindings[$0.id]?.target != nil } ?? false) { next = view.superview }
            guard let handle = next as? NodeView, let clipID = presenter.transformBindings[handle.id]?.clip, let clip = presenter.views[clipID] else {
                return ["error": "no photo binding (transformDragFor) at view \(v.id)"]
            }
            let offset = (req["at"] as? [Double]).map { CGPoint(x: $0[0], y: $0[1]) } ?? CGPoint(x: handle.bounds.midX, y: handle.bounds.midY)
            let point = handle.convert(offset, to: clip)
            if let error = DragLink.installed?.recognizedPinch(handle, scale: scale, focal: CGPoint(x: point.x - clip.bounds.midX, y: point.y - clip.bounds.midY)) { return ["error": error] }
            return ["tapped": Int(handle.id), "pinch": scale, "at": at, "delivery": "recognized"]
        }
        if req["hover"] as? Bool == true {
            // The pointer onto the target: the node with a hover handler at
            // the hit point enters, whatever was hovered leaves (UIKit
            // offers no pointer synthesis; a real one is the hover recognizer).
            var n: UIView? = hit
            while let cur = n, !((cur as? NodeView)?.handlers.contains("hover") ?? false) { n = cur.superview }
            if let node = n as? NodeView { presenter.hover(node, true) } else if let h = presenter.hovered { presenter.hover(h, false) }
            return ["tapped": Int(v.id), "hover": true, "at": at]
        }
        if let wheel = req["wheel"] as? [Double], wheel.count == 2 {
            // The web's sign (a positive dy scrolls down), points.
            guard wheel.allSatisfy(\.isFinite) else { return ["error": "wheel deltas must be finite"] }
            // A native standard row has no visible authored hit. Custom
            // rows keep their real nested scroll containers in the hit path.
            let projected = presenter.groupedLists?.draws(v.id) == true ? presenter.groupedLists?.scroller(for: v.id) : nil
            Agent.scroll(from: projected ?? hit, dx: CGFloat(wheel[0]), dy: CGFloat(wheel[1]))
            if ExactEnv.agentFreezes { presenter.settlePump() }
            return ["tapped": Int(v.id), "wheel": wheel, "at": at]
        }
        if v.kind == "iframe" { return session.webviews.tap(v, request: req, at: at) }
        var n: UIView? = hit
        while let cur = n, !(cur is NodeView) { n = cur.superview }
        // What a touch up does first (`NodeView.touchesEnded`, up the
        // responder chain): the nearest node that takes the focus takes it
        // — an input's field, a node with a focus/blur/key handler — and
        // whatever had it (a field, and the keyboard with it) lets go.
        // An SVG element under the finger takes the press (LLP 1055.000 D17).
        let element = (n as? NodeView).flatMap { $0.kind == "svg" && !$0.inert ? presenter.svg.target($0.id, at: $0.local(p)) : nil }
        // A Markdown run's link under the finger is the press, as `touchesEnded` follows it.
        if element == nil, let node = n as? NodeView, node.inlineActivationTarget(at: node.local(p)) == nil,
           let href = node.inlineLink(at: node.local(p)) {
            presenter.session?.follow(href)
            return ["tapped": Int(v.id), "at": at, "followed": href]
        }
        let action = element == nil ? (n as? NodeView)?.activationTarget(at: p) : nil
        var f: UIView? = n
        var took = false
        while let cur = f {
            if let node = cur as? NodeView, let field = (node.textArea as UIView?) ?? node.field { if !field.isFirstResponder { _ = field.becomeFirstResponder() }; took = true; break }
            if cur.canBecomeFirstResponder {
                // Under `retainFocus` the press takes nothing (`touchesEnded`).
                if !presenter.contextRetainsFocus(cur) { if !cur.isFirstResponder { _ = cur.becomeFirstResponder() }; took = true }
                break
            }
            // A pressed node handles touchesEnded without forwarding it to
            // its parent. An enclosing key handler must not steal the editor.
            if cur === action { break }
            f = cur.superview
        }
        // Nothing took the focus: the field being edited loses it (a page
        // blurs its input on a click anywhere else), and the keyboard goes.
        if !took && !presenter.contextRetainsFocus(n ?? v) { presenter.viewport.endEditing(true) }
        var pressed: Any = NSNull()
        // An iPad's hardware keys held through the tap (gallery F20).
        let held = (req["modifiers"] as? String).map { $0.hasSuffix("+") || $0.isEmpty ? $0 : $0 + "+" } ?? ""
        if let element { presenter.press(element, held: held); pressed = Int(element) }
        if let action, presenter.views[action.id] === action { presenter.press(action.id, held: held); action.finishPointerPress(); pressed = Int(action.id) }
        return ["tapped": Int(v.id), "at": at, "pressed": pressed]
    }

    /// Why `v`'s middle is not on screen for a finger, or nil: off the
    /// viewport it scrolls in, or off the window.
    func offscreen(_ v: NodeView, box b: CGRect, hit: UIView?) -> String? {
        let vp = presenter.viewport
        if v.isDescendant(of: vp), !CGRect(origin: .zero, size: vp.bounds.size).contains(CGPoint(x: b.midX, y: b.midY)) {
            return "its middle is outside the \(Agent.r2(vp.bounds.width))×\(Agent.r2(vp.bounds.height)) viewport; a finger would scroll it into view first"
        }
        return hit == nil ? "its middle is off the window" : nil
    }

    /// Why a finger at the middle of `v` (`p`, in the window) would land on
    /// something else, or nil: the software keyboard is a window of its own,
    /// which the app's hit test never sees; any other view is named. A node
    /// ancestor taking the touch is how a finger reaches a node that takes
    /// none itself. `v` may be a native view standing for a node (a
    /// grouped list's cell or accessory).
    func obscured(_ v: UIView, at p: CGPoint, hit: UIView) -> String? {
        if let container = presenter.modals.coordinateView ?? presenter.session?.view,
           let top = presenter.keyboardGuideTop(in: container), p.y >= container.convert(CGPoint(x: 0, y: top), to: nil).y {
            return "its middle is under the software keyboard; dismiss it or scroll the target above it first (state shows keyboard.top)"
        }
        if hit === v || hit.isDescendant(of: v) || (hit is NodeView && v.isDescendant(of: hit)) { return nil }
        return "\((hit as? NodeView).map { "node #\($0.id)" } ?? String(describing: Swift.type(of: hit))) covers its middle"
    }

    /// The web's rule for a phase-less wheel tick (LLP 1070 G2, Chrome's,
    /// measured in its §2): from the hit view up, the first scroll container
    /// that can take any of the tick's components takes the ones it can, and
    /// the rest is dropped; none other moves. One that can take none passes
    /// it on (`overscroll-behavior: auto`), unless the tick's axis is
    /// `contain` or `none` there, which keeps it. Whole points, bounded.
    static func scroll(from hit: UIView, dx: CGFloat, dy: CGFloat) {
        let dx = min(max(dx.rounded(), -1_000_000), 1_000_000), dy = min(max(dy.rounded(), -1_000_000), 1_000_000)
        var v: UIView? = hit
        while let cur = v {
            // A waiting scroll (a closed swipe row's) scrolls as the wheel asks.
            var target: UIScrollView? = cur as? ScrollView
            if let waiting = cur as? NodeView, waiting.scrollDormant { waiting.needScroll(); target = waiting.scrollView }
            // A grouped list's collection view scrolls in its place (LLP 1084 D8).
            if target == nil, cur is GroupedScroller { target = cur as? UIScrollView }
            if let sv = target {
                let scrollsX = (sv as? ScrollView)?.scrollsX ?? false, scrollsY = (sv as? ScrollView)?.scrollsY ?? true
                // Native bars and keyboard avoidance can make the resting
                // start negative. Their insets are part of the usable range.
                let i = sv.adjustedContentInset
                let minX = -i.left, minY = -i.top
                let maxX = max(minX, sv.contentSize.width + i.right - sv.bounds.width), maxY = max(minY, sv.contentSize.height + i.bottom - sv.bounds.height)
                let o = sv.contentOffset
                let takeX = scrollsX && dx != 0 && maxX > minX && ((dx > 0 && o.x < maxX) || (dx < 0 && o.x > minX))
                let takeY = scrollsY && dy != 0 && maxY > minY && ((dy > 0 && o.y < maxY) || (dy < 0 && o.y > minY))
                if takeX || takeY {
                    let target = CGPoint(x: takeX ? min(max(o.x + dx, minX), maxX) : o.x, y: takeY ? min(max(o.y + dy, minY), maxY) : o.y)
                    // A wheel interrupts a smooth correction as a drag does: a
                    // plain write sends no animation end (LLP 1070.000 §11).
                    if let list = sv.superview as? NodeView, let p = list.presenter, p.collections.animating.contains(list.id) {
                        p.collections.animationEnded(list.id, dragging: true)
                    }
                    sv.setContentOffset(target, animated: false)
                    return
                }
                if let owner = sv.superview as? NodeView,
                   (dx != 0 && owner.style["overscroll_behavior_x"]?.string ?? "auto" != "auto")
                    || (dy != 0 && owner.style["overscroll_behavior_y"]?.string ?? "auto" != "auto") { return }
            }
            v = cur.superview
        }
    }

    /// A key typed at a world's canvas, or a node in one, takes the keyboard's route first, as
    /// any other key the agent types here does: the shortcuts, then the
    /// canvas's `key` handlers and its ancestors'. One that takes it ends it
    /// there, as on the web and macOS (the platformer's diary, R8). Nil: the
    /// world has it (`canvasType`).
    func canvasRouted(_ v: NodeView, _ req: [String: Any]) -> [String: Any]? {
        let phase = req["phase"] as? String
        // A chord is its modifiers and its key (b6 review B6): `Shift+F24`
        // or `Control+v` meets the shortcuts with what it holds.
        guard phase == nil || phase == "down", let chord = req["key"] as? String else { return nil }
        let (held, key) = KeyCodes.split(chord)
        guard let device = KeyCodes.device(key), !KeyCodes.modifier(device.code), v.inputCanvas != nil else { return nil }
        if !v.isFirstResponder { _ = v.becomeFirstResponder() }
        var reply: [String: Any] = ["typed": Int(v.id), "key": chord, "delivery": "recognized"]
        if let phase { reply["phase"] = phase }
        // The down was taken; its up belongs to no world.
        let taken = { [self] () -> [String: Any] in
            if phase == "down", let token = req["releaseKey"] as? String { keyReleases[token] = { ["phase": "up", "delivery": "recognized"] } }
            return reply
        }
        #if os(iOS)
        if let node = presenter.shortcut(key: device.key, held: held, focus: v) {
            presenter.press(node.id)
            reply["shortcut"] = Int(node.id)
            return taken()
        }
        #endif
        return presenter.keyDown(at: v.isFirstResponder ? v : nil, device.key, held: held) ? taken() : nil
    }

    /// Set an input's text as typing does: the field focused, all selected,
    /// the text inserted — the field sends one change with the new value.
    func type(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), v.window != nil else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        guard !v.disabled else { return ["error": "view \(v.id) is disabled"] }
        if let edit = req["clipboard"] as? String { return clipboardType(v, edit, req["text"] as? String) }
        if session.canvases.wantsInput(v.id) {
            if let routed = canvasRouted(v, req) { return routed }
            // The world hears the key a chord names, not the chord's string.
            var bare = req
            if let chord = req["key"] as? String { bare["key"] = KeyCodes.split(chord).key }
            return canvasType(v, bare)
        }
        if v.isSurfaceControl, let key = req["key"] as? String, let code = KeyCodes.device(key)?.code, ["Space", "Enter", "NumpadEnter"].contains(code) {
            let phase = req["phase"] as? String
            guard phase == nil || phase == "down" || phase == "up" else { return ["error":"key: not a phase: \(phase!)"] }
            guard v.focusSurfacePointer() else { return ["error":"control cannot take focus"] }
            for step in phase.map({ [$0] }) ?? ["down", "up"] {
                guard v.controlKey(code, down: step == "down") else { return ["error":"control \(v.props["action"] ?? "") refused input"] }
            }
            if phase == "down", let token = req["releaseKey"] as? String {
                keyReleases[token] = { [weak v] in
                    guard let v else { return ["phase":"up", "delivery":"recognized"] }
                    return v.controlKey(code, down: false) ? ["phase":"up", "delivery":"recognized"] : ["error":"control refused release"]
                }
            }
            return ["typed":v.id, "key":key, "delivery":"recognized"]
        }
        if req["key"] == nil, let reply = presenter.controls.type(v, req["text"] as? String ?? "") { return reply }
        if v.props["editable"] == "false", req["key"] == nil || ["Enter", "Backspace"].contains(req["key"] as? String ?? "") { return ["error": "view \(v.id) is readonly"] }
        // @ref LLP 1038 D11 — type on the root delivers a location.
        if v.props["navigationBack"] != nil, req["key"] == nil {
            let location = req["text"] as? String ?? ""
            return session.navigate(location) ? ["typed": Int(v.id), "value": location, "delivery": "recognized"] : ["error": "navigate refused"]
        }
        if v.kind == "iframe" { return session.webviews.type(v, request: req) }
        if let key = req["key"] as? String, let device = KeyCodes.device(key), let canvas = v.inputCanvas,
           v.forwardsCanvasKey(device.code) {
            guard v.becomeFirstResponder() else { return ["error": "view takes no focus"] }
            if let routed = canvasRouted(v, req) { return routed }
            let phase = req["phase"] as? String
            guard phase == nil || phase == "down" || phase == "up" else { return ["error":"key: not a phase: \(phase!)"] }
            for step in phase.map({ [$0] }) ?? ["down", "up"] {
                guard session.canvases.input(canvas, ["t": "key", "code": device.code, "key": device.key, "down": step == "down", "repeat": false]) else { return ["error":"surface refused key"] }
            }
            if phase == "down", let token = req["releaseKey"] as? String {
                keyReleases[token] = { [weak self, weak canvas] in
                    if let self, let canvas { guard self.session.canvases.input(canvas, ["t": "key", "code": device.code, "key": device.key, "down": false, "repeat": false]) else { return ["error":"surface refused key release"] } }
                    return ["phase": "up", "delivery": "recognized"]
                }
            }
            return ["typed": v.id, "key": key, "delivery": "recognized"]
        }
        if let key = req["key"] as? String {
            // A key at the target as a hardware keyboard's (KeyEvents.swift):
            // the focus's `key` handlers and its ancestors', then, unless one
            // prevented it, its default — a press, or the editor's edit, made
            // here since UIKit synthesizes no presses. A target that takes no
            // focus leaves it where it is, as the web's `focus()` on one does.
            // A chord's modifiers ride with its key; with Control or Command
            // held a key types nothing, as a keyboard's shortcut does not.
            let (held, bare) = KeyCodes.split(key)
            // The same names as the web. A letter keeps the case the driver
            // named (`P` is "P"); an unknown name is refused, not inserted.
            guard let device = KeyCodes.device(bare) else { return ["error": "key: unsupported key \(bare)"] }
            let name = bare.count == 1 && bare != " " ? bare : device.key
            let types = name.count == 1 && !held.contains("Control+") && !held.contains("Meta+")
            if let f = v.textArea { if !f.isFirstResponder { _ = f.becomeFirstResponder() } }
            else if let f = v.field { if !f.isFirstResponder { _ = f.becomeFirstResponder() } }
            else if v.canBecomeFirstResponder, !v.isFirstResponder { _ = v.becomeFirstResponder() }
            let focus = v.field != nil || v.textArea != nil || v.isFirstResponder || v.handlers.contains("press") ? v : nil
            // The release's `keyup` handlers at the focus (#140): after the
            // down, or when a held key comes up. A modifier's own keydown
            // holds it and its keyup no longer does, as DOM's.
            let phase = req["phase"] as? String, code = device.code, lone = KeyCodes.modifier(code)
            let downHeld = lone ? KeyCodes.held(held, name, true) : held, upHeld = lone ? KeyCodes.held(held, name, false) : held
            // A chord's modifiers are their own keys around it, as a keyboard's
            // (KeyCodes.modifierPresses; Charlie, 2026-10-07): down in order
            // before the shortcuts and the key, up in reverse after its keyup,
            // each keyup without its own bit.
            let presses = KeyCodes.modifierPresses(key)
            let heardUp = { [weak presenter, weak focus] in
                _ = presenter?.keyUp(at: focus, name, held: upHeld, code: code)
                for (i, m) in presses.enumerated().reversed() {
                    _ = presenter?.keyUp(at: focus, m.key, held: i > 0 ? presses[i - 1].held : "", code: KeyCodes.device(m.key)?.code ?? "")
                }
            }
            defer {
                if phase != "down" { heardUp() }
                if phase == "down", let token = req["releaseKey"] as? String { keyReleases[token] = { heardUp(); return ["phase": "up", "delivery": "recognized"] } }
            }
            if phase != "up", req["repeat"] as? Bool != true { for m in presses { _ = presenter.keyDown(at: focus, m.key, held: m.held, code: KeyCodes.device(m.key)?.code ?? "") } }
            // The page's shortcuts first, as the web's capture listener and
            // macOS's `routeKey` hear them (gallery F18, ShortcutsIOS).
            #if os(iOS)
            if phase != "up", let node = presenter.shortcut(key: name, held: held, focus: focus ?? presenter.focusedNode) {
                presenter.press(node.id)
                return ["typed": Int(v.id), "key": key, "shortcut": Int(node.id), "delivery": "recognized"]
            }
            #endif
            if phase != "up", !presenter.keyDown(at: focus, name, held: downHeld, code: code, repeats: req["repeat"] as? Bool == true), let focus {
                if let f = focus.textArea {
                    if name == "Enter" { f.insertText("\n") } else if name == "Backspace" { f.deleteBackward() }
                    else if Agent.caretKey(name, in: f) {} else if types { f.insertText(name) }
                    pendingTextReveal = f as? TextArea
                } else if let f = focus.field as? TextField {
                    f.heard = name
                    if name == "Backspace" { f.deleteBackward() } else if name == "Enter" { _ = focus.textFieldShouldReturn(f) }
                    else if Agent.caretKey(name, in: f) {} else if types { f.insertText(name) }
                    f.heard = nil
                } else if presenter.controls.radioKey(focus, name, held: held) { // x2apps survey #2
                } else if focus.handlers.contains("press") || focus.defaultLink != nil, name == "Enter" || (name == " " && focus.props["href"] == nil) { presenter.press(focus.id) }
            }
            return ["typed": Int(v.id), "key": key, "value": Agent.shownValue(v.textArea?.text ?? v.field?.text ?? "", of: v), "delivery": "recognized"]
        }
        if let f = v.textArea {
            f.becomeFirstResponder()
            f.selectAll(nil)
            f.insertText(TextInputLimit.prefix(req["text"] as? String ?? "", props: v.props))
            // UITextView reveals an insertion asynchronously, including
            // when UIView animations are disabled. Observe it; never seek it.
            pendingTextReveal = f as? TextArea
            return ["typed": Int(v.id), "value": f.text ?? ""]
        }
        guard let f = v.field else { return ["error": "view \(v.id) is not an input"] }
        let text = TextInputLimit.prefix(req["text"] as? String ?? "", props: v.props)
        f.becomeFirstResponder()
        f.selectAll(nil)
        f.insertText(text)
        return ["typed": Int(v.id), "value": Agent.shownValue(f.text ?? "", of: v)]
    }

    /// A hardware keyboard's caret keys in a field, which UIKit performs and
    /// a driver's key does not: ArrowLeft and ArrowRight move the caret (or
    /// collapse a selection to that side), Delete removes what follows it —
    /// through `deleteBackward`, as Backspace does, so the field hears one
    /// edit. The web's names and defaults (`type <field> key Delete`).
    static func caretKey(_ name: String, in field: UIKeyInput & UITextInput) -> Bool {
        guard ["ArrowLeft", "ArrowRight", "Delete"].contains(name), let range = field.selectedTextRange else { return false }
        if name == "Delete" {
            if range.isEmpty {
                guard let next = field.position(from: range.end, offset: 1) else { return true }
                field.selectedTextRange = field.textRange(from: next, to: next)
            }
            field.deleteBackward()
            return true
        }
        let left = name == "ArrowLeft"
        let edge = left ? range.start : range.end
        let to = range.isEmpty ? field.position(from: edge, offset: left ? -1 : 1) ?? edge : edge
        field.selectedTextRange = field.textRange(from: to, to: to)
        return true
    }

    func screenshot(_ req: [String: Any]) -> [String: Any] {
        let loading = settleForPicture()
        // A canvas painting its children through its surface (LLP 1014)
        // shows its last capture: what is pending is captured and rendered
        // at the agent's clock first, as `clock` leaves it.
        if let now = session.clock, session.canvases.waitUntilReady() { session.canvases.settle(now: now) }
        presenter.canvas2d.waitForReplays()
        SvgFilterLive.waitForDraws()
        guard let path = req["path"] as? String else { return ["error": "screenshot needs a path"] }
        let vp = presenter.viewport
        let scale = vp.window?.screen.scale ?? vp.traitCollection.displayScale
        let format = UIGraphicsImageRendererFormat()
        format.scale = scale
        format.opaque = true
        // 8-bit sRGB: on a wide-color screen the renderer would write a
        // 16-bit PNG, which nothing downstream (scripts/png.mjs) reads.
        format.preferredRange = .standard
        let captureView: UIView = req["window"] as? Bool == true ? (vp.window ?? vp) : vp
        let size = captureView.bounds.size
        #if targetEnvironment(simulator)
        // The simulator needs both paths: takeSnapshot alone omits guest
        // text, and drawHierarchy alone rasterizes a blank remote layer —
        // unless a takeSnapshot just flushed it. So the snapshot is taken
        // (the flush), composed as the underlay, and the live view stays
        // visible on top with the real pixels (measured 2026-08-30; the
        // 0-guest-pixel failure returns if either half is dropped).
        Capture.web = session.webviews.snapshots().merging(session.natives.snapshots()) { web, _ in web }
        #else
        // A device capture has the macOS shape: hide every live WKWebView
        // and compose only the arm's takeSnapshot at the owning node.
        Capture.web = session.webviews.snapshots().merging(session.natives.snapshots()) { web, _ in web }
        let hidden = (presenter.views.values.compactMap(\.web) + session.natives.snapshotViews).map { ($0, $0.isHidden) }
        hidden.forEach { $0.0.isHidden = true }
        #endif
        Capture.capturing = true
        // drawHierarchy can reuse a clean backing layer without calling draw.
        // Materialize this turn's web/GPU pictures before composing the hierarchy.
        let captured = presenter.views.values.filter { Capture.web[$0.id] != nil || $0.kind == "canvas" }
        for node in captured { node.setNeedsDisplay(); node.layer.displayIfNeeded() }
        session.natives.redrawForCapture()
        let png = UIGraphicsImageRenderer(size: size, format: format).pngData { _ in
            captureView.drawHierarchy(in: CGRect(origin: .zero, size: size), afterScreenUpdates: true)
        }
        Capture.capturing = false
        #if !targetEnvironment(simulator)
        hidden.forEach { $0.0.isHidden = $0.1 }
        #endif
        Capture.web = [:]
        // Snapshot pixels belong to this capture, not the live backing layers.
        for node in captured { node.setNeedsDisplay() }
        do { try png.write(to: URL(fileURLWithPath: path)) } catch { return ["error": "write \(path): \(error)"] }
        var r: [String: Any] = ["screenshot": path, "w": Agent.r2(size.width), "h": Agent.r2(size.height), "scale": Agent.r2(scale)]
        if req["window"] as? Bool == true { r["window"] = true }
        if loading > 0 { r["imagesPending"] = loading }
        // The software keyboard is not in the capture and the app may stand above it (LLP 1102 §3.17):
        // say so, where the image alone reads as a shortened screen.
        let container = session.presenter.modals.coordinateView ?? session.view
        if let top = container.flatMap({ session.presenter.keyboardGuideTop(in: $0) }) {
            r["keyboard"] = ["visible": true, "top": Agent.r2(top)]
            r["note"] = "the software keyboard is up: it is not in the capture, and the app above it may be shortened (state shows keyboard.top)"
        }
        return r
    }

    /// The system appearance (LLP 1061 D5): the window scene's trait, which
    /// the window's own style — the app's `setScheme` — overrides, as the
    /// Settings switch sits beneath an app's choice.
    func systemScheme(dark: Bool) {
        guard let window = session.presenter.viewport.window else { return }
        window.windowScene?.traitOverrides.userInterfaceStyle = dark ? .dark : .light
        window.updateTraitsIfNeeded()
        session.view?.updateTraitsIfNeeded()
    }
    var systemDark: Bool { session.presenter.viewport.window?.windowScene?.traitCollection.userInterfaceStyle == .dark }
    /// `prefer contrast more` (LLP 1095 D7): the scene's contrast trait, so
    /// platform colours resolve as Increased Contrast shows them.
    func systemContrast(more: Bool) {
        guard let window = session.presenter.viewport.window else { return }
        window.windowScene?.traitOverrides.accessibilityContrast = more ? .high : .normal
        window.updateTraitsIfNeeded()
    }
}
#endif
