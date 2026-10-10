#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A scroll asks the pump for work only when work is owed: a paragraph
/// without all its pixels, or a list's rows. A screen whose text is all
/// painted scrolls with no pass queued and no display link.
final class ScrollPumpIOSTests: XCTestCase {
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
             "props": ["text": "Sign in with your account"], "style": ["font_size": 16.0]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 300.0, "h": 40.0],
        ]))
        window.layoutIfNeeded(); p.viewport.layer.displayIfNeeded()
        p.views[2]?.layer.displayIfNeeded()
        drain(p)
        p.scrollPump.reset()
        return session
    }
    private func drain(_ p: Presenter) {
        RegionTextExecutor.queue.waitUntilAllOperationsAreFinished()
        let end = Date().addingTimeInterval(2)
        while p.textRasters.inFlight > 0 && Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
    }

    func testAScrollOverPaintedTextAsksForNothing() throws {
        let session = fixture("pump-settled")
        defer { session.destroy() }
        let p = session.presenter
        XCTAssertTrue(try XCTUnwrap(p.views[2]).textRasterSettled)
        for _ in 0..<10 { p.scrollPump.scrolled(nil) }
        XCTAssertFalse(p.scrollPump.asksForFrames, "no pass queued and no display link")
    }

    func testAScrollOverTextWithoutItsPixelsAsksForAPass() throws {
        let session = fixture("pump-owed")
        defer { session.destroy() }
        let p = session.presenter
        p.views[2]?.dropTextRaster()
        p.scrollPump.scrolled(nil)
        XCTAssertTrue(p.scrollPump.asksForFrames, "the pass that gives it its pixels")
    }

    /// Owed work takes frames from the app's one link, asked once for the
    /// scroll: not a link made and torn down per frame.
    func testOwedWorkTakesTheAppsLinkOnce() throws {
        let session = fixture("pump-clock")
        defer { session.destroy() }
        let p = session.presenter
        p.views[2]?.dropTextRaster()
        RegionTextExecutor.queue.isSuspended = true
        defer { RegionTextExecutor.queue.isSuspended = false }
        var made: Set<Int> = []
        for _ in 0..<20 {
            p.scrollPump.scrolled(nil)
            XCTAssertTrue(FrameClock.shared.wants(p.scrollPump))
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.005))
            made.insert(FrameClock.shared.linksMade)
        }
        XCTAssertEqual(made.count, 1, "the app's one link")
    }
}
#endif
