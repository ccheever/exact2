// `pointerdown` and `pointerup` on UIKit (LLP 1005 §3; Charlie,
// 2026-10-03): DOM's names for a touch going down on a node and coming up
// or being cancelled, and `pointermove` while it is down (LLP 1056 §3 stage
// 3: UIKit delivers moves once a frame; a touch has no hover, and an iPad's
// pointer hovering is not delivered). An observer, not a gesture: it never
// recognizes, so it takes nothing from a press, a pan, a long press or the
// scroll view, and it fires before any of them has decided.
#if os(iOS) || os(tvOS)
import UIKit

extension Presenter {
    /// A window point from the viewport's top-left, the page scroll applied:
    /// DOM's `clientX`/`clientY`, `frame()`'s space (LLP 1094 D11). tvOS's
    /// samples (the agent's pan, a long press) read it too.
    func client(_ windowPoint: CGPoint) -> CGPoint {
        let p = viewport.convert(windowPoint, from: nil)
        return CGPoint(x: p.x - viewport.bounds.minX, y: p.y - viewport.bounds.minY)
    }
}
#endif

#if os(iOS)
import UIKit

final class PointerRecognizer: UIGestureRecognizer {
    weak var node: NodeView?
    private var touch: UITouch?
    /// The held touch's id, DOM's `pointerId`: the mouse is 1, so a touch
    /// counts on from 2, a new one for each contact.
    private var touchId = 1
    /// No touch held: a pooled row may keep it (it is synced with the
    /// handlers it is reused with).
    var idle: Bool { touch == nil }
    /// The held contact's buttons as DOM counts them: a touch or a pencil
    /// is the primary; an iPad's pointer says which it pressed (review
    /// b5-b 1: a secondary or middle click is 2 or 4, as on the web).
    private var buttons = 1

    /// DOM's `buttons` for UIKit's mask: primary 1, secondary 2, middle 4.
    static func domButtons(_ mask: UIEvent.ButtonMask) -> Int {
        var b = 0
        if mask.contains(.primary) { b |= 1 }
        if mask.contains(.secondary) { b |= 2 }
        if mask.contains(.button(3)) { b |= 4 }
        return b == 0 ? 1 : b
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesBegan(touches, with: event)
        guard touch == nil, let first = touches.first, let node, !node.disabled, !nearer(first.view) else { return }
        touch = first
        touchId += 1
        buttons = first.type == .indirectPointer ? Self.domButtons(event.buttonMask) : 1
        if node.handlers.contains("pointerdown") { node.presenter?.pointer(node.id, .down, sample(first)) }
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesMoved(touches, with: event)
        guard let held = touch, touches.contains(held), let node, node.handlers.contains("pointermove") else { return }
        if held.type == .indirectPointer { buttons = Self.domButtons(event.buttonMask) }
        node.presenter?.pointer(node.id, .move, sample(held))
    }
    /// The record of `t` as the node sees it, from its content box: a
    /// pencil's or a 3D Touch's force over its maximum where UIKit measures
    /// one, else DOM's 0.5 while down.
    private func sample(_ t: UITouch, lifted: Bool = false) -> PointerSample {
        guard let node else { return PointerSample(x: 0, y: 0, buttons: 0, pressure: 0, type: "touch", id: touchId, clientX: 0, clientY: 0) }
        let point = t.location(in: node), box = node.contentBox()
        let client = node.presenter?.client(t.location(in: nil)) ?? .zero
        let type = t.type == .pencil ? "pen" : t.type == .indirectPointer ? "mouse" : "touch"
        let pressure = lifted ? 0 : t.maximumPossibleForce > 0 ? Double(t.force / t.maximumPossibleForce) : 0.5
        // The keys a hardware keyboard holds, an iPad's ⇧ or ⌘ (gallery F20).
        return PointerSample(x: Double(point.x - box.minX), y: Double(point.y - box.minY), buttons: lifted ? 0 : buttons,
                             pressure: pressure, type: type, id: type == "mouse" ? 1 : touchId,
                             clientX: Double(client.x), clientY: Double(client.y), held: KeyCodes.held(modifierFlags))
    }
    /// Whether an enabled pointer node between the touched view and this
    /// one takes the touch: the innermost does, as on the web and macOS.
    func nearer(_ touched: UIView?) -> Bool {
        var view = touched
        while let v = view, v !== node {
            if let n = v as? NodeView, n.wantsPointer, !n.disabled { return true }
            view = v.superview
        }
        return false
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesEnded(touches, with: event)
        lift(touches)
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesCancelled(touches, with: event)
        lift(touches)
    }
    private func lift(_ touches: Set<UITouch>) {
        guard let held = touch, touches.contains(held) else { return }
        touch = nil
        if let node, node.handlers.contains("pointerup") { node.presenter?.pointer(node.id, .up, sample(held, lifted: true)) }
        state = .failed
    }
    override func reset() {
        super.reset()
        touch = nil
    }
    // Never in anyone's way.
    override func canPrevent(_ preventedGestureRecognizer: UIGestureRecognizer) -> Bool { false }
    override func canBePrevented(by preventingGestureRecognizer: UIGestureRecognizer) -> Bool { false }
}

extension NodeView {
    /// Install or remove the node's pointer observer with its handlers.
    var wantsPointer: Bool { handlers.contains("pointerdown") || handlers.contains("pointerup") || handlers.contains("pointermove") }
    func syncPointerRecognizer() {
        let wants = wantsPointer
        let current = gestureRecognizers?.first { $0 is PointerRecognizer }
        if wants, current == nil {
            let g = PointerRecognizer(target: nil, action: nil)
            g.node = self
            g.cancelsTouchesInView = false; g.delaysTouchesBegan = false; g.delaysTouchesEnded = false
            addGestureRecognizer(g)
        } else if !wants, let current {
            removeGestureRecognizer(current)
        }
    }
}
#elseif os(tvOS)
import UIKit

// tvOS touches no node: the Siri Remote moves focus (`RemoteTVOS.swift`), so
// nothing goes down on a node and no observer is installed.
final class PointerRecognizer: UIGestureRecognizer {
    var idle: Bool { true }
}

extension NodeView {
    func syncPointerRecognizer() {}
}
#endif

#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    // Press: a touch down and up inside the bounds. A node without a
    // handler passes the touch up the responder chain (UIView's default),
    // so a touch on a button's text reaches the button, as a DOM click
    // bubbles. A pan cancels it (the scroll view's `canCancelContentTouches`):
    // scroll always wins.
    package override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        if ((isSurfaceControl || ownsSurfaceControl) ? inputCanvas?.canvasInput : canvasInput)?.touches(touches, phase: "down", source: self, event: event) == true { return }
        guard !disabled else { pressed = false; return }
        if let touch = touches.first, let target = presenter?.svg.target(id, at: local(touch.location(in: nil))) {
            svgPressed = target; return
        }
        if let touch = touches.first, let run = inlineActivationTarget(at: local(touch.location(in: nil))) {
            inlinePressed = run.id; return
        }
        // A Markdown run's link has no view of its own (MarkupRuns): its target is the press.
        if let touch = touches.first, let href = inlineLink(at: local(touch.location(in: nil))) { linkPressed = href; return }
        if handlers.contains("press") || defaultLink != nil || presenter?.menus.closesPresentedContent(self) == true { pressed = true } else { super.touchesBegan(touches, with: event) }
    }
    package override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        inlinePressed = nil; linkPressed = nil
        if ((isSurfaceControl || ownsSurfaceControl) ? inputCanvas?.canvasInput : canvasInput)?.touches(touches, phase: "move", source: self, event: event) == true { return }
        if pressed { pressFollows(inside: touches.first.map(pressInside) ?? false) } else { super.touchesMoved(touches, with: event) }
    }
    package override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        if ((isSurfaceControl || ownsSurfaceControl) ? inputCanvas?.canvasInput : canvasInput)?.touches(touches, phase: "up", source: self, event: event) == true { finishPointerPress(); return }
        guard !disabled else { pressed = false; inlinePressed = nil; linkPressed = nil; svgPressed = nil; return }
        if let target = svgPressed {
            svgPressed = nil
            if let touch = touches.first, presenter?.svg.target(id, at: local(touch.location(in: nil))) == target { presenter?.press(target) }
            return
        }
        if let run = inlinePressed {
            inlinePressed = nil
            if let touch = touches.first, inlineActivationTarget(at: local(touch.location(in: nil)))?.id == run { _ = activateInline(run) }
            return
        }
        if let href = linkPressed {
            linkPressed = nil
            if let touch = touches.first, inlineLink(at: local(touch.location(in: nil))) == href { presenter?.session?.follow(href) }
            return
        }
        // Resolve before focus changes, as `activate` does: the field losing
        // it lowers the keyboard, and under `resizes-content` that moves this
        // control (a composer's toolbar) out from under the finger at once.
        let inside = pressed && (touches.first.map(pressInside) ?? false)
        // A press under `retainFocus` leaves the editor its focus, as macOS's
        // mouseDown does: every pressable can take the focus now.
        if canBecomeFirstResponder, !isFirstResponder, presenter?.contextRetainsFocus(self) != true { takeTouchFocus() }
        guard pressed else { return super.touchesEnded(touches, with: event) }
        pressed = false
        // A pressed node that did not take the focus: the field being edited
        // loses it, as a click on a button blurs a page's input.
        if !isFirstResponder && presenter?.contextRetainsFocus(self) != true { presenter?.viewport.endEditing(true) }
        if inside, presenter?.views[id] === self { presenter?.press(id, held: KeyCodes.held(event?.modifierFlags ?? [])); finishPointerPress() }
    }
    package override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        inlinePressed = nil; linkPressed = nil; svgPressed = nil
        if ((isSurfaceControl || ownsSurfaceControl) ? inputCanvas?.canvasInput : canvasInput)?.touches(touches, phase: "cancel", source: self, event: event) == true { return }
        if pressed { pressed = false } else { super.touchesCancelled(touches, with: event) }
    }

    /// A press delivered by the rule a touch gets: to this node when it has
    /// a handler, else to the nearest ancestor with one, if the point (in
    /// the window) is inside that node's box. The agent's `tap` and
    /// VoiceOver's activation come here — UIKit offers no public touch
    /// synthesis.
    @discardableResult
    func activate(at windowPoint: CGPoint) -> NodeView? {
        // An SVG element under the point takes it (LLP 1055.000 D17).
        if kind == "svg", let element = presenter?.svg.target(id, at: local(windowPoint)) { presenter?.press(element); return self }
        guard let target = activationTarget(at: windowPoint) else { return nil }
        if target.isSurfaceControl { return target.control("down") && target.control("up") ? target : nil }
        target.presenter?.press(target.id)
        return target
    }
    /// Whether a press is this node's own: a `press` handler, a link, a
    /// presented menu's closer, a surface control or a native button that
    /// invokes a command or a popover. A touch stops at the first such node
    /// from the one it hit up; `activationTarget` resolves it.
    var takesPress: Bool {
        handlers.contains("press") || defaultLink != nil || presenter?.menus.closesPresentedContent(self) == true || isSurfaceControl
            || isNativeButton && (props["commandfor"]?.isEmpty == false || props["popovertarget"]?.isEmpty == false)
    }
    /// Resolve before focus changes: a keyboard resize can move the control.
    func activationTarget(at windowPoint: CGPoint) -> NodeView? {
        guard !inert else { return nil }
        var v: UIView? = self
        while let cur = v {
            if let n = cur as? NodeView, n.disabled { return nil }
            if let n = cur as? NodeView, n.takesPress {
                guard n.bounds.contains(n.local(windowPoint)) else { return nil }
                return n
            }
            v = cur.superview
        }
        return nil
    }
    package override func accessibilityActivate() -> Bool {
        activate(at: convert(CGPoint(x: bounds.midX, y: bounds.midY), to: nil)) != nil
    }
}
#endif
