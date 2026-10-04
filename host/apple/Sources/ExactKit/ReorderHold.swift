// Arrange (`reorderFor` / `reorderdrop`, LLP 1041 §8.5) on the Apple hosts.
// The platform recognizes the contact (a handle drag or long press on iOS, a
// mouse drag on macOS); Rust holds the lifted row and asks the collection for
// the gap; this maps the pointer, keeps the handle's interaction pin until the
// source has settled, raises the row and scrolls the List at its edges.
import Foundation
import QuartzCore

/// The runtime's three Arrange calls (`exact.h`); a test substitutes its own.
protocol ReorderCalls: AnyObject {
    func reorderBegin(_ handle: UInt32, scrollTop: Double, now: Double) -> Batch
    func reorderMove(_ token: UInt64, dy: Double, scrollTop: Double, inside: Bool, now: Double) -> Batch
    func reorderEnd(_ token: UInt64, drop: Bool, dy: Double, scrollTop: Double, inside: Bool, velocity: Double, now: Double) -> Batch
}
extension Runtime: ReorderCalls {}

/// One `{"op":"reorder"}`: the contact's serial, its List and lifted wrapper.
struct ReorderState: Equatable {
    let token: UInt64
    let list: UInt32
    let wrapper: UInt32
    let phase: String
    let dispatched: Bool
    init?(_ op: [String: Any]) {
        guard let raw = op["token"] as? String, let token = UInt64(raw),
              let list = op["list"] as? Int, let wrapper = op["wrapper"] as? Int,
              let phase = op["phase"] as? String else { return nil }
        self.token = token; self.list = UInt32(clamping: list); self.wrapper = UInt32(clamping: wrapper)
        self.phase = phase; self.dispatched = op["dispatched"] as? Bool ?? false
    }
    static func last(in batch: Batch) -> ReorderState? {
        batch.ops.last { $0.op == .reorder }.flatMap { ReorderState($0.payload) }
    }
}

/// The web host's edge scroll: 32-point bands inside the port's top and
/// bottom (and past them), 720 points a second, at most 32 ms of catch-up.
enum ReorderEdge {
    static func direction(offset: Double, height: Double) -> Double {
        guard offset.isFinite, height.isFinite else { return 0 }
        return offset < 32 ? -1 : offset > height - 32 ? 1 : 0
    }
    static func step(direction: Double, dt: Double) -> Double {
        direction * 720 * min(0.032, max(0, dt))
    }
}

/// The source's release speed from its last two samples (points/second), as
/// the web's; a stationary end sample reads as rest.
struct ReorderVelocity {
    private var samples: [(time: Double, y: Double)] = []
    mutating func record(time: Double, y: Double) {
        guard time.isFinite, y.isFinite, samples.last.map({ time >= $0.time }) ?? true else { return }
        samples.append((time, y))
        if samples.count > 2 { samples.removeFirst(samples.count - 2) }
    }
    var value: Double {
        guard samples.count == 2 else { return 0 }
        let dt = samples[1].time - samples[0].time
        let v = dt > 0 ? (samples[1].y - samples[0].y) / dt : 0
        return v.isFinite ? v : 0
    }
}

final class ReorderHold {
    enum Phase { case active, settling, finished }
    weak var presenter: Presenter?
    weak var handle: NodeView?
    private weak var lifted: NodeView?
    let calls: ReorderCalls
    private let generation: Int?
    private(set) var state: ReorderState
    private(set) var phase = Phase.active
    private let pin: UInt64
    private let scroll0: Double
    private var dy = 0.0
    private(set) var point = CGPoint.zero
    private var velocity = ReorderVelocity()
    private var edgeTimer: Timer?
    private var edgeTime: Double?

    /// Pin the handle, then ask Rust to catch its row at the List's actual offset.
    init?(_ handle: NodeView, point: CGPoint, time: Double) {
        guard let presenter = handle.presenter, SwipeInput.allows(handle),
              !(handle.props["reorderFor"] ?? "").isEmpty,
              presenter.session?.isApplyingPresentation != true,
              let calls = presenter.reorderCalls ?? presenter.session?.runtime,
              let list = presenter.collections.owningCollection(handle.id) else { return nil }
        self.presenter = presenter; self.handle = handle; self.calls = calls
        generation = presenter.session?.generation
        // The pin reaches the runner (synchronous feedback) before the catch.
        pin = presenter.collections.holdPointer(handle.id)
        guard let top = presenter.collections.geometry(list)?.offset else {
            presenter.collections.releaseInteractionLater(ifCurrent: pin); return nil
        }
        let batch = calls.reorderBegin(handle.id, scrollTop: top, now: presenter.session?.now() ?? time * 1000)
        state = ReorderState.last(in: batch) ?? ReorderState(["token": "0", "list": 0, "wrapper": 0, "phase": "refused"])!
        scroll0 = top; self.point = point
        Self.apply(batch, presenter)
        guard state.phase == "active", state.list == list else {
            presenter.collections.releaseInteractionLater(ifCurrent: pin); return nil
        }
        presenter.reorder?.abandon()
        presenter.reorder = self
        velocity.record(time: time, y: 0)
        raise(true)
    }

    private static func apply(_ batch: Batch, _ presenter: Presenter) {
        if let session = presenter.session { session.apply(batch); return }
        // Sessionless (a test's calls): only the Arrange state is routed.
        for op in batch.ops where op.op == .reorder { presenter.reorder?.observe(ReorderState(op.payload)) }
    }
    private var now: Double { presenter?.session?.now() ?? CACurrentMediaTime() * 1000 }
    private var live: Bool {
        guard let presenter, presenter.reorder === self else { return false }
        guard let generation else { return true }
        return presenter.session?.generation == generation && presenter.session?.runtime.destroyed == false
    }

    /// One pointer sample: `dy` is its downward travel since recognition,
    /// `point` the pointer in window coordinates. False once no longer active.
    @discardableResult func move(dy: Double, point: CGPoint, time: Double) -> Bool {
        guard phase == .active, live, let presenter, dy.isFinite else { return false }
        if presenter.session?.isApplyingPresentation == true {
            DispatchQueue.main.async { [weak self] in _ = self?.move(dy: dy, point: point, time: time) }
            return true
        }
        self.dy = dy; self.point = point
        guard let port = portFacts() else { cancel(); return false }
        velocity.record(time: time, y: dy + port.top - scroll0)
        let batch = calls.reorderMove(state.token, dy: dy, scrollTop: port.top, inside: port.inside, now: now)
        Self.apply(batch, presenter)
        observe(ReorderState.last(in: batch))
        if batch.error != nil { cancel() }
        guard phase == .active else { return false }
        raise(true)
        updateEdge()
        return true
    }

    /// The contact ended: a drop at this sample, or a cancel.
    func finish(dy: Double, point: CGPoint, time: Double, cancel: Bool) {
        guard phase == .active, let presenter else { return }
        stopEdge()
        let port = live ? portFacts() : nil
        let drop = !cancel && port != nil && dy.isFinite
        if drop, let port { velocity.record(time: time, y: dy + port.top - scroll0) }
        let batch = calls.reorderEnd(state.token, drop: drop, dy: drop ? dy : 0, scrollTop: port?.top ?? scroll0,
            inside: port?.inside ?? false, velocity: drop ? velocity.value : 0, now: now)
        Self.apply(batch, presenter)
        observe(ReorderState.last(in: batch))
        if phase == .active, drop { self.cancel() }
        // Whatever Rust answers, this contact is over: no answer leaves it active.
        if phase == .active { finished() }
    }
    func cancel() { finish(dy: dy, point: point, time: 0, cancel: true) }

    /// A `reorder` op for this contact: settling (a terminal, or a receipt
    /// that ended it) or finished (the source settled; the pin retires).
    func observe(_ next: ReorderState?) {
        guard let next, next.token == state.token, phase != .finished else { return }
        state = next
        switch next.phase {
        case "settling": phase = .settling; stopEdge()
        case "finished", "refused": finished()
        default: break
        }
    }

    /// The session restarted or a successor took the owner: no Rust calls.
    func abandon() {
        guard phase != .finished else { return }
        phase = .finished; stopEdge(); raise(false)
        if presenter?.reorder === self { presenter?.reorder = nil }
    }

    private func finished() {
        phase = .finished; stopEdge(); raise(false)
        presenter?.collections.releaseInteractionLater(ifCurrent: pin)
        if presenter?.reorder === self { presenter?.reorder = nil }
    }

    /// Whether `view` is this contact's lifted row (until it finishes).
    func lifts(_ view: UInt32) -> Bool { phase != .finished && state.wrapper == view }

    /// Keep the lift through mounting and changes to its siblings.
    func raiseLifted() { if phase != .finished { raise(true) } }

    /// The lifted row paints above its later siblings while it is held.
    private func raise(_ on: Bool) {
        let view: NodeView?
        if on {
            view = presenter?.views[state.wrapper]
            if lifted !== view { lifted?.setLifted(false); lifted = view }
        } else {
            // A finished receipt may name wrapper 0, or the old row may
            // already be a ghost outside the presenter's lookup map.
            view = lifted
            lifted = nil
        }
        guard let view else { return }
        view.setLifted(on)
        #if os(macOS)
        if !on || view.arrangeShift != view.translate { view.applyTransform() }
        #endif
    }

    private func updateEdge() {
        guard phase == .active, let port = portFacts(),
              ReorderEdge.direction(offset: port.offset, height: port.height) != 0 else { stopEdge(); return }
        guard edgeTimer == nil else { return }
        edgeTime = nil
        let timer = Timer(timeInterval: 1.0 / 60, repeats: true) { [weak self] _ in self?.edgeTick() }
        RunLoop.main.add(timer, forMode: .common)
        edgeTimer = timer
    }
    private func edgeTick() {
        guard phase == .active, live, let port = portFacts() else { stopEdge(); return }
        let direction = ReorderEdge.direction(offset: port.offset, height: port.height)
        guard direction != 0 else { stopEdge(); return }
        let time = CACurrentMediaTime()
        let dt = edgeTime.map { time - $0 } ?? 0
        edgeTime = time
        guard dt > 0 else { return }
        if scrollList(by: ReorderEdge.step(direction: direction, dt: dt)) { move(dy: dy, point: point, time: time) } else { stopEdge() }
    }
    private func stopEdge() { edgeTimer?.invalidate(); edgeTimer = nil; edgeTime = nil }
}
