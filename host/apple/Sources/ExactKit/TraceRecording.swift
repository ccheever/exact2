// A trace the app records (`startTrace_EXPERIMENTAL()`,
// `stopTrace_EXPERIMENTAL()`; experimental while its fields may change): the
// frames Save Trace keeps (LLP 1079 D3–D5), from one press to the next, in any
// build, a production bake included, put on the clipboard as compact JSON an
// agent reads whole, or cut to a size a conversation takes
// (`stopTrace_EXPERIMENTAL(maxBytes)`); tvOS, with no clipboard, only saves
// the file. On iOS a person's scroll of an
// element's scroller, a route's page or the document is a drag
// (UIScrollView's `isDragging`) and a glide (`isDecelerating`), each its own
// segment, with an estimate of the hitch time ratio Apple's scroll metrics
// report (XCTOSSignpostMetric's scroll dragging and deceleration metrics:
// milliseconds late per second) from the display link's callbacks. A
// scroller that moves with no finger on it (an animation, the app's own
// scroll) is moving. A Mac recording follows no scroller: its frames are at rest.
import Foundation
import QuartzCore
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

extension ExactSession {
    /// The host commands `startTrace_EXPERIMENTAL()` and `stopTrace_EXPERIMENTAL([maxBytes])`.
    func traceCommand(_ name: String, _ args: [Any]) {
        if name == "startTrace_EXPERIMENTAL" { startTrace(); return }
        stopTrace(maxBytes: (args.first as? NSNumber)?.intValue)
    }

    /// Record each display-link callback from now until `stopTrace`.
    public func startTrace() {
        if sampler == nil { sampler = FrameSampler(session: self) }
        if HatchDiagnostics.available { HatchDiagnostics.measuring = true }
        guard sampler?.beginRecording() == true else {
            releaseSampler()
            log("startTrace_EXPERIMENTAL refused: a recording is running, or the window has no display link")
            return
        }
        log("trace recording started")
    }

    /// Let a production bake's sampler go once no recording keeps it, or
    /// its session is destroyed, and the host phases with the last sampler
    /// any session keeps. A build that measures on its own keeps its sampler.
    func releaseSampler(measured: Bool = FrameSampler.measured) {
        guard !measured, sampler?.recording == nil || state == .destroyed else { return }
        sampler?.stop()
        sampler = nil
        if !app.sessions.contains(where: { $0.sampler != nil }) { HostPhases.shared.on = false }
    }

    /// End the recording, save its trace and put it on the clipboard (on
    /// tvOS, which has none, only save it), cut to `maxBytes` when given;
    /// what was copied, or nil when no recording was running.
    @discardableResult public func stopTrace(maxBytes: Int? = nil) -> String? {
        guard let sampler, let recording = sampler.endRecording() else {
            log("stopTrace_EXPERIMENTAL refused: no recording is running")
            return nil
        }
        releaseSampler()
        let report = FrameSampler.report(recording, identity: traceIdentity())
        guard let data = try? JSONSerialization.data(withJSONObject: report, options: [.sortedKeys]) else {
            log("stopTrace_EXPERIMENTAL refused: the trace did not serialize")
            return nil
        }
        let stamps = ISO8601DateFormatter()
        stamps.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        var url: URL?
        do { url = try FrameSampler.save(data, in: FileManager.default.temporaryDirectory, stamp: stamps.string(from: Date()).replacingOccurrences(of: ":", with: "-")) } catch {
            log("stopTrace_EXPERIMENTAL: the whole trace was not saved: \(error.localizedDescription)")
        }
        let copied = maxBytes.map { FrameSampler.compact(report, file: url?.lastPathComponent, bytes: data.count, maxBytes: $0) } ?? data
        let text = String(decoding: copied, as: UTF8.self)
        let saved = url.map { ", saved to \($0.path)" } ?? ", not saved"
        #if os(tvOS)
        // tvOS has no clipboard: the file is the only copy
        log("trace recorded: \(data.count) bytes, \(recording.records.count) frames\(saved)")
        #else
        #if canImport(UIKit)
        UIPasteboard.general.string = text
        #else
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #endif
        log("trace copied: \(copied.count) bytes of \(data.count), \(recording.records.count) frames\(saved)")
        #endif
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
            // Low Power Mode limits a ProMotion display to 60 Hz
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
    /// Save a recording's whole trace in `directory` as
    /// `trace-recording-<stamp>-<suffix>.json`, never over another file: a
    /// name already taken (two recordings stopped in one millisecond) is
    /// tried again with a new suffix. Where it went, or why it could not.
    static func save(_ data: Data, in directory: URL, stamp: String, suffix: () -> String = { String(UUID().uuidString.prefix(8)) }) throws -> URL {
        var taken: Error?
        for _ in 0..<8 {
            let url = directory.appendingPathComponent("trace-recording-\(stamp)-\(suffix()).json")
            do { try data.write(to: url, options: .withoutOverwriting); return url } catch let e as CocoaError where e.code == .fileWriteFileExists { taken = e }
        }
        throw taken ?? CocoaError(.fileWriteFileExists)
    }

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
        // offset changed at all since the frame before is moving (a still
        // scroller's committed offset does not change, and a slow one moves
        // less than a point a frame), as is the first frame on a scroller the
        // recording took as it moved
        let taken = records.map { r in r.taken.map { recording.scrollers[$0] } }
        let scrollers = taken.map { $0?.name ?? "" }
        let phases = records.indices.map { i -> String in
            let r = records[i], p = r.scroll.map { phaseName[$0.1] ?? $0.1 } ?? "rest"
            guard p == "rest", let now = r.scroll?.0 else { return p }
            // Against the frame before on the same scroller; with none, this is the first the recording saw of it
            guard i > 0, scrollers[i] == scrollers[i - 1], let then = records[i - 1].scroll?.0 else { return taken[i]?.moved == true ? "moving" : p }
            return now != then ? "moving" : p
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
                // From the offset the segment's first interval began at: the
                // frame before's on the same scroller, or where a person's
                // finger took it; unknown for one taken as it moved
                let start = first > 0 && scrollers[first - 1] == scrollers[i] ? records[first - 1].scroll?.0 : taken[first]?.from
                if let a = start, let b = records[i].scroll?.0 { o["scrolledPt"] = r2(b - a) }
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
                "phase": "drag: a finger moves the scroller (isDragging); glide: it decelerates after release; bounce: it is past its content's edge; moving: it moves with no finger on it (an animation, the app's own scroll); rest: no scroll, and every frame on a Mac, which follows no scroller",
                "interval": "ms between this frame's display-link callback and the one before",
                "period": "the display's frame period, ms, as the callback before announced it (targetTimestamp − timestamp); on iOS a recording asks for the display's full rate",
                "missed": "frames the display link saw the display show again: round(interval / period) − 1",
                "droppedFrames": "callbacks after the first 36,000 (five minutes at 120 Hz), which a recording does not keep",
                "overrun": "ms the main thread's turn ran past the frame's target, after Core Animation's commit (a commit that landed late)",
                "main": "ms of main-thread run-loop turns that ended in this frame's interval",
                "batches, apply": "the app's batches applied in the frame, and the ms applying them took",
                "phases, unattributed": "main-thread ms by Exact host phase inside the interval (runtime: waiting on the runner and the kernel's layout); unattributed is the rest: idle, UIKit's layout, the commit, and anything unwrapped",
                "phasesCoverage, coverage": "partial when the host phases or hatch calls kept no longer reach back over the whole frame, so its phases or hatches are incomplete",
                "hitchMs, hitchMsPerS": "an estimate of the hitch time Apple's scroll metrics report, from the display link's callbacks: how late the frames it saw shown again were (interval − period, frames that missed only), and that per second of the segment. It leaves out a frame that only overran (see overrun) and a hitch only the render server sees. Apple's bands: under 5 is good, 5 to 10 is noticeable, over 10 is bad",
            ],
        ]
    }
}

extension FrameSampler {
    static let compactTotals = ["frames", "ms", "missed", "lateFrames", "hitchMsPerS", "worstIntervalMs", "meanMainMs", "maxMainMs"]
    static let compactSegments = ["phase", "scroller", "from", "ms", "frames", "missed", "lateFrames", "hitchMsPerS", "worstIntervalMs", "meanMainMs", "maxMainMs", "scrolledPt"]
    static let compactLate = ["t", "phase", "interval", "period", "missed", "overrun", "main", "batches", "apply", "unattributed", "phases", "hatches", "phasesCoverage", "coverage", "scroller"]

    /// `report` in at most `maxBytes` of JSON, for a conversation that limits
    /// a message: its totals and segments as rows under their columns, the
    /// periods at rest left out, and as many late frames as fit, those late
    /// the longest first. `file` and `bytes` name the whole trace, nil when
    /// it was not saved. A budget too small for the totals gets a note naming
    /// the size that fits them.
    static func compact(_ report: [String: Any], file: String?, bytes: Int, maxBytes: Int) -> Data {
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
                "droppedFrames": (report["recording"] as? [String: Any])?["droppedFrames"] ?? 0,
                "totalColumns": compactTotals,
                "recording": row(report["recording"] as? [String: Any] ?? [:], compactTotals),
                "byPhase": (report["byPhase"] as? [String: [String: Any]] ?? [:]).mapValues { row($0, compactTotals) },
                "segmentColumns": compactSegments,
                "segments": segments,
                "lateColumns": compactLate,
                "late": late.prefix(n).map { row($0, compactLate) },
                "shown": "\(segments.count) of \(allSegments.count) scroll segments (the periods at rest left out), \(min(n, late.count)) of \(late.count) late frames reported, those late the longest first",
                "full": file.map { "\($0) in the app's temporary directory, \(bytes) bytes" } ?? "not saved",
                "legend": [
                    "phase": "drag: a finger moves the scroller; glide: it decelerates; bounce: past its edge; moving: it moves with no finger; rest: still, and every frame on a Mac, which follows no scroller",
                    "t, from": "ms since the process started; interval: since the display-link callback before; period: the display's frame period (on iOS a recording asks for its full rate)",
                    "missed": "frames the display link saw the display show again; overrun: ms the main thread's turn ran past the frame's target; droppedFrames: callbacks after the first 36,000, which a recording does not keep",
                    "main": "ms of main-thread turns that ended in the frame; phases: Exact's own main-thread work in it, ms; unattributed: the rest of the interval (idle, UIKit's layout, the commit); phasesCoverage, coverage: partial when the host phases or hatch calls kept no longer reach back over the whole frame, so its phases or hatches are incomplete",
                    "hitchMsPerS": "ms late per second of the frames the display link saw shown again, an estimate of Apple's scroll hitch ratio that leaves out frames that only overran and hitches only the render server sees: under 5 is good, over 10 is bad",
                ],
            ]
            return (try? JSONSerialization.data(withJSONObject: out, options: [.sortedKeys])) ?? Data()
        }
        // The most of `count` that fits, each one more making the report larger
        func most(_ count: Int, _ size: (Int) -> Int) -> Int {
            var lo = 0, hi = count
            while lo < hi {
                let mid = (lo + hi + 1) / 2
                if size(mid) <= maxBytes { lo = mid } else { hi = mid - 1 }
            }
            return lo
        }
        let smallest = build(segments: 0, late: 0).count
        guard smallest <= maxBytes else {
            let note: [String: Any] = ["exactTrace": "recording, compact", "error": "maxBytes \(maxBytes) is less than the \(smallest) bytes of the smallest compact trace, its totals with no segment or late frame; ask for at least that",
                                       "full": file.map { "\($0) in the app's temporary directory, \(bytes) bytes" } ?? "not saved"]
            return (try? JSONSerialization.data(withJSONObject: note, options: [.sortedKeys])) ?? Data()
        }
        // Every segment that fits, then the most late frames
        let m = most(allSegments.count) { build(segments: $0, late: 0).count }
        return build(segments: m, late: most(late.count) { build(segments: m, late: $0).count })
    }
}

#if canImport(UIKit)
extension FrameSampler {
    /// The node a scroller belongs to, by its test id or id when it has one;
    /// `page` for the document's, which no node holds.
    static func scrollerName(_ scroll: UIScrollView) -> String {
        var view: UIView? = scroll
        while let v = view, !(v is NodeView) { view = v.superview }
        guard let node = view as? NodeView else { return "page" }
        return node.props["testId"] ?? node.props["id"].map { "#\($0)" } ?? "node \(node.id)"
    }
}
#endif
