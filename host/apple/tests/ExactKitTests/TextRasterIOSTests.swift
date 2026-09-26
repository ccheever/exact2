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
        while p.textRasters.inFlight > 0 && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.01)) }
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
