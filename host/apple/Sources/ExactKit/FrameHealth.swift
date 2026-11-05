import Foundation
import QuartzCore
import CExact
import ExactCore
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// Bounded callback and apply timing. Presentation is unmeasured; the optional native-scroll
/// source reports deadlines with unknown cadence.
final class FrameHealth {
    /// When a collector collects: from the start, in a development build or where `app.json` says `"core"`; while
    /// the card shows, in a production build, which no agent can attach to (LLP 1079.000 §9); or never, where it says
    /// `"off"`.
    enum Policy { case always, onDemand, off }
    static let policy: Policy = switch ExactEnv.appMetadata["ExactFrameHealthMode"] as? String {
    case "off": .off
    case "core": .always
    case "demand": .onDemand
    default: ExactEnv.productionBake ? .onDemand : .always
    }
    #if DEBUG
    static let developmentObserver = !ExactEnv.releaseBuild
    #else
    static let developmentObserver = false
    #endif
    let policy: Policy
    /// Whether the card shows, so an on-demand collector collects
    private var demanded = false
    private(set) var handle: UInt32
    let source: String
    var nativeCoverage = "unobserved"
    var nativeHealth: FrameHealth?
    var nativeScrollActive = false
    var nativeSampleLoss: UInt64 = 0
    weak var publicationOwner: FrameHealth?
    #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
    var scrollObserver: AnyObject?
    #endif
    #if os(macOS)
    weak var macContainer: NSView?
    var desktopOverlay: FrameHealthOverlayMac?
    #endif
    private var lifecycle: [NSObjectProtocol] = []
    /// Exact's own frame callbacks run: a frame was recorded since the source last stopped
    private(set) var running = false
    private(set) var unavailableReason: String?
    private(set) var unavailableCode: String?
    var captureID: UInt64 = 0
    /// The screen's late frames' work split, by when their work began (nanoseconds): seconds the main thread waited in
    /// their updates, seconds of their work in Exact's host phases and the card's own, and the nodes' views laid out;
    /// nil where unmeasured. The newest 32.
    private(set) var splits: [(at: UInt64, waited: Double, exact: Double, card: Double, layouts: Int)] = []

    /// A late frame of the screen's, its work begun at `start` (seconds), split.
    func split(at start: Double, waited: Double, exact: Double, card: Double, layouts: Int) {
        if splits.count == 32 { splits.removeFirst() }
        splits.append((Self.nanoseconds(start), waited, exact, card, layouts))
    }
    var agentCaptureUntil: Double = 0
    /// An agent's scroll track (`FrameHealthTrack`), nil unless one is leased.
    var track: FrameHealthTrack?
    var reportInFlight = false
    var requestedPeriodNs: UInt64 = 0
    var requestedHertz: Float = 0
    var hudDragging = false
    /// Work a frame started that finishes later: owner work, which finishes when it lands, and a
    /// canvas render that drew nothing, as without its drawable, which finishes when it draws
    /// (LLP 1079.000 §4.6).
    static let tick: UInt8 = 1, advance: UInt8 = 2, canvas: UInt8 = 4
    /// A frame's work: what it started and has not finished, and whether its callback has returned.
    private struct Pending { var start, deadline, finished: Double; var waiting: UInt8; var returned: Bool }
    /// Each kind is in flight one at a time, so at most three frames wait.
    private var pending = [Pending?](repeating: nil, count: 3)
    var changed: (() -> Void)? { didSet { FrameHealthPublication.update(self) } }
    /// A card's charts while it reads, fed by each frame (LLP 1079.000 §7.2)
    weak var chart: FrameHealthChart?
    var ended: (() -> Void)?
    var hudVisible = false
    /// The card shows and can be seen; an on-demand collector collects only then
    var hudEffectiveVisible = false { didSet { if hudEffectiveVisible != oldValue { updateCapture(at: CACurrentMediaTime()) } } }
    var hudBounds: CGRect?
    private(set) var virtual = false
    var publicationDirty = false
    /// The source of the card's observer of the screen, whose frames have work against UIKit's deadlines and no frame
    /// times (LLP 1079.000 §7.4)
    static let screenSource = "ui-update"
    var screenFrames: Bool { source == Self.screenSource }
    /// Whether it collects now
    var enabled: Bool { handle != 0 && (policy != .onDemand || demanded) }
    /// Whether it collects now or can when asked
    var available: Bool { unavailableCode == nil }
    /// Whether it may observe native scrolling now: in a development build, while it collects (LLP 1079.000 §6.1)
    var observesNativeScrolling: Bool { enabled && Self.developmentObserver }

    init(policy: Policy = FrameHealth.policy, source: String = "exact-app-display-link") {
        self.policy = policy
        self.source = source
        let thread = Thread.isMainThread
        handle = policy == .always && thread ? exact_frame_health_create() : 0
        unavailableCode = policy == .off ? "collection-disabled" : !thread ? "wrong-thread" : policy == .always && handle == 0 ? "collector-capacity" : nil
        unavailableReason = switch unavailableCode {
        case "collection-disabled": "Collection is disabled in this build."
        case "wrong-thread": "Frame health requires a UI-thread session."
        case "collector-capacity": Self.capacityReason
        default: nil
        }
    }

    convenience init(enabled: Bool, source: String = "exact-app-display-link") {
        self.init(policy: enabled ? .always : .off, source: source)
    }

    private static let capacityReason = "Collector capacity reached. Close unused sessions and reload."

    /// Why this session's card cannot read and how to change that, for a person; nil when it can.
    var unavailableExplanation: (title: String, message: String)? {
        guard !available else { return nil }
        guard unavailableCode != "collection-disabled" else {
            return ("Frame Health Is Off", "This app turns frame health off with \"frame_health\": { \"mode\": \"off\" } in app.json. To measure frame timings, remove that setting and build the app again.")
        }
        return ("Frame Health Is Unavailable", unavailableReason ?? "This session does not collect frame timing.")
    }

    /// Collect while the card shows, for an on-demand collector: its collector is made the first time and kept, and
    /// each demand starts a new generation, so nothing collected before mixes with what this one does. Without
    /// demand, every hook returns at its first check.
    func demand(_ wanted: Bool) {
        guard policy == .onDemand, available, wanted != demanded else { return }
        if wanted {
            if handle == 0 { handle = exact_frame_health_create() }
            guard handle != 0 else { (unavailableCode, unavailableReason) = ("collector-capacity", Self.capacityReason); return }
            let (hertz, period) = (requestedHertz, requestedPeriodNs)
            demanded = true
            reset()
            // The rate requested before this demand still governs the frames it collects
            (requestedHertz, requestedPeriodNs) = (hertz, period)
            if period > 0 { exact_frame_health_requested_period(handle, period) }
        } else {
            #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
            stopScroll()
            #endif
            stop()
            pending = [Pending?](repeating: nil, count: 3)
            demanded = false
        }
    }

    func attach(_ session: ExactSession) {
        virtual = session.clock != nil
        #if canImport(UIKit)
        guard available else { return }
        lifecycle.append(NotificationCenter.default.addObserver(forName: UIScene.willDeactivateNotification,
            object: nil, queue: .main) { [weak self, weak session] notification in
                guard let scene = session?.presenter.viewport.window?.windowScene,
                      notification.object as? UIScene === scene else { return }
                self?.stop()
                self?.nativeHealth?.stop()
                #if !targetEnvironment(macCatalyst)
                self?.stopScroll()
                #endif
            })
        #else
        guard available else { return }
        for name in [NSWindow.didMiniaturizeNotification, NSWindow.didChangeOcclusionStateNotification] {
            lifecycle.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self, weak session] notification in
                guard let window = session?.presenter.viewport.window, notification.object as? NSWindow === window,
                      window.isMiniaturized || !window.occlusionState.contains(.visible) else { return }
                self?.stop()
            })
        }
        #endif
    }

    /// Record one callback. `sent` is the owner work this frame sent, whose landing finishes it;
    /// `ran` says it did frame work of its own, and `held` that frame work waited on earlier work
    /// in flight. A callback that started no frame work is not a frame (LLP 1079.000 §4.6).
    func record(arrival: Double, target: Double, period: Double, finished: Double,
                held: Bool = false, ran: Bool = true, sent: UInt8 = 0, flags: UInt64 = 0) {
        guard enabled else { return }
        // The collector rejects the same observations, as it does one whose timestamp went backwards;
        // a rejected callback is no frame, so the work it registered has nothing to finish
        let valid = arrival.isFinite && target.isFinite && period.isFinite && finished.isFinite
            && arrival > 0 && target > 0 && period >= 0 && target >= period && finished >= arrival
        let outcome: UInt32 = sent != 0 ? 1 : ran ? 0 : held ? 2 : 3
        if valid { expireCapture(at: arrival) }
        let accepted = valid && exact_frame_health_record(handle, Self.nanoseconds(arrival), Self.nanoseconds(target),
            Self.nanoseconds(period), Self.nanoseconds(finished), outcome, flags) != 0
        // The card's charts take the frame before the work it finished within its callback
        if let chart {
            // A rejected or idle callback ends the run; a held one is no frame
            if !accepted || outcome == 3 { chart.endRun() }
            else if outcome != 2, period > 0 { chart.frame(at: target - period, finished: outcome == 0 ? finished : nil, deadline: target) }
        }
        // Work not yet returned was registered during this callback; any other is left from one that
        // was never recorded
        for index in pending.indices {
            guard var work = pending[index], !work.returned else { continue }
            guard accepted, outcome == 1, work.deadline == target else { pending[index] = nil; continue }
            // What it started may already have finished, during the callback
            work.returned = true
            work.finished = max(work.finished, finished)
            pending[index] = work
            finish(index)
        }
        guard valid else { return }
        running = true
        (publicationOwner ?? self).publicationDue(at: finished)
    }

    /// Work of `kind` is about to start for the frame starting at `start`. Owner work is registered
    /// before it is sent, since it can land before the callback returns.
    func sending(_ kind: UInt8, start: Double, deadline: Double) {
        guard enabled, start.isFinite, deadline.isFinite else { return }
        if let index = pending.firstIndex(where: { $0?.deadline == deadline && $0?.returned == false }) {
            pending[index]?.waiting |= kind
            return
        }
        let frame = Pending(start: start, deadline: deadline, finished: 0, waiting: kind, returned: false)
        // A free slot remains unless a finish was lost; then the oldest frame gives way
        let oldest = pending.indices.min { (pending[$0]?.deadline ?? 0) < (pending[$1]?.deadline ?? 0) } ?? 0
        pending[pending.firstIndex { $0 == nil } ?? oldest] = frame
    }

    /// Whether a frame waits on work of `kind`.
    func waits(on kind: UInt8) -> Bool { pending.contains { ($0?.waiting ?? 0) & kind != 0 } }

    /// Work of `kind` finished: owner work landed, applied or skipped, or a canvas render that waited
    /// on its drawable drew. The frame waiting on it may now be finished.
    func landed(_ kind: UInt8, at now: Double = CACurrentMediaTime()) {
        guard enabled else { return }
        var recorded = false
        for index in pending.indices {
            guard var work = pending[index], work.waiting & kind != 0 else { continue }
            work.waiting &= ~kind
            work.finished = max(work.finished, now)
            pending[index] = work
            recorded = finish(index) || recorded
        }
        guard recorded else { return }
        // A landing after the source stopped is the last change; publish it past the throttle
        if running { (publicationOwner ?? self).publicationDue(at: now) } else { (publicationOwner ?? self).publish() }
    }

    /// A canvas render that waited on its drawable will not draw: its canvas left, or the source
    /// stopped. The frame waiting on it never finishes, so it has no work record.
    func abandon(_ kind: UInt8) {
        for index in pending.indices where (pending[index]?.waiting ?? 0) & kind != 0 { pending[index] = nil }
    }

    /// Record a frame's work once its callback has returned and all it started has finished.
    @discardableResult
    private func finish(_ index: Int) -> Bool {
        guard let work = pending[index], work.returned, work.waiting == 0 else { return false }
        exact_frame_health_worked(handle, Self.nanoseconds(work.start), Self.nanoseconds(work.deadline), Self.nanoseconds(work.finished))
        chart?.work(start: work.start, deadline: work.deadline, finished: work.finished)
        pending[index] = nil
        return true
    }

    /// Frames whose work is still in flight, and those of them already past their deadline at `now`,
    /// so certainly late (LLP 1079.000 §4.4: unresolved exposure, apart from finalized counts).
    func pendingFrames(at now: Double) -> (pending: Int, overdue: Int) {
        let frames = pending.compactMap { $0 }
        return (frames.count, frames.filter { now > $0.deadline }.count)
    }

    func publicationDue(at now: Double) {
        guard changed != nil else { return }
        publicationDirty = true
        FrameHealthPublication.request(at: now)
    }

    func snapshot() -> ExactFrameHealthSnapshot { enabled ? exact_frame_health_snapshot(handle) : ExactFrameHealthSnapshot() }

    func stop() {
        guard enabled, running else { return }
        running = false
        abandon(Self.canvas)
        chart?.endRun()
        exact_frame_health_stop(handle)
        (publicationOwner ?? self).publish()
    }

    func reset() {
        guard enabled else { return }
        #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
        stopScroll()
        #endif
        nativeHealth?.clockChanged(virtual: virtual)
        track = nil
        exact_frame_health_reset(handle)
        chart?.reset()
        pending = [Pending?](repeating: nil, count: 3)
        requestedPeriodNs = 0
        requestedHertz = 0
        nativeSampleLoss = 0
        if captureID != 0 { FrameHealthEnvironment.release(); captureID = 0 }
        updateCapture(at: CACurrentMediaTime())
        running = false
        publish()
    }

    func clockChanged(virtual: Bool) {
        self.virtual = virtual
        reset()
    }

    fileprivate func publish() {
        guard changed != nil else { return }
        publicationDirty = true
        FrameHealthPublication.request(at: CACurrentMediaTime(), force: true)
    }

    func uiReply() -> [String: Any] {
        let bounds: Any = hudBounds.map { ["x": $0.minX, "y": $0.minY, "width": $0.width, "height": $0.height] } ?? NSNull()
        return ["hostApparatus": true, "identifier": "exact.host.frame-health-overlay", "visible": hudVisible, "effectiveVisible": hudEffectiveVisible,
            "windowLocalBounds": bounds, "excludedFromAppFindings": true, "excludedFromAppHitTargets": true]
    }

    func destroy() {
        track = nil
        let native = nativeHealth
        nativeHealth = nil
        #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
        let observer = scrollObserver
        scrollObserver = nil
        #endif
        #if os(macOS)
        let desktop = desktopOverlay
        desktopOverlay = nil
        #endif
        let notifications = lifecycle
        lifecycle.removeAll()
        if captureID != 0 { FrameHealthEnvironment.release(); captureID = 0 }
        changed = nil
        let finish = ended
        ended = nil
        let closing = handle
        handle = 0
        let release = {
            #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
            if #available(iOS 18.0, tvOS 18.0, *) { (observer as? FrameHealthScrollObserver)?.stop() }
            #endif
            native?.destroy()
            #if os(macOS)
            desktop?.setVisible(false); desktop?.removeFromSuperview()
            #endif
            notifications.forEach(NotificationCenter.default.removeObserver)
            finish?()
            if closing != 0 { exact_frame_health_destroy(closing) }
        }
        if Thread.isMainThread { release() }
        else { DispatchQueue.main.async(execute: release) }
    }

    deinit { destroy() }

    func reply(virtual: Bool) -> [String: Any] {
        let s = snapshot()
        return ["schemaVersion": 1, "available": enabled && !virtual,
            "qualification": "unqualified", "liveAgentWorkloadParity": ExactEnv.agentMode ? "unqualified" : "platform",
            "ui": uiReply(),
            "unavailableReason": unavailableReason as Any? ?? NSNull(),
            "source": source, "evidence": "callback-proxy",
            "coverage": "Exact frame callbacks and the owner work they send; optional bounded owner/apply evidence in perf frames report; Core Animation commits, native-only animations and presentation unobserved",
            "status": virtual ? "virtual" : !enabled ? (unavailableCode == "collection-disabled" ? "disabled" : "unavailable") : s.running == 0 ? "inactive" : "observing",
            "generation": s.generation, "callbacks": s.callbacks, "intervals": s.intervals,
            "activeNs": s.active_ns, "scheduledPeriodNs": s.period_ns == 0 ? NSNull() : s.period_ns as Any,
            "maximumFrameIntervalNs": s.max_interval_ns, "heldCallbacks": s.held,
            "workRecords": s.worked, "lateFrames": FrameHealthAssessment.deadlinesObserved ? s.late as Any : NSNull(),
            "cadenceChanges": s.cadence_changes, "presentedFrames": NSNull(), "contentLatenessNs": NSNull()]
    }

    /// The nearest nanosecond, so a converted time carries no downward bias.
    static func nanoseconds(_ seconds: Double) -> UInt64 {
        UInt64(min(max(0, (seconds * 1_000_000_000).rounded()), Double(UInt64.max / 2)))
    }
}
