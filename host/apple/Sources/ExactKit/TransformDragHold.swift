import Foundation

/// The original pair, handle and geometry incarnation are retained together.
/// Cleanup consumes only original survivors, including after an authored action
/// deletes the target or replaces one property. No native pin survives cleanup.
final class TransformDragHold {
    weak var session: ExactSession?
    weak var handle: NodeView?
    weak var target: NodeView?
    private weak var window: AnyObject?
    let binding: TransformDragBinding
    let generation: Int
    let sequence: UInt64
    let translate: UInt64
    let scale: UInt64
    let origin: TransformDragPosition
    private(set) var ended = false
    private var finishing = false
    private var pin: UInt64?
    /// The newest displayed pair [x, y, scale] and its time. Velocity is the
    /// engine's (LLP 1057.001 §3), measured over every value sent.
    private(set) var current: [Double]
    private var time: Double
    private var queued = false
    private var pendingMove: ([Double], Double)?
    private var pendingEnd: ([Double], Double, Bool)?

    init?(_ handle: NodeView, time: Double) {
        guard time.isFinite, let session = handle.presenter?.session, !session.isApplyingPresentation,
              let binding = session.presenter.transformBindings[handle.id],
              let targetID = binding.target, let targetKey = binding.targetKey, let clipKey = binding.clipKey,
              let target = session.presenter.views[targetID], SwipeInput.allows(handle), SwipeInput.allows(target),
              let geometry = session.presenter.transformGeometry.current(binding),
              let presentation = target.transformDragPresentation() else { return nil }
        self.session = session; self.handle = handle; self.target = target; self.binding = binding
        window = target.window
        generation = session.generation; sequence = geometry.sequence
        let packet = TransformDragPacket(op: 11, runtime: binding.runtime, handleKey: binding.handleKey,
            targetKey: targetKey, clipKey: clipKey, sequence: geometry.sequence,
            values: presentation.values + [0, 0, 0], now: session.now())
        guard let reply = session.runtime.transformMotion(packet) else { return nil }
        guard reply.accepted, reply.runtime == binding.runtime, reply.sequence == geometry.sequence,
              let translate = reply.translate, let scale = reply.scale, let origin = reply.value else {
            session.apply(reply.batch); return nil
        }
        self.translate = translate; self.scale = scale; self.origin = origin
        current = origin.values; self.time = time
        let previous = session.transformInputHold
        // Publish BOTH originals before any batch can invalidate/reenter us.
        session.transformInputHold = self
        session.apply(reply.batch)
        previous?.cancel()
        guard live && eligible else { cancel(); return nil }
        pin = session.presenter.collections.holdPointer(handle.id)
    }
    var incarnationLive: Bool {
        guard let session else { return false }
        return session.generation == generation && !session.runtime.destroyed
    }
    var live: Bool {
        !ended && incarnationLive && session?.runtime.hasHold(translate) == true && session?.runtime.hasHold(scale) == true
    }
    var eligible: Bool {
        guard incarnationLive, let session, let handle, let target else { return false }
        return session.presenter.views[handle.id] === handle && session.presenter.views[target.id] === target
            && session.presenter.transformBindings[handle.id] == binding
            && session.presenter.transformGeometry.current(binding)?.sequence == sequence
            && target.window === window && window != nil
            && SwipeInput.allows(handle) && SwipeInput.allows(target)
    }
    func cancelIfInputIneligible() { if !ended && (!eligible || !live) { cancel() } }
    func retire(runtime: UInt64, token: UInt64) {
        if runtime == binding.runtime && (token == translate || token == scale) { cancel() }
    }
    private func packet(_ op: UInt32, values: [Double]) -> TransformDragPacket? {
        guard let session, let target = binding.targetKey, let clip = binding.clipKey else { return nil }
        return TransformDragPacket(op: op, runtime: binding.runtime, handleKey: binding.handleKey,
            targetKey: target, clipKey: clip, sequence: sequence, translateToken: translate,
            scaleToken: scale, values: values, now: session.now())
    }
    private func schedule() {
        guard !queued else { return }
        queued = true
        DispatchQueue.main.async { [self] in
            queued = false
            if session?.isApplyingPresentation == true { schedule(); return }
            let end = pendingEnd, move = pendingMove
            pendingEnd = nil; pendingMove = nil
            if let end { finish(to: end.0, time: end.1, cancel: end.2) }
            else if let move { _ = self.move(to: move.0, time: move.1) }
        }
    }
    /// A pan by `dx, dy` from the pair's origin.
    @discardableResult func move(dx: Double, dy: Double, time: Double) -> Bool {
        guard let values = origin.moved(dx: dx, dy: dy) else { return false }
        return move(to: values, time: time)
    }
    /// Any pair [x, y, scale]: a pan, or a pinch anchored at its focal point.
    @discardableResult func move(to values: [Double], time: Double) -> Bool {
        guard !ended, pendingEnd == nil, time.isFinite, time >= self.time,
              TransformDragPosition(values) != nil, let session else { return false }
        if session.isApplyingPresentation || queued { pendingMove = (values, time); schedule(); return true }
        guard live && eligible else { if !finishing { cancel() }; return false }
        guard let packet = packet(12, values: values + [0, 0, 0]),
              let reply = session.runtime.transformMotion(packet) else { return false }
        session.apply(reply.batch)
        guard reply.accepted, reply.batch.error == nil, live, eligible,
              let displayed = target?.transformDragModel() else { return false }
        current = displayed.values; self.time = time
        return true
    }
    func finish(dx: Double, dy: Double, time: Double, cancel: Bool) {
        finish(to: origin.moved(dx: dx, dy: dy) ?? current, time: time, cancel: cancel)
    }
    /// The one release while both tokens are live (LLP 1002 D4's photo pair).
    func finish(to values: [Double], time: Double, cancel: Bool) {
        guard !ended, !finishing, pendingEnd == nil else { return }
        if session?.isApplyingPresentation == true || queued {
            pendingEnd = (values, time, cancel); schedule(); return
        }
        finishing = true
        let updated = !cancel && move(to: values, time: time)
        guard let session else { ended = true; return }
        var released = false, velocity = [0.0, 0.0, 0.0]
        if updated, live, eligible, let packet = packet(13, values: current + [0, 0, 0]),
           let reply = session.runtime.transformMotion(packet) {
            session.apply(reply.batch)
            released = reply.accepted && reply.committed
            velocity = reply.velocity ?? velocity
        }
        ended = true
        if session.transformInputHold === self { session.transformInputHold = nil }
        if incarnationLive {
            // Action/batch delivery may have replaced either property. The
            // hasHold check and Rust token preflight leave successors untouched.
            for (token, vx, vy) in [(translate, velocity[0], velocity[1]), (scale, velocity[2], 0)] {
                if session.runtime.hasHold(token) {
                    session.apply(session.runtime.holdEnd(token, cancel: !released, vx: vx, vy: vy, now: session.now()))
                }
            }
            session.presenter.collections.releaseInteractionLater(ifCurrent: pin)
        }
    }
    func cancel() { finish(to: current, time: time, cancel: true) }
}

extension TransformDragHold {
    /// The agent's `tap … pinch <scale>` where the platform offers no pinch to
    /// synthesize (LLP 1057.001 §5): the recognized scale, about `focal` (from
    /// the clip's centre, translate space), through the same paired hold and
    /// its one release as a finger's. Delivery `recognized`; nil when refused.
    static func recognizedPinch(_ handle: NodeView, scale: Double, focal: CGPoint) -> String? {
        guard scale.isFinite, scale > 0 else { return "pinch: expected a positive finite scale" }
        let start = ProcessInfo.processInfo.systemUptime
        guard let hold = TransformDragHold(handle, time: start), let origin = TransformDragPosition(hold.current) else {
            return "the photo binding on view \(handle.id) refused to begin (geometry not current, or disabled)"
        }
        for step in 1...8 {
            let factor = 1 + (scale - 1) * Double(step) / 8
            guard let values = origin.focused(from: focal, to: focal, factor: factor)?.values,
                  hold.move(to: values, time: start + Double(step) / 60) else { hold.cancel(); return "the pinch was cancelled" }
        }
        hold.finish(to: hold.current, time: start + 9 / 60, cancel: false)
        return nil
    }
}
