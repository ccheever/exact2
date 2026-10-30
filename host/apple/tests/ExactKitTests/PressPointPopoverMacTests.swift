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
        pointed = e.clientX != 0 || e.clientY != 0
        x = e.clientX
        y = e.clientY
        showPopover("pop", source="open")
      action far(e: MouseEvent)
        pointed = true
        x = e.clientX
        y = e.clientY
        showPopover("pop", source="native")
      action nudge
        x = x + 50
        y = y + 22
      view
        column width="100%" height="100%" position="relative"
          button id="open" testId="open" press=open appearance="none" position="absolute" left=40 top=30 width=300 height=360 padding=0
          text `${x},${y} ${pointed}` testId="status"
          column id="pop" testId="pop" popover="auto" margin=0 padding=0 border-width=0 width=120 height=80 left=(pointed ? `${x}px` : "auto") top=(pointed ? `${y}px` : "auto")
            box height=80
          button id="native" testId="native" press=open position="absolute" left=40 top=395
            text "Native"
          button testId="far" press=far appearance="none" position="absolute" left=200 top=395 width=150 height=20 padding=0
          button testId="nudge" press=nudge appearance="none" position="absolute" left=360 top=395 width=20 height=20 padding=0
          column id="menu" testId="menu" popover="auto" role="menu" margin=0 left=30 top=8
            button testId="row"
              text "Row"
    """

    private var fixture: SheetFixture?
    override func tearDown() { fixture?.close(); fixture = nil; super.tearDown() }

    private func make(painted: Bool = true) throws -> SheetFixture {
        let f = try SheetFixture(page: Self.page, size: CGSize(width: 400, height: 420))
        fixture = f
        f.presenter.menus.painted = painted
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

    /// AppKit's popover, as a hand-built app shows one at a click: pointed at
    /// a 1×1 rectangle at the press, in the window's content view, and
    /// pointed again where its author moves the insets while it shows
    /// (`sync`, `repoint`); from a key, at its invoker, the same popover.
    func testAppKitsPopoverPointsAtThePress() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent's chrome keeps it painted")
        let f = try make(painted: false)
        let open = try f.node("open"), pop = try f.node("pop")
        func pointed() throws -> CGPoint {
            let end = Date().addingTimeInterval(3)
            while !f.presenter.menus.isOpen(pop), Date() < end { f.settle() }
            let entry = try XCTUnwrap(f.presenter.menus.entries.first { $0.popover === pop })
            let native = try XCTUnwrap(entry.native, "AppKit's popover shows it")
            XCTAssertTrue(native.popover.isShown)
            let view = try XCTUnwrap(native.positioning?.view)
            return f.presenter.client(view.convert(native.popover.positioningRect.origin, to: nil))
        }
        try click(f, open, at: CGPoint(x: 70, y: 38))
        var at = try pointed()
        XCTAssertEqual(at.x, 70, accuracy: 0.5); XCTAssertEqual(at.y, 38, accuracy: 0.5)
        XCTAssertEqual(try XCTUnwrap(f.presenter.menus.entries.first { $0.popover === pop }?.native).popover.positioningRect.size, CGSize(width: 1, height: 1))
        XCTAssertTrue(f.presenter.menus.entries.first { $0.popover === pop }?.native?.positioning?.view === f.window.contentView)
        // Its author moves the insets while it shows (no click: AppKit's
        // transient popover closes on one).
        f.session.apply(f.session.runtime.press(try f.node("nudge").id, now: f.session.now())); f.settle()
        at = try pointed()
        XCTAssertEqual(at.x, 120, accuracy: 0.5); XCTAssertEqual(at.y, 60, accuracy: 0.5)
        f.presenter.menus.close(pop); f.settle()
        // A press on one node opening the popover with another as its source:
        // the point lies outside that source, and AppKit still shows it there.
        try click(f, try f.node("far"), at: CGPoint(x: 205, y: 400))
        at = try pointed()
        XCTAssertEqual(at.x, 205, accuracy: 0.5); XCTAssertEqual(at.y, 400, accuracy: 0.5)
        f.presenter.menus.close(pop); f.settle()
        f.presenter.press(open.id); f.settle()
        at = try pointed()
        XCTAssertEqual(at.x, 40, accuracy: 0.5, "the invoker's rectangle")
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

    /// AppKit's button sends its action from its tracking loop: the click it
    /// tracked is the press's, as a node's own is.
    func testANativeButtonsClickCarriesItsPoint() throws {
        let f = try make()
        let button = try XCTUnwrap(f.presenter.controls.controls[try f.node("native").id] as? NativeButtonMac)
        let at = button.convert(NSPoint(x: 10, y: button.bounds.midY), to: nil)
        func event(_ type: NSEvent.EventType) throws -> NSEvent {
            try XCTUnwrap(NSEvent.mouseEvent(with: type, location: at, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                             windowNumber: f.window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0))
        }
        let up = try event(.leftMouseUp)
        NSApp.postEvent(up, atStart: true)
        NSApp.sendEvent(try event(.leftMouseDown))
        if NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true) != nil { NSApp.sendEvent(up) }
        f.settle()
        let want = f.presenter.client(at), said = f.status.split(separator: " ")
        let got = said[0].split(separator: ",").compactMap { Double($0) }
        XCTAssertEqual(said.last, "true", f.status)
        XCTAssertEqual(got.first ?? 0, Double(want.x), accuracy: 0.5); XCTAssertEqual(got.last ?? 0, Double(want.y), accuracy: 0.5)
        XCTAssertNil(f.presenter.nativeClick)
        // VoiceOver's press, whatever click is still current, has none.
        XCTAssertTrue(button.accessibilityPerformPress())
        f.settle()
        XCTAssertEqual(f.status, "0,0 false")
    }

    /// A menu whose author wrote `left` and `top` pops up at that point, not
    /// at its invoker (`popUpPoint`), unless it also wrote a `position-area`.
    func testAMenuPopsUpAtItsInsets() throws {
        let f = try make()
        let open = try f.node("open"), menu = try f.node("menu"), pop = try f.node("pop")
        let at = f.presenter.menus.popUpPoint(NSMenu(), menu, in: open)
        let want = f.presenter.point(fromClient: CGPoint(x: 30, y: 8), in: open)
        XCTAssertEqual(at.x, want.x, accuracy: 0.5); XCTAssertEqual(at.y, want.y, accuracy: 0.5)
        // Under an authored `position-area` CSS lets the area win: no point.
        f.presenter.apply(wireBatch([["op": "style", "id": Int(menu.id), "style": ["position_area": "top", "left": 30.0, "top": 8.0]]]))
        XCTAssertNil(PositionArea.inset(of: menu, viewport: f.presenter.clientSize))
    }
}
#endif
