import Foundation
import QuartzCore
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// A development session's presented frames (LLP 1079 D3–D4): a display
/// link of its own, run only inside active segments. It watches; it never
/// keeps the session's frame source (`Frames`) running, so the measured app
/// schedules frames as the shipped one does. A production bake has none.
///
/// A segment starts on activity — a batch applied, the session's frame
/// source running — and ends 500 ms after the last, or when the app leaves
/// the foreground or its window is hidden. Its first callback is the
/// baseline, never a sample. The period is the link's own
/// `targetTimestamp − timestamp`, as the canvases read it (`duration` is
/// wrong under ProMotion); `missed = round(interval / period) − 1`.
final class FrameSampler: NSObject {
    /// Whether this binary measures at all: not a production bake.
    static let measured = !ExactEnv.productionBake
    static let ring = 600, lateKept = 100, quiet = 0.5

    struct Record {
        let t: Double, interval: Double, missed: Int, seq: (UInt64, UInt64)?, batches: Int, apply: Double
        var json: [String: Any] {
            var o: [String: Any] = ["t": t, "interval": interval, "missed": missed, "batches": batches, "apply": apply]
            o["seq"] = seq.map { [$0.0, $0.1] } ?? NSNull()
            return o
        }
    }

    weak var session: ExactSession?
    private var link: CADisplayLink?
    private var last: CFTimeInterval?
    private var active = -Double.infinity
    private var period: Double?
    private var records: [Record] = [], late: [Record] = []
    private var dropped = 0, presented = 0, lateCount = 0, missed = 0, segments = 0
    private var pending: (first: UInt64?, last: UInt64?, batches: Int, apply: Double) = (nil, nil, 0, 0)
    private var observers: [NSObjectProtocol] = []

    init(session: ExactSession) {
        self.session = session
        super.init()
        let center = NotificationCenter.default
        #if canImport(UIKit)
        observers.append(center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in self?.stop() })
        #else
        observers.append(center.addObserver(forName: NSWindow.didChangeOcclusionStateNotification, object: nil, queue: .main) { [weak self] note in
            if let window = note.object as? NSWindow, window === self?.session?.presenter.viewport.window, !window.occlusionState.contains(.visible) { self?.stop() }
        })
        #endif
    }

    deinit { for o in observers { NotificationCenter.default.removeObserver(o) } }

    /// A batch the session applied: its transactions (the trailer's `seq`)
    /// and the time applying it took, joined to the next sample.
    func batch(_ seq: (UInt64, UInt64)?, ms: Double) {
        // A nested apply reports before the batch around it: the range is the span.
        if let seq { pending.first = min(pending.first ?? seq.0, seq.0); pending.last = max(pending.last ?? seq.1, seq.1) }
        pending.batches += 1
        pending.apply += ms
        activity()
    }

    /// Something visible may be happening: open a segment, or keep it open.
    func activity() {
        guard let session, session.clock == nil, session.state != .destroyed else { return }
        active = CACurrentMediaTime()
        guard link == nil else { return }
        #if canImport(UIKit)
        let l = CADisplayLink(target: self, selector: #selector(tick(_:)))
        #else
        let viewport = session.presenter.viewport
        guard viewport.window?.occlusionState.contains(.visible) == true else { return }
        let l = viewport.displayLink(target: self, selector: #selector(tick(_:)))
        #endif
        l.add(to: .main, forMode: .common)
        link = l
        last = nil
        segments += 1
    }

    /// Forget every sample, and the link's baseline: a replaced runner
    /// numbers its transactions afresh. The next activity opens a segment.
    func reset() {
        stop()
        records = []; late = []
        dropped = 0; presented = 0; lateCount = 0; missed = 0; segments = 0
        pending = (nil, nil, 0, 0)
        period = nil
    }

    func stop() {
        link?.invalidate()
        link = nil
        last = nil
    }

    @objc func tick(_ link: CADisplayLink) {
        observe(now: link.timestamp, target: link.targetTimestamp)
        // The session's own frame source running is activity too.
        if session?.frames.link != nil { active = CACurrentMediaTime() }
        if CACurrentMediaTime() - active > Self.quiet { stop() }
    }

    /// One display callback: its time and the next frame's target, seconds.
    func observe(now: CFTimeInterval, target: CFTimeInterval) {
        period = (target - now) * 1000
        if let last { sample(interval: (now - last) * 1000, at: now) }
        last = now
    }

    private func sample(interval: Double, at now: CFTimeInterval) {
        let r2 = { (x: Double) in (x * 100).rounded() / 100 }
        let p = period ?? interval
        let missedHere = max(0, Int((interval / p).rounded()) - 1)
        let seq = pending.first.flatMap { first in pending.last.map { (first, $0) } }
        let record = Record(t: r2((now - ExactEnv.t0) * 1000), interval: r2(interval), missed: missedHere, seq: seq, batches: pending.batches, apply: r2(pending.apply))
        pending = (nil, nil, 0, 0)
        records.append(record)
        if records.count > Self.ring { records.removeFirst(); dropped += 1 }
        presented += 1
        guard missedHere > 0 else { return }
        lateCount += 1
        missed += missedHere
        late.append(record)
        if late.count > Self.lateKept { late.removeFirst() }
        let range = seq.map { " seq \($0.0)..\($0.1)" } ?? ""
        session?.log("frame late at \(record.t): \(missedHere) missed (\(record.interval) ms / \(r2(p)) target)\(range) apply \(record.apply)")
    }

    /// `{"op":"perf","frames":true[,"late":N]}` (D4): lifetime counters, the
    /// ring's percentiles with the range they cover, the most recent late
    /// records; `all` adds every retained record (a trace, D5).
    func reply(late n: Int = 20, all: Bool = false) -> [String: Any] {
        let xs = records.map(\.interval).sorted()
        let q = { (f: Double) -> Any in xs.isEmpty ? NSNull() : xs[min(xs.count - 1, Int(f * Double(xs.count)))] }
        var out: [String: Any] = [
            "period": ["ms": period.map { ($0 * 100).rounded() / 100 } ?? NSNull(), "source": "target"] as [String: Any],
            "covers": ["batches", "frame-source"],
            "lifetime": ["presented": presented, "late": lateCount, "missed": missed, "segments": segments],
            "window": ["from": records.first?.t ?? NSNull(), "to": records.last?.t ?? NSNull(), "samples": records.count, "dropped": dropped,
                       "p50": q(0.5), "p95": q(0.95), "p99": q(0.99), "max": xs.last ?? NSNull()] as [String: Any],
            "late": late.suffix(max(0, min(Self.lateKept, n))).map(\.json),
        ]
        if all { out["records"] = records.map(\.json) }
        return out
    }
}

extension ExactSession {
    /// Save Trace (LLP 1079 D5): this session's journal, its presented frames
    /// and `perf` over every root, with who made them and each timing's
    /// proxy, as `trace-<wallclock>.json` in the app's temporary directory;
    /// `agent.mjs trace <file>` reads it back. The path, or why not.
    public func saveTrace() -> Result<URL, Error> {
        guard let sampler else { return .failure(TraceError.production) }
        let raw = { (json: String) in (try? JSONSerialization.jsonObject(with: Data(json.utf8))) ?? ["error": "unreadable"] }
        let perf = raw(agent("{\"op\":\"perf\"}"))
        #if canImport(UIKit)
        let host = "ios", device = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"].map { "\($0) (simulator)" } ?? UIDevice.current.model
        #else
        let host = "macos", device = Host.current().localizedName ?? "Mac"
        #endif
        let bundle = Bundle.main.bundleIdentifier ?? ProcessInfo.processInfo.processName
        let trace: [String: Any] = [
            "identity": ["host": host, "app": bundle.split(separator: ".").last.map(String.init) ?? bundle, "bundle": bundle, "trust": "development",
                         "os": ProcessInfo.processInfo.operatingSystemVersionString, "device": device,
                         "bootWall": ((Date().timeIntervalSince1970 - (CACurrentMediaTime() - ExactEnv.t0)) * 1000).rounded()],
            "proxies": ["period": "target: the sampler's own display link, targetTimestamp − timestamp",
                        "interval": "the sampler's display-link callback gap", "covers": ["batches", "frame-source"], "loaf": "none on this host"],
            "plan": (perf as? [String: Any])?["plan"] ?? NSNull(),
            "journal": raw(agent("{\"op\":\"logs\",\"since\":0}")),
            "frames": sampler.reply(late: FrameSampler.lateKept, all: true),
            "perf": perf,
        ]
        do {
            let stamp = ISO8601DateFormatter().string(from: Date()).replacingOccurrences(of: ":", with: "-")
            let url = FileManager.default.temporaryDirectory.appendingPathComponent("trace-\(stamp).json")
            let data = try JSONSerialization.data(withJSONObject: trace)
            try data.write(to: url)
            // The last one under a name the driver knows, so a phone's is
            // copied off by name (`agent.mjs trace --phone`), as its
            // screenshots are: devicectl lists no app's files.
            try data.write(to: FileManager.default.temporaryDirectory.appendingPathComponent(Self.latestTrace), options: .atomic)
            log("trace saved: \(url.path)")
            FileHandle.standardError.write(Data("exact: trace saved to \(url.path)\n".utf8))
            return .success(url)
        } catch {
            log("trace refused: \(error.localizedDescription)")
            return .failure(error)
        }
    }

    enum TraceError: Error { case production }
    /// The last saved trace, beside the stamped one (`agent.mjs` reads the name).
    static let latestTrace = "trace-latest.json"
}
