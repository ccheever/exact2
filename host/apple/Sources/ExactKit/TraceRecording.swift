// A trace the app records (`startTrace()`, `stopTrace()`): the frames Save
// Trace keeps (LLP 1079 D3–D5), from one press to the next, in any build, a
// production bake included, put on the clipboard as compact JSON an agent
// reads whole, or cut to a size a conversation takes (`stopTrace(maxBytes)`).
// A person's scroll is a drag (UIScrollView's `isTracking`)
// and a glide (`isDecelerating`), each its own segment, with the hitch time
// ratio Apple's scroll metrics use (XCTOSSignpostMetric's scroll dragging
// and deceleration metrics: milliseconds late per second). A scroller that
// moves with no finger on it (an animation, the app's own scroll) is moving.
import Foundation
import QuartzCore
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

extension ExactSession {
    /// The host commands `startTrace()` and `stopTrace([maxBytes])`.
    func traceCommand(_ name: String, _ args: [Any]) {
        if name == "startTrace" { startTrace(); return }
        stopTrace(maxBytes: (args.first as? NSNumber)?.intValue)
    }

    /// Record every frame from now until `stopTrace`.
    public func startTrace() {
        if sampler == nil { sampler = FrameSampler(session: self) }
        if HatchDiagnostics.available { HatchDiagnostics.measuring = true }
        guard sampler?.beginRecording() == true else {
            log("startTrace refused: a recording is running, or the window has no display link")
            return
        }
        log("trace recording started")
    }

    /// End the recording, save its trace and put it on the clipboard, cut to
    /// `maxBytes` when given; what was copied, or nil when no recording was
    /// running.
    @discardableResult public func stopTrace(maxBytes: Int? = nil) -> String? {
        guard let sampler, let recording = sampler.endRecording() else {
            log("stopTrace refused: no recording is running")
            return nil
        }
        // A production bake measures only while it records
        if !FrameSampler.measured {
            sampler.stop()
            self.sampler = nil
            HostPhases.shared.on = false
        }
        let report = FrameSampler.report(recording, identity: traceIdentity())
        guard let data = try? JSONSerialization.data(withJSONObject: report, options: [.sortedKeys]) else {
            log("stopTrace refused: the trace did not serialize")
            return nil
        }
        let stamp = ISO8601DateFormatter().string(from: Date()).replacingOccurrences(of: ":", with: "-")
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("trace-recording-\(stamp).json")
        try? data.write(to: url)
        let copied = maxBytes.map { FrameSampler.compact(report, file: url.lastPathComponent, bytes: data.count, maxBytes: $0) } ?? data
        let text = String(decoding: copied, as: UTF8.self)
        #if os(tvOS)
        log("stopTrace: tvOS has no clipboard; the trace is only in the file")
        #elseif canImport(UIKit)
        UIPasteboard.general.string = text
        #else
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #endif
        log("trace copied: \(copied.count) bytes of \(data.count), \(recording.records.count) frames, saved to \(url.path)")
        return text
    }

    private func traceIdentity() -> [String: Any] {
        let info = Bundle.main.infoDictionary ?? [:], meta = ExactEnv.appMetadata
        var machine = utsname()
        uname(&machine)
        let model = withUnsafeBytes(of: &machine.machine) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
        var out: [String: Any] = [
            "bundle": Bundle.main.bundleIdentifier ?? "?",
            "version": "\(info["CFBundleShortVersionString"] as? String ?? "?") (\(info["CFBundleVersion"] as? String ?? "?"))",
            "os": ProcessInfo.processInfo.operatingSystemVersionString,
            "device": ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"].map { "\($0) (simulator)" } ?? model,
            "production": ExactEnv.productionBake,
            // Low Power Mode holds a ProMotion display at 60 Hz
            "lowPowerMode": ProcessInfo.processInfo.isLowPowerModeEnabled,
            "session": label,
        ]
        for (key, name) in [("ExactCommit", "exactCommit"), ("ExactAppCommit", "appCommit"), ("ExactBuildTime", "builtAt")] {
            if let value = meta[key] { out[name] = value }
        }
        #if canImport(UIKit)
        if let window = presenter.viewport.window {
            out["window"] = ["width": window.bounds.width, "height": window.bounds.height, "scale": window.screen.scale,
                             "maximumFramesPerSecond": window.screen.maximumFramesPerSecond]
        }
        #endif
        return out
    }
}

extension FrameSampler {
    /// Late frames kept in a report: some 64 KB at the 320 bytes one took in
    /// Lexy, small enough to paste into a conversation
    static let lateReported = 200

    /// A recording as an agent reads it: its segments by scroll phase and
    /// scroller, totals by phase, and its late frames with what the main
    /// thread did in each.
    static func report(_ recording: Recording, identity: [String: Any]) -> [String: Any] {
        let r2 = { (x: Double) in (x * 100).rounded() / 100 }
        let records = recording.records
        let phaseName = ["t": "drag", "d": "glide", "b": "bounce", "-": "rest"]
        // Each frame's scroller, and its phase: one at rest by its flags whose
        // offset moved since the frame before is moving
        let scrollers = records.map { r in recording.scrollers.last { $0.t <= r.t }?.name ?? "" }
        let phases = records.indices.map { i -> String in
            let r = records[i], p = r.scroll.map { phaseName[$0.1] ?? $0.1 } ?? "rest"
            guard p == "rest", i > 0, scrollers[i] == scrollers[i - 1], let now = r.scroll?.0, let then = records[i - 1].scroll?.0 else { return p }
            return abs(now - then) > 0.5 ? "moving" : p
        }
        // Count hitch time only for a frame the display showed again, so the display link's jitter adds none
        func lateness(_ r: Record) -> Double { r.missed > 0 ? max(0, r.interval - r.period) : 0 }

        struct Totals { var ms = 0.0, frames = 0, missed = 0, late = 0, hitch = 0.0, main = 0.0, worst = 0.0, maxMain = 0.0 }
        func add(_ t: inout Totals, _ r: Record) {
            t.ms += r.interval; t.frames += 1; t.missed += r.missed; t.hitch += lateness(r)
            if r.missed > 0 || r.overrun > 0 { t.late += 1 }
            t.main += r.main ?? 0; t.worst = max(t.worst, r.interval); t.maxMain = max(t.maxMain, r.main ?? 0)
        }
        func json(_ t: Totals) -> [String: Any] {
            ["ms": r2(t.ms), "frames": t.frames, "missed": t.missed, "lateFrames": t.late, "hitchMs": r2(t.hitch),
             "hitchMsPerS": t.ms > 0 ? r2(t.hitch / (t.ms / 1000)) : 0, "worstIntervalMs": r2(t.worst),
             "meanMainMs": t.frames > 0 ? r2(t.main / Double(t.frames)) : 0, "maxMainMs": r2(t.maxMain)]
        }

        // Segments: runs of one phase on one scroller
        var segments: [[String: Any]] = [], byPhase: [String: Totals] = [:]
        var first = 0
        for i in records.indices {
            add(&byPhase[phases[i], default: Totals()], records[i])
            let last = i == records.count - 1 || phases[i + 1] != phases[i] || (phases[i] != "rest" && scrollers[i + 1] != scrollers[i])
            guard last else { continue }
            let run = records[first...i]
            var o = json(run.reduce(into: Totals()) { add(&$0, $1) })
            o["phase"] = phases[i]; o["from"] = r2(records[first].t - records[first].interval); o["to"] = records[i].t
            if phases[i] != "rest" {
                o["scroller"] = scrollers[i]
                if let a = records[first].scroll?.0, let b = records[i].scroll?.0 { o["scrolledPt"] = r2(b - a) }
            }
            segments.append(o)
            first = i + 1
        }

        var late = records.indices.filter { records[$0].missed > 0 || records[$0].overrun > 0 }
        let lateCount = late.count
        if late.count > lateReported {
            late = Array(late.sorted { lateness(records[$0]) + records[$0].overrun > lateness(records[$1]) + records[$1].overrun }.prefix(lateReported)).sorted()
        }
        let lateJSON = late.map { i -> [String: Any] in
            var o = records[i].json
            o["phase"] = phases[i]
            if phases[i] != "rest" { o["scroller"] = scrollers[i] }
            o["target"] = nil
            return o
        }
        let all = records.reduce(into: Totals()) { add(&$0, $1) }
        return [
            "exactTrace": "recording",
            "identity": identity,
            "timebase": "t, from and to are ms since the process started (the media clock)",
            "periodMs": records.map(\.period).sorted().dropFirst(records.count / 2).first.map(r2) ?? NSNull(),
            "recording": json(all).merging(["droppedFrames": recording.dropped]) { a, _ in a },
            "byPhase": byPhase.mapValues(json),
            "segments": segments,
            "late": lateJSON,
            "lateShown": "\(lateJSON.count) of \(lateCount)\(lateCount > lateReported ? ", those late the longest" : "")",
            "hatches": HatchDiagnostics.available ? "timed" : "not timed in a production build",
            "legend": [
                "phase": "drag: a finger moves the scroller (isTracking); glide: it decelerates after release; bounce: it is past its content's edge; moving: it moves with no finger on it (an animation, the app's own scroll); rest: no scroll",
                "interval": "ms between this frame's display-link callback and the one before",
                "period": "the display's frame period, ms (the link's targetTimestamp − timestamp; a recording asks for the display's full rate)",
                "missed": "frames the display showed again: round(interval / period) − 1",
                "overrun": "ms the main thread's turn ran past the frame's target, after Core Animation's commit (a commit that landed late)",
                "main": "ms of main-thread run-loop turns that ended in this frame's interval",
                "batches, apply": "the app's batches applied in the frame, and the ms applying them took",
                "phases, unattributed": "main-thread ms by Exact host phase inside the interval (runtime: waiting on the runner and the kernel's layout); unattributed is the rest: idle, UIKit's layout, the commit, and anything unwrapped",
                "hitchMs, hitchMsPerS": "how late the frames shown again were (interval − period, frames that missed only), and that per second of the segment, as Apple's scroll metrics count it: under 5 is good, 5 to 10 is noticeable, over 10 is bad",
            ],
        ]
    }
}

extension FrameSampler {
    static let compactTotals = ["frames", "ms", "missed", "lateFrames", "hitchMsPerS", "worstIntervalMs", "meanMainMs", "maxMainMs"]
    static let compactSegments = ["phase", "scroller", "from", "ms", "frames", "missed", "lateFrames", "hitchMsPerS", "worstIntervalMs", "meanMainMs", "maxMainMs", "scrolledPt"]
    static let compactLate = ["t", "phase", "interval", "period", "missed", "overrun", "main", "batches", "apply", "unattributed", "phases", "hatches"]

    /// `report` in at most `maxBytes` of JSON, for a conversation that limits
    /// a message: its totals and segments as rows under their columns, the
    /// periods at rest left out, and as many late frames as fit, those late
    /// the longest first. `file` and `bytes` name the whole trace.
    static func compact(_ report: [String: Any], file: String, bytes: Int, maxBytes: Int) -> Data {
        func row(_ o: [String: Any], _ columns: [String]) -> [Any] {
            columns.map { c -> Any in
                switch c {
                case "phases": return (o["phases"] as? [[String: Any]] ?? []).map { "\($0["phase"] ?? "") \($0["ms"] ?? "")" }.joined(separator: ", ")
                case "hatches": return (o["hatches"] as? [[String: Any]] ?? []).map { "\($0["hatch"] ?? "") \($0["ms"] ?? "")" }.joined(separator: ", ")
                default: return o[c] ?? NSNull()
                }
            }
        }
        func lateness(_ o: [String: Any]) -> Double {
            let n = { (k: String) in (o[k] as? NSNumber)?.doubleValue ?? 0 }
            return (n("missed") > 0 ? max(0, n("interval") - n("period")) : 0) + n("overrun")
        }
        let identity = (report["identity"] as? [String: Any] ?? [:]).filter { ["version", "exactCommit", "device", "os", "production", "lowPowerMode", "window"].contains($0.key) }
        let allSegments = (report["segments"] as? [[String: Any]] ?? []).filter { $0["phase"] as? String != "rest" }
        let worstSegments = allSegments.indices.sorted { (allSegments[$0]["hitchMsPerS"] as? Double ?? 0) > (allSegments[$1]["hitchMsPerS"] as? Double ?? 0) }
        let late = (report["late"] as? [[String: Any]] ?? []).sorted { lateness($0) > lateness($1) }
        func build(segments m: Int, late n: Int) -> Data {
            let segments = worstSegments.prefix(m).sorted().map { row(allSegments[$0], compactSegments) }
            let out: [String: Any] = [
                "exactTrace": "recording, compact",
                "identity": identity,
                "periodMs": report["periodMs"] ?? NSNull(),
                "totalColumns": compactTotals,
                "recording": row(report["recording"] as? [String: Any] ?? [:], compactTotals),
                "byPhase": (report["byPhase"] as? [String: [String: Any]] ?? [:]).mapValues { row($0, compactTotals) },
                "segmentColumns": compactSegments,
                "segments": segments,
                "lateColumns": compactLate,
                "late": late.prefix(n).map { row($0, compactLate) },
                "shown": "\(segments.count) of \(allSegments.count) scroll segments (the periods at rest left out), \(min(n, late.count)) of \(late.count) late frames reported, those late the longest first",
                "full": "\(file) in the app's temporary directory, \(bytes) bytes",
                "legend": [
                    "phase": "drag: a finger moves the scroller; glide: it decelerates; bounce: past its edge; moving: it moves with no finger; rest: still",
                    "t, from": "ms since the process started; interval: since the frame before; period: the display's frame period, at its full rate while recording",
                    "missed": "frames the display showed again; overrun: ms the main thread's turn ran past the frame's target",
                    "main": "ms of main-thread turns that ended in the frame; phases: Exact's own main-thread work in it, ms; unattributed: the rest of the interval (idle, UIKit's layout, the commit)",
                    "hitchMsPerS": "ms late per second: under 5 is good, over 10 is bad",
                ],
            ]
            return (try? JSONSerialization.data(withJSONObject: out, options: [.sortedKeys])) ?? Data()
        }
        // The most late frames that fit, then, with none, the most segments
        var m = allSegments.count
        if build(segments: m, late: 0).count > maxBytes {
            while m > 0, build(segments: m, late: 0).count > maxBytes { m -= 1 }
            return build(segments: m, late: 0)
        }
        var lo = 0, hi = late.count
        while lo < hi {
            let mid = (lo + hi + 1) / 2
            if build(segments: m, late: mid).count <= maxBytes { lo = mid } else { hi = mid - 1 }
        }
        return build(segments: m, late: lo)
    }
}

#if canImport(UIKit)
extension FrameSampler {
    /// The node a scroller belongs to, by its test id or id when it has one.
    static func scrollerName(_ scroll: UIScrollView) -> String {
        var view: UIView? = scroll
        while let v = view, !(v is NodeView) { view = v.superview }
        guard let node = view as? NodeView else { return "?" }
        return node.props["testId"] ?? node.props["id"].map { "#\($0)" } ?? "node \(node.id)"
    }
}
#endif
