#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A list builds its lead rows below the scrollport, and nothing redisplays
/// a row when it scrolls in: its paragraphs must have their pixels by then.
/// UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class TextPaintIOSTests: XCTestCase {
    private var window: UIWindow!

    private func collections(_ rows: [Int]) -> [String: Any] {
        ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
            "rows": rows.map { ["view": $0, "root": $0, "epoch": 1] }, "correction": NSNull()]]]
    }
    /// A row (`base`) holding one clamped paragraph (`base + 1`), at `y`.
    private func rowOps(_ base: Int, y: Double, text: String) -> [[String: Any]] {
        [
            ["op": "create", "id": base, "kind": "view", "props": ["testId": "row-\(base)"]],
            ["op": "create", "id": base + 1, "kind": "text", "props": ["text": text],
             "style": ["font_size": 16.0, "line_clamp": 2.0, "text_overflow": "ellipsis"]],
            ["op": "children", "id": base, "ids": [base + 1]],
            ["op": "frame", "id": base, "x": 0.0, "y": y, "w": 300.0, "h": 60.0],
            ["op": "frame", "id": base + 1, "x": 10.0, "y": 10.0, "w": 280.0, "h": 40.0],
        ]
    }

    private func inkPixels(_ node: NodeView) -> Int {
        let format = UIGraphicsImageRendererFormat(); format.scale = 1; format.opaque = false
        let image = UIGraphicsImageRenderer(bounds: node.bounds, format: format).image { _ in node.draw(node.bounds) }
        guard let cg = image.cgImage, let data = cg.dataProvider?.data, let bytes = CFDataGetBytePtr(data) else { return 0 }
        var count = 0
        for y in 0..<cg.height { for x in 0..<cg.width where bytes[y * cg.bytesPerRow + x * 4 + 3] > 0 { count += 1 } }
        return count
    }

    func testHiddenTextKeepsItsGeometryAndVisibleInlineDescendantsPaint() throws {
        let session = ExactApp.shared.makeSession(label: "hidden-text")
        defer { session.destroy() }
        let p = session.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "props": ["text": "Hidden 👋"], "style": ["font_size": 24]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 60],
        ]))
        let node = try XCTUnwrap(p.views[1])
        let height = try XCTUnwrap(node.paragraphLayout()).height
        XCTAssertGreaterThan(inkPixels(node), 0)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["font_size": 24, "visibility": "hidden"]]]))
        XCTAssertEqual(try XCTUnwrap(node.paragraphLayout()).height, height)
        XCTAssertEqual(inkPixels(node), 0, "glyphs and colored emoji are hidden")
        XCTAssertFalse(node.isHidden, "the containing view cannot hide explicitly visible descendants")
        var hidden = InlineStyle(), shown = InlineStyle()
        try hidden.set("visibility", .string("hidden"))
        try hidden.set("background_color", .array([0, 255, 0, 255]))
        try shown.set("visibility", .string("visible"))
        node.props = [:]
        node.inlineText = [
            InlineText(id: 10, parent: 1, props: [:], style: hidden, handlers: [], paints: false),
            InlineText(id: 2, parent: 10, props: ["text": "Hidden"], style: hidden, handlers: [], paints: true),
            InlineText(id: 3, parent: 10, props: ["text": "Visible"], style: shown, handlers: [], paints: true),
        ]
        node.invalidateText()
        XCTAssertGreaterThan(inkPixels(node), 0, "an explicitly visible inline descendant still paints")
        XCTAssertEqual(node.paragraphSpec().runs.map(\.hidden), [true, false])
        XCTAssertNil(node.paragraphSpec().runs.last?.background, "a hidden inline parent paints no background behind its visible child")
    }

    /// `-exact-content-transition: numeric` rolls the ink layer's new pixels in. An
    /// HDR `text-shadow` is a layer of its own under it, and rolls with it.
    func testAnHDRTextShadowRollsWithItsNumerals() throws {
        let session = ExactApp.shared.makeSession(label: "numeric-hdr-shadow")
        defer { session.destroy() }
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        let hdr: [String: Any] = ["cs": [["s": "srgb-linear", "v": [4.0, 4.0, 4.0, 1.0]]], "c": [255.0, 255.0, 255.0, 255.0]]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "props": ["text": "41"],
             "style": ["font_size": 24.0, "content_transition": "numeric", "text_shadow": ["o": [3.0, 3.0], "b": 0.0, "c": hdr]]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 60.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        p.paintVisibleText()
        let ink = try XCTUnwrap(node.textRasterLayer), cast = try XCTUnwrap(ink.textCast)
        // Core Animation keeps a layer's transition under its own key.
        XCTAssertNil(ink.animation(forKey: kCATransition), "first pixels do not roll")
        XCTAssertNil(cast.animation(forKey: kCATransition))
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["text": "42"]]]))
        _ = p.refreshVisibleText()
        p.paintVisibleText()
        p.textRasters.settleVisible([node])
        XCTAssertTrue(node.textRasterReady, "the new text's pixels are up")
        try XCTSkipIf(UIAccessibility.isReduceMotionEnabled, "Reduce Motion: nothing rolls")
        XCTAssertNotNil(ink.animation(forKey: kCATransition))
        XCTAssertTrue(node.textRasterLayer?.textCast === cast)
        let roll = try XCTUnwrap(cast.animation(forKey: kCATransition) as? CATransition, "the shadow's layer rolls too")
        XCTAssertEqual(roll.subtype, .fromBottom)
    }

    /// A clamped paragraph (`line-clamp`) rasters like any other (its last
    /// line made again from its geometry, `LineGeometry.clamped`): built in
    /// the lead it asks a worker, and it has pixels when it scrolls in.
    func testAClampedParagraphInAReusedRowBuiltBelowTheScrollportPaints() throws {
        let session = ExactApp.shared.makeSession(label: "clamp-reuse")
        defer { session.destroy() }
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([collections([10]), ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]]
            + rowOps(10, y: 0, text: "Maybe family sounds draft later scroll deadline picnic thanks soon a meeting")
            + [["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
               ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        let first = try XCTUnwrap(p.views[11])

        // The row retires; the next of its shape is built in the lead, below the scrollport.
        p.apply(wireBatch([collections([20])] + [10, 11].map { ["op": "destroy", "id": $0] }
            + rowOps(20, y: 1000, text: "A sure family meeting schedule text layout definitely thanks")
            + [["op": "children", "id": 1, "ids": [20]]]))
        let text = try XCTUnwrap(p.views[21])
        XCTAssertTrue(text === first, "the pool lent the retired paragraph's view")
        XCTAssertNotNil(text.window)
        XCTAssertTrue(text.canRasterText, "a clamped paragraph rasters")
        XCTAssertFalse(text.drawsPaint, "so its view keeps no bitmap")
        XCTAssertFalse(p.textIsVisible(text), "built below the scrollport")
        XCTAssertEqual(inkPixels(text), 0, "draw(_:) paints none of it")

        // It scrolls in: before the frame commits, what shows has its pixels.
        p.apply(wireBatch([["op": "frame", "id": 20, "x": 0.0, "y": 0.0, "w": 300.0, "h": 60.0]]))
        XCTAssertTrue(p.textIsVisible(text))
        p.paintVisibleText()
        XCTAssertNotNil(text.textRaster)
        XCTAssertTrue(text.textRasterFrame.contains(p.textScrollportRect(text)), "the raster covers what shows")
    }

    /// The rasterizer's own queue under churn: clamped rows whose text
    /// changes every turn get worker jobs (`refreshVisibleText`) while main
    /// lays them out, paints what shows (`paintVisibleText`) and publishes
    /// what finished. Every row that shows ends with pixels for its text.
    func testClampedRowsRasterOnWorkersWhileMainRelaysThem() throws {
        let session = ExactApp.shared.makeSession(label: "clamp-race")
        defer { session.destroy() }
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        let words = ["family", "draft", "deadline", "picnic", "station", "meeting", "later", "thanks", "日本語", "שלום"]
        func text(_ row: Int, _ turn: Int) -> String {
            (0..<18).map { words[(row * 3 + turn + $0 * 7) % words.count] }.joined(separator: " ")
        }
        let rows = Array(0..<30).map { 100 + $0 * 2 }
        var ops: [[String: Any]] = [collections(rows), ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]]
        for (i, base) in rows.enumerated() { ops += rowOps(base, y: Double(i) * 60, text: text(i, 0)) }
        ops += [["op": "children", "id": 1, "ids": rows], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 800.0],
                ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]
        p.apply(wireBatch(ops))
        for turn in 1...24 {
            let changed = rows.enumerated().filter { ($0.offset + turn) % 3 == 0 }
            p.apply(wireBatch(changed.map { ["op": "props", "id": $0.element + 1, "set": ["text": text($0.offset, turn)]] }))
            _ = p.refreshVisibleText(velocity: 2000)
            for base in rows { _ = p.views[UInt32(base + 1)]?.paragraphLayout() }
            p.paintVisibleText()
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.001))
        }
        let shown = rows.compactMap { p.views[UInt32($0 + 1)] }.filter { p.textIsVisible($0) }
        XCTAssertGreaterThan(shown.count, 10)
        p.textRasters.settleVisible(shown)
        for node in shown {
            XCTAssertTrue(node.canRasterText)
            XCTAssertTrue(node.textRasterReady, "#\(node.id) has pixels for its current text")
            XCTAssertTrue(node.textRasterKey?.spec == node.paragraphSpec(), "keyed by its current text")
        }
    }

    /// A one-line label stretched across a row: `text-overflow: ellipsis`
    /// rasters (truncated in the job, as `draw(_:)` truncates), and the
    /// raster is its ink, not the box. No backing store of the row's width.
    func testAnEllipsizedLabelRastersItsInkNotItsBox() throws {
        let session = ExactApp.shared.makeSession(label: "ellipsis-raster")
        defer { session.destroy() }
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        let label: [String: Any] = ["font_size": 16.0, "white_space": "nowrap", "overflow_x": "hidden", "text_overflow": "ellipsis"]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Voltaris"], "style": label],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": String(repeating: "Wide label ", count: 12)], "style": label],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 360.0, "h": 20.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 40.0, "w": 200.0, "h": 20.0],
        ]))
        let short = try XCTUnwrap(p.views[2]), long = try XCTUnwrap(p.views[3])
        for node in [short, long] {
            XCTAssertTrue(node.canRasterText, "an ellipsized label rasters")
            XCTAssertFalse(node.drawsPaint, "so its view keeps no bitmap")
            p.textRasters.ensure(node, urgent: true)
            XCTAssertNotNil(node.textRaster)
            XCTAssertTrue(node.textRasterFrame.contains(node.bounds), "the raster answers for the whole box")
        }
        let shortInk = try XCTUnwrap(short.textRasterLayer).frame
        XCTAssertLessThan(shortInk.width, 120, "a short name's pixels are its text's, not the box's 360 pt")
        XCTAssertGreaterThan(shortInk.width, 30)
        let longInk = try XCTUnwrap(long.textRasterLayer).frame
        XCTAssertLessThanOrEqual(longInk.maxX, 201, "the overflowing line is cut at the box, with its ellipsis")
        XCTAssertGreaterThan(longInk.maxX, 150)
    }
}
#endif
