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
        presenter.menus.activating(from: source ?? node, pressing: node) {
            presenter.press(node.id, held: KeyCodes.held(event.modifierFlags), bubbled: true, click: event)
        }
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
    var pressSource: NodeView? { bubbledPress?.source ?? received?.source }
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
    /// The press an editor's tracking loop reports (`NodeView.tracksPointer`).
    weak var tracked: NSEvent?
    /// A press in an editor whose `pointerdown` the window's monitor sent
    /// before NSWindow focused the editor (`Presenter.pressEditorFirst`):
    /// its tracking reports only the up, and its release, if no tracking
    /// takes it, is the monitor's. Held until one of them takes it, as
    /// AppKit may let the event go first.
    var early: NSEvent?
    /// The press whose `pointerdown` an action prevented (`preventDefault()`):
    /// it keeps the focus where it is, places no caret and starts no
    /// selection; its press still fires (LLP 1005 §3, as Chrome).
    weak var prevented: NSEvent?
    /// The button's up whose `pointerup` an action prevented: a mouse's back
    /// or forward button then steps no history (`historyButton`).
    weak var upPrevented: NSEvent?
    /// The left button's contact had its `pointerdown` prevented: its drags
    /// extend no selection. Said again at each left down (`pointerPressed`),
    /// the only way to a left drag.
    var preventedContact = false
    /// The down being handled (its view's `mouseDown`, `rightMouseDown` or
    /// `otherMouseDown`, while it runs) had its `pointerdown` prevented: a
    /// canvas or surface control takes no focus from it (`focusSurfacePointer`).
    var keepingFocus = false
    /// Early presses released before their tracking began (the session's
    /// view left its window, or a loop took the up first): their up is
    /// gone, so no tracking runs for them. The last few, each until then.
    var reported: [NSEvent] = []
    /// The early down is still bubbling (`pointerPressed`): the press's up,
    /// if a handler's loop took it then (a sheet's animation), is held and
    /// queued again once the down has bubbled, so the window has the down
    /// and then the up, as a hand's come, and no node hears its up before
    /// its down or the focus the press moves. A release asked for meanwhile
    /// (the view left its window) waits for the bubbling too.
    var earlyBubbling = false, heldUp: NSEvent?, releaseAfterBubbling = false
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
        // A down a view forwards to its superview's `mouseDown` (an inline atom's) is the same contact, said already.
        if event?.type == .leftMouseDown, presenter?.mouseChain.prevented !== event { presenter?.mouseChain.preventedContact = false }
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
        presenter.defaultPrevented = false
        presenter.propagating {
            for node in path where node.handlers.contains("pointerdown") && presenter.views[node.id] === node && !node.disabled {
                presenter.pointer(node.id, .down, node.pointerSample(event))
                if presenter.propagationStopped { break }
            }
        }
        // One that prevented it keeps the focus where it is (`MouseChain.prevented`).
        if presenter.defaultPrevented, let event {
            presenter.mouseChain.prevented = event
            if event.type == .leftMouseDown { presenter.mouseChain.preventedContact = true }
        }
        presenter.defaultPrevented = false
    }
    /// Says `MouseChain.keepingFocus` for the handling of the down `event`,
    /// which ends when the returned closure runs (the outer down's value
    /// again), on the chain it was said on, though the node is gone by then.
    func keepingFocus(for event: NSEvent) -> () -> Void {
        guard let chain = presenter?.mouseChain else { return {} }
        let outer = chain.keepingFocus
        chain.keepingFocus = chain.prevented === event
        return { chain.keepingFocus = outer }
    }
    /// `pointerdown` and `pointerup` around the tracking loop of a text
    /// area or an input's editor, which keeps the button's drags and up from
    /// the node, as a native button's loop does (NativeButtonsMac.swift). An
    /// input's field hands its down to its editor, which reports nothing
    /// again. NSWindow focuses the hit view before its `mouseDown`, so in a
    /// session's window the monitor has sent the down already, before the
    /// old field's `change` and `blur` and the new one's `focus`, as Chrome
    /// orders them (`Presenter.pressEditorFirst`; LLP 1005); here only the
    /// up is left. The press a nested event loop inside a handler tracks
    /// (an alert's, a menu's) is its own, and the outer one is kept.
    func tracksPointer(_ event: NSEvent, _ track: () -> Void) {
        guard let chain = presenter?.mouseChain, chain.tracked !== event else { return track() }
        let outer = chain.tracked
        chain.tracked = event
        defer { chain.tracked = outer }
        // Released already, its up gone: AppKit's tracking would wait for another, so none runs.
        if let index = chain.reported.firstIndex(where: { $0 === event }) { chain.reported.remove(at: index); return }
        if chain.early === event { chain.early = nil } else { pointerPressed(event) }
        track()
        pointerReleased(nil)
    }
    /// A press an editor does not track (disabled, or not selectable) goes
    /// the node's way, as a disabled box's does: its `mouseUp` reaches the
    /// node through the responder chain.
    func pressesUntracked(_ editor: NSView, _ event: NSEvent) -> Bool {
        let tracks = tracksPress(editor)
        if !tracks { mouseDown(with: event) }
        return !tracks
    }
    func tracksPress(_ editor: NSView) -> Bool {
        !disabled && ((editor as? NSTextView)?.isSelectable ?? true) && ((editor as? NSControl)?.isEnabled ?? true)
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
        presenter.defaultPrevented = false
        presenter.propagating {
            for id in path {
                guard let node = presenter.views[id], node.handlers.contains("pointerup") else { continue }
                presenter.pointer(id, .up, node.pointerSample(event, lifted: true))
                if presenter.propagationStopped { break }
            }
        }
        if presenter.defaultPrevented { presenter.mouseChain.upPrevented = event ?? NSApp.currentEvent }
        presenter.defaultPrevented = false
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
        // no hardware state. AppKit's first five bits are DOM's. An other
        // button made by `NSEvent.mouseEvent` says 0: the middle's.
        var buttons = NSEvent.pressedMouseButtons & 0x1f
        let other = max(e.map { Self.subtyped.contains($0.type) ? $0.buttonNumber : 2 } ?? 2, 2)
        switch e?.type {
        case .leftMouseDown?, .leftMouseDragged?: buttons |= 1
        case .rightMouseDown?, .rightMouseDragged?: buttons |= 2
        case .otherMouseDown?, .otherMouseDragged?: buttons |= Self.domButtons(other)
        case .mouseMoved?, .mouseEntered?: buttons = 0
        default: break
        }
        if lifted { buttons = 0 }
        let button: Int
        switch e?.type {
        case .leftMouseDown?, .leftMouseUp?: button = 0
        case .rightMouseDown?, .rightMouseUp?: button = 2
        case .otherMouseDown?, .otherMouseUp?: button = Self.domButton(other)
        default: button = -1
        }
        let pressure = lifted || buttons == 0 ? 0 : pen ? Double(e?.pressure ?? 0) : 0.5
        let client = e.flatMap { e in presenter.map { $0.client(Self.point(e, in: $0.viewport.window)) } } ?? .zero
        return PointerSample(x: Double(point.x - box.minX), y: Double(point.y - box.minY), buttons: buttons,
                             pressure: pressure, type: pen ? "pen" : "mouse", id: pen ? 2 : 1,
                             clientX: Double(client.x), clientY: Double(client.y), held: KeyCodes.held(e?.modifierFlags ?? []), button: button)
    }
    /// DOM's `MouseEvent.button` for AppKit's `buttonNumber` (UI Events):
    /// AppKit counts left, right, middle, back, forward; DOM primary,
    /// auxiliary, secondary, back, forward. A button past the fifth, which
    /// DOM does not name, is the auxiliary's, as it was before.
    static func domButton(_ number: Int) -> Int {
        switch number {
        case 0: return 0
        case 1: return 2
        case 3, 4: return number
        default: return 1
        }
    }
    /// DOM's `buttons` bit for AppKit's `buttonNumber`: 1, 2, 4, 8, 16.
    static func domButtons(_ number: Int) -> Int { [1, 4, 2, 8, 16][domButton(number)] }
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
    /// A press in a text area or an input reports its `pointerdown` here,
    /// from the window's monitor, before NSWindow gives the editor the focus
    /// (the old field's `change` and `blur`, the new one's `focus`), as
    /// Chrome orders them (LLP 1005); the editor's tracking reports the up
    /// (`NodeView.tracksPointer`). Only where AppKit gives the editor the
    /// press: the window's topmost view at the point (its toolbar's too, for
    /// a search field lent to it) is the editor, which tracks
    /// (`tracksPress`), in a window that takes the click (key, or the editor
    /// takes the first click), and not a control-click, AppKit's contextual
    /// click. Anything else goes its way as before.
    /// True when its `pointerdown` was prevented: the window does not get the
    /// down, so the editor takes no focus, no caret and no selection; the
    /// button's up releases it as any early press (LLP 1005 §3).
    @discardableResult func pressEditorFirst(_ event: NSEvent) -> Bool {
        guard event.type == .leftMouseDown else { return false }
        // A new press: an early one whose up nothing saw (a loop took it) is over; one still
        // bubbling is not (this press is a click in its handler's loop, and goes its way).
        if mouseChain.early != nil, !mouseChain.earlyBubbling { releaseEarlyPress(nil) }
        guard !event.modifierFlags.contains(.control), mouseChain.tracked == nil, !pointerHolds,
              let window = event.window, let frame = window.contentView?.superview ?? window.contentView,
              let hit = TableRowCell.whatIsThere({ frame.hitTest(frame.superview?.convert(event.locationInWindow, from: nil) ?? event.locationInWindow) }),
              let node = Self.editorOwner(hit), views[node.id] === node, node.tracksPress(hit),
              // A window under a sheet (an alert's too) or another's modal session takes no click.
              window.attachedSheet == nil, NSApp.modalWindow.map({ $0 === window }) ?? true,
              (NSApp.isActive && window.isKeyWindow) || hit.acceptsFirstMouse(for: event) else { return false }
        mouseChain.early = event
        mouseChain.earlyBubbling = true
        node.pointerPressed(event)
        mouseChain.earlyBubbling = false
        if let up = mouseChain.heldUp {
            mouseChain.heldUp = nil
            NSApp.postEvent(up, atStart: true) // for the editor's tracking, else the monitor's release
        }
        if mouseChain.releaseAfterBubbling { mouseChain.releaseAfterBubbling = false; releaseEarlyPress(nil) }
        return mouseChain.prevented === event
    }
    /// The node whose editor `hit` is, as each editor's `mouseDown` finds it
    /// (TextAreaMac.swift): a text area's, an input's field (the page's, or
    /// one lent to the toolbar or an alert) or that field's editor.
    static func editorOwner(_ hit: NSView) -> NodeView? {
        if let area = hit as? TextArea { return area.owner.flatMap { $0.textArea === area ? $0 : nil } }
        let field = (hit as? NSTextField) ?? ((hit as? NSTextView)?.delegate as? NSTextField)
        guard let field, let node = field.delegate as? NodeView, node.field === field else { return nil }
        return hit === field || hit === field.currentEditor() ? node : nil
    }
    /// The up of an early press no tracking took (AppKit kept a first click,
    /// a `pointerdown` handler took the editor away, or its window went):
    /// its `pointerup`, before anything else can take the up, or when the
    /// session's view leaves its window and the monitor with it. True for
    /// the press's up while its down still bubbles (`heldUp`): the monitor
    /// keeps it from the window, which has not had the down yet.
    @discardableResult func releaseEarlyPress(_ event: NSEvent?) -> Bool {
        guard let early = mouseChain.early else { return false }
        if mouseChain.earlyBubbling {
            // The first up is the press's; another is a later contact's, and goes its way.
            guard let event else { mouseChain.releaseAfterBubbling = true; return false }
            guard mouseChain.heldUp == nil else { return false }
            mouseChain.heldUp = event
            return true
        }
        mouseChain.early = nil
        mouseChain.reported = Array((mouseChain.reported + [early]).suffix(4))
        pointerSource?.pointerReleased(event)
        return false
    }
    /// A node still holds the button that went down: the held one, or an
    /// ancestor its down bubbled to, waiting for the up.
    var pointerHolds: Bool { pointerHeld != nil || !pointerPath.isEmpty }
    /// A window point from the viewport's top-left, the page scroll applied:
    /// DOM's `clientX`/`clientY`, `frame()`'s space (LLP 1094 D11).
    func client(_ windowPoint: NSPoint) -> NSPoint {
        let clip = viewport.contentView, p = clip.convert(windowPoint, from: nil)
        return NSPoint(x: p.x - clip.bounds.minX, y: p.y - clip.bounds.minY)
    }
    /// A viewport point, `client`'s answer, in `view`'s coordinates.
    func point(fromClient p: NSPoint, in view: NSView) -> NSPoint {
        let clip = viewport.contentView
        return view.convert(NSPoint(x: p.x + clip.bounds.minX, y: p.y + clip.bounds.minY), from: clip)
    }
    /// The viewport's size, what an inset's percentage is of.
    var clientSize: CGSize { viewport.contentView.bounds.size }
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
    /// A scroll moved content under a resting pointer: its hover follows once
    /// the scroll rests, `scrollRestInterval` after its last move, as a
    /// browser's does (WebKit fires its fake mouse move once per scroll, at its
    /// end: https://trac.webkit.org/r134150; Chromium keeps that timer). One a
    /// frame was a hit-test and a round trip to the runner for every row a
    /// fling carried under the pointer.
    static let scrollRestInterval: TimeInterval = 0.1
    func followPointerAfterScroll() {
        // A drive's frozen clock has no wall time to wait out: its hover follows at the frame.
        if ExactEnv.agentFreezes { followPointer(); return }
        scrollRestFollow?.cancel()
        let work = DispatchWorkItem { [weak self] in
            self?.scrollRestFollow = nil
            self?.followPointer()
        }
        scrollRestFollow = work
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.scrollRestInterval, execute: work)
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
        // The page, or a split's detail pane beside it (LLP 1115.002 D4).
        let pane = navigation.split.active ? navigation.split.controller?.detailPane : nil
        let ports = [viewport] + [pane].compactMap { $0 }
        guard let content = viewport.window?.contentView,
              ports.contains(where: { $0.bounds.contains($0.convert(point, from: nil)) }) else {
            setHoverPath([]); pointerCursor(over: nil, at: point); return nil
        }
        // What is under the pointer, a table's row included (the table takes its clicks, not its hover).
        let hit = TableRowCell.whatIsThere { content.hitTest(content.superview?.convert(point, from: nil) ?? point) }
        var path: [NodeView] = []
        var leaf: NodeView?
        var region = false
        var view = hit.flatMap { hit in ports.contains { hit.isDescendant(of: $0) } ? hit : nil }
        while let current = view {
            if let node = current as? NodeView, !node.inert {
                if leaf == nil { leaf = node }
                path.append(node)
            } else if leaf == nil, current is RegionSurfaceMac { region = true }
            // A subtree in the pane is its authored parent's, out in the page.
            view = pane != nil && current.superview === pane ? authoredSuperview(of: current) : current.superview
        }
        let run = leaf.flatMap { $0.inlineTarget(at: $0.local(point), handler: "hover") }
        setHoverPath(path, inline: run?.id)
        pointerCursor(over: leaf, at: point, region: region)
        return leaf
    }
    func trackPointer(_ event: NSEvent) {
        guard trackedPointerEvent !== event else { return }
        // A move in a popover's or a sheet's own window is that window's
        // (`BlockView.pointerTracking`); the page's hover is left as it is.
        let apart = event.window.map { $0 !== viewport.window && hosts($0) } ?? false
        // Rows sliding under a still pointer enter and exit their tracking areas: while a scroll
        // runs, its hover waits for the scroll to rest (`followPointerAfterScroll`), as a
        // browser's does, since each was a hit-test the next scrolled frame replaced. A move is
        // the person's and is heard at once, and a pointer leaving the content, or crossing into
        // a window in front of it (a popover), ends hover.
        if !apart, scrollRestFollow != nil, event.type == .mouseEntered || event.type == .mouseExited && restingPointer(frontmost: true) != nil { return }
        trackedPointerEvent = event
        trackedCanvas = nil
        // A held contact owns move delivery, but leaving the window still
        // ends hover. Other tracking areas may exit inside the window; hit
        // testing there keeps the subtree currently under the pointer.
        if event.type == .mouseExited { if !apart { _ = hoverAt(event.locationInWindow) } else { hoverLooks(from: event.window.flatMap { Self.node(at: event.locationInWindow, in: $0) }) }; return }
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
