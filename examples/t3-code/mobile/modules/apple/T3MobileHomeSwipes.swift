#if os(iOS)
// @ref llp/1107.004-home-projection.decision.md#decision
// Recognition only. Contract owns row geometry and semantic Home actions.
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
    let context: ExactModuleContext
    init(context: ExactModuleContext) { self.context = context }

    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard alive else { throw ExactNativeRefusal("The Home swipe session ended.") }
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
        for bridge in liveBridges { bridge.invalidate() }
        alive = false
        rows.values.forEach { $0.end() }; lists.values.forEach { $0.end() }
        snooze.values.forEach { $0.end() }
        rows.removeAll(); lists.removeAll(); snooze.removeAll(); bridges.removeAll(); routes.removeAll()
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
