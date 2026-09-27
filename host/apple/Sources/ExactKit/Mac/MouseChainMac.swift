#if os(macOS)
import AppKit

/// One mouse contact's candidates, in LLP 1057.001 §1's order: innermost first,
/// then reorder > transform > height > pan > swipe on one node. Every recognizer
/// arms on down; drags go to the first armed candidate that takes them, and the
/// first to engage keeps the contact while the others are cancelled.
protocol MouseRecognizer: AnyObject {
    var armed: NodeView? { get }
    var engaged: Bool { get }
    func arm(_ node: NodeView, event: NSEvent)
    func drag(_ event: NSEvent) -> Bool
    func up(_ event: NSEvent) -> Bool
    func cancel()
}
extension MouseReorder: MouseRecognizer {
    var armed: NodeView? { candidate }
    var engaged: Bool { hold != nil }
    func arm(_ node: NodeView, event: NSEvent) { down(node, event: event) }
}
extension MouseTransformDrag: MouseRecognizer {
    var armed: NodeView? { candidate }
    var engaged: Bool { hold != nil }
    func arm(_ node: NodeView, event: NSEvent) { down(node, event: event) }
}
extension MouseHeightDrag: MouseRecognizer {
    var armed: NodeView? { candidate }
    var engaged: Bool { hold != nil }
    func arm(_ node: NodeView, event: NSEvent) { down(node, event: event) }
}
extension MouseLayoutPan: MouseRecognizer {
    var armed: NodeView? { candidate }
    var engaged: Bool { active }
    func arm(_ node: NodeView, event: NSEvent) { _ = down(node, event: event) }
}
extension MouseSwipe: MouseRecognizer {
    var armed: NodeView? { candidate }
    var engaged: Bool { hold != nil }
    func arm(_ node: NodeView, event: NSEvent) { down(node, event: event) }
}

final class MouseChain {
    weak var presenter: Presenter?
    private weak var downEvent: NSEvent?
    private(set) var order: [MouseRecognizer] = []
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Rule 4's rank on one node, the tie-break after depth.
    var ranked: [MouseRecognizer] {
        guard let p = presenter else { return [] }
        return [p.mouseReorder, p.mouseTransformDrag, p.mouseHeightDrag, p.mouseLayoutPan, p.mouseSwipe]
    }
    /// Arms every recognizer once per event (a `super.mouseDown` reaches the
    /// ancestors' overrides with the same event). True while a `pan` is armed:
    /// AppKit arms no `press` on a pan contact, as before.
    func down(_ node: NodeView, event: NSEvent) -> Bool {
        if downEvent !== event {
            downEvent = event
            let recognizers = ranked
            for recognizer in recognizers { recognizer.arm(node, event: event) }
            order = MouseChain.ordered(recognizers.map { $0.armed.flatMap { MouseChain.depth(of: $0, from: node) } })
                .map { recognizers[$0] }
        }
        return presenter?.mouseLayoutPan.armed != nil
    }
    func drag(_ event: NSEvent) -> Bool {
        if let owner = order.first(where: { $0.engaged }) { return owner.drag(event) }
        for recognizer in order where recognizer.armed != nil {
            guard recognizer.drag(event) else { continue }
            if recognizer.engaged {
                for other in order where other !== recognizer { other.cancel() }
                order = [recognizer]
            }
            return true
        }
        return false
    }
    func up(_ event: NSEvent) -> Bool {
        let recognizers = order
        order = []; downEvent = nil
        var taken = false
        for recognizer in recognizers { taken = recognizer.up(event) || taken }
        return taken
    }
    /// Indices of the armed candidates (non-nil depths), innermost first, then by rank.
    static func ordered(_ depths: [Int?]) -> [Int] {
        depths.enumerated().compactMap { index, depth in depth.map { (index, $0) } }
            .sorted { ($0.1, $0.0) < ($1.1, $1.0) }.map(\.0)
    }
    static func depth(of candidate: NSView, from node: NSView) -> Int? {
        var at: NSView? = node, steps = 0
        while let view = at {
            if view === candidate { return steps }
            at = view.superview; steps += 1
        }
        return nil
    }
}

extension NodeView {
    /// The innermost enabled `dblclick` from here up; dispatched after this
    /// click's own `press`, the web's order (LLP 1057.001 §1 rule 5).
    func dblclickTarget(_ event: NSEvent) -> NodeView? {
        guard event.clickCount == 2 else { return nil }
        var next: NSView? = self
        while let view = next {
            if let node = view as? NodeView, !node.disabled, node.handlers.contains("dblclick") { return node }
            next = view.superview
        }
        return nil
    }
    func dispatchDblclick(_ node: NodeView?) {
        guard let node, let presenter = node.presenter, presenter.views[node.id] === node, !node.disabled else { return }
        presenter.dblclick(node.id)
    }
}
#endif
