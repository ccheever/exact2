// `pointerdown` and `pointerup` on UIKit (LLP 1005 §3; Charlie,
// 2026-10-03): DOM's names for a touch going down on a node and coming up
// or being cancelled, bubbling to every enabled ancestor that hears either
// (Charlie, 2026-10-15), and `pointermove` while it is down (LLP 1056 §3 stage
// 3: UIKit delivers moves once a frame; a touch has no hover, and an iPad's
// pointer hovering is not delivered). An observer, not a gesture: it never
// recognizes, so it takes nothing from a press, a pan, a long press or the
// scroll view, and it fires before any of them has decided.
#if os(iOS) || os(tvOS)
import UIKit

extension Presenter {
    /// Touches whose `pointerdown` closed an opening they cross, before the
    /// hit view's down: UIKit's touches are never shared between sessions.
    var cancelledPressTouches: NSHashTable<UITouch> { Self.cancelledPressTouches }
    private static let cancelledPressTouches = NSHashTable<UITouch>.weakObjects()
    /// A window point from the viewport's top-left, the page scroll applied:
    /// DOM's `clientX`/`clientY`, `frame()`'s space (LLP 1094 D11). tvOS's
    /// samples (the agent's pan, a long press) read it too.
    func client(_ windowPoint: CGPoint) -> CGPoint {
        let p = viewport.convert(windowPoint, from: nil)
        return CGPoint(x: p.x - viewport.pageOrigin.x, y: p.y - viewport.pageOrigin.y)
    }
}
#endif

#if os(iOS)
import UIKit

final class PointerRecognizer: UIGestureRecognizer {
    weak var node: NodeView?
    /// A sheet mount's observer for the presented route's logical
    /// ancestors: its node is the listener each touch resolves to.
    weak var relay: MountRelay?
    private var touch: UITouch?
    /// The held touch's id, DOM's `pointerId`: the mouse is 1, so a touch
    /// counts on from 2, a new one for each contact.
    private var touchId = 1
    /// The nodes from this one up, through authored parents, that hear
    /// `pointerdown` or `pointerup`, innermost first: the held touch's down
    /// bubbled to each, and each hears its up.
    private var path: [Weak] = []
    private struct Weak { weak var node: NodeView? }
    /// The presenter the touch went down in: the held node may leave the
    /// tree before the up, which its surviving ancestors still hear.
    private weak var presenter: Presenter?
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
        if touch == nil, let relay { node = relay.listener(touches.first?.view) { $0.wantsPointer } }
        // A contact going down ends a live press's handoff before its `pointerdown` runs.
        node?.presenter?.supersedePress()
        guard touch == nil, let first = touches.first, let node, !node.disabled, relay != nil || !nearer(first.view) else { return }
        touch = first
        touchId += 1
        buttons = first.type == .indirectPointer ? Self.domButtons(event.buttonMask) : 1
        // This recognizer's node is the innermost that hears the pointer:
        // the others from here up leave the touch to it (`nearer`), so it
        // sends the bubbling down and up for them all.
        path = Self.bubbling(from: node).map(Weak.init)
        presenter = node.presenter
        let target = first.view.flatMap { view -> NodeView? in
            guard let start = NodeView.innermost(view) else { return nil }
            let point = first.location(in: nil), local = start.local(point)
            if presenter?.svg.target(start.id, at: local) != nil
                || start.inlineActivationTarget(at: local) != nil || start.inlineLink(at: local) != nil { return nil }
            return start.activationTarget(at: point)
        }
        let pressPath = first.view.flatMap { start -> PressPath? in
            guard let presenter, let target else { return nil }
            let path = PressPath(from: start, to: target, presenter: presenter)
            return path.bubbled ? path : nil
        }
        let lifetimes = pressPath == nil && target != nil ? first.view.map { presenter?.pressLifetimes(from: $0, to: target) ?? [] } ?? [] : []
        // A handler may disable a node further up before its turn, or stop
        // this down; the up's path and the hold stay.
        dispatch(path.compactMap(\.node).filter { $0.handlers.contains("pointerdown") }) { n in
            if !n.disabled { n.presenter?.pointer(n.id, .down, sample(first, at: n)) }
        }
        if let pressPath, let presenter {
            if !pressPath.isLive(in: presenter) { presenter.cancelledPressTouches.add(first) }
        } else if !lifetimes.allSatisfy({ $0() }) { presenter?.cancelledPressTouches.add(first) }
    }
    /// One bubbling dispatch to `nodes`, innermost first, each still the
    /// tree's when its turn comes, until a handler calls `stopPropagation()`.
    private func dispatch(_ nodes: [NodeView], _ send: (NodeView) -> Void) {
        guard let presenter else { return }
        presenter.propagating {
            for n in nodes where Self.live(n) {
                send(n)
                if presenter.propagationStopped { break }
            }
        }
    }
    /// Up the authored tree (`Presenter.pointerParent`).
    static func bubbling(from node: NodeView) -> [NodeView] {
        var nodes: [NodeView] = []
        var view: UIView? = node
        let presenter = node.presenter
        while let v = view {
            if let n = v as? NodeView, !n.disabled, n.handlers.contains("pointerdown") || n.handlers.contains("pointerup") { nodes.append(n) }
            view = presenter?.pointerParent(of: v) ?? v.superview
        }
        return nodes
    }
    /// Still the tree's node: a handler further in may have removed it.
    private static func live(_ node: NodeView) -> Bool { node.presenter?.views[node.id] === node }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        super.touchesMoved(touches, with: event)
        guard let held = touch, touches.contains(held), let node, node.handlers.contains("pointermove") else { return }
        if held.type == .indirectPointer { buttons = Self.domButtons(event.buttonMask) }
        node.presenter?.pointer(node.id, .move, sample(held))
    }
    /// The record of `t` as `node` (this one's by default) sees it, from its
    /// content box: a pencil's or a 3D Touch's force over its maximum where
    /// UIKit measures one, else DOM's 0.5 while down.
    private func sample(_ t: UITouch, lifted: Bool = false, at node: NodeView? = nil) -> PointerSample {
        guard let node = node ?? self.node else { return PointerSample(x: 0, y: 0, buttons: 0, pressure: 0, type: "touch", id: touchId, clientX: 0, clientY: 0) }
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
        presenter?.cancelledPressTouches.remove(held)
        let nodes = path.compactMap(\.node).filter { $0.handlers.contains("pointerup") }
        path = []
        dispatch(nodes) { n in n.presenter?.pointer(n.id, .up, sample(held, lifted: true, at: n)) }
        if relay != nil { node = nil }
        state = .failed
    }
    override func reset() {
        super.reset()
        touch = nil
        path = []
    }
    // Never in anyone's way.
    override func canPrevent(_ preventedGestureRecognizer: UIGestureRecognizer) -> Bool { false }
    override func canBePrevented(by preventingGestureRecognizer: UIGestureRecognizer) -> Bool { false }
}

extension Presenter {
    /// What a touch's pointer events, long press and double tap bubble to
    /// from `view`, as DOM's bubble out of the top layer: a lifted popover's
    /// or a modal dialog's authored parent, though its view is in the top
    /// layer or a sheet, a presented route's logical parent, else the
    /// superview. `eventParent` stays inside a dialog's sheet: the walks on
    /// it also read inertness and visibility, which a modal dialog escapes.
    func pointerParent(of view: UIView) -> UIView? {
        if let node = view as? NodeView, let parent = menus.sheets.authoredParent(of: node) { return parent }
        return menus.logicalParent(of: view) ?? eventParent(of: view)
    }
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

/// The responder hop taken on down, not a gesture recognizer. Weak view and
/// touch references leave UIKit's original hit and contact lifetime intact.
final class PressContact {
    weak var touch: UITouch?
    let incarnation: UInt64
    weak var responder: UIResponder?
    weak var responderContact: PressContact?
    var responderIncarnation: UInt64?
    var responderUsesCanvas = false
    var cancelled = false
    var cancelledByReparenting = false
    /// A handlerless custom invoker's look, independent of the forwarded press: nil when it owns none.
    var invokerLook: Bool?
    /// Bubbled in from a presentation's mount to a logical ancestor behind
    /// it, which is inert behind the presentation but still hears a click
    /// bubbling from it (HTML: inertness keeps a node from being the target,
    /// not its listeners from the bubbling), unless authored inert.
    var bubbled = false
    var path: PressPath?
    init(_ touch: UITouch, incarnation: UInt64) {
        self.touch = touch; self.incarnation = incarnation
    }
    var destination: UIResponder? {
        guard let responder else { return nil }
        if let node = responder as? NodeView {
            guard node.incarnation == responderIncarnation, node.presenter?.views[node.id] === node else { return nil }
            if !responderUsesCanvas {
                guard let responderContact, node.pressContact === responderContact else { return nil }
            }
        }
        return responder
    }
    var routeActive: Bool {
        guard !cancelled else { return false }
        guard responderIncarnation != nil else { return true }
        guard let destination else { return false }
        if responderUsesCanvas { return true }
        guard let node = destination as? NodeView, let downstream = responderContact,
              node.pressContact === downstream else { return false }
        return downstream.routeActive
    }
    var routesToCanvas: Bool { responderUsesCanvas || responderContact?.routesToCanvas == true }
    var routesBubbledPress: Bool { bubbled || responderContact?.routesBubbledPress == true }
}

extension NodeView {
    private func setInvokerLook(_ contact: PressContact, _ inside: Bool?) {
        guard contact.invokerLook != inside else { return }
        contact.invokerLook = inside
        if !holds("active") { showPress(pressed || overlayHoldsPress || inside == true) }
    }

    var hasPressContact: Bool { pressContact.map { !$0.cancelled } ?? false }
    /// UIView forwards to `next`. Only Exact's inserted overflow wrapper is
    /// skipped: UIScrollView otherwise swallows the authored ancestor's down.
    private var pressResponder: UIResponder? {
        // A lifted root leaves UIKit's presentation for its authored parent,
        // which may be a scroller's `ScrollView` or one of Exact's containers.
        var responder = next
        if let presenter, let parent = presenter.listenerParent(of: self), parent !== superview { responder = parent }
        // Exact's own containers (a clip box, a material or glass effect
        // and its content view) are no step: a container swap keeps the route.
        while let view = responder as? UIView, view is PlainView || view is UIVisualEffectView
                || (view.superview as? UIVisualEffectView)?.contentView === view {
            responder = view.next
        }
        if let scroll = responder as? ScrollView, let owner = scroll.superview as? NodeView, owner.scroll === scroll { return owner }
        return responder
    }
    /// Whether a touch is inside what it presses this node through: its own
    /// box (`pressInside`), or, bubbled out of a sheet, the content it hit
    /// (`Presenter.pressBox`).
    private func pressLands(_ touch: UITouch) -> Bool {
        guard let start = touch.view.map({ Self.innermost($0) ?? $0 }), let box = presenter?.pressBox(of: self, from: start), box !== self else {
            return pressInside(touch)
        }
        if let node = box as? NodeView { return node.pressInside(touch) }
        return box.bounds.contains(touch.location(in: box))
    }
    private func currentPressContact(_ touches: Set<UITouch>) -> PressContact? {
        guard let contact = pressContact, contact.incarnation == incarnation, let held = contact.touch,
              touches.contains(where: { $0 === held }) else { return nil }
        if let path = contact.path, let presenter {
            if !path.isLive(in: presenter) { cancelPressContact() }
        } else if disabled || (contact.bubbled ? authoredInert : inert) { cancelPressContact() }
        return contact.cancelled ? nil : contact
    }
    /// Retirement, reparenting and disable cancel the old chain while its
    /// identities are still available. A late terminal callback cannot clear
    /// a new contact or a new incarnation's feedback.
    func cancelPressContact(reparenting: Bool = false) {
        guard let contact = pressContact, !contact.cancelled, contact.incarnation == incarnation else { return }
        contact.cancelled = true
        contact.cancelledByReparenting = reparenting
        setInvokerLook(contact, nil)
        pressed = overlayHoldsPress; inlinePressed = nil; linkPressed = nil; svgPressed = nil
        if let touch = contact.touch { _ = canvasTouches([touch], phase: "cancel", event: nil) }
        if contact.responderUsesCanvas, let touch = contact.touch {
            contact.destination?.touchesCancelled([touch], with: nil)
        } else if let node = contact.destination as? NodeView {
            node.cancelPressContact()
        } else if let touch = contact.touch { contact.destination?.touchesCancelled([touch], with: nil) }
    }
    func cancelPressContactsInSubtree() {
        func cancel(_ view: UIView) {
            if let node = view as? NodeView { node.pressEpoch &+= 1; node.cancelPressContact() }
            for child in view.subviews { cancel(child) }
        }
        cancel(self)
    }
    /// Surface controls accept descendant touches, but never ones forwarded
    /// out of a presentation. Keep that decision after cancellation/reparenting.
    private func canvasTouches(_ touches: Set<UITouch>, phase: String, event: UIEvent?) -> Bool {
        if touches.isEmpty, pressContact?.bubbled == true { return false }
        let bubbled = touches.contains { touch in
            if phase != "down", let contact = pressContact, contact.touch === touch { return contact.bubbled }
            return touch.view.flatMap { start in presenter.map { $0.pressBox(of: self, from: start) !== self } } ?? false
        }
        guard !bubbled else { return false }
        return ((isSurfaceControl || ownsSurfaceControl) ? inputCanvas?.canvasInput : canvasInput)?.touches(touches, phase: phase, source: self, event: event) == true
    }
    // Press: a touch down and up inside the bounds. A node without a
    // handler passes the touch up the captured responder chain,
    // so a touch on a button's text reaches the button, as a DOM click
    // bubbles. A pan cancels it (the scroll view's `canCancelContentTouches`):
    // scroll always wins.
    package override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        if ExactEnv.agentMode { TouchLog.reach(touches) }
        HeldState.input(at: self) // (ExactWindow's too: a contact on a held node lets its hold go, HeldState.swift)
        presenter?.supersedePress()
        if touches.contains(where: { presenter?.cancelledPressTouches.contains($0) == true }) { cancelPressContact(); return }
        if canvasTouches(touches, phase: "down", event: event) { return }
        guard let touch = touches.first else { return }
        cancelPressContact()
        let contact = PressContact(touch, incarnation: incarnation)
        contact.bubbled = touch.view.flatMap { start in presenter.map { $0.pressBox(of: self, from: start) !== self } } ?? false
        pressContact = contact
        if contact.bubbled, let presenter, let start = touch.view {
            let path = PressPath(from: start, to: self, presenter: presenter)
            contact.path = path
            if !path.isLive(in: presenter) { cancelPressContact(); return }
        }
        guard !disabled, !(contact.bubbled ? authoredInert : inert) else { cancelPressContact(); return }
        if !contact.bubbled, let target = presenter?.svg.target(id, at: local(touch.location(in: nil))) {
            svgPressed = target; return
        }
        if !contact.bubbled, let run = inlineActivationTarget(at: local(touch.location(in: nil))) {
            inlinePressed = run.id; return
        }
        // A Markdown run's link has no view of its own (MarkupRuns): its target is the press.
        if !contact.bubbled, let href = inlineLink(at: local(touch.location(in: nil))) { linkPressed = href; return }
        if handlers.contains("press") || defaultLink != nil || presenter?.menus.closesPresentedContent(self) == true
            || contact.bubbled && isNativeButton && presenter?.invocation(of: self) != nil { pressed = true } else {
            if !isNativeButton, presenter?.showsPopoverPress(of: self) == true {
                setInvokerLook(contact, true)
            }
            contact.responder = pressResponder
            contact.responderIncarnation = (contact.responder as? NodeView)?.incarnation
            if let node = contact.responder as? NodeView {
                contact.responderUsesCanvas = node.isSurfaceControl && node.inputCanvas?.canvasInput != nil
                    && (presenter?.pressBox(of: node, from: touch.view ?? self) ?? node) === node
            }
            contact.responder?.touchesBegan(touches, with: event)
            if let node = contact.responder as? NodeView, node.incarnation == contact.responderIncarnation,
               let downstream = node.pressContact, downstream.touch === touch { contact.responderContact = downstream }
        }
    }
    package override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        if canvasTouches(touches, phase: "move", event: event) { return }
        guard let contact = currentPressContact(touches) else { return }
        inlinePressed = nil; linkPressed = nil
        if contact.invokerLook != nil { setInvokerLook(contact, touches.first.map(pressLands) ?? false) }
        if pressed { pressFollows(inside: touches.first.map(pressLands) ?? false) } else { contact.destination?.touchesMoved(touches, with: event) }
    }
    package override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        if canvasTouches(touches, phase: "up", event: event) { return }
        guard let contact = currentPressContact(touches) else { return }
        // Its press may have removed the overlay that held the look with it,
        // which left the look to this contact: the look is the overlay's alone now.
        defer {
            if pressContact === contact {
                setInvokerLook(contact, nil)
                pressContact = nil
                if pressed != overlayHoldsPress { pressed = overlayHoldsPress }
            }
        }
        guard contact.routeActive else { return }
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
        let ownsPress = pressed
        let inside = ownsPress && (touches.first.map(pressLands) ?? false)
        // The press's target settles the focus, its hit resolved, from the
        // node the touch landed in; a touch that presses nothing settles it
        // where it landed (`focusForPress`).
        let touched = Self.innermost(touches.first?.view ?? self) ?? self
        var handoff: PressHandoff?
        defer { handoff?.dispatched() }
        if ownsPress || (touched === self && !pressesAnAncestor && !contact.routesToCanvas) {
            handoff = touched.focusForPress(target: ownsPress ? self : nil)
            handoff?.point = touches.first?.location(in: nil)
        }
        guard pressContact === contact, incarnation == contact.incarnation else { return }
        if let path = contact.path, let presenter, !path.isLive(in: presenter) { cancelPressContact(); return }
        // Focus can move the pressed button into a new parent after `inside`
        // was resolved. Its original hit still owns this up while it remains
        // the same enabled node; an earlier move or a retired hit does not.
        let capturedHit = touched === self || (touched.isDescendant(of: self)
            && touched.presenter?.views[touched.id] === touched && touched.pressContact?.touch === contact.touch
            && touched.pressContact?.cancelled == false && !touched.disabled && !touched.inert)
        let movedByFocus = ownsPress && capturedHit && contact.cancelledByReparenting
            && presenter?.views[id] === self && !disabled && !inert
        guard !contact.cancelled || movedByFocus else { return }
        guard ownsPress else {
            if touched === self, !pressesAnAncestor, !contact.routesToCanvas, touches.first.map(pressInside) == true {
                presenter?.menus.activating(from: touched, pressing: nil) { contact.destination?.touchesEnded(touches, with: event) }
            } else { contact.destination?.touchesEnded(touches, with: event) }
            return
        }
        let lands = inside && pressContact === contact && incarnation == contact.incarnation && (!contact.cancelled || movedByFocus) && presenter?.views[id] === self
        // A tap on a list's row selects its cell before the press runs, and
        // the cell stays selected under a screen the press pushed (LLP 1115.000 D16).
        let cells = presenter?.rowCells, screen = cells?.screen(of: self)
        let row = lands ? cells?.select(self) : nil
        pressed = overlayHoldsPress
        if lands {
            presenter?.menus.activating(from: touched, pressing: self) {
                presenter?.press(id, held: KeyCodes.held(event?.modifierFlags ?? []), bubbled: contact.bubbled)
            }
        }
        if let row { cells?.ran(row, on: screen) }
    }
    package override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        if canvasTouches(touches, phase: "cancel", event: event) { return }
        if touches.isEmpty { cancelPressContact(); pressed = overlayHoldsPress; return }
        guard let contact = currentPressContact(touches) else { return }
        contact.cancelled = true
        setInvokerLook(contact, nil)
        inlinePressed = nil; linkPressed = nil; svgPressed = nil
        if pressed { pressed = overlayHoldsPress } else { contact.destination?.touchesCancelled(touches, with: event) }
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
        let target = activationTarget(at: windowPoint), consumed = controlTakesTouch(at: windowPoint)
        if target == nil, !takesPress, consumed { return nil }
        let bubbled = target.map { (presenter?.pressBox(of: $0, from: self) ?? $0) !== $0 } ?? false
        if let target, !bubbled, target.isSurfaceControl { return target.control("down") && target.control("up") ? target : nil }
        // VoiceOver's activation of a focused opener hands its focus on as a tap's does.
        let handoff = target.flatMap { $0.presenter?.beginPress(control: $0.textArea ?? $0.field ?? $0) }
        defer { handoff?.dispatched() }
        let invoker = presenter?.menus.activating(from: self, pressing: target, consumed: consumed) {
            if let target { target.presenter?.press(target.id, bubbled: bubbled) }
        }
        return target ?? invoker
    }

    /// `handoff`: VoiceOver's activation, which hands the press's focus on as a tap's does (`activate(at:)`).
    func activateKey(_ name: String, handoff: Bool = false) -> Bool {
        guard !disabled, name == "Enter" || (name == " " && props["href"] == nil && UIDevice.current.userInterfaceIdiom != .tv) else { return false }
        #if os(iOS)
        if kind == "button", let presenter, presenter.invocation(of: self) != nil {
            guard !inert else { return false }
            // Keyboard activation has no hit test; menu overlays own only touches.
            let target = sequence(first: self as UIView, next: { presenter.listenerParent(of: $0) })
                .compactMap { $0 as? NodeView }.prefix { !$0.disabled && !$0.authoredInert }
                .first { $0.takesPress(bubbled: presenter.pressBox(of: $0, from: self) !== $0) }
            let pressHandoff = handoff ? target.map { presenter.beginPress(control: $0.textArea ?? $0.field ?? $0) } : nil
            defer { pressHandoff?.dispatched() }
            presenter.menus.activating(from: self, pressing: target) {
                if let target { presenter.press(target.id, bubbled: presenter.pressBox(of: target, from: self) !== target) }
            }
            return true
        }
        #endif
        guard handlers.contains("press") || defaultLink != nil else { return false }
        presenter?.press(id)
        return true
    }
    /// Whether a press is this node's own: a `press` handler, a link, a
    /// presented menu's closer, a surface control or a native button that
    /// invokes a command or a popover. A touch stops at the first such node
    /// from the one it hit up; a surface control alone takes no bubbled press.
    var takesPress: Bool { takesPress(bubbled: false) }
    /// The button over this invoker still holds its press look when a press contact ends.
    var overlayHoldsPress: Bool { presenter?.menus.overlayHoldsPress(self) ?? false }
    func takesPress(bubbled: Bool) -> Bool {
        handlers.contains("press") || defaultLink != nil || presenter?.menus.closesPresentedContent(self) == true || (!bubbled && isSurfaceControl)
            || isNativeButton && presenter?.invocation(of: self) != nil
    }
    /// Resolve before focus changes: a keyboard resize can move the control.
    func activationTarget(at windowPoint: CGPoint) -> NodeView? {
        guard !inert else { return nil }
        if !takesPress, controlTakesTouch(at: windowPoint) { return nil }
        var v: UIView? = self
        while let cur = v {
            if let n = cur as? NodeView, n.disabled || n.authoredInert { return nil }
            if let n = cur as? NodeView {
                let box = presenter?.pressBox(of: n, from: self) ?? n
                if n.takesPress(bubbled: box !== n) {
                    guard box.bounds.contains((box as? NodeView)?.local(windowPoint) ?? box.convert(windowPoint, from: nil)) else { return nil }
                    return n
                }
            }
            v = presenter?.listenerParent(of: cur) ?? cur.superview
        }
        return nil
    }
    /// A native control takes its own hit area; padding that hits its
    /// NodeView forwards the touch. A native button's action still bubbles.
    func controlTakesTouch(at windowPoint: CGPoint) -> Bool {
        guard let hit = window?.hitTest(windowPoint, with: nil), hit.isDescendant(of: self) else { return false }
        var at: UIView? = hit, takesHit = false
        while let cur = at, !(cur is NodeView) {
            takesHit = takesHit || cur === field || (cur as? UIControl)?.isEnabled == true && !(cur is NativeButtonIOS)
            #if os(iOS)
            takesHit = takesHit || (cur as? UITextView)?.isEditable == true
            #endif
            at = cur.superview
        }
        return at === self && takesHit
    }
    package override func accessibilityActivate() -> Bool {
        #if os(iOS)
        if kind == "button", presenter?.invocation(of: self) != nil { return activateKey("Enter", handoff: true) }
        #endif
        return activate(at: convert(CGPoint(x: bounds.midX, y: bounds.midY), to: nil)) != nil
    }
}
extension NodeView {
    /// A press's one focus decision, before any of its callbacks, walking from the node it landed in (this one)
    /// up to its target, or the whole way with none: the innermost node that asked for focus takes it, as a click
    /// focuses the nearest focusable ancestor; with none, the text editing under way ends, as a click on a button
    /// blurs a page's input, and any other focus stays, as UIKit's buttons leave the first responder. Under
    /// `retainFocus`, or past a disabled node, the press changes nothing.
    /// The press's handoff begins first, so an autofocus its `focus` handler
    /// mounts may take the focus; the caller tells it when the press's
    /// dispatch has returned (PressHandoffIOS).
    func focusForPress(target: NodeView?) -> PressHandoff? {
        guard let presenter else { return nil }
        let handoff = presenter.beginPress(control: target.map { $0.textArea ?? $0.field ?? $0 })
        guard !presenter.contextRetainsFocus(self) else { return handoff }
        let bubbled = target.map { presenter.pressBox(of: $0, from: self) !== $0 } ?? false
        var at: UIView? = self
        while let view = at, view !== presenter.viewport {
            if let node = view as? NodeView {
                if node.disabled { return handoff }
                if node.focusesOnPress {
                    handoff.control = node
                    if !node.isFirstResponder { node.becomeFirstResponder() }
                    return handoff
                }
                if node === target { break }
            }
            // Focus stays in the content the touch hit. In particular, a
            // press bubbling out of a dialog cannot blur the page's field.
            if bubbled, presenter.listenerParent(of: view) !== view.superview {
                view.endEditing(true)
                return handoff
            }
            at = view.superview
        }
        guard presenter.hasKeyboardEditor else { return handoff }
        // A button keeps the keyboard and its field, as a `UIButton` leaves
        // the first responder alone: what was typed is committed (`change`),
        // as the web's blur commits it, and the field is not blurred. Ground
        // that presses nothing ends the editing, as a hand-built screen's
        // tap on its background does.
        if let target, target.actsAsButton || target.isNativeButton {
            // The focus the press leaves is the editor's, which an autofocus
            // the press mounts may take, as it took the button's (`pressHandsOff`).
            if let editor = presenter.views.values.lazy.compactMap({ $0.textArea.map { $0 as UIView } ?? $0.field.map { $0 as UIView } }).first(where: \.isFirstResponder) {
                handoff.control = editor
            }
            if let id = presenter.edited, let field = presenter.views[id], let text = field.field?.text ?? field.textArea?.text {
                presenter.commitEdit(id, text, change: field.handlers.contains("change"))
            }
            return handoff
        }
        presenter.endEditing()
        return handoff
    }
    /// A finger's touch bubbling from this node reaches a node it pressed
    private var pressesAnAncestor: Bool {
        var at = presenter?.listenerParent(of: self) ?? superview
        while let view = at { if (view as? NodeView)?.pressed == true { return true }; at = presenter?.listenerParent(of: view) ?? view.superview }
        return false
    }
    /// The node a touch landed in: its view, or that view's nearest node
    static func innermost(_ view: UIView) -> NodeView? {
        var at: UIView? = view
        while let candidate = at { if let node = candidate as? NodeView { return node }; at = candidate.superview }
        return nil
    }
}
#endif
