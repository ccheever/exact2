// @ref LLP 1115.002 D5 — stage 3a, the gate's third question: ten thousand
// rows scrolled, in the table and as the list is laid out today, the same
// page and the same travel. What can be counted is held on every run: a row
// moves into a cell once and out once, and neither arrangement mounts more
// than a window of rows. What is timed is a measurement, written where
// `EXACT_TABLE_PERF` says and asserted nowhere: this Mac is shared, and a
// frame's time under another lane's build is not this code's. So each
// frame's work is given as processor time too, which waiting for a core
// does not lengthen.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class TableFeedPerfMacTests: XCTestCase {
    private var fixture: TableFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    /// Where a fling that leaves `from` at `velocity` points a second is
    /// after `t` seconds, slowing as a trackpad's momentum does.
    private static func fling(from: CGFloat, velocity: CGFloat, at t: Double, tau: Double = 0.5) -> CGFloat {
        from + velocity * CGFloat(tau * (1 - exp(-t / tau)))
    }
    private static func duration(velocity: CGFloat, tau: Double = 0.5) -> Double { tau * log(Double(velocity) / 20) }

    func testARowIsMountedOnceInItsCellWhileTenThousandScroll() throws {
        let f = try TableFixture(count: 10_000)
        fixture = f
        let moved = f.feed.moves, noted = f.feed.noted, arrived = f.feed.arrivals
        var deepest = 0, mounted = 0, views = 0, outside = 0
        // A hard fling's travel, a frame at a time at 60 a second.
        let seconds = Self.duration(velocity: 12_000)
        for frame in 1...Int(seconds * 60) {
            f.scroll(to: Self.fling(from: 0, velocity: 12_000, at: Double(frame) / 60))
            deepest = max(deepest, f.cells.keys.max() ?? 0)
            mounted = max(mounted, f.mounted.count)
            views = max(views, f.feed.rowViews)
            outside = max(outside, f.mounted.values.filter { !($0.superview is TableRowCell) }.count)
        }
        XCTAssertGreaterThan(deepest, 150, "the fling passed some hundreds of rows")
        XCTAssertLessThan(mounted, 120, "a window of rows was mounted at any time")
        // What stage 3b cut. A row goes into its cell in the batch that
        // made it and leaves it when the runner destroys it: one move a row
        // the runner made (it makes some twice, ahead of a fling and again),
        // where stage 3a's feed moved each in and out as it crossed the port.
        let made = f.feed.arrivals - arrived
        XCTAssertGreaterThan(made, deepest - 60, "the runner made the rows the fling passed")
        XCTAssertEqual(f.feed.moves - moved, made, "each row the runner made went into a cell, once, and none came out to the document")
        XCTAssertEqual(outside, 0, "every mounted row was in a cell at every frame: the table keeps a view for each")
        XCTAssertLessThan(views, 2 * mounted, "and it keeps no more views than the rows about the port")
        // AppKit hears a height only where the table's differs from layout's:
        // here the rows with a second paragraph or controls, three in ten.
        XCTAssertLessThan(f.feed.noted - noted, made / 2, "a row of the estimated height told AppKit nothing")
    }

    private struct Times {
        var ms: [Double] = []
        func line(_ name: String) -> String {
            let s = ms.sorted()
            guard !s.isEmpty else { return "\(name): no frames" }
            let q = { (p: Double) in s[min(s.count - 1, Int(p * Double(s.count)))] }
            let over = { (budget: Double) in s.filter { $0 > budget }.count }
            return String(format: "%@: %d frames; ms min %.2f p50 %.2f p90 %.2f p95 %.2f p99 %.2f max %.2f; over 8.33 ms %d, over 13.33 ms %d, over 16.67 ms %d",
                          name, s.count, s[0], q(0.5), q(0.9), q(0.95), q(0.99), s[s.count - 1], over(8.33), over(13.33), over(16.67))
        }
    }
    /// A frame's time three ways. On a shared machine the wall's time is
    /// mostly waiting for a core; the processor's time is the work.
    private struct Frames {
        var wall = Times(), main = Times(), process = Times()
        /// The main thread's processor time in each step of a frame, summed (`frame`'s five).
        var steps = [Double](repeating: 0, count: 5)
        mutating func add(_ frame: (wall: Double, main: Double, process: Double, steps: [Double])) {
            wall.ms.append(frame.wall); main.ms.append(frame.main); process.ms.append(frame.process)
            for (i, ms) in frame.steps.enumerated() { steps[i] += ms }
        }
        func lines(_ name: String) -> [String] {
            let each = steps.map { $0 / Double(max(1, main.ms.count)) }
            return [main.line("\(name), the main thread's processor time"), process.line("\(name), every thread's processor time"), wall.line("\(name), by the wall"),
                    String(format: "%@, the main thread's mean by step: the port moved %.2f, the turn's queued work %.2f, the pump %.2f, layout and drawing %.2f, the commit %.2f ms", name, each[0], each[1], each[2], each[3], each[4])]
        }
    }
    private static func processor(_ clock: clockid_t) -> Double { Double(clock_gettime_nsec_np(clock)) / 1_000_000 }
    private static func load() -> String {
        var loads = [Double](repeating: 0, count: 3)
        getloadavg(&loads, 3)
        return String(format: "load averages %.2f %.2f %.2f", loads[0], loads[1], loads[2])
    }

    /// One frame's work on the main thread, as a scroll event brings it:
    /// the port moves, the turn's queued work runs (the pump's slice, a
    /// report's continuation), the window lays out and draws, the layer
    /// tree is committed.
    private func frame(_ f: TableFixture, to y: CGFloat) -> (wall: Double, main: Double, process: Double, steps: [Double]) {
        let began = CACurrentMediaTime(), main = Self.processor(CLOCK_THREAD_CPUTIME_ID), process = Self.processor(CLOCK_PROCESS_CPUTIME_ID)
        var marks = [main]
        f.clip.scroll(to: NSPoint(x: 0, y: y))
        f.list.scroll!.reflectScrolledClipView(f.clip)
        marks.append(Self.processor(CLOCK_THREAD_CPUTIME_ID))
        RunLoop.main.run(mode: .default, before: Date())
        marks.append(Self.processor(CLOCK_THREAD_CPUTIME_ID))
        f.presenter.pump()
        marks.append(Self.processor(CLOCK_THREAD_CPUTIME_ID))
        f.window.displayIfNeeded()
        marks.append(Self.processor(CLOCK_THREAD_CPUTIME_ID))
        CATransaction.flush()
        marks.append(Self.processor(CLOCK_THREAD_CPUTIME_ID))
        return ((CACurrentMediaTime() - began) * 1000, marks[5] - main, Self.processor(CLOCK_PROCESS_CPUTIME_ID) - process, zip(marks.dropFirst(), marks).map { $0 - $1 })
    }

    /// The travels, each a list of offsets a frame apart at `hz`.
    private func travels(_ f: TableFixture, hz: Double) -> [(String, [CGFloat])] {
        func flung(_ velocity: CGFloat, from: CGFloat) -> [CGFloat] {
            (1...Int(Self.duration(velocity: velocity) * hz)).map { Self.fling(from: from, velocity: velocity, at: Double($0) / hz) }
        }
        return [
            ("a fling at 6,000 points a second", flung(6_000, from: 0)),
            ("a fling at 12,000 points a second", flung(12_000, from: 4_000)),
            ("a fling at 40,000 points a second (a port a frame)", flung(40_000, from: 12_000)),
            ("5,000 points a second held for 3 s", (1...Int(3 * hz)).map { 40_000 + CGFloat($0) * 5_000 / CGFloat(hz) }),
            ("the same back up", (1...Int(3 * hz)).map { 55_000 - CGFloat($0) * 5_000 / CGFloat(hz) }),
        ]
    }

    func testTenThousandRowsScrolledAgainstTheFrameBudget() throws {
        guard let path = ProcessInfo.processInfo.environment["EXACT_TABLE_PERF"] else { throw XCTSkip("a measurement, run on request: EXACT_TABLE_PERF=<file to write>") }
        var report: [String] = []
        func say(_ line: String) { report.append(line); print(line) }
        let screen = try XCTUnwrap(NSScreen.main)
        let hz = Double(max(1, screen.maximumFramesPerSecond))
        // Both arrangements at once, side by side on the screen, and each travel run on one then the other and back: the load is the same for both.
        NSApplication.shared.setActivationPolicy(.accessory)
        let table = try TableFixture(count: 10_000), today = try TableFixture(count: 10_000, table: false)
        defer { table.close(); today.close() }
        for (f, x) in [(table, CGFloat(40)), (today, 560)] {
            f.window.setFrameTopLeftPoint(NSPoint(x: x, y: screen.visibleFrame.maxY - 40))
            f.window.orderFrontRegardless()
        }
        // The window server says a window shows in an event, so the application's queue is read, not only the run loop turned.
        let shown = Date().addingTimeInterval(5)
        while !(table.window.occlusionState.contains(.visible) && today.window.occlusionState.contains(.visible)), Date() < shown {
            if let event = NSApp.nextEvent(matching: .any, until: Date().addingTimeInterval(0.05), inMode: .default, dequeue: true) { NSApp.sendEvent(event) }
        }
        table.settle(); today.settle()
        // How ExactKit was compiled, said by whoever built it optimized (`swift build -c release`): a debug build overstates every time here.
        let build = ProcessInfo.processInfo.environment["EXACT_TABLE_PERF_BUILD"] ?? "a debug build of ExactKit"
        say("ten thousand rows, the gate's fixture (mixed heights; a button, a checkbox and a field in every tenth row), two 480 by 600 windows on screen: the table, and the list as the presenter lays it out today")
        say(String(format: "display %.0f Hz at %.0fx: a frame is %.2f ms; %@; windows visible %@ %@; %@", hz, screen.backingScaleFactor, 1000 / hz, build,
                   "\(table.window.occlusionState.contains(.visible))", "\(today.window.occlusionState.contains(.visible))", Self.load()))
        // `EXACT_TABLE_PERF_ONLY=table` (or `today`): that arrangement alone, for a profile of it. `EXACT_TABLE_PERF_REPEAT=<n>`: each travel
        // that many times over. `EXACT_TABLE_PERF_RAW=<file>`: every frame's main-thread time, a line a frame, for a distribution over
        // several runs. `EXACT_TABLE_PERF_LINK=0`: without the display link's part.
        let environment = ProcessInfo.processInfo.environment
        let both = [("table", table), ("today", today)].filter { environment["EXACT_TABLE_PERF_ONLY"] == nil || environment["EXACT_TABLE_PERF_ONLY"] == $0.0 }
        let names = both.map(\.0), repeats = max(1, environment["EXACT_TABLE_PERF_REPEAT"].flatMap { Int($0) } ?? 1)
        var raw: [String] = []

        // The work of each frame, one after another with no waiting: what the main thread does for a frame.
        say("\n== work per frame on the main thread (the port moved, the turn's queued work, layout, drawing, the commit)")
        for (travel, offsets) in travels(table, hz: hz) {
            var times = ["table": Frames(), "today": Frames()]
            for (name, f) in Array(repeating: both + both.reversed(), count: repeats).joined() {
                f.scroll(to: offsets[0])
                for y in offsets { times[name]!.add(frame(f, to: y)) }
                f.settle()
            }
            say("  \(travel), \(repeats == 1 ? "twice" : "\(2 * repeats) times") each; \(Self.load())")
            for name in names {
                times[name]!.lines(name).forEach { say("    \($0)") }
                raw += times[name]!.main.ms.map { String(format: "%@\t%@\t%.4f", name, travel, $0) }
            }
        }
        var ends = ["table": Frames(), "today": Frames()], after = ["table": Frames(), "today": Frames()]
        for round in 0..<3 {
            for (name, f) in round % 2 == 0 ? both : both.reversed() {
                f.scroll(to: CGFloat(round) * 1_000)
                ends[name]!.add(frame(f, to: f.document.frame.height - f.clip.bounds.height))
                for _ in 0..<20 { after[name]!.add(frame(f, to: f.document.frame.height - f.clip.bounds.height)) }
                ends[name]!.add(frame(f, to: CGFloat(round) * 1_000))
                for _ in 0..<20 { after[name]!.add(frame(f, to: CGFloat(round) * 1_000)) }
                f.settle()
            }
        }
        say("  a jump to the end and a jump back, three times; \(Self.load())")
        for name in names {
            ends[name]!.lines("\(name), the frame of the jump").forEach { say("    \($0)") }
            after[name]!.lines("\(name), the 20 frames after each").forEach { say("    \($0)") }
        }
        say("  the feed, over all of it: \(table.feed.moves) moves of a row between the document and a cell, \(table.feed.noted) row heights told to the table")
        if let file = environment["EXACT_TABLE_PERF_RAW"] { try raw.joined(separator: "\n").write(toFile: file, atomically: true, encoding: .utf8) }

        // In real time, by the display's own link: the frames the repository's sampler counts (`perf frames`).
        say("\n== real time, by the display link: the session's frame sampler (what `agent.mjs macos perf frames` reads)")
        for (travel, velocity) in [("a fling at 6,000 points a second", CGFloat(6_000)), ("a fling at 12,000 points a second", 12_000), ("a fling at 40,000 points a second", 40_000)] where environment["EXACT_TABLE_PERF_LINK"] != "0" {
            for (name, f) in both + both.reversed() {
                let sampler = try XCTUnwrap(f.session.sampler)
                f.scroll(to: 0)
                RunLoop.main.run(until: Date().addingTimeInterval(0.7)) // the sampler's segment ends
                sampler.reset()
                let driver = Driver(clip: f.clip, scroll: f.list.scroll!) { Self.fling(from: 0, velocity: velocity, at: $0) }
                let link = f.view.displayLink(target: driver, selector: #selector(Driver.tick(_:)))
                link.add(to: .main, forMode: .common)
                let seconds = Self.duration(velocity: velocity)
                let until = Date().addingTimeInterval(seconds + 0.2)
                while Date() < until { RunLoop.main.run(mode: .default, before: until) }
                link.invalidate()
                let reply = sampler.reply(late: 100, all: true)
                let lifetime = reply["lifetime"] as? [String: Int] ?? [:], window = reply["window"] as? [String: Any] ?? [:]
                var apply = Times(), overrun = Times()
                for record in reply["records"] as? [[String: Any]] ?? [] {
                    if let ms = record["apply"] as? Double { apply.ms.append(ms) }
                    if let ms = record["overrun"] as? Double, ms > 0 { overrun.ms.append(ms) }
                }
                say("  \(name), \(travel), \(String(format: "%.1f", seconds)) s, \(driver.ticks) scroll ticks: presented \(lifetime["presented"] ?? -1), late \(lifetime["late"] ?? -1), missed \(lifetime["missed"] ?? -1), main thread past its frame's target \(lifetime["overruns"] ?? -1); interval ms p50 \(window["p50"] ?? "?") p95 \(window["p95"] ?? "?") p99 \(window["p99"] ?? "?") max \(window["max"] ?? "?"); \(Self.load())")
                say("    \(apply.line("batch apply per presented frame"))")
                if !overrun.ms.isEmpty { say("    \(overrun.line("the frames past their target, by how much"))") }
                f.settle()
            }
        }
        try report.joined(separator: "\n").write(toFile: path, atomically: true, encoding: .utf8)
    }
}

/// A display link's target: each frame it puts the port where the travel is by then.
private final class Driver: NSObject {
    private let clip: NSClipView, scroll: NSScrollView
    private let position: (Double) -> CGFloat
    private var began: CFTimeInterval?
    private(set) var ticks = 0
    init(clip: NSClipView, scroll: NSScrollView, position: @escaping (Double) -> CGFloat) {
        self.clip = clip; self.scroll = scroll; self.position = position
    }
    @objc func tick(_ link: CADisplayLink) {
        let start = began ?? link.timestamp
        began = start
        ticks += 1
        clip.scroll(to: NSPoint(x: 0, y: position(link.targetTimestamp - start)))
        scroll.reflectScrolledClipView(clip)
    }
}
#endif
