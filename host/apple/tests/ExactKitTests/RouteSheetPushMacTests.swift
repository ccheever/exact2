#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class RouteSheetPushMacTests: XCTestCase {
    private func make(navigate: Bool = false, closedby: String = "closerequest", detail: String = "", detailClosedby: String = "closerequest", bar: Bool = true, heading: Bool = true) throws -> SheetFixture {
        try SheetFixture(page: """
        routes nav
          home "/"
            modal "/modal"
            detail "/detail"

        component Proof
          action open
            nav = push(nav, "/modal")
          action pushDetail
            nav = push(nav, "/detail")
          action cancel
            nav = go(nav, "/")
          action follow(location: string)
            nav = go(nav, location)
          view
            column testId="root" navigationKey=`${top(nav).id}` navigationBack="back"\(navigate ? " navigate=follow" : "") width="100%" height="100%"
              for e in stack(nav) key=e.id
                column navigationKey=`${e.id}` navigationPresentation=(e.name == "modal" ? "modal" : "push") closedby=(e.name == "modal" ? "\(closedby)" : "\(detailClosedby)") testId=e.name navigationBar=(e.name == "detail" ? "\(bar ? "auto" : "none")" : "auto") width=400
                  if e.name == "home"
                    button testId="open" press=open
                      text "Open"
                  if e.name == "modal"
                    header
                      text "Modal" role="heading"
                      button id="back" press=cancel
                        text "Cancel"
                    button testId="push" press=pushDetail
                      text "Push"
                  if e.name == "detail"
                    if \(heading ? "true" : "false")
                      header
                        text "Detail" role="heading"
                    button testId="ask" commandfor="confirm" command="show-modal"
                      text "Ask"
                    \(detail)
              dialog id="confirm" role="alertdialog" aria-label="Continue?"
                text "An alert over the route."
                button commandfor="confirm" command="close" testId="answer"
                  text "OK"
        """)
    }

    func testAncestorExitKeepsThePushedSheetUntilDestroy() throws {
        let f = try make()
        defer { f.close() }
        try f.press("open"); try f.press("push")
        let root = try f.node("root"), detail = try f.node("detail"), window = try XCTUnwrap(detail.window)
        f.presenter.apply(wireBatch([["op": "exit", "id": Int(root.id)]]))
        XCTAssertNil(f.presenter.views[detail.id])
        XCTAssertTrue(f.window.attachedSheet === window)
        XCTAssertTrue(detail.window === window)
        XCTAssertTrue(detail.inert)
        XCTAssertTrue(detail.isAccessibilityHidden())
        f.presenter.apply(wireBatch([]))
        XCTAssertTrue(f.window.attachedSheet === window)
        f.presenter.apply(wireBatch([["op": "destroy", "id": Int(root.id)]]))
        XCTAssertNil(f.window.attachedSheet)
        XCTAssertNil(detail.superview)
    }

    func testHiddenAndHeaderlessNavigationBarsLayOutTheirBackRow() throws {
        for heading in [false, true] {
            for bar in [false, true] {
                let f = try make(bar: bar, heading: heading)
                defer { f.close() }
                try f.press("open"); try f.press("push")
                let chrome = try XCTUnwrap(f.front.contentView as? RouteChrome)
                chrome.layoutSubtreeIfNeeded()
                XCTAssertEqual(chrome.back.isHidden, !bar)
                XCTAssertEqual(chrome.rows, bar ? RouteChrome.titleRow + 1 : 0)
                XCTAssertEqual(chrome.content?.frame.minY, bar ? RouteChrome.titleRow + 1 : 0)
                if bar {
                    XCTAssertGreaterThan(chrome.back.frame.width, 0)
                    XCTAssertGreaterThan(chrome.back.frame.minY, 0)
                    XCTAssertLessThan(chrome.back.frame.maxY, RouteChrome.titleRow)
                    chrome.back.performClick(nil); f.settle()
                    XCTAssertNil(f.presenter.navigation.pushedRoute)
                } else { XCTAssertTrue(f.presenter.navigation.goBack()) }
            }
        }
    }

    private func key(_ code: UInt16, _ characters: String, _ flags: NSEvent.ModifierFlags = [], in f: SheetFixture) {
        let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: f.front.windowNumber,
                                    context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
        XCTAssertTrue(f.presenter.routeKey(event, focused: true, in: f.front))
        f.settle()
    }

    private func outside(_ f: SheetFixture) -> Bool {
        var taken = false
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            let event = NSEvent.mouseEvent(with: type, location: NSPoint(x: 20, y: 20), modifierFlags: [], timestamp: 0, windowNumber: f.window.windowNumber,
                                          context: nil, eventNumber: 1, clickCount: 1, pressure: 0)!
            taken = f.presenter.dialogs.pointer(event) || taken
        }
        f.settle()
        return taken
    }

    func testDismissalUsesTheModalBaseAcrossPushAndBack() throws {
        for commandPeriod in [false, true] {
            let f = try make(detailClosedby: "none")
            defer { f.close() }
            try f.press("open"); try f.press("push")
            if commandPeriod { key(47, ".", .command, in: f) } else { key(53, "\u{1b}", in: f) }
            XCTAssertNil(f.window.attachedSheet, "dismissal leaves the whole modal segment")
        }
        let none = try make(closedby: "none")
        defer { none.close() }
        try none.press("open"); try none.press("push")
        let detail = try none.node("detail")
        key(53, "\u{1b}", in: none)
        XCTAssertTrue(none.presenter.dialogs.active === detail)
        key(47, ".", .command, in: none)
        XCTAssertTrue(none.presenter.dialogs.active === detail)
        XCTAssertFalse(outside(none))
        XCTAssertTrue(none.presenter.navigation.goBack()); none.settle()
        let modal = try none.node("modal")
        key(53, "\u{1b}", in: none)
        XCTAssertTrue(none.presenter.dialogs.active === modal)
        XCTAssertFalse(outside(none))

        for popFirst in [false, true] {
            let any = try make(closedby: "any", detailClosedby: "none")
            defer { any.close() }
            try any.press("open"); try any.press("push")
            if popFirst { XCTAssertTrue(any.presenter.navigation.goBack()); any.settle() }
            XCTAssertTrue(outside(any))
            XCTAssertNil(any.window.attachedSheet)
        }
    }

    func testBackRefusesAnAlertOrAnotherSheetOverAPushedRoute() throws {
        for navigate in [false, true] {
            let f = try make(navigate: navigate)
            defer { f.close() }
            try f.press("open"); try f.press("push")
            let route = try f.node("detail")
            XCTAssertTrue(f.presenter.navigation.canGoBack)
            try f.press("ask")
            XCTAssertNotNil(f.presenter.alerts.modal)
            XCTAssertFalse(f.presenter.navigation.canGoBack)
            XCTAssertFalse(try XCTUnwrap(f.presenter.navigation.agentBack()).refused.isEmpty)
            XCTAssertTrue(f.presenter.navigation.pushedRoute?.route === route)
            XCTAssertTrue(f.presenter.alerts.key("Enter")); f.settle()
            let sheet = try XCTUnwrap(route.window), cover = NSWindow()
            sheet.beginSheet(cover)
            XCTAssertFalse(f.presenter.navigation.canGoBack)
            XCTAssertFalse(f.presenter.navigation.goBack())
            XCTAssertTrue(f.presenter.navigation.pushedRoute?.route === route)
            sheet.endSheet(cover)
            let limit = Date().addingTimeInterval(2)
            while sheet.attachedSheet != nil && Date() < limit { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
            XCTAssertTrue(f.presenter.navigation.goBack()); f.settle()
            XCTAssertNil(f.presenter.navigation.pushedRoute)
        }
    }
}
#endif
