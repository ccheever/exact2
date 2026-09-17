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
    private var samples: TransformDragVelocity
    private var last = CGPoint.zero
    private var queued = false
    private var pendingMove: (Double, Double, Double)?
    private var pendingEnd: (Double, Double, Double, Bool)?

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
        samples = TransformDragVelocity(value: origin.values, time: time)
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
            if let end { finish(dx: end.0, dy: end.1, time: end.2, cancel: end.3) }
            else if let move { _ = self.move(dx: move.0, dy: move.1, time: move.2) }
        }
    }
    @discardableResult func move(dx: Double, dy: Double, time: Double, ending: Bool = false) -> Bool {
        guard !ended, pendingEnd == nil, time.isFinite, time >= samples.time,
              let values = origin.moved(dx: dx, dy: dy), let session else { return false }
        if session.isApplyingPresentation || queued { pendingMove = (dx, dy, time); schedule(); return true }
        guard live && eligible else { if !finishing { cancel() }; return false }
        guard let packet = packet(12, values: values + [0, 0, 0]),
              let reply = session.runtime.transformMotion(packet) else { return false }
        session.apply(reply.batch)
        guard reply.accepted, reply.batch.error == nil, live, eligible,
              let displayed = target?.transformDragModel() else { return false }
        samples.record(value: displayed.values, time: time, ending: ending)
        last = CGPoint(x: dx, y: dy)
        return true
    }
    func finish(dx: Double, dy: Double, time: Double, cancel: Bool) {
        guard !ended, !finishing, pendingEnd == nil else { return }
        if session?.isApplyingPresentation == true || queued {
            pendingEnd = (dx, dy, time, cancel); schedule(); return
        }
        finishing = true
        let updated = !cancel && move(dx: dx, dy: dy, time: time, ending: true)
        guard let session else { ended = true; return }
        var released = false
        if updated, live, eligible, let packet = packet(13, values: samples.value + samples.velocity),
           let reply = session.runtime.transformMotion(packet) {
            session.apply(reply.batch)
            released = reply.accepted && reply.committed
        }
        ended = true
        if session.transformInputHold === self { session.transformInputHold = nil }
        if incarnationLive {
            // Action/batch delivery may have replaced either property. The
            // hasHold check and Rust token preflight leave successors untouched.
            for (token, vx, vy) in [(translate, samples.velocity[0], samples.velocity[1]), (scale, samples.velocity[2], 0)] {
                if session.runtime.hasHold(token) {
                    session.apply(session.runtime.holdEnd(token, cancel: !released, vx: vx, vy: vy, now: session.now()))
                }
            }
            session.presenter.collections.releaseInteractionLater(ifCurrent: pin)
        }
    }
    func cancel() { finish(dx: Double(last.x), dy: Double(last.y), time: samples.time, cancel: true) }
}
