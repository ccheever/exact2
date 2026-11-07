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
// milliseconds late per second) from the display link's callbacks, UIKit's
// commit deadlines and, on iOS and tvOS 18 and later, a presentation probe
// that sees when each frame reached the screen (`PresentationProbe`). A
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
        runtime.measureWork()
        sampler?.recording?.work["before"] = traceWork()
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
        recording.work["after"] = traceWork()
        var render: [String: Any]?
        #if canImport(UIKit)
        if let window = presenter.viewport.window {
            let began = CACurrentMediaTime()
            render = FrameSampler.renderInventory(in: window, scroller: recording.scroller)
            render?["ms"] = ((CACurrentMediaTime() - began) * 100_000).rounded() / 100
        }
        #endif
        let report = FrameSampler.report(recording, identity: traceIdentity(recording), render: render)
        guard let data = FrameSampler.traceJSON(report) else {
            log("stopTrace_EXPERIMENTAL refused: the trace did not serialize")
            return nil
        }
        let stamps = ISO8601DateFormatter()
        stamps.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        var url: URL?
        do { url = try FrameSampler.save(data, in: FrameSampler.traceDirectory, stamp: stamps.string(from: Date()).replacingOccurrences(of: ":", with: "-")) } catch {
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

    private func traceWork() -> [String: Any] {
        func read(_ request: String) -> Any {
            (try? JSONSerialization.jsonObject(with: Data(agent(request).utf8))) ?? NSNull()
        }
        return ["perf": read("{\"op\":\"perf\"}"), "journal": read("{\"op\":\"logs\",\"since\":0}")]
    }

    private func traceIdentity(_ recording: FrameSampler.Recording) -> [String: Any] {
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
            // Low Power Mode limits a ProMotion display to 60 Hz, and a hot device may lower its rate
            "lowPowerMode": ProcessInfo.processInfo.isLowPowerModeEnabled,
            "thermal": recording.thermal == ProcessInfo.processInfo.thermalState ? recording.thermal.name : "\(recording.thermal.name) to \(ProcessInfo.processInfo.thermalState.name)",
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

extension ProcessInfo.ThermalState {
    var name: String {
        switch self {
        case .nominal: "nominal"
        case .fair: "fair"
        case .serious: "serious"
        case .critical: "critical"
        @unknown default: "unknown"
        }
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

    /// Where a stopped recording saves its whole trace: the app's temporary
    /// directory, or a test's.
    static var traceDirectory = FileManager.default.temporaryDirectory

    /// Late frames kept in a report: some 64 KB at the 320 bytes one took in
    /// Lexy, small enough to paste into a conversation
    static let lateReported = 200

    /// A late commit presents its frame a whole period later for each
    /// period, or part of one, it is late
    static func commitDelay(overrun: Double, period: Double) -> Double {
        overrun > 0 && period > 0 ? period * (overrun / period).rounded(.up) : 0
    }

    /// Whether a recorded frame was late by any measure; a quiet gap owed no frame
    static func isLate(_ r: Record) -> Bool { (r.missed > 0 && !r.quiet) || r.overrun > 0 || r.screenLate != nil || r.unshown }

    /// A recording as an agent reads it: its segments by scroll phase and
    /// scroller, totals by phase, its late frames with what the main thread
    /// did in each, and what on screen costs the render server
    /// (`renderInventory`), when it was read.
    static func report(_ recording: Recording, identity: [String: Any], render: [String: Any]? = nil) -> [String: Any] {
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
        // Each frame's hitch: the latest of a callback the display link saw shown again (`interval −
        // period`; the link's jitter adds none), a late commit and how late it reached the screen, a period
        // when it never did, as the frame before stayed shown in its place. A late commit of a frame's own
        // update stalls the next callback too, so the gap of the frame right after it counts only what that
        // commit's delay did not. A commit counts in full: it is late against its own update's deadline,
        // which a stall before the update does not leave in the past
        //
        // Each late frame's cause is the part its hitch came from: `update`, a commit past its deadline
        // through the update's own work; `main`, one the main thread delayed, busy before the update
        // began (its `start` longer than its `work`) or through all of a gap but one period; `rate`, a gap
        // in which something moved, the main thread free for a period or more of it, and UIKit made no
        // update; `render`, a frame the probe saw reach the
        // screen late or never. A display link's callback gap is the main thread's
        var hitch: [Double] = [], causes: [String?] = [], carried = 0.0
        for (i, r) in records.enumerated() {
            if i > 0, abs(r.t - r.interval - records[i - 1].t) > 0.5 { carried = 0 }
            let commit = FrameSampler.commitDelay(overrun: r.overrun, period: r.period)
            let shownAgain = r.missed > 0 && !r.quiet ? max(0, r.interval - r.period - carried) : 0
            let screen = r.unshown ? r.period : r.screenLate ?? 0
            let h = max(shownAgain, commit, screen)
            hitch.append(h)
            causes.append(h <= 0 ? nil : h == commit ? (r.update.map { $0.start > $0.work } == true ? "main" : "update")
                          : h == shownAgain ? (r.update != nil && (r.main ?? 0) < r.interval - r.period ? "rate" : "main") : "render")
            carried = FrameSampler.commitDelay(overrun: r.ownOverrun, period: r.period)
        }

        struct Totals {
            var ms = 0.0, frames = 0, missed = 0, late = 0, hitch = 0.0, main = 0.0, worst = 0.0, maxMain = 0.0, probed = 0, unshown = 0
            var updates = 0, work = 0.0, maxWork = 0.0, commit = 0.0, own = 0.0, maxOwn = 0.0, causes: [String: Int] = [:]
        }
        func add(_ t: inout Totals, _ i: Int) {
            let r = records[i]
            // A quiet gap's frame stands for the one period it was shown in
            t.ms += r.quiet ? r.period : r.interval; t.frames += 1; t.missed += r.quiet ? 0 : r.missed; t.hitch += hitch[i]
            if FrameSampler.isLate(r) { t.late += 1 }
            if let cause = causes[i] { t.causes[cause, default: 0] += 1 }
            if r.probed { t.probed += 1 }
            if r.unshown { t.unshown += 1 }
            t.main += r.main ?? 0; t.worst = max(t.worst, r.quiet ? r.period : r.interval); t.maxMain = max(t.maxMain, r.main ?? 0)
            if let u = r.update {
                t.updates += 1; t.work += u.work; t.maxWork = max(t.maxWork, u.work); t.commit += r.commit ?? 0
                t.own += u.own; t.maxOwn = max(t.maxOwn, u.own)
            }
        }
        func json(_ t: Totals) -> [String: Any] {
            var o: [String: Any] = ["ms": r2(t.ms), "frames": t.frames, "missed": t.missed, "lateFrames": t.late, "hitchMs": r2(t.hitch),
             "hitchMsPerS": t.ms > 0 ? r2(t.hitch / (t.ms / 1000)) : 0, "worstIntervalMs": r2(t.worst),
             "meanMainMs": t.frames > 0 ? r2(t.main / Double(t.frames)) : 0, "maxMainMs": r2(t.maxMain), "probed": t.probed, "unshown": t.unshown,
             "hz": t.ms > 0 ? (Double(t.frames) / (t.ms / 1000)).rounded() : 0, "causes": t.causes]
            if t.updates > 0 {
                let n = Double(t.updates)
                o["meanWorkMs"] = r2(t.work / n); o["maxWorkMs"] = r2(t.maxWork); o["meanCommitMs"] = r2(t.commit / n)
                o["ownMeanMs"] = r2(t.own / n); o["ownMaxMs"] = r2(t.maxOwn)
            }
            return o
        }

        // Segments: runs of one phase on one scroller
        var segments: [[String: Any]] = [], byPhase: [String: Totals] = [:]
        var first = 0
        for i in records.indices {
            add(&byPhase[phases[i], default: Totals()], i)
            let last = i == records.count - 1 || phases[i + 1] != phases[i] || (phases[i] != "rest" && scrollers[i + 1] != scrollers[i])
            guard last else { continue }
            var o = json((first...i).reduce(into: Totals()) { add(&$0, $1) })
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

        var late = records.indices.filter { FrameSampler.isLate(records[$0]) }
        let lateCount = late.count
        if late.count > lateReported {
            late = Array(late.sorted { hitch[$0] > hitch[$1] }.prefix(lateReported)).sorted()
        }
        let lateJSON = late.map { i -> [String: Any] in
            var o = records[i].json
            o["hitchMs"] = r2(hitch[i])
            o["cause"] = causes[i]
            o["phase"] = phases[i]
            // Its scroller and that scroller's offset, whose phase `phase` names
            if phases[i] != "rest" { o["scroller"] = scrollers[i]; o["offset"] = records[i].scroll?.0 }
            o["scroll"] = nil
            o["target"] = nil
            return o
        }
        let all = records.indices.reduce(into: Totals()) { add(&$0, $1) }
        var out: [String: Any] = [
            "exactTrace": "recording",
            "identity": identity,
            "work": recording.work,
            "periodMs": records.map(\.period).sorted().dropFirst(records.count / 2).first.map(r2) ?? NSNull(),
            "recording": json(all).merging(["droppedFrames": recording.dropped]) { a, _ in a },
            "byPhase": byPhase.mapValues(json),
            "segments": segments,
            "late": lateJSON,
            "lateShown": "\(lateJSON.count) of \(lateCount)",
            "hatches": HatchDiagnostics.available ? "timed" : "not timed in a production build",
        ]
        if let render { out["render"] = render.merging(["columns": renderColumns]) { a, _ in a } }
        return out
    }
}

extension FrameSampler {
    /// The render inventory's columns (`renderInventory`, iOS and tvOS)
    static let renderColumns = ["kind", "place", "node", "top", "height", "count"]
    /// Late frames a compact copy keeps before the render inventory's rows
    static let compactLateFirst = 12
    static let compactTotals = ["frames", "ms", "hz", "missed", "lateFrames", "hitchMsPerS", "causes", "worstIntervalMs", "meanMainMs", "maxMainMs", "meanWorkMs", "maxWorkMs", "meanCommitMs", "probed", "unshown", "ownMeanMs", "ownMaxMs"]
    static let compactSegments = ["phase", "scroller", "from", "ms", "frames", "hz", "missed", "lateFrames", "hitchMsPerS", "causes", "worstIntervalMs", "meanMainMs", "maxMainMs", "meanWorkMs", "maxWorkMs", "scrolledPt"]
    static let compactLate = ["t", "phase", "cause", "hitchMs", "interval", "quiet", "missed", "overrun", "start", "events", "links", "work", "commit", "screenLate", "unshown", "main", "batches", "apply", "unattributed", "phases", "hatches", "phasesCoverage", "coverage", "scroller", "offset"]

    /// A trace as JSON with sorted keys and each number in its shortest form:
    /// `JSONSerialization` writes a `Double` with 17 significant digits (8.33
    /// as 8.3300000000000001), a third of a trace's bytes.
    static func traceJSON(_ object: Any) -> Data? {
        func shortest(_ value: Any) -> Any {
            switch value {
            case let d as Double where type(of: value) == Double.self && d.isFinite: return NSDecimalNumber(string: "\(d)")
            case let a as [Any]: return a.map(shortest)
            case let o as [String: Any]: return o.mapValues(shortest)
            default: return value
            }
        }
        return try? JSONSerialization.data(withJSONObject: shortest(object), options: [.sortedKeys])
    }

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
        func lateness(_ o: [String: Any]) -> Double { (o["hitchMs"] as? NSNumber)?.doubleValue ?? 0 }
        let identity = (report["identity"] as? [String: Any] ?? [:]).filter { ["version", "exactCommit", "device", "os", "production", "lowPowerMode", "thermal", "window"].contains($0.key) }
        let allSegments = (report["segments"] as? [[String: Any]] ?? []).filter { $0["phase"] as? String != "rest" }
        let worstSegments = allSegments.indices.sorted { (allSegments[$0]["hitchMsPerS"] as? Double ?? 0) > (allSegments[$1]["hitchMsPerS"] as? Double ?? 0) }
        let late = (report["late"] as? [[String: Any]] ?? []).sorted { lateness($0) > lateness($1) }
        let lateFrames = (report["recording"] as? [String: Any])?["lateFrames"] as? Int ?? late.count
        let render = report["render"] as? [String: Any], renderRows = render?["rows"] as? [[Any]] ?? []
        func build(segments m: Int, render k: Int = 0, late n: Int) -> Data {
            let segments = worstSegments.prefix(m).sorted().map { row(allSegments[$0], compactSegments) }
            var out: [String: Any] = [
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
                "shown": "\(segments.count) of \(allSegments.count) scroll segments, \(min(n, late.count)) of \(lateFrames) late frames",
                "full": file.map { "\($0) in the app's temporary directory, \(bytes) bytes" } ?? "not saved",
            ]
            if let render {
                out["render"] = ["layers": render["layers"] ?? 0, "columns": renderColumns, "rows": Array(renderRows.prefix(k)),
                                 "shown": "\(min(k, renderRows.count)) of \(renderRows.count)"] as [String: Any]
            }
            return traceJSON(out) ?? Data()
        }
        // The most of `count` that fits, at least `least`, each one more making the report larger
        func most(_ count: Int, least: Int = 0, _ size: (Int) -> Int) -> Int {
            var lo = min(least, count), hi = count
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
            return traceJSON(note) ?? Data()
        }
        // Every segment that fits, then the worst late frames, then what costs the render server, then more late frames
        let m = most(allSegments.count) { build(segments: $0, late: 0).count }
        let worst = most(min(late.count, compactLateFirst)) { build(segments: m, late: $0).count }
        let k = most(renderRows.count) { build(segments: m, render: $0, late: worst).count }
        return build(segments: m, render: k, late: most(late.count, least: worst) { build(segments: m, render: k, late: $0).count })
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
