#if os(iOS)
// @ref llp/1107.004-home-projection.decision.md#decision
// App-owned recognizers/observations. Never changes Exact's geometry or delegates.
import UIKit

private final class T3HomeContactPan: UIPanGestureRecognizer {
    private weak var contactWindow: UIWindow?
    private var origin: CGPoint?
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        if origin == nil, let touch = touches.first, let window = view?.window {
            contactWindow = window; origin = touch.location(in: window)
        }
        super.touchesBegan(touches, with: event)
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        // Source failOffsetY applies only before recognition; an accepted horizontal
        // contact is cancelled by the actual list scroll gate, not this threshold.
        if state == .possible, let window = contactWindow, let origin, let touch = touches.first,
           abs(touch.location(in: window).y - origin.y) > 10 { state = .failed; return }
        super.touchesMoved(touches, with: event)
    }
    func displacement() -> CGPoint {
        guard let window = view?.window else { return .zero }
        guard contactWindow === window, let origin else { return translation(in: window) }
        let at = location(in: window)
        return CGPoint(x: at.x - origin.x, y: at.y - origin.y)
    }
    override func reset() { super.reset(); contactWindow = nil; origin = nil }
}

final class T3HomeSwipeRow: NSObject, UIGestureRecognizerDelegate {
    private weak var owner: T3MobileHomeSwipes?
    private(set) weak var element: ExactElement?
    private(set) weak var view: UIView?
    private(set) var identity: T3HomeSwipeRowIdentity
    private var open: Bool
    private var active = true
    private var recognizer: T3HomeContactPan?
    private weak var bridge: T3HomeSwipeBridge?
    private var gesture = 0
    private var sequence = 0
    private var last = CGPoint.zero
    private var size = CGSize.zero
    init(owner: T3MobileHomeSwipes, element: ExactElement, view: UIView, identity: T3HomeSwipeRowIdentity, open: Bool) {
        self.owner = owner; self.element = element; self.view = view; self.identity = identity; self.open = open
        super.init()
        let pan = T3HomeContactPan(target: self, action: #selector(changed(_:)))
        pan.delegate = self; pan.maximumNumberOfTouches = 1; pan.allowedScrollTypesMask = .continuous
        pan.cancelsTouchesInView = true; pan.delaysTouchesBegan = false; pan.delaysTouchesEnded = false
        recognizer = pan; view.addGestureRecognizer(pan)
    }
    func update(_ identity: T3HomeSwipeRowIdentity, open: Bool) {
        if self.identity != identity { retire(); self.identity = identity }
        self.open = open
    }
    private func live() -> Bool {
        guard active, identity.enabled, element?.isLive == true, let view, element?.view === view, view.window != nil,
              let owner, owner.bridge(for: identity.owner) != nil else { return false }
        var ancestor: UIView? = view
        while let at = ancestor {
            if at.isHidden || at.alpha <= 0 || !at.isUserInteractionEnabled { return false }
            ancestor = at.superview
        }
        return true
    }
    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard live(), let pan = recognizer, gestureRecognizer === pan, let owner, let view,
              let list = owner.list(for: identity.owner, view: view), list.allowsNewContact else { return false }
        let delta = pan.displacement()
        return abs(delta.y) <= 10 && abs(delta.x) > abs(delta.y) && (open ? abs(delta.x) > 8 : delta.x < -8)
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        guard let owner, let view else { return false }
        return other === owner.list(for: identity.owner, view: view)?.scroll?.panGestureRecognizer
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        guard live(), let view else { return false }
        var current = touch.view
        while let at = current, at !== view {
            if at is UITextField || at is UITextView || at is UIControl { return false }
            current = at.superview
        }
        return current === view
    }
    @objc private func changed(_ pan: T3HomeContactPan) {
        let point = pan.displacement()
        switch pan.state {
        case .began:
            guard live(), let owner, let view, let bridge = owner.bridge(for: identity.owner) else { cancel(); return }
            self.bridge = bridge; gesture = owner.nextSerial(); sequence = 0; size = view.bounds.size; last = point
            send("begin", point: point, velocity: pan.velocity(in: view.window))
        case .changed, .ended:
            guard gesture != 0, live(), let owner, let bridge, owner.current(bridge) else { cancel(); return }
            last = point
            send(pan.state == .ended ? "end" : "change", point: point, velocity: pan.velocity(in: view?.window))
            if pan.state == .ended { gesture = 0; self.bridge = nil }
        case .cancelled, .failed: cancel()
        default: break
        }
    }
    private func send(_ phase: String, point: CGPoint, velocity: CGPoint) {
        guard gesture > 0 else { return }
        bridge?.row(token: identity.row, gesture: gesture, sequence: sequence, phase: phase, point: point, velocity: velocity, size: size)
        sequence += 1
    }
    func cancel() {
        if gesture > 0 { send("cancel", point: last, velocity: .zero); gesture = 0; bridge = nil }
        if let recognizer, recognizer.state == .began || recognizer.state == .changed {
            recognizer.isEnabled = false; recognizer.isEnabled = true
        }
    }
    private func retire() { cancel(); owner?.retired(owner: identity.owner, row: identity.row) }
    func end() {
        guard active else { return }; retire(); active = false
        if let recognizer, recognizer.view === view { view?.removeGestureRecognizer(recognizer) }
        recognizer = nil; element = nil; view = nil; owner = nil
    }
}

final class T3HomeSwipeList: NSObject {
    private weak var owner: T3MobileHomeSwipes?
    private(set) weak var element: ExactElement?
    private(set) weak var scroll: UIScrollView?
    let ownerKey: String
    private(set) var active = true
    private var dragging = false
    private var gated = false
    private var dragY: CGFloat = 0
    private var offset: NSKeyValueObservation?
    private var deceleration: NSKeyValueObservation?
    private var settle: DispatchWorkItem?
    private var settling = false
    init(owner: T3MobileHomeSwipes, element: ExactElement, scroll: UIScrollView, ownerKey: String) {
        self.owner = owner; self.element = element; self.scroll = scroll; self.ownerKey = ownerKey
        super.init()
        scroll.panGestureRecognizer.addTarget(self, action: #selector(panChanged(_:)))
        offset = scroll.observe(\.contentOffset, options: [.new]) { [weak self] scroll, _ in self?.moved(scroll) }
        deceleration = scroll.observe(\.isDecelerating, options: [.new]) { [weak self] scroll, _ in
            guard let self, self.active else { return }
            if scroll.isDecelerating { self.settle?.cancel(); self.settle = nil; self.settling = false }
            else if !self.dragging { self.clear() }
        }
    }
    var allowsNewContact: Bool {
        guard active, element?.isLive == true, let scroll, scroll.window != nil else { return false }
        // Also release a completed momentum gate at admission if UIKit did not KVO its final flag.
        if gated, !dragging, !settling, !scroll.isDecelerating { clear() }
        return !gated && !scroll.isDecelerating
    }
    @objc private func panChanged(_ pan: UIPanGestureRecognizer) {
        guard active, let scroll else { return }
        switch pan.state {
        case .began:
            settle?.cancel(); settle = nil; settling = false; dragging = true; dragY = scroll.contentOffset.y
            owner?.scrollEvent(owner: ownerKey, phase: "scroll-begin")
        case .ended, .cancelled, .failed:
            dragging = false; settle?.cancel(); settling = true
            let item = DispatchWorkItem { [weak self] in
                guard let self, self.active else { return }
                self.settle = nil; self.settling = false
                if self.scroll?.isDecelerating != true { self.clear() }
            }
            settle = item; DispatchQueue.main.asyncAfter(deadline: .now() + 0.16, execute: item)
        default: break
        }
    }
    private func moved(_ scroll: UIScrollView) {
        guard active else { return }
        if dragging, !gated, abs(scroll.contentOffset.y - dragY) > 4 {
            gated = true; owner?.scrollEvent(owner: ownerKey, phase: "scroll-gated")
        }
        if gated, !dragging, !settling, !scroll.isDecelerating { clear() }
    }
    private func clear() {
        if gated { gated = false; owner?.scrollEvent(owner: ownerKey, phase: "scroll-clear") }
    }
    func reset() { settle?.cancel(); settle = nil; dragging = false; settling = false; gated = false }
    func end() {
        guard active else { return }; active = false; reset()
        offset?.invalidate(); deceleration?.invalidate(); offset = nil; deceleration = nil
        scroll?.panGestureRecognizer.removeTarget(self, action: #selector(panChanged(_:)))
        scroll = nil; element = nil; owner = nil
    }
}
#endif
