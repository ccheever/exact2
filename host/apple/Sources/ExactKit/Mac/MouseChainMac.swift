#if os(macOS)
import AppKit

/// One mouse contact's candidates, in LLP 1057.001 §1's order: innermost first,
/// then reorder > transform > height > pan > swipe on one node. Every recognizer
/// arms on down; drags go to the first armed candidate that takes them, and the
/// first to engage keeps the contact while the others are cancelled.
package protocol MouseRecognizer: AnyObject {
    var armed: NodeView? { get }
    var engaged: Bool { get }
    func arm(_ node: NodeView, event: NSEvent)
    func drag(_ event: NSEvent) -> Bool
    func up(_ event: NSEvent) -> Bool
    func cancel()
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

extension Presenter {
    /// Only an open dialog or popover forwards a press to its authored parent.
    func presentationParent(of view: NSView) -> NSView? {
        guard let node = view as? NodeView else { return nil }
        return dialogs.authoredParent(of: node) ?? menus.authoredParent(of: node)
    }
}

struct PressCrossing {
    weak var root: NodeView?
    weak var parent: NSView?
    private let open: () -> Bool
    var isOpen: Bool { open() }
    init(_ root: NodeView, parent: NSView, presenter: Presenter) {
        self.root = root; self.parent = parent
        if let entry = presenter.dialogs.entries.first(where: { $0.dialog === root }) {
            open = { [weak presenter, weak entry] in
                guard let entry else { return false }
                return presenter?.dialogs.entries.contains { $0 === entry } == true
            }
        } else if let entry = presenter.menus.entries.first(where: { $0.popover === root }) {
            open = { [weak presenter, weak entry] in
                guard let entry else { return false }
                return presenter?.menus.entries.contains { $0 === entry } == true
            }
        } else {
            open = { [weak presenter, weak root, weak parent] in
                guard let root, let parent else { return false }
                return presenter?.presentationParent(of: root) === parent && root.superview !== parent
            }
        }
    }
}

/// A node on the captured press path, rechecked after app callbacks.
struct PressNode {
    private(set) weak var node: NodeView?
    private weak var presenter: Presenter?
    private let id: UInt32
    private let moves: UInt64
    private let bubbled: Bool
    init(_ node: NodeView, bubbled: Bool) {
        self.node = node; presenter = node.presenter; id = node.id; moves = node.pressMoves; self.bubbled = bubbled
    }
    var isLive: Bool {
        guard let node else { return false }
        return presenter?.views[id] === node && node.pressMoves == moves && node.window != nil && !node.disabled
            && !(bubbled ? node.authoredInert : node.inert)
    }
    func isInside(_ subtree: NSView) -> Bool { node?.isDescendant(of: subtree) == true }
}

struct PressPath {
    var nodes: [PressNode] = []
    var crossings: [PressCrossing] = []
    var isLive: Bool { crossings.allSatisfy(\.isOpen) && nodes.allSatisfy(\.isLive) }
    func isInside(_ subtree: NSView) -> Bool { nodes.contains { $0.isInside(subtree) } }
}

/// Only a down forwarded out of a lifted root owns a bubbled press.
/// Cancellation keeps its identity until another down replaces it.
final class BubbledPress {
    let event: NSEvent
    private(set) weak var source: NodeView?
    private(set) weak var target: NodeView?
    private var path: PressPath
    private(set) var cancelled = false
    var box: NodeView? { cancelled || target == nil ? nil : source }

    init(_ event: NSEvent, source: NodeView, path: PressPath) {
        self.event = event; self.source = source; self.path = path
    }
    func responder(from node: NodeView, event: NSEvent, presenter: Presenter) -> NSResponder? {
        validate()
        guard !cancelled else { return nil }
        if let crossing = path.crossings.first(where: { $0.root === node }) { return crossing.parent }
        guard let parent = presenter.presentationParent(of: node), parent !== node.superview else { return node.nextResponder }
        guard event.type == .leftMouseDown else { return node.nextResponder }
        path.crossings.append(PressCrossing(node, parent: parent, presenter: presenter))
        return parent
    }
    func reaches(_ node: NodeView) -> Bool {
        guard let root = path.crossings.first?.root else { return false }
        return node !== root && !node.isDescendant(of: root)
    }
    func validate() {
        if !path.isLive { cancel() }
    }
    func cancel(in subtree: NSView? = nil) {
        guard !cancelled else { return }
        if let subtree, !path.isInside(subtree) { return }
        cancelled = true
        if let target, target.pressed { target.pressed = false }
    }
    func down(on node: NodeView) {
        guard !cancelled, target == nil else { return }
        path.nodes.append(PressNode(node, bubbled: true))
        validate()
        guard !cancelled else { return }
        node.presenter?.interacting = node.id
        if node.pressable { target = node; node.pressed = true }
        else { node.presenter?.mouseChain.responder(from: node, event: event)?.mouseDown(with: event) }
    }
    func lands(_ event: NSEvent) -> Bool {
        box.map { $0.pressInside(NodeView.point(event, in: $0.window)) } ?? false
    }
    func drag(on node: NodeView, event: NSEvent) {
        if target === node { node.pressFollows(inside: lands(event)) }
        node.presenter?.mouseChain.responder(from: node, event: event)?.mouseDragged(with: event)
    }
    func up(on node: NodeView, event: NSEvent) {
        guard target === node else {
            node.presenter?.mouseChain.responder(from: node, event: event)?.mouseUp(with: event)
            return
        }
        guard node.pressed, let presenter = node.presenter else { return }
        guard lands(event) else { node.pressed = false; return }
        // A field's `change` can invalidate any node or opening on the path.
        presenter.commitPendingEdit()
        validate()
        guard !cancelled else { return }
        node.pressed = false
        presenter.press(node.id, held: KeyCodes.held(event.modifierFlags), bubbled: true)
    }
}

final class MouseChain {
    weak var presenter: Presenter?
    private weak var downEvent: NSEvent?
    private struct ReceivedDown {
        let event: NSEvent
        weak var source: NodeView?
        let path: PressPath
    }
    private var received: ReceivedDown?
    private(set) var bubbledPress: BubbledPress?
    func completed(_ contact: BubbledPress?, at source: NodeView) {
        // A cancelled click ends at whichever view hears its up.
        guard let contact, bubbledPress === contact, contact.cancelled || contact.source === source else { return }
        contact.cancel()
        bubbledPress = nil
    }
    /// Capture the hit and its boundary before pointer callbacks can move either.
    /// This records no press; only forwarding across that boundary creates one.
    func receivedDown(_ source: NodeView, event: NSEvent) {
        guard bubbledPress?.event !== event, received?.event !== event else { return }
        bubbledPress?.cancel()
        bubbledPress = nil
        var at: NSView? = source
        var path = PressPath()
        while let view = at {
            if let node = view as? NodeView { path.nodes.append(PressNode(node, bubbled: false)) }
            if let node = view as? NodeView, let presenter,
               let parent = presenter.presentationParent(of: node), parent !== node.superview {
                path.crossings.append(PressCrossing(node, parent: parent, presenter: presenter))
                break
            }
            at = view.superview
        }
        received = ReceivedDown(event: event, source: source, path: path)
    }
    func responder(from node: NodeView, event: NSEvent) -> NSResponder? {
        if let contact = bubbledPress, let presenter { return contact.responder(from: node, event: event, presenter: presenter) }
        guard event.type == .leftMouseDown else { return node.nextResponder }
        let saved = received?.path.crossings.first
        let crossing: PressCrossing?
        if let saved, saved.root === node { crossing = saved }
        else if let presenter, let parent = presenter.presentationParent(of: node), parent !== node.superview {
            crossing = PressCrossing(node, parent: parent, presenter: presenter)
        } else { crossing = nil }
        guard let crossing else { return node.nextResponder }
        if let received, let source = received.source, let presenter {
            var path = received.path
            if saved?.root !== node {
                // A callback may have opened a presentation along the captured path.
                guard let end = path.nodes.firstIndex(where: { $0.node === node }) else { return nil }
                path.nodes = Array(path.nodes[...end])
                path.crossings = [crossing]
            }
            let contact = BubbledPress(event, source: source, path: path)
            bubbledPress = contact
            self.received = nil
            if presenter.views[source.id] !== source || source.removedUnderHold { contact.cancel() }
            return contact.responder(from: node, event: event, presenter: presenter)
        }
        return nil
    }
    /// The click last asked for its `dblclick`: one a view passes up its
    /// superviews is one double click (`NodeView.dblclickTarget`).
    weak var doubled: NSEvent?
    private(set) var order: [MouseRecognizer] = []
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Rule 4's rank on one node, the tie-break after depth.
    var ranked: [MouseRecognizer] {
        guard let p = presenter else { return [] }
        return [p.mouseDrags.reorder, p.mouseDrags.transform, p.mouseDrags.height, p.mouseLayoutPan, p.mouseSwipe]
    }
    /// Arms every recognizer once per event (forwarding reaches the
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
                bubbledPress?.cancel()
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
    /// The innermost enabled `dblclick` from here up the authored tree
    /// (`eventParent`); dispatched after this click's own `press`, the web's
    /// order (LLP 1057.001 §1 rule 5). Once an event, however many of the
    /// views on the way pass its up along.
    func dblclickTarget(_ event: NSEvent) -> NodeView? {
        guard event.clickCount == 2, let presenter, presenter.mouseChain.doubled !== event else { return nil }
        presenter.mouseChain.doubled = event
        var next: NSView? = self
        while let view = next {
            if let node = view as? NodeView, !node.disabled, node.handlers.contains("dblclick") { return node }
            next = presenter.eventParent(of: view)
        }
        return nil
    }
    /// `pointerdown` (LLP 1005 §3): the button down, heard by every enabled
    /// node from here up that hears `pointerdown`, innermost first, as DOM's
    /// bubbles (Charlie, 2026-10-15). Each node from here up that hears it
    /// or `pointerup` is held until the button comes up, wherever that is;
    /// the innermost hearing any of the three, `pointermove` included, also
    /// holds its drags as its moves (LLP 1056 §3 stage 3). Up the authored
    /// tree (`eventParent`): a press in an open popover or a modal dialog
    /// reaches its ancestors, though its view is in the top layer.
    func pointerPressed(_ event: NSEvent?) {
        presenter?.focusByPointer = true
        presenter?.flushHoverMove()
        guard let presenter, presenter.pointerHeld == nil, presenter.pointerPath.isEmpty else { return }
        var path: [NodeView] = []
        var next: NSView? = self
        while let view = next {
            if let node = view as? NodeView, !node.disabled, node.wantsPointer {
                if presenter.pointerHeld == nil {
                    presenter.pointerHeld = node.id
                    presenter.pointerSource = self
                }
                if node.handlers.contains("pointerdown") || node.handlers.contains("pointerup") { path.append(node) }
            }
            next = presenter.eventParent(of: view)
        }
        presenter.pointerPath = path.map(\.id)
        // A handler may remove or disable a node further up before its turn,
        // or stop this down there; the up's path and the hold stay.
        presenter.propagating {
            for node in path where node.handlers.contains("pointerdown") && presenter.views[node.id] === node && !node.disabled {
                presenter.pointer(node.id, .down, node.pointerSample(event))
                if presenter.propagationStopped { break }
            }
        }
    }
    /// `pointerup` for every node the button's down was held by, innermost first.
    func pointerReleased(_ event: NSEvent?) {
        holdPresenter?.flushHoverMove()
        // The button is up: what lies under the pointer now is what is
        // hovered, with no further move, as AppKit's tracking areas report an
        // enter "on mouseUp after a drag" (NSTrackingArea.h; X80). While the
        // button was held nothing changed, which is AppKit's rule and not a
        // browser's (LLP 1115.000 D7).
        defer { (holdPresenter ?? presenter)?.followPointer() }
        guard let presenter = holdPresenter else { return }
        // The view it went down on goes with the button, though a release
        // took the held node first (LLP 1005: that clears the hold).
        let source = presenter.pointerSource
        presenter.pointerSource = nil
        defer { source?.releaseHold() }
        let path = presenter.pointerPath
        presenter.pointerHeld = nil
        presenter.pointerPath = []
        presenter.propagating {
            for id in path {
                guard let node = presenter.views[id], node.handlers.contains("pointerup") else { continue }
                presenter.pointer(id, .up, node.pointerSample(event, lifted: true))
                if presenter.propagationStopped { break }
            }
        }
    }
    /// A drag of the held pointer: its node's `pointermove`. AppKit
    /// coalesces drags to one a frame.
    func pointerDragged(_ event: NSEvent) {
        // A drag a view passes up its superviews is one move.
        guard let presenter = holdPresenter, presenter.pointerDrag !== event, let held = presenter.pointerHeld,
              let node = presenter.views[held], node.handlers.contains("pointermove") else { return }
        presenter.pointerDrag = event
        presenter.pointer(held, .move, node.pointerSample(event))
    }
    /// A free pointer moving over this node: its `pointermove`, when no
    /// node under it nearer in the authored tree (`eventParent`) hears one,
    /// at most one a display frame, the latest, as the web host sends it
    /// (`input-glue.js`; LLP 1056 §3). A mouse can report several a frame.
    /// The hit is in the event's window: a popover's or a sheet's own.
    func pointerHovered(_ event: NSEvent) {
        guard let presenter, presenter.pointerHeld == nil, handlers.contains("pointermove"), !disabled,
              let content = (event.window ?? window)?.contentView,
              let hit = TableRowCell.whatIsThere({ content.hitTest(content.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow) }) else { return }
        var view: NSView? = hit
        while let v = view, v !== self {
            if let n = v as? NodeView, n.handlers.contains("pointermove"), !n.disabled, !n.inert { return }
            view = presenter.eventParent(of: v)
        }
        guard view === self else { return }
        presenter.hoverMoved(id, pointerSample(event))
    }
    /// The presenter this view's held button reports to. AppKit sends the
    /// drags and the up to the view the button went down on even after it
    /// leaves the tree, an `if` having removed the child of the held node
    /// the press landed on (a board square's piece, the chess diary via
    /// fix/syntax6): the hold is the node's, as on the web, so the gone view
    /// keeps reaching its presenter until the button comes up.
    var holdPresenter: Presenter? {
        presenter ?? (objc_getAssociatedObject(self, &holdKey) as? HoldBox)?.presenter
    }
    var wantsPointer: Bool { handlers.contains("pointerdown") || handlers.contains("pointerup") || handlers.contains("pointermove") }
    /// The record of `event` (the current one when nil) as this node sees it:
    /// from its content box, through every transform (`local`). A tablet's
    /// pen has its pressure; a mouse, DOM's 0.5 while a button is down.
    func pointerSample(_ event: NSEvent?, lifted: Bool = false) -> PointerSample {
        let e = event ?? NSApp.currentEvent
        let point = e.map { local(Self.point($0, in: window)) } ?? .zero
        let box = contentBox()
        // `subtype` is a mouse event's: read from a key event, an enter or an
        // exit, it raises. A menu item chosen with Return leaves its key's up
        // as the current event when `popUp` returns, where the right press
        // that opened the menu ends its hold (MenusMac.swift `track`).
        let pen = e.map { Self.subtyped.contains($0.type) && $0.subtype == .tabletPoint } ?? false
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
        let client = e.flatMap { e in presenter.map { $0.client(Self.point(e, in: $0.viewport.window)) } } ?? .zero
        return PointerSample(x: Double(point.x - box.minX), y: Double(point.y - box.minY), buttons: buttons,
                             pressure: pressure, type: pen ? "pen" : "mouse", id: pen ? 2 : 1,
                             clientX: Double(client.x), clientY: Double(client.y), held: KeyCodes.held(e?.modifierFlags ?? []))
    }
    /// The events whose `subtype` can be read: a button's, a move, a drag.
    static let subtyped: Set<NSEvent.EventType> = [.leftMouseDown, .leftMouseUp, .rightMouseDown, .rightMouseUp,
        .otherMouseDown, .otherMouseUp, .mouseMoved, .leftMouseDragged, .rightMouseDragged, .otherMouseDragged]
    /// `event`'s point in `window`'s coordinates: a press in a native
    /// popover, its own window, bubbles to authored ancestors in the page's.
    static func point(_ event: NSEvent, in window: NSWindow?) -> NSPoint {
        guard let from = event.window, let window, from !== window else { return event.locationInWindow }
        return window.convertPoint(fromScreen: from.convertPoint(toScreen: event.locationInWindow))
    }
    func dispatchDblclick(_ node: NodeView?) {
        guard let node, let presenter = node.presenter, presenter.views[node.id] === node, !node.disabled else { return }
        presenter.dblclick(node.id)
    }
}
extension Presenter {
    /// A node still holds the button that went down: the held one, or an
    /// ancestor its down bubbled to, waiting for the up.
    var pointerHolds: Bool { pointerHeld != nil || !pointerPath.isEmpty }
    /// A window point from the viewport's top-left, the page scroll applied:
    /// DOM's `clientX`/`clientY`, `frame()`'s space (LLP 1094 D11).
    func client(_ windowPoint: NSPoint) -> NSPoint {
        let clip = viewport.contentView, p = clip.convert(windowPoint, from: nil)
        return NSPoint(x: p.x - clip.bounds.minX, y: p.y - clip.bounds.minY)
    }
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
    /// A batch or a scroll moved what lies under a resting pointer: at the
    /// next display frame the pointer is hit-tested where it rests and its
    /// hover follows, as a browser's does after layout or a scroll (the
    /// boundary events of a synthetic mouse move; #139). AppKit's tracking
    /// areas report only a pointer that moves. One hit-test a frame, and
    /// none while a button is down or the pointer is outside the window.
    func followPointer() {
        guard followLink?.isPaused != false, restingPointer() != nil else { return }
        if let followLink { followLink.isPaused = false; return }
        // One link, paused between hit-tests: a fling asks every frame.
        let link = viewport.displayLink(target: followTarget, selector: #selector(PumpTarget.tick(_:)))
        // A presenter released with a hit-test pending leaves no link firing.
        followTarget.fire = { [weak self, weak link] _ in
            guard let self else { link?.invalidate(); return }
            self.hoverUnderPointer()
        }
        link.add(to: .main, forMode: .common)
        followLink = link
    }
    /// Whether a hit-test waits for the next frame.
    var followPending: Bool { followLink?.isPaused == false }
    /// The pointer in window points while it rests over this window's
    /// content with no button down: the agent's under the agent (never the
    /// system cursor, which the drive does not own), else the cursor, and
    /// only where no other window covers it when `frontmost` is asked.
    func restingPointer(frontmost: Bool = false) -> NSPoint? {
        guard pointerHeld == nil, pointerSource == nil, let window = viewport.window, let content = window.contentView else { return nil }
        let p: NSPoint
        if let agentPointer { p = agentPointer } else {
            // Hover is the key window's (LLP 1115.000 D7.9).
            guard !ExactEnv.agentMode, window.isVisible, window.isKeyWindow, NSEvent.pressedMouseButtons == 0 else { return nil }
            p = window.mouseLocationOutsideOfEventStream
            if frontmost, NSWindow.windowNumber(at: NSEvent.mouseLocation, belowWindowWithWindowNumber: 0) != window.windowNumber { return nil }
        }
        return content.bounds.contains(content.convert(p, from: nil)) ? p : nil
    }
    /// A move and a resting-pointer layout update share the same hit test.
    /// Ancestor tracking exits cannot evict a child under visible overflow.
    func hoverAt(_ point: NSPoint) -> NodeView? {
        guard let content = viewport.window?.contentView,
              viewport.bounds.contains(viewport.convert(point, from: nil)) else {
            setHoverPath([]); pointerCursor(over: nil, at: point); return nil
        }
        // What is under the pointer, a table's row included (the table takes its clicks, not its hover).
        let hit = TableRowCell.whatIsThere { content.hitTest(content.superview?.convert(point, from: nil) ?? point) }
        var path: [NodeView] = []
        var leaf: NodeView?
        var region = false
        var view = hit?.isDescendant(of: viewport) == true ? hit : nil
        while let current = view {
            if let node = current as? NodeView, !node.inert {
                if leaf == nil { leaf = node }
                path.append(node)
            } else if leaf == nil, current is RegionSurfaceMac { region = true }
            view = current.superview
        }
        let run = leaf.flatMap { $0.inlineTarget(at: $0.local(point), handler: "hover") }
        setHoverPath(path, inline: run?.id)
        pointerCursor(over: leaf, at: point, region: region)
        return leaf
    }
    func trackPointer(_ event: NSEvent) {
        guard trackedPointerEvent !== event else { return }
        trackedPointerEvent = event
        trackedCanvas = nil
        // A move in a popover's or a sheet's own window is that window's
        // (`BlockView.pointerTracking`); the page's hover is left as it is.
        let apart = event.window.map { $0 !== viewport.window && hosts($0) } ?? false
        // A held contact owns move delivery, but leaving the window still
        // ends hover. Other tracking areas may exit inside the window; hit
        // testing there keeps the subtree currently under the pointer.
        if event.type == .mouseExited { if !apart { _ = hoverAt(event.locationInWindow) } else { hoverLooks([]) }; return }
        guard pointerHeld == nil, pointerSource == nil else { return }
        let leaf = apart ? event.window.flatMap { Self.node(at: event.locationInWindow, in: $0) } : hoverAt(event.locationInWindow)
        // Its own window's `:hover` looks: the node and its authored ancestors there (LLP 1121 D8).
        if apart { hoverLooks(from: leaf) }
        guard let leaf else { return }
        // The `pointermove` nearest it in the authored tree: out of a popover
        // or a dialog to its ancestors, never to a page node under it.
        var view: NSView? = leaf
        while let current = view {
            if let node = current as? NodeView, !node.disabled, node.handlers.contains("pointermove") {
                node.pointerHovered(event); break
            }
            view = eventParent(of: current)
        }
        view = leaf
        while let current = view {
            if let input = (current as? NodeView)?.canvasInput {
                _ = input.pointer(event, phase: "move")
                trackedCanvas = input
                break
            }
            view = current.superview
        }
    }
    /// The innermost node under `point` in `window` that is not inert.
    static func node(at point: NSPoint, in window: NSWindow) -> NodeView? {
        guard let content = window.contentView else { return nil }
        // A table's row included, as `hoverAt` reads the page's.
        var view = TableRowCell.whatIsThere { content.hitTest(content.superview?.convert(point, from: nil) ?? point) }
        while let current = view {
            if let node = current as? NodeView, !node.inert { return node }
            view = current.superview
        }
        return nil
    }
    func hoverUnderPointer() {
        followLink?.isPaused = true
        guard let point = restingPointer(frontmost: true) else { return }
        _ = hoverAt(point)
    }
    /// The window stopped being key: nothing in it is hovered, as nothing in
    /// a background AppKit window is (LLP 1115.000 D7.9), and the cursor this
    /// set for its text is given back. The agent's pointer is its own: its
    /// window is never key, and its hover is where its drive put it.
    func windowLostKey() {
        guard agentPointer == nil, !ExactEnv.agentMode else { return }
        setHoverPath([])
        pointerCursor(over: nil, at: .zero)
        // A button held as the window resigns has its up delivered to no
        // view of it: the press and what it shows end here (LLP 1061 D7).
        for view in views.values where view.pressed { view.pressed = false }
    }

}
extension NodeView {
    /// AppKit sends the held button's drags and up only to the view it went
    /// down on, and only while that view is in its window: one taken out
    /// leaves the hold deaf. So the view the button went down on stays,
    /// transparent, until the button comes up, when a batch removes it (an `if`
    /// dropping the piece a board square was pressed on), and the held node
    /// keeps hearing its moves and its up, as on the web (`holdPresenter`).
    /// Removing it then is AppKit's `removeFromSuperview`, the one way the
    /// presenter takes a view out. Only in the window: one already out hears
    /// nothing, and AppKit also asks here when it frees the parent
    /// (`-[NSView _finalize]`), which a view that stayed would outlive,
    /// pointing at it (the T3 Code clone's crash in `releaseHold`).
    package override func removeFromSuperview() {
        holdPresenter?.mouseChain.bubbledPress?.cancel(in: self)
        if let presenter = holdPresenter, presenter.pointerHolds, presenter.pointerSource === self, window != nil {
            // Transparent, not hidden: AppKit sends a hidden view no drags either.
            let box = holdBox(presenter)
            if !box.hid { box.hid = true; box.alpha = alphaValue; alphaValue = 0 }
            return
        }
        super.removeFromSuperview()
    }
    private func holdBox(_ presenter: Presenter) -> HoldBox {
        if let box = objc_getAssociatedObject(self, &holdKey) as? HoldBox { return box }
        let box = HoldBox(presenter)
        objc_setAssociatedObject(self, &holdKey, box, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return box
    }
    var removedUnderHold: Bool { (objc_getAssociatedObject(self, &holdKey) as? HoldBox)?.hid == true }
    /// Leaving the tree while it is the source of a hold (`forget`).
    func keepHold() {
        guard let presenter, presenter.pointerHolds, presenter.pointerSource === self else { return }
        _ = holdBox(presenter)
    }
    /// A batch that removed the held source puts it in a parent again (a
    /// move, or a reorder among its siblings): it is not leaving the tree, so
    /// it goes there now, shown — still in the window, it still hears the
    /// hold (b6 review B8).
    func rejoinUnderHold() {
        guard let box = objc_getAssociatedObject(self, &holdKey) as? HoldBox, box.hid else { return }
        box.hid = false
        alphaValue = box.alpha
        super.removeFromSuperview()
    }
    /// The button came up: a source a batch removed under the hold goes now,
    /// and one the tree still has shows again.
    func releaseHold() {
        guard let box = objc_getAssociatedObject(self, &holdKey) as? HoldBox else { return }
        objc_setAssociatedObject(self, &holdKey, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        guard box.hid else { return }
        alphaValue = box.alpha
        if presenter?.views[id] !== self { super.removeFromSuperview() }
    }
}
private var holdKey: UInt8 = 0
private final class HoldBox {
    weak package var presenter: Presenter?
    /// The batch removed the view under the hold: it stayed, transparent,
    /// its own opacity kept here.
    var hid = false
    var alpha: CGFloat = 1
    init(_ presenter: Presenter) { self.presenter = presenter }
}
#endif
