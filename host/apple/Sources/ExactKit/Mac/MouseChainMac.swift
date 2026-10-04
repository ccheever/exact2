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
    /// ancestors' overrides with the same event). The `press` arms too, as on
    /// the web and Linux; a recognizer that engages cancels it (rule 4).
    func down(_ node: NodeView, event: NSEvent) {
        guard downEvent !== event else { return }
        downEvent = event
        let recognizers = ranked
        for recognizer in recognizers { recognizer.arm(node, event: event) }
        order = MouseChain.ordered(recognizers.map { $0.armed.flatMap { MouseChain.depth(of: $0, from: node) } })
            .map { recognizers[$0] }
    }
    func drag(_ event: NSEvent) -> Bool {
        if let owner = order.first(where: { $0.engaged }) { return owner.drag(event) }
        for recognizer in order where recognizer.armed != nil {
            guard recognizer.drag(event) else { continue }
            if recognizer.engaged {
                for other in order where other !== recognizer { other.cancel() }
                order = [recognizer]
                // A drag must not also dispatch the click armed at down.
                if let presenter {
                    presenter.selection.clear()
                    for view in presenter.views.values where view.pressed { view.pressed = false }
                }
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
    /// `pointerdown` (LLP 1005 §3): the primary button down on the innermost
    /// enabled node from here up that hears `pointerdown`, `pointerup` or
    /// `pointermove`; it is held until the button comes up, wherever that is,
    /// and its drags are that node's moves (LLP 1056 §3 stage 3).
    func pointerPressed(_ event: NSEvent?) {
        presenter?.flushHoverMove()
        guard let presenter, presenter.pointerHeld == nil else { return }
        var next: NSView? = self
        while let view = next {
            if let node = view as? NodeView, !node.disabled, node.wantsPointer {
                presenter.pointerHeld = node.id
                if node.handlers.contains("pointerdown") { presenter.pointer(node.id, .down, node.pointerSample(event)) }
                return
            }
            next = view.superview
        }
    }
    /// `pointerup` for the node the button went down on.
    func pointerReleased(_ event: NSEvent?) {
        presenter?.flushHoverMove()
        guard let presenter, let held = presenter.pointerHeld else { return }
        presenter.pointerHeld = nil
        if let node = presenter.views[held], node.handlers.contains("pointerup") { presenter.pointer(held, .up, node.pointerSample(event, lifted: true)) }
    }
    /// A drag of the held pointer: its node's `pointermove`. AppKit
    /// coalesces drags to one a frame.
    func pointerDragged(_ event: NSEvent) {
        // A drag a view passes up its superviews is one move.
        guard let presenter, presenter.pointerDrag !== event, let held = presenter.pointerHeld,
              let node = presenter.views[held], node.handlers.contains("pointermove") else { return }
        presenter.pointerDrag = event
        presenter.pointer(held, .move, node.pointerSample(event))
    }
    /// A free pointer moving over this node: its `pointermove`, when no
    /// node under it nearer hears one (every tracking area's owner is told),
    /// at most one a display frame, the latest, as the web host sends it
    /// (`input-glue.js`; LLP 1056 §3). A mouse can report several a frame.
    func pointerHovered(_ event: NSEvent) {
        guard let presenter, presenter.pointerHeld == nil, handlers.contains("pointermove"), !disabled,
              let content = window?.contentView,
              let hit = content.hitTest(content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow) else { return }
        var view: NSView? = hit
        while let v = view, v !== self {
            if let n = v as? NodeView, n.handlers.contains("pointermove"), !n.disabled { return }
            view = v.superview
        }
        guard view === self else { return }
        presenter.hoverMoved(id, pointerSample(event))
    }
    var wantsPointer: Bool { handlers.contains("pointerdown") || handlers.contains("pointerup") || handlers.contains("pointermove") }
    /// The record of `event` (the current one when nil) as this node sees it:
    /// from its content box, through every transform (`local`). A tablet's
    /// pen has its pressure; a mouse, DOM's 0.5 while a button is down.
    func pointerSample(_ event: NSEvent?, lifted: Bool = false) -> PointerSample {
        let e = event ?? NSApp.currentEvent
        let point = e.map { local($0.locationInWindow) } ?? .zero
        let box = contentBox()
        let pen = e?.subtype == .tabletPoint
        // The event's own button too: a synthesized one (the agent's) moves
        // no hardware state.
        var buttons = NSEvent.pressedMouseButtons
        switch e?.type {
        case .leftMouseDown?, .leftMouseDragged?: buttons |= 1
        case .rightMouseDown?, .rightMouseDragged?: buttons |= 2
        case .otherMouseDown?, .otherMouseDragged?: buttons |= 4
        case .mouseMoved?, .mouseEntered?: buttons = 0
        default: break
        }
        if lifted { buttons = 0 }
        let pressure = lifted || buttons == 0 ? 0 : pen ? Double(e?.pressure ?? 0) : 0.5
        return PointerSample(x: Double(point.x - box.minX), y: Double(point.y - box.minY), buttons: buttons,
                             pressure: pressure, type: pen ? "pen" : "mouse", id: pen ? 2 : 1)
    }
    func dispatchDblclick(_ node: NodeView?) {
        guard let node, let presenter = node.presenter, presenter.views[node.id] === node, !node.disabled else { return }
        presenter.dblclick(node.id)
    }
}
extension Presenter {
    /// Keep each node's latest free move and send them at the next display
    /// frame: a pointer crossing two `pointermove` nodes in one frame leaves
    /// each its own last move.
    func hoverMoved(_ id: UInt32, _ sample: PointerSample) {
        if let i = hoverMoves.firstIndex(where: { $0.0 == id }) { hoverMoves[i].1 = sample } else { hoverMoves.append((id, sample)) }
        guard hoverLink == nil else { return }
        let link = viewport.displayLink(target: hoverTarget, selector: #selector(PumpTarget.tick(_:)))
        // A presenter released with a move pending leaves no link firing.
        hoverTarget.fire = { [weak self, weak link] _ in
            guard let self else { link?.invalidate(); return }
            self.flushHoverMove()
        }
        link.add(to: .main, forMode: .common)
        hoverLink = link
    }
    /// The pending free moves, now: at their frame, before a down or an up,
    /// and when the agent moved the pointer (it reads the state right after).
    func flushHoverMove() {
        hoverLink?.invalidate()
        hoverLink = nil
        let moves = hoverMoves
        hoverMoves = []
        for (id, sample) in moves where views[id] != nil { pointer(id, .move, sample) }
    }
}
#endif
