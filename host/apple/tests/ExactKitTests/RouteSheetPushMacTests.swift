#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class RouteSheetPushMacTests: XCTestCase {
    private func make(navigate: Bool = false, closedby: String = "closerequest", detail: String = "") throws -> SheetFixture {
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
                column navigationKey=`${e.id}` navigationPresentation=(e.name == "modal" ? "modal" : "push") closedby=(e.name == "modal" ? "\(closedby)" : "closerequest") testId=e.name width=400
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
