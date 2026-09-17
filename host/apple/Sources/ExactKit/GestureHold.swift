// Values and native lifetime only. The platform retains recognition/arbitration.
import Foundation

struct NativeHold {
    let token: UInt64
    let x: Double
    let y: Double
    init?(_ op: [String: Any]) {
        guard let raw = op["token"] as? String, let token = UInt64(raw),
              let x = op["x"] as? Double, let y = op["y"] as? Double,
              x.isFinite, y.isFinite else { return nil }
        self.token = token; self.x = x; self.y = y
    }
}

/// Invert resistance at the caught presentation, then apply finger displacement.
/// Zero displacement is exactly the captured value, even beyond the knee.
enum SwipeRecognition {
    static func accepts(x: Double, y: Double, presentedX: Double) -> Bool {
        abs(x) > abs(y) && (x > 0 || presentedX > 0.5)
    }
}

struct SwipeDisplacement {
    let base: Double
    private var origin: Double {
        abs(base) <= 64 ? base : (base < 0 ? -1 : 1) * (64 + (abs(base) - 64) / 0.2)
    }
    func value(_ displacement: Double) -> Double {
        if displacement == 0 { return base }
        let position = origin + displacement
        return abs(position) <= 64 ? position : (position < 0 ? -1 : 1) * (64 + (abs(position) - 64) * 0.2)
    }
    func velocity(displacement: Double, fingerVelocity: Double) -> Double {
        fingerVelocity * (abs(origin + displacement) > 64 ? 0.2 : 1)
    }
}

struct SwipeIndicator {
    let base: Double
    let progressAtCatch: Double
    func value(_ progress: Double) -> Double {
        if progress == progressAtCatch { return base }
        if progress < progressAtCatch { return base * progress / progressAtCatch }
        return base + (1 - base) * (progress - progressAtCatch) / (1 - progressAtCatch)
    }
}

/// Input eligibility is distinct from token ownership: an inert/hidden row
/// still owns its token until cancellation releases the presentation and pin.
enum SwipeInput {
    static func allows(_ view: NodeView) -> Bool {
        guard view.window != nil, !view.disabled, !view.inert, !view.isHidden else { return false }
        #if os(iOS)
        guard view.isUserInteractionEnabled else { return false }
        #endif
        var ancestor = view.superview
        while let parent = ancestor {
            if parent.isHidden || (parent as? NodeView)?.disabled == true { return false }
            #if os(iOS)
            if !parent.isUserInteractionEnabled { return false }
            #endif
            ancestor = parent.superview
        }
        return true
    }
}

/// One recognized swipe, at most one translate and two authored indicator holds.
/// Every callback checks both the session incarnation and native view identity.
final class SwipeHold {
    weak var session: ExactSession?
    weak var view: NodeView?
    let generation: Int
    let primary: NativeHold
    var indicators: [(NativeHold, Double)] = []
    private(set) var ended = false
    private var finishing = false
    private var displacement = 0.0
    private var pin: UInt64?
    private var deliveryQueued = false
    private var pendingMove: Double?
    private var pendingEnd: (Double, Double, Bool)?

    // A recognizer can cancel synchronously while UIKit reparents or disables
    // a view. One queued delivery per gesture crosses the *session* boundary,
    // including its final motion flag, then rechecks incarnation/view/token.
    private func scheduleDelivery() {
        guard !deliveryQueued else { return }
        deliveryQueued = true
        DispatchQueue.main.async { [self] in
            deliveryQueued = false
            if session?.isApplyingPresentation == true { scheduleDelivery(); return }
            let end = pendingEnd, sample = pendingMove
            pendingEnd = nil; pendingMove = nil
            if let end { finish(displacement: end.0, fingerVelocity: end.1, cancel: end.2) }
            else if let sample { _ = move(sample) }
        }
    }
    var mapping: SwipeDisplacement { SwipeDisplacement(base: primary.x) }

    init?(_ view: NodeView) {
        guard let session = view.presenter?.session, !session.isApplyingPresentation,
              SwipeInput.allows(view), view.handlers.contains("swiperight") else { return nil }
        self.session = session; self.view = view; generation = session.generation
        let (hold, batch) = session.runtime.holdBegin(view.id, property: 0, now: session.now())
        guard let hold else { session.apply(batch); return nil }
        primary = hold
        session.apply(batch)
        guard live else { return nil }
        // Only the first explicitly authored direct child is the companion.
        if let indicator = view.subviews.compactMap({ $0 as? NodeView }).first(where: { $0.props["swipeIndicator"] == "true" }) {
            for property: UInt32 in [3, 1] {
                let (token, batch) = session.runtime.holdBegin(indicator.id, property: property, now: session.now())
                if let token { indicators.append((token, token.x)) }
                session.apply(batch)
            }
        }
        pin = session.presenter.collections.holdPointer(view.id)
        session.trackInputHold(self)
        guard inputEligible else { cancel(); return nil }
    }
    var incarnationLive: Bool {
        guard let session, let view else { return false }
        return session.generation == generation && !session.runtime.destroyed && session.presenter.views[view.id] === view
    }
    var live: Bool { !ended && incarnationLive && session?.runtime.hasHold(primary.token) == true }

    var inputEligible: Bool {
        guard incarnationLive, let view else { return false }
        return SwipeInput.allows(view) && view.handlers.contains("swiperight")
    }
    func cancelIfInputIneligible() {
        if !ended && !inputEligible { cancel() }
    }

    @discardableResult func move(_ displacement: Double) -> Bool {
        guard displacement.isFinite, !ended, pendingEnd == nil, let session else { return false }
        if session.isApplyingPresentation || deliveryQueued {
            pendingMove = displacement; scheduleDelivery(); return true
        }
        guard live else { return false }
        guard inputEligible else {
            if !finishing { cancel() }
            return false
        }
        self.displacement = displacement
        let value = mapping.value(displacement)
        let batch = session.runtime.holdUpdate(primary.token, x: value, y: primary.y, now: session.now())
        session.apply(batch)
        guard batch.error == nil, live else { return false }
        let progress = min(1, max(0, value / 64))
        for (hold, base) in indicators {
            session.apply(session.runtime.holdUpdate(hold.token, x: SwipeIndicator(base: base, progressAtCatch: min(1, max(0, primary.x / 64))).value(progress), y: 0, now: session.now()))
        }
        return live
    }

    func finish(displacement: Double, fingerVelocity: Double, cancel: Bool) {
        guard !ended, !finishing, pendingEnd == nil else { return }
        if session?.isApplyingPresentation == true || deliveryQueued {
            pendingEnd = (displacement, fingerVelocity, cancel)
            scheduleDelivery(); return
        }
        finishing = true
        // Apply the last pointer sample before checking eligibility for action.
        let updated = move(displacement)
        guard let session else { ended = true; return }
        if updated && !cancel && fingerVelocity.isFinite && mapping.value(displacement) >= 64,
           live, inputEligible, let view {
            session.apply(session.runtime.swiperight(view.id, now: session.now()))
        }
        ended = true
        session.retireInputHold(self)
        // An action may remove this view or reboot the runtime. Never end in a successor.
        if session.generation == generation && !session.runtime.destroyed {
            let velocity = mapping.velocity(displacement: displacement, fingerVelocity: fingerVelocity)
            session.apply(session.runtime.holdEnd(primary.token, cancel: cancel || !updated || !inputEligible || !velocity.isFinite, vx: velocity.isFinite ? velocity : 0, now: session.now()))
            for (hold, _) in indicators {
                session.apply(session.runtime.holdEnd(hold.token, cancel: true, now: session.now()))
            }
            session.presenter.collections.releaseInteractionLater(ifCurrent: pin)
        }
    }
    func cancel() { finish(displacement: pendingMove ?? displacement, fingerVelocity: 0, cancel: true) }
}
