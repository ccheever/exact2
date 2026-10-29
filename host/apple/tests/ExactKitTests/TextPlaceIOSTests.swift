#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A paragraph's pixels and its place, apart (`NodeView.placeTextRaster`):
/// the bitmap holds its words as laid out in their box, and the text's layer
/// is framed at the box in its view, so a box that moves is a position UIKit's
/// open animation carries, as it carries a label's frame, and its pixels are
/// not painted again. Bands of a tall paragraph and an HDR shadow's cast are
/// in that layer. It needs UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class TextPlaceIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        window?.isHidden = true
        window = nil
        session = nil
        super.tearDown()
    }

    /// A paragraph `style` in a 400-point window, its content box `box`, painted and shown.
    private func paragraph(_ label: String, text: String, style: [String: Any] = [:], height: Double = 40, box: [Double]) throws -> (Presenter, NodeView) {
        let session = ExactApp.shared.makeSession(label: label)
        self.session = session
        let p = session.presenter
        // A scene's window, whose layers Core Animation commits
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow()
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 400)
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        var paint: [String: Any] = ["font_size": 20.0]
        paint.merge(style) { $1 }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": text], "style": paint],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": max(400, height)],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": height],
            ["op": "fieldContent", "id": 2, "rect": box],
        ]))
        let node = try XCTUnwrap(p.views[2])
        p.paintVisibleText()
        p.textRasters.settleVisible([node])
        // A frame shows it: its ink has a presentation copy
        let end = Date().addingTimeInterval(2)
        while node.textRasterLayer?.presentation() == nil, Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
        XCTAssertTrue(node.textRasterReady, "painted")
        return (p, node)
    }

    /// Where the words are in their view, whatever layers hold them.
    private func words(_ node: NodeView) throws -> CGRect {
        let ink = try XCTUnwrap(node.textRasterLayer)
        return node.layer.convert(ink.frame, from: ink.superlayer)
    }

    /// The animation that moves the words, on any layer between them and their view.
    private func slide(_ node: NodeView) -> CAAnimation? {
        guard let ink = node.textRasterLayer else { return nil }
        return sequence(first: ink, next: \.superlayer).prefix { $0 !== node.layer }.lazy.compactMap { layer in
            (layer.animationKeys() ?? []).filter { $0.hasPrefix("position") }.compactMap { layer.animation(forKey: $0) }.first
        }.first
    }

    /// The Landmarks port's heading, its padding following the list's
    /// clearance as the sidebar comes in: the words keep their pixels and
    /// move with the animation from where they were, as a label's frame
    /// would. Painted again, they showed at their end place at once.
    func testAParagraphWhoseBoxMovesKeepsItsPixelsAndMovesInTheAnimation() throws {
        let (p, node) = try paragraph("text-box-moves", text: "Collections", box: [26, 0, 300, 40])
        let pixels = try XCTUnwrap(node.textRaster), before = try words(node)
        UIView.animate(withDuration: 0.5) {
            p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [46.0, 0.0, 300.0, 40.0]]]))
        }
        p.paintVisibleText()
        p.textRasters.settleVisible([node]) // a worker's pixels, if it painted them again
        XCTAssertTrue(node.textRaster === pixels, "not painted again")
        XCTAssertEqual(try words(node).minX - before.minX, 20, accuracy: 0.01, "the words are where the box is")
        let moving = try XCTUnwrap(slide(node), "they move inside the animation")
        XCTAssertEqual(moving.duration, 0.5, accuracy: 0.01, "UIKit's")
    }

    /// A box that moves by a fraction of a pixel stays on the device's pixel
    /// grid, as a label's frame does: its glyphs are as crisp as before.
    func testATextsBoxIsOnThePixelGrid() throws {
        let (p, node) = try paragraph("text-box-grid", text: "Collections", box: [10.2, 3.1, 300, 30])
        let scale = try XCTUnwrap(node.textRasterKey).scale
        let ink = try XCTUnwrap(node.textRasterLayer), box = try XCTUnwrap(ink.superlayer)
        for edge in [box.frame.minX, box.frame.minY, ink.frame.minX, ink.frame.minY] {
            XCTAssertEqual(edge * scale, (edge * scale).rounded(), accuracy: 1e-6, "on the grid")
        }
        XCTAssertEqual(box.frame.minX, 10.2, accuracy: 0.5 / scale, "the nearest pixel to the box")
        p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [20.55, 3.1, 300.0, 30.0]]]))
        XCTAssertEqual(box.frame.minX * scale, (box.frame.minX * scale).rounded(), accuracy: 1e-6)
        XCTAssertEqual(box.frame.minX, 20.55, accuracy: 0.5 / scale)
        // From the view's absolute origin, as its own edges are: a view laid out at 0.2 with its box 0.2 in is drawn at 0.4's pixel.
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.2, "y": 0.0, "w": 400.0, "h": 30.0],
                           ["op": "fieldContent", "id": 2, "rect": [0.2, 3.1, 300.0, 30.0]]]))
        XCTAssertEqual(node.frame.minX + box.frame.minX, PixelGrid.round(0.4, scale: scale), accuracy: 1e-6)
    }

    /// A tall paragraph's band is of its box's rows: moved, the box keeps the
    /// band and carries it, as it does whole pixels.
    func testATallParagraphsBandMovesWithItsBox() throws {
        let text = String(repeating: "The measured page draws its words once and moves them as a label moves. ", count: 260)
        let (p, node) = try paragraph("text-box-band", text: text, height: 6000, box: [16, 0, 360, 6000])
        let key = try XCTUnwrap(node.textRasterKey)
        XCTAssertNotNil(key.clip, "a band")
        let pixels = try XCTUnwrap(node.textRaster), before = try words(node)
        UIView.animate(withDuration: 0.5) {
            p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [36.0, 0.0, 360.0, 6000.0]]]))
        }
        p.paintVisibleText()
        _ = p.refreshVisibleText()
        p.textRasters.settleVisible([node])
        XCTAssertTrue(node.textRaster === pixels, "the band is kept")
        XCTAssertEqual(node.textRasterKey?.clip?.minY, key.clip?.minY, "its rows")
        XCTAssertEqual(node.textRasterKey?.clip?.maxY, key.clip?.maxY, "its rows")
        XCTAssertEqual(try words(node).minX - before.minX, 20, accuracy: 0.01)
        XCTAssertNotNil(slide(node), "it moves inside the animation")
    }

    /// A tall paragraph whose band holds only its padding (its box far below
    /// the port) has pixels for that band, no ink in them: not a failure that
    /// hands the paragraph to UIKit's backing store.
    func testABandOfOnlyPaddingIsPaintedNotFailed() throws {
        let text = String(repeating: "Words far below the port. ", count: 200)
        let (_, node) = try paragraph("text-box-padding-band", text: text, height: 6000, box: [16, 5000, 360, 1000])
        XCTAssertNotNil(node.textRasterKey?.clip, "a band")
        XCTAssertFalse(node.textRasterFailed, "pixels for the band, not UIKit's backing store")
    }

    /// A kept image whose ink a narrow view cut does not keep a wider view's,
    /// which it cannot serve, out of the cache: the wider one replaces it.
    func testAWiderViewsLabelReplacesANarrowOnesInTheCache() throws {
        let session = ExactApp.shared.makeSession(label: "text-box-kept")
        self.session = session
        let p = session.presenter
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        // About 440 points of ink: past a 20-point view's limit (its bounds and 256 points), within a 400-point one's
        let label = String(repeating: "M", count: 60), style: [String: Any] = ["font_size": 10.0, "white_space": "nowrap"]
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view"]]
        for (id, width) in [(2, 20.0), (3, 400.0), (4, 400.0)] {
            ops += [["op": "create", "id": id, "kind": "text", "props": ["text": label], "style": style],
                    ["op": "frame", "id": id, "x": 0.0, "y": Double(id) * 20, "w": width, "h": 12.0],
                    ["op": "fieldContent", "id": id, "rect": [0, 0, 20, 12]]]
        }
        ops += [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0]]
        p.apply(wireBatch(ops))
        let narrow = try XCTUnwrap(p.views[2]), wide = try XCTUnwrap(p.views[3]), again = try XCTUnwrap(p.views[4])
        for node in [narrow, wide] {
            node.dropTextRaster()
            p.textRasters.ensure(node, urgent: true)
            XCTAssertTrue(node.textRasterReady)
        }
        XCTAssertLessThan(try words(narrow).maxX, try words(wide).maxX - 100, "the narrow view cut its ink")
        again.dropTextRaster()
        RegionTextExecutor.queue.isSuspended = true
        defer { RegionTextExecutor.queue.isSuspended = false }
        p.textRasters.ensure(again, urgent: false)
        XCTAssertTrue(again.textRasterReady, "the wider view's kept pixels, at once")
        XCTAssertEqual(p.textRasters.inFlight, 0, "nothing painted again")
    }

    /// A band holds the view's columns (`TextRasterJob.bandColumns`): a line
    /// a negative `text-indent` takes into the padding is whole.
    func testATallParagraphsBandHoldsALineInItsPadding() throws {
        let text = String(repeating: "The measured page draws its words once and moves them as a label moves. ", count: 260)
        let (_, node) = try paragraph("text-box-indent", text: text, style: ["text_indent": -100.0], height: 6000, box: [100, 0, 280, 6000])
        XCTAssertNotNil(node.textRasterKey?.clip, "a band")
        XCTAssertLessThan(try words(node).minX, 4, "its first line starts in the padding, at the view's edge")
    }

    /// A line a negative `text-indent` starts far into the padding keeps its
    /// ink, which the view's bounds hold (`TextRasterKey.room`), and a tall
    /// paragraph's band holds the view's rows: a line height tighter than
    /// the font puts its first glyphs well above its box.
    func testInkFarIntoThePaddingIsKept() throws {
        let (_, node) = try paragraph("text-box-far", text: "Collections of landmarks", style: ["text_indent": -320.0], height: 40, box: [320, 0, 80, 40])
        XCTAssertLessThan(try words(node).minX, 4, "its first line starts at the view's edge")
        let text = String(repeating: "Tight lines of large type. ", count: 400)
        let (_, tall) = try paragraph("text-box-tight", text: text, style: ["font_size": 200.0, "line_height": "20px"], height: 6000, box: [16, 100, 360, 5900])
        XCTAssertNotNil(tall.textRasterKey?.clip, "a band")
        XCTAssertLessThan(try words(tall).minY, 100 - 40, "its first glyphs rise well above its box")
    }

    /// A line that runs past its box into the padding keeps its ink there, as
    /// the box's own padding would show it; a tall paragraph's band keeps a
    /// line a negative `text-indent` starts far into the padding.
    func testALineRunningIntoThePaddingKeepsItsInk() throws {
        let (_, node) = try paragraph("text-box-nowrap", text: String(repeating: "M", count: 22), style: ["white_space": "nowrap"], box: [0, 0, 80, 40])
        XCTAssertGreaterThan(try words(node).maxX, 340, "its line's ink, past the box")
        let text = String(repeating: "The measured page draws its words once and moves them as a label moves. ", count: 260)
        let (_, tall) = try paragraph("text-box-band-far", text: text, style: ["text_indent": -320.0], height: 6000, box: [320, 0, 80, 6000])
        XCTAssertNotNil(tall.textRasterKey?.clip, "a band")
        XCTAssertLessThan(try words(tall).minX, 4, "its first line starts at the view's edge")
    }

    /// Lines that overflow a short box into its padding keep their ink, as
    /// the padding shows them; an overflowing right-to-left line, which
    /// starts far before its box, keeps a bitmap no wider than the view and
    /// `maxInkOverflow` to either side (`TextRasterKey.room`).
    func testOverflowingLinesKeepTheirInkInABoundedBitmap() throws {
        let lines = (1...20).map { "Line \($0)" }.joined(separator: "\n")
        let (_, short) = try paragraph("text-box-overflow", text: lines, style: ["white_space": "pre", "line_height": "20px"], height: 400, box: [0, 0, 400, 20])
        XCTAssertGreaterThan(try words(short).maxY, 380, "its last line, near the view's bottom")
        let (_, rtl) = try paragraph("text-box-rtl", text: String(repeating: "M", count: 10_000), style: ["white_space": "nowrap", "direction": "rtl"], box: [0, 0, 300, 40])
        let pixels = try XCTUnwrap(rtl.textRaster), scale = try XCTUnwrap(rtl.textRasterKey).scale
        XCTAssertLessThanOrEqual(CGFloat(pixels.width), (400 + 2 * TextRasterJob.maxInkOverflow) * scale + 2, "bounded across")
    }

    /// Ink the view's bounds cut (a line running past them) is painted again,
    /// in the frame that shows it, when the view grows to show more of it:
    /// the old pixels do not answer for the larger view, and kept pixels cut
    /// so serve no larger view. A box that only moves in its view keeps its
    /// pixels (`testAParagraphWhoseBoxMovesKeepsItsPixelsAndMovesInTheAnimation`).
    /// The type is small enough for the cut line's pixels to be kept.
    func testInkTheViewCutIsPaintedAgainWhenTheViewGrows() throws {
        let (p, node) = try paragraph("text-box-grows", text: String(repeating: "M", count: 175), style: ["white_space": "nowrap", "font_size": 7.0], box: [0, 0, 80, 40])
        XCTAssertLessThanOrEqual(try words(node).maxX, 400 + TextRasterJob.maxInkOverflow + 1, "cut past the view")
        let window = try XCTUnwrap(self.window)
        window.frame.size.width = 1000
        p.viewport.frame = window.bounds
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 1000.0, "h": 400.0], ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 800.0, "h": 40.0]]))
        p.paintVisibleText()
        XCTAssertGreaterThan(try words(node).maxX, 400 + TextRasterJob.maxInkOverflow + 100, "more of the line, before any worker")
    }

    /// Flex text's lines move down their box (`textOffset`) with no frame or
    /// content-box op: the words move with the animation.
    func testLinesMovedDownTheirBoxMoveInTheAnimation() throws {
        let (p, node) = try paragraph("text-box-offset", text: "Collections", height: 60, box: [26, 0, 300, 60])
        let before = try words(node)
        UIView.animate(withDuration: 0.5) {
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["textOffset": "10"]]]))
        }
        XCTAssertEqual(try words(node).minY - before.minY, 10, accuracy: 0.01)
        XCTAssertNotNil(slide(node))
    }

    /// An HDR shadow's cast, a layer of its own under the ink, is in the
    /// text's box with it: one move carries both.
    func testAnHDRShadowsCastMovesWithItsInk() throws {
        let hdr: [String: Any] = ["cs": [["s": "srgb-linear", "v": [4.0, 4.0, 4.0, 1.0]]], "c": [255.0, 255.0, 255.0, 255.0]]
        let (p, node) = try paragraph("text-box-cast", text: "HDR", style: ["text_shadow": ["o": [3.0, 3.0], "b": 0.0, "c": hdr]], box: [26, 0, 300, 40])
        let ink = try XCTUnwrap(node.textRasterLayer), cast = try XCTUnwrap(ink.textCast)
        XCTAssertTrue(cast.superlayer === ink.superlayer, "beside the ink, in the text's box")
        let pixels = try XCTUnwrap(node.textRaster), before = node.layer.convert(cast.frame, from: cast.superlayer)
        UIView.animate(withDuration: 0.5) {
            p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [46.0, 0.0, 300.0, 40.0]]]))
        }
        p.paintVisibleText()
        p.textRasters.settleVisible([node]) // a worker's pixels, if it painted them again
        XCTAssertTrue(node.textRaster === pixels)
        XCTAssertTrue(ink.textCast === cast)
        XCTAssertEqual(node.layer.convert(cast.frame, from: cast.superlayer).minX - before.minX, 20, accuracy: 0.01)
        XCTAssertNotNil(slide(node))
    }
}
#endif
