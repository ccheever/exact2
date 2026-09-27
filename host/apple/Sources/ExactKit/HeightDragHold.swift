import Foundation

/// A single header/target/token incarnation. Neither recognition nor snap state
/// lives here: the platform recognizes, and the authored release action snaps.
final class HeightDragHold {
    weak var session: ExactSession?
    weak var header: NodeView?
    weak var target: NodeView?
    let binding: HeightDragBinding
    let generation: Int
    let primary: NativeHold
    let mapping: HeightDragPosition
    private(set) var ended = false
    private var finishing = false
    private var pin: UInt64?
    private var lastDisplacement = 0.0
    /// The newest shown height and its time; the release velocity is the
    /// engine's, over the heights shown (LLP 1057.001 §3).
    private var shown: Double
    private var time: Double
    private var queued = false
    private var pendingMove: (Double, Double)?
    private var pendingEnd: (Double, Double, Bool)?

    init?(_ header: NodeView, time: Double) {
        guard time.isFinite, let session = header.presenter?.session,
              !session.isApplyingPresentation,
              let binding = session.presenter.heightBindings[header.id],
              let targetID = binding.target, let targetKey = binding.targetKey, let target = session.presenter.views[targetID],
              SwipeInput.allows(header), SwipeInput.allows(target) else { return nil }
        self.session = session; self.header = header; self.target = target
        self.binding = binding; generation = session.generation
        let (hold, batch) = session.runtime.heightDragBegin(binding.handleKey, targetKey: targetKey, now: session.now())
        guard let hold, let mapping = HeightDragPosition(base: hold.x) else {
            session.apply(batch); return nil
        }
        primary = hold; self.mapping = mapping
        shown = hold.x; self.time = time
        let previous = session.heightInputHold
        session.heightInputHold = self
        session.apply(batch)
        previous?.cancel()
        guard live && eligible else { cancel(); return nil }
        pin = session.presenter.collections.holdPointer(header.id)
    }
    var incarnationLive: Bool {
        guard let session else { return false }
        return session.generation == generation && !session.runtime.destroyed
    }
    var live: Bool { !ended && incarnationLive && session?.runtime.hasHold(primary.token) == true }
    var eligible: Bool {
        guard incarnationLive, let session, let header, let target else { return false }
        return session.presenter.views[header.id] === header
            && session.presenter.views[target.id] === target
            && session.presenter.heightBindings[header.id] == binding
            && SwipeInput.allows(header) && SwipeInput.allows(target)
    }
    func cancelIfInputIneligible() { if !ended && (!eligible || !live) { cancel() } }

    private func schedule() {
        guard !queued else { return }
        queued = true
        DispatchQueue.main.async { [self] in
            queued = false
            if session?.isApplyingPresentation == true { schedule(); return }
            let end = pendingEnd, move = pendingMove
            pendingEnd = nil; pendingMove = nil
            if let end { finish(downward: end.0, time: end.1, cancel: end.2) }
            else if let move { _ = self.move(downward: move.0, time: move.1) }
        }
    }
    @discardableResult func move(downward: Double, time: Double) -> Bool {
        guard !ended, pendingEnd == nil, time.isFinite, time >= self.time,
              let height = mapping.value(downward: downward), let session else { return false }
        if session.isApplyingPresentation || queued {
            pendingMove = (downward, time); schedule(); return true
        }
        guard live && eligible else { if !finishing { cancel() }; return false }
        let batch = session.runtime.heightDragUpdate(primary.token, height: height, now: session.now())
        session.apply(batch)
        guard batch.error == nil, live, eligible, let target else { return false }
        shown = Double(target.bounds.height); self.time = time
        lastDisplacement = downward
        return true
    }
    func finish(downward: Double, time: Double, cancel: Bool) {
        guard !ended, !finishing, pendingEnd == nil else { return }
        if session?.isApplyingPresentation == true || queued {
            pendingEnd = (downward, time, cancel); schedule(); return
        }
        finishing = true
        let updated = !cancel && move(downward: downward, time: time)
        guard let session else { ended = true; return }
        if updated, live, eligible {
            session.apply(session.runtime.heightDragRelease(primary.token, height: shown, now: session.now()))
        }
        ended = true
        if session.heightInputHold === self { session.heightInputHold = nil }
        if incarnationLive {
            session.apply(session.runtime.holdEnd(primary.token, cancel: cancel || !updated, measured: true, now: session.now()))
            session.presenter.collections.releaseInteractionLater(ifCurrent: pin)
        }
    }
    func cancel() { finish(downward: lastDisplacement, time: time, cancel: true) }
}
