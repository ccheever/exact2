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
}
#endif
