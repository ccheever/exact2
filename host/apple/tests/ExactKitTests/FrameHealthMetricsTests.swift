import Foundation
import QuartzCore
import SwiftUI
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

    func testTheContinuousOutlineLiesOnSwiftUIsContinuousRoundedRectangle() {
        // Apple's curve: SwiftUI's continuous rounded rectangle has the elements of UIKit's
        // `UIBezierPath(roundedRect:cornerRadius:)`
        let rect = CGRect(x: 0, y: 0, width: 400, height: 400), radius: CGFloat = 100
        let apple = polyline(RoundedRectangle(cornerRadius: radius, style: .continuous).path(in: rect).cgPath)
        let drawn = polyline(FrameHealthCardDrawing.continuous(rect, radius: radius))
        let circular = polyline(CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil))
        let apart = farthest(drawn, apple) / radius, circularApart = farthest(circular, apple) / radius
        XCTAssertLessThan(apart, 0.001, "the drawn outline lies on Apple's curve: \(apart) radii from it at most")
        XCTAssertGreaterThan(circularApart, 0.01, "a circular arc lies \(circularApart) radii from it, so the test tells the two apart")
    }

    func testTheFacesFillsTakeTheContinuousOutline() {
        // The segmented control's top-left corner, its track's and its chosen chip's, drawn at 16 times its size in colors far apart
        var p = Self.palette
        (p.background, p.track, p.chip) = (CGColor(gray: 0, alpha: 1), CGColor(gray: 1, alpha: 1), CGColor(red: 1, green: 0, blue: 0, alpha: 1))
        let layout = FrameHealthCardFace.Layout(width: FrameHealthCardFace.width), chip = layout.segments.0
        let corner = CGRect(x: layout.track.minX, y: layout.track.minY, width: 11, height: 11), scale: CGFloat = 16
        func picture(_ draw: (CGContext) -> Void) -> [UInt8] {
            let side = Int(corner.width * scale)
            var pixels = [UInt8](repeating: 0, count: side * side * 4)
            pixels.withUnsafeMutableBytes { buffer in
                guard let ctx = CGContext(data: buffer.baseAddress, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                          space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return }
                ctx.scaleBy(x: scale, y: scale)
                ctx.translateBy(x: -corner.minX, y: -corner.minY)
                draw(ctx)
            }
            XCTAssertTrue(CapturePixels.hasPaint(pixels), "the face drawing contains paint")
            return pixels
        }
        func fills(_ outline: (CGRect, CGFloat) -> CGPath) -> [UInt8] {
            picture { ctx in
                for (color, path) in [(p.background, CGPath(rect: corner, transform: nil)), (p.track, outline(layout.track, 8)), (p.chip, outline(chip, 6))] {
                    ctx.setFillColor(color)
                    ctx.addPath(path)
                    ctx.fillPath()
                }
            }
        }
        let face = picture { FrameHealthCardFace.draw($0, layout, state: .init(), palette: p) }
        let apart = { (a: [UInt8], b: [UInt8]) in zip(a, b).filter { abs(Int($0) - Int($1)) > 64 }.count }
        XCTAssertEqual(apart(face, fills { FrameHealthCardDrawing.continuous($0, radius: $1) }), 0, "the track and the chip take the continuous outline")
        XCTAssertGreaterThan(apart(face, fills { CGPath(roundedRect: $0, cornerWidth: $1, cornerHeight: $1, transform: nil) }), 100,
                             "and circular corners would draw other pixels")
        XCTExpectFailure("an empty picture cannot establish a different outline") { _ = picture { _ in } }
    }

    /// The outline `path` traces, its curves divided into 64 lines each
    private func polyline(_ path: CGPath) -> [CGPoint] {
        var points: [CGPoint] = []
        path.applyWithBlock { element in
            let p = element.pointee.points, from = points.last ?? .zero
            func sample(_ at: (CGFloat) -> CGPoint) { points += (1...64).map { at(CGFloat($0) / 64) } }
            switch element.pointee.type {
            case .moveToPoint, .addLineToPoint: points.append(p[0])
            case .addCurveToPoint:
                sample { t in let u = 1 - t, (b0, b1, b2, b3) = (u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t)
                    return CGPoint(x: b0 * from.x + b1 * p[0].x + b2 * p[1].x + b3 * p[2].x, y: b0 * from.y + b1 * p[0].y + b2 * p[1].y + b3 * p[2].y) }
            case .closeSubpath: if let first = points.first { points.append(first) }
            default: XCTFail("no outline here has a \(element.pointee.type)")
            }
        }
        return points
    }

    /// The largest distance from a point of either outline to the other: zero when they trace one shape
    private func farthest(_ a: [CGPoint], _ b: [CGPoint]) -> CGFloat {
        func distance(_ p: CGPoint, to line: [CGPoint]) -> CGFloat {
            zip(line, line.dropFirst()).map { s, e in
                let d = CGPoint(x: e.x - s.x, y: e.y - s.y), length = d.x * d.x + d.y * d.y
                let t = length == 0 ? 0 : min(1, max(0, ((p.x - s.x) * d.x + (p.y - s.y) * d.y) / length))
                return hypot(p.x - s.x - t * d.x, p.y - s.y - t * d.y)
            }.min() ?? .infinity
        }
        return max(a.map { distance($0, to: b) }.max() ?? 0, b.map { distance($0, to: a) }.max() ?? 0)
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
        // Following the time, the plots move between frames, and stop once the newest frame has left them
        chart.follow(true)
        chart.tick(at: 106.55)
        XCTAssertEqual(lines[0].position.x, 0, "not within a column of the newest frame, which moved them")
        chart.tick(at: 107.5)
        XCTAssertEqual(lines[0].position.x, -(width / 30 * 2).rounded(.down) / 2, "the layer moves with the time")
        chart.tick(at: 136.5)
        XCTAssertEqual(chart.head, 136.5)
        XCTAssertNil(chart.clock, "flat once the newest frame has left the plots, and nothing moves them")
        chart.frame(at: 140)
        XCTAssertNotNil(chart.clock, "until the next frame")
        // A long spell of the main thread's work is the work of the column it ended in, against one refresh, and the
        // plots reach it without its being a frame
        chart.busy(start: 140.21, end: 140.27, budget: 1 / 120)
        let spell = try XCTUnwrap(chart.columns.first { $0.index == 1402 })
        XCTAssertEqual(spell.work, 60, accuracy: 0.01)
        XCTAssertEqual(spell.budget, 1000 / 120, accuracy: 0.01)
        XCTAssertEqual(spell.frame, 0, "a spell is no frame")
        XCTAssertEqual(chart.newest, 140)
        XCTAssertEqual(chart.latest, 140.27)
        chart.tick(at: 140.3)
        XCTAssertEqual(chart.head, 140.27, "not within a column of the spell, which moved them")
        // Held, the layers keep their picture and the charts take no frames, so the paused span is a gap; let go, they
        // show the present
        chart.present()
        let shown = lines[0].position.x
        chart.hold(true, at: 140.4)
        XCTAssertNil(chart.clock, "a held picture does not follow the time")
        chart.frame(at: 141)
        chart.busy(start: 141.1, end: 141.2, budget: 1 / 120)
        chart.present()
        XCTAssertEqual(lines[0].position.x, shown)
        XCTAssertEqual(chart.latest, 140.27, "the picture is the moment it was held")
        XCTAssertFalse(chart.columns.contains { $0.index >= 1410 })
        chart.hold(false, at: 150)
        XCTAssertEqual(chart.head, 150, "the present, past the pause")
        XCTAssertEqual(lines[0].position.x, 0, "drawn whole at the present")
        chart.frame(at: 150.15)
        XCTAssertEqual(chart.columns.first { $0.index == 1501 }?.joined, false, "the next frame begins a new run")
        chart.follow(false)
        XCTAssertNil(chart.clock)
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
                        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
                    ctx.scaleBy(x: 2, y: 2)
                    layer.render(in: ctx)
                }
                XCTAssertTrue(CapturePixels.hasPaint(bytes), "the chart layer draws")
                return bytes
            }
            func shown() {
                chart.present(flush: true)
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
            let empty = CALayer()
            empty.bounds = layers[0].bounds
            XCTExpectFailure("an empty chart layer cannot establish matching redraws") { _ = pixels(empty) }
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
            FrameHealthCardDrawing.plotLines(ctx, chart: 1, plot: plot, columns: chart.columns, latest: chart.latest,
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

    func testAReleasedCardRestsAnywhereAlongTheNearestEdge() {
        typealias Position = FrameHealthCardFace.Position
        let area = CGRect(x: 8, y: 8, width: 800, height: 1000), size = CGSize(width: 320, height: 416)
        for fraction: CGFloat in [0.17, 0.5, 0.83] {
            let x = area.minX + fraction * (area.width - size.width)
            let y = area.minY + fraction * (area.height - size.height)
            for (release, target) in [
                (CGPoint(x: 20, y: y), CGPoint(x: 8, y: y)),
                (CGPoint(x: 470, y: y), CGPoint(x: 488, y: y)),
                (CGPoint(x: x, y: 20), CGPoint(x: x, y: 8)),
                (CGPoint(x: x, y: 580), CGPoint(x: x, y: 592))
            ] {
                let position = Position(nearest: release, velocity: .zero, size: size, in: area)
                let landed = position.origin(of: size, in: area)
                XCTAssertEqual(landed.x, target.x, accuracy: 0.001)
                XCTAssertEqual(landed.y, target.y, accuracy: 0.001)
            }
        }
        // Normal deceleration carries a release at 1,000 points a second 499 points
        let glide = Position(nearest: CGPoint(x: 20, y: 500), velocity: CGVector(dx: 0, dy: -1000), size: size, in: area)
        XCTAssertEqual(glide.origin(of: size, in: area), CGPoint(x: 20, y: 8))
        let side = Position(nearest: CGPoint(x: 20, y: 300), velocity: CGVector(dx: 1000, dy: 100), size: size, in: area)
        XCTAssertEqual(side.origin(of: size, in: area).x, 488)
        XCTAssertEqual(side.origin(of: size, in: area).y, 349.9, accuracy: 0.001)
        let middle = Position(nearest: CGPoint(x: 20, y: 300), velocity: .zero, size: size, in: area)
        let smaller = CGRect(x: 8, y: 8, width: 600, height: 708)
        XCTAssertEqual(middle.origin(of: size, in: smaller), CGPoint(x: 8, y: 154))
        XCTAssertEqual(middle.origin(of: CGSize(width: 900, height: 1200), in: area), area.origin)
        XCTAssertEqual(middle.origin(of: size, in: area), CGPoint(x: 8, y: 300), "a temporary size constraint preserves the chosen position")
        XCTAssertEqual(Position(nearest: CGPoint(x: -1000, y: 2000), velocity: .zero, size: size, in: area).origin(of: size, in: area), CGPoint(x: 8, y: 592))
        XCTAssertEqual(Position(nearest: .zero, velocity: .zero, size: size, in: CGRect(x: 8, y: 8, width: 0, height: 0)).origin(of: size, in: area), area.origin)
    }

    func testADragKeepsItsReleaseVelocityAndResistsPastTheEdges() {
        let area = CGRect(x: 8, y: 8, width: 400, height: 800), size = CGSize(width: 320, height: 416)
        var drag = FrameHealthCardFace.Drag(at: .zero, time: 1, origin: .zero, control: nil)
        drag.track(CGPoint(x: 10, y: 0), at: 1.01)
        // A lift at the last point keeps the momentum; one after the press held still has none
        drag.track(CGPoint(x: 10, y: 0), at: 1.011)
        XCTAssertEqual(drag.velocity(releasedAt: 1.011).dx, 1000, accuracy: 0.001)
        XCTAssertEqual(drag.velocity(releasedAt: 1.07), .zero)
        // Dragged past an edge, the card goes part of the way, the less the further, and never a whole span
        let resisted = { (x: CGFloat) in FrameHealthCardFace.resisted(CGPoint(x: x, y: 100), size: size, in: area).x }
        XCTAssertEqual(FrameHealthCardFace.resisted(CGPoint(x: 40, y: 100), size: size, in: area), CGPoint(x: 40, y: 100), "within the area, under the press")
        // 100 points past the left edge of a 400-point area
        let ratio: CGFloat = 0.55 * 100 / 400 + 1
        XCTAssertEqual(resisted(-92), 8 - (1 - 1 / ratio) * 400, accuracy: 0.001)
        XCTAssertGreaterThan(resisted(-92), -92)
        XCTAssertGreaterThan(resisted(-92) - resisted(-192), 0, "further still goes further")
        XCTAssertLessThan(resisted(-92) - resisted(-192), 100, "but less than the press")
        XCTAssertGreaterThan(resisted(-1e6), 8 - 400)
        XCTAssertGreaterThan(resisted(500), 88, "past the far edge too")
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

    func testLiveCaptureGuardsAllClockRequestsAndTheCardsCaptureIsIndependent() throws {
        let session = ExactApp.shared.makeSession(label: "frame-health-clock-guard")
        defer { session.destroy() }
        session.clock = nil
        let health = session.frameHealth
        health.setHUDVisible(true)
        _ = health.setAgentCapture("start")
        for request: [String: Any] in [["take": true], ["data": true], ["settle": true], ["to": 500], ["land": true]] {
            XCTAssertEqual(session.agentInstance.clock(request)["code"] as? String, "frame-health-clock-takeover")
            XCTAssertNil(session.clock)
        }
        _ = health.setAgentCapture("stop")
        XCTAssertNotEqual(health.captureID, 0, "closing an agent lease does not end the card's capture")
        health.setHUDVisible(false)
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
        // A read moments later, as at a lifecycle boundary, has no frame, so it names no span
        XCTAssertTrue(reader.sample(health, now: 108.01, force: true))
        XCTAssertEqual(FrameHealthCardText.window(reader.window(0.5), nominal: 0.5), "Last 0.5s", "not Last 0.0s")
        health.reset()
        XCTAssertFalse(reader.sample(health, now: 108.5), "a new generation starts again from a baseline")
        XCTAssertTrue(reader.samples.isEmpty)
    }

    func testAtMostFourReadersKeepHistoryAtOnce() {
        // Four readers keep their history within LLP 1079.000 §5.2's storage budget with the collectors
        let readers = (0..<5).map { _ in FrameHealthCardReader() }
        XCTAssertEqual(readers.prefix(4).map { $0.start() }, [true, true, true, true])
        XCTAssertFalse(readers[4].start(), "a fifth card reads nothing")
        readers[0].stop()
        XCTAssertTrue(readers[4].start(), "a card that stops frees its place")
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
            "9999 frames at 1 refresh, 10000 at 3 refreshes or more", "the shared text gives counts in full")
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

    func testTheScreensFiguresNameItsFramesAndTheMainThreadsWork() {
        typealias T = FrameHealthCardText
        let none = T.Rates(requestedHz: 0, periodMs: 0), idle = T.Clock(running: false, rates: none)
        let source = T.Source.screen(refreshMs: 1000 / 120, exact: idle)
        XCTAssertEqual(T.context(none, source: source).0, "This app's frames")
        XCTAssertEqual(T.clock(idle), "Exact's clock idle")
        XCTAssertEqual(T.clock(.init(running: true, rates: .init(requestedHz: 0, periodMs: 1000 / 60))), "Exact's clock at the default rate")
        XCTAssertEqual(T.clock(.init(running: true, rates: .init(requestedHz: 120, periodMs: 1000 / 60))), "Exact asks 120 FPS, limit 60")
        XCTAssertEqual(T.rows(source).map(\.0).prefix(3), ["Frame rate", "Late frames", "Frames"], "the face gives late frames beside the rate")
        XCTAssertFalse(T.rows(source).map(\.0).contains("Max frame time"))
        XCTAssertEqual(T.rows(source).last?.0, "Main thread over 8.3ms")
        XCTAssertEqual(T.caption(none, source: source, varied: false), "Late: sent after UIKit's deadline")
        // The screen's frames bound no frame time: its rate is its frames over the time a window covers, and the main
        // thread's work stands in for pacing
        var t = FrameHealthCardReader.Window()
        (t.from, t.read) = (10, 10.5)
        XCTAssertEqual(T.pacing(t, rates: none, adaptiveSync: false, source: source).0, "Longest 0.0ms")
        XCTAssertEqual(T.pacing(t, rates: none, adaptiveSync: false, source: source).1, "None over one refresh, 8.3ms")
        XCTAssertEqual(T.figures(t, rates: none, adaptiveSync: false, source: source).fps, "—", "no frames, no rate")
        t.counts.callbacks = 60; t.counts.worked = 60; t.counts.late = 3; t.counts.long = 2; t.mainLongest = 61.2
        let screen = T.figures(t, rates: none, adaptiveSync: false, source: source)
        XCTAssertEqual([screen.fps, screen.frames, screen.late, screen.mainLongest, screen.mainLong, screen.changes],
            ["120 FPS", "60", "3", "61.2ms", "2", "—"])
        XCTAssertEqual(T.pacing(t, rates: none, adaptiveSync: false, source: source).1, "2 over one refresh, 8.3ms")
        // Over a window that covers more than its nominal span, the rate is over the span it covers, which it names
        t.from = 9.5
        XCTAssertEqual(T.figures(t, rates: none, adaptiveSync: false, source: source).fps, "60 FPS")
        XCTAssertEqual(T.window(t, nominal: 0.5, source: source), "Last 1.0s")
        let exact = T.figures(t, rates: none, adaptiveSync: false)
        XCTAssertEqual([exact.fps, exact.frames, exact.mainLongest, exact.mainLong], ["—", "—", "—", "—"])
        // The screen's work chart has a gridline at each whole refresh at the display's highest rate, the first dotted,
        // and draws no deadline
        let area = CGRect(x: 12, y: 206, width: 296, height: 200)
        let work = FrameHealthCardDrawing.plots(area, .init(reader: FrameHealthCardReader(), rates: none, displayMaximumHz: 120, screen: true),
            expanded: nil)[1].plot
        XCTAssertEqual(work.labels.map(\.0), ["", "8.3", "16.7", "25", "33.3ms"])
        for (label, refreshes) in zip(work.labels, 0...4) {
            XCTAssertEqual(label.1, Double(refreshes) * 1000 / 120, accuracy: 1e-9, "whole refreshes at 120 Hz")
        }
        XCTAssertEqual(work.labels.map(\.2), [.axis, .dotted, .grid, .grid, .grid])
        XCTAssertFalse(work.deadlines)
        // Exact's frame-time chart has its gridlines at whole refreshes too, plain, as it draws each frame's deadline
        let frames = FrameHealthCardDrawing.plots(area, .init(reader: FrameHealthCardReader(), rates: none, displayMaximumHz: 120),
            expanded: nil)[1].plot
        XCTAssertEqual(frames.labels.map(\.0), ["", "8.3", "16.7", "25", "33.3ms"])
        XCTAssertEqual(frames.labels.map(\.2), [.axis, .grid, .grid, .grid, .grid])
        XCTAssertTrue(frames.deadlines, "Exact's frame-time chart draws each frame's deadline")
    }

    func testTheScreensFrameRateIsItsFramesOfTheLastHalfSecondAndFallsAsTheyLeave() {
        let chart = FrameHealthChart(in: CALayer())
        chart.start()
        for n in 0...120 { chart.screenFrame(at: 100 + Double(n) / 120, work: 0.002, budget: 1 / 120.0) }
        let column = { (t: Double) in chart.columns[Int(Int64((t / FrameHealthChart.columnSeconds).rounded(.down)) % 300)] }
        XCTAssertEqual(column(101).fps, 120, accuracy: 2)
        XCTAssertEqual(column(101).work, 2, accuracy: 0.01)
        XCTAssertEqual(column(101).budget, 1000 / 120, accuracy: 0.01)
        XCTAssertFalse(chart.columns.contains { $0.frame > 0 }, "the screen's frames bound no frame time")
        // Once motion ends, the line falls as the frames leave the last half second, as the figures do
        chart.tick(at: 101.25)
        XCTAssertEqual(column(101.25).fps, 60, accuracy: 4)
        chart.tick(at: 101.6)
        XCTAssertEqual(column(101.6).fps, 0)
    }

    func testTheScreensReportGivesNoFrameTimes() throws {
        let screen = FrameHealth(policy: .always, source: FrameHealth.screenSource), exact = FrameHealth(enabled: true)
        defer { screen.destroy(); exact.destroy() }
        // The card shows, so the collector keeps its late frames, which a report gives within two minutes
        screen.setHUDVisible(true)
        let now = CACurrentMediaTime()
        screen.record(arrival: now - 0.02, target: now - 0.012, period: 0, finished: now - 0.01)
        let report = FrameHealthReport.snapshot(health: screen, session: nil, now: now)
        let core = report["coreSinceReload"] as? [String: Any] ?? [:]
        XCTAssertEqual(report["evidence"] as? String, "ui-update-commit-deadlines")
        for key in ["frameIntervals", "activeNs", "maximumFrameIntervalNs", "scheduledPeriodNs", "missedFrames"] {
            XCTAssertTrue(core[key] is NSNull, key)
        }
        XCTAssertEqual(core["lateFrames"] as? String, "1", "UIKit's deadlines hold on the simulator too")
        XCTAssertEqual((report["unobserved"] as? [String])?.first, "frame times")
        // Its rows give only what it measures: a late frame's work against its deadline, and its neighbours' work
        XCTAssertEqual(report["columns"] as? [String], ["sequence", "capture", "generation", "age_us", "flags", "target_offset_us",
            "callback_us", "before_callback_us", "after_offset_us", "after_callback_us", "waited_us", "exact_us", "card_us", "layouts"])
        let row = try XCTUnwrap((report["hitches"] as? [[Any]])?.first)
        XCTAssertEqual(row.count, 14)
        XCTAssertTrue(row[10...].allSatisfy { $0 is NSNull }, "a frame the screen did not split")
        XCTAssertEqual(row[5] as? Int, 8_000, "the deadline, after the update began")
        XCTAssertEqual(row[6] as? UInt32, 10_000, "the work, from the update's start to its last commit")
        XCTAssertTrue(row[8] is NSNull && row[9] is NSNull, "its next frame was not seen")
        let exactReport = FrameHealthReport.snapshot(health: exact, session: nil)
        XCTAssertEqual((exactReport["columns"] as? [String])?.count, 23)
        let exactCore = exactReport["coreSinceReload"] as? [String: Any] ?? [:]
        XCTAssertEqual(exactCore["frameIntervals"] as? String, "0")
    }

    func testEachReadTakesTheMainThreadsWorkAndTheReadAfterAPauseCoversIt() {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        let reader = FrameHealthCardReader()
        XCTAssertTrue(reader.start())
        defer { reader.stop() }
        var taken = 0
        reader.sample(health, now: 1, force: true, main: { taken += 1; return (0.004, 0) })
        health.record(arrival: 1.1, target: 1.11, period: 0, finished: 1.105)
        reader.sample(health, now: 1.6, force: true, main: { taken += 1; return (0.061, 2) })
        XCTAssertEqual(taken, 2, "each read takes the main thread's work, the baseline's too, so none carries over")
        XCTAssertEqual(reader.window(5).mainLongest, 61, accuracy: 1e-9)
        XCTAssertEqual(reader.window(5).counts.long, 2)
        // A paused card reads nothing; its first read after covers the paused span, from the last read before it
        health.record(arrival: 1.7, target: 1.71, period: 0, finished: 1.705)
        health.record(arrival: 3.7, target: 3.71, period: 0, finished: 3.705)
        reader.sample(health, now: 4.6, main: { (0.001, 0) })
        XCTAssertEqual(reader.window(0.5).from, 1.6, accuracy: 1e-9)
        XCTAssertEqual(reader.window(0.5).counts.callbacks, 2)
        XCTAssertEqual(reader.samples.count, 2)
    }

    func testAProductionBuildCollectsOnlyWhileTheCardShows() {
        let off = FrameHealth(enabled: false), demand = FrameHealth(policy: .onDemand)
        defer { off.destroy(); demand.destroy() }
        XCTAssertEqual(off.unavailableExplanation?.title, "Frame Health Is Off")
        XCTAssertTrue(off.unavailableExplanation?.message.contains("\"frame_health\": { \"mode\": \"off\" } in app.json") == true,
                      "it names the setting that turns it off")
        // Hidden, an on-demand collector can read but collects nothing, and has no collector yet
        XCTAssertTrue(demand.available)
        XCTAssertNil(demand.unavailableExplanation)
        XCTAssertFalse(demand.enabled || demand.observesNativeScrolling)
        XCTAssertEqual(demand.handle, 0)
        demand.requestedRate(120)
        // Seen, the card has it collect, under the rate requested before
        demand.hudEffectiveVisible = true
        XCTAssertTrue(demand.enabled && demand.observesNativeScrolling)
        XCTAssertEqual(demand.requestedHertz, 120)
        demand.record(arrival: 2, target: 2.008, period: 0.008, finished: 2.001)
        XCTAssertEqual(demand.snapshot().callbacks, 1)
        let generation = demand.snapshot().generation
        demand.hudEffectiveVisible = false
        XCTAssertFalse(demand.enabled || demand.observesNativeScrolling)
        XCTAssertNotEqual(demand.handle, 0, "its collector stays for the next demand")
        // Seen again, it collects in a new generation, which holds nothing collected before
        demand.hudEffectiveVisible = true
        XCTAssertGreaterThan(demand.snapshot().generation, generation)
        XCTAssertEqual(demand.snapshot().callbacks, 0)
    }

    func testTheHeaderButtonsFitApartAtTheMinimumWidth() {
        let layout = FrameHealthCardFace.Layout(width: FrameHealthCardFace.minimumWidth)
        let header = [layout.close, layout.title, layout.status, layout.details, layout.share]
        for (left, right) in zip(header, header.dropFirst()) { XCTAssertLessThanOrEqual(left.maxX + 2, right.minX, "\(left) and \(right)") }
        XCTAssertEqual(layout.share.maxX, layout.width - 6)
        // Details and Share are 4 points apart, so their touch targets overlap; each takes its own half
        XCTAssertEqual(layout.control(at: CGPoint(x: layout.details.maxX + 1, y: 22)), .details)
        XCTAssertEqual(layout.control(at: CGPoint(x: layout.share.minX - 1, y: 22)), .share)
        XCTAssertEqual(layout.control(at: CGPoint(x: layout.title.minX + 1, y: 22)), .title, "within the title, not Close's target")
    }

    func testTheHeaderKeepsClearOfRounderTopCorners() {
        let plain = FrameHealthCardFace.Layout(width: FrameHealthCardFace.width), width = FrameHealthCardFace.width
        // A 51-point corner, as the folded iPhone Duo's screen gives a card in its top-right corner
        let right = FrameHealthCardFace.Layout(width: width, corners: (12, 51))
        XCTAssertEqual(right.share.maxX, plain.share.maxX - 0.2915 * 39, accuracy: 0.01, "Share moves in")
        XCTAssertEqual(right.share.minX - right.details.minX, plain.share.minX - plain.details.minX, accuracy: 0.001, "and Details with it")
        XCTAssertEqual(right.close, plain.close)
        // Close moves in as far as the 6 points the title leaves it, and the title keeps its place
        XCTAssertEqual(plain.title.minX - plain.close.maxX, 2 + 6, "the title stands 6 points beyond Close's 2")
        let left = FrameHealthCardFace.Layout(width: width, corners: (51, 12))
        XCTAssertEqual(left.close.minX, plain.close.minX + 6, accuracy: 0.001, "Close moves in, as far as the title allows")
        XCTAssertEqual(left.title.minX - left.close.maxX, 2, accuracy: 0.001)
        XCTAssertEqual(FrameHealthCardFace.Layout(width: width, corners: (20, 12)).close.minX, plain.close.minX + 0.2915 * 8, accuracy: 0.01)
        XCTAssertEqual(left.title, plain.title, "the title stays")
        XCTAssertEqual(left.share, plain.share)
        // The narrowest card, both corners rounded: the title's room and the moves shrink to keep the header apart
        let both = FrameHealthCardFace.Layout(width: FrameHealthCardFace.minimumWidth, corners: (51, 51))
        let header = [both.close, both.title, both.status, both.details, both.share]
        for (a, b) in zip(header, header.dropFirst()) { XCTAssertLessThanOrEqual(a.maxX + 2, b.minX + 0.001, "\(a) and \(b)") }

        // Close pressed in the 51-point corner, drawn at 1x across the corner alone in colors far apart
        var p = Self.palette
        (p.background, p.iconPressed) = (CGColor(gray: 0, alpha: 1), CGColor(red: 1, green: 0, blue: 0, alpha: 1))
        var state = FrameHealthCardFace.State()
        state.pressed = .close
        let side = 48
        var pixels = [UInt8](repeating: 0, count: side * side * 4)
        pixels.withUnsafeMutableBytes { buffer in
            guard let ctx = CGContext(data: buffer.baseAddress, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                      space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return }
            // The face's y runs down, as a view's does
            ctx.translateBy(x: 0, y: CGFloat(side))
            ctx.scaleBy(x: 1, y: -1)
            FrameHealthCardFace.draw(ctx, left, state: state, palette: p)
        }
        func red(_ point: CGPoint) -> Bool {
            let i = (Int(point.y) * side + Int(point.x)) * 4
            return pixels[i] > 200 && pixels[i + 1] < 60
        }
        XCTAssertTrue(red(CGPoint(x: left.close.minX + 4, y: left.close.maxY - 4)), "the pressed fill shows")
        // Inside the fill's own corner, but outside the card's outline inset by 2 points
        XCTAssertFalse(red(CGPoint(x: left.close.minX + 4, y: left.close.minY + 4)), "but not past the card's rounded corner")
    }

    func testTheSharedTextNamesEveryFigureWithinTheReportCap() {
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
        let text = FrameHealthCardText.summary(reader: reader, rates: .init(requestedHz: 120, periodMs: 1000 / 120.0), display: display,
            paused: false, health: health).text
        XCTAssertLessThan(text.utf8.count, 4000)
        // Two seconds of samples cover two seconds, whatever the window's nominal span
        XCTAssertTrue(text.contains("Max work time: 3.0ms (last 0.5s), 3.0ms (last 2.0s), 3.0ms (last 2.0s)"), text)
        XCTAssertTrue(text.contains("Frame rate: 120 FPS (last 0.5s)"), text)
        XCTAssertFalse(text.contains("·"), "no middots in the figures")
        XCTAssertTrue(text.contains("\nCadence (last 2.0s): "), "and Details' results: \(text)")
    }

    func testDetailsGivesTheCollectorsResultsOverTheLast5sAndTheCountsBehindThem() {
        let health = FrameHealth(enabled: true)
        defer { health.destroy() }
        health.requestedRate(120)
        let reader = FrameHealthCardReader()
        XCTAssertFalse(reader.sample(health, now: 100, force: true))
        var time = 100.0
        for second in stride(from: 100.0, to: 106.0, by: 0.5) {
            while time < second + 0.5 {
                // One frame late by a whole refresh in the newest five seconds, with its slot missed
                if abs(time - 105.2) < 0.004 { time += 1.0 / 120 }
                health.record(arrival: time, target: time + 1 / 120, period: 1 / 120, finished: time + (abs(time - 105.0) < 0.004 ? 0.012 : 0.004))
                time += 1.0 / 120
            }
            reader.sample(health, now: second + 0.5)
        }
        let rows = FrameHealthCardText.details(reader: reader, health: health)
        let five = reader.window(5).counts
        XCTAssertEqual(rows.map(\.name), ["Cadence", "Target rate", "Content latency", "Frames", "Requested slots", "Missed frames",
            "Skipped refreshes", "Late frames", "Limited by the system"])
        XCTAssertEqual(rows[1].value, "Fail")
        XCTAssertEqual(rows[1].reason, "missed frames")
        XCTAssertEqual(rows[2].value, "Not enough evidence")
        XCTAssertEqual(rows[2].reason, "not observed")
        XCTAssertEqual(rows[3].value, String(five.frames))
        XCTAssertEqual(rows[5].value, String(five.missed))
        XCTAssertGreaterThan(five.missed, 0)
        XCTAssertEqual(rows[6].value, String(five.skipped))
        XCTAssertNil(rows[3].reason, "a count has no reason")
        // The same rules as the collector's own reply over the same span
        let lateness = FrameHealthAssessment.deadlinesObserved
        XCTAssertEqual(rows[0].value, five.skipped > 0 || (lateness && five.late > 0) ? "Fail" : "Pass by proxy")
        XCTAssertEqual(FrameHealthCardText.detailsCaption(reader: reader),
            "Last 5s. Exact does not observe presentation, compositor or GPU time, or input latency.")
    }

    func testDetailsRowsScrollWhereTheyOutgrowTheCardAndTheCardKeepsItsSize() {
        let rows = (0..<3).map { FrameHealthCardText.Detail(name: "Result \($0)", value: "Fail", reason: "late work") }
            + (0..<6).map { FrameHealthCardText.Detail(name: "Count \($0)", value: "0") }
        let caption = "Last 5s. Exact does not observe presentation, compositor or GPU time, or input latency."
        let full = FrameHealthCardFace.Layout(width: FrameHealthCardFace.width, showsDetails: true)
        let height = FrameHealthCardFace.detailsHeight(width: full.width, rows: rows, caption: caption)
        XCTAssertEqual(full.detailsArea.maxY, FrameHealthCardFace.height, "Details fills the card below the header")
        XCTAssertLessThanOrEqual(height, full.detailsArea.height, "today's rows fit the whole card")
        let compact = FrameHealthCardFace.Layout(width: FrameHealthCardFace.width, compact: true, showsDetails: true)
        XCTAssertGreaterThan(height, compact.detailsArea.height, "and scroll on a compact card, which stays its height")
        XCTAssertNil(full.control(at: CGPoint(x: full.segments.0.midX, y: full.segments.0.midY)), "Charts and Table are not there")
    }
}
