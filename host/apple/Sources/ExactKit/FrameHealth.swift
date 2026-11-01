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
    #if DEBUG
    static let nativeObserverAllowed = !ExactEnv.releaseBuild
    #else
    static let nativeObserverAllowed = false
    #endif
    static let configured = (ExactEnv.appMetadata["ExactFrameHealthMode"] as? String)
        .map { $0 == "core" } ?? !ExactEnv.productionBake
    private(set) var handle: UInt32
    let source: String
    let window = FrameHealthWindow()
    var nativeCoverage = "unobserved"
    var nativeHealth: FrameHealth?
    var nativeScrollActive = false
    var nativeSampleLoss: UInt64 = 0
    weak var publicationOwner: FrameHealth?
    var inspectorCapture = false
    var inspected: (() -> Void)? { didSet { FrameHealthPublication.update(self) } }
    var inspectionEnded: (() -> Void)?
    #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
    var scrollObserver: AnyObject?
    #endif
    #if os(macOS)
    weak var macContainer: NSView?
    var inspectorController: NSWindowController?
    var desktopOverlay: FrameHealthOverlayMac?
    #endif
    private var lifecycle: [NSObjectProtocol] = []
    private var running = false
    let unavailableReason: String?
    let unavailableCode: String?
    var captureID: UInt64 = 0
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
    var hudEffectiveVisible = false
    var hudBounds: CGRect?
    private(set) var virtual = false
    var publicationDirty = false
    var enabled: Bool { handle != 0 }

    init(enabled: Bool = FrameHealth.configured, source: String = "exact-app-display-link") {
        self.source = source
        handle = enabled && Thread.isMainThread ? exact_frame_health_create() : 0
        unavailableCode = !enabled ? "collection-disabled" : !Thread.isMainThread ? "wrong-thread" : handle == 0 ? "collector-capacity" : nil
        unavailableReason = enabled ? (!Thread.isMainThread ? "Frame health requires a UI-thread session." : handle == 0 ? "Collector capacity reached. Close unused sessions and reload." : nil)
            : "Collection is disabled in this build."
    }

    func attach(_ session: ExactSession) {
        virtual = session.clock != nil
        #if canImport(UIKit)
        guard enabled else { return }
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
        guard enabled else { return }
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
        // The detail panel's cached read predates this work record
        window.invalidate()
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
        guard changed != nil || inspected != nil else { return }
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
        window.invalidate()
        (publicationOwner ?? self).publish()
    }

    func reset() {
        guard enabled else { return }
        #if (os(iOS) || os(tvOS)) && !targetEnvironment(macCatalyst)
        stopScroll()
        #endif
        nativeHealth?.clockChanged(virtual: virtual)
        track = nil
        window.invalidate()
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
        guard changed != nil || inspected != nil else { return }
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
        let desktop = desktopOverlay, inspector = inspectorController
        desktopOverlay = nil; inspectorController = nil
        #endif
        let inspection = inspectionEnded
        inspectionEnded = nil
        inspected = nil
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
            desktop?.setVisible(false); desktop?.removeFromSuperview(); inspector?.close()
            #endif
            notifications.forEach(NotificationCenter.default.removeObserver)
            inspection?()
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
