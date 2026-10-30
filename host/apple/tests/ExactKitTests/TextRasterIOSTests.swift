#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// Paragraph pixels come from worker jobs the pump admits ahead of the list's
/// travel. What shows gets pixels before the frame commits, from its job if a
/// worker began it, else painted then; a job for what the list has carried
/// past at speed is dropped. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class TextRasterIOSTests: XCTestCase {
    private var window: UIWindow!

    private func fixture(_ label: String) -> ExactSession {
        let session = ExactApp.shared.makeSession(label: label)
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text",
             "props": ["text": "Why a magazine measures first and draws second"], "style": ["font_size": 16.0]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 2000.0],
            // Below the 400 pt scrollport, inside the lead.
            ["op": "frame", "id": 2, "x": 0.0, "y": 500.0, "w": 300.0, "h": 40.0],
        ]))
        // First display asks for pixels too; let that settle, then take them
        // away, so what follows is the pump's alone.
        window.layoutIfNeeded(); p.viewport.layer.displayIfNeeded()
        p.views[2]?.layer.displayIfNeeded()
        drain(p)
        p.views[2]?.dropTextRaster()
        // The pump the publication woke stays out of it: each test calls it.
        p.scrollPump.reset()
        return session
    }
    /// Where the list's scroll carries it: the view moves, nothing redraws.
    private func move(_ p: Presenter, y: CGFloat) { p.views[2]?.frame.origin.y = y }
    /// Let the suspended worker queue run and its mailboxes publish.
    private func drain(_ p: Presenter) {
        RegionTextExecutor.queue.isSuspended = false
        RegionTextExecutor.queue.waitUntilAllOperationsAreFinished()
        let end = Date().addingTimeInterval(2)
        while p.textRasters.inFlight > 0 && Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
    }

    private func preferred(_ node: NodeView) throws -> CGRect {
        let key = try XCTUnwrap(node.textRasterKey)
        guard case let .viewport(_, preferred, _) = key.retention else { XCTFail("expected a viewport request"); return .zero }
        return preferred
    }

    func testAlignedExtentsKeepTheirPhysicalPixelSpan() {
        let bounds = CGRect(x: 0, y: 0, width: 402, height: 874)
        func extent(_ physical: CGRect, scale: CGFloat = 3, clip: CGRect? = nil) -> TextRasterExtent {
            TextRasterJob.alignedExtent(
                CGRect(x: physical.minX / scale, y: physical.minY / scale,
                       width: physical.width / scale, height: physical.height / scale),
                bounds: bounds, clip: clip, scale: scale)
        }

        let hero = extent(CGRect(x: -2, y: -6, width: 787, height: 274))
        XCTAssertEqual(hero.rect.minX, -2 / 3, accuracy: 1e-12)
        XCTAssertEqual(hero.rect.width, 787 / 3, accuracy: 1e-12)
        XCTAssertEqual(hero.rect.height, 274 / 3, accuracy: 1e-12)
        XCTAssertEqual(hero.pixels, CGSize(width: 787, height: 274),
                       "787 physical pixels allocate 787, not 788")

        XCTAssertEqual(extent(CGRect(x: 1, y: 3, width: 781, height: 5)).pixels,
                       CGSize(width: 781, height: 5), "positive thirds retain their edge span")
        XCTAssertEqual(extent(CGRect(x: -700, y: 0, width: 600, height: 5)).pixels,
                       CGSize(width: 600, height: 5), "negative overflow retains its edge span")
        XCTAssertEqual(extent(CGRect(x: -700, y: 0, width: 781, height: 5)).pixels,
                       CGSize(width: 781, height: 5), "an extent crossing zero retains its edge span")
        XCTAssertEqual(extent(CGRect(x: 1, y: 3, width: 782, height: 5)).pixels,
                       CGSize(width: 782, height: 5), "adjacent positive thirds retain their edge span")

        for scale: CGFloat in [1, 2] {
            let ordinary = extent(CGRect(x: -7, y: -5, width: 263, height: 91), scale: scale)
            XCTAssertEqual(ordinary.pixels, CGSize(width: 263, height: 91), "\(scale)x")
        }

        let clipped = TextRasterJob.alignedExtent(
            CGRect(x: -20, y: -20, width: 300, height: 160), bounds: bounds,
            clip: CGRect(x: CGFloat(1) / 3, y: CGFloat(2) / 3,
                         width: CGFloat(781) / 3, height: CGFloat(274) / 3), scale: 3)
        XCTAssertEqual(clipped.pixels, CGSize(width: 781, height: 274))
        XCTAssertEqual(clipped.rect.minX, 1 / 3, accuracy: 1e-12)

        let unaligned = CGRect(x: 0.2, y: 0.1, width: 1, height: 1.1)
        let unchanged = TextRasterJob.alignedExtent(unaligned, bounds: unaligned, clip: nil, scale: 3)
        XCTAssertEqual(unchanged.rect, unaligned)
        XCTAssertNil(unchanged.pixels, "an unchanged layout box keeps width-only allocation")
        XCTAssertEqual((unchanged.rect.width * 3).rounded(.up), 3)
    }

    func testCroppedRenderAllocatesOnePixelPerAlignedPhysicalPixel() throws {
        let engine = TextEngine(resolve: { _ in nil })
        let run = Run(text: "Finder Apple Health", size: 40, weight: 900, family: 0,
                      italic: true, lineHeight: 44, letterSpacing: -0.5)
        let spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [255, 255, 255, 255])
        let paragraph = engine.paragraph(spec, width: 360)
        let geometry = try XCTUnwrap(engine.measuredBreaks(spec, width: 360))
        let box = CGRect(x: 0, y: 0, width: 360, height: paragraph.height)
        let job = TextRasterJob(source: engine.attributed(spec), ranges: geometry.ranges,
                                baselines: geometry.baselines, flush: 0, box: box,
                                size: box.size, scale: 3, crop: true)
        let raster = try XCTUnwrap(job.render(lines: paragraph.lines))
        XCTAssertNotEqual(raster.frame, box, "crop exercises the aligned-ink path")
        XCTAssertEqual(raster.frame.minX * 3, (raster.frame.minX * 3).rounded(), accuracy: 1e-9)
        XCTAssertEqual(raster.frame.minY * 3, (raster.frame.minY * 3).rounded(), accuracy: 1e-9)
        XCTAssertEqual(raster.image.width, Int((raster.frame.width * 3).rounded()))
        XCTAssertEqual(raster.image.height, Int((raster.frame.height * 3).rounded()))
        XCTAssertTrue(raster.covered.contains(raster.frame), "crop keeps the larger covered region")
    }

    func testAParagraphThatShowsBeforeItsWorkerBeganIsPaintedBeforeTheCommit() throws {
        let session = fixture("text-visible-paint")
        defer { session.destroy(); RegionTextExecutor.queue.isSuspended = false }
        let p = session.presenter
        let node = try XCTUnwrap(p.views[2])
        XCTAssertFalse(p.textIsVisible(node))
        RegionTextExecutor.queue.isSuspended = true
        p.refreshVisibleText()
        XCTAssertEqual(p.textRasters.inFlight, 1, "the lead paragraph has a worker job")
        XCTAssertFalse(node.textRasterReady)

        // The list scrolls it in; no worker has begun its job.
        move(p, y: 100)
        XCTAssertTrue(p.textIsVisible(node))
        p.paintVisibleText()
        XCTAssertTrue(node.textRasterReady, "what shows has pixels before the frame commits")
        XCTAssertNotNil(node.textRaster)

        drain(p)
        XCTAssertEqual(p.textRasters.inFlight, 0)
        XCTAssertFalse(node.textRasterFailed, "the dropped job is not a failure")
        XCTAssertTrue(node.textRasterSettled)
    }

    func testAJobForAParagraphTheListCarriedPastAtSpeedIsDroppedAndOwedAgain() throws {
        let session = fixture("text-passed")
        defer { session.destroy(); RegionTextExecutor.queue.isSuspended = false }
        let p = session.presenter
        let node = try XCTUnwrap(p.views[2])
        RegionTextExecutor.queue.isSuspended = true
        p.refreshVisibleText()
        XCTAssertEqual(p.textRasters.inFlight, 1)

        // Travelling down at 5,000 pt/s, the list has carried it above the port.
        move(p, y: -200)
        p.refreshVisibleText(velocity: 5000)
        XCTAssertNil(node.textRasterKey, "a dropped job leaves the paragraph owing one")
        drain(p)
        XCTAssertEqual(p.textRasters.inFlight, 0)
        XCTAssertNil(node.textRaster, "no worker painted the passed paragraph")
        XCTAssertFalse(node.textRasterFailed)

        // The list turns back: it is ahead of the travel again and gets a job.
        move(p, y: 500)
        RegionTextExecutor.queue.isSuspended = true
        p.refreshVisibleText(velocity: 5000)
        XCTAssertEqual(p.textRasters.inFlight, 1)
    }

    /// A paragraph taller than 4,096 pt paints a band around the port. A
    /// worker paints one that leads the port's travel; the port keeps it
    /// until it comes within half a port of its edge ahead, and then a worker
    /// paints the next. A paragraph that shows without pixels is painted here.
    /// An 800 pt port: at 3x only the worker's budget holds two of them.
    func testATallParagraphsBandsArePaintedAheadOfThePort() throws {
        let session = ExactApp.shared.makeSession(label: "text-bands")
        defer { session.destroy(); RegionTextExecutor.queue.isSuspended = false }
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 800, height: 800))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        let words = Array(repeating: "Why a magazine measures first and draws second.", count: 600).joined(separator: " ")
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": words], "style": ["font_size": 16.0]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 800.0, "h": 6000.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 800.0, "h": 6000.0],
        ]))
        window.layoutIfNeeded()
        func scroll(_ y: CGFloat) { p.viewport.contentOffset.y = y }
        // Mid-paragraph, where no edge clamps the band. A scroll paints what
        // shows at once: let that settle, then take the pixels away.
        scroll(2000)
        drain(p)
        let node = try XCTUnwrap(p.views[2])
        node.dropTextRaster()
        RegionTextExecutor.queue.isSuspended = true
        p.textRasters.ensure(node, urgent: false)
        XCTAssertEqual(p.textRasters.inFlight, 1, "a worker paints the band")
        let first = try preferred(node)
        let port = p.textPreparationRect(node)
        XCTAssertTrue(first.contains(port))
        XCTAssertGreaterThan(first.maxY - port.maxY, port.minY - first.minY, "with more of it ahead of the travel than behind")
        drain(p)
        XCTAssertTrue(node.textRasterReady)

        scroll(2050)
        XCTAssertTrue(p.textRasters.ensure(node, urgent: false))
        XCTAssertEqual(node.specs?.accepted?.result.coverage.answered.maxY, first.maxY, "well inside the band, the port keeps its rows")
        XCTAssertEqual(p.textRasters.inFlight, 0)

        scroll(first.maxY - 150 - port.height)
        RegionTextExecutor.queue.isSuspended = true
        p.textRasters.ensure(node, urgent: false)
        let next = try preferred(node)
        XCTAssertEqual(p.textRasters.inFlight, 1, "within half a port of its edge, a worker paints the next")
        XCTAssertGreaterThan(next.maxY, first.maxY)
        drain(p)

        scroll(4000)
        RegionTextExecutor.queue.isSuspended = true
        node.dropTextRaster()
        p.textRasters.ensure(node, urgent: true)
        XCTAssertTrue(node.textRasterReady, "shown without pixels: painted here, the queue held")
        XCTAssertTrue(node.textRasterAnswers(p.textPreparationRect(node)))
    }

    /// Two tall paragraphs side by side share one allowance for room ahead
    /// of the port: while one band holds it, the other's is the port's
    /// height, and once that paragraph lets its pixels go the other leads.
    func testTallParagraphsShareTheRoomAheadOfThePort() throws {
        let session = ExactApp.shared.makeSession(label: "text-bands-shared")
        defer { session.destroy(); RegionTextExecutor.queue.isSuspended = false }
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 1600, height: 800))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        let words = Array(repeating: "Why a magazine measures first and draws second.", count: 600).joined(separator: " ")
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": words], "style": ["font_size": 16.0]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": words], "style": ["font_size": 16.0]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 1600.0, "h": 6000.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 800.0, "h": 6000.0],
            ["op": "frame", "id": 3, "x": 800.0, "y": 0.0, "w": 800.0, "h": 6000.0],
        ]))
        window.layoutIfNeeded()
        p.viewport.contentOffset.y = 2000
        drain(p)
        let left = try XCTUnwrap(p.views[2]), right = try XCTUnwrap(p.views[3])
        left.dropTextRaster(); right.dropTextRaster()
        RegionTextExecutor.queue.isSuspended = true
        p.textRasters.ensure(left, urgent: false)
        let held = try preferred(left)
        p.textRasters.ensure(right, urgent: false)
        let rightDemand = p.textPreparationRect(right)
        drain(p)
        for node in [left, right] {
            let accepted = try XCTUnwrap(node.specs?.accepted)
            XCTAssertTrue(node.textRasterAnswers(p.textPreparationRect(node)))
            XCTAssertFalse(accepted.result.coverage.whole)
            XCTAssertLessThanOrEqual(accepted.result.image.bytesPerRow * accepted.result.image.height, 48 * 1024 * 1024)
        }
        let pixels = try XCTUnwrap(left.textRaster)
        left.invalidateText()
        XCTAssertTrue(left.textRaster === pixels, "old output stays mounted during invalidation")
        right.dropTextRaster()
        RegionTextExecutor.queue.isSuspended = true
        p.textRasters.ensure(right, urgent: false)
        XCTAssertTrue(try preferred(right).contains(rightDemand))
        XCTAssertLessThanOrEqual(try preferred(right).height, held.height)
        drain(p)
        XCTAssertTrue(right.textRasterAnswers(p.textPreparationRect(right)))
    }

    /// A paragraph whose pixels are its words already returns before
    /// `ensure` asks whether it is urgent or what shows: a batch's display
    /// pass asks again of every paragraph the batch gave new words.
    func testEnsureAsksNothingOfAParagraphWhosePixelsAreCurrent() throws {
        let session = fixture("text-current")
        defer { session.destroy() }
        let p = session.presenter
        let node = try XCTUnwrap(p.views[2])
        move(p, y: 100)
        p.paintVisibleText()
        XCTAssertTrue(node.textRasterReady)
        let image = try XCTUnwrap(node.textRaster)
        p.textRasters.ensure(node, urgent: true)
        XCTAssertTrue(node.textRaster === image)
        node.invalidateText()
        p.textRasters.ensure(node, urgent: true)
        XCTAssertTrue(node.textRaster === image, "identical content can reuse its accepted pixels")
        XCTAssertTrue(node.textRasterSettled)
    }

    /// A scroll moves no paragraph in its scroller's content, so the index
    /// the passes read (`TextIndex`) stands across it: one far from the port
    /// is not visited, and one the scroll brings in is painted.
    func testTheIndexSkipsAFarParagraphAndPaintsOneAScrollBringsIn() throws {
        let session = fixture("text-index-scroll")
        defer { session.destroy() }
        let p = session.presenter
        let node = try XCTUnwrap(p.views[2])
        p.viewport.contentSize = CGSize(width: 400, height: 2000)
        move(p, y: 1500)
        var visited: [UInt32] = []
        p.textIndex.visit(p.textViews, reach: 400, viewport: p.viewport) { visited.append($0.id); return true }
        XCTAssertEqual(visited, [], "700 pt below the port's lead")
        let builds = p.textIndex.builds
        p.viewport.contentOffset.y = 1300
        p.paintVisibleText()
        XCTAssertEqual(p.textIndex.builds, builds, "the scroll left the index standing")
        XCTAssertTrue(p.textIsVisible(node))
        XCTAssertTrue(node.textRasterReady, "what the scroll brought in has pixels")
    }

    /// A document in its mount, under a native container (r27's inbox), is
    /// indexed by its place in the mount as it is in the viewport: a far
    /// paragraph is not visited every pass.
    func testTheIndexSkipsAFarParagraphInTheDocumentsMount() throws {
        let session = fixture("text-index-mount")
        defer { session.destroy() }
        let p = session.presenter
        window.addSubview(p.documentMount)
        p.hostDocument(fixed: true)
        XCTAssertTrue(p.documentFixed)
        move(p, y: 1500)
        var visited: [UInt32] = []
        p.textIndex.visit(p.textViews, reach: 400, viewport: p.viewport) { visited.append($0.id); return true }
        XCTAssertEqual(visited, [], "1,100 pt below the mount's bounds, past the reach")
        move(p, y: 300)
        p.textIndex.visit(p.textViews, reach: 0, viewport: p.viewport) { visited.append($0.id); return true }
        XCTAssertEqual(visited, [2], "in the mount's bounds")
    }

    /// The clips a refresh finds once per ancestor (`TextClips`) meet as
    /// boxes in the viewport's space, which a rotated parent's are not:
    /// under a transform the walk answers, as it does with none.
    func testTheMemoAnswersAsTheWalkUnderARotatedClip() throws {
        let session = fixture("text-rotated-clip")
        defer { session.destroy() }
        let p = session.presenter
        let parent = try XCTUnwrap(p.views[1]), node = try XCTUnwrap(p.views[2])
        parent.frame = CGRect(x: 100, y: 100, width: 100, height: 100)
        parent.clipsToBounds = true
        parent.transform = CGAffineTransform(rotationAngle: .pi / 4)
        // Below the parent's own bounds, inside their rotated bounding box.
        node.frame = CGRect(x: 0, y: 120, width: 100, height: 20)
        p.textRasters.ensure(node, urgent: true)
        let walked = p.textScrollportRect(node)
        XCTAssertTrue(walked.isEmpty, "\(walked)")
        p.textClips = TextClips(p.viewport, soon: 0, reach: 0)
        defer { p.textClips = nil }
        XCTAssertEqual(p.textScrollportRect(node), walked)
    }

    func testColdHorizontalOverflowAndReversalConvergeWithoutReplacingTheNode() throws {
        let session = fixture("horizontal-raster-window")
        defer { session.destroy(); RegionTextExecutor.queue.isSuspended = false }
        let p = session.presenter, node = try XCTUnwrap(session.presenter.views[2])
        let source = String(repeating: "independently scrolling code line ", count: 400)
        p.apply(wireBatch([
            ["op": "props", "id": 2, "set": ["text": source]],
            ["op": "style", "id": 2, "set": ["white_space": "pre", "font_family": "monospace"]],
            ["op": "frame", "id": 2, "x": 14.0, "y": 100.0, "w": 336.0, "h": 44.0],
        ]))
        let spec = node.paragraphSpec(), paragraph = try XCTUnwrap(node.paragraphLayout())
        let advance = paragraph.lines.map { CTLineGetTypographicBounds($0, nil, nil, nil) }.max()!
        let scale = window.screen.scale
        p.viewport.contentSize.width = ceil((advance + 14) * scale) / scale
        let edge = p.viewport.contentSize.width - p.viewport.bounds.width
        let incarnation = node.incarnation
        p.viewport.contentOffset.x = edge
        node.dropTextRaster()
        RegionTextExecutor.queue.isSuspended = true
        p.paintVisibleText()
        XCTAssertGreaterThan(p.viewport.contentOffset.x, node.frame.maxX)
        XCTAssertFalse(p.textScrollportRect(node).isEmpty, "ink remains visible after the allocation has left")
        XCTAssertTrue(node.textRasterAnswers(p.textScrollportRect(node)))
        XCTAssertFalse(node.textRasterWhole)
        let builds = p.textIndex.builds
        for x in [CGFloat(0), edge, edge / 2, edge] {
            p.viewport.contentOffset.x = x
            p.paintVisibleText()
            XCTAssertTrue(node.textRasterAnswers(p.textScrollportRect(node)))
            XCTAssertEqual(node.incarnation, incarnation)
            XCTAssertEqual(node.paragraphSpec(), spec)
        }
        for _ in 0..<TextRasterizer.concurrency * 3 {
            XCTAssertFalse(p.refreshVisibleText(), "current admitted demand stays out of deferred debt while workers are paused")
            XCTAssertTrue(node.textRasterAnswers(p.textScrollportRect(node)))
        }
        drain(p)
        _ = p.refreshVisibleText()
        drain(p)
        XCTAssertFalse(p.refreshVisibleText(), "accepted or admitted work is not deferred pump debt")
        drain(p)
        let pixels = node.textRaster
        XCTAssertFalse(p.refreshVisibleText())
        XCTAssertTrue(node.textRaster === pixels, "an unchanged demand does not repaint trimmed lead")
        XCTAssertEqual(p.textIndex.builds, builds, "moving a raster window does not rebuild the index")
        let checkpoint = session.text.checkpoint()
        session.text.restore(checkpoint)
        XCTAssertFalse(node.textRasterSettled)
        XCTAssertFalse(node.textRasterReady, "old engine namespaces cannot serve current requests")
        p.paintVisibleText()
        XCTAssertTrue(node.textRasterAnswers(p.textScrollportRect(node)))
    }
}
#endif
