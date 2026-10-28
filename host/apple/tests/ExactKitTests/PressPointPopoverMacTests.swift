// LLP 1021 D2: a popover opened at the press. The button's `press` hears the
// click's `clientX`/`clientY` and writes them as the popover's `left`/`top`;
// a key's press is at 0, 0 and leaves them `auto`, so the popover opens at
// its invoker. A painted popover goes to the point, the other way from it
// near the window's edge; a menu pops up at it.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class PressPointPopoverMacTests: XCTestCase {
    static let page = """
    component Proof
      state x = 0
      state y = 0
      state pointed = false
      action open(e: MouseEvent)
        pointed = e.clientX != 0 or e.clientY != 0
        x = e.clientX
        y = e.clientY
        showPopover("pop", source="open")
      view
        column width="100%" height="100%" position="relative"
          button id="open" testId="open" press=open appearance="none" position="absolute" left=40 top=30 width=300 height=360 padding=0
          text `${x},${y} ${pointed}` testId="status"
          column id="pop" testId="pop" popover="auto" margin=0 padding=0 border-width=0 width=120 height=80 left=(pointed ? `${x}px` : "auto") top=(pointed ? `${y}px` : "auto")
            box height=80
          column id="menu" testId="menu" popover="auto" role="menu" margin=0 left=30 top=8
            button testId="row"
              text "Row"
    """

    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    private func make() throws -> SheetFixture {
        let f = try SheetFixture(page: Self.page, size: CGSize(width: 400, height: 420))
        fixture = f
        f.presenter.menus.painted = true
        f.window.orderFrontRegardless()
        f.settle()
        return f
    }

    /// A click released at `client`, a point of the viewport, on `node`, sent as its press.
    private func click(_ f: SheetFixture, _ node: NodeView, at client: CGPoint) throws {
        let p = f.presenter.point(fromClient: client, in: f.view), window = f.view.convert(p, to: nil)
        let up = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseUp, location: window, modifierFlags: [], timestamp: 0, windowNumber: f.window.windowNumber,
                                                  context: nil, eventNumber: 1, clickCount: 1, pressure: 0))
        f.presenter.press(node.id, click: up)
        f.settle()
    }

    /// Where the open popover's border box is, from the viewport's top-left.
    private func opened(_ f: SheetFixture, _ id: String) throws -> CGRect {
        let pop = try f.node(id), end = Date().addingTimeInterval(3)
        while !f.presenter.menus.isOpen(pop), Date() < end { f.settle() }
        f.settle(); f.presenter.menus.layout(); f.settle()
        let layer = try XCTUnwrap(f.presenter.menus.entries.first { $0.popover === pop }?.layer)
        let origin = f.presenter.point(fromClient: .zero, in: layer)
        return pop.convert(pop.bounds, to: layer).offsetBy(dx: -origin.x, dy: -origin.y)
    }

    func testAPressedPopoverOpensAtThePressAndTheOtherWayNearTheEdge() throws {
        let f = try make()
        let open = try f.node("open"), pop = try f.node("pop")
        try click(f, open, at: CGPoint(x: 70, y: 38))
        XCTAssertEqual(f.status, "70,38 true")
        let near = try opened(f, "pop")
        XCTAssertEqual(near.minX, 70, accuracy: 0.5); XCTAssertEqual(near.minY, 38, accuracy: 0.5)
        f.presenter.menus.close(pop); f.settle()
        // By the window's bottom-right corner: up and to the left of the point, inside the window.
        try click(f, open, at: CGPoint(x: 330, y: 380))
        let far = try opened(f, "pop")
        XCTAssertEqual(far.maxX, 330, accuracy: 0.5); XCTAssertEqual(far.maxY, 380, accuracy: 0.5)
        XCTAssertTrue(CGRect(origin: .zero, size: f.presenter.clientSize).contains(far), "\(far)")
        f.presenter.menus.close(pop); f.settle()
    }

    /// A key's press (no click) is at 0, 0: the author leaves the insets
    /// `auto` and its invoker places the popover, from the invoker's start
    /// (below it, the window too short, shifted up into it).
    func testAKeysPressOpensItAtItsInvoker() throws {
        let f = try make()
        let open = try f.node("open")
        f.presenter.press(open.id); f.settle()
        XCTAssertEqual(f.status, "0,0 false")
        let box = try opened(f, "pop")
        XCTAssertEqual(box.minX, 40, accuracy: 0.5)
    }

    /// A menu whose author wrote `left` and `top` pops up at that point, not
    /// at its invoker (`popUpPoint`), and is no AppKit popover when it is not one.
    func testAMenuPopsUpAtItsInsets() throws {
        let f = try make()
        let open = try f.node("open"), menu = try f.node("menu"), pop = try f.node("pop")
        let at = f.presenter.menus.popUpPoint(NSMenu(), menu, in: open)
        let want = f.presenter.point(fromClient: CGPoint(x: 30, y: 8), in: open)
        XCTAssertEqual(at.x, want.x, accuracy: 0.5); XCTAssertEqual(at.y, want.y, accuracy: 0.5)
        XCTAssertTrue(MenuHost.selectsPainted(menu))
        XCTAssertFalse(MenuHost.selectsPainted(pop), "its insets are auto until a click says where")
    }
}
#endif
