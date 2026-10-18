// A custom invoker's activation (HTML's activation behaviour): the nearest
// invoker on a press's path is invoked after the press handler runs, once.

#if os(iOS) || os(tvOS)
import UIKit

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
