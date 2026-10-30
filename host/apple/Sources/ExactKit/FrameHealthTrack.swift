import Foundation
import QuartzCore
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// An agent's scroll track (`perf frames track`, `exact.frame-health.track/1`): one node's scroller, each committed
/// offset change with its arrival time and phase, in a fixed ring, under a lease. It is fed only from the scroll
/// callbacks the host already has (iOS `scrollViewDidScroll`, the Mac clip's bounds change): no display link, timer
/// or observer of its own, so tracking a fling never changes how the app schedules frames. It exists only while an
/// agent leases it, where #1141's capture lease may be taken (`FrameHealth.enabled`, not a virtual clock).
final class FrameHealthTrack {
    static let capacity = 4096, maximumBytes = 4000, format = "exact.frame-health.track/1"
    /// A gap this long between two rows starts a new run: no comparable predecessor, interval 0 (as a sampler
    /// segment ended 500 ms after its last activity). So does a gap of more than two periods after a `t` or `-` row.
    static let quiet = 0.5
    struct Row { var t: Double; var offset: Float; var phase: UInt8 }

    let node: Int, began: Double, until: Double, period: Double
    private weak var scroller: AnyObject?
    private var rows: [Row]
    /// Rows written since the track began; row `n` is at `n % capacity` while retained.
    private(set) var written: UInt64 = 0
    /// Stopped: no feed writes again, and the ring stays readable by cursor until a read answers `more: false` (then it
    /// is released), its lease ends, or a new start replaces it.
    var frozen = false
    /// The last offset written, or the scroller's at start: a Mac bounds change that leaves the origin where it was (a
    /// resize at rest) writes nothing.
    var last: Double?

    init(node: Int, scroller: AnyObject, now: Double, durationMs: Double, maximumFramesPerSecond: Int) {
        self.node = node; self.scroller = scroller; began = now; until = now + durationMs / 1000
        period = 1 / Double(maximumFramesPerSecond > 0 ? maximumFramesPerSecond : 60)
        rows = Array(repeating: Row(t: 0, offset: 0, phase: 0), count: Self.capacity)
    }

    func tracks(_ scroll: AnyObject) -> Bool { scroller === scroll }

    /// A committed offset of the tracked scroller. Nothing allocates here.
    func record(_ offset: Double, phase: Character, at now: Double) {
        rows[Int(written % UInt64(Self.capacity))] = Row(t: now, offset: Float(offset), phase: phase.asciiValue ?? 45)
        last = offset
        written += 1
    }

    /// One page of rows from `cursor` (a row count; nil is row 0, the track's first), the oldest retained first, as
    /// many as fit `maximumBytes` with the carrier's newline. `dropped` counts rows overwritten before this read
    /// reached them.
    func page(from cursor: UInt64?, lease now: Double, extra: [String: Any] = [:]) -> [String: Any] {
        let oldest = written > UInt64(Self.capacity) ? written - UInt64(Self.capacity) : 0
        let asked = cursor ?? 0, start = max(asked, oldest), dropped = start - min(asked, start)
        var head: [String: Any] = ["format": Self.format, "node": node, "capacity": Self.capacity,
            "columns": ["t_us", "interval_us", "missed", "offset_cpt", "phase"],
            "period_us": Int((period * 1e6).rounded()), "dropped": dropped,
            "leaseRemainingMs": max(0, Int(((until - now) * 1000).rounded()))]
        head.merge(extra) { _, new in new }
        var body: [String] = [], next = start
        // The header's own bytes, with the room `rows`, `cursor` and `more` take.
        var used = (Self.json(head)?.utf8.count ?? 0) + 64
        while next < written {
            let row = rows[Int(next % UInt64(Self.capacity))]
            let previous: Row? = next > oldest && next > 0 ? rows[Int((next - 1) % UInt64(Self.capacity))] : nil
            var interval = previous.map { row.t - $0.t } ?? 0
            if interval >= Self.quiet || interval < 0 { interval = 0 }
            // A held finger (`t`) or a scroller at rest (`-`) moves nothing in a frame and writes no row: a gap after
            // one is a pause, not frames lost. Only motion UIKit or AppKit drives (`d`, `b`) is a frame-by-frame run.
            if let previous, previous.phase == 116 || previous.phase == 45, interval > 2 * period { interval = 0 }
            let missed = interval > 0 ? max(0, Int((interval / period).rounded()) - 1) : 0
            let text = "[\(Int(((row.t - began) * 1e6).rounded())),\(Int((interval * 1e6).rounded())),\(missed),\(Self.centipoints(row.offset)),\"\(Character(UnicodeScalar(row.phase)))\"]"
            if used + text.utf8.count + 1 > Self.maximumBytes { break }
            used += text.utf8.count + 1
            body.append(text)
            next += 1
        }
        head["cursor"] = String(next)
        head["more"] = next < written
        // `rows` is spliced in as written, so the page is measured as it goes out.
        guard var text = Self.json(head) else { return ["error": "track-encoding"] }
        text.removeLast()
        text += ",\"rows\":[" + body.joined(separator: ",") + "]}"
        return ["raw": text, "more": next < written]
    }

    /// An offset in hundredths of a point, clamped to JSON's safe integers (±(2^53 − 1)): any finite `Float` encodes.
    static func centipoints(_ offset: Float) -> Int {
        let limit = 9_007_199_254_740_991.0, value = (Double(offset) * 100).rounded()
        return value.isNaN ? 0 : Int(min(limit, max(-limit, value)))
    }

    static func json(_ object: [String: Any]) -> String? {
        (try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])).map { String(decoding: $0, as: UTF8.self) }
    }
}

extension FrameHealth {
    /// `perf frames track start|read|stop`. One track a session: a second `start` replaces the first (its unread
    /// rows go with it, and the reply says `replaced`). Allowed exactly where the capture lease is.
    func trackRequest(_ action: String, node: Int?, scroller: AnyObject?, durationMs: Double?, cursor: String?,
                      maximumFramesPerSecond: Int, now: Double = CACurrentMediaTime()) -> [String: Any] {
        expireTrack(at: now)
        switch action {
        case "start":
            let duration = durationMs ?? 30_000
            guard duration.isFinite, duration >= 1, duration <= 120_000, duration.rounded() == duration else {
                return ["error": "perf frames track start takes a node and a lease of 1–120000 ms."]
            }
            guard enabled, !virtual else {
                return ["format": FrameHealthTrack.format, "available": false, "track": false, "reason": unavailableCode ?? "virtual-clock"]
            }
            guard let node, let scroller else { return ["format": FrameHealthTrack.format, "error": "no-scroller", "reason": "The node has no native scroller to track."] }
            let replaced = track != nil
            let started = FrameHealthTrack(node: node, scroller: scroller, now: now, durationMs: duration, maximumFramesPerSecond: maximumFramesPerSecond)
            #if os(macOS)
            // The origin at start is the baseline: a first bounds change that leaves it (a resize at rest) writes nothing.
            if let scroll = scroller as? ChainingScrollView { started.last = Self.trackAxis(scroll).at }
            #endif
            track = started
            return ["format": FrameHealthTrack.format, "available": true, "track": true, "node": node, "replaced": replaced,
                "capacity": FrameHealthTrack.capacity, "leaseMs": Int(duration), "maximumReplyBytes": FrameHealthTrack.maximumBytes]
        case "read", "stop":
            guard let track else { return ["format": FrameHealthTrack.format, "track": false, "reason": "no-track"] }
            let asked = cursor.flatMap { UInt64($0) }
            if cursor != nil, asked == nil { return ["error": "perf frames track read: the cursor is a row count a read returned."] }
            // `stop` freezes the track: feeds end at once, and what is left is read to the end, page by page, from the
            // ring as it stands. The read that answers `more: false` releases it.
            if action == "stop" { track.frozen = true }
            var page = track.page(from: asked, lease: now, extra: track.frozen ? ["stopped": true] : [:])
            if track.frozen, page["more"] as? Bool == false { self.track = nil }
            page["more"] = nil
            return page
        default:
            return ["error": "perf frames track takes start, read or stop."]
        }
    }

    /// A lease ends at the first feed or read after its deadline, never by a timer; its rows go with it.
    func expireTrack(at now: Double) {
        if let track, now >= track.until { self.track = nil }
    }

    #if canImport(UIKit)
    /// The feed, from the host's existing `scrollViewDidScroll` calls: a nil check when no agent tracks.
    func trackScrolled(_ scroll: UIScrollView) {
        guard let track, track.tracks(scroll), !track.frozen else { return }
        guard enabled else { self.track = nil; return }
        let now = CACurrentMediaTime()
        if now >= track.until { self.track = nil; return }
        let (offset, phase) = scroll.scrollSample
        track.record(offset, phase: Character(phase), at: now)
    }
    #else
    /// The feed, from the Mac clip's existing bounds-change handler (`NodeView.clipScrolled`). The offset is along the
    /// scrolling axis (x for a scroller that moves only sideways, as `scrollSample` on iOS); the phase is the one the
    /// scroller's own wheel handler took from the event that moved it (`ChainingScrollView.trackPhase`), used once.
    func trackScrolled(_ scroll: ChainingScrollView) {
        guard let track, track.tracks(scroll), !track.frozen else { return }
        guard enabled else { self.track = nil; return }
        let now = CACurrentMediaTime()
        if now >= track.until { self.track = nil; return }
        let (at, range) = Self.trackAxis(scroll)
        guard track.last.map({ abs($0 - at) > 0.001 }) ?? true else { return }
        var phase = scroll.trackPhase
        scroll.trackPhase = "-"
        if at < -0.5 || at > max(0, Double(range)) + 0.5 { phase = "b" }
        track.record(at, phase: phase, at: now)
    }

    /// The scrolling axis's offset and range. The overflow flags decide when only one axis scrolls (a sideways
    /// scroller with taller content is still read along x); with both or neither, it is x only for a document wider
    /// than the clip that fits its height, as `scrollSample` on iOS.
    static func trackAxis(_ scroll: ChainingScrollView) -> (at: Double, range: Double) {
        let clip = scroll.contentView.bounds, document = scroll.documentView?.frame.size ?? .zero
        let sideways = scroll.scrollsX != scroll.scrollsY ? scroll.scrollsX
            : document.width > clip.width + 0.5 && document.height <= clip.height + 0.5
        return sideways ? (Double(clip.origin.x), Double(document.width - clip.width)) : (Double(clip.origin.y), Double(document.height - clip.height))
    }
    #endif
}
