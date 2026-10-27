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
// `insertText`; `screenshot` draws the viewport's hierarchy to PNG, including Metal.
// `window: true` or a UIKit presentation captures the window, including its alert, action sheet, popover or sheet.
#if os(iOS) || os(tvOS)
import UIKit

extension Agent {
    /// Listen on the socket; when the driver connects, `ready` goes out and
    /// requests are answered until it hangs up (which ends the process).
    /// `sessions` are what a request's `session` label routes among.
    public static func startSocket(ready: [String: Any], sessions: [(String, ExactSession)]) {
        routes = sessions
        _ = KeyboardMotion.shared // a press waits out the keyboard's motion (`settleForPress`)
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
            // @ref LLP 1012#summary — only pre-handshake startup retries. UIKit
            // lifecycle reads and observers stay on the main thread; the worker
            // waits off-main under one deadline, including inactive time.
            let startup = AgentSocketStartup(active: UIApplication.shared.applicationState == .active)
            let notifications = NotificationCenter.default
            let observers = [
                notifications.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { _ in startup.setActive(true) },
                notifications.addObserver(forName: UIApplication.willResignActiveNotification, object: nil, queue: .main) { _ in startup.setActive(false) },
                notifications.addObserver(forName: UIApplication.willTerminateNotification, object: nil, queue: .main) { _ in startup.cancel() },
            ]
            Thread {
                let fd: Int32
                do { fd = try startup.connect(to: addr) }
                catch {
                    DispatchQueue.main.async { observers.forEach(notifications.removeObserver) }
                    FileHandle.standardError.write(Data("exact: agent startup: \(error)\n".utf8))
                    return
                }
                DispatchQueue.main.async { observers.forEach(notifications.removeObserver) }
                out = FileHandle(fileDescriptor: fd, closeOnDealloc: false)
                reply(announcement)
                DispatchQueue.main.async { watchForeground() }
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
            DispatchQueue.main.async { watchForeground() }
            serve(fd: client)
        }.start()
    }

    var presenter: Presenter { session.presenter }

    /// An input that started or met a sheet's presentation or dismissal
    /// returns once UIKit has finished it, bounded at two seconds (LLP
    /// 1035.003 D5, frozen timing). Unanimated, UIKit still takes a few
    /// frames of the wall to end one, and a sheet over a sheet is presented
    /// only once the one under it has finished presenting: a drive's next
    /// `tap`, a few milliseconds later, found the second sheet's route in
    /// the tree and not on screen (the Bluesky clone's prompt over its muted
    /// words sheet). An alert or action sheet is waited for too: a press on
    /// one of its buttons is dispatched only once UIKit has dismissed it
    /// (`dismissConfirmation`), and a `clock data` straight after the tap
    /// found no press. Platform timing leaves this to `clock settle`.
    /// Answers whether the hosts left their transitions.
    @discardableResult func awaitModalTransitions(bound: TimeInterval = 2) -> Bool {
        let modals = presenter.modals, navigation = presenter.navigation, menus = presenter.menus
        guard modals.inTransition || menus.inTransition else { return true }
        let deadline = Date(timeIntervalSinceNow: bound)
        // The navigation host's pending sync is the next sheet waiting on
        // the one now presenting or leaving.
        while modals.inTransition || navigation.inTransition || menus.inTransition {
            if Date() >= deadline { return false }
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
        }
        return true
    }

    /// Under platform timing UIKit animates an alert's dismissal, and a
    /// chosen button's press lands only after it (`dismissConfirmation`):
    /// the input still returns once it has, bounded at two seconds. Sheets
    /// and pushes are left to `clock settle` there.
    @discardableResult func awaitChosenPress(bound: TimeInterval = 2) -> Bool {
        let deadline = Date(timeIntervalSinceNow: bound)
        while presenter.menus.choosing {
            if Date() >= deadline { return false }
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
        }
        return true
    }

    /// Native transitions, keyboard work and an agent-issued caret reveal
    /// must finish before `clock settle` returns (LLP 1035.003 D5).
    func nativeInFlight() -> Bool {
        if presenter.navigation.inTransition || presenter.modals.inTransition || presenter.menus.inTransition || nativeGeometryInFlight() { return true }
        // A smooth correction (a list following its end, a smooth
        // `scrollIntoView`) and an element reveal are UIKit's scroll
        // animation under platform timing (LLP 1070.000 §11, §14): the fixed
        // point is where it lands, within settle's bound (LLP 1035.003 D5).
        if !presenter.collections.animating.isEmpty || presenter.elementScrolls.revealsInFlight || session.activationInFlight || presenter.launchAutofocusPending { return true }
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
        var pending = [presenter.viewport] + presenter.presentations(includingRetiring: true)
        let now = CACurrentMediaTime()
        var moving = false
        Agent.pruneUnbegun(now: now)
        while let view = pending.popLast() {
            guard !view.isHidden, view.alpha > 0 else { continue }
            // Only one still running (`inFlight`): not one held at speed 0
            // for the agent's clock (a lowered `@keyframes`), repeating for
            // ever, or kept on the layer past its end. Every one is looked
            // at, so ones with no begin time are first seen together.
            for key in view.layer.animationKeys() ?? [] {
                if let a = view.layer.animation(forKey: key), movesGeometry(a), Agent.inFlight(a, on: view.layer, key: key, now: now) { moving = true }
            }
            pending.append(contentsOf: view.subviews)
        }
        return moving
    }

    /// What UIKit knows for `state` (LLP 1035.002 D2): which node holds the
    /// focus and through which responder, a focus still waiting on a
    /// presentation; the keyboard's visibility, overlap, top edge and guide
    /// in the viewport's space, the resize policy, whether a drag is
    /// dismissing it; the route the root names, UIKit's stack by key, the
    /// presentation and its close policy, and the transition's phase.
    func stateSections() -> [String: Any] {
        let vp = presenter.viewport
        var focus: [String: Any] = ["logical": NSNull(), "editor": NSNull(), "responder": NSNull(), "pending": NSNull(),
                                    "native": NSNull(), "events": ResignTrace.events]
        let responding = presenter.views.values
            .filter { $0.isFirstResponder || $0.field?.isFirstResponder == true || $0.textArea?.isFirstResponder == true }
            .min { $0.id < $1.id }
        if let node = responding ?? presenter.editing {
            focus["logical"] = Int(node.id)
            if node.field != nil || node.textArea != nil { focus["editor"] = Int(node.id) }
            let responder: UIResponder = node.textArea ?? node.field ?? node
            focus["responder"] = String(describing: Swift.type(of: responder))
        }
        // The native editor UIKit holds as first responder, by its token (`ResignTrace`)
        if let node = responding, let editor: UIView = node.textArea ?? node.field, editor.isFirstResponder {
            focus["native"] = ResignTrace.token(editor)
        }
        if let pending = presenter.pendingFocusObservation {
            focus["pending"] = pending
        }
        let keyboardContainer = presenter.keyboardContainer
        let keyboardTop = keyboardContainer.flatMap { presenter.keyboardGuideTop(in: $0) }
        var keyboard: [String: Any] = ["visible": keyboardTop != nil, "overlap": Agent.r2(presenter.keyboardInset),
                                       "policy": presenter.keyboardPolicy, "interactive": presenter.interactiveKeyboardDrag]
        if let top = keyboardTop, let container = keyboardContainer {
            keyboard["top"] = Agent.r2(vp.convert(CGPoint(x: 0, y: top), from: container).y - vp.pageOrigin.y)
        }
        #if !os(tvOS)
        if let view = presenter.session?.view, view.hasKeyboardGuide { keyboard["guide"] = Agent.r2(view.keyboardLayoutGuide.layoutFrame.minY) }
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
        Self.box(region ?? v.bounds, of: v, viewport: presenter.viewport)
    }

    static func box(_ bounds: CGRect, of v: UIView, viewport: UIScrollView) -> CGRect {
        if (v as? NodeView)?.placedAncestor?.placementHidden == true { return .zero }
        let vp = viewport
        let o = vp.pageOrigin
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

    /// How far `sv`'s offset travels on each axis from one end to the other (LLP 1012 `rx`/`ry`): the ends the wheel
    /// routes by and `ox`/`oy` measure from, its adjusted insets included, whether or not a wheel or a drag may move it
    /// that way.
    static func range(of sv: UIScrollView) -> [String: Any] {
        let i = sv.adjustedContentInset
        return ["rx": r2(max(0, sv.contentSize.width + i.left + i.right - sv.bounds.width)),
                "ry": r2(max(0, sv.contentSize.height + i.top + i.bottom - sv.bounds.height))]
    }

    /// `v`'s scrollport (LLP 1012 `port`): `sv` less the room UIKit insets its content by (a bar, the keyboard), carried
    /// out as `v`'s box is, through transforms and a canvas's placement.
    func port(of v: NodeView, _ sv: UIScrollView) -> [Double] {
        let f = box(v, region: v.convert(v.scrollPort(sv), from: sv))
        return [f.minX, f.minY, f.width, f.height].map(Agent.r2)
    }


    /// `layout <node>` (LLP 1035.002 D1): the runner's rows and their sources
    /// for one node, then what UIKit knows about it — its box in the viewport,
    /// the window and the screen, the scroll and clip chains above it, whether
    /// it is hidden, inert, in the viewport or clipped away, and what was
    /// mounted for it. Observations, never a second model; a stale id is
    /// refused by name. An inline run's geometry is its fragment union.
    func layout(_ req: [String: Any]) -> [String: Any] {
        var reply = layout()
        guard let id = req["id"] as? Int else { return reply }
        // A flat leaf (FlatLeavesIOS) is a layer in the view it is drawn in: answered as one, its own id kept.
        let leaf = presenter.textHost(UInt32(id)) == nil ? presenter.flats.leaves[UInt32(id)] : nil
        let includePlan = req["plan"] as? Bool == true
        guard let d = session.agent("{\"op\":\"node\",\"id\":\(id),\"plan\":\(includePlan)}").data(using: .utf8),
              var node = (try? JSONSerialization.jsonObject(with: d)) as? [String: Any] else { return ["error": "node #\(id): unreadable"] }
        if let e = node["error"] { return ["error": e] }
        MainActor.assumeIsolated { if let held = HeldState.rows(for: UInt32(id), in: presenter) { node["held"] = held } } // what is held on it (LLP 1122 D8)
        if let leaf, let parent = presenter.flats.host(of: leaf.id), let frame = presenter.flats.frameInHost(leaf.id) {
            let b = box(parent, region: frame)
            node["radii"] = BorderPaint.reduced(BorderPaint.radii(leaf.style, in: CGRect(origin: .zero, size: leaf.frame.size)),
                                                  in: CGRect(origin: .zero, size: leaf.frame.size)).map { [Double($0.width), Double($0.height)] }
            node["space"] = ["viewport": ["x": Agent.r2(b.minX), "y": Agent.r2(b.minY), "w": Agent.r2(b.width), "h": Agent.r2(b.height)],
                             "local": ["w": Agent.r2(leaf.frame.width), "h": Agent.r2(leaf.frame.height)]]
            node["visible"] = ["hidden": leaf.layer.isHidden || leaf.opacity == 0, "inert": false, "inViewport": b.intersects(presenter.reach(of: parent)), "clipped": b.isEmpty]
            node["native"] = ["view": "CALayer", "flatLeaf": true, "parent": "#\(parent.id)", "sheet": presenter.modals.active]
            node["observed"] = ["clock": session.now(), "wall": Date().timeIntervalSince1970 * 1000]
            reply["node"] = node
            return reply
        }
        // A live node with no view of its own (a button face's svg): its rows, nothing mounted.
        guard let v = presenter.textHost(UInt32(id)) else { return Self.unmounted(node, into: reply, now: session.now()) }
        if presenter.menus.ownsConfirmationNode(v) {
            node["space"] = ["unavailable": "UIAlertAction exposes no public action geometry"]
            node["native"] = ["presentation": "confirmation"]
            node["visible"] = ["unavailable": "use the native presentation observation and a window capture"]
            reply["node"] = node
            return reply
        }
        // A bar's segment is answered where UIKit draws it.
        if let scope = presenter.navigation.barSegment(v), let window = scope.control.window {
            func rect(_ r: CGRect) -> [String: Any] { ["x": Agent.r2(r.minX), "y": Agent.r2(r.minY), "w": Agent.r2(r.width), "h": Agent.r2(r.height)] }
            let vp = presenter.viewport, frame = scope.control.convert(scope.segment, to: window)
            node["space"] = ["viewport": rect(scope.control.convert(scope.segment, to: vp).offsetBy(dx: -vp.pageOrigin.x, dy: -vp.pageOrigin.y)), "window": rect(frame),
                             "screen": rect(window.convert(frame, to: window.screen.coordinateSpace)), "local": ["w": Agent.r2(scope.segment.width), "h": Agent.r2(scope.segment.height)],
                             "capture": ["scale": Agent.r2(window.screen.scale)]]
            node["visible"] = ["hidden": false, "inert": v.disabled, "inViewport": true, "clipped": false]
            node["native"] = ["view": "UISegmentedControl segment", "accessibilityLabel": v.accessibleName]
            node["observed"] = ["clock": session.now(), "wall": Date().timeIntervalSince1970 * 1000]
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
        if presenter.inlineText(UInt32(id)) == nil {
            node["radii"] = v.effectiveCornerSizes(in: v.bounds).map { [Double($0.width), Double($0.height)] }
        }
        let host = v.paragraphOwner
        // @ref LLP 1043.000 §3 D7 — diagnose actual painted fragments, not a replay.
        if !host.flowShapes.isEmpty, let paragraph = host.paragraphLayout() {
            node["flow"] = paragraph.flowFacts
        }
        let vp = presenter.viewport
        let rect = { (r: CGRect) -> [String: Any] in ["x": Agent.r2(r.origin.x), "y": Agent.r2(r.origin.y), "w": Agent.r2(r.width), "h": Agent.r2(r.height)] }
        let local = presenter.inlineText(UInt32(id)).map { host.inlineBounds($0) ?? .zero } ?? host.bounds
        let b = presenter.inlineFrame(UInt32(id)) ?? box(host, region: local)
        var space: [String: Any] = ["viewport": rect(b), "local": ["w": Agent.r2(local.width), "h": Agent.r2(local.height)],
                                    "capture": ["scale": Agent.r2(vp.window?.screen.scale ?? vp.traitCollection.displayScale)]]
        if let w = host.window {
            let inWindow = host.convert(local, to: w)
            space["window"] = rect(inWindow)
            space["screen"] = rect(w.convert(inWindow, to: w.screen.coordinateSpace))
        }
        node["geometry"] = geometry(host, region: local)
        if let scroll = host.scroll { node["scrollGeometry"] = Self.scrollGeometry(scroll) }
        node["space"] = space
        // Who hides or inerts it is named: a reader must not guess which
        // ancestor did.
        let describe = { (view: UIView) -> String in (view as? NodeView).map { "#\($0.id)" } ?? String(describing: Swift.type(of: view)) }
        var hiddenBy: String? = host.isHidden || (host.alpha == 0 && host.placement == nil) ? describe(host) : nil
        var inertBy: String? = host.isUserInteractionEnabled ? nil : describe(host)
        var clipped = b.isEmpty
        var chain: [[String: Any]] = []
        var clippers: [(UIView, UInt32, String)] = []
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
            if let sv = s as? ScrollView, let route = sv.superview?.next as? RouteController, route.contentViewport === sv {
                let port = box(sv, region: sv.bounds.inset(by: sv.adjustedContentInset))
                chain.append(["route": Int(route.node.id), "sx": Agent.r2(sv.contentOffset.x), "sy": Agent.r2(sv.contentOffset.y),
                              "port": [port.minX, port.minY, port.width, port.height].map(Agent.r2)].merging(Agent.range(of: sv)) { a, _ in a })
                clippers.append((sv, route.node.id, "viewport"))
            }
            if let n = s as? NodeView {
                if let sv = n.scrollView { chain.append(["id": Int(n.id), "sx": Agent.r2(sv.contentOffset.x), "sy": Agent.r2(sv.contentOffset.y + n.scrollTopInset(sv)), "port": port(of: n, sv)].merging(Agent.range(of: sv)) { a, _ in a }) } // CSS scrollTop, as `tree` reads it
                else if n.scrollDormant { chain.append(["id": Int(n.id), "sx": 0.0, "sy": 0.0, "rx": 0.0, "ry": 0.0]) }
                if n.clipsToBounds || n.clipBox != nil { clippers.append((n, n.id, "overflow")) }
                if n.clipPath != nil { clippers.append((n, n.id, "clip-path")) }
            }
            above = s.superview
        }
        // The viewport's scrolling, or none: a presentation's mount does not scroll (LLP 1123 D2)
        let popoverLayer = PopoverLayer.containing(host)
        let inViewport = popoverLayer.map { presenter.inDocument($0) } ?? (presenter.port(of: host) === vp)
        var scroll: [[String: Any]] = [inViewport ? ["viewport": true, "sx": Agent.r2(vp.contentOffset.x), "sy": Agent.r2(vp.contentOffset.y)].merging(Agent.range(of: vp)) { a, _ in a }
                                                  : ["presentation": true, "sx": 0.0, "sy": 0.0, "rx": 0.0, "ry": 0.0]]
        scroll.append(contentsOf: chain.reversed())
        var clip: [[String: Any]] = []
        for (n, id, kind) in clippers.reversed() {
            clip.append(["id": Int(id), "kind": kind])
            if let w = host.window, !host.convert(host.bounds, to: w).intersects(n.convert(n.bounds, to: w)) { clipped = true }
        }
        node["scroll"] = scroll
        node["clip"] = clip
        // CSS `visibility: hidden` with nothing of it showing, or a hidden run (e28279b3b keeps the view).
        if hiddenBy == nil, !host.accessibilityExposed || presenter.inlineText(UInt32(id))?.hidden == true { hiddenBy = "visibility" }
        let reach = popoverLayer.map { box($0) } ?? presenter.reach(of: host)
        var visible: [String: Any] = ["hidden": hiddenBy != nil, "inert": inertBy != nil, "inViewport": b.intersects(reach), "clipped": clipped]
        if let hiddenBy { visible["hiddenBy"] = hiddenBy }
        if let inertBy { visible["inertBy"] = inertBy }
        node["visible"] = visible
        var native: [String: Any] = ["view": String(describing: Swift.type(of: v)), "sheet": presenter.modals.active]
        if presenter.inlineText(UInt32(id)) != nil { native["inline"] = true }
        if let canvas { native["canvas"] = "#\(canvas.id)" }
        if let f = host.field { native["editor"] = String(describing: Swift.type(of: f)); native["firstResponder"] = f.isFirstResponder }
        #if os(iOS)
        if let stepper = host.stepperObservation { native["stepper"] = stepper }
        #endif
        if let t = host.textArea { native["editor"] = String(describing: Swift.type(of: t)); native["firstResponder"] = t.isFirstResponder }
        if let m = host.materialKind { native["effect"] = m; if m != "backdrop" { native["material"] = Materials.agentMaterial(m) } }
        host.glassAgentFields(&native)
        if presenter.leaves.isPending(host) { native["pending"] = true }
        // Read the owned bar's geometry independently of its accessibility representation
        let navigation = presenter.navigation
        let tabs = navigation.adopts(tablist: host) ? navigation.tabController : nil
        let projectedBar = presenter.segments.bar(of: host.id)
        if tabs != nil || projectedBar != nil {
            let shown = tabs?.shownTabBar ?? NavigationHost.shownBar(projectedBar)
            var observation: [String: Any] = ["visible": shown != nil]
            if let shown { observation["frame"] = rect(box(shown)) }
            native["tabBar"] = observation
        }
        // A route's navigation bar, which draws its header (LLP 1115.000 D5.2)
        if let route = navigation.controllers[host.id], route.node === host, let nav = route.navigationController {
            let shown = !nav.isNavigationBarHidden && nav.navigationBar.window != nil
            var observation: [String: Any] = ["visible": shown]
            if shown { observation["frame"] = rect(box(nav.navigationBar)) }
            native["navigationBar"] = observation
        }
        if let segment = presenter.segments.observation(host) { native["segmentedControl"] = segment }
        if let grouped = presenter.groupedLists?.observation(host) { native["groupedList"] = grouped }
        if let control = presenter.controls.observation(host) { native["control"] = control }
        if let vc = ViewPresentation.controller(for: host) { native["controller"] = String(describing: Swift.type(of: vc)) }
        if let key = presenter.navigation.routeKey(containing: host) { native["route"] = key }
        if let sheet = presenter.modals.coordinateView, host.isDescendant(of: sheet) { native["presentation"] = presenter.modals.presentation == "fullscreen" ? "fullscreen" : "sheet" }
        if let leaf = host.symbolView {
            let source = host.imageSource ?? "", name = host.props["symbolName"] ?? ""
            let points = max(0, host.number("font_size", PageFacts.defaultRootFontSize))
            let size = leaf.image?.size ?? CGSize(width: points, height: points)
            var symbol: [String: Any] = ["renderer": String(describing: Swift.type(of: leaf)), "source": source, "name": name, "found": host.symbolFound, "intrinsic": [Agent.r2(size.width), Agent.r2(size.height)], "frame": rect(box(leaf))]
            if !host.symbolFound { symbol["reason"] = source == "symbol:sf/" || source == "symbol:" ? "empty" : name.isEmpty ? "role" : "os" }
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

    /// `tap {back:true}` (LLP 1116.006 D4): the platform's Back as a person
    /// takes it — a real share sheet's close (`--touch platform`, Share.swift),
    /// an open confirmation's cancel (a keyboard's Escape, a tap
    /// outside), else a pushed route's Back button, else the top sheet's
    /// swipe down. The system's button is no node, so no tap reaches it.
    func back() -> [String: Any] {
        #if os(iOS)
        if ShareSheet.agentBack(in: session) || session.picker.agentBackDocument() { return backReply(("sheet", "")) }
        #endif
        if let cancelled = presenter.menus.agentCancel() { return backReply(("dialog", cancelled ? "" : "the alert is still moving: `clock settle` first")) }
        if let refused = presenter.navigation.agentBack() { return backReply(("route", refused)) }
        return backReply(presenter.modals.agentDismiss().map { ("sheet", $0) })
    }

    func tap(_ req: [String: Any]) -> [String: Any] {
        if req["holdState"] != nil { return holdTap(req) } // a held state (LLP 1122 D8, HeldState.swift)
        // A drag of files from outside the app is the Mac's and the web's (FileDragMac.swift).
        if req["drop"] != nil || req["dragenter"] != nil || req["dragleave"] != nil { return ["delivery": "unsupported", "reason": "iOS has no drag of files from outside the app: no drop, dragenter or dragleave"] }
        // A header node the app's navigation bar leaves out is not there to
        // tap, whatever the agent's chrome paints (LLP 1116 D2).
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)], let why = presenter.navigation.unshownByBar()[node.id] {
            return ["error": "tap #\(id)\(node.props["testId"].map { " [\($0)]" } ?? ""): \(why)"]
        }
        if let reply = touchForm(req) { return reply }
        // One finger at a time: a press while the held contact is down would
        // nest a second gesture on its pointer, as a second `down` would.
        if req["phase"] == nil, req["wheel"] == nil, req["hover"] == nil, panContact != nil {
            return ["error": "a contact is already down; use `tap up` or `tap cancel` first"]
        }
        settleForPress()
        // Each synthetic contact ends a live press's handoff before it is
        // delivered, as a real one's touch-down does (PressHandoffIOS).
        presenter.supersedePress()
        // A press delivered lets a hold go where it lands (HeldState.swift), never one refused: a host activation
        // once it took (`delivered`), a finger once its hit is checked (below and `recognizedPan`).
        let delivered = { [presenter] (reply: [String: Any], at: UIView?) -> [String: Any] in
            if reply["error"] == nil { MainActor.assumeIsolated { if let node = at as? NodeView { HeldState.input(on: node.id, in: presenter) } else { HeldState.input(at: at) } } }
            return reply
        }
        if req["scrollDrag"] != nil { return scrollDrag(req) }
        if view(req)?.placedAncestor?.placementHidden == true { return ["error": "placed child is hidden"] }
        if req["phase"] == nil, req["wheel"] == nil,
           let node = view(req), presenter.mounts(node), presenter.groupedLists?.draws(node.id) != true,
           // A swipe action's control sits past its row's edge until a swipe
           // reveals it; its tap is the action's (below), wherever it sits.
           !presenter.swipeActions.ownsAction(node.id) {
            guard let point = tapPoint(req, node: node) else {
                return ["error": "tap #\(req["id"] ?? node.id): no visible text fragment; scroll it into view first"]
            }
            if !presenter.reach(of: node).contains(point) {
                return ["error": "tap #\(node.id): its middle is outside the viewport; scroll it into view first"]
            }
        }
        // (A canvas's down or one-shot press lets a hold go; its hover, wheel or a contact's later phase does not.)
        if let reply = canvasTap(req) { return req["wheel"] == nil && req["hover"] == nil && (req["phase"] as? String ?? "down") == "down" ? delivered(reply, view(req)) : reply }
        if req["phase"] == nil, req["wheel"] == nil, req["x"] == nil, req["y"] == nil, req["at"] == nil,
           let id = req["id"] as? UInt32, let run = presenter.inlineText(id), let node = presenter.textHost(id) {
            guard node.window != nil, !node.inert, !(req["contextmenu"] as? Bool == true ? node.formDisabled : node.disabled), !run.hidden else { return ["error": "inline node #\(id) is unavailable"] }
            if req["hover"] as? Bool == true {
                presenter.hoverInline(run.handlers.contains("hover") ? id : nil)
                return ["tapped": Int(id), "hover": true]
            }
            if req["contextmenu"] == nil, req["dblclick"] == nil, node.activateInline(id) { return delivered(["tapped": Int(id), "delivery": "host-activation", "native": "inline-text"], node) }
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
        // Light-dismiss before resolving the tap's press.
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil {
            presenter.menus.agentTap(node, landing: tapLanding(req, node: node))
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let activated = presenter.menus.activate(node) {
            return activated ? delivered(["tapped": id, "delivery": "host-activation", "native": "confirmation"], node)
                : ["error": "confirmation #\(id) is unavailable, transitioning, or its source is no longer active; `clock settle` first waits out a push or sheet still moving"]
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let reply = presenter.groupedLists?.activate(node, before: { nativePointer(node, request: req, at: $0) }) {
            return delivered(reply, node)
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           tapHitsControl(node, request: req),
           let activated = presenter.controls.activate(node, before: { nativePointer(node, request: req, at: $0) }) {
            if var unsupported = presenter.controls.unopened(node) {
                unsupported["reason"] = (unsupported["reason"] as? String ?? "") + "; an iOS simulator can use --touch platform for a real touch"
                return unsupported
            }
            return activated ? delivered(["tapped": id, "delivery": "host-activation", "native": "control"], node)
                : ["error": "control #\(id) is disabled, inert or not shown"]
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let why = drawnCovered(node) {
            return ["error": "tap #\(id): \(why)"]
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let activated = presenter.segments.activate(node, before: { nativePointer(node, request: req, at: $0) }) {
            return activated ? delivered(["tapped": id, "delivery": "host-activation", "native": "segmented-control"], node)
                : ["error": "native segment #\(id) is unavailable"]
        }
        if let id = req["id"] as? Int, let node = presenter.views[UInt32(id)],
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil,
           let reply = presenter.navigation.activateChrome(node, before: { nativePointer(node, request: req, at: $0) }) {
            return delivered(reply, node)
        }
        if let id = req["id"] as? Int, presenter.swipeActions.ownsAction(UInt32(id)),
           req["wheel"] == nil, req["hover"] == nil, req["contextmenu"] == nil, req["dblclick"] == nil {
            guard let button = presenter.swipeActions.actionView(UInt32(id)), let window = button.window,
                  let source = presenter.views[UInt32(id)], !source.disabled else {
                // Not revealed: the action as assistive technology performs
                // it, as the web's tap scrolls the row to it (splitter rough
                // 12). A real swipe is `tap <row> drag …` (`--touch platform`).
                if presenter.views[UInt32(id)]?.disabled == false, presenter.swipeActions.perform(UInt32(id), before: {
                    guard let source = presenter.views[UInt32(id)] else { return false }
                    return nativePointer(source, request: req)
                }) == true {
                    return delivered(["tapped": id, "pressed": id, "delivery": "host-activation", "native": "swipe-action", "revealed": false], presenter.views[UInt32(id)])
                }
                return ["error": "native swipe action #\(id) is not offered: disabled, hidden or inert, or its name is not unique"]
            }
            let b = box(button), point = button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.midY), to: window)
            guard let hit = window.hitTest(point, with: nil), hit === button || hit.isDescendant(of: button) else { return ["error": "native swipe action #\(id) is occluded"] }
            let event: UIControl.Event = button.allControlEvents.contains(.primaryActionTriggered) ? .primaryActionTriggered : .touchUpInside
            guard nativePointer(source, request: req, at: point) else { return ["error": "swipe action #\(id) changed during pointer handlers"] }
            button.sendActions(for: event)
            return delivered(["tapped": id, "at": [Agent.r2(b.midX), Agent.r2(b.midY)], "delivery": "host-activation", "native": "swipe-action"], source)
        }
        // A wheel on what a grouped list draws scrolls that list, even when
        // the node is a custom row's view its cell's reuse took off screen.
        if let wheel = req["wheel"] as? [Double], wheel.count == 2, wheel.allSatisfy(\.isFinite),
           let id = req["id"] as? Int, let list = presenter.groupedLists?.scroller(for: UInt32(id)), presenter.views[UInt32(id)]?.window == nil {
            Agent.scroll(from: list, dx: CGFloat(wheel[0]), dy: CGFloat(wheel[1]))
            return ["tapped": id, "wheel": wheel]
        }
        guard let v = view(req), let win = v.window else { return miss(req) }
        var b = v.tapBox(box(v))
        // A run is touched on a fragment it paints (the web's driver taps
        // the span's), not at its paragraph's middle.
        if ["at", "x", "y"].allSatisfy({ req[$0] == nil }), (req["id"] as? Int).map({ presenter.inlineText(UInt32($0)) != nil }) == true,
           let q = tapPoint(req, node: v) { b = CGRect(origin: q, size: .zero) }
        // A plain press: `at` is a point in the target, the press whatever a
        // finger there reaches; without one, the target's own press
        // (`AgentAddressedTap.swift`).
        let plain = ["wheel", "hover", "contextmenu", "dblclick", "pinch", "x", "y"].allSatisfy { req[$0] == nil } && v.kind != "iframe"
        // A long press or a double tap at a point too: where its word is (LLP 1115.000 D11.6).
        if req["x"] != nil || req["y"] != nil || req["at"] != nil && (plain || req["contextmenu"] as? Bool == true || req["dblclick"] as? Bool == true) {
            if req["at"] != nil {
                guard let at = req["at"] as? [Double], at.count == 2, at.allSatisfy(\.isFinite) else { return ["error": "tap at needs two finite numbers"] }
            }
            guard let point = tapPoint(req, node: v), point.x.isFinite, point.y.isFinite else { return ["error": "tap needs finite coordinates"] }
            let whole = box(v)
            guard whole.contains(point) else { return ["error": "tap #\(v.id) at: (\(Agent.r2(point.x - whole.minX)), \(Agent.r2(point.y - whole.minY))) is outside its \(Agent.r2(whole.width))×\(Agent.r2(whole.height)) box"] }
            b = CGRect(x: point.x, y: point.y, width: 0, height: 0)
        }
        // The middle of the box as seen — through a surface's placement when
        // there is one (LLP 1014 D5) — as a point in the window.
        let vp = presenter.viewport
        let window = { (q: CGPoint) in vp.convert(CGPoint(x: q.x + vp.pageOrigin.x, y: q.y + vp.pageOrigin.y), to: nil) }
        var p = window(CGPoint(x: b.midX, y: b.midY))
        var at = [Agent.r2(b.midX), Agent.r2(b.midY)]
        if req["wheel"] == nil, req["hover"] == nil { awaitAppOverlay(at: p, in: win) }
        let seen = win.hitTest(p, with: nil)
        // A finger lands only where the target is seen (LLP 1035.003: action
        // dispatch is never substituted for a contact), so a press, a menu or
        // a double click is refused, having changed nothing, when the software
        // keyboard or another view is over it. Off-viewport targets must be
        // scrolled into view first, as on the web carrier.
        if req["wheel"] == nil, let why = offscreen(v, box: b, hit: seen) {
            return ["error": "tap #\(v.id): \(why)"]
        }
        var hit = seen ?? v
        if req["wheel"] == nil, req["hover"] == nil, let why = obscured(v, at: p, hit: hit) {
            return ["error": "tap #\(v.id) at (\(at[0]), \(at[1])): \(why)"]
        }
        // Named, it presses what it names: its own press, never a control
        // inside it that its middle happens to reach.
        var avoided: PressReach?
        if plain, req["at"] == nil, (req["id"] as? Int).map({ presenter.inlineText(UInt32($0)) == nil }) == true {
            switch addressedPoint(v, box: b, in: win) {
            case .refused(let refusal): return refusal.reply
            case .at(let q, let middle):
                if let middle {
                    avoided = middle
                    p = window(q); at = [Agent.r2(q.x), Agent.r2(q.y)]
                    hit = win.hitTest(p, with: nil) ?? v
                }
            }
        }
        if req["contextmenu"] as? Bool == true || req["dblclick"] as? Bool == true {
            let event = req["contextmenu"] as? Bool == true ? "contextmenu" : "dblclick"
            // Selectable text (LLP 1115.000 D11.6): a double tap, and a long
            // press no enclosing context menu takes, select the word
            // there. UIKit synthesizes neither, so the selection is the
            // recognized one; a long press's stops there, as a finger's does.
            var selection: [String: Any]?
            #if os(iOS)
            if event == "contextmenu", let target = hit as? NodeView,
               presenter.menus.eligible(target, contextMenu: true),
               let run = target.inlineContextTarget(at: target.convert(p, from: nil)) {
                let handled = run.handlers.contains("contextmenu")
                if handled { target.deliverContext(at: target.convert(p, from: nil), inline: run.id) }
                let opened = presenter.menus.agentContext(target, inline: run.id)
                if handled || opened {
                    MainActor.assumeIsolated { HeldState.input(at: hit) }
                    return ["tapped": Int(run.id), "event": event, "injected": true, "at": at]
                }
            }
            selection = presenter.agentSelect(hit, at: p, event: event)
            #endif
            // Delivered (a handler heard it, or it selected): a long press or a double tap lets a hold where it landed go (HeldState.swift).
            let landed = { MainActor.assumeIsolated { HeldState.input(at: hit) } }
            if let selection, event == "contextmenu" { landed(); return ["tapped": Int((hit as? NodeView)?.id ?? v.id), "event": event, "delivery": "recognized", "selection": selection, "at": at] }
            var next: UIView? = hit
            while let node = next {
                // A context menu's popover (LLP 1021 §5.1) answers it too: the
                // node's own action first, then the popover opens painted.
                if let target = node as? NodeView, !(event == "contextmenu" ? target.formDisabled : target.disabled), target.handlers.contains(event)
                    || (event == "contextmenu" && target.props["contextPopover"]?.isEmpty == false) {
                    if event == "contextmenu" {
                        let handled = target.handlers.contains(event)
                        if handled { presenter.contextmenu(target.id) }
                        // A node with only a popover that cannot open it is not the target.
                        // One behind a modal hears it, but its popover would open inert.
                        if target.inert || !presenter.menus.agentContext(target), !handled { next = presenter.listenerParent(of: node); continue }
                    } else { presenter.dblclick(target.id) }
                    landed()
                    return ["tapped": Int(target.id), "event": event, "injected": true, "at": at].merging(selection.map { ["selection": $0] } ?? [:]) { a, _ in a }
                }
                // What a finger reaches, out of a sheet or a lifted popover too (`MountRelay`)
                next = presenter.listenerParent(of: node)
            }
            if let selection { landed(); return ["tapped": Int((hit as? NodeView)?.id ?? v.id), "event": event, "delivery": "recognized", "selection": selection, "at": at] }
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
            return delivered(["tapped": Int(handle.id), "pinch": scale, "at": at, "delivery": "recognized"], hit)
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
        if v.kind == "iframe" { return delivered(session.webviews.tap(v, request: req, at: at), hit) }
        #if os(iOS)
        // A finger on a number input's stepper steps it (LLP 1116.007 D4).
        if plain, let text = hit as? NodeView, presenter.textSelection.text(for: text)?.selected.length ?? 0 > 0 {
            presenter.textSelection.clear()
            return delivered(["tapped": Int(text.id), "at": at, "selection": "cleared"], hit)
        }
        if plain, req["at"] != nil, let stepped = steppedTap(v, hit: hit, at: p, tapped: at) { return delivered(stepped, hit) }
        #endif
        // What the finger reaches, taken where it lands (`fingerReach`):
        // what the pointer events below do to the tree never sends the press
        // elsewhere, as a real contact keeps its target.
        var reach = fingerReach(hit, at: p)
        // A finger's `pointerdown` as it lands and `pointerup` as it lifts,
        // before what the lift does (DOM's order), as `PointerRecognizer`
        // hears a real touch before the node does; a held contact's lift
        // sends its own (`fingerPhase`).
        let fingers = Agent.fingerPath(from: hit)
        // On a module's own view, what Contract hears of a finger (a press,
        // a pointer listener) is delivered; with nothing to hear it, refused.
        let unreached = plain && reach.action == nil ? moduleTouch(hit) : nil
        if let why = unreached, fingers.isEmpty { return ["error": "unsupported: tap #\(v.id) at (\(at[0]), \(at[1])) \(why)"] }
        // An iPad's hardware keys held through the tap (gallery F20).
        let held = Agent.heldPrefix(req)
        MainActor.assumeIsolated { HeldState.input(at: hit) } // the finger lands where its hit was checked: a held node there lets go (HeldState.swift)
        fingerPointer(.down, fingers, at: p, held: held)
        reach.rearm(presenter)
        fingerPointer(.up, fingers, at: p, held: held)
        var reply: [String: Any] = ["tapped": Int(v.id), "at": at].merging(lift(reach, at: p, held: held)) { a, _ in a }
        // Its middle reaches a control inside it: the tap landed beside it.
        if let avoided { reply["avoided"] = ["middle": [Agent.r2(b.midX), Agent.r2(b.midY)], "pressing": avoided.pressing ?? NSNull(), "what": avoided.described] as [String: Any] }
        if let why = unreached { reply["unreached"] = "Contract's pointer listeners heard it, but it \(why)" }
        return reply
    }

    /// At a point, the view a finger there lands on, as `PopoverTouch` sees it; nil without one.
    func tapLanding(_ req: [String: Any], node: NodeView) -> UIView? {
        guard ["at", "x", "y"].contains(where: { req[$0] != nil }), let point = tapPoint(req, node: node),
              point.x.isFinite, point.y.isFinite else { return nil }
        return node.window?.hitTest(windowPoint(point), with: nil)
    }

    /// A control's padding follows the ordinary press path when a point is given.
    private func tapHitsControl(_ node: NodeView, request: [String: Any]) -> Bool {
        if ["at", "x", "y"].allSatisfy({ request[$0] == nil }) { return true }
        if request["at"] != nil {
            guard let at = request["at"] as? [Double], at.count == 2, at.allSatisfy(\.isFinite) else { return false }
        }
        guard let control = presenter.controls.controls[node.id], let point = tapPoint(request, node: node),
              box(node).contains(point), let hit = node.window?.hitTest(windowPoint(point), with: nil) else { return false }
        return hit === control || hit.isDescendant(of: control)
    }

    /// What a finger at `p` (in the window) reaches, as `touchesEnded`
    /// resolves it (`AgentAddressedTap.swift`), nil where it would not land
    /// on `v` (covered, under the keyboard): an SVG
    /// element, an inline run or link, a canvas, a native control, else the
    /// nearest node from the hit one up that takes a press — or a disabled
    /// one on the way, which stops it.
    func pressReach(_ v: NodeView, at p: CGPoint, in win: UIWindow) -> PressReach? {
        guard let hit = win.hitTest(p, with: nil), obscured(v, at: p, hit: hit) == nil else { return nil }
        // A grouped list's cell: every cell is in the list's node, so its row
        // (LLP 1084) is what a finger there selects, or its switch or detail
        // button is what it presses (`TouchLog.landing`'s projection).
        // A custom row's own views are carried into its cell and take the
        // ordinary path (GroupedListIOS `list(drawing:)`): a node of theirs
        // under the finger is inside the cell too, and is no projection.
        if let projected = GroupedListsLink.part?(hit), let row = projected["row"] as? Int, let list = Agent.enclosing(hit).first,
           GroupedListsLink.part?(list) == nil {
            let part = projected["part"] as? String ?? "cell"
            return .part(list, part == "cell" ? "row #\(row)" : "the \(part) of row #\(row)", UInt32(row))
        }
        var at: UIView? = hit, node: NodeView?
        while let cur = at {
            if let n = cur as? NodeView { node = n; break }
            // A native button's control stands for its node (LLP 1069.011 D4);
            // any other enabled control, or a text editor, takes the touch itself.
            if let button = cur as? NativeButtonIOS, let owner = button.owner { node = owner; break }
            // tvOS has no editable text view (`isEditable` is unavailable there).
            #if os(tvOS)
            let takes = (cur as? UIControl)?.isEnabled == true
            #else
            let takes = (cur as? UIControl)?.isEnabled == true || (cur as? UITextView)?.isEditable == true
            #endif
            if takes, let n = Agent.enclosing(cur).first {
                if n.takesPress { node = n; break }
                return .control(n)
            }
            at = cur.superview
        }
        guard let node, !node.inert else { return .nothing }
        let local = node.local(p)
        if node.kind == "svg", let element = presenter.svg.target(node.id, at: local) { return .part(node, "SVG element #\(element)", element) }
        if let run = node.inlineActivationTarget(at: local) { return .part(node, "inline run #\(run.id)", run.id) }
        if let href = node.inlineLink(at: local) { return .part(node, "the link \(href)", nil) }
        if session.canvases.wantsInput(node.id), !node.isSurfaceControl { return .canvas(node) }
        var up: UIView? = node
        while let cur = up {
            if let n = cur as? NodeView {
                if n.disabled || n.authoredInert { return .blocked(n) }
                let box = presenter.pressBox(of: n, from: node)
                if n.takesPress(bubbled: box !== n) {
                    return box.bounds.contains((box as? NodeView)?.local(p) ?? box.convert(p, from: nil)) ? .node(n) : .nothing
                }
            }
            up = presenter.listenerParent(of: cur)
        }
        return .nothing
    }

    /// A plain tap's point for `v` (`AgentAddressedTap.swift`): `b` is its
    /// box in the viewport, the answer a point there, or the refusal.
    func addressedPoint(_ v: NodeView, box b: CGRect, in win: UIWindow) -> AddressedAim {
        if let native = nativeControlPoint(v, box: b, in: win) { return native }
        let vp = presenter.viewport
        let seen = b.intersection(presenter.reach(of: v))
        let window = { (q: CGPoint) in vp.convert(CGPoint(x: q.x + vp.pageOrigin.x, y: q.y + vp.pageOrigin.y), to: nil) }
        return Agent.addressedAim(v, middle: CGPoint(x: b.midX, y: b.midY), area: seen) { q in
            pressReach(v, at: window(q), in: win)
        }
    }

    /// Why `v`'s middle is not on screen for a finger, or nil: off the
    /// viewport it scrolls in, or off the window.
    func offscreen(_ v: NodeView, box b: CGRect, hit: UIView?) -> String? {
        let vp = presenter.viewport
        if presenter.mounts(v), !presenter.reach(of: v).contains(CGPoint(x: b.midX, y: b.midY)) {
            return "its middle is outside the \(Agent.r2(vp.pageSize.width))×\(Agent.r2(vp.pageSize.height)) viewport; a finger would scroll it into view first"
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
        if let container = presenter.keyboardContainer,
           let top = presenter.keyboardGuideTop(in: container), p.y >= container.convert(CGPoint(x: 0, y: top), to: nil).y {
            return "its middle is under the software keyboard; dismiss it or scroll the target above it first (state shows keyboard.top)"
        }
        if hit === v || hit.isDescendant(of: v) || (hit is NodeView && v.isDescendant(of: hit)) { return nil }
        return Agent.covers(hit)
    }

    /// What is over a target, named: a node by its id; another view by its
    /// class and the node holding it. The app's own native view (a module's
    /// splash) may be animating away on the wall clock, which the agent's
    /// clock does not move.
    static func covers(_ hit: UIView) -> String {
        if let node = hit as? NodeView { return "node #\(node.id) covers its middle" }
        var up = hit.superview
        while let u = up, !(u is NodeView) { up = u.superview }
        let cls: AnyClass = Swift.type(of: hit), name = String(describing: cls)
        let held = (up as? NodeView).map { "\(name), in node #\($0.id)," } ?? name
        // build.mjs compiles an app's native modules as one Swift module of this name.
        let own = String(reflecting: cls).hasPrefix("ExactAppModules.") || appOverlay(hit)
        return "\(held) covers its middle" + (own ? "; it is the app's own view, on the wall's time: a tap waits up to 3 s while one over its point moves, `clock settle` up to 1.5 s for its UIView.animate, `clock +N real` lets a display link's motion run, and one drawn from the module's `frames` or `after` follows `clock +N`" : "")
    }

    /// Why the agent's press would not do what a finger on `hit` does, or
    /// nil: `hit` is a native module's own view, whose controls, gesture
    /// recognizers and `touchesBegan` only a real touch drives (UIKit has no
    /// public touch synthesis). A responder in it is focused by the press
    /// (`lift`), as a finger focuses it.
    func moduleTouch(_ hit: UIView) -> String? {
        var at: UIView? = hit
        while let cur = at, !(cur is NodeView) {
            if cur.canBecomeFirstResponder { return nil }
            at = cur.superview
        }
        guard at !== hit, let node = at as? NodeView, node.kind == "native" else { return nil }
        return "lands on the app's own native view (\(String(describing: Swift.type(of: hit))), in node #\(node.id)), whose controls and gesture recognizers only a real touch drives: drive it with `--touch platform` on a simulator"
    }

    /// The web's rule for a phase-less wheel tick (LLP 1070 G2, Chrome's,
    /// measured in its §2): from the hit view up, the first scroll container
    /// that can take any of the tick's components takes the ones it can, and
    /// the rest is dropped; none other moves. One that can take none passes
    /// it on (`overscroll-behavior: auto`), unless the tick's axis is
    /// `contain` or `none` there, which keeps it. Whole points, bounded.
    /// The one taking it is left as a person's scroll by that much leaves it
    /// (`rest`).
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
                // At its top, the scroller a navigation bar follows takes a tick up only if a title it opens moves it
                let pull = !takeY && scrollsY && dy < 0 && o.y <= minY && Agent.bar(following: sv) != nil
                if takeX || takeY || pull {
                    // A wheel interrupts a smooth correction as a drag does: a
                    // plain write sends no animation end (LLP 1070.000 §11).
                    if let list = sv.superview as? NodeView, let p = list.presenter, p.collections.animating.contains(list.id) {
                        p.collections.animationEnded(list.id, dragging: true)
                    }
                    let before = (sv.contentOffset, sv.adjustedContentInset)
                    Agent.rest(sv, at: CGPoint(x: o.x + (takeX ? dx : 0), y: o.y + (takeY || pull ? dy : 0)), x: takeX, y: takeY || pull)
                    if takeX || takeY || sv.contentOffset != before.0 || sv.adjustedContentInset != before.1 {
                        // As a trackpad's scroll shows them, fading after.
                        sv.flashScrollIndicators()
                        return
                    }
                }
                if let owner = (sv as? ScrollView)?.scrollingNode ?? (sv.superview as? NodeView),
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
            // A repeat is not a press, as on every host.
            if req["repeat"] as? Bool != true { presenter.pressShortcut(node) }
            reply["shortcut"] = Int(node.id)
            return taken()
        }
        #endif
        return presenter.keyDown(at: v.isFirstResponder ? v : nil, device.key, held: held) ? taken() : nil
    }

    /// Set an input's text as typing does: the field focused, all selected,
    /// the text inserted — the field sends one change with the new value.
    func type(_ req: [String: Any]) -> [String: Any] {
        guard let v = view(req), v.window != nil else { return miss(req) }
        guard !v.disabled else { return ["error": "view \(v.id) is disabled"] }
        // An inert view takes no input, as a person cannot reach it (HTML inert; a modal presentation's background)
        guard !v.inert else { return ["error": "view \(v.id) is inert"] }
        if let edit = req["clipboard"] as? String { return withAgentPasteboard(req) { clipboardType(v, edit, req["text"] as? String, board: $0) } }
        if session.canvases.wantsInput(v.id) {
            if let routed = canvasRouted(v, req) { return routed }
            // The world hears the key a chord names, not the chord's string.
            var bare = req
            if let chord = req["key"] as? String { bare["key"] = KeyCodes.split(chord).key }
            return canvasType(v, bare)
        }
        if v.isSurfaceControl, let key = req["key"] as? String, let code = KeyCodes.device(key)?.code, ["Space", "Enter", "NumpadEnter"].contains(code) {
            let phase = req["phase"] as? String
            guard phase == nil || phase == "down" || phase == "up" else { return ["malformed": true, "error":"key: not a phase: \(phase!)"] }
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
            let location = session.typedLocation(req["text"] as? String ?? "")
            return session.navigate(location) ? ["typed": Int(v.id), "value": location, "delivery": "recognized"] : ["error": "navigate refused"]
        }
        if v.kind == "iframe" { return session.webviews.type(v, request: req) }
        if let key = req["key"] as? String, let device = KeyCodes.device(key), let canvas = v.inputCanvas,
           v.forwardsCanvasKey(device.code) {
            guard v.becomeFirstResponder() else { return ["error": "view takes no focus"] }
            if let routed = canvasRouted(v, req) { return routed }
            let phase = req["phase"] as? String
            guard phase == nil || phase == "down" || phase == "up" else { return ["malformed": true, "error":"key: not a phase: \(phase!)"] }
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
            // named (`P` is "P"); an unknown name is never inserted as text.
            guard let device = KeyCodes.device(bare) else { return ["malformed": true, "error": "key: unsupported key \(bare)"] }
            let name = bare.count == 1 && bare != " " ? bare : device.key
            let types = name.count == 1 && !held.contains("Control+") && !held.contains("Meta+")
            // A release focuses nothing: a Tab's down has already moved the
            // focus, which its up must not take back.
            let phase = req["phase"] as? String
            if phase != "up" {
                if let f = v.textArea { if !f.isFirstResponder { _ = f.becomeFirstResponder() } }
                else if let f = v.selectionField { if !f.isFirstResponder { _ = f.becomeFirstResponder() } }
                else if v.canBecomeFirstResponder, !v.isFirstResponder { _ = v.becomeFirstResponder() }
            }
            // With nothing focused the first root hears it, as a keyboard's key reaches it (`ExactView`).
            let focus = v.field != nil || v.textArea != nil || v.isFirstResponder || v.handlers.contains("press") ? v : presenter.focusedNode == nil ? presenter.keyRoot : nil
            // The release's `keyup` handlers at the focus (#140): after the
            // down, or when a held key comes up. A modifier's own keydown
            // holds it and its keyup no longer does, as DOM's.
            let code = device.code, lone = KeyCodes.modifier(code)
            let downHeld = lone ? KeyCodes.held(held, name, true) : held, upHeld = lone ? KeyCodes.held(held, name, false) : held
            // A chord's modifiers are their own keys around it, as a keyboard's
            // (KeyCodes.modifierPresses; Charlie, 2026-10-07): down in order
            // before the shortcuts and the key, up in reverse after its keyup,
            // each keyup without its own bit.
            let presses = KeyCodes.modifierPresses(key)
            // Tab's release, its modifiers' too, reaches where its down left
            // the focus, as a keyboard's keyup reaches the focused element.
            let heardUp = { [weak presenter, weak focus] in
                let at = name == "Tab" ? presenter?.focusedNode : focus
                _ = presenter?.keyUp(at: at, name, held: upHeld, code: code)
                for (i, m) in presses.enumerated().reversed() {
                    _ = presenter?.keyUp(at: at, m.key, held: i > 0 ? presses[i - 1].held : "", code: KeyCodes.device(m.key)?.code ?? "")
                }
            }
            defer {
                if phase != "down" { heardUp() }
                if phase == "down", let token = req["releaseKey"] as? String { keyReleases[token] = { heardUp(); return ["phase": "up", "delivery": "recognized"] } }
            }
            if phase != "up" { presenter.heldKey(at: focus) } // before the shortcuts: a key on a held node lets its hold go (HeldState.swift)
            if phase != "up", req["repeat"] as? Bool != true { for m in presses { _ = presenter.keyDown(at: focus, m.key, held: m.held, code: KeyCodes.device(m.key)?.code ?? "") } }
            // The page's shortcuts first, as the web's capture listener and
            // macOS's `routeKey` hear them (gallery F18, ShortcutsIOS).
            #if os(iOS)
            if phase != "up", let node = presenter.shortcut(key: name, held: held, focus: focus ?? presenter.focusedNode) {
                presenter.supersedePress() // a key, as `keyDown` and `performShortcut` take it
                if req["repeat"] as? Bool != true { presenter.pressShortcut(node) }
                return ["typed": Int(v.id), "key": key, "shortcut": Int(node.id), "delivery": "recognized"]
            }
            #endif
            let unprevented = phase != "up" && !presenter.keyDown(at: focus, name, held: downHeld, code: code, repeats: req["repeat"] as? Bool == true)
            // Tab's default moves the focus through the sequential order, as a
            // hardware keyboard's Tab does (`NodeView.tabCommands`), from a field
            // or textarea too, ending its editing (blur, and change if edited);
            // Shift goes back. From no focus it takes the first stop. The web and
            // Linux do the same (LLP 1088 D7.3); bench t9-profile, 2026-10-08.
            if unprevented, name == "Tab", ["Control+", "Meta+", "Alt+"].allSatisfy({ !held.contains($0) }) {
                presenter.moveFocus(backward: held.contains("Shift+"))
            } else if unprevented, let focus {
                if let f = focus.textArea {
                    if name == "Enter" { f.insertText("\n") } else if name == "Backspace" { f.deleteBackward() }
                    else if Agent.caretKey(name, in: f) {} else if types { f.insertText(name) }
                    pendingTextReveal = f as? TextArea
                } else if name == "Enter", let prompt = focus.headerField, let field = prompt.delegate as? PromptField {
                    // A prompt's field is UIKit's, standing in for this one: its Return.
                    _ = field.returnHeard(prompt)
                } else if let f = focus.selectionField {
                    (f as? TextField)?.heard = name
                    if name == "Backspace" { f.deleteBackward() } else if name == "Enter" { _ = f.delegate?.textFieldShouldReturn?(f) }
                    else if Agent.caretKey(name, in: f) {} else if types { f.insertText(name) }
                    (f as? TextField)?.heard = nil
                } else if presenter.controls.radioKey(focus, name, held: held) { // x2apps survey #2
                } else {
                    #if os(iOS)
                    _ = focus.activateKey(name)
                    #else
                    if focus.handlers.contains("press") || focus.defaultLink != nil, name == "Enter" || (name == " " && focus.props["href"] == nil) { presenter.press(focus.id) }
                    #endif
                }
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
        // Header search and prompts edit UIKit's displayed field, not their authored copy
        guard let f = v.selectionField else { return ["error": "view \(v.id) is not an input"] }
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
        // The simulator driver's check, after it read the display, that the display was still this app's.
        if req["probe"] as? Bool == true { return ["front": displayFront()] }
        let loading = settleForPicture()
        // UIKit's own motion lands first, unless the picture is a film's frame (LLP 1116.003 D8).
        let moving = req["film"] as? Bool != true && !settleMotion()
        // A canvas painting its children through its surface (LLP 1014)
        // shows its last capture: what is pending is captured and rendered
        // at the agent's clock first, as `clock` leaves it.
        if let now = session.clock, session.canvases.waitUntilReady() { session.canvases.settle(now: now) }
        presenter.canvas2d.waitForReplays()
        SvgFilterLive.waitForDraws()
        guard let path = req["path"] as? String else { return ["malformed": true, "error": "screenshot needs a path"] }
        let vp = presenter.viewport
        let scale = vp.window?.screen.scale ?? vp.traitCollection.displayScale
        let format = UIGraphicsImageRendererFormat()
        format.scale = scale
        format.opaque = true
        // 8-bit sRGB: on a wide-color screen the renderer would write a
        // 16-bit PNG, which nothing downstream (scripts/png.mjs) reads.
        format.preferredRange = .standard
        // What UIKit presents over the app (an alert, an action sheet, a popover, a presented route's sheet) is a view of
        // the window beside the viewport, never in the viewport's picture: with one up, the capture is the window's, as
        // `window` draws it, and says so (LLP 1116.012 D3; grocery-list's first two captures missed its clear alert).
        // A film's frames keep one size.
        let presented = req["film"] as? Bool == true ? nil : presentedOverApp()
        let wholeWindow = req["window"] as? Bool == true || presented != nil
        let page = presenter.documentHost
        let captureView: UIView = wholeWindow ? (vp.window ?? page) : page
        // The page's picture is its area, whose top-left is a box's (0, 0), not the bands it is drawn past (LLP 1130).
        let margins = wholeWindow ? .zero : presenter.pageMargins
        let size = captureView.bounds.inset(by: margins).size
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
        // Header nodes the app's navigation bar leaves out, as the agent's
        // chrome paints them: struck through in red (LLP 1116 D2).
        let unshown = presenter.navigation.unshownByBar().keys.sorted().compactMap { presenter.views[$0] }.filter {
            $0.window != nil && !$0.isHidden && $0.bounds.width > 0 && $0.bounds.height > 0
        }
        let png = UIGraphicsImageRenderer(size: size, format: format).pngData { _ in
            captureView.drawHierarchy(in: CGRect(origin: CGPoint(x: -margins.left, y: -margins.top), size: captureView.bounds.size), afterScreenUpdates: true)
            UIColor.systemRed.setStroke()
            for node in unshown {
                let r = node.convert(node.bounds, to: captureView).insetBy(dx: -1, dy: -1).offsetBy(dx: -captureView.bounds.minX - margins.left, dy: -captureView.bounds.minY - margins.top)
                let outline = UIBezierPath(rect: r)
                outline.lineWidth = 2
                outline.setLineDash([4, 3], count: 2, phase: 0)
                outline.stroke()
                let strike = UIBezierPath()
                strike.move(to: CGPoint(x: r.minX, y: r.maxY))
                strike.addLine(to: CGPoint(x: r.maxX, y: r.minY))
                strike.lineWidth = 1.5
                strike.stroke()
            }
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
        if wholeWindow { r["window"] = true }
        // Whether the simulator driver may take the display's pixels for this picture (devices.mjs `withDisplay`).
        if wholeWindow { r["front"] = displayFront() }
        if wholeWindow, let window = vp.window { r["opaque"] = displayOwnPoints(window) }
        // Glass or a blur in the page's picture is drawHierarchy's approximation: the picture's area in window
        // points, for the simulator driver to put the display's pixels there (devices.mjs `withMaterials`).
        if !wholeWindow, req["film"] as? Bool != true, let window = vp.window {
            let area = captureView.convert(captureView.bounds.inset(by: margins), to: window)
            if backdropRects(window).contains(where: { $0.insetBy(dx: 12, dy: 12).intersects(area) }) {
                r["materials"] = [Agent.r2(area.minX), Agent.r2(area.minY), Agent.r2(area.width), Agent.r2(area.height)]
            }
        }
        if moving { r["moving"] = true }
        #if os(iOS)
        // The status bar is SpringBoard's, never in the window's picture: its height, for the simulator driver to put it in (devices.mjs `withDisplay`).
        if wholeWindow, let bar = vp.window?.windowScene?.statusBarManager?.statusBarFrame.height, bar > 0, !(vp.window?.windowScene?.statusBarManager?.isStatusBarHidden ?? true) { r["statusBar"] = Agent.r2(bar) }
        // The window's own safe area, whatever the page's `viewport-fit`: the rows `compare` leaves out (the home indicator's too).
        if wholeWindow, let window = vp.window { r["safeArea"] = ["top": Agent.r2(window.safeAreaInsets.top), "bottom": Agent.r2(window.safeAreaInsets.bottom)] }
        #endif
        if let presented { r["presented"] = presented }
        if loading > 0 { r["imagesPending"] = loading }
        if !unshown.isEmpty {
            r["unshown"] = unshown.map { n -> [String: Any] in ["id": Int(n.id), "testId": n.props["testId"] ?? NSNull()] }
            r["unshownNote"] = "struck through in red: \(NavigationHost.unshownReason)"
        }
        // What the picture shows in the page that the app presents as UIKit's
        // own, said where the image cannot (euxy port, FRICTION §2).
        // Under native presentation what is still painted is what a person sees painted too (a content popover).
        let painted = presenter.menus.presented.filter { $0.window != nil && !$0.isHidden }
        if !painted.isEmpty, ExactEnv.agentPaints {
            r["painted"] = painted.map { n -> [String: Any] in ["id": Int(n.id), "testId": n.props["testId"] ?? NSNull()] }
            r["paintedNote"] = "the agent, not the app, painted the open popover in this picture, in the page (LLP 1021 D4); the app shows it as UIKit's (a menu at its button), which a screenshot shows under --touch platform"
        }
        let bars = presenter.navigation.paintedForBars()
        if !bars.isEmpty {
            r["chrome"] = bars.map { n -> [String: Any] in ["id": Int(n.id), "testId": n.props["testId"] ?? NSNull()] }
            r["chromeNote"] = "--chrome agent paints the authored header and tablist: the app shows UIKit's navigation bar and tab bar for them, which --chrome platform (exact.mjs's default) shows and `screenshot <png> window` draws"
        }
        // The software keyboard is not in the capture, which UIKit draws in a process of its own, and the app may
        // stand above it (LLP 1102 §3.17): say so, where the image alone reads as a shortened screen. The simulator
        // driver puts the keyboard's pixels in a window capture (`windowTop`, LLP 1116.008 D8; devices.mjs).
        let container = session.presenter.keyboardContainer
        if let top = container.flatMap({ session.presenter.keyboardGuideTop(in: $0) }) {
            var keyboard: [String: Any] = ["visible": true, "top": Agent.r2(top)]
            if let window = keyboardTopForCheck() { keyboard["windowTop"] = Agent.r2(window) }
            r["keyboard"] = keyboard
            r["note"] = "the software keyboard is up: it is not in the capture, and the app above it may be shortened (state shows keyboard.top)"
        }
        return r
    }

    /// Whether the display shows this app's window, or why not: an app that
    /// is not active is behind a system alert, another app or the home
    /// screen, and a window that is not key is behind another of its scene's.
    func displayFront() -> Any {
        if UIApplication.shared.applicationState != .active { return "the app is not active" }
        if presenter.viewport.window?.windowScene?.activationState != .foregroundActive { return "the app's scene is not in front" }
        if presenter.viewport.window?.isKeyWindow != true { return "the app's window is not key" }
        return true
    }

    /// Points of `window`, in its points, where the display must show what
    /// `drawHierarchy` drew, for the simulator driver's check that nothing of
    /// another process lies over the app (a notification banner leaves the
    /// app active and its window key): a 16-point grid, less the status bar,
    /// the bottom safe area, the keyboard, and each of `backdropRects`.
    func displayOwnPoints(_ window: UIWindow) -> [[Double]] {
        let skipped = backdropRects(window)
        let top = window.safeAreaInsets.top + 4
        let bottom = min(window.bounds.height - window.safeAreaInsets.bottom, keyboardTopForCheck() ?? .infinity) - 4
        var points: [[Double]] = []
        for y in stride(from: top, through: bottom, by: 16) {
            for x in stride(from: CGFloat(8), through: window.bounds.width - 8, by: 16) where !skipped.contains(where: { $0.contains(CGPoint(x: x, y: y)) }) {
                points.append([Agent.r2(x), Agent.r2(y)])
            }
        }
        return points
    }

    /// Each visible layer of `window` that draws what is behind it (glass, a
    /// blur, a portal) or filters what it draws, in window points, with 12
    /// points around it for a glass edge's light and shadow: what
    /// `drawHierarchy` draws only approximately.
    func backdropRects(_ window: UIWindow) -> [CGRect] {
        var skipped: [CGRect] = []
        // A blur whose radius is unset or 0 (a navigation bar's between
        // transitions) changes nothing: there the display matched the app's
        // drawing exactly (euxy's Patterns bar, light and dark).
        func changes(_ layer: CALayer, _ path: String) -> Bool {
            ((layer.value(forKey: path) as? [Any]) ?? []).contains { filter in
                let name = String(describing: filter)
                guard name.contains("Blur") else { return true }
                return ((layer.value(forKeyPath: "\(path).\(name).inputRadius") as? NSNumber)?.doubleValue ?? 0) != 0
            }
        }
        func visit(_ layer: CALayer) {
            guard !layer.isHidden, layer.opacity > 0 else { return }
            let kind = String(describing: Swift.type(of: layer))
            // A backdrop that only captures (a scroll edge effect's source) draws nothing.
            let backdrop = ["Backdrop", "Portal", "SDF", "Glass"].contains(where: kind.contains)
                && !(kind.contains("Backdrop") && (layer.value(forKey: "captureOnly") as? Bool) == true)
            if backdrop || changes(layer, "filters") || changes(layer, "backgroundFilters") {
                skipped.append(layer.convert(layer.bounds, to: window.layer).insetBy(dx: -12, dy: -12))
                return
            }
            layer.sublayers?.forEach(visit)
        }
        visit(window.layer)
        return skipped
    }

    /// What UIKit has presented over the app's window, by what a person calls it: "alert", "action sheet", "menu",
    /// "popover" or "sheet" (a presented route); nil with nothing presented. The innermost one: a sheet's own alert reads as the alert.
    func presentedOverApp() -> String? {
        var found: UIViewController?
        func visit(_ c: UIViewController) {
            var at = c
            while let next = at.presentedViewController { found = next; at = next }
            for child in c.children { visit(child) }
        }
        guard let window = presenter.viewport.window, let root = window.rootViewController else { return nil }
        visit(root)
        if let alert = found as? UIAlertController { return alert.preferredStyle == .alert ? "alert" : "action sheet" }
        #if os(iOS)
        if found is UIActivityViewController { return "share sheet" }
        #endif
        // A menu one of the popovers gave UIKit (`--touch platform`), however
        // UIKit presents it; tvOS has no menus or popovers.
        #if !os(tvOS)
        if menuShown(in: window) { return "menu" }
        #endif
        guard let top = found else { return nil }
        #if !os(tvOS)
        if top.modalPresentationStyle == .popover { return "popover" }
        #endif
        return "sheet"
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
