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
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 500.0, "h": 2000.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 20.0, "w": 400.0, "h": 30.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 800.0, "w": 400.0, "h": 30.0],
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
        presenter.refreshVisibleText()
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
        let child = NodeView(id: 2, kind: "text", presenter: presenter)
        parent.setTextChildren([child])
        let text = "fractional inline code wraps across several measured lines"
        child.props["text"] = text
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
            parent.style = try style(16); child.style = try style(14.72)
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
