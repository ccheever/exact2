#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class ShelfMacTests: XCTestCase {
    /// Shelf's Add Book → Cancel → Search drive, with its library data left out.
    func testSearchCanBeEditedAfterCancellingAddBook() throws {
        let page = """
        component Shelf
          state adding = false
          state query = ""
          action add
            adding = true
          action cancel
            adding = false
          action search(value: string)
            query = value
          view
            main navigationKey=(adding ? "add" : "library") navigationBack="back" width="100%" height="100%" position="relative"
              column navigationKey="library" position="absolute" inset=0
                header
                  text "Library" role="heading" aria-level=1
                  button press=add aria-label="Add Book" testId="add-book"
                    image "symbol:plus"
                  input type="search" placeholder="Search" value=query input=search testId="search"
              if adding
                column navigationKey="add" navigationPresentation="modal" position="absolute" inset=0
                  header
                    button id="back" press=cancel testId="add-cancel"
                      text "Cancel"
                    text "New Book" role="heading"
                  input placeholder="Title" width=380 height=24

        """
        let f = try SheetFixture(page: page, size: CGSize(width: 900, height: 700))
        defer { f.close() }
        f.window.setFrameOrigin(NSPoint(x: -20000, y: -20000))
        f.window.orderFrontRegardless()
        XCTAssertTrue(f.view.attachWindowToolbar(to: f.window))
        let p = f.presenter, agent = f.session.agentInstance
        XCTAssertNil(agent.tap(["id": Int(try f.node("add-book").id)])["error"])
        _ = agent.clock(["settle": true])
        XCTAssertNotNil(f.window.attachedSheet)
        XCTAssertNil(agent.tap(["id": Int(try f.node("add-cancel").id)])["error"])
        _ = agent.clock(["settle": true])
        XCTAssertNil(f.window.attachedSheet)
        let search = try f.node("search")
        let reply = agent.type(["id": Int(search.id), "text": "piranesi"])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["value"] as? String, "piranesi")
        XCTAssertTrue(try XCTUnwrap(p.toolbar.search.item).isEnabled)
        XCTAssertEqual(search.field?.stringValue, "piranesi")
        XCTAssertNotNil(search.field?.currentEditor())
    }
}
#endif
