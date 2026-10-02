import Foundation

/// One bounded observation per authored handle. No timer or frame loop: native
/// layout/scroll and presentation batches schedule a coalesced UI turn. Facts
/// exclude the target's own Translate/Scale so panning cannot feed back itself.
final class TransformGeometryHost {
    struct Observation {
        let binding: TransformDragBinding
        let facts: TransformGeometryFacts?
        let sequence: UInt64
        var accepted: Bool
    }
    weak var presenter: Presenter?
    private var observations: [UInt32: Observation] = [:]
    private var serial: UInt64 = 0
    private var queued = false
    private var delivering = false
    private var remainingPasses = 0
    init(_ presenter: Presenter) { self.presenter = presenter }
    func reset() { observations.removeAll(); serial = 0; remainingPasses = delivering ? 2 : 0 }
    func retire(_ id: UInt32) { observations.removeValue(forKey: id) }
    /// Moves on every native layout, scroll, window move and presentation
    /// batch — what may have moved a view — so a reader can keep a geometric
    /// answer until it does (a canvas's on-screen test, `Canvases.shown`).
    private(set) var epoch = 0
    func changed() {
        epoch &+= 1
        guard !delivering, presenter?.transformBindings.isEmpty == false else { return }
        remainingPasses = 2
        enqueue()
    }
    private func enqueue() {
        guard !queued, remainingPasses > 0 else { return }
        queued = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            queued = false
            guard presenter?.session?.isApplyingPresentation != true else { enqueue(); return }
            refresh()
        }
    }
    func current(_ binding: TransformDragBinding) -> Observation? {
        guard let observation = observations[binding.id], observation.accepted,
              observation.binding == binding, observation.facts?.ready == true,
              let facts = presenter?.transformFacts(binding), facts == observation.facts else { return nil }
        return observation
    }
    func refresh() {
        guard let presenter, let session = presenter.session, !session.isApplyingPresentation,
              !session.runtime.destroyed, remainingPasses > 0 else { return }
        remainingPasses -= 1
        var refine = false
        observations = observations.filter { presenter.transformBindings[$0.key] != nil }
        // Copy the small authored binding set because a geometry action may
        // replace/remove any of these handles while this turn is delivered.
        let bindings = presenter.transformBindings
        for binding in bindings.values.sorted(by: { $0.id < $1.id }) {
            guard presenter.transformBindings[binding.id] == binding,
                  let target = binding.targetKey, let clip = binding.clipKey else { continue }
            let facts = presenter.transformFacts(binding)
            if let prior = observations[binding.id], prior.binding == binding, prior.facts == facts, prior.accepted { continue }
            guard serial < UInt64.max else { session.transformInputHold?.cancel(); return }
            serial += 1
            let sequence = serial
            observations[binding.id] = Observation(binding: binding, facts: facts, sequence: sequence, accepted: false)
            let values = facts.map { $0.dimensions + [0, 0] } ?? [Double](repeating: 0, count: 6)
            let packet = TransformDragPacket(op: facts == nil ? 14 : 10, runtime: binding.runtime,
                handleKey: binding.handleKey, targetKey: target, clipKey: clip,
                sequence: sequence, values: values, now: session.now())
            delivering = true
            if let reply = session.runtime.transformMotion(packet) {
                observations[binding.id]?.accepted = reply.accepted
                session.apply(reply.batch)
            }
            delivering = false
            refine = refine || presenter.transformBindings[binding.id] != binding || presenter.transformFacts(binding) != facts
        }
        // A geometry action may change layout. One later UI refinement is
        // allowed; a refusal by itself never starts a polling/frame loop.
        if refine || presenter.transformBindings != bindings { enqueue() }
    }
}
