import AppKit
import XCTest

// Where a keyboard-opened native menu goes (T3MediaActions.anchorPoint, the media menu's `anchor`),
// measured on the f870c419fc reference in Chrome on macOS:
// a focused view's centre, as Chromium's keyboard contextmenu (127, 238 for the 239×78 card
// at 8, 199), or its bottom-left corner (8, 180 for the card at 8, 102).
final class FocusRow: NSView { override var acceptsFirstResponder: Bool { true } }
final class FlippedView: NSView { override var isFlipped: Bool { true } }

final class SidebarMenuAnchorTests: XCTestCase {
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
            let centre = T3MediaActions.anchorPoint("center", focus: thread, in: content)
            XCTAssertEqual(centre?.topLeft, NSPoint(x: 127, y: 238), "flipped \(flipped)")
            XCTAssertEqual(centre?.point, flipped ? NSPoint(x: 127, y: 238) : NSPoint(x: 127, y: 840 - 238))
            let (_, content2, draft) = window(flipped: flipped, rowTop: 102)
            let corner = T3MediaActions.anchorPoint("bottom-left", focus: draft, in: content2)
            XCTAssertEqual(corner?.topLeft, NSPoint(x: 8, y: 180), "flipped \(flipped)")
            XCTAssertEqual(corner?.point, flipped ? NSPoint(x: 8, y: 180) : NSPoint(x: 8, y: 660))
        }
    }

    func testNoFocusedRowFallsBackToThePointer() {
        let (_, content, row) = window(flipped: false, rowTop: 199)
        XCTAssertNil(T3MediaActions.anchorPoint("center", focus: nil, in: content))
        XCTAssertNil(T3MediaActions.anchorPoint("center", focus: content, in: content), "the window itself holds focus")
        XCTAssertNil(T3MediaActions.anchorPoint("middle", focus: row, in: content), "unknown anchors are ignored")
        XCTAssertNil(T3MediaActions.anchorPoint("center", focus: FocusRow(frame: .zero), in: content), "a view outside the window")
    }
}

let suite = XCTestSuite(forTestCaseClass: SidebarMenuAnchorTests.self)
suite.run()
let run = suite.testRun!
print("Keyboard menu anchor tests: \(run.executionCount) run, \(run.totalFailureCount) failed")
exit(run.totalFailureCount == 0 ? 0 : 1)
