import AppKit
import XCTest

// r5-composer: the ref lists' next-page signal (R5ComposerScroll.swift) and the
// citation jump to an assistant row (T3TimelineTurns.swift `cite:` rows).
private let resolve: ExactHatches.ResolveFn = { _, _, _, _, _ in 0 }
private let act: ExactHatches.ActFn = { _, _, _ in 0 }
private let log: ExactHatches.LogFn = { _, _, _ in }
private let delegate: ExactHatches.DelegateFn = { _, _, _ in }
private final class Flipped: NSView { override var isFlipped: Bool { true } }

private final class Fixture {
    let table = UnsafeMutableRawPointer.allocate(byteCount: 40, alignment: 8)
    let host = UnsafeMutablePointer<Int>.allocate(capacity: 1)
    let hooks: ExactHatches
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
    init() {
        _ = NSApplication.shared
        host.initialize(to: 0)
        table.initializeMemory(as: UInt8.self, repeating: 0, count: 40)
        table.storeBytes(of: UInt32(40), as: UInt32.self)
        table.storeBytes(of: unsafeBitCast(resolve, to: UnsafeRawPointer.self), toByteOffset: 8, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(act, to: UnsafeRawPointer.self), toByteOffset: 16, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(log, to: UnsafeRawPointer.self), toByteOffset: 24, as: UnsafeRawPointer.self)
        table.storeBytes(of: unsafeBitCast(delegate, to: UnsafeRawPointer.self), toByteOffset: 32, as: UnsafeRawPointer.self)
        hooks = ExactHatches(host: UnsafeMutableRawPointer(host), table: UnsafeRawPointer(table))!
        window.isReleasedWhenClosed = false
    }
    func scroll(height: CGFloat, content: CGFloat) -> NSScrollView {
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 300, height: height))
        scroll.documentView = Flipped(frame: NSRect(x: 0, y: 0, width: 300, height: content))
        window.contentView?.addSubview(scroll)
        return scroll
    }
}

final class R5ComposerTests: XCTestCase {
    func testScrollTowardTheEndCountsOncePerPage() {
        let fixture = Fixture()
        let scroll = fixture.scroll(height: 224, content: 2800)
        let element = ExactElement(hatch: .t3Anchor, id: "refs", node: 3, hatches: fixture.hooks)
        element.view = scroll; element.platform = scroll
        element.data = ExactData(["anchor": "scroll:details-refs"])
        var changes = 0
        let ends = R5ComposerScroll(changed: { _ in changes += 1 })
        ends.install(element)
        func move(_ y: CGFloat) {
            scroll.contentView.scroll(to: NSPoint(x: 0, y: y)); scroll.reflectScrolledClipView(scroll.contentView)
            NotificationCenter.default.post(name: NSView.boundsDidChangeNotification, object: scroll.contentView)
        }
        move(1000)
        XCTAssertNil((ends.status["scrollEnds"] as? [String: Int])?["scroll:details-refs"], "far from the end: no page")
        move(2800 - 224 - 120)
        XCTAssertEqual(ends.ends["scroll:details-refs"], nil, "120pt from the end is outside the 96pt threshold")
        move(2800 - 224 - 90)
        XCTAssertEqual(ends.ends["scroll:details-refs"], 1, "within 96pt of the end while scrolling down")
        move(2800 - 224)
        XCTAssertEqual(ends.ends["scroll:details-refs"], 1, "one signal per page")
        move(2000); move(2800 - 224)
        XCTAssertEqual(ends.ends["scroll:details-refs"], 1, "scrolling back up and down the same page asks nothing more")
        // The next page arrives: the list grows, which is never a scroll by itself.
        scroll.documentView!.setFrameSize(NSSize(width: 300, height: 5600))
        NotificationCenter.default.post(name: NSView.boundsDidChangeNotification, object: scroll.contentView)
        XCTAssertEqual(ends.ends["scroll:details-refs"], 1)
        move(5600 - 224 - 40)
        XCTAssertEqual(ends.ends["scroll:details-refs"], 2, "the grown list arms the next page")
        XCTAssertEqual(changes, 2)
        ends.remove(element)
        move(5600 - 224)
        XCTAssertEqual(ends.ends["scroll:details-refs"], 2, "a removed list reports nothing")
    }

    func testAMountingListDoesNotCountAsAScroll() {
        let fixture = Fixture()
        // An unflipped document: its growth moves the reading of "top" without any scroll.
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 300, height: 224))
        scroll.documentView = NSView(frame: NSRect(x: 0, y: 0, width: 300, height: 10))
        fixture.window.contentView?.addSubview(scroll)
        let element = ExactElement(hatch: .t3Anchor, id: "refs", node: 4, hatches: fixture.hooks)
        element.view = scroll; element.platform = scroll
        element.data = ExactData(["anchor": "scroll:strip-refs"])
        let ends = R5ComposerScroll()
        ends.install(element)
        scroll.documentView!.setFrameSize(NSSize(width: 300, height: 2800))
        NotificationCenter.default.post(name: NSView.boundsDidChangeNotification, object: scroll.contentView)
        XCTAssertNil(ends.ends["scroll:strip-refs"], "the list mounting its rows is not the reader scrolling")
        // Unflipped, the end is the bottom: origin 0. Scroll there from the top.
        scroll.contentView.scroll(to: NSPoint(x: 0, y: 2800 - 224)); NotificationCenter.default.post(name: NSView.boundsDidChangeNotification, object: scroll.contentView)
        scroll.contentView.scroll(to: NSPoint(x: 0, y: 40)); NotificationCenter.default.post(name: NSView.boundsDidChangeNotification, object: scroll.contentView)
        XCTAssertEqual(ends.ends["scroll:strip-refs"], 1, "a scroll toward the end of an unflipped list counts too")
        let other = ExactElement(hatch: .t3Anchor, id: "other", node: 5, hatches: fixture.hooks)
        other.view = scroll; other.platform = scroll; other.data = ExactData(["anchor": "traits"])
        ends.install(other)
        XCTAssertEqual(ends.ends.count, 1, "plain anchors are not lists")
    }

    func testCitationJumpPlacesTheAssistantRowAndKeepsItOffTheMinimap() {
        let fixture = Fixture()
        let scroll = fixture.scroll(height: 300, content: 3000)
        let transcript = ExactElement(hatch: .t3Transcript, id: "transcript", node: 6, hatches: fixture.hooks)
        transcript.view = scroll; transcript.platform = scroll
        let user = NSView(frame: NSRect(x: 0, y: 100, width: 300, height: 40)), answer = NSView(frame: NSRect(x: 0, y: 1800, width: 300, height: 400))
        scroll.documentView!.addSubview(user); scroll.documentView!.addSubview(answer)
        let rows = [(7, "u1", user), (8, "cite:a1", answer)].map { (node: UInt32, id: String, view: NSView) -> ExactElement in
            let row = ExactElement(hatch: .t3Turn, id: id, node: node, hatches: fixture.hooks)
            row.view = view; row.data = ExactData(["turn": id]); return row
        }
        let turns = T3TimelineTurns(changed: { _ in })
        turns.install(transcript); rows.forEach(turns.install)
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(turns.status["turnsInView"] as? [String], ["u1"], "an assistant row is a jump target, not a turn")
        let done = expectation(description: "jump")
        turns.jump(["op": "timelineJump", "id": "cite:a1", "index": 1, "count": 2, "lead": "citation", "inset": 2.0, "generation": 1]) { reply in
            XCTAssertEqual(reply["ok"] as? Bool, true); done.fulfill()
        }
        wait(for: [done], timeout: 2)
        RunLoop.main.run(until: Date().addingTimeInterval(0.6))
        // min(120, 300 / 3) = 100 above the cited Markdown, which starts 2pt into the row.
        XCTAssertEqual(scroll.contentView.bounds.minY, 1800 + 2 - 100, accuracy: 0.5)
        XCTAssertEqual(turns.status["turnAbove"] as? String, "u1")
        turns.destroy()
    }
}

let suite = XCTestSuite(forTestCaseClass: R5ComposerTests.self)
suite.run()
let run = suite.testRun!
print("R5 composer: \(run.executionCount) tests, \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 && run.executionCount == 3 ? 0 : 1)
