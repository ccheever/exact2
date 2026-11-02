// A custom invoker's activation (HTML's activation behaviour): the nearest
// button on a press's path owns activation after the press handler runs.

#if os(iOS) || os(tvOS)
import UIKit

/// The transparent button `MenuHost` lays over an invoker, above its
/// children: a touch anywhere in the invoker opens its menu, as on a UIButton
/// whose subviews are its content. What the invoker's own hit test would
/// reach without it keeps the touch when it is interactive itself: a field,
/// a control, a link, a node with its own press or target, or a platform
/// view that takes touches (a module's or a hatch's, with a recognizer).
/// Its highlight is one holder of the invoker's press look, as a UIButton's
/// content dims while a finger holds it; a press contact is the other.
class InvokerButton: UIButton {
    weak var invoker: NodeView? { didSet { if pressing !== invoker { releasePress() } } }
    /// The node this button's highlight pressed, until it lets go.
    private(set) weak var pressing: NodeView?
    /// Inside the invoker's own hit test, which must not find this button.
    private var resolving = false
    override var isHighlighted: Bool {
        didSet {
            guard isHighlighted != oldValue else { return }
            releasePress()
            if isHighlighted, let invoker { pressing = invoker; invoker.pressed = true }
        }
    }
    func releasePress() {
        guard let node = pressing else { return }
        pressing = nil
        if !node.hasPressContact { node.pressed = false }
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesCancelled(touches, with: event)
        releasePress()
    }
    override func didMoveToSuperview() {
        super.didMoveToSuperview()
        if superview == nil { releasePress() }
    }
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard !resolving, let hit = super.hitTest(point, with: event) else { return nil }
        guard let invoker, superview === invoker else { return hit }
        // No event: a touch's side effects (a paragraph's text interaction,
        // a swipe row's cell) belong to the real pass, not this question.
        resolving = true
        let target = invoker.hitTest(convert(point, to: invoker), with: nil)
        resolving = false
        // Nothing there takes touches (`pointer-events: none`): nor does this.
        guard let target else { return nil }
        var at: UIView? = target
        while let view = at, view !== invoker {
            if view !== invoker.scroll, Self.interactive(view) { return nil }
            at = view.superview
        }
        return hit
    }
    private static func interactive(_ view: UIView) -> Bool {
        guard let node = view as? NodeView else {
            #if os(iOS)
            if (view as? UITextView)?.isEditable == true { return true }
            #endif
            if (view as? UIControl)?.isEnabled == true { return true }
            return view.gestureRecognizers?.contains { $0.isEnabled } == true
        }
        if node.field != nil || node.textArea != nil || node.isSurfaceControl { return true }
        if node.kind == "button" || node.kind == "control" || node.handlers.contains("press") { return true }
        return ["href", "popovertarget", "commandfor"].contains { !(node.props[$0] ?? "").isEmpty }
    }
}

extension MenuHost {
    /// The node a touch on `source` presses, as a touch resolves it: its own
    /// handler, else the nearest ancestor's, out of a presentation, none past
    /// a disabled or inert node; and whether the press bubbled to it.
    func pressTarget(of source: NodeView) -> (node: NodeView, bubbled: Bool)? {
        guard let presenter, !source.inert else { return nil }
        var at: UIView? = source
        while let view = at {
            if let node = view as? NodeView {
                if node.disabled || node.authoredInert { return nil }
                let box = presenter.pressBox(of: node, from: source)
                if node.takesPress(bubbled: box !== node) { return (node, box !== node) }
            }
            at = presenter.listenerParent(of: view) ?? view.superview
        }
        return nil
    }
    /// Dispatches that press, then runs what `source` invokes unless it
    /// opens something: the caller presents that. The press itself invokes
    /// nothing, as `source`, the innermost button, owns activation; so a
    /// handler that retargets it to hide or close an open popover hides it.
    func pressAsTouched(_ source: NodeView) {
        guard let (node, bubbled) = pressTarget(of: source) else { return }
        let token = source.incarnation
        presenting(source) {
            withoutInvocation(of: node) { presenter?.press(node.id, bubbled: bubbled) }
            guard source.incarnation == token, presenter?.views[source.id] === source else { return }
            invoke(source, bubbled: presenter?.pressBox(of: source, from: source) !== source)
        }
    }
    /// Another path owns activation; dispatch this node's handler alone.
    func withoutInvocation(of source: NodeView?, _ press: () -> Void) {
        let previous = invocationOwner
        invocationOwner = source
        defer { invocationOwner = previous }
        press()
    }
    /// The caller presents what `source` opens; its press runs the rest.
    func presenting(_ source: NodeView, _ press: () -> Void) {
        let previous = presentationOwner
        presentationOwner = source
        defer { presentationOwner = previous }
        press()
    }

    /// The innermost button owns activation, even when it names no target.
    func invoker(from touched: NodeView) -> NodeView? {
        guard let node = presenter?.activationButton(from: touched),
              !(node.props["popovertarget"] ?? "").isEmpty || !(node.props["commandfor"] ?? "").isEmpty else { return nil }
        return node
    }

    /// Activation belongs to the button, while its press can reach another listener.
    /// A native control that took the touch (`consumed`) consumes it: only its own press runs.
    @discardableResult
    func activating(from touched: NodeView, pressing target: NodeView?, consumed: Bool = false, _ press: () -> Void) -> NodeView? {
        #if os(iOS)
        if !consumed {
            let source = presenter?.activationButton(from: touched)
            if source !== target {
                let token = source?.incarnation
                let bubbled = source.map { presenter?.pressBox(of: $0, from: touched) !== $0 } ?? false
                withoutInvocation(of: target, press)
                guard let source, source.incarnation == token, live(source), eligible(source) else { return nil }
                invoke(source, bubbled: bubbled)
                return source
            }
        }
        #endif
        press()
        return nil
    }
}
#endif
