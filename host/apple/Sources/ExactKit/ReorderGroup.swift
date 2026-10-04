// Dropping across lists on the Apple hosts (LLP 1094 D5–D9). A grip whose
// list shares a `reorderGroup` lifts as a ghost in the window's top layer —
// a snapshot of its row, which the runner then hides — and the ghost follows
// the contact by the grip's offset. The ghost's centre picks the target: the
// grouped list whose port holds it; its content y goes to Rust
// (`exact_reorder_move_into`). Outside every port nothing is sent, so the last
// certified gap stands. The lists, then their scroll ancestors, scroll when
// the centre is near an edge. A drop may hold until its move shows; then the
// ghost springs onto the row (or fades when the row went) and the session
// finishes. A key's or custom action's session has no ghost.
import CExact
import Foundation
import QuartzCore
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// The runtime's grouped calls (`exact.h`); a test substitutes its own.
protocol ReorderGroupCalls: AnyObject {
    func groupBegin(_ handle: UInt32, scrollTop: Double, ghost: Bool, now: Double) -> Batch
    func moveInto(_ token: UInt64, target: UInt32, contentY: Double, scrollTop: Double, inside: Bool, now: Double) -> Batch
    func groupStep(_ token: UInt64, step: ReorderGroupStep, now: Double) -> Batch
    func groupEnd(_ token: UInt64, drop: Bool, now: Double) -> Batch
    func groupFinish(_ token: UInt64, now: Double) -> Batch
}
extension Runtime: ReorderGroupCalls {
    func groupBegin(_ handle: UInt32, scrollTop: Double, ghost: Bool, now: Double) -> Batch {
        on { read(exact_reorder_group_begin(rt, handle, scrollTop, ghost ? 1 : 0, now)) }
    }
    func moveInto(_ token: UInt64, target: UInt32, contentY: Double, scrollTop: Double, inside: Bool, now: Double) -> Batch {
        on { read(exact_reorder_move_into(rt, token, target, contentY, scrollTop, inside ? 1 : 0, now)) }
    }
    func groupStep(_ token: UInt64, step: ReorderGroupStep, now: Double) -> Batch {
        on { read(exact_reorder_step(rt, token, step.rawValue, now)) }
    }
    func groupEnd(_ token: UInt64, drop: Bool, now: Double) -> Batch {
        on { read(exact_reorder_group_end(rt, token, drop ? 1 : 0, now)) }
    }
    func groupFinish(_ token: UInt64, now: Double) -> Batch {
        on { read(exact_reorder_group_finish(rt, token, now)) }
    }
}

/// D9's moves: the gap a row up or down, or the previous or next list.
enum ReorderGroupStep: UInt32 {
    case earlier = 1, later, previousList, nextList
    /// The arrows, by DOM's names.
    init?(key: String) {
        switch key {
        case "ArrowUp": self = .earlier
        case "ArrowDown": self = .later
        case "ArrowLeft": self = .previousList
        case "ArrowRight": self = .nextList
        default: return nil
        }
    }
    /// The custom actions' names (D9), in their order.
    static let actions: [(String, ReorderGroupStep)] = [
        ("Move earlier", .earlier), ("Move later", .later),
        ("Move to previous list", .previousList), ("Move to next list", .nextList),
    ]
}

/// One grouped `{"op":"reorder","group":true}`.
struct ReorderGroupState: Equatable {
    let token: UInt64
    let list: UInt32
    let wrapper: UInt32
    /// active, holding, cancelling, settling, finished or refused.
    let phase: String
    let ending: String?
    let target: UInt32
    /// The wrapper that holds the dragged row now: where a ghost lands.
    let row: UInt32
    let dispatched: Bool
    init?(_ op: [String: Any]) {
        guard op["group"] as? Bool == true, let raw = op["token"] as? String, let token = UInt64(raw),
              let phase = op["phase"] as? String else { return nil }
        let id = { (name: String) in UInt32(clamping: op[name] as? Int ?? 0) }
        self.token = token; self.phase = phase
        list = id("list"); wrapper = id("wrapper"); target = id("target"); row = id("row")
        ending = op["ending"] as? String
        dispatched = op["dispatched"] as? Bool ?? false
    }
    static func last(in batch: Batch) -> ReorderGroupState? {
        batch.ops.last { $0.op == .reorder && $0.payload["group"] as? Bool == true }.flatMap { ReorderGroupState($0.payload) }
    }
}

extension CollectionHost {
    /// The row wrapper at or above `descendant`: what a ghost snapshots.
    func wrapper(of descendant: UInt32) -> NodeView? {
        guard let presenter, var node: NSObject = presenter.views[descendant] else { return nil }
        while true {
            if let view = node as? NodeView, presenter.views[view.id] === view,
               entries.values.contains(where: { $0.snapshot.rows.contains { $0.view == view.id } }) { return view }
            #if os(macOS)
            guard let parent = (node as? NSView)?.superview else { return nil }
            #else
            guard let parent = (node as? UIView)?.superview else { return nil }
            #endif
            node = parent
        }
    }
}

extension NodeView {
    /// The grouped list this grip's row is in, and its group (D1).
    var reorderGroupList: (list: UInt32, group: String)? {
        guard !(props["reorderFor"] ?? "").isEmpty, let presenter,
              let list = presenter.collections.owningCollection(id),
              let group = presenter.views[list]?.props["reorderGroup"], !group.isEmpty else { return nil }
        return (list, group)
    }
    /// A grouped grip with no `press`, `key`, `pan` or `pointerdown` of its
    /// own takes the keys (D9).
    var reorderKeys: Bool {
        handlers.isDisjoint(with: ["press", "key", "pan", "pointerdown"]) && reorderGroupList != nil
    }
}

final class ReorderGroupHold {
    weak var presenter: Presenter?
    weak var handle: NodeView?
    let calls: ReorderGroupCalls
    let group: String
    private let generation: Int?
    private(set) var state: ReorderGroupState
    private let pin: UInt64
    /// The ghost, in the window's top layer, and where in it the contact is.
    private(set) var ghost: ReorderGhost?
    private var grab = CGPoint.zero
    private(set) var point = CGPoint.zero
    private var edgeTimer: Timer?
    private var edgeTime: Double?
    private var landing = false

    /// Pin the grip, snapshot its row (before the runner hides it), and lift.
    init?(_ handle: NodeView, point: CGPoint, ghost drawn: Bool) {
        guard let presenter = handle.presenter, SwipeInput.allows(handle),
              presenter.reorderGroup == nil, presenter.reorder == nil,
              presenter.session?.isApplyingPresentation != true,
              let (list, group) = handle.reorderGroupList,
              let calls = presenter.reorderGroupCalls ?? presenter.session?.runtime else { return nil }
        self.presenter = presenter; self.handle = handle; self.calls = calls; self.group = group
        generation = presenter.session?.generation
        pin = presenter.collections.holdPointer(handle.id)
        guard let top = presenter.collections.geometry(list)?.offset else {
            presenter.collections.releaseInteractionLater(ifCurrent: pin); return nil
        }
        let wrapper = presenter.collections.wrapper(of: handle.id)
        let ghost = drawn ? wrapper.flatMap { ReorderGhost(of: $0) } : nil
        let batch = calls.groupBegin(handle.id, scrollTop: top, ghost: drawn, now: presenter.session?.now() ?? CACurrentMediaTime() * 1000)
        state = ReorderGroupState.last(in: batch) ?? ReorderGroupState(["group": true, "token": "0", "phase": "refused"])!
        self.point = point
        guard state.phase == "active" else {
            Self.apply(batch, presenter)
            presenter.collections.releaseInteractionLater(ifCurrent: pin); return nil
        }
        presenter.reorderGroup = self
        if let ghost {
            self.ghost = ghost
            ghost.show()
            grab = CGPoint(x: point.x - ghost.frame.minX, y: point.y - ghost.frame.minY)
            #if os(iOS)
            UIImpactFeedbackGenerator(style: .light).impactOccurred()
            #endif
        }
        Self.apply(batch, presenter)
    }

    private static func apply(_ batch: Batch, _ presenter: Presenter) {
        if let session = presenter.session { session.apply(batch); return }
        for op in batch.ops where op.op == .reorder { presenter.reorderGroup?.observe(ReorderGroupState(op.payload)) }
    }
    private var now: Double { presenter?.session?.now() ?? CACurrentMediaTime() * 1000 }
    private var live: Bool {
        guard let presenter, presenter.reorderGroup === self else { return false }
        guard let generation else { return true }
        return presenter.session?.generation == generation && presenter.session?.runtime.destroyed == false
    }
    var active: Bool { state.phase == "active" }

    /// The contact moved to `point` (window coordinates): the ghost follows
    /// by the grip's offset, and its centre picks the target and the gap.
    @discardableResult func move(_ point: CGPoint) -> Bool {
        guard active, live, let presenter else { return false }
        if presenter.session?.isApplyingPresentation == true {
            DispatchQueue.main.async { [weak self] in _ = self?.move(point) }
            return true
        }
        self.point = point
        ghost?.place(CGPoint(x: point.x - grab.x, y: point.y - grab.y))
        sample()
        updateEdge()
        return active
    }

    /// The ghost's centre, or the contact's point without one.
    var centre: CGPoint {
        guard let ghost else { return point }
        return CGPoint(x: ghost.frame.midX, y: ghost.frame.midY)
    }

    /// The grouped list whose port holds the centre (D7), with its facts.
    private func target() -> (list: UInt32, port: ReorderPort)? {
        guard let presenter else { return nil }
        for (id, node) in presenter.views where node.props["reorderGroup"] == group {
            if let port = ReorderPort(list: node, presenter: presenter), port.rect.contains(centre) { return (id, port) }
        }
        return nil
    }

    private func sample() {
        guard let presenter, let (list, port) = target() else { return }
        let batch = calls.moveInto(state.token, target: list, contentY: port.contentY(centre),
            scrollTop: port.scrollTop, inside: true, now: now)
        Self.apply(batch, presenter)
        observe(ReorderGroupState.last(in: batch))
    }

    /// A key's or custom action's move (D9).
    func step(_ step: ReorderGroupStep) {
        guard active, live, let presenter else { return }
        let batch = calls.groupStep(state.token, step: step, now: now)
        Self.apply(batch, presenter)
        observe(ReorderGroupState.last(in: batch))
    }

    /// The contact ended (D8): a drop into the current target, or a cancel.
    /// A drop that holds keeps the ghost where it is until the move shows.
    func finish(cancel: Bool) {
        guard active, let presenter else { return }
        stopEdge()
        if !cancel { sample() }
        let batch = calls.groupEnd(state.token, drop: !cancel && live, now: now)
        Self.apply(batch, presenter)
        observe(ReorderGroupState.last(in: batch))
        if active { end() }
    }
    /// Escape, a lost contact, the window resigning: before a drop only.
    func cancel() { if active { finish(cancel: true) } }

    /// This session's op (D8): a hold keeps the ghost; the end of one, or a
    /// cancel, lands it; `finished` retires everything.
    func observe(_ next: ReorderGroupState?) {
        guard let next, next.token == state.token, state.phase != "finished" else { return }
        state = next
        switch next.phase {
        case "holding": stopEdge()
        case "settling", "cancelling": stopEdge(); land()
        case "finished", "refused": end()
        default: break
        }
    }

    /// The ghost springs onto the row wherever it is (landed, timeout, home)
    /// or fades when the row went; then the session finishes and the row shows.
    private func land() {
        guard !landing else { return }
        landing = true
        guard let ghost, let presenter else { finishSession(); return }
        let row = state.ending == "gone" ? nil : presenter.views[state.row]
        ghost.land(on: row) { [weak self] in self?.finishSession() }
    }

    private func finishSession() {
        guard let presenter, state.phase != "finished" else { end(); return }
        let landedHere = state.ending == "landed" || state.ending == "timeout"
        let row = state.row
        let batch = calls.groupFinish(state.token, now: now)
        Self.apply(batch, presenter)
        observe(ReorderGroupState.last(in: batch))
        if state.phase != "finished" { end() }
        // Focus follows the moved row's grip when it landed, the source's otherwise (D9).
        focusAfter(row: landedHere ? row : nil)
    }

    /// The session restarted: no Rust calls.
    func abandon() { end() }

    private func end() {
        stopEdge()
        ghost?.remove(); ghost = nil
        if state.phase != "finished" && state.phase != "refused" {
            state = ReorderGroupState(["group": true, "token": String(state.token), "phase": "finished"])!
        }
        presenter?.collections.releaseInteractionLater(ifCurrent: pin)
        if presenter?.reorderGroup === self { presenter?.reorderGroup = nil }
    }

    private func focusAfter(row: UInt32?) {
        guard let presenter else { return }
        let grip: NodeView? = row.flatMap { presenter.views[$0] }.flatMap { ReorderGroupHold.grip(in: $0) } ?? handle
        grip?.focusAfterReorder()
    }
    private static func grip(in view: NodeView) -> NodeView? {
        if !(view.props["reorderFor"] ?? "").isEmpty { return view }
        for case let child as NodeView in view.subviews { if let found = grip(in: child) { return found } }
        for child in view.subviews where !(child is NodeView) {
            for case let inner as NodeView in child.subviews { if let found = grip(in: inner) { return found } }
        }
        return nil
    }

    // Autoscroll (D7): the target list's port on its own axis, then each
    // scroll ancestor of it whose port holds the centre; the innermost that
    // can still move toward the edge it is near scrolls, at the web's band
    // and speed (`ReorderEdge`); after a scroll the gap is sampled again.
    private func edgeCandidates() -> [ReorderScroller] {
        guard let presenter else { return [] }
        let list = target()?.list ?? state.target
        guard let node = presenter.views[list] else { return [] }
        return ReorderScroller.chain(from: node, holding: centre)
    }
    private func updateEdge() {
        guard active, edgeCandidates().contains(where: { $0.direction(centre) != 0 && $0.canScroll(toward: $0.direction(centre)) }) else { stopEdge(); return }
        guard edgeTimer == nil else { return }
        edgeTime = nil
        let timer = Timer(timeInterval: 1.0 / 60, repeats: true) { [weak self] _ in self?.edgeTick() }
        RunLoop.main.add(timer, forMode: .common)
        edgeTimer = timer
    }
    private func edgeTick() {
        guard active, live else { stopEdge(); return }
        let time = CACurrentMediaTime()
        let dt = edgeTime.map { time - $0 } ?? 0
        edgeTime = time
        guard dt > 0 else { return }
        let centre = centre
        guard let scroller = edgeCandidates().first(where: { $0.direction(centre) != 0 && $0.canScroll(toward: $0.direction(centre)) }) else { stopEdge(); return }
        let direction = scroller.direction(centre)
        if scroller.scroll(by: ReorderEdge.step(direction: direction, dt: dt)) { sample() } else { stopEdge() }
    }
    private func stopEdge() { edgeTimer?.invalidate(); edgeTimer = nil; edgeTime = nil }
}

extension NodeView {
    /// D9's keys on a grouped grip: Space lifts (no ghost); then the arrows
    /// step, Space or Enter drops and Escape cancels. True when taken.
    func reorderKey(_ name: String) -> Bool {
        guard reorderKeys, let presenter else { return false }
        if let hold = presenter.reorderGroup {
            guard hold.handle === self else { return false }
            if let step = ReorderGroupStep(key: name) { hold.step(step); return true }
            switch name {
            case " ", "Enter": hold.finish(cancel: false); return true
            case "Escape": hold.cancel(); return true
            default: return hold.active
            }
        }
        guard name == " " else { return false }
        return ReorderGroupHold(self, point: .zero, ghost: false) != nil
    }

    /// One custom action (D9): begin, one step and the drop, in one turn;
    /// the hold still runs, and the gap closes in place when it ends.
    func reorderAction(_ step: ReorderGroupStep) -> Bool {
        guard reorderGroupList != nil, let hold = ReorderGroupHold(self, point: .zero, ghost: false) else { return false }
        hold.step(step)
        hold.finish(cancel: false)
        return true
    }
}
