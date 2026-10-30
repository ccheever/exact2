// Stage 14 B on the Mac: a popover is laid out in the viewport, a block the
// kernel implies, not in its positioned ancestor. The painted popover (the
// driver's own chrome; AppKit's is `PopoverNativeMacTests`, whose host block
// wins) is the size the web gives it, the runner reads it where it is shown,
// a pane's popover is sized by the window and not by the pane, and a box
// crossing into its block does not move.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class TopLayerBlockMacTests: XCTestCase {
    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    private func make(_ page: String) throws -> SheetFixture {
        let f = try SheetFixture(page: page)
        fixture = f
        f.presenter.menus.painted = true
        f.window.orderFrontRegardless()
        f.settle()
        return f
    }

    private func open(_ f: SheetFixture, _ invoker: String, _ pop: NodeView) throws {
        try f.press(invoker)
        let end = Date().addingTimeInterval(3)
        while !f.presenter.menus.isOpen(pop), Date() < end { f.settle() }
        XCTAssertTrue(f.presenter.menus.isOpen(pop), "painted in the page's top layer")
        f.settle()
    }

    /// Harmony's Pins popover with no width (`top-layer-pins.contract`):
    /// its cards on one line, 368 by 64 as Chrome lays the same contract
    /// out, where the header 100 points wide had trimmed it to one a line.
    func testAPaintedPopoverWithNoWidthIsTheWebsSize() throws {
        let source = try String(contentsOfFile: #filePath.replacingOccurrences(of: "ExactKitTests/TopLayerBlockMacTests.swift", with: "fixtures/top-layer-pins.contract"), encoding: .utf8)
        let f = try make(source), p = f.presenter
        let pins = try f.node("pins")
        try open(f, "open", pins)
        XCTAssertEqual(pins.frame.size, CGSize(width: 368, height: 64))
        XCTAssertEqual(p.blocks.rects[pins.id], CGRect(x: 0, y: 0, width: 368, height: 64), "its block: no margins")
        // Told to the runner by its block, where a person sees it.
        let told = try XCTUnwrap(p.openTopLayer.frames.first { $0.0 == pins.id })
        XCTAssertEqual(told.1.size, CGSize(width: 368, height: 64))
    }

    /// A split's pane is a block a host gives (LLP 1115.002 D8.2); a
    /// popover in it is in the viewport's, the window's layout viewport, as
    /// on the web: `width: 50%` is half the window, not half the pane.
    func testAPanesPopoverIsSizedByTheWindowNotThePane() throws {
        let f = try make("""
        component Proof
          view
            column testId="page" width="100%" height="100%"
              button testId="open" popovertarget="half"
                text "Open"
              box testId="stack" position="relative" flex=1 min-height=0
                column testId="detail" position="absolute" inset=0
                  column id="half" testId="half" popover="auto" width="50%" height=40
        """)
        let p = f.presenter, detail = try f.node("detail"), half = try f.node("half")
        // In the page's root: the menus open only a popover connected to it.
        let pane = BlockView(frame: NSRect(x: 0, y: 0, width: 300, height: 400))
        p.root.addSubview(pane)
        p.blocks.show(detail, in: pane, block: .size(pane.bounds.size))
        f.settle()
        XCTAssertTrue(detail.superview === pane)
        try open(f, "open", half)
        XCTAssertEqual(half.frame.width, 450, "half the window's 900")
        XCTAssertNotEqual(half.frame.width, pane.bounds.width / 2)
    }

    /// An open painted popover whose author makes it `sticky` leaves the
    /// viewport's block, and moves back in when the `sticky` goes: a change
    /// of space, not of place, so its declared layout transition plays
    /// nothing either way.
    func testAPaintedPopoverCrossingIntoItsBlockDoesNotMove() throws {
        let f = try make("""
        component Proof
          state inflow = false
          action toggle
            inflow = !inflow
          view
            column testId="page" width="100%" height="100%"
              button testId="open" popovertarget="pop"
                text "Open"
              button testId="toggle" press=toggle
                text "Toggle"
              view height=100
              column id="pop" testId="pop" popover="manual" position=(inflow ? "sticky" : "absolute") height=50 -exact-layout-transition="1s linear"
                text "Inside"
        """)
        let p = f.presenter, pop = try f.node("pop")
        try open(f, "open", pop)
        XCTAssertNotNil(p.blocks.rects[pop.id], "in the viewport's block")
        for _ in 0..<2 {
            try f.press("toggle")
            let end = Date().addingTimeInterval(0.3)
            while Date() < end {
                f.settle()
                XCTAssertEqual(pop.layoutOffset, .zero, "no move across the block's boundary")
                XCTAssertEqual(pop.layoutResize, .zero)
            }
        }
        XCTAssertNil(p.blocks.used[pop.id], "the host gave it none")
    }
}
#endif
