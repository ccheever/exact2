// A custom invoker's activation (HTML's activation behaviour): the nearest
// invoker on a press's path is invoked after the press handler runs, once.

#if os(iOS) || os(tvOS)
import UIKit

/// The transparent button `MenuHost` lays over an invoker, above its
/// children: a touch anywhere in the invoker opens its menu, as on a UIButton
/// whose subviews are its content. A child that is interactive itself (a
/// field, a control, a link, a node with its own press or target) keeps its
/// touches. Its highlight is the invoker's press look, as a UIButton's
/// content dims while a finger holds it.
class InvokerButton: UIButton {
    weak var invoker: NodeView? { didSet { if pressing !== invoker { releasePress() } } }
    /// The node this button's highlight pressed, until it lets go.
    private weak var pressing: NodeView?
    override var isHighlighted: Bool {
        didSet {
            guard isHighlighted != oldValue else { return }
            releasePress()
            if isHighlighted, let invoker { invoker.pressed = true; pressing = invoker }
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
        guard let hit = super.hitTest(point, with: event) else { return nil }
        guard let invoker, superview === invoker else { return hit }
        // What the point would reach without this button: the frontmost child.
        for child in NodeView.hitOrder(invoker.subviews) where child !== self {
            guard let under = child.hitTest(convert(point, to: child), with: event) else { continue }
            var at: UIView? = under
            while let view = at, view !== invoker {
                if Self.interactive(view) { return nil }
                at = view.superview
            }
            break
        }
        return hit
    }
    private static func interactive(_ view: UIView) -> Bool {
        guard let node = view as? NodeView else {
            #if os(iOS)
            if (view as? UITextView)?.isEditable == true { return true }
            #endif
            return (view as? UIControl)?.isEnabled == true
        }
        if node.field != nil || node.textArea != nil || node.isSurfaceControl { return true }
        if node.kind == "button" || node.kind == "control" || node.handlers.contains("press") { return true }
        return ["href", "popovertarget", "commandfor"].contains { !(node.props[$0] ?? "").isEmpty }
    }
}

extension MenuHost {
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

    /// The nearest invoker at or above `touched`, as a click's activation
    /// finds it (DOM ancestry, out of a presentation): a node naming a target,
    /// resolved after the press handler runs, which may mount it. None past a
    /// disabled or inert node, or a link, whose activation is its own.
    func invoker(from touched: NodeView) -> NodeView? {
        #if os(iOS)
        guard !touched.inert else { return nil }
        var at: UIView? = touched
        while let view = at {
            if let node = view as? NodeView {
                if node.disabled || node.authoredInert || !(node.props["href"] ?? "").isEmpty { return nil }
                if !(node.props["popovertarget"] ?? "").isEmpty || !(node.props["commandfor"] ?? "").isEmpty { return node }
            }
            at = presenter?.listenerParent(of: view) ?? view.superview
        }
        #endif
        return nil
    }

    /// HTML activates the nearest invoker even when its click reaches a different listener.
    /// A native control that took the touch (`consumed`) consumes it: only its own press runs.
    @discardableResult
    func activating(from touched: NodeView, pressing target: NodeView?, consumed: Bool = false, _ press: () -> Void) -> NodeView? {
        #if os(iOS)
        if !consumed, let source = invoker(from: touched), source.kind == "button", source !== target {
            let token = source.incarnation
            let bubbled = presenter.map { $0.pressBox(of: source, from: touched) !== source } ?? false
            withoutInvocation(of: target, press)
            guard source.incarnation == token, live(source), eligible(source) else { return nil }
            invoke(source, bubbled: bubbled)
            return source
        }
        #endif
        press()
        return nil
    }
}
#endif
