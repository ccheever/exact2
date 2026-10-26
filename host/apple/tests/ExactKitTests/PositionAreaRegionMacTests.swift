// A painted popover sized in its position-area option's region (LLP 1021
// D2), as Chrome sizes the box (host/web/tests/position-area-region.test.mjs,
// whose table this is): in a 400×600 window, a 40×20 invoker and a popover of
// twelve 50×18 boxes in a wrapping row. Each option measures the popover in
// its region (`Kernel::measure_block`), the first that fits is chosen, and
// the region becomes its block, so its frame is Chrome's, mirrored right to
// left as Chrome mirrors it.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class PositionAreaRegionMacTests: XCTestCase {
    /// The page: each row's invoker where Chrome's anchor is, and its popover
    /// (tests/fixtures/position-area-region{,-rtl}.contract, generated with the table).
    static func page(rtl: Bool) throws -> String {
        try String(contentsOfFile: #filePath.replacingOccurrences(of: "ExactKitTests/PositionAreaRegionMacTests.swift", with: "fixtures/position-area-region\(rtl ? "-rtl" : "").contract"), encoding: .utf8)
    }

    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    private func check(rtl: Bool) throws {
        let rows = rtl ? PositionAreaRegionTable.rows.map(PositionAreaRegionTable.mirrored) : PositionAreaRegionTable.rows
        let f = try SheetFixture(page: Self.page(rtl: rtl), size: CGSize(width: 400, height: 600))
        fixture = f
        f.presenter.menus.painted = true
        if rtl { f.presenter.documentDirection = "rtl" }
        f.window.orderFrontRegardless()
        f.settle()
        for (i, row) in rows.enumerated() {
            let pop = try f.node("p\(i)")
            try f.press("a\(i)")
            let end = Date().addingTimeInterval(3)
            while !f.presenter.menus.isOpen(pop), Date() < end { f.settle() }
            f.settle(); f.presenter.menus.layout(); f.settle()
            let layer = try XCTUnwrap(f.presenter.menus.entries.first { $0.popover === pop }?.layer)
            let got = pop.convert(pop.bounds, to: layer)
            let note = "\(rtl ? "rtl " : "")\(row.0),\(row.1) \(row.2) / \(row.3): \(got) want \(row.4)"
            for (a, b) in [(got.minX, row.4.minX), (got.minY, row.4.minY), (got.width, row.4.width), (got.height, row.4.height)] {
                XCTAssertEqual(a, b, accuracy: 0.5, note)
            }
            f.presenter.menus.close(pop)
            f.settle()
        }
    }

    func testAPaintedPopoverIsSizedInItsOptionsRegionAsChromeSizesIt() throws { try check(rtl: false) }
    func testRightToLeftIsTheMirrorImageAsInChrome() throws { try check(rtl: true) }

    /// Six 50×18 boxes in a wrapping row below a 40×20 invoker at (300, 100)
    /// in a 420-wide window: `bottom span-right`'s region is 120 wide, two
    /// boxes a row (a wrapping row is as wide as its region, as in Chrome);
    /// the viewport's would hold all six in one.
    private static func sixBoxes(area: String, flips: String) -> String {
        """
        component Proof
          state flips = \(flips)
          action enable
            flips = some("flip-inline")
          action reset
            flips = none
          view
            column width="100%" height="100%" position="relative"
              button testId="open" popovertarget="pop" appearance="none" position="absolute" left=300 top=100 width=40 height=20 padding=0
              row id="pop" testId="pop" popover="auto" flex-wrap="wrap" margin=0 padding=0 border-width=0 position-area="\(area)" position-try-fallbacks=(match flips { case some(value) => value, case none => none })
                button testId="enable" press=enable width=50 height=18 flex-shrink=0 appearance="none" padding=0
                button testId="reset" press=reset width=50 height=18 flex-shrink=0 appearance="none" padding=0
                box testId="third" width=50 height=18 flex-shrink=0
                box width=50 height=18 flex-shrink=0
                box width=50 height=18 flex-shrink=0
                box width=50 height=18 flex-shrink=0
        """
    }

    /// An authored `none` is no area: the popover keeps the viewport the top
    /// layer implies, one row, at D2's default placement held in the window,
    /// not the region below and right of its invoker (Astra round 1).
    func testAnAuthoredNoneKeepsTheViewport() throws {
        let f = try SheetFixture(page: Self.sixBoxes(area: "none", flips: #"some("flip-inline")"#), size: CGSize(width: 420, height: 600))
        fixture = f
        f.presenter.menus.painted = true
        f.window.orderFrontRegardless()
        f.settle()
        let pop = try f.node("pop")
        try f.press("open")
        f.settle(); f.presenter.menus.layout(); f.settle()
        let layer = try XCTUnwrap(f.presenter.menus.entries.first { $0.popover === pop }?.layer)
        XCTAssertEqual(pop.convert(pop.bounds, to: layer), CGRect(x: 120, y: 120, width: 300, height: 18))
        XCTAssertNil(f.presenter.menus.entries.first?.block, "no region told")
    }

    /// AppKit's popover gives the box its own block and takes it back: painted
    /// again, the region is told again, and the boxes wrap in it, two a row,
    /// as the first time (Astra round 1).
    func testARegionIsToldAgainAfterAppKitsPopover() throws {
        let f = try SheetFixture(page: Self.sixBoxes(area: "bottom span-right", flips: #"some("flip-inline")"#), size: CGSize(width: 420, height: 600))
        fixture = f
        f.window.setFrameOrigin(NSPoint(x: 200, y: 120))
        f.window.makeKeyAndOrderFront(nil)
        f.settle()
        let p = f.presenter, pop = try f.node("pop"), third = try f.node("third")
        func painted(_ note: String) throws {
            let end = Date().addingTimeInterval(3)
            while third.frame.minY != 18, Date() < end { f.settle(); p.menus.layout() }
            let entry = try XCTUnwrap(p.menus.entries.first)
            XCTAssertNil(entry.native, note)
            XCTAssertEqual(pop.convert(pop.bounds, to: entry.layer), CGRect(x: 300, y: 120, width: 120, height: 54), note)
            XCTAssertEqual(third.frame.origin, CGPoint(x: 0, y: 18), "\(note): wrapped in the region")
        }
        try f.press("open")
        try painted("painted")
        try f.press("reset")
        let native = try XCTUnwrap(p.menus.entries.first?.native, "AppKit's")
        native.popover.animates = false
        f.settle()
        try f.press("enable")
        try painted("painted again")
    }
}
#endif
