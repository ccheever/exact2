#if os(iOS)
// @ref examples/t3-code/mobile/docs/1130.004-home-projection.decision.md#decision
// Native recognition and mounted-copy receipts. Contract owns all row geometry.
import UIKit

final class T3MobileHomeSwipes {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class BridgeRef { weak var value: T3HomeSwipeBridge?; init(_ value: T3HomeSwipeBridge) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var bridges: [ObjectIdentifier: BridgeRef] = [:]
    private var rows: [ObjectIdentifier: T3HomeSwipeRow] = [:]
    private var lists: [ObjectIdentifier: T3HomeSwipeList] = [:]
    private var snooze: [ObjectIdentifier: T3HomeSwipeSnooze] = [:]
    private var serial = 0
    private var alive = true
    private var sidebarVisible = false
    private var dismissalRows: [ObjectIdentifier: T3HomeDismissalRow] = [:]
    private var dismissalElements: [ObjectIdentifier: ExactElement] = [:]
    private var dismissals: [String: T3HomeDismissalOperation] = [:]
    private var dismissalClock: ExactTicket?
    private var lastDismissalSeq: UInt64 = 0
    private var actionPending = false
    private var actionRoute = ""
    let context: ExactModuleContext
    init(context: ExactModuleContext) { self.context = context }

    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard alive else { throw ExactNativeRefusal("The Home swipe session ended.") }
        if props["swipe-controller-pending"] != nil {
            let lease = T3HomeDismissalLease(owner: self, events: events); try lease.setProps(props); return lease
        }
        if props["swipe-dismissal-identity"] != nil {
            let row = T3HomeDismissalRow(owner: self, events: events)
            dismissalRows[ObjectIdentifier(row)] = row
            do { try row.setProps(props); return row } catch { row.destroy(); throw error }
        }
        let bridge = T3HomeSwipeBridge(owner: self, events: events)
        bridges[ObjectIdentifier(bridge)] = BridgeRef(bridge)
        try bridge.setProps(props)
        return bridge
    }
    func configure(_ route: ExactRoute) {
        if let previous = routes[route.key]?.value, previous !== route { end(previous) }
        routes[route.key] = RouteRef(route)
        for bridge in liveBridges where bridge.routeKey == route.key { bridge.scheduleDeadline() }
    }
    func end(_ route: ExactRoute) {
        guard routes[route.key]?.value === route else { return }
        for bridge in liveBridges where bridge.routeKey == route.key { bridge.invalidate() }
        routes.removeValue(forKey: route.key)
    }
    func setSidebarVisible(_ visible: Bool) {
        guard sidebarVisible != visible else { return }
        sidebarVisible = visible
        for bridge in liveBridges where bridge.routeKey == "t3-workspace-sidebar" {
            if visible { bridge.scheduleDeadline() } else { bridge.invalidate() }
        }
    }
    private var liveBridges: [T3HomeSwipeBridge] { bridges.values.compactMap(\.value).filter(\.alive) }
    func current(_ bridge: T3HomeSwipeBridge) -> Bool {
        guard alive, bridge.alive, bridge.active, bridge.view.window != nil, !bridge.ownerKey.isEmpty,
              let route = routes[bridge.routeKey]?.value, route.isLive,
              route.controller.viewIfLoaded?.window != nil,
              bridge.routeKey != "t3-workspace-sidebar" || sidebarVisible else { return false }
        return liveBridges.filter { $0.ownerKey == bridge.ownerKey && $0.active }.count == 1
    }
    func route(for bridge: T3HomeSwipeBridge) -> ExactRoute? { current(bridge) ? routes[bridge.routeKey]?.value : nil }
    func bridge(for owner: String) -> T3HomeSwipeBridge? {
        let matching = liveBridges.filter { $0.ownerKey == owner && current($0) }
        return matching.count == 1 ? matching[0] : nil
    }
    func list(for owner: String, view: UIView) -> T3HomeSwipeList? {
        let matching = lists.values.filter { $0.active && $0.ownerKey == owner && $0.element?.isLive == true && $0.scroll.map { view.isDescendant(of: $0) } == true }
        return matching.count == 1 ? matching[0] : nil
    }
    func nextSerial() -> Int { serial += 1; return serial }
    func configure(_ element: ExactElement) {
        let key = ObjectIdentifier(element)
        element.reusable = true
        if element.hatch == .mobileHomeDismissal {
            dismissalElements[key] = element
            for row in dismissalRows.values { bindDismissal(row) }
            return
        }
        if element.hatch == .mobileHomeSwipeList {
            guard let owner = element.data[.mobileSwipeOwner], T3HomeSwipePacket.token(owner), let scroll = element.scrollView else {
                lists.removeValue(forKey: key)?.end(); return
            }
            if let old = lists[key], old.ownerKey == owner, old.scroll === scroll { return }
            lists.removeValue(forKey: key)?.end()
            lists[key] = T3HomeSwipeList(owner: self, element: element, scroll: scroll, ownerKey: owner)
            return
        }
        guard let identity = T3HomeSwipeRowIdentity(element), let view = element.view else {
            rows.removeValue(forKey: key)?.end(); snooze.removeValue(forKey: key)?.end(); return
        }
        if element.hatch == .mobileHomeSnooze {
            guard element.data[.mobileHomeSurface] == "sidebar-list" else { snooze.removeValue(forKey: key)?.end(); return }
            if let old = snooze[key], old.matches(element, view) { old.update(element, identity: identity) }
            else { snooze.removeValue(forKey: key)?.end(); snooze[key] = T3HomeSwipeSnooze(owner: self, element: element, view: view, identity: identity) }
            return
        }
        if let old = rows[key], old.element === element, old.view === view {
            old.update(identity, open: element.data[.mobileSwipeOpen] == "true")
        } else {
            rows.removeValue(forKey: key)?.end()
            rows[key] = T3HomeSwipeRow(owner: self, element: element, view: view, identity: identity, open: element.data[.mobileSwipeOpen] == "true")
        }
    }
    func end(_ element: ExactElement) {
        let key = ObjectIdentifier(element)
        dismissalElements.removeValue(forKey: key)
        for row in Array(dismissalRows.values) where row.element === element { row.unbind() }
        rows.removeValue(forKey: key)?.end(); lists.removeValue(forKey: key)?.end(); snooze.removeValue(forKey: key)?.end()
        element.reusable = true
    }
    func invalidate(_ bridge: T3HomeSwipeBridge) {
        for row in rows.values where row.identity.owner == bridge.ownerKey { row.cancel() }
        snooze.values.filter { $0.ownerKey == bridge.ownerKey }.forEach { $0.invalidate() }
        for list in lists.values where list.ownerKey == bridge.ownerKey { list.reset() }
    }
    func removed(_ bridge: T3HomeSwipeBridge) { invalidate(bridge); bridges.removeValue(forKey: ObjectIdentifier(bridge)) }
    func scrollEvent(owner: String, phase: String) {
        if phase == "scroll-gated" { for row in rows.values where row.identity.owner == owner { row.cancel() } }
        bridge(for: owner)?.surface(phase)
    }
    func retired(owner: String, row: String) { bridge(for: owner)?.retired(row) }
    func destroy() {
        guard alive else { return }
        cancelDismissals()
        Array(dismissalRows.values).forEach { $0.destroy() }
        dismissalElements.removeAll()
        for bridge in liveBridges { bridge.invalidate() }
        alive = false
        rows.values.forEach { $0.end() }; lists.values.forEach { $0.end() }
        snooze.values.forEach { $0.end() }
        rows.removeAll(); lists.removeAll(); snooze.removeAll(); bridges.removeAll(); routes.removeAll()
    }
}

fileprivate struct T3HomeDismissalScope: Equatable {
    let requestRoute: String, environmentId: String, threadId: String, origin: String
    let generation: Int
    init?(_ value: [String: Any]) {
        guard let requestRoute = value["requestRoute"] as? String, let environmentId = value["environmentId"] as? String,
              let threadId = value["threadId"] as? String, let origin = value["origin"] as? String,
              let generation = value["generation"] as? Int else { return nil }
        self.requestRoute = requestRoute; self.environmentId = environmentId; self.threadId = threadId
        self.origin = origin; self.generation = generation
    }
}

private final class T3HomeDismissalOperation {
    struct Capture { let row: T3HomeDismissalRow; let incarnation: Int }
    let token: String
    let scope: T3HomeDismissalScope
    let captures: [Capture]
    var waiting: Set<Int>
    var reply: ExactReply?
    init(token: String, scope: T3HomeDismissalScope, rows: [T3HomeDismissalRow]) {
        self.token = token; self.scope = scope
        captures = rows.map { Capture(row: $0, incarnation: $0.incarnation) }
        waiting = Set(captures.map(\.incarnation))
    }
}

extension T3MobileHomeSwipes {
    func observeActionLease(pending: Bool, route: String) {
        if !pending || actionRoute != route { cancelDismissals() }
        actionPending = pending; actionRoute = route
    }
    func bindDismissal(_ row: T3HomeDismissalRow) {
        let matching = dismissalElements.values.filter {
            $0.isLive && $0.data[.mobileSwipeOwner] == row.ownerKey && $0.data[.mobileSwipeRow] == row.rowKey
                && $0.data[.mobileDismissalIdentity] == row.identity && $0.view.map { row.view.isDescendant(of: $0) } == true
        }
        if matching.count == 1 { row.bind(matching[0]) } else { row.unbind() }
    }
    func dismissalRetired(_ incarnation: Int) {
        for operation in Array(dismissals.values) {
            operation.waiting.remove(incarnation); answerDismissal(operation)
        }
    }
    func dismissalRemoved(_ row: T3HomeDismissalRow) { dismissalRows.removeValue(forKey: ObjectIdentifier(row)) }
    func dismissalEchoSequence() -> UInt64 { lastDismissalSeq }
    func closeDismissalGesture(owner: String, row: String) {
        for item in rows.values where item.identity.owner == owner && item.identity.row == row { item.cancel() }
        // Presentation cleanup also reaches mounted, inactive copies.
        for bridge in liveBridges where bridge.ownerKey == owner { bridge.retired(row) }
    }
    func dismissalFinished(_ incarnation: Int, token: String) {
        guard let operation = dismissals[token] else { return }
        operation.waiting.remove(incarnation); answerDismissal(operation)
    }
    private func answerDismissal(_ operation: T3HomeDismissalOperation) {
        if operation.waiting.isEmpty, let reply = operation.reply {
            operation.reply = nil
            reply.send(["ok": true, "generation": operation.scope.generation, "value": ["token": operation.token, "completed": true]])
        }
        reconcileDismissalClock()
    }
    private func reconcileDismissalClock() {
        guard dismissals.values.contains(where: { $0.reply != nil && !$0.waiting.isEmpty }) else {
            dismissalClock?.stop(); dismissalClock = nil; return
        }
        guard dismissalClock == nil else { return }
        dismissalClock = context.frames { [weak self] frame in
            guard let self else { return }
            self.lastDismissalSeq = frame.seq
            for row in Array(self.dismissalRows.values) { row.frame(frame) }
        }
    }
    private func finishDismissal(_ operation: T3HomeDismissalOperation, committed: Bool) {
        dismissals.removeValue(forKey: operation.token)
        for capture in operation.captures where capture.row.incarnation == capture.incarnation {
            capture.row.finish(operation.token, committed: committed)
        }
        operation.reply?.fail("The Home dismissal ended."); operation.reply = nil
        reconcileDismissalClock()
    }
    private func cancelDismissals() {
        for operation in Array(dismissals.values) { finishDismissal(operation, committed: false) }
    }
    func dismissal(_ request: [String: Any], reply: ExactReply) {
        guard alive else { reply.fail("The Home session ended."); return }
        guard let scope = T3HomeDismissalScope(request) else { reply.fail("Invalid Home dismissal scope."); return }
        func answer(_ value: [String: Any] = [:]) { reply.send(["ok": true, "generation": scope.generation, "value": value]) }
        switch request["phase"] as? String {
        case "prepare":
            let rows = dismissalRows.values.filter { $0.matches(scope) && $0.element?.isLive == true && $0.element?.view != nil }
            if rows.isEmpty { answer(); return }
            guard actionPending, actionRoute == scope.requestRoute else { reply.fail("The Home action ended."); return }
            let token = String(nextSerial())
            dismissals[token] = T3HomeDismissalOperation(token: token, scope: scope, rows: Array(rows))
            answer(["token": token])
        case "begin":
            guard let token = request["token"] as? String, let operation = dismissals[token], operation.reply == nil, operation.scope == scope,
                  actionPending, actionRoute == scope.requestRoute else { reply.fail("The Home dismissal changed."); return }
            operation.reply = reply
            do {
                for capture in operation.captures where operation.waiting.contains(capture.incarnation) {
                    if capture.row.incarnation != capture.incarnation { operation.waiting.remove(capture.incarnation) }
                    else if try !capture.row.begin(token) { operation.waiting.remove(capture.incarnation) }
                }
            } catch { finishDismissal(operation, committed: false); return }
            answerDismissal(operation)
        case "finish":
            if let token = request["token"] as? String, let operation = dismissals[token], operation.scope == scope {
                finishDismissal(operation, committed: request["committed"] as? Bool == true)
            }
            answer()
        default: reply.fail("Unknown Home dismissal phase.")
        }
    }
}

private final class T3HomeDismissalLease: ExactNativeInstance {
    private weak var owner: T3MobileHomeSwipes?
    private let root = UIView()
    override var view: UIView { root }
    init(owner: T3MobileHomeSwipes, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events); root.isUserInteractionEnabled = false
    }
    override func setProps(_ props: [String: String]) throws {
        guard let pending = props["swipe-controller-pending"], ["true", "false"].contains(pending),
              let route = props["swipe-controller-route"] else { throw ExactNativeRefusal("Invalid Home action lease.") }
        owner?.observeActionLease(pending: pending == "true", route: route)
    }
    override func destroy() { owner?.observeActionLease(pending: false, route: ""); owner = nil }
}

final class T3HomeDismissalRow: ExactNativeInstance {
    private weak var owner: T3MobileHomeSwipes?
    private let root = UIView()
    private var scope: T3HomeDismissalScope?
    private var operation = "", displayToken = "", expectedPhase = ""
    private var echo: (phase: String, after: UInt64)?
    private var width = 0.0, height = 0.0
    private var dismissed = false, alive = true
    private(set) var ownerKey = "", rowKey = "", identity = ""
    private(set) var incarnation = 0
    private(set) weak var element: ExactElement?
    override var view: UIView { root }
    init(owner: T3MobileHomeSwipes, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events); root.isUserInteractionEnabled = false
    }
    override func setProps(_ props: [String: String]) throws {
        guard alive, let owner, let key = props["swipe-owner"], let row = props["swipe-row"],
              let identity = props["swipe-dismissal-identity"], T3HomeSwipePacket.token(identity),
              let json = props["swipe-context"]?.data(using: .utf8),
              let value = (try? JSONSerialization.jsonObject(with: json)) as? [String: Any] else { throw ExactNativeRefusal("Invalid Home dismissal row.") }
        if self.identity != identity || ownerKey != key || rowKey != row {
            retireAuthored(identity: identity, token: props["swipe-dismissal-token"] ?? "",
                registration: Int(props["swipe-dismissal-registration"] ?? "") ?? 0)
            owner.dismissalRetired(incarnation); incarnation = owner.nextSerial()
            operation = ""; displayToken = ""; expectedPhase = ""; echo = nil; dismissed = false; element = nil
        }
        self.identity = identity; ownerKey = key; rowKey = row; scope = T3HomeDismissalScope(value)
        owner.bindDismissal(self)
        if !operation.isEmpty, props["swipe-dismissal-token"] == displayToken,
           props["swipe-dismissal-registration"] == String(incarnation),
           Double(props["swipe-dismissal-progress"] ?? "") == 1,
           let phase = props["swipe-dismissal-phase"], phase == (expectedPhase == "out" ? "out-done" : "collapsed"), !expectedPhase.isEmpty {
            if echo == nil { echo = (phase, owner.dismissalEchoSequence()) }
        }
    }
    fileprivate func matches(_ scope: T3HomeDismissalScope) -> Bool { alive && self.scope == scope }
    func bind(_ next: ExactElement) {
        if element === next { return }
        if element != nil { unbind() }
        element = next
    }
    func unbind() {
        guard element != nil else { return }
        retireAuthored(identity: identity, token: displayToken, registration: incarnation)
        element = nil; owner?.dismissalRetired(incarnation); incarnation = owner?.nextSerial() ?? 0
        operation = ""; displayToken = ""; expectedPhase = ""; echo = nil; dismissed = false
    }
    private func retireAuthored(identity: String, token: String, registration: Int) {
        guard T3HomeSwipePacket.token(token), registration > 0 else { return }
        events.change(["2", identity, String(registration), token, "retired"].joined(separator: "|"))
    }
    func begin(_ token: String) throws -> Bool {
        guard alive, let view = element?.view, element?.isLive == true else { return false }
        operation = token
        if dismissed { return false }
        width = Double(view.bounds.width); height = Double(view.bounds.height)
        guard width.isFinite, height.isFinite, width > 0, height > 0 else { throw ExactNativeRefusal("The mounted Home row has no measurable bounds.") }
        owner?.closeDismissalGesture(owner: ownerKey, row: rowKey)
        displayToken = token; expectedPhase = "out"; echo = nil; send("out")
        return true
    }
    private func send(_ phase: String, at: Double? = nil) {
        guard let owner, !displayToken.isEmpty else { return }
        events.change(["2", identity, String(incarnation), displayToken, phase, String(at ?? owner.context.now()), String(width), String(height)].joined(separator: "|"))
    }
    func frame(_ frame: ExactFrame) {
        guard alive, !operation.isEmpty, !expectedPhase.isEmpty else { return }
        if let echo {
            guard frame.seq > echo.after else { return }
            self.echo = nil
            if echo.phase == "out-done" { expectedPhase = "collapse"; send("collapse", at: frame.now) }
            else if echo.phase == "collapsed" {
                expectedPhase = ""; dismissed = true; owner?.dismissalFinished(incarnation, token: operation)
            }
            return
        }
        // The authored action advances progress; elapsed native time never completes a row.
        send("tick", at: frame.now)
    }
    func finish(_ token: String, committed: Bool) {
        guard alive, operation == token else { return }
        if !committed { send("restore"); dismissed = false; displayToken = "" }
        operation = ""; expectedPhase = ""; echo = nil
    }
    override func destroy() {
        guard alive else { return }; alive = false
        unbind(); owner?.dismissalRetired(incarnation); owner?.dismissalRemoved(self); owner = nil
    }
}

private final class T3HomeSwipeEventView: UIView {
    var windowChanged: (() -> Void)?
    override func didMoveToWindow() { super.didMoveToWindow(); windowChanged?() }
}

final class T3HomeSwipeBridge: ExactNativeInstance {
    private weak var owner: T3MobileHomeSwipes?
    private let root = T3HomeSwipeEventView()
    private var timer: DispatchWorkItem?
    private var deadline: Double = 0
    private var epoch: Double = 0
    private var emittedDeadline: Double?
    private var timerSerial = 0
    private(set) var ownerKey = ""
    private(set) var routeKey = ""
    private(set) var active = false
    private(set) var alive = true
    override var view: UIView { root }
    init(owner: T3MobileHomeSwipes, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        root.isUserInteractionEnabled = false; root.accessibilityElementsHidden = true
        root.windowChanged = { [weak self] in
            guard let self else { return }
            if self.root.window == nil { self.invalidate() } else { self.scheduleDeadline() }
        }
    }
    override func setProps(_ props: [String: String]) throws {
        guard alive, let key = props["swipe-owner"], T3HomeSwipePacket.token(key),
              let route = props["swipe-route-key"], !route.isEmpty,
              let at = Double(props["swipe-refresh-at"] ?? "0"), at.isFinite, at >= 0, at.rounded() == at,
              let epoch = Double(props["swipe-epoch-at-zero"] ?? "0"), epoch.isFinite,
              let enabled = props["swipe-active"], ["true", "false"].contains(enabled) else {
            throw ExactNativeRefusal("Home swipe bridge requires a live owner, route, flag and finite deadline clock.")
        }
        let nextActive = enabled == "true", changed = ownerKey != key || routeKey != route || active != nextActive
        if changed { invalidate() }
        if ownerKey != key || routeKey != route { emittedDeadline = nil }
        let clockChanged = deadline != at || self.epoch != epoch
        ownerKey = key; routeKey = route; active = nextActive; deadline = at; self.epoch = epoch
        if changed || clockChanged { scheduleDeadline() }
    }
    func surface(_ phase: String) {
        guard alive, let owner, T3HomeSwipePacket.token(ownerKey) else { return }
        events.change("1|\(ownerKey)|\(owner.nextSerial())|\(phase)")
    }
    func retired(_ token: String) {
        guard alive, let owner, T3HomeSwipePacket.token(ownerKey), T3HomeSwipePacket.token(token) else { return }
        events.change("1|\(ownerKey)|\(owner.nextSerial())|retired|\(token)")
    }
    func row(token: String, gesture: Int, sequence: Int, phase: String, point: CGPoint, velocity: CGPoint, size: CGSize) {
        guard alive, let packet = T3HomeSwipePacket.row(owner: ownerKey, row: token, gesture: gesture, sequence: sequence,
            phase: phase, x: Double(point.x), y: Double(point.y), vx: Double(velocity.x), vy: Double(velocity.y), width: Double(size.width), height: Double(size.height)) else { return }
        events.change(packet)
    }
    func invalidate() {
        timer?.cancel(); timer = nil; timerSerial += 1
        owner?.invalidate(self)
        if active { surface("scroll-begin"); surface("scroll-clear") }
    }
    func scheduleDeadline() {
        timer?.cancel(); timer = nil; timerSerial += 1
        guard let owner, owner.current(self), deadline > 0, emittedDeadline != deadline, !owner.context.agent else { return }
        let now = epoch + owner.context.now(), delay = max(0, (deadline - now) / 1000)
        guard now.isFinite, delay.isFinite, delay <= 86_400 else { return }
        let captured = timerSerial, at = deadline, key = ownerKey
        let item = DispatchWorkItem { [weak self] in
            guard let self, let owner = self.owner, owner.current(self), self.timerSerial == captured,
                  self.ownerKey == key, self.deadline == at, self.emittedDeadline != at else { return }
            self.timer = nil; self.emittedDeadline = at
            self.events.change("1|\(key)|\(String(format: "%.0f", at))|tick")
        }
        timer = item; DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: item)
    }
    override func destroy() {
        guard alive else { return }; invalidate(); alive = false; root.windowChanged = nil; owner?.removed(self); owner = nil
    }
}
#endif
