import Foundation
import QuartzCore
import CExact
import ExactCore

extension FrameHealth {
    func setHUDVisible(_ visible: Bool) {
        nativeHealth?.setHUDVisible(visible)
        hudVisible = visible
        if !visible { hudDragging = false }
        updateCapture(at: CACurrentMediaTime())
    }

    func setInspectorVisible(_ visible: Bool) {
        inspectorCapture = visible
        if !visible { window.close() }
        nativeHealth?.setInspectorVisible(visible)
        updateCapture(at: CACurrentMediaTime())
    }

    /// Agent capture is a bounded lease; expiration uses existing work or reads, never a timer.
    func setAgentCapture(_ action: String, duration: Double = 60_000) -> [String: Any] {
        guard ["start", "stop", "clear"].contains(action), duration.isFinite,
              duration >= 1, duration <= 120_000, duration.rounded() == duration else {
            return ["error": "Frame capture requires start, stop or clear and a duration of 1–120000 ms."]
        }
        guard enabled, !virtual else {
            return ["available": false, "capture": false, "reason": unavailableCode ?? "virtual-clock"]
        }
        _ = nativeHealth?.setAgentCapture(action, duration: duration)
        let now = CACurrentMediaTime()
        if action == "start" { agentCaptureUntil = now + duration / 1000 }
        if action == "stop" { agentCaptureUntil = 0 }
        updateCapture(at: now)
        if action == "clear", enabled {
            exact_frame_health_capture(handle, 2, Self.nanoseconds(now))
            if captureID != 0 { captureID = exact_frame_health_history_status(handle).capture }
        }
        return ["available": enabled && !virtual, "capture": captureID != 0,
            "agentLeaseRemainingMs": max(0, ((agentCaptureUntil - now) * 1000).rounded()),
            "reason": unavailableCode as Any? ?? (virtual ? "virtual-clock" : NSNull()),
            "capacity": 16, "maximumAgeMs": 120_000, "maximumReportBytes": FrameHealthReport.maximumBytes]
    }

    func updateCapture(at now: Double) {
        let wanted = enabled && !virtual && (hudVisible || inspectorCapture || agentCaptureUntil > now)
        guard wanted != (captureID != 0) else { return }
        exact_frame_health_capture(handle, wanted ? 1 : 0, Self.nanoseconds(now))
        if wanted {
            captureID = exact_frame_health_history_status(handle).capture
            FrameHealthEnvironment.retain()
        } else {
            captureID = 0
            FrameHealthEnvironment.release()
        }
    }

    func expireCapture(at now: Double) {
        if agentCaptureUntil != 0, now >= agentCaptureUntil {
            agentCaptureUntil = 0
            updateCapture(at: now)
        }
    }

    func frameFlags(_ frames: Frames, session: ExactSession) -> UInt64 {
        guard captureID != 0 else { return 0 }
        var bits = FrameHealthEnvironment.flags | (1 << 23)
        if frames.motion { bits |= 1 << 16 }
        if frames.spatial { bits |= 1 << 17 }
        if frames.canvas2d { bits |= 1 << 18 }
        if frames.tasks { bits |= 1 << 19 }
        if session.fillInFlight { bits |= 1 << 20 }
        if session.tickInFlight { bits |= 1 << 21 }
        if frames.hatches { bits |= 1 << 22 }
        if hudVisible { bits |= 1 << 35 }
        if hudDragging { bits |= 1 << 36 }
        if inspectorCapture { bits |= 1 << 38 }
        return bits
    }

    /// Record the outermost apply; match owner timestamps only to this batch's captured frame update.
    func applied(_ batch: Batch, began: Double, flags: UInt64 = 0) {
        guard enabled, !virtual, began > 0 else { return }
        let finished = CACurrentMediaTime()
        expireCapture(at: finished)
        let owner = batch.frameWorkTiming.flatMap { $0.capture == captureID ? $0 : nil }
        var work = ExactFrameHealthWork()
        work.capture = captureID
        work.began_ns = Self.nanoseconds(began)
        work.finished_ns = Self.nanoseconds(finished)
        work.operations = UInt64(batch.ops.count)
        work.target_ns = batch.frameWorkTiming?.frame.map { Self.nanoseconds(ExactEnv.t0 + $0 / 1000) } ?? 0
        if let owner, captureID != 0 {
            work.target_ns = owner.frame.map { Self.nanoseconds(ExactEnv.t0 + $0 / 1000) } ?? 0
            work.queued_ns = Self.nanoseconds(owner.queued)
            work.owner_began_ns = Self.nanoseconds(owner.began)
            work.owner_finished_ns = Self.nanoseconds(owner.finished)
            work.landed_ns = Self.nanoseconds(owner.landed)
        }
        exact_frame_health_applied(handle, work, flags | FrameHealthEnvironment.flags | (hudVisible ? 1 << 35 : 0) | (hudDragging ? 1 << 36 : 0))
        if nativeScrollActive, let native = nativeHealth, native.enabled {
            var context = work
            context.capture = native.captureID
            // Retain nearby apply timing without counting it again as native-scroll work
            context.queued_ns = 0; context.owner_began_ns = 0; context.owner_finished_ns = 0; context.landed_ns = 0
            exact_frame_health_context(native.handle, context)
        }
        (publicationOwner ?? self).publicationDue(at: finished)
    }

    func historyStatus() -> ExactFrameHealthHistoryStatus {
        enabled ? exact_frame_health_history_status(handle) : ExactFrameHealthHistoryStatus()
    }

    func hitch(_ index: UInt32) -> ExactFrameHealthHitch { exact_frame_health_hitch(handle, index) }

    func requestedRate(_ hertz: Float) {
        guard enabled, !virtual, hertz.isFinite, hertz != requestedHertz else { return }
        requestedHertz = hertz
        let period = hertz > 0 ? UInt64(min(Double(UInt64.max / 2), (1_000_000_000 / Double(hertz)).rounded())) : 0
        guard period != requestedPeriodNs else { return }
        requestedPeriodNs = period
        exact_frame_health_requested_period(handle, period)
    }
}

/// Shared, notification-driven power context. No battery polling or per-frame property queries.
enum FrameHealthEnvironment {
    static var flags: UInt64 = 0
    private static var users = 0
    private static var observers: [NSObjectProtocol] = []

    static func retain() {
        users += 1
        guard users == 1 else { return }
        refresh()
        for name in [ProcessInfo.thermalStateDidChangeNotification, .NSProcessInfoPowerStateDidChange] {
            observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { _ in refresh() })
        }
    }

    static func release() {
        guard Thread.isMainThread else { DispatchQueue.main.async { release() }; return }
        users -= 1
        guard users == 0 else { return }
        observers.forEach(NotificationCenter.default.removeObserver)
        observers.removeAll()
    }

    private static func refresh() {
        let process = ProcessInfo.processInfo
        flags = UInt64(process.thermalState.rawValue) << 32
        if process.isLowPowerModeEnabled { flags |= 1 << 34 }
    }
}
