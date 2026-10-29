import Foundation
import QuartzCore
import CExact
import ExactCore
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// Existing app-clock callbacks only. Native-only scrolling and presentation are unobserved.
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
    var reportInFlight = false
    var requestedPeriodNs: UInt64 = 0
    var requestedHertz: Float = 0
    var hudDragging = false
    var changed: (() -> Void)? { didSet { FrameHealthPublication.update(self) } }
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

    func record(arrival: Double, target: Double, period: Double, finished: Double, flags: UInt64 = 0) {
        guard enabled, arrival.isFinite, target.isFinite, period.isFinite, finished.isFinite,
              arrival > 0, target > 0, period >= 0, finished > 0 else { return }
        expireCapture(at: arrival)
        exact_frame_health_record(handle, Self.nanoseconds(arrival), Self.nanoseconds(target),
            Self.nanoseconds(period), Self.nanoseconds(finished), flags)
        running = true
        (publicationOwner ?? self).publicationDue(at: finished)
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
        window.invalidate()
        exact_frame_health_reset(handle)
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

    private func publish() {
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
            "coverage": "Synchronous Exact frame callbacks; optional bounded owner/apply evidence in perf frames report; native-only animations and presentation unobserved",
            "status": virtual ? "virtual" : !enabled ? (unavailableCode == "collection-disabled" ? "disabled" : "unavailable") : s.running == 0 ? "inactive" : "observing",
            "generation": s.generation, "callbacks": s.callbacks, "intervals": s.intervals,
            "activeNs": s.active_ns, "scheduledPeriodNs": s.period_ns == 0 ? NSNull() : s.period_ns as Any,
            "maximumCallbackGapNs": s.max_gap_ns, "synchronousCallbackOverruns": s.work_overruns,
            "exactWorkDeadlineOverruns": NSNull(),
            "cadenceChanges": s.cadence_changes, "presentedFrames": NSNull(), "contentLatenessNs": NSNull()]
    }

    static func nanoseconds(_ seconds: Double) -> UInt64 {
        UInt64(min(seconds * 1_000_000_000, Double(UInt64.max / 2)))
    }
}
