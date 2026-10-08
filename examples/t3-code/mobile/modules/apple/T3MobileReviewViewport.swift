#if os(iOS)
// @ref llp/1109.006-review-and-files.decision.md#navigator-selection-follow-up
// ReviewSheet/T3ReviewDiffView at365aa87982: manual crossings select a file;
// programmatic jumps keep the explicit destination selected.
import UIKit

private struct T3ReviewViewportIdentity: Equatable {
    let owner: String
    let section: String
    let route: String
    init?(_ element: ExactElement) {
        guard let owner = element.data[.mobileReviewOwner], !owner.isEmpty,
              let section = element.data[.mobileReviewSection], !section.isEmpty,
              let route = element.data[.mobileReviewRoute], !route.isEmpty else { return nil }
        self.owner = owner; self.section = section; self.route = route
    }
    init?(props: [String: String]) {
        guard let owner = props["review-owner"], !owner.isEmpty,
              let section = props["review-section"], !section.isEmpty,
              let route = props["review-route"], !route.isEmpty else { return nil }
        self.owner = owner; self.section = section; self.route = route
    }
}

struct T3ReviewViewportFrame {
    let path: String
    let minY: Double
    let maxY: Double
}

// Value-only admission state, also exercised by the Foundation source-body witness.
struct T3ReviewViewportTracking {
    private(set) var manual = false
    private(set) var pending = false
    private var previous: String?
    static func visiblePath(top: Double, rows: [T3ReviewViewportFrame]) -> String? {
        guard top.isFinite else { return nil }
        if top <= 0.5 { return "" }
        return rows.filter { $0.minY.isFinite && $0.maxY.isFinite && $0.minY <= top + 0.5 && $0.maxY > top - 0.5 }
            .max(by: { $0.minY < $1.minY })?.path
    }
    mutating func reset() { manual = false; pending = false; previous = nil }
    mutating func panBegan() { manual = true; pending = true }
    mutating func panEnded() { if manual { pending = true } }
    mutating func offsetChanged(userMoving: Bool) {
        guard manual else { return }
        if userMoving { pending = true } else { manual = false }
    }
    mutating func sample(_ path: String) -> Bool {
        guard pending else { return false }
        pending = false
        guard previous != path else { return false }
        previous = path
        return true
    }
}

private final class T3ReviewRowBinding {
    weak var element: ExactElement?
    weak var view: UIView?
    let identity: T3ReviewViewportIdentity
    let path: String
    init(element: ExactElement, identity: T3ReviewViewportIdentity, path: String) {
        self.element = element; view = element.view; self.identity = identity; self.path = path
    }
}

private final class T3ReviewListBinding: NSObject {
    weak var owner: T3MobileReviewViewport?
    weak var element: ExactElement?
    weak var scroll: UIScrollView?
    let identity: T3ReviewViewportIdentity
    var tracking = T3ReviewViewportTracking()
    var scheduled = false
    var active = true
    private var offsetObservation: NSKeyValueObservation?
    private var sizeObservation: NSKeyValueObservation?
    init(owner: T3MobileReviewViewport, element: ExactElement, scroll: UIScrollView, identity: T3ReviewViewportIdentity) {
        self.owner = owner; self.element = element; self.scroll = scroll; self.identity = identity
        super.init()
        offsetObservation = scroll.observe(\.contentOffset, options: [.new]) { [weak self] scroll, _ in
            guard let self, self.active else { return }
            let gesture = scroll.panGestureRecognizer.state
            self.tracking.offsetChanged(userMoving: gesture == .began || gesture == .changed || scroll.isTracking || scroll.isDragging || scroll.isDecelerating)
            self.owner?.schedule(self)
        }
        sizeObservation = scroll.observe(\.contentSize, options: [.new]) { [weak self] _, _ in
            guard let self, self.active else { return }; self.owner?.schedule(self)
        }
        scroll.panGestureRecognizer.addTarget(self, action: #selector(panChanged(_:)))
    }
    @objc private func panChanged(_ pan: UIPanGestureRecognizer) {
        guard active else { return }
        switch pan.state {
        case .began: tracking.panBegan()
        case .ended, .cancelled, .failed: tracking.panEnded()
        default: break
        }
        owner?.schedule(self)
    }
    func detach() {
        active = false; tracking.reset()
        offsetObservation?.invalidate(); offsetObservation = nil
        sizeObservation?.invalidate(); sizeObservation = nil
        scroll?.panGestureRecognizer.removeTarget(self, action: #selector(panChanged(_:)))
    }
    deinit { detach() }
}

final class T3MobileReviewViewport {
    private var active = true
    private var lists: [ObjectIdentifier: T3ReviewListBinding] = [:]
    private var rows: [ObjectIdentifier: T3ReviewRowBinding] = [:]
    private final class BridgeRef {
        weak var value: T3ReviewViewportBridge?
        init(_ value: T3ReviewViewportBridge) { self.value = value }
    }
    private var bridges: [ObjectIdentifier: BridgeRef] = [:]

    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard active else { throw ExactNativeRefusal("The review session has ended.") }
        let bridge = T3ReviewViewportBridge(owner: self, events: events)
        bridges[ObjectIdentifier(bridge)] = BridgeRef(bridge)
        try bridge.setProps(props)
        return bridge
    }
    func configure(_ element: ExactElement) {
        guard active else { return }
        // No view mutation or delegate replacement. Ending removes all observations.
        element.reusable = true
        let key = ObjectIdentifier(element)
        guard let identity = T3ReviewViewportIdentity(element) else { end(element); return }
        if element.hatch == .mobileReviewList {
            guard let scroll = element.scrollView else { lists.removeValue(forKey: key)?.detach(); return }
            if let existing = lists[key], existing.identity == identity, existing.scroll === scroll { return }
            lists.removeValue(forKey: key)?.detach()
            lists[key] = T3ReviewListBinding(owner: self, element: element, scroll: scroll, identity: identity)
        } else if element.hatch == .mobileReviewRow {
            guard let path = element.data[.mobileReviewPath], !path.isEmpty, element.view != nil else { rows.removeValue(forKey: key); return }
            rows[key] = T3ReviewRowBinding(element: element, identity: identity, path: path)
            for list in lists.values where list.identity == identity { schedule(list) }
        }
    }
    func end(_ element: ExactElement) {
        element.reusable = true
        let key = ObjectIdentifier(element)
        lists.removeValue(forKey: key)?.detach()
        rows.removeValue(forKey: key)
    }
    fileprivate func changed(_ bridge: T3ReviewViewportBridge, previous: T3ReviewViewportIdentity?) {
        for list in lists.values where list.identity == previous || list.identity == bridge.identity { list.tracking.reset() }
    }
    fileprivate func remove(_ bridge: T3ReviewViewportBridge) {
        bridges.removeValue(forKey: ObjectIdentifier(bridge))
        for list in lists.values where list.identity == bridge.identity { list.tracking.reset() }
    }
    fileprivate func schedule(_ list: T3ReviewListBinding) {
        guard active, list.active, list.tracking.pending, !list.scheduled else { return }
        list.scheduled = true
        // After this UIKit/Exact layout turn, not from a scroll delegate or render loop.
        DispatchQueue.main.async { [weak self, weak list] in
            guard let self, let list else { return }
            list.scheduled = false
            self.sample(list)
        }
    }
    private func sample(_ list: T3ReviewListBinding) {
        guard active, list.active, list.tracking.pending, list.element?.isLive == true,
              let scroll = list.scroll, scroll.window != nil, scroll.bounds.height > 0,
              !scroll.isHidden, scroll.alpha > 0 else { return }
        let matching = bridges.values.compactMap(\.value).filter { $0.alive && $0.focused && $0.identity == list.identity }
        // Ambiguous retained/replacement instances cannot select each other's navigator.
        guard matching.count == 1, let bridge = matching.first else { list.tracking.reset(); return }
        let top = scroll.contentOffset.y + scroll.adjustedContentInset.top
        let candidates = rows.values.compactMap { row -> T3ReviewViewportFrame? in
                guard row.identity == list.identity, row.element?.isLive == true, let view = row.view,
                      view.window === scroll.window, view.isDescendant(of: scroll), view.bounds.height > 0 else { return nil }
                var ancestor: UIView? = view
                while let at = ancestor, at !== scroll {
                    if at.isHidden || at.alpha <= 0 { return nil }
                    ancestor = at.superview
                }
                let rect = view.convert(view.bounds, to: scroll)
                return T3ReviewViewportFrame(path: row.path, minY: Double(rect.minY), maxY: Double(rect.maxY))
        }
        guard let path = T3ReviewViewportTracking.visiblePath(top: Double(top), rows: candidates) else {
            // A fast fling can precede virtualization/layout. A row mount/content-size
            // change retries this pending sample; never guess an overscan row's file.
            return
        }
        guard list.tracking.sample(path) else { return }
        bridge.publish(path: path)
    }
    func destroy() {
        active = false
        for list in lists.values { list.detach() }
        lists.removeAll(); rows.removeAll(); bridges.removeAll()
    }
}

private final class T3ReviewViewportBridge: ExactNativeInstance {
    private let root = UIView()
    private weak var owner: T3MobileReviewViewport?
    fileprivate var identity: T3ReviewViewportIdentity?
    fileprivate var focused = false
    fileprivate var alive = true
    private var navigationRevision = 0
    override var view: UIView { root }
    init(owner: T3MobileReviewViewport, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events)
        root.isUserInteractionEnabled = false; root.isAccessibilityElement = false
    }
    override func setProps(_ props: [String: String]) throws {
        let next = T3ReviewViewportIdentity(props: props)
        guard let revision = Int(props["review-navigation-revision"] ?? "0"), revision >= 0 else {
            throw ExactNativeRefusal("The review navigation revision is invalid.")
        }
        let nextFocused = props["review-focused"] == "true"
        let previous = identity
        let changed = next != identity || revision != navigationRevision || nextFocused != focused
        identity = next; navigationRevision = revision; focused = nextFocused
        if changed { owner?.changed(self, previous: previous) }
    }
    fileprivate func publish(path: String) {
        guard alive, focused, let identity else { return }
        let payload: [String: Any] = ["owner": identity.owner, "sectionId": identity.section, "routeId": identity.route,
                                      "navigationRevision": navigationRevision, "path": path]
        guard let data = try? JSONSerialization.data(withJSONObject: payload) else { return }
        events.change(String(decoding: data, as: UTF8.self))
    }
    override func destroy() { alive = false; owner?.remove(self); owner = nil }
}
#endif
