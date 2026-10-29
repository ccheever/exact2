#if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
import UIKit
import QuartzCore

private final class FrameHealthScrollLifetime: UIView {
    var detached: (() -> Void)?
    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil { detached?() }
    }
}

/// Development feasibility adapter. It observes one active scroller without requesting updates or a rate.
@available(iOS 18.0, tvOS 18.0, *)
final class FrameHealthScrollObserver {
    weak var owner: FrameHealth?
    weak var session: ExactSession?
    weak var scrollView: UIScrollView?
    let health: FrameHealth
    private(set) var link: UIUpdateLink?
    private let lifetime = FrameHealthScrollLifetime(frame: .zero)
    private var began = 0.0
    private var deadline = 0.0

    init(owner: FrameHealth, session: ExactSession) {
        self.owner = owner; self.session = session
        health = FrameHealth(enabled: owner.enabled, source: "native-scroll-ui-update")
        health.nativeCoverage = "one-active-scroller; passive observer; cadence unqualified"
        health.publicationOwner = owner
    }

    func begin(_ scrollView: UIScrollView) {
        guard session?.clock == nil, scrollView.window?.windowScene != nil, FrameHealth.nativeObserverAllowed, health.enabled else { return }
        if self.scrollView === scrollView, link != nil { return }
        stop()
        self.scrollView = scrollView
        lifetime.isHidden = true
        lifetime.isUserInteractionEnabled = false
        lifetime.accessibilityIdentifier = "exact.host.frame-health-observer-lifetime"
        lifetime.detached = { [weak self] in self?.stop() }
        scrollView.addSubview(lifetime)
        let update = UIUpdateLink(view: scrollView)
        // Keep the platform defaults: passive observation, default rate, normal latency and presentation.
        update.addAction(to: .beforeCADisplayLinkDispatch) { [weak self] _, info in
            guard let self else { return }
            self.began = CACurrentMediaTime()
            self.deadline = info.completionDeadlineTime
        }
        update.addAction(to: .afterCATransactionCommit) { [weak self] _, _ in self?.completed() }
        link = update
        update.isEnabled = true
        owner?.nativeScrollActive = true
    }

    private func completed() {
        guard let session, session.clock == nil, let scrollView, scrollView.window != nil else { stop(); return }
        guard began > 0, deadline > 0 else {
            health.nativeSampleLoss = health.nativeSampleLoss == UInt64.max ? UInt64.max : health.nativeSampleLoss + 1
            return
        }
        let finished = CACurrentMediaTime()
        // UIUpdateInfo gives a completion deadline, not a complete VRR schedule. Keep period unknown.
        health.record(arrival: began, target: deadline, period: 0, finished: finished,
            flags: owner?.frameFlags(session.frames, session: session) ?? 0)
        began = 0
    }

    func end(_ scrollView: UIScrollView) { if self.scrollView === scrollView { stop() } }
    func stop() {
        owner?.nativeScrollActive = false
        link?.isEnabled = false
        link = nil
        lifetime.detached = nil
        lifetime.removeFromSuperview()
        scrollView = nil
        began = 0
        health.stop()
    }
    deinit { link?.isEnabled = false }
}

extension FrameHealth {
    func prepareScrollObservation(session: ExactSession) {
        guard enabled, !virtual, FrameHealth.nativeObserverAllowed else { return }
        if #available(iOS 18.0, tvOS 18.0, *) {
            if scrollObserver == nil {
                let observer = FrameHealthScrollObserver(owner: self, session: session)
                scrollObserver = observer
                nativeHealth = observer.health
                nativeHealth?.setHUDVisible(hudVisible)
                nativeHealth?.setInspectorVisible(inspectorCapture)
                if liveCapture { _ = nativeHealth?.setAgentCapture("start", duration: min(120_000, max(1, (agentCaptureUntil - CACurrentMediaTime()) * 1000).rounded())) }
                nativeCoverage = observer.health.enabled ? "passive native-scroll proxy; presentation/cadence unqualified" : "native-scroll collector capacity exhausted"
            }
        } else { nativeCoverage = "native-scroll observer requires iOS/tvOS 18" }
    }
    func beginScroll(_ view: UIScrollView, session: ExactSession) {
        prepareScrollObservation(session: session)
        if #available(iOS 18.0, tvOS 18.0, *) { (scrollObserver as? FrameHealthScrollObserver)?.begin(view) }
    }
    func endScroll(_ view: UIScrollView) {
        if #available(iOS 18.0, tvOS 18.0, *) { (scrollObserver as? FrameHealthScrollObserver)?.end(view) }
    }
    func stopScroll() {
        if #available(iOS 18.0, tvOS 18.0, *) { (scrollObserver as? FrameHealthScrollObserver)?.stop() }
    }
}
#endif
