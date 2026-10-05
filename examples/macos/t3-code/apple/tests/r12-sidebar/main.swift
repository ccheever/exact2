import AppKit
import XCTest

// Lane r12-sidebar: a keyboard-opened sidebar menu (ContextMenu on a focused thread
// row, ContextMenu or Shift+F10 on a draft row) opens at the focused row, as the
// f870c419fc reference does in Chrome on macOS: a thread row's centre (127, 238 for
// the 239×78 card at 8, 199) and a draft row's bottom-left corner (8, 180 for the
// card at 8, 102). T3Sidebar.anchorPoint and the `sidebarMenu` request's `anchor`.
final class FocusRow: NSView { override var acceptsFirstResponder: Bool { true } }
final class FlippedView: NSView { override var isFlipped: Bool { true } }

final class R12SidebarMenuAnchorTests: XCTestCase {
    func window(flipped: Bool, rowTop: CGFloat) -> (NSWindow, NSView, FocusRow) {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1280, height: 840), styleMask: [.titled], backing: .buffered, defer: true)
        let content: NSView = flipped ? FlippedView(frame: NSRect(x: 0, y: 0, width: 1280, height: 840)) : NSView(frame: NSRect(x: 0, y: 0, width: 1280, height: 840))
        window.contentView = content
        // A list container between the content and the row, as the sidebar's scroll view is.
        let list: NSView = flipped ? FlippedView(frame: NSRect(x: 0, y: 0, width: 256, height: 840)) : NSView(frame: NSRect(x: 0, y: 0, width: 256, height: 840))
        content.addSubview(list)
        let row = FocusRow(frame: NSRect(x: 8, y: flipped ? rowTop : 840 - rowTop - 78, width: 239, height: 78))
        list.addSubview(row)
        return (window, content, row)
    }

    func testThreadRowCentreAndDraftRowBottomLeftMatchTheReference() {
        for flipped in [false, true] {
            let (_, content, thread) = window(flipped: flipped, rowTop: 199)
            let centre = T3Sidebar.anchorPoint("center", focus: thread, in: content)
            XCTAssertEqual(centre?.topLeft, NSPoint(x: 127, y: 238), "flipped \(flipped)")
            XCTAssertEqual(centre?.point, flipped ? NSPoint(x: 127, y: 238) : NSPoint(x: 127, y: 840 - 238))
            let (_, content2, draft) = window(flipped: flipped, rowTop: 102)
            let corner = T3Sidebar.anchorPoint("bottom-left", focus: draft, in: content2)
            XCTAssertEqual(corner?.topLeft, NSPoint(x: 8, y: 180), "flipped \(flipped)")
            XCTAssertEqual(corner?.point, flipped ? NSPoint(x: 8, y: 180) : NSPoint(x: 8, y: 660))
        }
    }

    func testNoFocusedRowFallsBackToThePointer() {
        let (_, content, row) = window(flipped: false, rowTop: 199)
        XCTAssertNil(T3Sidebar.anchorPoint("center", focus: nil, in: content))
        XCTAssertNil(T3Sidebar.anchorPoint("center", focus: content, in: content), "the window itself holds focus")
        XCTAssertNil(T3Sidebar.anchorPoint("middle", focus: row, in: content), "unknown anchors are ignored")
        XCTAssertNil(T3Sidebar.anchorPoint("center", focus: FocusRow(frame: .zero), in: content), "a view outside the window")
    }

    func testTheAgentReplyCarriesTheAnchorOfTheWindowsFocusedRow() {
        let (window, _, row) = window(flipped: false, rowTop: 199)
        XCTAssertTrue(window.makeFirstResponder(row))
        let sidebar = T3Sidebar(agent: true) { _ in }
        let reply = expectation(description: "menu reply")
        sidebar.perform(["op": "sidebarMenu", "items": [], "anchor": "center", "generation": 7]) { response in
            let value = response["value"] as? [String: Any]
            XCTAssertEqual(value?["shown"] as? Bool, false, "the agent never enters menu tracking")
            XCTAssertEqual((value?["anchor"] as? [CGFloat]) ?? [], [127, 238])
            reply.fulfill()
        }
        let plain = expectation(description: "pointer menu reply")
        sidebar.perform(["op": "sidebarMenu", "items": [], "generation": 8]) { response in
            XCTAssertTrue((response["value"] as? [String: Any])?["anchor"] is NSNull, "a right click has no anchor")
            plain.fulfill()
        }
        wait(for: [reply, plain], timeout: 1)
        _ = window
    }
}

let suite = XCTestSuite(forTestCaseClass: R12SidebarMenuAnchorTests.self)
suite.run()
let run = suite.testRun!
print("R12 sidebar menu anchor tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)
