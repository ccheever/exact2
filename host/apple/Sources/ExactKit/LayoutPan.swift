// @ref LLP 1043.000 §3 D8 — recognized deltas commit ordinary application state.
// No motion token, release animation or second geometry owner. Cancellation keeps
// the last committed position. Coordinates are viewport CSS pixels on every host.
// @ref LLP 1057 §10.6 — a pan that began ends once with `panrelease(vx, vy)`, after
// its last delta: UIKit's velocity on iOS, the engine's tracker on AppKit (LLP
// 1057.001 §3). A cancelled contact releases at rest; one that never began, nothing.
#if os(macOS)
import AppKit
final class MouseLayoutPan {
    weak var presenter: Presenter?
    private(set) weak var candidate: NodeView?
    private var last = CGPoint.zero
    private(set) var active = false
    private var escape: Any?
    private var inactive: NSObjectProtocol?
    init(_ presenter: Presenter) {
        self.presenter = presenter
        escape = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            if event.keyCode == 53, self?.candidate != nil { self?.cancel(); return nil }
            return event
        }
        inactive = NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification,
            object: nil, queue: .main) { [weak self] note in
                if let window = note.object as? NSWindow, window === self?.presenter?.viewport.window { self?.cancel() }
            }
    }
    deinit {
        if let escape { NSEvent.removeMonitor(escape) }
        if let inactive { NotificationCenter.default.removeObserver(inactive) }
    }
    func down(_ node: NodeView, event: NSEvent) -> Bool {
        cancel()
        var at: NSView? = node
        while let current = at {
            if let view = current as? NodeView {
                guard SwipeInput.allows(view) else { return false }
                if view.handlers.contains("pan") {
                    candidate = view
                    last = presenter!.viewport.convert(event.locationInWindow, from: nil)
                    presenter!.onPanSample?(true, Double(last.x), Double(last.y), event.timestamp)
                    return true
                }
                if view.field != nil || view.textArea != nil || view.handlers.contains("press") { return false }
            }
            at = current.superview
        }
        return false
    }
    func drag(_ event: NSEvent) -> Bool {
        guard let view = candidate else { return false }
        guard let presenter, presenter.views[view.id] === view,
              view.handlers.contains("pan"), SwipeInput.allows(view) else { cancel(); return false }
        let point = presenter.viewport.convert(event.locationInWindow, from: nil)
        presenter.onPanSample?(false, Double(point.x), Double(point.y), event.timestamp)
        let dx = point.x - last.x, dy = point.y - last.y
        if !active && max(abs(dx), abs(dy)) <= Gesture.slop { return true }
        active = true; last = point
        presenter.selection.clear()
        if dx != 0 || dy != 0 { presenter.pan(view.id, Double(dx), Double(dy)) }
        return true
    }
    /// Taken only when the pan began: a contact that never left the slop is
    /// still the node's press (rule 4, as on the web and Linux).
    func up(_ event: NSEvent) -> Bool {
        if candidate != nil { _ = drag(event) }
        let took = active
        if took, let view = candidate, let presenter {
            let (vx, vy) = presenter.panVelocity?(event.timestamp) ?? (0, 0)
            presenter.panRelease(view.id, vx, vy)
        }
        abandon(); return took
    }
    /// Escape, the window resigning key, a node that stops panning or is
    /// retired: a contact that began and is still shown releases at rest.
    func cancel() {
        if active, let view = candidate, let presenter, presenter.views[view.id] === view { presenter.panRelease(view.id, 0, 0) }
        abandon()
    }
    /// Forget the contact without a release (a restart: its node is gone).
    func abandon() { candidate = nil; active = false }
    func retire(_ id: UInt32) { if candidate?.id == id { cancel() } }
}
#elseif os(iOS)
import UIKit

// UIPanGestureRecognizer begins after its recognition threshold. Preserve the
// original contact, as the web and AppKit pans do, instead of losing that first
// distance when UIKit starts reporting translation (visible on slider thumbs).
private final class ContactLayoutPan: UIPanGestureRecognizer {
    private weak var startWindow: UIWindow?
    private var startPoint: CGPoint?
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        if startPoint == nil, let touch = touches.first, let window = view?.window {
            startWindow = window
            startPoint = touch.location(in: window)
        }
        super.touchesBegan(touches, with: event)
    }
    func displacement(in viewport: UIView) -> CGPoint {
        guard let startWindow, startWindow === viewport.window, let startPoint else {
            return translation(in: viewport)
        }
        let start = viewport.convert(startPoint, from: startWindow)
        let current = location(in: viewport)
        return CGPoint(x: current.x - start.x, y: current.y - start.y)
    }
    override func reset() {
        super.reset()
        startWindow = nil; startPoint = nil
    }
}

extension NodeView {
    func updateLayoutPan() {
        if handlers.contains("pan"), layoutPanRecognizer == nil {
            let g = ContactLayoutPan(target: self, action: #selector(layoutPanning(_:)))
            g.maximumNumberOfTouches = 1; g.delegate = self
            addGestureRecognizer(g); layoutPanRecognizer = g
        } else if !handlers.contains("pan"), let g = layoutPanRecognizer {
            removeGestureRecognizer(g); layoutPanRecognizer = nil
        }
    }
    @objc func layoutPanning(_ gesture: UIPanGestureRecognizer) {
        guard let presenter, presenter.views[id] === self, SwipeInput.allows(self) else {
            // A contact that began and is cut short while shown releases at rest.
            if let presenter, presenter.views[id] === self, [.changed, .ended].contains(gesture.state) { presenter.panRelease(id, 0, 0) }
            gesture.isEnabled = false; gesture.isEnabled = true; return
        }
        let p = (gesture as? ContactLayoutPan)?.displacement(in: presenter.viewport) ?? gesture.translation(in: presenter.viewport)
        if gesture.state == .began { layoutPanOrigin = .zero }
        if [.began, .changed, .ended].contains(gesture.state) {
            let dx = p.x - layoutPanOrigin.x, dy = p.y - layoutPanOrigin.y
            layoutPanOrigin = p
            if dx != 0 || dy != 0 { presenter.pan(id, Double(dx), Double(dy)) }
        }
        // UIKit measures the release (LLP 1057.001 §3); its action sees
        // .cancelled or .failed only after .began.
        if gesture.state == .ended {
            let v = gesture.velocity(in: presenter.viewport)
            presenter.panRelease(id, Double(v.x), Double(v.y))
        } else if [.cancelled, .failed].contains(gesture.state) {
            presenter.panRelease(id, 0, 0)
        }
    }
}
#endif
