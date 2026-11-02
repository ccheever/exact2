import Foundation
import QuartzCore
import Darwin
import CExact
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// Export bounded timing evidence on request, without files, timers, screenshots or uploads.
enum FrameHealthReport {
    static let maximumBytes = 4000
    static let maximumAgeNs: UInt64 = 120_000_000_000
    private static let maximumInteger: UInt64 = 9_007_199_254_740_991
    static let format = "exact.frame-health/2"
    fileprivate static var encoding = false
    fileprivate static var nextAgentReport: Double = 0

    static func snapshot(health: FrameHealth, session: ExactSession?, now: Double = CACurrentMediaTime()) -> [String: Any] {
        health.expireCapture(at: now)
        let snapshot = health.snapshot(), history = health.historyStatus(), timestamp = FrameHealth.nanoseconds(now)
        var expired = 0
        var clamped = false
        var rows: [[Any]] = []
        // Every boundary is relative to its event, keeping JSON integers exact after long uptimes.
        func integer(_ value: UInt64) -> UInt64 {
            if value > maximumInteger { clamped = true; return maximumInteger }
            return value
        }
        func offset(_ value: UInt64, from origin: UInt64) -> Any {
            guard value != 0 else { return NSNull() }
            return value >= origin ? Int64(integer(value - origin)) : -Int64(integer(origin - value))
        }
        /// A hitch's stored microsecond offset; `INT32_MIN` was not observed.
        func stored(_ value: Int32) -> Any { value == Int32.min ? NSNull() : Int(value) }
        for index in 0..<min(history.retained, 16) {
            let event = health.hitch(UInt32(index))
            guard event.sequence != 0 else { continue }
            guard event.at_ns <= timestamp, timestamp - event.at_ns <= maximumAgeNs else { expired += 1; continue }
            // A boundary or identity that did not fit its packed field invalidates precision claims
            if event.flags & 64 != 0 { clamped = true }
            let work = event.work
            let hasWork = event.flags & 16 != 0, hasAfter = event.flags & 8 != 0
            rows.append([
                String(event.sequence), String(event.capture), String(event.generation), integer((timestamp - event.at_ns) / 1000), integer(event.flags),
                stored(event.target_us), event.period_us, event.requested_period_us, event.gap_us, event.callback_us,
                event.before_gap_us, event.before_callback_us,
                hasAfter ? stored(event.after_at_us) : NSNull(),
                hasAfter && event.after_gap_us != 0 ? event.after_gap_us as Any : NSNull(), hasAfter ? event.after_callback_us as Any : NSNull(),
                hasWork ? stored(work.queued_us) : NSNull(),
                hasWork ? stored(work.owner_began_us) : NSNull(),
                hasWork ? stored(work.owner_finished_us) : NSNull(),
                hasWork ? stored(work.landed_us) : NSNull(),
                hasWork ? stored(work.began_us) : NSNull(),
                hasWork ? stored(work.finished_us) : NSNull(),
                hasWork ? stored(work.target_us) : NSNull(),
                hasWork ? work.operations as Any : NSNull()])
        }
        let metadata = ExactEnv.appMetadata
        var identity: [String: Any] = ["app": limited(Bundle.main.bundleIdentifier ?? "embedded", 160),
            "device": deviceModel, "os": limited(ProcessInfo.processInfo.operatingSystemVersionString, 128),
            "build": limited(metadata["ExactCommit"] as? String ?? "unknown", 64),
            "appBuild": limited(metadata["ExactAppCommit"] as? String ?? "unknown", 64),
            "buildDirty": metadata["ExactCommitDirty"] as? Bool ?? false,
            "appBuildDirty": metadata["ExactAppCommitDirty"] as? Bool ?? false,
            "builtAt": limited(metadata["ExactBuildTime"] as? String ?? "unknown", 64),
            "nativeABI": EXACT_ABI_VERSION,
            "buildKind": limited(metadata["ExactBuildKind"] as? String ?? "unknown", 32),
            "simulator": ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] != nil]
        if let session {
            let viewport = session.presenter.viewport
            identity["viewportPoints"] = [viewport.bounds.width, viewport.bounds.height]
            #if canImport(UIKit)
            identity["displayMaximumHz"] = viewport.window?.screen.maximumFramesPerSecond ?? 0
            identity["displayScale"] = viewport.window?.screen.scale ?? 0
            #else
            identity["displayMaximumHz"] = viewport.window?.screen?.maximumFramesPerSecond ?? 0
            identity["displayScale"] = viewport.window?.backingScaleFactor ?? 0
            #endif
        }
        var packet: [String: Any] = [
            "format": format, "source": health.source, "identity": identity, "capturedAt": Date().timeIntervalSince1970,
            "nativeUpdateSamplesLost": String(health.nativeSampleLoss),
            "qualification": "unqualified", "evidence": "callback-and-apply-timing-proxies",
            "available": health.enabled && !health.virtual,
            "unavailableReason": health.unavailableCode as Any? ?? (health.virtual ? "virtual-clock" : NSNull()),
            "time": ["clock": "CACurrentMediaTime", "unit": "us", "clamped": clamped],
            "scope": ["generation": String(snapshot.generation), "capture": String(history.capture),
                "armed": history.active != 0, "callbackSourceRunning": snapshot.running != 0,
                "segments": String(history.segments), "observedCallbacks": String(history.callbacks),
                "captureStartedOffsetNs": offset(history.started_ns, from: timestamp),
                "captureChangedOffsetNs": offset(history.changed_ns, from: timestamp),
                "hudVisible": health.hudVisible, "liveAgentParity": ExactEnv.agentMode ? "unqualified" : "platform"],
            "retention": ["capacity": 16, "maximumAgeMs": 120_000, "retainedBeforeAgeFilter": history.retained,
                "expiredAtRead": expired, "overwrittenSinceClear": String(history.overwritten),
                "eventsSinceClear": String(history.total), "automaticDiskBytes": 0, "maximumReportBytes": maximumBytes,
                "omittedForBudget": 0, "selection": "newest"],
            "coreSinceReload": ["callbacks": String(snapshot.callbacks), "heldCallbacks": String(snapshot.held),
                "frameIntervals": String(snapshot.intervals), "activeNs": String(snapshot.active_ns),
                "maximumFrameIntervalNs": String(snapshot.max_interval_ns), "scheduledPeriodNs": snapshot.period_ns,
                "missedFrames": String(snapshot.missed_frames),
                "lateFrames": FrameHealthAssessment.deadlinesObserved ? String(snapshot.late) as Any : NSNull(), "detailMayHaveBeenOff": true],
            "unobserved": ["presentation", "GPU execution", "CPU-versus-descheduled time", "other main-thread work", "native-only animations", "uncaptured owner work", "app input latency"],
            "columns": ["sequence", "capture", "generation", "age_us", "flags", "target_offset_us", "period_us", "requested_period_us", "gap_us", "callback_us",
                "before_gap_us", "before_callback_us", "after_offset_us", "after_gap_us", "after_callback_us",
                "owner_queued_offset_us", "owner_began_offset_us", "owner_finished_offset_us", "main_landed_offset_us",
                "apply_began_offset_us", "apply_finished_offset_us", "work_target_offset_us", "operations"],
            "hitches": rows]
        if let root = session?.frameHealth {
            let others = [root, root.nativeHealth].compactMap { $0 }.filter { $0 !== health }
            packet["otherSources"] = others.map { ["source": $0.source, "retainedEvents": $0.historyStatus().retained] as [String: Any] }
            packet["selection"] = "one-source; request app or scroll explicitly for another source"
        }
        if var time = packet["time"] as? [String: Any] { time["clamped"] = clamped; packet["time"] = time }
        return packet
    }

    static func encode(_ packet: [String: Any]) -> Data {
        // Reserve the carrier's newline. Explanations live in the versioned schema documentation.
        let budget = maximumBytes - 1
        let failure = Data("{\"format\":\"exact.frame-health/2\",\"available\":false,\"error\":\"report-size-or-encoding-budget\"}".utf8)
        guard let rows = packet["hitches"] as? [[Any]], rows.count <= 16,
              var retention = packet["retention"] as? [String: Any] else { return failure }
        var packet = packet
        // Encode each row once to choose a suffix; one byte reserves a change in omitted-count digits.
        packet["hitches"] = [[Any]]()
        retention["omittedForBudget"] = rows.count
        packet["retention"] = retention
        guard let base = try? JSONSerialization.data(withJSONObject: packet, options: [.sortedKeys]),
              base.count + 1 <= budget else { return failure }
        var size = base.count + 1, count = 0
        for row in rows.reversed() {
            guard let encoded = try? JSONSerialization.data(withJSONObject: row) else { return failure }
            let added = encoded.count + (count == 0 ? 0 : 1)
            guard size + added <= budget else { break }
            size += added
            count += 1
        }
        packet["hitches"] = Array(rows.suffix(count))
        retention["omittedForBudget"] = rows.count - count
        packet["retention"] = retention
        guard let encoded = try? JSONSerialization.data(withJSONObject: packet, options: [.sortedKeys]),
              encoded.count <= budget else { return failure }
        return encoded
    }

    /// Copy bounded evidence on the main thread, then encode it on a utility worker.
    /// Allow one export at a time to prevent an encoding backlog.
    static func prepare(health: FrameHealth, session: ExactSession?, now: Double = CACurrentMediaTime(), completion: @escaping (Data) -> Void) {
        guard !health.reportInFlight, !encoding else {
            completion(Data("{\"format\":\"exact.frame-health/2\",\"available\":false,\"error\":\"report-busy\",\"retryAfterMs\":1000}".utf8))
            return
        }
        health.reportInFlight = true
        encoding = true
        let packet = snapshot(health: health, session: session, now: now)
        DispatchQueue.global(qos: .utility).async {
            let data = encode(packet)
            DispatchQueue.main.async { [weak health] in
                health?.reportInFlight = false
                encoding = false
                completion(data)
            }
        }
    }

    private static func limited(_ text: String, _ count: Int) -> String { String(decoding: text.utf8.prefix(count), as: UTF8.self) }
    private static let deviceModel = limited(BuildInfo.device(), 160)
}

extension FrameHealth {
    /// Admit at most one agent report every 15 seconds; each report is at most 4000 bytes.
    func agentReport(session: ExactSession?, source: FrameHealth? = nil, now: Double = CACurrentMediaTime(), completion: @escaping (Data) -> Void) {
        guard now >= FrameHealthReport.nextAgentReport else {
            let delay = Int(ceil((FrameHealthReport.nextAgentReport - now) * 1000))
            completion(Data("{\"format\":\"exact.frame-health/2\",\"available\":false,\"error\":\"report-rate-limit\",\"retryAfterMs\":\(delay)}".utf8))
            return
        }
        let selected = source ?? reportSource
        guard !selected.reportInFlight, !FrameHealthReport.encoding else {
            FrameHealthReport.prepare(health: selected, session: session, now: now, completion: completion)
            return
        }
        FrameHealthReport.nextAgentReport = now + 15
        FrameHealthReport.prepare(health: selected, session: session, now: now, completion: completion)
    }
}
