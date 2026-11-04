#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A scroll asks the pump for work only when work is owed: a paragraph
/// without all its pixels, or a list's rows. A screen whose text is all
/// painted scrolls with no pass queued and no display link.
final class ScrollPumpIOSTests: XCTestCase {
    private var window: UIWindow!

    /// A 400-point square window showing `p`'s viewport.
    private func mount(_ p: Presenter) {
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
    }

    /// Paragraphs below the port and within a port of it, 18 points apart,
    /// which workers prepare: owed while none has its pixels.
    private func belowThePort(_ ids: [Int]) -> [[String: Any]] {
        ids.flatMap { id in [
            ["op": "create", "id": id, "kind": "text", "props": ["text": "Paragraph \(id) below the port"], "style": ["font_size": 16.0]],
            ["op": "frame", "id": id, "x": 0.0, "y": 420.0 + Double(id - ids[0]) * 18, "w": 300.0, "h": 18.0],
        ] as [[String: Any]] }
    }

    private func fixture(_ label: String) -> ExactSession {
        let session = ExactApp.shared.makeSession(label: label)
        let p = session.presenter
        mount(p)
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
        RegionTextExecutor.queue.isSuspended = true
        defer { RegionTextExecutor.queue.isSuspended = false }
        // Owed at every scroll while its worker cannot run, where a
        // paragraph painted before would take its kept pixels at the first
        p.apply(wireBatch(belowThePort([3]) + [
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0]]))
        window.layoutIfNeeded(); p.views[3]?.layer.displayIfNeeded()
        var made: Set<Int> = []
        for _ in 0..<20 {
            p.scrollPump.scrolled(nil)
            XCTAssertTrue(FrameClock.shared.wants(p.scrollPump))
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.005))
            made.insert(FrameClock.shared.linksMade)
        }
        XCTAssertEqual(made.count, 1, "the app's one link")
    }

    /// At rest, text that waits only for a worker's slot asks for no
    /// frames: each slot that frees asks for the next pass, a dropped
    /// job's too.
    func testTextWaitingForAWorkersSlotAsksForNoFrames() throws {
        let session = ExactApp.shared.makeSession(label: "pump-slots")
        defer { session.destroy() }
        let p = session.presenter
        mount(p)
        // More paragraphs than there are slots
        let ids = Array(10..<30)
        RegionTextExecutor.queue.isSuspended = true
        defer { RegionTextExecutor.queue.isSuspended = false }
        p.apply(wireBatch([["op": "create", "id": 1, "kind": "view"]] + belowThePort(ids) + [
            ["op": "children", "id": 1, "ids": ids], ["op": "roots", "ids": [1]], ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0]]))
        window.layoutIfNeeded(); p.viewport.layer.displayIfNeeded()
        for id in ids { p.views[UInt32(id)]?.layer.displayIfNeeded() }
        p.scrollPump.reset()
        p.scrollPump.requestText()
        XCTAssertTrue(p.scrollPump.asksForFrames)
        waitFor("the pump gives up its frames") { !p.scrollPump.asksForFrames }
        XCTAssertFalse(p.textRasters.hasRoom, "every worker's slot is taken")
        // New words for every paragraph: the next pass drops the jobs in the
        // slots unstarted, and only their freed slots can ask for the passes
        // that give the paragraphs workers again
        p.apply(wireBatch(ids.map { ["op": "props", "id": $0, "set": ["text": "New words for paragraph \($0)"]] }))
        waitFor("the pump gives up its frames again") { !p.scrollPump.asksForFrames }
        XCTAssertFalse(p.textRasters.hasRoom, "every slot is still taken")
        let settled = { ids.allSatisfy { p.views[UInt32($0)]?.textRasterSettled == true } }
        XCTAssertFalse(settled())
        RegionTextExecutor.queue.isSuspended = false
        // Stuck: no worker in flight and no pass to come, with paragraphs still owed
        let stuck = { p.textRasters.inFlight == 0 && !p.scrollPump.asksForFrames && !settled() }
        waitFor("each freed slot asks for the pass that gives the next paragraph a worker", giveUp: stuck) {
            settled() && !p.scrollPump.asksForFrames
        }
    }
}
#endif
