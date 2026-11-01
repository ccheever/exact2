#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A paragraph's pixels and its place, apart (`NodeView.placeTextRaster`):
/// the bitmap holds its words as laid out in their box, and the text's layer
/// is framed at the box in its view, so a box that moves is a position UIKit's
/// open animation carries, as it carries a label's frame, and its pixels are
/// not painted again. Bands of a tall paragraph and an HDR shadow's cast are
/// in that layer. Words laid out again while UIKit resizes their box
/// cross-fade to their new lines over the resize. It needs UIKit, so a
/// simulator runs it:
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

    /// A paragraph `style` `width` points wide in a 400-point window, its content box `box`, painted and shown.
    private func paragraph(_ label: String, text: String, style: [String: Any] = [:], width: Double = 400, height: Double = 40, box: [Double]) throws -> (Presenter, NodeView) {
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
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": width, "h": height],
            ["op": "fieldContent", "id": 2, "rect": box],
        ]))
        let node = try XCTUnwrap(p.views[2])
        p.paintVisibleText()
        p.textRasters.settleVisible([node])
        // A frame shows it: its ink has a presentation copy, and the view is
        // shown (`Presenter.unshown`), so UIKit's animations move it
        let end = Date().addingTimeInterval(2)
        while node.textRasterLayer?.presentation() == nil || !node.showed, Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
        XCTAssertTrue(node.textRasterReady, "painted")
        XCTAssertTrue(node.showed, "shown")
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
        let ink = try XCTUnwrap(node.textRasterLayer), box = try XCTUnwrap(node.textBoxLayer)
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
    private static let fuji = "When seen at a distance, Mount Fuji presents a beautiful, nearly symmetric, often snow-capped profile and is Japan's tallest and most iconic mountain."

    /// The words' lines and their box, which UIKit resizes.
    private func relaid(_ node: NodeView) throws -> (lines: CALayer, box: CALayer) {
        let box = try XCTUnwrap(node.textBoxLayer), lines = try XCTUnwrap(box.lines)
        XCTAssertTrue(node.textRasterLayer?.superlayer === lines, "the ink is in the box's lines")
        XCTAssertNotNil(box.animation(forKey: "bounds.size"), "UIKit resizes the box")
        return (lines, box)
    }

    /// The lines cross-fading: a fade on them, which Core Animation runs on their clock.
    @discardableResult
    private func fading(_ lines: CALayer) throws -> CATransition {
        let fade = try XCTUnwrap(lines.animation(forKey: kCATransition) as? CATransition, "the new lines cross-fade in")
        XCTAssertEqual(fade.type, .fade)
        return fade
    }

    /// Where the fade is at `time` in the box's time: the fade runs from 1 to
    /// 2 in the lines' time, which their clock maps from the box's.
    private func faded(_ lines: CALayer, at time: CFTimeInterval) throws -> CFTimeInterval {
        let fade = try fading(lines)
        let local = (time - lines.beginTime) * CFTimeInterval(lines.speed) + lines.timeOffset
        return min(1, max(0, (local - fade.beginTime) / fade.duration))
    }

    /// The box's time now.
    private func now(_ box: CALayer) -> CFTimeInterval { box.convertTime(CACurrentMediaTime(), from: nil) }

    /// The viewport's clock, which every layer in it keeps, stopped at `time`.
    private func stop(_ p: Presenter, at time: CFTimeInterval) {
        p.viewport.layer.speed = 0
        p.viewport.layer.timeOffset = time
        CATransaction.flush()
    }

    /// The viewport's clock stopped where the paragraph's box is now.
    private func stop(_ p: Presenter, _ node: NodeView) throws { stop(p, at: now(try XCTUnwrap(node.textBoxLayer))) }

    /// A turn of the run loop after a commit, which the app's own turn makes
    /// (one run inside a test was seen to leave it to the test's): the box
    /// aims its fade.
    private func turn() {
        CATransaction.flush()
        RunLoop.main.run(mode: .default, before: .distantPast)
    }

    /// The words the paragraph shows.
    private func shownWords(_ node: NodeView) -> String? { node.textRasterSpec?.runs.map(\.text).joined() }

    /// The Landmarks port's Info closing: the detail widens on UIKit's spring
    /// and its paragraphs break their lines again. The new lines cross-fade
    /// in over the resize, timed as Core Animation commits it: none of them
    /// as the box sets out, half at its middle, all at its end.
    func testAParagraphLaidOutAgainInsideAnAnimationCrossFadesItsLines() throws {
        let (p, node) = try paragraph("text-relaid", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        let pixels = try XCTUnwrap(node.textRaster)
        try stop(p, node)
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        XCTAssertFalse(node.textRaster === pixels, "painted again")
        let (lines, box) = try relaid(node)
        turn()
        let resize = try XCTUnwrap(box.animation(forKey: "bounds.size"))
        let end = resize.beginTime + resize.duration
        XCTAssertEqual(try faded(lines, at: resize.beginTime), 0, accuracy: 1e-5, "the old lines as the box sets out")
        XCTAssertEqual(try faded(lines, at: (resize.beginTime + end) / 2), 0.5, accuracy: 1e-5, "half way at its middle")
        XCTAssertEqual(try faded(lines, at: end), 1, accuracy: 1e-5, "the new lines as it ends")
        let shown = try XCTUnwrap(lines.presentation(), "committed")
        XCTAssertEqual(shown.speed, lines.speed, "the lines' clock sent to the render server")
        XCTAssertEqual(shown.beginTime, lines.beginTime)
    }

    /// Info opening: the detail narrows, and its old pixels still cover it,
    /// so a worker paints the new lines after the resize began. They fade in
    /// from then to the resize's end.
    func testNewLinesAWorkerPaintsFadeInOverTheRestOfTheResize() throws {
        let (p, node) = try paragraph("text-relaid-worker", text: Self.fuji, height: 200, box: [16, 0, 368, 200])
        let pixels = try XCTUnwrap(node.textRaster)
        try stop(p, node)
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 268.0, 200.0]]]))
        }
        XCTAssertTrue(node.textRaster === pixels, "a worker paints them")
        CATransaction.flush() // the resize commits
        let box = try XCTUnwrap(node.textBoxLayer)
        let resize = try XCTUnwrap(box.animation(forKey: "bounds.size"))
        let then = resize.beginTime + resize.duration * 0.2
        stop(p, at: then)
        p.textRasters.settleVisible([node])
        XCTAssertFalse(node.textRaster === pixels, "painted again")
        let (lines, _) = try relaid(node)
        turn()
        XCTAssertEqual(try faded(lines, at: then), 0, accuracy: 1e-5, "the old lines when the new ones came")
        XCTAssertEqual(try faded(lines, at: resize.beginTime + resize.duration), 1, accuracy: 1e-5, "the new ones as the resize ends")
    }

    /// A gesture that holds the resize (UIKit pauses its animator and sets
    /// how far it is), scrubs it forward and back, then lets it run on and
    /// back: the fade is held where the finger has the resize, and runs on
    /// with it, toward the new lines or back to the old ones, aimed when
    /// UIKit re-times its animation on the box. It ends when UIKit's animator
    /// finishes and removes the resize.
    func testTheFadeFollowsAGestureThatHoldsAndScrubsTheResize() throws {
        let (p, node) = try paragraph("text-relaid-gesture", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        let animator = UIViewPropertyAnimator(duration: 1, curve: .easeInOut) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        animator.startAnimation()
        defer { if animator.state == .active { animator.stopAnimation(true) } }
        p.textRasters.settleVisible([node])
        turn()
        let (lines, box) = try relaid(node)
        let fade = try fading(lines)
        let taken = try faded(lines, at: now(box))
        animator.pauseAnimation()
        animator.fractionComplete = 0.3
        turn()
        XCTAssertEqual(lines.speed, 0, "held while the finger holds it")
        let held = try faded(lines, at: now(box))
        XCTAssertEqual(held, taken, accuracy: 0.02, "where it was when the finger took hold, though the resize went to 0.3")
        for (fraction, more) in [(0.7, true), (0.4, false)] {
            let before = try faded(lines, at: now(box))
            animator.fractionComplete = fraction
            turn()
            let after = try faded(lines, at: now(box))
            XCTAssertEqual(after > before, more, "with the finger to \(fraction)")
            XCTAssertTrue(lines.animation(forKey: kCATransition) === fade, "the same fade")
        }
        XCTAssertGreaterThan(try faded(lines, at: now(box)), held, "0.4 is past where it took hold")
        animator.continueAnimation(withTimingParameters: nil, durationFactor: 1)
        turn()
        let on = try XCTUnwrap(box.animation(forKey: "bounds.size"))
        XCTAssertGreaterThan(lines.speed, 0, "on with the resize")
        XCTAssertEqual(try faded(lines, at: on.beginTime + (on.duration - on.timeOffset) / CFTimeInterval(on.speed)), 1, accuracy: 1e-4, "done as it ends")
        animator.pauseAnimation()
        animator.fractionComplete = 0.6
        animator.isReversed = true
        animator.continueAnimation(withTimingParameters: nil, durationFactor: 1)
        turn()
        let back = try XCTUnwrap(box.animation(forKey: "bounds.size"))
        XCTAssertLessThan(back.speed, 0, "UIKit runs it back")
        XCTAssertLessThan(lines.speed, 0, "back toward the old lines")
        let home = back.beginTime + (back.duration + back.timeOffset) / CFTimeInterval(-back.speed)
        XCTAssertEqual(try faded(lines, at: home), 0, accuracy: 1e-4, "the old lines as it gets back")
        // Core Animation has the box back at its start then, and not before
        stop(p, at: home - 0.2)
        XCTAssertGreaterThan(try XCTUnwrap(box.presentation()).bounds.width, 270, "still on its way back")
        stop(p, at: home)
        XCTAssertEqual(try XCTUnwrap(box.presentation()).bounds.width, 268, accuracy: 0.5, "back at its start")
        animator.stopAnimation(false)
        animator.finishAnimation(at: .start)
        XCTAssertNil(box.animation(forKey: "bounds.size"), "UIKit removed its resize")
        turn()
        XCTAssertNil(lines.animation(forKey: kCATransition), "the fade ended")
        XCTAssertEqual(lines.speed, 1, "the lines' clock running")
        XCTAssertEqual(lines.timeOffset, 0)
        XCTAssertEqual(lines.beginTime, 0)
    }

    /// A resize a gesture holds, scrubbed, then removed and another held in
    /// its place (UIKit's animator stopped, another made and paused): the
    /// fade takes hold again where it is, rather than where the first one had it.
    func testAnotherHeldResizeTakesHoldWhereTheFadeIs() throws {
        let (p, node) = try paragraph("text-relaid-held", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, box) = try relaid(node)
        let spring = try XCTUnwrap(box.animation(forKey: "bounds.size") as? CABasicAnimation)
        /// The spring's resize held paused at `fraction`, as UIKit's animator holds its own.
        func held(_ fraction: Double) {
            let copy = spring.copy() as! CABasicAnimation
            copy.speed = 0
            copy.timeOffset = copy.duration * fraction
            box.add(copy, forKey: "bounds.size")
        }
        held(0.4)
        turn()
        let taken = try faded(lines, at: now(box))
        held(0.7)
        turn()
        let scrubbed = try faded(lines, at: now(box))
        XCTAssertGreaterThan(scrubbed, taken + 0.2, "on with the finger")
        box.removeAnimation(forKey: "bounds.size")
        held(0.1)
        turn()
        XCTAssertEqual(try faded(lines, at: now(box)), scrubbed, accuracy: 1e-5, "where it was")
        held(0.6)
        turn()
        let again = try faded(lines, at: now(box))
        XCTAssertGreaterThan(again, scrubbed, "on from there")
        // Another resize of the box, running, added and removed: the held one keeps its hold
        let other = spring.copy() as! CABasicAnimation
        box.add(other, forKey: "bounds.size-other")
        turn()
        box.removeAnimation(forKey: "bounds.size-other")
        turn()
        held(0.1)
        turn()
        XCTAssertEqual(try faded(lines, at: now(box)), scrubbed, accuracy: 1e-5, "back where it took hold")
    }

    /// A gesture taking hold of a running resize (UIKit's animator re-adds it
    /// paused): the fade is held where it is in the transaction that pauses
    /// the resize, so a commit that comes before the fade is aimed leaves it
    /// neither running on nor finishing.
    func testAFadeIsHeldWithTheChangeBeforeItIsAimed() throws {
        let (p, node) = try paragraph("text-relaid-held-change", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, box) = try relaid(node)
        turn()
        XCTAssertGreaterThan(lines.speed, 0, "running with the resize")
        let paused = try XCTUnwrap(box.animation(forKey: "bounds.size")?.copy() as? CABasicAnimation)
        paused.speed = 0
        paused.timeOffset = paused.duration * 0.4
        box.add(paused, forKey: "bounds.size")
        CATransaction.flush() // committed, not yet aimed
        let held = try faded(lines, at: now(box))
        XCTAssertEqual(try XCTUnwrap(lines.presentation()).speed, 0, "held with the change")
        XCTAssertEqual(try faded(lines, at: now(box) + 10), held, "not running on")
        turn()
        XCTAssertEqual(try faded(lines, at: now(box)), held, accuracy: 1e-5, "taken hold of where it was")
    }

    /// A gesture that takes hold of a resize at its end or its start leaves
    /// the fade where it is, and moves it from there with the finger.
    func testAHoldTakenAtAnEndLeavesTheFadeWhereItIs() throws {
        let (p, node) = try paragraph("text-relaid-hold-end", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        try stop(p, node)
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        turn()
        let (lines, box) = try relaid(node)
        let spring = try XCTUnwrap(box.animation(forKey: "bounds.size") as? CABasicAnimation)
        stop(p, at: spring.beginTime + spring.duration * 0.4)
        let under = try faded(lines, at: now(box))
        XCTAssertEqual(under, 0.4, accuracy: 1e-5)
        /// The spring's resize held paused at `fraction`, as UIKit's animator holds its own.
        func held(_ fraction: Double) {
            let copy = spring.copy() as! CABasicAnimation
            copy.speed = 0
            copy.timeOffset = copy.duration * fraction
            box.add(copy, forKey: "bounds.size")
        }
        for (end, toward) in [(1.0, 0.5 * under), (0.0, under + (1 - under) * 0.5)] {
            box.removeAnimation(forKey: "bounds.size")
            held(end)
            turn()
            XCTAssertEqual(try faded(lines, at: now(box)), under, accuracy: 1e-5, "where it was, held at \(end)")
            held(0.5)
            turn()
            XCTAssertEqual(try faded(lines, at: now(box)), toward, accuracy: 1e-5, "with the finger from \(end)")
            held(end)
            turn()
        }
    }

    /// A change Core Animation has not committed, where a run loop runs inside
    /// the callout that made it: the fade is aimed when, and only when, the
    /// change is committed (UIKit's update was seen to commit it in such a
    /// pass at times, and not at others).
    func testAFadeInARunLoopInsideItsCalloutIsAimedOnceCommitted() throws {
        let (p, node) = try paragraph("text-relaid-nested", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, box) = try relaid(node)
        RunLoop.main.run(mode: .default, before: .distantPast) // inside this test's callout
        let committed = try XCTUnwrap(box.animation(forKey: "bounds.size")).beginTime != 0
        XCTAssertEqual(lines.speed != 0, committed, "aimed when, and only when, the change is committed")
        turn()
        let resize = try XCTUnwrap(box.animation(forKey: "bounds.size"))
        XCTAssertEqual(try faded(lines, at: resize.beginTime + resize.duration), 1, accuracy: 1e-4, "aimed after the commit")
    }

    /// A fade aimed in a run loop run inside a callout still under way sends
    /// nothing of that callout, whose changes go with its own commit.
    func testAimingAFadeSendsNothingOfACalloutUnderWay() throws {
        let (p, node) = try paragraph("text-relaid-callout", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        let other = CALayer()
        other.actions = ["opacity": NSNull()]
        try XCTUnwrap(window).layer.addSublayer(other)
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, _) = try relaid(node)
        CATransaction.flush() // committed, not yet aimed
        other.opacity = 0.5 // this callout's own change
        RunLoop.main.run(mode: .default, before: .distantPast) // inside this callout
        XCTAssertGreaterThan(lines.speed, 0, "aimed")
        XCTAssertEqual(other.presentation()?.opacity, 1, "the callout's change not sent")
    }

    /// A commit an open explicit transaction holds back: the fade waits for
    /// it, the old lines held, and is aimed once it comes.
    func testAFadeWaitsForACommitAnOpenTransactionHolds() throws {
        let (p, node) = try paragraph("text-relaid-held-commit", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        CATransaction.begin()
        var held = true
        defer { if held { CATransaction.commit() } }
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, box) = try relaid(node)
        turn()
        XCTAssertEqual(try box.animation(forKey: "bounds.size").map(\.beginTime), 0, "not committed")
        XCTAssertEqual(try faded(lines, at: now(box) + 10), 0, "the old lines held")
        CATransaction.commit(); held = false
        turn()
        let resize = try XCTUnwrap(box.animation(forKey: "bounds.size"))
        XCTAssertGreaterThan(resize.beginTime, 0, "committed")
        XCTAssertEqual(try faded(lines, at: resize.beginTime + resize.duration), 1, accuracy: 1e-4, "done as the resize ends")
    }

    /// A resize committed, then another under a commit an open explicit
    /// transaction holds back: the fade waits for that commit, though the
    /// first one's has come.
    func testAFadeWaitsForTheLatestChangesCommit() throws {
        let (p, node) = try paragraph("text-relaid-latest", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, box) = try relaid(node)
        CATransaction.flush() // committed, not yet aimed
        CATransaction.begin()
        var held = true
        defer { if held { CATransaction.commit() } }
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 300.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        turn()
        try fading(lines)
        XCTAssertEqual(try faded(lines, at: now(box) + 10), 0, "the old lines held")
        CATransaction.commit(); held = false
        turn()
        let last = try XCTUnwrap((box.animationKeys() ?? []).filter { $0.hasPrefix("bounds") }.compactMap(box.animation(forKey:)).max { $0.beginTime < $1.beginTime })
        XCTAssertEqual(try faded(lines, at: last.beginTime + last.duration), 1, accuracy: 1e-4, "done as the latest resize ends")
    }

    /// A fade that ends while a commit is held back leaves nothing on its box
    /// to wait for it.
    func testAFadeEndedWhileACommitIsHeldLeavesNothingWaiting() throws {
        let (p, node) = try paragraph("text-relaid-ended-held", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        CATransaction.begin()
        var held = true
        defer { if held { CATransaction.commit() } }
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, box) = try relaid(node)
        try fading(lines)
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["text": "Mount Fuji is an active volcano."]]]))
        p.textRasters.settleVisible([node])
        XCTAssertNil(lines.animation(forKey: kCATransition), "ended")
        XCTAssertEqual((box.animationKeys() ?? []).filter { !$0.hasPrefix("bounds") && $0 != "position" }, [], "nothing waiting")
        CATransaction.commit(); held = false
        turn()
    }

    /// A spring's resize run to its end: Core Animation finishes the fade
    /// with it, and the lines' clock runs again.
    func testTheFadeEndsWithTheResize() throws {
        let (p, node) = try paragraph("text-relaid-ends", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, box) = try relaid(node)
        turn()
        try fading(lines)
        let deadline = Date().addingTimeInterval(3)
        while box.animation(forKey: "bounds.size") != nil || lines.animation(forKey: kCATransition) != nil, Date() < deadline {
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01))
        }
        XCTAssertNil(box.animation(forKey: "bounds.size"), "the resize over")
        XCTAssertNil(lines.animation(forKey: kCATransition), "the fade finished")
        XCTAssertEqual(lines.speed, 1, "the lines' clock running")
        XCTAssertEqual(lines.timeOffset, 0)
        XCTAssertEqual(lines.beginTime, 0)
    }

    /// Core Animation tells a fade's delegate that it stopped after the box
    /// ended it (other words, then the same words in a resize): a later
    /// fade goes on.
    func testAStopToldLateLeavesALaterFade() throws {
        let (p, node) = try paragraph("text-relaid-late", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 200.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, _) = try relaid(node)
        let first = try fading(lines)
        let delegate = try XCTUnwrap(first.delegate)
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["text": "Mount Fuji is an active volcano."]]]))
        p.textRasters.settleVisible([node])
        XCTAssertNil(lines.animation(forKey: kCATransition), "ended")
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 268.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let second = try fading(lines)
        delegate.animationDidStop?(first, finished: false)
        XCTAssertTrue(lines.animation(forKey: kCATransition) === second, "the later fade goes on")
    }

    /// What the screen shows half way through the fade is the two states
    /// blended: Core Animation's clock stopped at the resize's middle, each
    /// point is about the old lines' frame and the new ones' mixed as far as
    /// the fade is. The one-colour ink is a colour masked by its coverage,
    /// which a fade on the ink alone was seen to leave out.
    func testHalfWayThroughTheFadeTheScreenShowsBothLinesBlended() throws {
        let (p, node) = try paragraph("text-relaid-pixels", text: Self.fuji, height: 200, box: [16, 0, 368, 200])
        XCTAssertTrue(node.textRasterLayer is TintInk, "one colour, masked by its coverage")
        let window = try XCTUnwrap(window)
        let old = try screen(window)
        try stop(p, node)
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 268.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        turn()
        let (lines, box) = try relaid(node)
        let resize = try XCTUnwrap(box.animation(forKey: "bounds.size"))
        stop(p, at: resize.beginTime + resize.duration * 0.5)
        let blend = try faded(lines, at: now(box))
        XCTAssertEqual(blend, 0.5, accuracy: 1e-5)
        let middle = try screen(window)
        stop(p, at: resize.beginTime + resize.duration + 1)
        let new = try screen(window)
        var apart = 0.0, off = 0.0, unlikeNew = 0.0
        for i in old.indices {
            let a = Double(old[i]), b = Double(new[i]), m = Double(middle[i])
            apart += abs(a - b); off += abs(m - (a * (1 - blend) + b * blend)); unlikeNew += abs(m - b)
        }
        XCTAssertGreaterThan(apart, 0, "the lines changed")
        XCTAssertLessThan(off, apart * 0.15, "the middle is the two blended as far as the fade is")
        XCTAssertGreaterThan(unlikeNew, apart * 0.2, "not the new lines alone")
    }

    /// The window's pixels as the screen shows them, at one point each: their luminance.
    private func screen(_ window: UIWindow) throws -> [UInt8] {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1; format.opaque = true
        var drawn = false
        let shot = try XCTUnwrap(UIGraphicsImageRenderer(bounds: window.bounds, format: format).image { _ in
            drawn = window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }.cgImage)
        XCTAssertTrue(drawn, "the screen's pixels")
        var gray = [UInt8](repeating: 0, count: shot.width * shot.height)
        let context = try XCTUnwrap(CGContext(data: &gray, width: shot.width, height: shot.height, bitsPerComponent: 8, bytesPerRow: shot.width,
                                              space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue))
        context.draw(shot, in: CGRect(x: 0, y: 0, width: shot.width, height: shot.height))
        return gray
    }

    /// A box that moves in its view as it grows (its padding following a
    /// safe area that shrinks), its new lines shown before the frame
    /// commits: the fade is on the lines, which keep their place in the box,
    /// so the old lines move with the box rather than stay where it was.
    func testLinesInABoxThatMovesCrossFadeWhereTheBoxIs() throws {
        let (p, node) = try paragraph("text-relaid-moves", text: Self.fuji, width: 300, height: 200, box: [26, 0, 248, 200])
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [6.0, 0.0, 388.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, box) = try relaid(node)
        XCTAssertNotNil(box.animation(forKey: "position"), "the box moves in its view")
        try fading(lines)
        XCTAssertEqual(lines.position, .zero, "at the box's origin")
        XCTAssertEqual(lines.animationKeys(), [kCATransition], "it does not move in the box")
    }

    /// A fitted paragraph (`-exact-fit`) scales its words with its box: the
    /// same words at another scale cross-fade as lines broken again do.
    func testAFittedParagraphRescaledInsideAnAnimationCrossFades() throws {
        let (p, node) = try paragraph("text-relaid-fit", text: Self.fuji, height: 200, box: [16, 0, 368, 200])
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["fitScale": "0.8"]],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 268.0, 200.0]]]))
        }
        CATransaction.flush()
        p.textRasters.settleVisible([node])
        XCTAssertEqual(node.textRasterKey?.spec.runs.first?.size, 16, "painted at its new scale")
        try fading(try relaid(node).lines)
    }

    /// A fade under way goes on unbroken: through lines painted again in its
    /// resize (another colour), and into a new resize, from where it is to
    /// that resize's end, also one back to the old lines' size.
    func testAFadeUnderWayGoesOnUnbrokenThroughRepaintsAndNewResizes() throws {
        for (label, target) in [("text-relaid-again", 200.0), ("text-relaid-back", 368.0)] {
            let (p, node) = try paragraph(label, text: Self.fuji, height: 200, box: [16, 0, 368, 200])
            try stop(p, node)
            UIView.animate(springDuration: 0.5, bounce: 0) {
                p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 268.0, 200.0]]]))
            }
            p.textRasters.settleVisible([node])
            turn()
            let (lines, box) = try relaid(node)
            let first = try fading(lines)
            let resize = try XCTUnwrap(box.animation(forKey: "bounds.size"))
            stop(p, at: resize.beginTime + resize.duration * 0.3)
            let under = try faded(lines, at: now(box))
            XCTAssertEqual(under, 0.3, accuracy: 1e-5)
            let pixels = try XCTUnwrap(node.textRaster)
            p.apply(wireBatch([["op": "style", "id": 2, "style": ["font_size": 20.0, "text_color": [200, 0, 0, 255]]]]))
            p.textRasters.settleVisible([node])
            XCTAssertFalse(node.textRaster === pixels, "painted again")
            turn()
            XCTAssertTrue(lines.animation(forKey: kCATransition) === first, "the same fade goes on")
            XCTAssertEqual(try faded(lines, at: now(box)), under, accuracy: 1e-5, "where it was")
            UIView.animate(springDuration: 0.5, bounce: 0) {
                p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, target, 200.0]]]))
            }
            p.textRasters.settleVisible([node])
            turn()
            XCTAssertTrue(lines.animation(forKey: kCATransition) === first, "on into the new resize (\(label))")
            XCTAssertEqual(try faded(lines, at: now(box)), under, accuracy: 1e-5, "unbroken (\(label))")
            let next = try XCTUnwrap((box.animationKeys() ?? []).filter { $0.hasPrefix("bounds.size") }.compactMap(box.animation(forKey:)).max { $0.beginTime < $1.beginTime })
            XCTAssertEqual(try faded(lines, at: next.beginTime + next.duration), 1, accuracy: 1e-5, "done as the new resize ends (\(label))")
            session?.destroy(); session = nil
        }
    }

    /// In an animation UIKit delays, whose delay Core Animation counts from
    /// the commit, the old lines hold until the box moves. (A clock stopped
    /// on the viewport would hold the delay too: it runs on the app's.)
    func testNewLinesInADelayedAnimationWaitForItsResize() throws {
        let (p, node) = try paragraph("text-relaid-delayed", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        UIView.animate(withDuration: 0.5, delay: 0.3) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, box) = try relaid(node)
        let before = now(box)
        turn()
        let resize = try XCTUnwrap(box.animation(forKey: "bounds.size"))
        XCTAssertGreaterThanOrEqual(resize.beginTime, before + 0.3, "delayed from the commit")
        XCTAssertLessThanOrEqual(resize.beginTime, now(box) + 0.3)
        XCTAssertEqual(try faded(lines, at: resize.beginTime - 0.01), 0, accuracy: 1e-5, "the old lines until the box moves")
        XCTAssertEqual(try faded(lines, at: resize.beginTime + resize.duration / 2), 0.5, accuracy: 1e-5)
        XCTAssertEqual(try faded(lines, at: resize.beginTime + resize.duration), 1, accuracy: 1e-5)
    }

    /// A spring that goes past the box's new size and back: the fade runs
    /// with the resize's time and never goes back.
    func testABouncySpringsFadeNeverGoesBack() throws {
        let (p, node) = try paragraph("text-relaid-bounce", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        try stop(p, node)
        UIView.animate(springDuration: 0.8, bounce: 0.6) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        turn()
        let (lines, box) = try relaid(node)
        let resize = try XCTUnwrap(box.animation(forKey: "bounds.size"))
        var last = -1.0, passed = false, back = false
        for step in 1..<80 {
            let time = resize.beginTime + resize.duration * Double(step) / 80
            stop(p, at: time)
            let width = try XCTUnwrap(box.presentation()).bounds.width
            if width > 368 { passed = true }
            if passed, width < 368 { back = true }
            let at = try faded(lines, at: time)
            XCTAssertGreaterThanOrEqual(at, last, "never back")
            last = at
        }
        XCTAssertTrue(passed && back, "the spring went past and back")
    }

    /// An HDR shadow's cast is painted again with the ink, in the lines the
    /// fade is on, and arrives with it.
    func testAnHDRShadowsCastCrossFadesWithItsLines() throws {
        let hdr: [String: Any] = ["cs": [["s": "srgb-linear", "v": [4.0, 4.0, 4.0, 1.0]]], "c": [255.0, 255.0, 255.0, 255.0]]
        let (p, node) = try paragraph("text-relaid-cast", text: Self.fuji, style: ["text_shadow": ["o": [3.0, 3.0], "b": 0.0, "c": hdr]],
                                      width: 300, height: 200, box: [16, 0, 268, 200])
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, _) = try relaid(node)
        let cast = try XCTUnwrap(node.textRasterLayer?.textCast)
        XCTAssertTrue(cast.superlayer === lines, "in the lines")
        try fading(lines)
    }

    /// Other words in a box UIKit resizes show at once, as a label's new text
    /// does inside an animation, and end a fade under way.
    func testOtherWordsInAResizedBoxShowAtOnce() throws {
        let (p, node) = try paragraph("text-relaid-words", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        let words = "Mount Fuji is an active volcano. Its last eruption was in 1707."
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["text": words]],
                               ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                               ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        XCTAssertEqual(shownWords(node), words, "the new words show")
        let (lines, box) = try relaid(node)
        XCTAssertNil(lines.animation(forKey: kCATransition))
        box.removeAllAnimations() // that resize over
        CATransaction.flush()
        // Laid out again in a resize, then other words while its fade runs
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 268.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        turn()
        try fading(lines)
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["text": Self.fuji]]]))
        p.textRasters.settleVisible([node])
        XCTAssertEqual(shownWords(node), Self.fuji, "the new words show")
        XCTAssertNil(lines.animation(forKey: kCATransition), "at once, the fade ended")
        XCTAssertEqual(lines.speed, 1, "the lines' clock running")
        XCTAssertEqual(lines.timeOffset, 0)
        XCTAssertEqual(lines.beginTime, 0)
    }

    /// A numeral roll still running in the lines when they begin to fade
    /// finishes at once: the fade sets the lines' clock, which would hold it.
    func testANumeralRollUnderWayFinishesWhenItsLinesFade() throws {
        try XCTSkipIf(UIAccessibility.isReduceMotionEnabled, "Reduce Motion: nothing rolls")
        // An HDR shadow keeps the ink's own pixels, which roll (a one-colour ink's are its mask's)
        let hdr: [String: Any] = ["cs": [["s": "srgb-linear", "v": [4.0, 4.0, 4.0, 1.0]]], "c": [255.0, 255.0, 255.0, 255.0]]
        let (p, node) = try paragraph("text-relaid-roll", text: "41 landmarks", style: ["content_transition": "numeric", "text_shadow": ["o": [3.0, 3.0], "b": 0.0, "c": hdr]],
                                      height: 40, box: [16, 0, 368, 40])
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["text": "42 landmarks"]]]))
        p.textRasters.settleVisible([node])
        let ink = try XCTUnwrap(node.textRasterLayer)
        XCTAssertNotNil(ink.animation(forKey: kCATransition), "rolling")
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 268.0, 40.0]]]))
        }
        p.textRasters.settleVisible([node])
        try fading(try relaid(node).lines)
        XCTAssertNil(ink.animation(forKey: kCATransition), "the roll finished")
    }

    /// Lines laid out again after the resize was removed, or outside an
    /// animation, show at once; with resizes on the box that have ended
    /// (at any speed), the fade ends at the commit. The lines' clock runs.
    func testLinesLaidOutAgainWithNoResizeRunningShowAtOnce() throws {
        let (p, node) = try paragraph("text-relaid-still", text: Self.fuji, width: 300, height: 200, box: [16, 0, 268, 200])
        UIView.animate(springDuration: 0.5, bounce: 0) {
            p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 200.0, 200.0]]]))
        }
        p.textRasters.settleVisible([node])
        let (lines, box) = try relaid(node)
        try fading(lines)
        box.removeAllAnimations() // the resize over
        let pixels = try XCTUnwrap(node.textRaster)
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
                           ["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 368.0, 200.0]]]))
        p.textRasters.settleVisible([node])
        XCTAssertFalse(node.textRaster === pixels, "painted again")
        XCTAssertNil(lines.animation(forKey: kCATransition), "outside an animation")
        XCTAssertEqual(lines.speed, 1, "the lines' clock running")
        turn()
        // Resizes that ended, kept on the box
        let ended = CABasicAnimation(keyPath: "bounds.size")
        ended.isAdditive = true
        ended.fromValue = NSValue(cgSize: CGSize(width: 100, height: 0)); ended.toValue = NSValue(cgSize: .zero)
        ended.beginTime = now(box) - 5; ended.duration = 1
        ended.isRemovedOnCompletion = false
        box.add(ended, forKey: "bounds.size")
        let slow = ended.copy() as! CABasicAnimation
        slow.speed = 0.5
        box.add(slow, forKey: "bounds.size-slow")
        CATransaction.flush()
        let wide = try XCTUnwrap(node.textRaster)
        p.apply(wireBatch([["op": "fieldContent", "id": 2, "rect": [16.0, 0.0, 268.0, 200.0]]]))
        p.textRasters.settleVisible([node])
        XCTAssertFalse(node.textRaster === wide, "painted again")
        turn()
        XCTAssertNil(lines.animation(forKey: kCATransition), "resizes that have ended")
        XCTAssertEqual(lines.speed, 1, "the lines' clock running")
        XCTAssertEqual(lines.timeOffset, 0)
    }
}
#endif
