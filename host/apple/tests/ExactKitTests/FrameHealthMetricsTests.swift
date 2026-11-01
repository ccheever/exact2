import Foundation
import QuartzCore
import XCTest
import CExact
#if os(macOS)
import AppKit
#endif
@testable import ExactKit

final class FrameHealthMetricsTests: XCTestCase {
    override class func setUp() {
        super.setUp()
        #if os(macOS)
        _ = NSApplication.shared
        #endif
    }
    func testMetricCursorsFinalizeWholeIntervalsAndKeepTheirOwnWorkWindow() throws {
        // Lateness is unknown on the simulator; this test reads the counts a device reports
        let simulator = FrameHealthAssessment.deadlinesObserved
        FrameHealthAssessment.deadlinesObserved = true
        defer { FrameHealthAssessment.deadlinesObserved = simulator }
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        health.record(arrival: 1, target: 1.008, period: 0.008, finished: 1.001)
        let before = health.metrics()
        let cursor = try XCTUnwrap(before["cursor"] as? [String: Any])
        // The next frame starts 24 ms later and its work finishes 3 ms after its deadline
        health.record(arrival: 1.024, target: 1.032, period: 0.008, finished: 1.035)
        let after = health.metrics(cursor: cursor)
        let frames = try XCTUnwrap(after["frames"] as? [String: Any])
        XCTAssertEqual(frames["intervals"] as? String, "1")
        XCTAssertEqual(frames["observedNs"] as? String, "24000000")
        XCTAssertEqual(frames["p99BoundsNs"] as? [UInt64], [24_000_000, 48_000_000])
        let work = try XCTUnwrap(after["work"] as? [String: Any])
        XCTAssertEqual(work["records"] as? String, "1")
        XCTAssertEqual(work["late"] as? String, "1")
        XCTAssertEqual(work["medianBoundsNs"] as? [UInt64], [11_000_000, 11_500_000], "work runs from the frame's start")
        let window = try XCTUnwrap(after["window"] as? [String: Any])
        XCTAssertEqual(window["intervalThroughNs"] as? String, "1024000000")
        XCTAssertEqual(window["workThroughNs"] as? String, "1035000000")
        FrameHealthAssessment.deadlinesObserved = false
        let unknown = try XCTUnwrap(health.metrics(cursor: cursor)["work"] as? [String: Any])
        XCTAssertTrue(unknown["late"] is NSNull, "where the deadline carries no information, lateness is unknown")
        XCTAssertEqual(unknown["latenessUnknown"] as? String, "simulator-deadlines")
        FrameHealthAssessment.deadlinesObserved = true
        let idleCursor = try XCTUnwrap(after["cursor"] as? [String: Any])
        health.stop()
        let idle = health.metrics(cursor: idleCursor)
        XCTAssertEqual((idle["frames"] as? [String: Any])?["observedNs"] as? String, "0")
        XCTAssertTrue((idle["frames"] as? [String: Any])?["p99BoundsNs"] is NSNull)
        health.reset()
        XCTAssertEqual(health.metrics(cursor: cursor)["error"] as? String, "baseline-required")
        XCTAssertEqual(health.metrics(cursor: ["version": 2])["error"] as? String, "baseline-required")
    }

    func testAPendingFrameFinishesWhenEverythingItSentHasLanded() throws {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        // A frame sends a tick and a frame-task advance; the advance lands first
        health.sending(FrameHealth.tick, start: 1, deadline: 1.008)
        health.sending(FrameHealth.advance, start: 1, deadline: 1.008)
        health.record(arrival: 1.0001, target: 1.008, period: 0.008, finished: 1.001, sent: FrameHealth.tick | FrameHealth.advance)
        XCTAssertEqual(health.snapshot().worked, 0, "a pending frame has no work record yet")
        health.landed(FrameHealth.advance, at: 1.004)
        XCTAssertEqual(health.snapshot().worked, 0)
        health.landed(FrameHealth.tick, at: 1.012)
        let s = health.snapshot()
        XCTAssertEqual([s.worked, s.late], [1, 1], "it finished at the later landing, after its deadline")
        XCTAssertEqual(s.work_through_ns, 1_012_000_000)
        // A callback whose work waited on earlier work is not a frame
        health.record(arrival: 1.0081, target: 1.016, period: 0.008, finished: 1.009, held: true, ran: false)
        XCTAssertEqual(health.snapshot().held, 1)
        XCTAssertEqual(health.snapshot().intervals, 0)
        // Work that lands with no frame waiting for it adds nothing
        health.landed(FrameHealth.tick, at: 1.02)
        XCTAssertEqual(health.snapshot().worked, 1)
        // Work can land during its own callback, before the callback is recorded: the frame
        // finishes when the callback returns
        health.sending(FrameHealth.tick, start: 1.016, deadline: 1.024)
        health.landed(FrameHealth.tick, at: 1.017)
        XCTAssertEqual(health.snapshot().worked, 1, "its callback has not returned")
        health.record(arrival: 1.0161, target: 1.024, period: 0.008, finished: 1.018, sent: FrameHealth.tick)
        XCTAssertEqual(health.snapshot().worked, 2)
        XCTAssertEqual(health.snapshot().work_through_ns, 1_018_000_000)
        XCTAssertEqual(health.pendingFrames(at: 2).pending, 0, "nothing is left waiting")
    }

    func testTheChartsTakeEachFrameAsAMovingAverageAndExactFrameTimes() {
        let chart = FrameHealthChart(in: CALayer())
        chart.start()
        func column(_ time: Double) -> FrameHealthChart.Column { chart.columns[Int(Int64((time / 0.1).rounded(.down)) % 300)] }
        var t = 1000.0
        for _ in 0..<120 {
            t += 1 / 120.0
            chart.frame(at: t, finished: t + 0.004, deadline: t + 1 / 120.0)
        }
        XCTAssertEqual(Double(column(t).fps), 120, accuracy: 0.5)
        XCTAssertEqual(Double(column(t).frame), 1000 / 120.0, accuracy: 0.01)
        XCTAssertEqual(Double(column(t).work), 4, accuracy: 0.01)
        // After 0.3 s at 60 FPS the last half second holds 18 intervals of 1/60 s and about 24 of 1/120 s
        for _ in 0..<18 {
            t += 1 / 60.0
            chart.frame(at: t)
        }
        XCTAssertEqual(Double(column(t).fps), 84, accuracy: 1, "the frame rate's moving average over the last half second")
        t += 0.05
        chart.frame(at: t)
        XCTAssertEqual(Double(column(t).frame), 50, accuracy: 0.01, "a hitch keeps its own frame time")
        XCTAssertLessThan(column(t).fps, 80, "and lowers the average")
        // Work that lands after its callback joins its frame's column
        chart.work(start: t, deadline: t + 1 / 120.0, finished: t + 0.02)
        XCTAssertEqual(Double(column(t).work), 20, accuracy: 0.01)
        XCTAssertEqual(Double(column(t).budget), 1000 / 120.0, accuracy: 0.01)
        // A repeated timestamp is no frame, and ends the run
        let fps = column(t).fps
        chart.frame(at: t)
        XCTAssertEqual(column(t).fps, fps)
        t += 0.1
        chart.frame(at: t)
        XCTAssertEqual(column(t).fps, 0, "the frame after it bounds no interval")
        XCTAssertFalse(column(t).joined)
        // At 5 FPS each frame lands two columns after the last and joins it across the empty one
        for _ in 0..<5 {
            t += 0.2
            chart.frame(at: t)
        }
        XCTAssertTrue(column(t).joined)
        XCTAssertEqual(Double(column(t).frame), 200, accuracy: 0.01)
        XCTAssertEqual(Double(column(t).fps), 5, accuracy: 0.01)
        // After a run ends, as at an idle callback, the next frame bounds no interval and joins nothing
        chart.endRun()
        let u = 2000.05
        chart.frame(at: u, finished: u + 0.03, deadline: u + 1 / 120.0)
        XCTAssertEqual(column(u).frame, 0)
        XCTAssertEqual(column(u).fps, 0)
        XCTAssertFalse(column(u).joined)
        // Thirty seconds on, the same place holds the new tenth of a second alone
        chart.frame(at: u + 30)
        XCTAssertEqual(column(u + 30).index, 20300)
        XCTAssertEqual(column(u + 30).work, 0)
    }

    private static let palette: FrameHealthCardDrawing.Palette = {
        let gray = { (white: CGFloat) in CGColor(gray: white, alpha: 1) }
        return .init(background: gray(1), label: gray(0), secondaryLabel: gray(0.5), separator: gray(0.9), fill: gray(0.8),
            secondaryFill: gray(0.85), tertiaryFill: gray(0.9), quaternaryFill: gray(0.95), blue: gray(0.3), orange: gray(0.6),
            green: gray(0.4), gray: gray(0.5), dark: false)
    }()

    private static func plots(expanded: Int? = nil) -> [(chart: Int, area: CGRect, plot: FrameHealthCardDrawing.Plot)] {
        FrameHealthCardDrawing.plots(CGRect(x: 12, y: 206, width: 296, height: 200), .init(reader: FrameHealthCardReader(),
            rates: .init(requestedHz: 120, periodMs: 1000 / 120.0), displayMaximumHz: 120), expanded: expanded)
    }

    func testEachFrameMovesTheChartsAndRedrawsThemWholeEveryFiveSeconds() throws {
        let palette = Self.palette, plots = Self.plots()
        let host = CALayer(), chart = FrameHealthChart(in: host)
        chart.start()
        chart.frame(at: 100)
        chart.configure(plots, scale: 1, contentsScale: 2, palette: palette)
        let windows = host.sublayers ?? [], lines = windows.compactMap { $0.sublayers?.first }
        XCTAssertEqual(lines.count, 2, "a layer for each plot")
        XCTAssertTrue(lines.allSatisfy(\.isOpaque))
        let width = plots[0].plot.right - plots[0].plot.left
        XCTAssertEqual(lines[0].bounds.width, width * 35 / 30, accuracy: 0.01, "wider than its plot by five seconds of axis")
        chart.frame(at: 101)
        XCTAssertEqual(lines[0].position.x, 0, "the frame hooks only store a frame")
        chart.present()
        XCTAssertEqual(lines[0].position.x, -(width / 30 * 2).rounded(.down) / 2, "the layer moves left by the time it advanced, in whole pixels")
        chart.frame(at: 106.5)
        chart.present()
        XCTAssertEqual(lines[0].position.x, 0, "past the five seconds it reaches, the layer is drawn whole again")
        // A dotted line's dashes keep their place in time as the plot moves on
        var earlier = plots[0].plot, later = earlier
        (earlier.now, later.now) = (100, 103.7)
        XCTAssertEqual(earlier.dash(earlier.x(101.23)), later.dash(later.x(101.23)), accuracy: 1e-6)
        chart.configure([], scale: 1, contentsScale: 2, palette: palette)
        XCTAssertTrue(windows.allSatisfy(\.isHidden), "the table or a compact card shows no plot")
        chart.stop()
        XCTAssertTrue(chart.columns.isEmpty && chart.times.isEmpty, "a card that stops reading frees its frames")
        XCTAssertTrue(lines.allSatisfy { $0.contents == nil })
    }

    #if os(macOS)
    func testTheChartLayersShowEachChangeAtOnce() throws {
        // Core Animation animates a layer's changes only in a window's layer tree
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 320, height: 416), styleMask: [.borderless], backing: .buffered,
            defer: false)
        let view = NSView(frame: CGRect(x: 0, y: 0, width: 320, height: 416))
        view.wantsLayer = true
        window.contentView = view
        let chart = FrameHealthChart(in: try XCTUnwrap(view.layer))
        chart.start()
        chart.frame(at: 100)
        chart.configure(Self.plots(), scale: 1, contentsScale: 2, palette: Self.palette)
        let layers = (view.layer?.sublayers ?? []).flatMap { [$0] + ($0.sublayers ?? []) }
        CATransaction.flush()
        for time in [100.5, 101, 107] {
            chart.frame(at: time)
            chart.present()
            layers.forEach { $0.displayIfNeeded() }
            CATransaction.flush()
        }
        chart.configure(Self.plots(expanded: 1), scale: 1, contentsScale: 2, palette: Self.palette)
        layers.forEach { $0.displayIfNeeded() }
        CATransaction.flush()
        XCTAssertTrue(layers.allSatisfy { $0.animationKeys() == nil }, "a new picture, place or size shows at once, never faded or stretched")
    }
    #endif

    func testColumnsDrawnAgainMatchAWholeDrawing() {
        // Frames redraw only the columns they changed, so the layers must come out as a whole drawing draws them, in
        // each layout: the dotted target, and the dotted deadline through a change of deadline, carets, work that
        // lands late, a pause within a run, a run's end, a slow run and the five-second whole drawing. Core Graphics
        // antialiases a line a few levels differently when its path covers another span, so a byte may differ by 8.
        for expanded in [nil, 0, 1] {
            let host = CALayer(), chart = FrameHealthChart(in: host)
            chart.start()
            chart.frame(at: 100)
            chart.configure(Self.plots(expanded: expanded), scale: 1, contentsScale: 2, palette: Self.palette)
            let layers = (host.sublayers ?? []).filter { !$0.isHidden }.compactMap { $0.sublayers?.first }
            func pixels(_ layer: CALayer) -> [UInt8] {
                layer.displayIfNeeded()
                let width = Int(layer.bounds.width * 2), height = Int(layer.bounds.height * 2)
                var bytes = [UInt8](repeating: 0, count: width * height * 4)
                bytes.withUnsafeMutableBytes { buffer in
                    let ctx = CGContext(data: buffer.baseAddress, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
                    ctx.scaleBy(x: 2, y: 2)
                    layer.render(in: ctx)
                }
                return bytes
            }
            func shown() {
                chart.present()
                for layer in layers { layer.displayIfNeeded() }
            }
            // What the frames drew against a whole drawing of the same frames, which the layers then keep
            func compare(_ stage: String) -> [[UInt8]] {
                let drawn = layers.map(pixels)
                for layer in layers { layer.setNeedsDisplay() }
                for (which, (layer, bytes)) in zip(layers, drawn).enumerated() {
                    let whole = pixels(layer), worst = zip(bytes, whole).map { abs(Int($0) - Int($1)) }.max() ?? 0
                    XCTAssertLessThanOrEqual(worst, 8, "\(stage), layout \(String(describing: expanded)), layer \(which): levels from a whole drawing")
                }
                return drawn
            }
            var t = 100.0
            for n in 1...360 {
                t += n == 200 ? 1 : 1 / 120.0
                chart.frame(at: t, finished: t + (n % 50 == 0 ? 0.02 : 0.004), deadline: t + (n < 180 ? 1 / 120.0 : 1 / 90.0))
                shown()
                if n == 300 {
                    chart.work(start: t - 1 / 120.0, deadline: t, finished: t + 0.03)
                    shown()
                }
            }
            // Before the five-second whole drawing, which would draw over what the frames left
            _ = compare("120 FPS")
            chart.endRun()
            t += 0.5
            // Then 5 FPS and 2 FPS, whose lines reach back across columns without frames, past a caret's reach at
            // 2 FPS, each with work that lands after the next frame began
            for (period, frames) in [(0.2, 6), (0.5, 4)] {
                for n in 0..<frames {
                    t += period
                    chart.frame(at: t, finished: t + 0.004, deadline: t + 1 / 120.0)
                    shown()
                    if n == frames - 2 {
                        chart.work(start: t - period, deadline: t - period + 1 / 120.0, finished: t + 0.01)
                        shown()
                    }
                }
            }
            let drawn = compare("slow frames")
            if expanded == nil {
                // At 2 FPS the frame-rate line joins each frame to the one before, across the empty columns between
                let plot = Self.plots()[0].plot, width = plot.right - plot.left, right = Int((-layers[0].position.x + width) * 2)
                let row = Int(layers[0].bounds.width * 2) * 4
                let dark = stride(from: 0, to: drawn[0].count, by: 4).filter { i in
                    (right - Int(width / 30 * 2)..<right).contains((i % row) / 4) && drawn[0][i] < 120
                }
                XCTAssertGreaterThan(dark.count, 20, "the last second's frame-rate line")
            }
        }
    }

    func testADottedLineKeepsItsDashesEvenAcrossColumns() {
        // Thirty seconds of frames at 20 FPS against one deadline, drawn whole at twice the plot's points
        let chart = FrameHealthChart(in: CALayer())
        chart.start()
        for n in 0..<600 {
            let t = 1000 + Double(n) * 0.05
            chart.frame(at: t, finished: t + 0.004, deadline: t + 1 / 90.0)
        }
        var plot = Self.plots()[1].plot
        plot.now = chart.newest
        let width = Int((plot.right - plot.left) * 2), height = Int((plot.bottom + 9 - plot.top) * 2)
        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        bytes.withUnsafeMutableBytes { buffer in
            let ctx = CGContext(data: buffer.baseAddress, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
            ctx.setFillColor(Self.palette.background)
            ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
            ctx.translateBy(x: 0, y: CGFloat(height))
            ctx.scaleBy(x: 2, y: -2)
            ctx.translateBy(x: -plot.left, y: -(plot.top - 8))
            FrameHealthCardDrawing.plotLines(ctx, chart: 1, plot: plot, columns: chart.columns, newest: chart.newest,
                palette: Self.palette, margin: 0)
        }
        // Where each dash begins along the deadline's row: one every 5 points, as along one straight line
        let row = Int((plot.y(1000 / 90.0) - (plot.top - 8)) * 2)
        let dark = (0..<width).map { bytes[(row * width + $0) * 4] < 180 }
        let starts = (1..<width).filter { dark[$0] && !dark[$0 - 1] }
        XCTAssertGreaterThan(starts.count, 40)
        let gaps = zip(starts.dropFirst(), starts).map { $0 - $1 }
        XCTAssertTrue(gaps.allSatisfy { (9...11).contains($0) }, "dash spacing in pixels: \(gaps)")
    }

    func testTheFrameHooksStoreFramesForTheChartsAndDrawNothing() {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let host = CALayer(), chart = FrameHealthChart(in: host)
        chart.start()
        chart.configure(Self.plots(), scale: 1, contentsScale: 2, palette: Self.palette)
        health.chart = chart
        let lines = (host.sublayers ?? []).compactMap { $0.sublayers?.first }
        for layer in lines { layer.displayIfNeeded() }
        func column(_ time: Double) -> FrameHealthChart.Column { chart.columns[Int(Int64((time / 0.1).rounded(.down)) % 300)] }
        let period = 1 / 120.0
        // A callback for a frame at `start` that ran its own work for 2 ms
        func ran(_ start: Double) { health.record(arrival: start + 0.0001, target: start + period, period: period, finished: start + 0.002) }
        ran(1.01)
        ran(1.01 + period)
        XCTAssertEqual(chart.newest, 1.01 + period, accuracy: 1e-9, "a frame starts at its callback's target less a period")
        XCTAssertEqual(Double(column(1.01).frame), period * 1000, accuracy: 0.01)
        XCTAssertEqual(Double(column(1.01).work), 2, accuracy: 0.01)
        XCTAssertEqual(lines[0].position.x, 0, "the hooks only store frames")
        XCTAssertFalse(lines[0].needsDisplay())
        // A held callback is no frame
        health.record(arrival: 1.05, target: 1.05 + period, period: period, finished: 1.051, held: true, ran: false)
        XCTAssertEqual(chart.newest, 1.01 + period, accuracy: 1e-9)
        // Work sent to the owner joins its frame when it lands
        health.sending(FrameHealth.tick, start: 1.12, deadline: 1.12 + period)
        health.record(arrival: 1.1201, target: 1.12 + period, period: period, finished: 1.121, sent: FrameHealth.tick)
        XCTAssertTrue(column(1.12).joined)
        XCTAssertEqual(column(1.12).work, 0)
        health.landed(FrameHealth.tick, at: 1.15)
        XCTAssertEqual(Double(column(1.12).work), 30, accuracy: 0.01, "work sent to the owner joins its frame when it lands")
        // An idle callback ends the run, and so does a rejected one
        health.record(arrival: 1.16, target: 1.16 + period, period: period, finished: 1.161, ran: false)
        ran(1.25)
        XCTAssertEqual(column(1.25).frame, 0)
        XCTAssertFalse(column(1.25).joined)
        ran(1.35)
        XCTAssertTrue(column(1.35).joined)
        health.record(arrival: 1.4, target: 1.4 + period, period: -period, finished: 1.401)
        ran(1.45)
        XCTAssertFalse(column(1.45).joined)
        chart.present()
        XCTAssertTrue(lines[0].needsDisplay(), "present() draws what the hooks stored")
        // A new generation forgets every frame
        health.reset()
        XCTAssertTrue(chart.columns.allSatisfy { $0.index == -1 })
    }

    func testACardHoldsUnder51KiBWhileItReads() {
        // What malloc holds for an array: its 32-byte header and its capacity, rounded up to the allocator's size
        func heap<T>(_ array: [T]) -> Int { malloc_good_size(32 + array.capacity * MemoryLayout<T>.stride) }
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let reader = FrameHealthCardReader(), chart = FrameHealthChart(in: CALayer())
        XCTAssertTrue(reader.start())
        defer { reader.stop() }
        chart.start()
        for index in 0..<4000 {
            let time = 1 + Double(index) / 120
            health.record(arrival: time, target: time + 1 / 120, period: 1 / 120, finished: time + 0.003)
            chart.frame(at: time, finished: time + 0.003, deadline: time + 1 / 120)
            if index % 60 == 59 { reader.sample(health, now: time + 0.001, force: true) }
        }
        XCTAssertEqual(reader.samples.count, 60)
        let read = FrameHealthMetrics.read(health)
        let samples = heap(reader.samples) + reader.samples.reduce(0) { $0 + heap($1.bins) }
        let previous = heap(read.intervals) + heap(read.work) + heap(read.refreshes) + heap(read.lateBy)
        let charts = heap(chart.columns) + heap(chart.times)
        let objects = malloc_size(Unmanaged.passUnretained(reader).toOpaque()) + malloc_size(Unmanaged.passUnretained(chart).toOpaque())
        let card = samples + previous + charts + objects
        XCTAssertLessThan(card, 51 * 1024)
        // Four readers at once and the collectors' 48,776-byte registry (LLP 1079.000 §10)
        XCTAssertLessThan(4 * card + 48_776, 256 * 1024, "within the process's 256 KiB")
    }

    func testAReleasedCardRestsInTheCornerItsGlideReaches() {
        typealias Corner = FrameHealthCardFace.Corner
        let area = CGRect(x: 8, y: 8, width: 400, height: 800), size = CGSize(width: 320, height: 416)
        XCTAssertEqual(Corner(nearest: CGPoint(x: 20, y: 500), velocity: .zero, size: size, in: area), Corner(maxY: true),
            "at rest, the corner nearest the card's center")
        // Normal deceleration carries a release at 1,000 points a second 499 points
        XCTAssertEqual(Corner(nearest: CGPoint(x: 20, y: 500), velocity: CGVector(dx: 0, dy: -1000), size: size, in: area), Corner())
        XCTAssertEqual(Corner(maxX: true, maxY: true).origin(of: size, in: area), CGPoint(x: 88, y: 392))
        var drag = FrameHealthCardFace.Drag(at: .zero, time: 1, origin: .zero, control: nil)
        drag.track(CGPoint(x: 10, y: 0), at: 1.01)
        // A lift at the last point keeps the momentum; one after the press held still has none
        drag.track(CGPoint(x: 10, y: 0), at: 1.011)
        XCTAssertEqual(drag.velocity(releasedAt: 1.011).dx, 1000, accuracy: 0.001)
        XCTAssertEqual(drag.velocity(releasedAt: 1.07), .zero)
    }

    func testACanvasRenderWithoutItsDrawableFinishesWhenItDraws() throws {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        health.sending(FrameHealth.canvas, start: 1, deadline: 1.008)
        health.record(arrival: 1.0001, target: 1.008, period: 0.008, finished: 1.001, sent: FrameHealth.canvas)
        XCTAssertTrue(health.waits(on: FrameHealth.canvas))
        XCTAssertEqual(health.snapshot().worked, 0, "the render has not drawn")
        XCTAssertEqual(health.pendingFrames(at: 1.009).overdue, 1, "past its deadline and not drawn, it is certainly late")
        health.landed(FrameHealth.canvas, at: 1.012)
        XCTAssertEqual([health.snapshot().worked, health.snapshot().late], [1, 1], "it finished when the render drew")
        // A render whose canvas leaves the screen before it draws: the frame never finishes
        health.sending(FrameHealth.canvas, start: 1.016, deadline: 1.024)
        health.record(arrival: 1.0161, target: 1.024, period: 0.008, finished: 1.017, sent: FrameHealth.canvas)
        health.abandon(FrameHealth.canvas)
        health.landed(FrameHealth.canvas, at: 1.03)
        XCTAssertEqual(health.snapshot().worked, 1)
        XCTAssertEqual(health.pendingFrames(at: 2).pending, 0)
        // A malformed callback is rejected before the collector, and the work it registered waits on nothing
        health.sending(FrameHealth.tick, start: 1.032, deadline: 1.04)
        health.record(arrival: 1.0321, target: 1.04, period: -0.008, finished: 1.033, sent: FrameHealth.tick)
        health.landed(FrameHealth.tick, at: 1.034)
        XCTAssertEqual(health.snapshot().worked, 1)
        XCTAssertEqual(health.pendingFrames(at: 2).pending, 0)
        health.sending(FrameHealth.tick, start: 1.04, deadline: .nan)
        XCTAssertEqual(health.pendingFrames(at: 2).pending, 0, "work without a finite deadline is not registered")
        // Work registered by a callback that was never recorded is dropped by the next one
        health.sending(FrameHealth.tick, start: 1.04, deadline: 1.048)
        health.record(arrival: 1.0481, target: 1.056, period: 0.008, finished: 1.049)
        XCTAssertEqual(health.pendingFrames(at: 2).pending, 0)
    }

    func testAssessmentsAreScopedAndMissingEvidenceNeverPasses() throws {
        func results(_ metrics: [String: Any]) -> [String] {
            let assessments = metrics["assessments"] as? [String: Any] ?? [:]
            return ["cadence", "target", "contentLatency"].map { key in
                let result = assessments[key] as? [String: Any] ?? [:]
                return "\(result["result"] ?? "?")/\(result["reason"] ?? "?")"
            }
        }
        func predicate(_ metrics: [String: Any], _ assessment: String, _ key: String) -> String? {
            (((metrics["assessments"] as? [String: Any])?[assessment] as? [String: Any])?["predicates"] as? [String: Any])?[key] as? String
        }
        // Reads at the last callback's finish, so the stale rule sees the source running
        var clock = 0.0
        func metrics(_ health: FrameHealth, cursor: [String: Any]? = nil) -> [String: Any] {
            FrameHealthMetrics.reply(FrameHealthMetrics.read(health, now: clock), health: health, cursor: cursor)
        }
        func frames(_ health: FrameHealth, from start: Double, count: Int, hertz: Double, late: Set<Int> = [], skipped: Set<Int> = []) {
            for index in 0..<count where !skipped.contains(index) {
                let time = start + Double(index) / hertz, finished = time + (late.contains(index) ? 2 : 0.5) / hertz
                health.record(arrival: time, target: time + 1 / hertz, period: 1 / hertz, finished: finished)
                clock = finished
            }
        }
        let simulator = FrameHealthAssessment.deadlinesObserved
        FrameHealthAssessment.deadlinesObserved = true
        defer { FrameHealthAssessment.deadlinesObserved = simulator }
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        XCTAssertEqual(results(health.metrics()), ["not-applicable/idle", "not-applicable/idle", "not-applicable/idle"])
        // Frame tasks at the default rate have a schedule to meet but no requested rate
        frames(health, from: 10, count: 4, hertz: 120)
        XCTAssertEqual(results(metrics(health)), ["proxy-pass/presentation-unobserved", "not-applicable/no-rate-request",
            "insufficient-evidence/content-lateness-unobserved"])
        health.stop()
        health.requestedRate(120)
        frames(health, from: 11, count: 10, hertz: 120)
        let steady = metrics(health)
        XCTAssertEqual(results(steady), ["proxy-pass/presentation-unobserved", "insufficient-evidence/presentation-unobserved", "insufficient-evidence/content-lateness-unobserved"])
        let assessments = try XCTUnwrap(steady["assessments"] as? [String: Any])
        XCTAssertEqual(assessments["policy"] as? String, FrameHealthAssessment.policy)
        XCTAssertEqual((assessments["cadence"] as? [String: Any])?["evidence"] as? String, "callback-and-owner-work-proxies")
        XCTAssertEqual(predicate(steady, "target", "requestedSlots"), "10", "stopped time before the request is not a slot")
        XCTAssertEqual(predicate(steady, "cadence", "frameIntervals"), "12", "the default-rate run's intervals count too")
        let cursor = try XCTUnwrap(steady["cursor"] as? [String: Any])
        frames(health, from: 11 + 10.0 / 120, count: 6, hertz: 120, skipped: [2])
        let skipped = metrics(health, cursor: cursor)
        XCTAssertEqual(results(skipped).prefix(2), ["fail/skipped-refreshes", "fail/missed-frames"])
        XCTAssertEqual((skipped["frames"] as? [String: Any])?["missedFrames"] as? String, "1")
        XCTAssertEqual((skipped["frames"] as? [String: Any])?["requestedSlots"] as? String, "6")
        let later = try XCTUnwrap(skipped["cursor"] as? [String: Any])
        frames(health, from: 11 + 16.0 / 120, count: 3, hertz: 120)
        XCTAssertEqual(results(metrics(health, cursor: later))[0], "proxy-pass/presentation-unobserved", "a failure belongs to the window that saw it")
        let clean = try XCTUnwrap(metrics(health)["cursor"] as? [String: Any])
        frames(health, from: 11 + 19.0 / 120, count: 3, hertz: 120, late: [1, 2])
        let late = metrics(health, cursor: clean)
        XCTAssertEqual(results(late)[0], "fail/late-work")
        XCTAssertEqual(predicate(late, "cadence", "lateFrames"), "2")
        // Where every callback arrives after its own target, lateness is unknown; a skipped refresh still fails.
        FrameHealthAssessment.deadlinesObserved = false
        XCTAssertEqual(results(metrics(health, cursor: clean))[0], "insufficient-evidence/simulator-deadlines")
        XCTAssertEqual(results(metrics(health, cursor: cursor))[0], "fail/skipped-refreshes")
        FrameHealthAssessment.deadlinesObserved = true

        // After a stop, a 60 Hz schedule under the 120 Hz request is even on its own schedule but restricted.
        health.stop()
        let restrictedCursor = try XCTUnwrap(metrics(health)["cursor"] as? [String: Any])
        frames(health, from: 20, count: 5, hertz: 60)
        let restricted = metrics(health, cursor: restrictedCursor)
        XCTAssertEqual(results(restricted).prefix(2), ["proxy-pass/presentation-unobserved", "restricted/unknown"])
        XCTAssertEqual(predicate(restricted, "target", "restrictedIntervals"), "4")
        XCTAssertEqual(predicate(restricted, "target", "missedFrames"), "4")
        // A schedule change while running leaves the interval across it without a refresh count; it
        // ran partly on the slower schedule, so it is restricted.
        let changeCursor = try XCTUnwrap(restricted["cursor"] as? [String: Any])
        frames(health, from: 20 + 5.0 / 60, count: 3, hertz: 120)
        XCTAssertEqual(results(metrics(health, cursor: changeCursor)).prefix(2), ["insufficient-evidence/schedule-changed", "restricted/unknown"])

        let scroll = FrameHealth(enabled: true, source: "native-scroll-ui-update")
        defer { scroll.destroy() }
        scroll.record(arrival: 30, target: 30.01, period: 0, finished: 30.001)
        scroll.record(arrival: 30.02, target: 30.03, period: 0, finished: 30.021)
        clock = 30.021
        XCTAssertEqual(results(metrics(scroll)).prefix(2), ["insufficient-evidence/schedule-unknown", "insufficient-evidence/schedule-unknown"])
        scroll.record(arrival: 30.04, target: 30.045, period: 0, finished: 30.05)
        clock = 30.05
        XCTAssertEqual(results(metrics(scroll))[0], "fail/late-work", "a missed completion deadline is observed even with unknown cadence")

        health.clockChanged(virtual: true)
        XCTAssertEqual(results(health.metrics()), Array(repeating: "insufficient-evidence/virtual-clock", count: 3))
        let disabled = FrameHealth(enabled: false)
        XCTAssertEqual(results(disabled.metrics()), Array(repeating: "insufficient-evidence/collection-disabled", count: 3))
    }

    func testStaleAndOutOfOrderEvidenceCannotPass() throws {
        let simulator = FrameHealthAssessment.deadlinesObserved
        FrameHealthAssessment.deadlinesObserved = true
        defer { FrameHealthAssessment.deadlinesObserved = simulator }
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        health.requestedRate(120)
        for index in 0..<10 {
            let time = 1 + Double(index) / 120
            health.record(arrival: time, target: time + 1 / 120, period: 1 / 120, finished: time + 0.001)
        }
        // Read two seconds after the last callback while the source still runs, as after a blocked main thread
        let stale = FrameHealthMetrics.reply(FrameHealthMetrics.read(health, now: 3.1), health: health)
        XCTAssertEqual(stale["status"] as? String, "stale")
        let cadence = (stale["assessments"] as? [String: Any])?["cadence"] as? [String: Any]
        XCTAssertEqual(cadence?["reason"] as? String, "stale")
        // A callback whose timestamp went backwards is rejected, and the work it sent makes no record
        health.sending(FrameHealth.tick, start: 0.5, deadline: 0.508)
        health.record(arrival: 3.2, target: 0.508, period: 0.008, finished: 3.21, sent: FrameHealth.tick)
        health.landed(FrameHealth.tick, at: 3.3)
        XCTAssertEqual(health.snapshot().worked, 10)
        XCTAssertEqual(health.snapshot().out_of_order, 1)
        XCTAssertEqual(health.pendingFrames(at: 4).pending, 0)
    }

    func testDenseCursorsStayInTheFourKBEnvelopeAndMalformedBinsFailClosed() throws {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let snapshot = ExactFrameHealthSnapshot()
        let full = { (count: Int) in Array(repeating: UInt32.max, count: count) }
        let sample = FrameHealthMetrics.Sample(producer: health.handle, snapshot: snapshot,
            intervals: full(64), work: full(64), refreshes: full(32), lateBy: full(32), read: UInt64.max)
        let reply = FrameHealthMetrics.reply(sample, health: health)
        XCTAssertNil(reply["error"])
        XCTAssertLessThanOrEqual(try JSONSerialization.data(withJSONObject: reply).count + 1, 4000)
        XCTAssertEqual((reply["histogram"] as? [String: Any])?["valid"] as? Bool, false)
        var invalid = health.metrics()["cursor"] as! [String: Any]
        invalid["intervals"] = Array(repeating: -1, count: 64)
        XCTAssertEqual(health.metrics(cursor: invalid)["error"] as? String, "baseline-required")
        invalid["intervals"] = Array(repeating: 0, count: 65)
        XCTAssertEqual(health.metrics(cursor: invalid)["error"] as? String, "baseline-required")
        invalid["intervals"] = Array(repeating: 0, count: 64)
        invalid["values"] = Array(repeating: "0", count: 15)
        XCTAssertEqual(health.metrics(cursor: invalid)["error"] as? String, "baseline-required", "a cursor has sixteen values")
    }

    func testPlatformFrameReadsDoNotAllocateAnObserverOrStartTheLegacySampler() throws {
        guard ExactEnv.agentMode, !ExactEnv.agentFreezes else { throw XCTSkip("platform-agent regression") }
        let session = ExactApp.shared.makeSession(label: "read-without-observation")
        defer { session.destroy() }
        session.sampler?.stop(); session.sampler = nil
        let pipe = Pipe(), saved = Agent.out
        Agent.out = pipe.fileHandleForWriting
        defer { Agent.out = saved }
        session.agentInstance.handle(op: "perf", ["frames": true, "source": "scroll", "health": true], line: "")
        let missing = try XCTUnwrap(JSONSerialization.jsonObject(with: pipe.fileHandleForReading.availableData) as? [String: Any])
        XCTAssertEqual(missing["error"] as? String, "native-scroll-unobserved")
        XCTAssertNil(session.frameHealth.nativeHealth)
        session.agentInstance.handle(op: "perf", ["frames": true], line: "")
        let reply = try XCTUnwrap(JSONSerialization.jsonObject(with: pipe.fileHandleForReading.availableData) as? [String: Any])
        XCTAssertEqual(reply["format"] as? String, FrameHealthMetrics.format)
        XCTAssertNil(session.sampler)
    }

    func testLiveCaptureGuardsAllClockRequestsAndInspectorOwnershipIsIndependent() throws {
        let session = ExactApp.shared.makeSession(label: "frame-health-clock-guard")
        defer { session.destroy() }
        session.clock = nil
        let health = session.frameHealth
        health.setInspectorVisible(true)
        _ = health.setAgentCapture("start")
        for request: [String: Any] in [["take": true], ["data": true], ["settle": true], ["to": 500], ["land": true]] {
            XCTAssertEqual(session.agentInstance.clock(request)["code"] as? String, "frame-health-clock-takeover")
            XCTAssertNil(session.clock)
        }
        _ = health.setAgentCapture("stop")
        XCTAssertNotEqual(health.captureID, 0, "closing an agent lease does not close the human inspector")
        health.setInspectorVisible(false)
        XCTAssertEqual(health.captureID, 0)
        XCTAssertNotNil(session.agentInstance.clock(["take": true])["clock"])
    }
    func testTheCardReaderTakesMaximaAndCoversItsWindows() throws {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        health.requestedRate(120)
        let reader = FrameHealthCardReader()
        func run(from start: Double, seconds: Double, hitchAt: Double? = nil) {
            var time = start
            while time < start + seconds {
                if let hitchAt, time >= hitchAt, time < hitchAt + 1.0 / 120 { time += 3.0 / 120 }
                health.record(arrival: time, target: time + 1 / 120, period: 1 / 120, finished: time + 0.004)
                time += 1.0 / 120
            }
        }
        XCTAssertFalse(reader.sample(health, now: 100, force: true), "the first read is a baseline")
        for second in stride(from: 100.0, to: 106.0, by: 0.5) {
            run(from: second, seconds: 0.5, hitchAt: second == 105.5 ? 105.7 : nil)
            XCTAssertTrue(reader.sample(health, now: second + 0.5))
        }
        let now = reader.window(0.5), five = reader.window(5)
        XCTAssertEqual(now.read - now.from, 0.5, accuracy: 1e-9)
        XCTAssertEqual(FrameHealthCardText.window(now, nominal: 0.5), "Last 0.5s")
        XCTAssertEqual(five.read - five.from, 5, accuracy: 1e-9, "ten samples")
        XCTAssertEqual(now.longest, 4 * 1000 / 120.0, accuracy: 0.01, "the hitch's interval, exact")
        XCTAssertEqual(now.slowest, 4, accuracy: 0.01)
        XCTAssertEqual(now.counts.missed, 3)
        XCTAssertFalse(reader.sample(health, now: 106.2), "at most twice a second")
        // A sample after a stall names the span it covers
        run(from: 106, seconds: 0.1)
        XCTAssertTrue(reader.sample(health, now: 108))
        XCTAssertEqual(FrameHealthCardText.window(reader.window(0.5), nominal: 0.5), "Last 2.0s")
        health.reset()
        XCTAssertFalse(reader.sample(health, now: 108.5), "a new generation starts again from a baseline")
        XCTAssertTrue(reader.samples.isEmpty)
    }

    func testAtMostFourReadersKeepHistoryAtOnce() {
        // Four readers keep their history within LLP 1079.000 §5.2's storage budget with the collectors
        let readers = (0..<5).map { _ in FrameHealthCardReader() }
        XCTAssertEqual(readers.prefix(4).map { $0.start() }, [true, true, true, true])
        XCTAssertFalse(readers[4].start(), "a fifth card reads nothing")
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        XCTAssertNil(health.rollingMetrics(), "a Details inspector shares the four places")
        XCTAssertTrue(FrameHealthReadout.detail(health).contains(FrameHealthReaders.full))
        readers[0].stop()
        XCTAssertNotNil(health.rollingMetrics(), "until one closes")
        XCTAssertFalse(readers[4].start())
        health.setInspectorVisible(false)
        XCTAssertTrue(readers[4].start(), "and an inspector that closes frees its place")
        readers.forEach { $0.stop() }
    }

    func testCardFiguresAndPacingReadAsDesigned() {
        typealias T = FrameHealthCardText
        XCTAssertEqual(T.duration(10.5, .up), "10.5ms")
        XCTAssertEqual(T.duration(192.3, .up), "193ms")
        XCTAssertEqual(T.duration(1600, .up), "1.6s")
        XCTAssertEqual(T.durationText(T.workLayout, 63), "≥786s")
        XCTAssertEqual(T.durationText(T.workLayout, 20), "10.5ms")
        let rates = T.Rates(requestedHz: 120, periodMs: 1000 / 120.0)
        var t = FrameHealthCardReader.Window()
        t.counts.frames = 600; t.refreshes[0] = 600
        XCTAssertEqual(T.pacing(t, rates: rates, adaptiveSync: false).0, "Even at 120 FPS")
        XCTAssertEqual(T.pacing(t, rates: rates, adaptiveSync: false).1, "600 frames at 8.3ms")
        t.refreshes[0] = 597; t.refreshes[2] = 2; t.refreshes[5] = 1
        XCTAssertEqual(T.pacing(t, rates: rates, adaptiveSync: false).1, "597 frames at 8.3ms, 3 at 25.0ms to 50.0ms")
        t.refreshes[31] = 4
        XCTAssertEqual(T.pacing(t, rates: rates, adaptiveSync: false).1, "597 frames at 8.3ms, 7 at 25.0ms or more")
        t.refreshes[31] = 0
        // Across a schedule change the verdict says so, with the frames across it, and counts are in refreshes
        t.counts.epochs = 1; t.counts.unclassified = 2
        XCTAssertEqual(T.pacing(t, rates: rates, adaptiveSync: false).0, "Schedule changed, 2 frames across")
        XCTAssertEqual(T.pacing(t, rates: rates, adaptiveSync: false).1, "597 frames at 1 refresh, 3 at 3 to 6 refreshes")
        var long = t
        long.refreshes[0] = 9_999; long.refreshes[2] = 5_000; long.refreshes[31] = 4_999
        XCTAssertEqual(T.pacing(long, rates: rates, adaptiveSync: false, count: { String($0) }).1,
            "9999 frames at 1 refresh, 10000 at 3 refreshes or more", "Copy gives counts in full")
        XCTAssertEqual(T.pacing(long, rates: rates, adaptiveSync: false).1, "9k frames at 1 refresh, 10k at 3 refreshes or more")
        var locked = FrameHealthCardReader.Window()
        locked.counts.frames = 300; locked.refreshes[1] = 300
        XCTAssertEqual(T.pacing(locked, rates: rates, adaptiveSync: false).0, "Locked at 60 FPS")
        XCTAssertEqual(T.deadline(rates), "Deadline: one refresh, 8.3ms (120 FPS)")
        XCTAssertEqual(T.deadline(rates, varied: true), "Deadline: one refresh, now 8.3ms (120 FPS)")
        XCTAssertEqual(T.request(T.Rates(requestedHz: 120, periodMs: 1000 / 60.0)).1, "system limit 60 FPS")
        // A median never reads above the exact maximum it cannot exceed
        var work = FrameHealthCardReader.Window()
        work.counts.worked = 1; work.work[16] = 1; work.slowest = 8.201
        XCTAssertEqual(T.medianWork(work), "8.2ms", "as the maximum reads, not rounded up past it")
        XCTAssertEqual(T.durationText(T.lateLayout, 31), "≥1.5s", "lateness's last bin is open")
        XCTAssertEqual(T.durationText(T.lateLayout, 30), "1.6s")
        // Missed frames are known wherever requested slots passed, even without a frame interval
        var held = FrameHealthCardReader.Window()
        held.counts.requested = 2; held.counts.missed = 2
        XCTAssertEqual(T.figures(held, rates: T.Rates(requestedHz: 0, periodMs: 0), adaptiveSync: false).missed, "2")
        // Lateness without a work record is unknown, not zero
        let figures = T.figures(FrameHealthCardReader.Window(), rates: rates, adaptiveSync: false)
        XCTAssertEqual([figures.fps, figures.missed, figures.late, figures.maxFrame], ["—", "—", "—", "—"])
    }

    func testTheCopyTextNamesEveryFigureWithinTheReportCap() {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        health.requestedRate(120)
        let reader = FrameHealthCardReader()
        reader.sample(health, now: 1, force: true)
        for index in 0..<240 {
            let time = 1 + Double(index) / 120
            health.record(arrival: time, target: time + 1 / 120, period: 1 / 120, finished: time + 0.003)
            if index % 60 == 59 { reader.sample(health, now: time + 0.001) }
        }
        let display = FrameHealthCardText.Display(name: "ProMotion", detail: "adaptive refresh up to 120 Hz", maximumHz: 120, adaptiveSync: false)
        let text = FrameHealthCardText.summary(reader: reader, rates: .init(requestedHz: 120, periodMs: 1000 / 120.0), display: display, paused: false)
        XCTAssertLessThan(text.utf8.count, 4000)
        // Two seconds of samples cover two seconds, whatever the window's nominal span
        XCTAssertTrue(text.contains("Max work time: 3.0ms (last 0.5s), 3.0ms (last 2.0s), 3.0ms (last 2.0s)"), text)
        XCTAssertTrue(text.contains("Frame rate: 120 FPS (last 0.5s)"), text)
        XCTAssertFalse(text.contains("·"), "no middots in the figures")
    }
}
