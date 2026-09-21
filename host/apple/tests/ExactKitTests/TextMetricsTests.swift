// JSON presentation and the measurement ABI must resolve identical text metrics.
// @ref LLP 1008 §3, LLP 1001 §5
#if os(macOS)
import AppKit
import IOSurface
import CoreText
import CExact
import XCTest
@testable import ExactKit

final class TextMetricsTests: XCTestCase {
    func testUnbreakableLineRasterIsBoundedAndContainsVisibleInk() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "unbreakable-raster")
        let presenter = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 200),
                              styleMask: [], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        defer { window.close(); session.destroy() }
        let node = NodeView(id: 1, kind: "text", presenter: presenter)
        node.applyStyle(["font_size": 16.0, "line_height": "24px"])
        node.applyProps(set: ["text": String(repeating: "x", count: 65_536)], clear: [])
        node.frame = NSRect(x: 0, y: 0, width: 400, height: 24)
        node.prepareToMount()
        presenter.root.addSubview(node)
        let paragraph = try XCTUnwrap(node.paragraphLayout())
        XCTAssertEqual(paragraph.lines.count, 1)
        XCTAssertGreaterThan(paragraph.width, 100_000)
        XCTAssertTrue(presenter.textRasters.ensure(node, urgent: true))
        let surface = try XCTUnwrap(node.textRaster)
        XCTAssertTrue(node.textRasterReady)
        XCTAssertFalse(node.needsTextRaster)
        XCTAssertLessThanOrEqual(node.textRasterFrame.width, node.bounds.width + 2 * TextRasterizer.maxInkOverflow)
        XCTAssertLessThanOrEqual(node.textRasterFrame.height, node.bounds.height + 2 * TextRasterizer.maxInkOverflow)
        surface.lock(options: .readOnly, seed: nil)
        defer { surface.unlock(options: .readOnly, seed: nil) }
        let bytes = surface.baseAddress.assumingMemoryBound(to: UInt8.self)
        let left = Int(-node.textRasterFrame.minX * node.textRasterScale)
        let right = left + Int(node.bounds.width * node.textRasterScale)
        XCTAssertTrue((0..<surface.height).contains { y in
            (left..<right).contains { x in bytes[y * surface.bytesPerRow + x * 4 + 3] != 0 }
        }, "the visible part of the unbreakable line must contain pixels")
    }

    func testFailedRasterDrawsAndStopsRetryingUntilInvalidated() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "failed-raster")
        let presenter = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 200),
                              styleMask: [], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        defer { window.close(); session.destroy() }
        let node = NodeView(id: 1, kind: "text", presenter: presenter)
        node.applyStyle(["font_size": 16.0])
        node.applyProps(set: ["text": "Fallback ink"], clear: [])
        node.frame = NSRect(x: 0, y: 0, width: 400, height: 30)
        node.prepareToMount()
        presenter.root.addSubview(node)
        XCTAssertTrue(presenter.textRasters.ensure(node, urgent: true))
        // Simulate a failed replacement after this node already had pixels.
        let key = try XCTUnwrap(node.textRasterKey)
        node.textRasterReady = false
        node.showTextRaster(nil, for: key)
        XCTAssertNil(node.textRaster)
        XCTAssertNil(node.layer?.contents)
        XCTAssertFalse(node.wantsUpdateLayer, "AppKit must use draw after surface failure")
        for _ in 0..<3 {
            XCTAssertFalse(node.needsTextRaster)
            XCTAssertTrue(presenter.textRasters.ensure(node, urgent: true))
            XCTAssertEqual(node.textRasterKey, key)
            XCTAssertNil(node.textRaster)
        }
        let bitmap = try XCTUnwrap(node.bitmapImageRepForCachingDisplay(in: node.bounds))
        node.cacheDisplay(in: node.bounds, to: bitmap)
        XCTAssertTrue((0..<bitmap.pixelsHigh).contains { y in
            (0..<bitmap.pixelsWide).contains { x in (bitmap.colorAt(x: x, y: y)?.alphaComponent ?? 0) > 0 }
        })
        node.frame.size.width += 10
        node.textRasterGeometryChanged() // the presenter's frame operation
        XCTAssertTrue(node.rastersText, "a new geometry may try again")
        XCTAssertTrue(presenter.textRasters.ensure(node, urgent: true))
        XCTAssertNotNil(node.textRaster)
        node.textRasterReady = false
        node.showTextRaster(nil, for: try XCTUnwrap(node.textRasterKey))
        node.applyProps(set: ["text": "Changed ink"], clear: [])
        node.invalidateText() // the presenter's props operation
        XCTAssertTrue(node.rastersText, "new content may try again")
        XCTAssertTrue(presenter.textRasters.ensure(node, urgent: true))
        XCTAssertNotNil(node.textRaster)
    }

    func testDenseInlineCountsDoNotCreateNativeViews() {
        for repetitions in [16, 256, 4096] {
            let presenter = Presenter()
            let rows: [[String: Any]] = (0..<(repetitions * 8)).map { i in
                ["id": i + 2, "parent": 1, "paint": true,
                 "props": ["text": "run", "testId": "run-\(i)"], "style": ["font_weight": i % 2 == 0 ? 700 : 400]]
            }
            presenter.apply(Batch(ops: [["op": "create", "id": 1, "kind": "text"],
                ["op": "paragraph", "id": 1, "runs": rows]], timers: false, motion: false, clock: nil, error: nil))
            XCTAssertEqual(presenter.views.count, 1)
            XCTAssertEqual(presenter.inlineOwners.count, repetitions * 8)
            XCTAssertEqual(presenter.views[1]?.paragraphSpec().runs.count, repetitions * 8)
        }
    }


    func testInlineOwnerTransferDoesNotEraseTheNewOwner() throws {
        let p = Presenter()
        let old = NodeView(id: 1, kind: "text", presenter: p)
        let next = NodeView(id: 2, kind: "text", presenter: p)
        p.views[1] = old; p.views[2] = next
        p.applyParagraph(1, [["id": 3, "parent": 1, "paint": true, "props": ["text": "retained"]]])
        p.applyParagraph(2, [["id": 3, "parent": 2, "paint": true, "props": ["text": "retained"]]])
        p.applyParagraph(1, [])
        XCTAssertTrue(p.textHost(3) === next)
        XCTAssertEqual(p.inlineText(3)?.props["text"], "retained")
        p.forgetParagraph(next)
        XCTAssertNil(p.textHost(3))
    }

    func testInlineRangesEventsAppearanceAndMetricReuse() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "inline-metadata")
        defer { session.destroy() }
        let p = session.presenter
        let node = NodeView(id: 1, kind: "text", presenter: p)
        p.views[1] = node
        node.frame = NSRect(x: 0, y: 0, width: 300, height: 120)
        node.appearance = NSAppearance(named: .aqua)
        let value = "👩‍🚀 link e\u{301}"
        func rows(_ size: Double) -> [[String: Any]] { [
            ["id": 2, "parent": 1, "paint": true, "props": ["text": "Plain ", "href": ""]],
            ["id": 3, "parent": 1, "paint": false, "props": ["href": "file:///example.md", "testId": "link"], "handlers": ["press", "hover"]],
            ["id": 4, "parent": 3, "paint": true, "props": ["text": value],
             "style": ["font_size": size, "text_color": [[10.0, 20.0, 30.0, 255.0], [210.0, 220.0, 230.0, 255.0]]]],
        ] }
        p.applyParagraph(1, rows(16))
        XCTAssertEqual(p.views.count, 1)
        XCTAssertEqual(p.inlineText(3)?.range, NSRange(location: 6, length: value.utf16.count))
        XCTAssertEqual(p.inlineText(4)?.range, p.inlineText(3)?.range)
        XCTAssertFalse(node.activateInline(2), "an empty href is ordinary text")
        let elements = try XCTUnwrap(node.accessibilityChildren() as? [NSAccessibilityElement])
        XCTAssertEqual(elements.count, 1)
        XCTAssertEqual(elements.first?.accessibilityRole(), .link)
        XCTAssertEqual(elements.first?.accessibilityLabel(), value)
        XCTAssertEqual(elements.first?.accessibilityIdentifier(), "link")
        let old = try XCTUnwrap(node.paragraphLayout())
        let rect = try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(4))).first)
        let point = CGPoint(x: rect.midX, y: rect.midY)
        XCTAssertEqual(node.inlineTarget(at: point, handler: "press")?.id, 3)
        XCTAssertEqual(node.inlineLink(at: point), "file:///example.md")
        XCTAssertNil(node.inlineLink(at: CGPoint(x: 299, y: 119)))
        var pressed: UInt32?
        var hovered: [Bool] = []
        p.onPress = { pressed = $0 }
        p.onHover = { id, over in XCTAssertEqual(id, 3); hovered.append(over) }
        XCTAssertTrue(node.activateInline(3)); XCTAssertEqual(pressed, 3)
        p.hoverInline(3); p.hoverInline(nil); XCTAssertEqual(hovered, [true, false])
        node.appearance = NSAppearance(named: .darkAqua)
        node.viewDidChangeEffectiveAppearance()
        XCTAssertEqual(node.paragraphSpec().runs[1].color, [210, 220, 230, 255])
        let colored = try XCTUnwrap(node.paragraphLayout())
        XCTAssertTrue(old.shape?.identity === colored.shape?.identity)
        XCTAssertEqual(old.baselines, colored.baselines)
        XCTAssertEqual(old.lines.map { CTLineGetStringRange($0).length }, colored.lines.map { CTLineGetStringRange($0).length })
        XCTAssertNil(colored.shape?.lineBreakBoundaries, "paint reuses ranges without discovering breaks")
        p.applyParagraph(1, rows(24))
        let resized = try XCTUnwrap(node.paragraphLayout())
        XCTAssertFalse(colored.shape?.identity === resized.shape?.identity)
        XCTAssertNotNil(resized.shape?.lineBreakBoundaries)
        p.forgetParagraph(node)
        XCTAssertNil(p.inlineText(3)); XCTAssertNil(p.inlineText(4))
    }

    func testUrgentLinesMatchFreshWorkerPixels() throws {
        for measured in [false, true] {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "raster-lines")
        let presenter = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        presenter.root.frame = NSRect(x: 0, y: 0, width: 600, height: 2000)
        defer { window.close(); session.destroy() }
        func pixels(_ surface: IOSurface) -> Data {
            surface.lock(options: .readOnly, seed: nil)
            defer { surface.unlock(options: .readOnly, seed: nil) }
            var data = Data()
            for y in 0..<surface.height {
                data.append(Data(bytes: surface.baseAddress.advanced(by: y * surface.bytesPerRow),
                                 count: surface.width * 4))
            }
            return data
        }
        let texts = ["Words with a soft\u{ad}hyphen and trailing spaces.  ",
                     "日本語 e\u{301} 👨‍👩‍👧‍👦\nSecond line", "العربية שלום Latin", ""]
        for dark in [false, true] { for width in [140.0, 500.0] {
            for align in ["left", "center", "right"] { for text in texts {
                window.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
                let node = NodeView(id: 1, kind: "text", presenter: presenter)
                node.applyStyle(["font_size": 16.0, "line_height": "24px", "text_align": align,
                                 "text_color": [[30.0, 60.0, 90.0, 255.0], [220.0, 180.0, 150.0, 255.0]]])
                presenter.views[1] = node
                let inline: [String: Any] = ["id": 2, "parent": 1, "paint": true,
                    "style": ["font_size": 19.0, "font_style": "italic", "font_weight": 700.0,
                              "text_color": [180.0, 70.0, 40.0, 255.0]],
                    "props": ["text": text, "href": "https://example.invalid/"]]
                let regular: [String: Any] = ["id": 3, "parent": 1, "paint": true, "props": ["text": "Regular → "]]
                presenter.applyParagraph(1, text.isEmpty ? [inline] : [regular, inline])
                node.frame = NSRect(x: 0, y: 800, width: width, height: 240)
                node.prepareToMount()
                presenter.root.addSubview(node)
                XCTAssertFalse(presenter.textIsVisible(node))
                if measured {
                    var black = node.paragraphSpec()
                    black.color = [0, 0, 0, 255]
                    for i in black.runs.indices { black.runs[i].color = nil }
                    _ = session.text.paragraph(black, width: node.contentBox().width)
                    XCTAssertNotNil(session.text.measuredBreaks(node.paragraphSpec(), width: node.contentBox().width))
                } else {
                    XCTAssertNil(session.text.measuredBreaks(node.paragraphSpec(), width: node.contentBox().width))
                }
                XCTAssertTrue(presenter.textRasters.ensure(node, urgent: true))
                let urgent = pixels(try XCTUnwrap(node.textRaster))
                XCTAssertEqual(node.cachedTextLayout == nil, measured,
                               "measured ranges must avoid constructing a paragraph")
                node.dropTextRaster()
                XCTAssertTrue(presenter.textRasters.ensure(node, urgent: true))
                XCTAssertEqual(pixels(try XCTUnwrap(node.textRaster)), urgent, "repeat exact-painted reuse")
                node.dropTextRaster()
                XCTAssertTrue(presenter.textRasters.ensure(node, urgent: false))
                let deadline = Date(timeIntervalSinceNow: 2)
                while !node.textRasterReady && Date() < deadline {
                    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.001))
                }
                XCTAssertEqual(pixels(try XCTUnwrap(node.textRaster)), urgent,
                               "fresh worker and reused urgent lines must paint identically")
                node.forget(); node.removeFromSuperview()
            } }
        } }
        }
    }

    func testRasterShapesReuseOnlyExactPaintAndRespectColdBudget() {
        let engine = TextEngine(resolve: { _ in nil }, coldTextTargetBytes: 64 * 1024)
        let run = Run(text: "Painted 日本語 e\u{301}", size: 16, weight: 400,
                      family: 0, italic: false, lineHeight: 24, letterSpacing: 0)
        var spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [10, 20, 30, 255])
        let ranges = [CFRange(location: 0, length: (run.text as NSString).length)]
        let first = engine.rasterLines(spec, ranges: ranges).0
        XCTAssertTrue(first === engine.rasterLines(spec, ranges: ranges).0)
        spec.color = [200, 40, 20, 255]
        XCTAssertFalse(first === engine.rasterLines(spec, ranges: ranges).0)
        let minimal = TextEngine(resolve: { _ in nil }, coldTextTargetBytes: 0)
        weak var released: NSAttributedString?
        autoreleasepool {
            released = minimal.rasterLines(spec, ranges: ranges).0
            // The existing soft policy keeps one current oversize value.
            XCTAssertEqual(minimal.residencyStats.coldEntries, 1)
            XCTAssertGreaterThan(minimal.residencyStats.coldOverageBytes, 0)
        }
        XCTAssertNotNil(released)
        autoreleasepool {
            var replacement = spec
            replacement.runs[0].text = "A new source evicts the previous cold shape"
            _ = minimal.rasterLines(replacement, ranges: [])
        }
        XCTAssertNil(released, "eviction must release the previous painted source")
        XCTAssertEqual(minimal.residencyStats.coldEntries, 1)
        for i in 0..<1000 {
            spec.runs[0].text = "Unique painted source \(i)"
            _ = engine.rasterLines(spec, ranges: [])
        }
        XCTAssertLessThanOrEqual(engine.residencyStats.coldEstimatedBytes, 64 * 1024)
        XCTAssertLessThanOrEqual(engine.residencyStats.coldEntries, TextResidency.maxColdEntries)
    }

    func testManyInlineLineBoxesMatchTheirIndependentLines() {
        let engine = TextEngine(resolve: { _ in nil })
        let strut = Run(text: "", size: 16, weight: 400, family: 0, italic: false,
                        lineHeight: 24, letterSpacing: 0)
        for direction in [0, 1] {
            var all: [Run] = [], baselines: [CGFloat] = [], bottoms: [CGFloat] = []
            var height: CGFloat = 0
            for i in 0..<32 {
                var left = strut, right = strut, empty = strut
                left.text = i % 2 == 0 ? "Latin e\u{301} " : "العربية "
                right.text = i % 2 == 0 ? "👨‍👩‍👧‍👦 日本語\n" : "שלום Latin\n"
                left.lineHeight = CGFloat(26 + i % 4) + 0.25
                right.lineHeight = CGFloat(36 + i % 5) + 0.5
                // Equal font attributes coalesce despite distinct authored boxes;
                // other lines exercise multiple CoreText runs and bidi ordering.
                right.weight = i % 3 == 0 ? 700 : 400
                empty.lineHeight = 1000
                let runs = [empty, left, right, empty]
                let spec = Spec(runs: runs, align: 0, lineClamp: 0, color: [0, 0, 0, 255],
                                direction: direction, strut: strut)
                let line = engine.paragraph(spec, width: 1000)
                XCTAssertEqual(line.lines.count, 1)
                XCTAssertLessThan(line.height, 100, "empty runs at a line boundary have no glyph interval")
                baselines.append(height + line.baselines[0])
                height += line.height
                bottoms.append(height)
                all.append(contentsOf: runs)
            }
            let spec = Spec(runs: all, align: 0, lineClamp: 0, color: [0, 0, 0, 255],
                            direction: direction, strut: strut)
            let paragraph = engine.paragraph(spec, width: 1000)
            XCTAssertEqual(paragraph.lines.count, 32)
            XCTAssertEqual(paragraph.height, height, accuracy: 0.000001)
            for (actual, expected) in zip(paragraph.baselines, baselines) {
                XCTAssertEqual(actual, expected, accuracy: 0.000001)
            }
            for (actual, expected) in zip(paragraph.lineBottoms, bottoms) {
                XCTAssertEqual(actual, expected, accuracy: 0.000001)
            }
        }
    }

    func testWidthChangesReuseUnchangedLinesWithoutReusingPaintOrEllipses() {
        let engine = TextEngine(resolve: { _ in nil })
        let input = Spec(runs: [Run(text: "First line\nSecond line", size: 16, weight: 400,
                                   family: 0, italic: false, lineHeight: 24, letterSpacing: 0)],
                         align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        let first = engine.paragraph(input, width: 1000)
        engine.accepted(first)
        let resized = withExtendedLifetime(first) { engine.paragraph(input, width: 1001) }
        XCTAssertEqual(first.lines.count, 2)
        XCTAssertEqual(resized.lines.count, first.lines.count)
        for (old, new) in zip(first.lines, resized.lines) { XCTAssertTrue(old === new) }
        XCTAssertEqual(resized.baselines, first.baselines)
        XCTAssertEqual(resized.lineBottoms, first.lineBottoms)
        XCTAssertEqual(resized.width, first.width)
        XCTAssertEqual(resized.height, first.height)
        let narrow = engine.paragraph(input, width: 40)
        let fresh = TextEngine(resolve: { _ in nil }).paragraph(input, width: 40)
        XCTAssertEqual(narrow.baselines, fresh.baselines)
        XCTAssertEqual(narrow.width, fresh.width)
        XCTAssertEqual(narrow.height, fresh.height)
        XCTAssertEqual(narrow.lines.map { CTLineGetStringRange($0).length },
                       fresh.lines.map { CTLineGetStringRange($0).length })
        var painted = input
        painted.color = [255, 0, 0, 255]
        let red = engine.paragraph(painted, width: 1002)
        for (old, new) in zip(resized.lines, red.lines) { XCTAssertFalse(old === new) }
        var clamped = input
        clamped.lineClamp = 1
        let ellipsis = engine.paragraph(clamped, width: 70)
        let changedEllipsis = withExtendedLifetime(ellipsis) { engine.paragraph(clamped, width: 71) }
        XCTAssertEqual(changedEllipsis.lines.count, 1)
        XCTAssertFalse(ellipsis.lines[0] === changedEllipsis.lines[0])
    }

    func testLineReuseDoesNotRetainAnAcceptedParagraph() {
        let engine = TextEngine(resolve: { _ in nil })
        let input = Spec(runs: [Run(text: "A released view", size: 16, weight: 400,
                                   family: 0, italic: false, lineHeight: 24, letterSpacing: 0)],
                         align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        weak var released: Paragraph?
        let shape = autoreleasepool {
            let paragraph = engine.paragraph(input, width: 1000)
            engine.accepted(paragraph)
            released = paragraph
            return paragraph.shape!
        }
        withExtendedLifetime(shape) { XCTAssertNil(released) }
    }

    func testWidthRetirementDoesNotWalkSavedScalarMeasurements() {
        let input = Spec(runs: [Run(text: "A measured paragraph", size: 16, weight: 400,
                                   family: 0, italic: false, lineHeight: 24, letterSpacing: 0)],
                         align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        for history in [10, 100, 1000] {
            var cache = TextResidency()
            let identity = cache.identity(input)
            for width in 0..<history {
                cache.put(identity, kind: .definite(Double(width).bitPattern),
                          metrics: ExactMetrics(width: Float(width), height: 24, baseline: 16))
            }
            let before = cache.stats.maintenanceVisits
            cache.retireWidths(TextParagraphKey(shape: TextShapeKey(identity: identity, paint: TextPaint(input)), width: 2000))
            let visits = cache.stats.maintenanceVisits - before
            XCTAssertLessThanOrEqual(visits, 16, "Retiring layouts must not scan saved scalar widths")
            print("scalar-width-history=\(history) retirement-visits=\(visits)")
            for width in 0..<history {
                let metrics = cache.scalar(identity, kind: .definite(Double(width).bitPattern))
                XCTAssertEqual(metrics?.width, Float(width))
                XCTAssertEqual(metrics?.height, 24)
                XCTAssertEqual(metrics?.baseline, 16)
            }
        }
    }

    func testRasterPreservesDescendersOutsideTightLineBox() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "text-ink-overflow")
        let presenter = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 300),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        defer { window.close(); session.destroy() }
        presenter.apply(Batch(ops: [
            ["op": "create", "id": 1, "kind": "text", "props": ["text": "The Measured Page"],
             "style": ["font_size": 52.0, "font_weight": 700, "line_height": 0.6]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 20.0, "y": 30.0, "w": 460.0, "h": 31.2],
        ], timers: false, motion: false, clock: nil, error: nil))
        let node = try XCTUnwrap(presenter.views[1])
        let paragraph = try XCTUnwrap(node.paragraphLayout())
        let line = try XCTUnwrap(paragraph.lines.first)
        let ink = CTLineGetBoundsWithOptions(line, .useGlyphPathBounds)
        let baseline = try XCTUnwrap(paragraph.baselines.first).rounded()
        let bottom = baseline - ink.minY
        XCTAssertGreaterThan(bottom, node.bounds.maxY, "fixture must paint below its CSS line box")
        XCTAssertGreaterThan(node.textRasterFrame.maxY, bottom)
        XCTAssertLessThan(node.textRasterFrame.minY, baseline - ink.maxY)
        XCTAssertEqual(node.bounds.height, 31.2, accuracy: 0.001, "ink must not change layout")
        let surface = try XCTUnwrap(node.textRaster)
        let layer = try XCTUnwrap(node.textRasterOverflowLayer)
        XCTAssertEqual(layer.frame, node.textRasterFrame)
        XCTAssertTrue(layer.contents as? IOSurface === surface)
        XCTAssertNil(node.layer?.contents)
        XCTAssertFalse(try XCTUnwrap(node.layer).masksToBounds)
        surface.lock(options: .readOnly, seed: nil)
        let bytes = surface.baseAddress.assumingMemoryBound(to: UInt8.self)
        let firstBelow = Int(ceil((node.bounds.maxY - node.textRasterFrame.minY) * node.textRasterScale))
        var descenderPixels = 0
        for y in firstBelow..<surface.height {
            for x in 0..<surface.width where bytes[y * surface.bytesPerRow + x * 4 + 3] != 0 {
                descenderPixels += 1
            }
        }
        surface.unlock(options: .readOnly, seed: nil)
        XCTAssertGreaterThan(descenderPixels, 0, "bitmap must contain the formerly cropped descender")
        let capture = try XCTUnwrap(node.bitmapImageRepForCachingDisplay(in: node.bounds))
        Capture.capturing = true
        node.cacheDisplay(in: node.bounds, to: capture)
        Capture.capturing = false
        XCTAssertNil(layer.superlayer, "direct capture retires the old overflow ink")
        presenter.settlePump()
        let restored = try XCTUnwrap(node.textRasterOverflowLayer)
        XCTAssertNotNil(restored.contents, "capture must restore live text without another app event")
        node.applyStyle(["font_size": 52.0, "line_height": 0.6, "overflow_x": "hidden", "overflow_y": "hidden"])
        XCTAssertTrue(try XCTUnwrap(node.layer).masksToBounds, "authored clipping still applies")
        node.dropTextRaster()
        XCTAssertNil(restored.superlayer)
        XCTAssertNil(node.textRasterOverflowLayer)
    }

    func testSharedLineBreakerReleasesParagraphInputAfterEachCall() {
        let engine = TextEngine(resolve: { _ in nil })
        for index in 0..<2 {
            weak var input: NSString?
            autoreleasepool {
                let source = NSString(string: String(repeating: "borrowed paragraph \(index) text ", count: 1024))
                input = source
                _ = engine.lineBoundaries(source, length: source.length)
            }
            withExtendedLifetime(engine) {
                XCTAssertNil(input, "a reused tokenizer must not retain source outside text residency")
            }
        }
    }

    func testBatchPaintsVisibleTextButDefersOffscreenPreparation() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "text-admission")
        let presenter = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        defer { window.close(); session.destroy() }
        presenter.apply(Batch(ops: [
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "visible paragraph"]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "offscreen paragraph"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "offscreen overdraw"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 500.0, "h": 2000.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 20.0, "w": 400.0, "h": 30.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 800.0, "w": 400.0, "h": 30.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 900.0, "w": 400.0, "h": 30.0],
        ], timers: false, motion: false, clock: nil, error: nil))
        let visible = try XCTUnwrap(presenter.views[2]), offscreen = try XCTUnwrap(presenter.views[3])
        XCTAssertTrue(visible.rastersText)
        XCTAssertTrue(presenter.root.visibleRect.intersects(visible.convert(visible.bounds, to: presenter.root)))
        XCTAssertTrue(offscreen.rastersText)
        XCTAssertFalse(presenter.root.visibleRect.intersects(offscreen.convert(offscreen.bounds, to: presenter.root)))
        XCTAssertTrue(visible.textRasterReady, "visible pixels cannot wait for a later slice")
        XCTAssertNil(offscreen.textRasterKey, "mounting must not also prepare speculative text")
        let overdraw = try XCTUnwrap(presenter.views[4])
        XCTAssertFalse(presenter.root.visibleRect.intersects(overdraw.convert(overdraw.bounds, to: presenter.root)))
        XCTAssertNil(overdraw.textRasterKey)
        // AppKit may request an offscreen layer before the next pump callback.
        // Its raster must still belong to a worker, not synchronous overdraw.
        overdraw.updateLayer()
        XCTAssertNotNil(overdraw.textRasterKey)
        XCTAssertFalse(overdraw.textRasterReady, "offscreen overdraw must not rasterize synchronously")
        presenter.settlePump()
        XCTAssertNotNil(offscreen.textRasterKey, "the existing pump must still admit deferred text")
    }

    func testOffscreenResizeRetiresTheRasterItsLayerWouldStretch() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "text-resize")
        let presenter = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        defer { window.close(); session.destroy() }
        func batch(_ ops: [[String: Any]]) {
            presenter.apply(Batch(ops: ops, timers: false, motion: false, clock: nil, error: nil))
        }
        batch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text",
             "props": ["text": "Why a magazine measures first and draws second"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 500.0, "h": 2000.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 900.0, "w": 400.0, "h": 30.0],
        ])
        let node = try XCTUnwrap(presenter.views[2])
        XCTAssertTrue(node.rastersText)
        XCTAssertFalse(presenter.textIsVisible(node))
        // The pixels a worker painted for this width, published by the pump.
        let pixels = try XCTUnwrap(IOSurface(properties: [.width: 800, .height: 60, .bytesPerElement: 4]))
        let key = TextRasterKey(spec: node.paragraphSpec(), size: node.bounds.size,
                                box: node.contentBox(), scale: window.backingScaleFactor)
        node.textRasterKey = key
        node.showTextRaster(pixels, for: key, deferOffscreen: true)
        presenter.refreshVisibleText()
        XCTAssertFalse(node.needsTextRaster)
        XCTAssertTrue(node.layer?.contents as? IOSurface === pixels)

        // Moving the paragraph is not resizing it: those pixels still fit.
        batch([["op": "frame", "id": 2, "x": 0.0, "y": 880.0, "w": 400.0, "h": 30.0]])
        XCTAssertEqual(node.textRasterKey, key)
        XCTAssertFalse(node.needsTextRaster)

        // The window widens while the paragraph is off screen. Its layer
        // would stretch the old surface across the new width.
        batch([["op": "frame", "id": 2, "x": 0.0, "y": 880.0, "w": 460.0, "h": 30.0]])
        XCTAssertNotEqual(node.textRasterKey, key)
        XCTAssertTrue(node.needsTextRaster, "a resized paragraph still owes the pump pixels")
        XCTAssertTrue(node.layer?.contents as? IOSurface === pixels, "the old pixels stay up until new ones arrive")

        // Scrolled back to it: painted at the width it has now, not stretched.
        batch([["op": "frame", "id": 2, "x": 0.0, "y": 40.0, "w": 460.0, "h": 30.0]])
        XCTAssertTrue(presenter.textIsVisible(node))
        presenter.refreshVisibleText()
        XCTAssertFalse(node.needsTextRaster)
        XCTAssertEqual(node.textRasterKey?.size, node.bounds.size)
        XCTAssertFalse(node.layer?.contents as? IOSurface === pixels)
    }

    func testWorkerPublicationDefersOnlyOffscreenCurrentPixels() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "text-publication")
        let presenter = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        defer { window.close(); session.destroy() }
        presenter.apply(Batch(ops: [
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "visible paragraph"]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "offscreen paragraph"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "second offscreen paragraph"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 500.0, "h": 2000.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 20.0, "w": 400.0, "h": 30.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 800.0, "w": 400.0, "h": 30.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 900.0, "w": 400.0, "h": 30.0],
        ], timers: false, motion: false, clock: nil, error: nil))
        let visible = try XCTUnwrap(presenter.views[2]), offscreen = try XCTUnwrap(presenter.views[3])
        let pixels = try XCTUnwrap(IOSurface(properties: [.width: 800, .height: 60, .bytesPerElement: 4]))
        let late = try XCTUnwrap(IOSurface(properties: [.width: 800, .height: 60, .bytesPerElement: 4]))
        func prepare(_ node: NodeView) -> TextRasterKey {
            node.dropTextRaster()
            let key = TextRasterKey(spec: node.paragraphSpec(), size: node.bounds.size,
                                    box: node.contentBox(), scale: window.backingScaleFactor)
            node.textRasterKey = key
            return key
        }
        var key = prepare(offscreen)
        XCTAssertFalse(presenter.textIsVisible(offscreen))
        offscreen.showTextRaster(pixels, for: key, deferOffscreen: true)
        XCTAssertTrue(offscreen.textRasterReady)
        XCTAssertTrue(offscreen.textRasterPending)
        XCTAssertTrue(offscreen.needsTextRaster)
        XCTAssertNil(offscreen.layer?.contents, "worker completion must not publish offscreen")
        let second = try XCTUnwrap(presenter.views[4])
        second.showTextRaster(late, for: prepare(second), deferOffscreen: true)
        XCTAssertTrue(second.textRasterPending)
        let actionsDisabled = CATransaction.disableActions()
        presenter.refreshVisibleText()
        XCTAssertEqual(CATransaction.disableActions(), actionsDisabled)
        XCTAssertTrue(second.textRasterReady)
        XCTAssertFalse(second.textRasterPending)
        XCTAssertTrue(second.layer?.contents as? IOSurface === late)
        XCTAssertFalse(offscreen.textRasterPending)
        XCTAssertFalse(offscreen.needsTextRaster)
        XCTAssertTrue(offscreen.layer?.contents as? IOSurface === pixels, "the pump publishes accepted pixels without rendering again")

        let visibleKey = prepare(visible)
        visible.showTextRaster(pixels, for: visibleKey, deferOffscreen: true)
        XCTAssertFalse(visible.textRasterPending)
        XCTAssertTrue(visible.layer?.contents as? IOSurface === pixels)
        visible.showTextRaster(late, for: visibleKey, deferOffscreen: true)
        XCTAssertTrue(visible.textRaster === pixels, "a late worker cannot replace urgent pixels")

        key = prepare(offscreen)
        offscreen.showTextRaster(pixels, for: key, deferOffscreen: true)
        offscreen.frame.origin.y = 80
        XCTAssertTrue(presenter.textIsVisible(offscreen))
        presenter.textRasters.ensure(offscreen, urgent: true)
        XCTAssertFalse(offscreen.textRasterPending)
        XCTAssertTrue(offscreen.layer?.contents as? IOSurface === pixels, "visible takeover must reuse pending pixels")

        offscreen.frame.origin.y = 800
        key = prepare(offscreen)
        offscreen.showTextRaster(pixels, for: key, deferOffscreen: true)
        offscreen.invalidateText()
        XCTAssertFalse(offscreen.textRasterPending)
        offscreen.showTextRaster(late, for: key, deferOffscreen: true)
        XCTAssertNil(offscreen.textRasterKey)
        XCTAssertFalse(offscreen.textRasterPending)
        key = prepare(offscreen)
        offscreen.showTextRaster(pixels, for: key, deferOffscreen: true)
        offscreen.forget()
        offscreen.showTextRaster(late, for: key, deferOffscreen: true)
        XCTAssertNil(offscreen.textRaster)
        XCTAssertNil(offscreen.textRasterKey)
        XCTAssertFalse(offscreen.textRasterPending)
    }

    func testFlowChangesRetireOrdinaryRasterAndRejectItsLateWorker() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "flow-raster")
        let presenter = session.presenter
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        defer { window.close(); session.destroy() }
        presenter.apply(Batch(ops: [
            ["op": "create", "id": 1, "kind": "text", "props": ["text": "A paragraph around a shape"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
        ], timers: false, motion: false, clock: nil, error: nil))
        let node = try XCTUnwrap(presenter.views[1])
        XCTAssertTrue(node.rastersText)
        let key = try XCTUnwrap(node.textRasterKey), pixels = try XCTUnwrap(node.textRaster)
        node.textRasterPending = true
        node.applyFlow([["kind": "Circle", "cx": 150.0, "cy": 40.0, "r": 25.0]])
        XCTAssertFalse(node.rastersText)
        XCTAssertNil(node.textRaster)
        XCTAssertNil(node.textRasterKey)
        XCTAssertFalse(node.textRasterPending)
        node.showTextRaster(pixels, for: key)
        XCTAssertNil(node.textRaster, "ordinary worker completion cannot replace flowed ink")
        XCTAssertFalse(try XCTUnwrap(node.paragraphLayout()).fragments.isEmpty)
        node.applyFlow([])
        XCTAssertTrue(node.rastersText)
        XCTAssertNil(node.cachedTextLayout)
        XCTAssertNil(node.textRasterKey)
        XCTAssertFalse(node.textRasterPending)
        XCTAssertTrue(try XCTUnwrap(node.paragraphLayout()).fragments.isEmpty)
    }

    func testLateRasterKeepsUrgentPixelsButChangedKeyStillPublishes() throws {
        let presenter = Presenter()
        let node = NodeView(id: 1, kind: "text", presenter: presenter)
        func surface() throws -> IOSurface {
            try XCTUnwrap(IOSurface(properties: [.width: 20, .height: 20, .bytesPerElement: 4]))
        }
        let first = try surface(), late = try surface(), replacement = try surface()
        let key = TextRasterKey(spec: node.paragraphSpec(), size: CGSize(width: 20, height: 20),
                                box: CGRect(x: 0, y: 0, width: 20, height: 20), scale: 1)
        node.textRasterKey = key
        node.showTextRaster(first, for: key)
        node.showTextRaster(late, for: key)
        XCTAssertTrue(node.textRaster === first, "a late worker must not replace accepted urgent pixels")
        let next = TextRasterKey(spec: key.spec, size: key.size, box: key.box, scale: 2)
        node.textRasterKey = next
        node.textRasterReady = false
        node.showTextRaster(late, for: key)
        XCTAssertFalse(node.textRasterReady, "obsolete work cannot satisfy a changed key")
        node.showTextRaster(nil, for: next)
        XCTAssertFalse(node.textRasterReady, "a failed urgent paint must allow its worker to finish")
        node.showTextRaster(replacement, for: next)
        XCTAssertTrue(node.textRasterReady)
        XCTAssertTrue(node.textRaster === replacement)
    }

    func testFractionalInlineMetricsReuseKernelMeasuredBreaks() throws {
        _ = NSApplication.shared
        let presenter = Presenter()
        let parent = NodeView(id: 1, kind: "text", presenter: presenter)
        presenter.views[1] = parent
        let text = "fractional inline code wraps across several measured lines"
        let cases: [(Any, Float?, Float?)] = [
            (1.625, Float(16) * 1.625, Float(14.72) * 1.625),
            (1.3, Float(16) * 1.3, Float(14.72) * 1.3),
            ("normal", nil, nil), (0.0, 0, 0),
            ("23.1px", Float(23.1), Float(23.1)),
        ]
        for (lineHeight, parentHeight, childHeight) in cases {
            func style(_ size: Double) throws -> [String: Any] {
                let wire = try JSONSerialization.data(withJSONObject: [
                    "font_size": size, "font_family": 1, "font_weight": 400,
                    "line_height": lineHeight, "letter_spacing": 0.1,
                ])
                return try XCTUnwrap(JSONSerialization.jsonObject(with: wire) as? [String: Any])
            }
            parent.style = try style(16)
            presenter.applyParagraph(1, [["id": 2, "parent": 1, "paint": true,
                "props": ["text": text], "style": try style(14.72)]])
            parent.invalidateText()
            let spec = parent.paragraphSpec()
            let engine = TextEngine(resolve: { _ in nil })
            let bytes = Array(text.utf8)
            bytes.withUnsafeBufferPointer { bytes in
                // Independent ABI values: Rust resolves unitless line height
                // in f32, then C carries the resolved length, not the ratio.
                var run = ExactTextRun()
                run.text = bytes.baseAddress; run.len = bytes.count
                run.font_size = 14.72; run.font_weight = 400; run.font_family = 1
                run.letter_spacing = 0.1
                run.has_line_height = childHeight == nil ? 0 : 1
                run.line_height = childHeight ?? 0
                var strut = run
                strut.text = nil; strut.len = 0; strut.font_size = 16
                strut.has_line_height = parentHeight == nil ? 0 : 1
                strut.line_height = parentHeight ?? 0
                withUnsafePointer(to: &run) { run in
                    var request = ExactMeasureRequest()
                    request.runs = run; request.count = 1; request.strut = strut
                    request.width = 160; request.height = -1
                    XCTAssertTrue(TextMetricKey.matches(request, spec.geometry), "\(lineHeight)")
                    let metrics = engine.measure(request)
                    // Ask before paint creates a paragraph: it must reuse the
                    // measurement's breaks, not silently typeset a second time.
                    let breaks = engine.measuredBreaks(spec, width: 160)
                    XCTAssertNotNil(breaks, "\(lineHeight)")
                    let painted = engine.paragraph(spec, width: 160)
                    XCTAssertEqual(Float(painted.height), metrics.height)
                    XCTAssertEqual(breaks?.0.map(\.location), painted.lines.map { CTLineGetStringRange($0).location })
                    XCTAssertEqual(breaks?.0.map(\.length), painted.lines.map { CTLineGetStringRange($0).length })
                    XCTAssertEqual(breaks?.1, painted.baselines)
                }
            }
        }
    }
}
#endif
