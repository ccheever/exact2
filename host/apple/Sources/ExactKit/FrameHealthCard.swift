import Foundation
import QuartzCore
import CExact

/// The card's reader (LLP 1079.000 §5.3, §7.2). It samples on the collector's own publications,
/// at most twice a second, keeps at most 60 samples over 30 seconds and takes the per-read maxima,
/// which no other reader does. It reads nothing while the card is hidden or paused.
final class FrameHealthCardReader {
    /// Whether this card holds one of `FrameHealthReaders`' places, and reads
    private(set) var reading = false

    /// Take a place, starting from no samples; false when four other readers hold them.
    func start() -> Bool {
        guard !reading else { return true }
        guard FrameHealthReaders.claim() else { return false }
        reading = true
        reset()
        samples.reserveCapacity(60)
        return true
    }

    /// Give the place back and free the samples' storage, which the limit counts.
    func stop() {
        guard reading else { return }
        FrameHealthReaders.release()
        reading = false
        samples = []
        previous = nil
        overdue = 0
        stale = false
    }

    deinit { if reading { FrameHealthReaders.release() } }

    /// What changed between two reads, `from` and `read`, with the exact maxima taken at the second.
    /// A half second holds a few hundred frames at most, so its bins keep 16 bits.
    struct Sample {
        var from = 0.0, read = 0.0
        var running = false
        var periodNs: UInt64 = 0
        var counts = Counts()
        /// Milliseconds: the longest frame interval, and the slowest work with its own budget
        var longest = 0.0, slowest = 0.0, budget = 0.0
        /// Frame intervals (64), work (64), refreshes (32) and lateness (32), in that order
        var bins = [UInt16](repeating: 0, count: 192)
        /// False when a bin's count did not fit 16 bits
        var valid = true

        var fps: Double? { counts.fps }
    }

    /// Counts a window adds from its samples. Thirty seconds of them fit 32 bits, which keeps the
    /// card within its memory budget (LLP 1079.000 §5.2); only the time observed needs 64.
    struct Counts {
        var activeNs: UInt64 = 0
        var frames: UInt32 = 0, callbacks: UInt32 = 0, held: UInt32 = 0, changes: UInt32 = 0, epochs: UInt32 = 0
        var unclassified: UInt32 = 0, outOfOrder: UInt32 = 0, skipped: UInt32 = 0
        var requested: UInt32 = 0, missed: UInt32 = 0, restricted: UInt32 = 0, worked: UInt32 = 0, late: UInt32 = 0
        var fps: Double? { activeNs == 0 ? nil : Double(frames) * 1e9 / Double(activeNs) }
        mutating func add(_ c: Counts) {
            activeNs += c.activeNs; frames += c.frames; callbacks += c.callbacks; held += c.held; changes += c.changes
            epochs += c.epochs; unclassified += c.unclassified; outOfOrder += c.outOfOrder; skipped += c.skipped
            requested += c.requested; missed += c.missed; restricted += c.restricted; worked += c.worked; late += c.late
        }
    }

    /// The samples that began within a span, added together.
    struct Window {
        var from = 0.0, read = 0.0
        var running = false
        var periodNs: UInt64 = 0
        var counts = Counts()
        var longest = 0.0, slowest = 0.0, budget = 0.0
        var intervals = [UInt32](repeating: 0, count: 64), work = [UInt32](repeating: 0, count: 64)
        var refreshes = [UInt32](repeating: 0, count: 32), lateBy = [UInt32](repeating: 0, count: 32)
        var valid = true

        var frames: UInt64 { UInt64(counts.frames) }
        var fps: Double? { counts.fps }
    }

    private(set) var samples: [Sample] = []
    private var previous: FrameHealthMetrics.Sample?
    /// At the newest read, as the collector's own reply takes them: frames past their deadline still
    /// pending, and whether a running source has sent nothing for a second
    private(set) var overdue: UInt64 = 0, stale = false

    /// Forget every sample: on Hide, a new generation or a reset.
    func reset() { samples.removeAll(keepingCapacity: true); previous = nil; overdue = 0; stale = false }

    /// Read the collector once at least half a second has passed. The first read after a reset is
    /// only a baseline, and its maxima cover time the card did not watch. A new generation discards
    /// the samples at once, without waiting for the half second.
    @discardableResult
    func sample(_ health: FrameHealth, now: Double = CACurrentMediaTime(), force: Bool = false) -> Bool {
        guard health.enabled else { return false }
        if let previous, previous.snapshot.generation != health.snapshot().generation { reset() }
        if let last = samples.last, !force, now - last.read < 0.5 { return false }
        let current = FrameHealthMetrics.read(health, now: now)
        let maxima = exact_frame_health_take_maxima(health.handle)
        defer { previous = current }
        guard let previous, previous.snapshot.generation == current.snapshot.generation else {
            samples.removeAll(keepingCapacity: true)
            return false
        }
        let a = previous.snapshot, b = current.snapshot
        var s = Sample(from: Double(previous.read) / 1e9, read: now, running: b.running != 0, periodNs: b.period_ns)
        func delta(_ path: KeyPath<ExactFrameHealthSnapshot, UInt64>) -> UInt32 { UInt32(clamping: b[keyPath: path] &- a[keyPath: path]) }
        s.counts = Counts(activeNs: b.active_ns &- a.active_ns, frames: delta(\.intervals), callbacks: delta(\.callbacks),
            held: delta(\.held), changes: delta(\.pacing_changes), epochs: delta(\.schedule_epochs),
            unclassified: delta(\.unclassified_intervals), outOfOrder: delta(\.out_of_order), skipped: delta(\.skipped_refreshes),
            requested: delta(\.requested_slots), missed: delta(\.missed_frames), restricted: delta(\.restricted_intervals),
            worked: delta(\.worked), late: delta(\.late))
        overdue = UInt64(health.pendingFrames(at: Double(current.read) / 1e9).overdue)
        stale = b.running != 0 && b.last_arrival_ns != 0 && current.read > b.last_arrival_ns + 1_000_000_000
        s.longest = Double(maxima.longest_ns) / 1e6
        s.slowest = Double(maxima.slowest_ns) / 1e6
        s.budget = Double(maxima.budget_ns) / 1e6
        // A saturated bin stops counting while its population goes on, so its deltas are not a distribution
        s.valid = b.histogram_overflows == a.histogram_overflows
        var index = 0
        for (now, before) in [(current.intervals, previous.intervals), (current.work, previous.work),
                              (current.refreshes, previous.refreshes), (current.lateBy, previous.lateBy)] {
            for (n, b) in zip(now, before) {
                let delta = n &- b
                s.valid = s.valid && delta <= UInt32(UInt16.max)
                s.bins[index] = UInt16(clamping: delta)
                index += 1
            }
        }
        // Room first, so the storage stays the 60 samples `start` reserved
        while let first = samples.first, samples.count >= 60 || now - first.read > 30 { samples.removeFirst() }
        samples.append(s)
        return true
    }

    /// The samples that began within the last `seconds`, added together, and always the newest; the
    /// last 0.5 s is the newest alone. Reads follow publications, which a stall or an idle spell
    /// delays, so a window's `from` gives the time it actually covers.
    func window(_ seconds: Double) -> Window {
        var total = Window()
        guard let newest = samples.last else { return total }
        total.read = newest.read; total.from = newest.from; total.running = newest.running; total.periodNs = newest.periodNs
        let span = seconds <= 0.5 ? 0 : seconds + 0.1
        for s in samples.reversed() where s.read == newest.read || newest.read - s.from <= span {
            total.from = min(total.from, s.from)
            total.counts.add(s.counts)
            total.longest = max(total.longest, s.longest)
            if s.slowest > total.slowest { total.slowest = s.slowest; total.budget = s.budget }
            total.valid = total.valid && s.valid
            for i in 0..<64 { total.intervals[i] += UInt32(s.bins[i]); total.work[i] += UInt32(s.bins[64 + i]) }
            for i in 0..<32 { total.refreshes[i] += UInt32(s.bins[128 + i]); total.lateBy[i] += UInt32(s.bins[160 + i]) }
        }
        return total
    }
}

/// The card's figures, words and the text Share sends, common to iOS and macOS. Numbers follow LLP 1079.000
/// §7.2: no space before a unit, no "<", "average" or middots, and "—" where there is no value.
enum FrameHealthCardText {
    struct Display { var name: String, detail: String, maximumHz: Double, adaptiveSync: Bool }

    /// What the app asked for and what the platform scheduled, from the newest sample.
    struct Rates {
        var requestedHz: Double
        var periodMs: Double
        var scheduleHz: Double { periodMs > 0 ? 1000 / periodMs : 0 }
        var limited: Bool { requestedHz > 0 && scheduleHz > 0 && scheduleHz.rounded() < requestedHz.rounded() }
    }

    static let workLayout = FrameHealthMetrics.Layout.frames, lateLayout = FrameHealthMetrics.Layout.late
    static let rows: [(String, KeyPath<Figures, String>)] = [("Frame rate", \.fps), ("Missed frames", \.missed),
        ("Work time, median", \.median), ("Max work time", \.maxWork), ("Late frames", \.late),
        ("Late by, median", \.lateBy), ("Max frame time", \.maxFrame), ("Pacing changes", \.changes)]

    struct Figures { var fps, missed, median, maxWork, late, lateBy, maxFrame, changes: String }

    /// The figures for one window. `count` abbreviates large counts on the card; the shared text gives them in full.
    static func figures(_ t: FrameHealthCardReader.Window, rates: Rates, adaptiveSync: Bool,
                        count: (UInt64) -> String = abbreviated) -> Figures {
        // Each figure has its own population: a window can hold misses or late work without a
        // completed frame interval, as when a run's end finalizes a held callback's slot
        let c = t.counts, lateness = FrameHealthAssessment.deadlinesObserved
        return Figures(
            fps: t.fps.map { "\(Int($0.rounded())) FPS" } ?? "—",
            missed: c.requested > 0 ? count(UInt64(c.missed)) : "—",
            median: t.valid ? medianWork(t) : "—",
            maxWork: t.slowest > 0 ? duration(t.slowest, .nearest) : "—",
            late: c.worked > 0 && lateness ? count(UInt64(c.late)) : "—",
            lateBy: lateness && t.valid ? durationText(lateLayout, FrameHealthMetrics.bin(t.lateBy, fraction: 0.5)) : "—",
            maxFrame: t.longest > 0 ? duration(t.longest, .nearest) : "—",
            changes: c.frames > 0 && !adaptiveSync ? count(UInt64(c.changes)) : "—")
    }

    /// The median work time: its bin's upper bound, but never more than the exact maximum it cannot
    /// exceed, which then reads as the maximum does.
    static func medianWork(_ t: FrameHealthCardReader.Window) -> String {
        guard let bin = FrameHealthMetrics.bin(t.work, fraction: 0.5) else { return "—" }
        let (lower, upper) = workLayout.bounds(bin)
        guard let upper else { return "≥" + duration(Double(lower) / 1e6, .down) }
        let edge = Double(upper) / 1e6
        return t.slowest > 0 && t.slowest < edge ? duration(t.slowest, .nearest) : duration(edge, .up)
    }

    /// A window's name: its nominal span, or the span it actually covers when that differs by more
    /// than a tenth and the window has frames whose figures the span qualifies.
    static func window(_ t: FrameHealthCardReader.Window, nominal: Double) -> String {
        let span = t.read - t.from
        guard t.read > 0, t.frames > 0, abs(span - nominal) > nominal / 10 else { return nominal < 1 ? "Last 0.5s" : "Last \(Int(nominal))s" }
        return "Last " + (span < 10 ? String(format: "%.1fs", span) : "\(Int(span.rounded()))s")
    }

    /// Pacing over a window: a verdict, then the counts behind it. Counts, not rounded shares, so a
    /// rare long frame never reads as 0%. Refresh counts are in each frame's own schedule, so a
    /// window whose schedule changed gives them without a rate or a time.
    static func pacing(_ t: FrameHealthCardReader.Window, rates: Rates, adaptiveSync: Bool,
                       count: (UInt64) -> String = abbreviated) -> (String, String) {
        guard t.frames > 0 else { return ("No frames", "") }
        guard t.valid else { return ("—", "") }
        if adaptiveSync {
            let n = t.intervals.reduce(0) { $0 + UInt64($1) }
            guard n >= 100, let bin = FrameHealthMetrics.bin(t.intervals, fraction: 0.99) else {
                return ("VRR", "\(count(n)) frames, too few for a 99th percentile")
            }
            let (lower, upper) = workLayout.bounds(bin)
            return ("VRR", upper.map { "99% of frames under \(duration(Double($0) / 1e6, .up))" }
                ?? "Over 1% of frames \(duration(Double(lower) / 1e6, .down)) or longer")
        }
        let held = t.refreshes.indices.filter { t.refreshes[$0] > 0 }.sorted { t.refreshes[$0] > t.refreshes[$1] }
        // Refresh counts from different schedules convert to no single time, and an interval across a
        // change has none: the verdict says the schedule changed, with the frames across the change
        let changed = t.counts.epochs > 0 || t.counts.unclassified > 0 || rates.periodMs == 0
        let verdict = "Schedule changed" + (t.counts.unclassified > 0 ? ", \(count(UInt64(t.counts.unclassified))) frames across" : "")
        guard let most = held.first else { return (verdict, "") }
        // The last bin holds every frame over 31 refreshes
        func time(_ i: Int) -> String {
            let refreshes = min(i, 30) + 1
            let value = changed ? (refreshes == 1 ? "1 refresh" : "\(refreshes) refreshes") : ms(Double(refreshes) * rates.periodMs)
            return i == 31 ? "over " + value : value
        }
        let first = "\(count(UInt64(t.refreshes[most]))) frames at \(time(most))"
        let rest = held.dropFirst()
        guard let low = rest.min(), let high = rest.max() else {
            return (changed ? verdict : most == 31 ? "Stalled" : most == 0 ? "Even at \(Int(rates.scheduleHz.rounded())) FPS"
                : "Locked at \(Int((rates.scheduleHz / Double(most + 1)).rounded())) FPS", first)
        }
        let others = rest.reduce(0) { $0 + UInt64(t.refreshes[$1]) }
        let range = low == high ? time(low) : high == 31 ? "\(time(low)) or more"
            : changed ? "\(low + 1) to \(time(high))" : "\(time(low)) to \(time(high))"
        return (changed ? verdict : "Uneven", "\(first), \(count(others)) at \(range)")
    }

    /// The deadline is one scheduled refresh after each frame starts; `varied` says the schedule
    /// changed in the window, so the current value is not every frame's.
    static func deadline(_ rates: Rates, varied: Bool = false) -> String {
        guard rates.periodMs > 0 else { return "Deadline: —" }
        return "Deadline: one refresh, \(varied ? "now " : "")\(ms(rates.periodMs)) (\(Int(rates.scheduleHz.rounded())) FPS)"
    }

    static func request(_ rates: Rates) -> (String, String) {
        guard rates.requestedHz > 0 else { return ("No rate requested", "") }
        return ("Requested \(Int(rates.requestedHz.rounded())) FPS",
            rates.limited ? "system limit \(Int(rates.scheduleHz.rounded())) FPS" : "no limit")
    }

    /// The shared summary, in the parts the image draws too.
    struct Summary {
        var title: String, context: [String], figures: [String], results: [String], notes: [String]
        var text: String { ([title] + context + figures + results + notes).joined(separator: "\n") }
    }

    /// The shared summary: every figure with its window, leaving out windows without one, then the
    /// results Details gives.
    static func summary(reader: FrameHealthCardReader, rates: Rates, display: Display, paused: Bool, health: FrameHealth) -> Summary {
        let windows = [(0.5, reader.window(0.5)), (5, reader.window(5)), (30, reader.window(30))]
            .map { (window($0.1, nominal: $0.0).lowercased(), $0.1) }
        let figures = windows.map { figures($0.1, rates: rates, adaptiveSync: display.adaptiveSync, count: { String($0) }) }
        let (verdict, detail) = pacing(windows[1].1, rates: rates, adaptiveSync: display.adaptiveSync, count: { String($0) })
        return Summary(title: "Frame health, \(paused ? "paused" : "live")",
            context: [rates.requestedHz > 0
                    ? "Requested rate: \(Int(rates.requestedHz.rounded())) FPS" + (rates.limited ? ", limited by the system to \(Int(rates.scheduleHz.rounded())) FPS" : "")
                    : "Requested rate: none",
                "Display: \(display.name), \(display.detail)", deadline(rates, varied: windows[2].1.counts.epochs > 0),
                "Pacing (\(windows[1].0)): " + [verdict, detail].filter { !$0.isEmpty }.joined(separator: "; ")],
            figures: rows.map { name, key in
                let values = zip(figures, windows).filter { $0.0[keyPath: key] != "—" }.map { "\($0.0[keyPath: key]) (\($0.1.0))" }
                return "\(name): " + (values.isEmpty ? "—" : values.joined(separator: ", "))
            },
            results: results(reader: reader, health: health),
            notes: ["Frames (\(windows[2].0)): \(windows[2].1.frames)",
                "Not measured: when pictures appeared on screen, Core Animation's commit, compositor and GPU time, input latency"])
    }

    /// A row of Details: a result with its reason, or a count.
    struct Detail { var name: String, value: String, reason: String? = nil }

    /// Details: the collector's three results over the last 5s, each with its reason, by the
    /// collector's own rules (`FrameHealthAssessment`), then the counts behind them.
    static func details(reader: FrameHealthCardReader, health: FrameHealth) -> [Detail] {
        let t = reader.window(5), c = t.counts
        // The deltas the collector's reply assesses, laid out as `FrameHealthMetrics.Sample.values`
        var delta = [UInt64](repeating: 0, count: 17)
        for (index, value) in [(0, c.callbacks), (1, c.held), (2, c.frames), (6, c.worked), (7, c.late), (11, c.requested), (12, c.missed),
                               (13, c.restricted), (14, c.skipped), (15, c.unclassified), (16, c.outOfOrder)] { delta[index] = UInt64(value) }
        delta[3] = c.activeNs
        let reply = FrameHealthAssessment.reply(.init(delta, scheduledPeriodNs: t.periodNs, running: t.running, overdue: reader.overdue,
            stale: reader.stale), health: health)
        let results = [("Cadence", "cadence"), ("Target rate", "target"), ("Content latency", "contentLatency")].map { name, key in
            let r = reply[key] as? [String: Any] ?? [:]
            return Detail(name: name, value: result(r["result"] as? String), reason: reason(r["reason"] as? String))
        }
        // Where deadlines carry no information, lateness is unknown, not zero
        let late = FrameHealthAssessment.deadlinesObserved ? String(c.late) : "—"
        return results + [("Frames", String(c.frames)), ("Requested slots", String(c.requested)), ("Missed frames", String(c.missed)),
            ("Skipped refreshes", String(c.skipped)), ("Late frames", late), ("Limited by the system", String(c.restricted))]
            .map { Detail(name: $0, value: $1) }
    }

    /// Details' caption: the window its rows cover, and what Exact does not observe.
    static func detailsCaption(reader: FrameHealthCardReader) -> String {
        window(reader.window(5), nominal: 5) + ". Exact does not observe presentation, compositor or GPU time, or input latency."
    }

    /// Each result with its window, as the shared text gives them.
    static func results(reader: FrameHealthCardReader, health: FrameHealth) -> [String] {
        let span = window(reader.window(5), nominal: 5).lowercased()
        return details(reader: reader, health: health).prefix(3).map { "\($0.name) (\(span)): \($0.value)" + ($0.reason.map { ", " + $0 } ?? "") }
    }

    /// A result in the card's words: the collector's "proxy-pass" reads "Pass by proxy", and every
    /// result short of evidence "Not enough evidence".
    private static func result(_ code: String?) -> String {
        switch code {
        case "proxy-pass": "Pass by proxy"
        case "fail": "Fail"
        case "restricted": "Limited"
        case "not-applicable": "Not applicable"
        default: "Not enough evidence"
        }
    }

    private static func reason(_ code: String?) -> String? {
        switch code {
        case "unknown": "cause unknown"
        case "content-lateness-unobserved": "not observed"
        case "no-callbacks": "no frames"
        case "no-rate-request": "no rate requested"
        case "stale": "no recent frames"
        case let code?: code.replacingOccurrences(of: "-", with: " ")
        case nil: nil
        }
    }

    /// The definitions sheet: each figure and what Exact does not measure.
    static let definitions: [(String, String)] = [
        ("Frame rate (FPS)", "Frames Exact produced per second, over the last 0.5s and over the last 5s. Both are averages over their window; the chart plots the average over the last 0.5s at the newest frame of each tenth of a second. A frame is a display-link callback that ran Exact's work; a callback that had to wait for earlier work still in flight is not a frame."),
        ("Requested rate and limits", "The rate the app asked the system for. Moving content asks for the display's maximum; a simple fade asks for 60 FPS. \"System limit\" means the system scheduled fewer frames than requested, for example in Low Power Mode."),
        ("Displays", "Fixed refresh redraws the screen at one rate. Adaptive refresh, which ProMotion uses, has the system choose the rate from a set to suit the content and the app's request; frames still land on that rate's refreshes. Variable refresh rate (VRR: VESA Adaptive-Sync) refreshes the screen when each frame is ready, within the display's range."),
        ("Missed frames", "Frames the app asked for but did not produce in time. With fixed or adaptive refresh the previous picture stays up for another refresh; on a VRR display the screen waits for the late frame. Either way the frame is missed, so it counts the same on every display."),
        ("Frame time and work time", "Frame time is the time from one frame to the next, from the display's frame timestamps; Exact does not see when a picture reaches the screen. Work time is how long Exact took to make a frame: from its start until its callback returned and the work it sent to Exact's runtime was applied. Work that finishes after the frame's deadline makes a late frame. Exact records work time in 0.5ms steps below 24ms, and the table rounds each figure up to its step."),
        ("Late by", "How far past the deadline the median late frame finished. It tells near misses from severe ones: at 120 Hz, work of 10ms is late by 1.7ms and work of 15ms by 6.7ms."),
        ("Pacing", "Whether frames come at equal intervals. \"Even\" means every frame came one refresh after the last. \"Locked\" means every frame came the same longer time after the last, such as 16.7ms for 60 FPS on a 120 Hz screen: smooth, at a lower rate. \"Uneven\" gives the most common frame time and the rest; frames alternating between two times look like judder even when the rate is high. On a VRR display frame times vary with the work, so pacing gives the time 99% of frames came within."),
        ("Work time, median and max", "The median is the middle frame's work time, rounded up to its 0.5ms step. The max is the slowest frame's, exact."),
        ("Max frame time", "The longest time between two frames: the worst hitch, exact from the display's frame timestamps."),
        ("Pacing changes", "How often a frame's time differed from the frame before. Alternating frame times change on almost every frame; a single hitch changes twice."),
        ("Not measured", "When a picture actually appeared on screen, Core Animation's commit after Exact's work, compositor and GPU time, and input latency. Exact sees when frames are scheduled and when its own work finishes. On the iOS simulator callbacks arrive after their own deadline, so there late frames are unknown."),
    ]

    enum Rounding { case up, down, nearest }

    /// About three significant figures and at most one decimal, in seconds from one second.
    static func duration(_ milliseconds: Double, _ rounding: Rounding) -> String {
        let (amount, unit) = milliseconds < 1000 ? (milliseconds, "ms") : (milliseconds / 1000, "s")
        let places = amount > 0 ? min(1, max(0, 2 - Int(floor(log10(amount))))) : 1
        let scaled = amount * pow(10, Double(places))
        // Allow for binary fractions such as 10.500000001, so an exact edge keeps its value
        let rounded = switch rounding {
        case .up: ceil(scaled - 1e-9)
        case .down: floor(scaled + 1e-9)
        case .nearest: scaled.rounded()
        }
        return String(format: "%.\(places)f", rounded / pow(10, Double(places))) + unit
    }

    /// A duration from a bin: its upper bound rounded up, so a figure never understates; the open
    /// last bin reads as at least its lower bound.
    static func durationText(_ layout: FrameHealthMetrics.Layout, _ bin: Int?) -> String {
        guard let bin else { return "—" }
        let (lower, upper) = layout.bounds(bin)
        return upper.map { duration(Double($0) / 1e6, .up) } ?? "≥" + duration(Double(lower) / 1e6, .down)
    }

    static func ms(_ value: Double) -> String { String(format: "%.1fms", value) }

    static func abbreviated(_ n: UInt64) -> String { n < 1000 ? String(n) : n < 1_000_000 ? "\(n / 1000)k" : "1M+" }
}

/// What the card shows, built once per refresh from the reader and used for its drawing,
/// accessibility and Share alike, on iOS and macOS.
struct FrameHealthCardPresentation {
    typealias T = FrameHealthCardText
    var status = FrameHealthCardFace.Status.idle
    var request = ("No rate requested", ""), display = ""
    var rows = [("Last 0.5s", "—", "—"), ("Last 5s", "—", "—")]
    var pacing = ("—", ""), pacingWindow = "last 5s"
    var input: FrameHealthCardDrawing.Input?
    var columns: [String] = [], table: [(String, [String])] = [], caption = ""
    var rateSummary = "Frame rate", timeSummary = "Frame time"
    var details: [T.Detail] = [], detailsCaption = ""

    init() {}

    /// `details` asks for Details' rows too, which only a card that shows them needs.
    init(reader: FrameHealthCardReader, rates: T.Rates, display: T.Display, status: FrameHealthCardFace.Status,
         unavailable: String?, health: FrameHealth, details: Bool = false) {
        let now = reader.window(0.5), five = reader.window(5), thirty = reader.window(30)
        if details {
            self.details = T.details(reader: reader, health: health)
            detailsCaption = T.detailsCaption(reader: reader)
        }
        self.status = status
        request = unavailable.map { ($0, "") } ?? T.request(rates)
        self.display = "\(display.name) · \(display.detail)"
        rows = [(now, 0.5), (five, 5.0)].map { t, nominal in
            (T.window(t, nominal: nominal), t.fps.map { String(Int($0.rounded())) } ?? "—",
             T.figures(t, rates: rates, adaptiveSync: display.adaptiveSync).missed)
        }
        pacing = T.pacing(five, rates: rates, adaptiveSync: display.adaptiveSync)
        pacingWindow = T.window(five, nominal: 5).lowercased()
        input = .init(reader: reader, rates: rates, displayMaximumHz: display.maximumHz)
        let windows = [(five, 5.0), (thirty, 30.0)]
        let figures = windows.map { T.figures($0.0, rates: rates, adaptiveSync: display.adaptiveSync) }
        columns = windows.map { T.window($0.0, nominal: $0.1) }
        table = T.rows.dropFirst(2).map { name, key in (name, figures.map { $0[keyPath: key] }) }
        caption = T.deadline(rates, varied: thirty.counts.epochs > 0)
        let request = rates.requestedHz > 0 ? "\(Int(rates.requestedHz.rounded())) requested" : "none requested"
        rateSummary = "Frame rate over the last 30 seconds: \(rows[0].1) FPS in the newest sample, \(rows[1].1) over the \(pacingWindow), \(request)."
        timeSummary = "Frame time over the last 30 seconds. \([pacing.0, pacing.1].filter { !$0.isEmpty }.joined(separator: ", ")). "
            + "Over the \(pacingWindow), the longest frame time was \(figures[0].maxFrame), the slowest work \(figures[0].maxWork), "
            + "and \(figures[0].late) frames were late. " + caption + "."
    }

    /// Carry the figures into the face, keeping its own interaction state.
    func apply(to state: inout FrameHealthCardFace.State) {
        state.status = status; state.request = request; state.display = display; state.rows = rows; state.pacing = pacing
        state.pacingWindow = pacingWindow
    }
}

/// What Share offers, each alone, so every app takes it the same way; each holds everything,
/// whichever view the card shows.
enum FrameHealthShare: CaseIterable {
    case report, summary, image
    var title: String {
        switch self {
        case .report: "Agent Report"
        case .summary: "Summary"
        case .image: "Image"
        }
    }
}

extension FrameHealthCardText {
    /// The shared image, from the same reader the card shows (frozen while paused): the summary's
    /// context, both charts, every figure and the results.
    static func picture(reader: FrameHealthCardReader, chart: FrameHealthChart, rates: Rates, display: Display, paused: Bool,
                        health: FrameHealth, palette: FrameHealthCardDrawing.Palette) -> Data? {
        let summary = summary(reader: reader, rates: rates, display: display, paused: paused, health: health)
        let windows = [(reader.window(0.5), 0.5), (reader.window(5), 5.0), (reader.window(30), 30.0)]
        let all = windows.map { figures($0.0, rates: rates, adaptiveSync: display.adaptiveSync, count: { String($0) }) }
        let table = [[""] + windows.map { window($0.0, nominal: $0.1) }]
            + rows.map { name, key in [name] + all.map { $0[keyPath: key] } }
        let input = FrameHealthCardDrawing.Input(reader: reader, rates: rates, displayMaximumHz: display.maximumHz)
        return FrameHealthCardDrawing.picture(title: summary.title, context: summary.context, input: input, chart: chart,
            table: table, results: summary.results, notes: summary.notes, palette: palette)
    }
}
