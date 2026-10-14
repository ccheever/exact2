#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class InlineFrameIOSTests: XCTestCase {
    func testEmptyRunHasAnEncodableFrameAtItsInsertionPoint() throws { try checkEdgeCase("empty") }
    func testSmallRunUsesItsOwnFontMetrics() throws { try checkEdgeCase("small") }
    func testHangingSpaceDoesNotAddAClientRect() throws { try checkEdgeCase("wrap") }

    private func checkEdgeCase(_ name: String) throws {
        let session = ExactApp.shared.makeSession(label: "inline-edge")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: InlineFrameFixture.plan, size: CGSize(width: 600, height: 400)).error)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 600, height: 400))
        let view = ExactView(session: session)
        view.frame = window.bounds
        window.addSubview(view)
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        window.layoutIfNeeded()
        try InlineFrameFixture.check(name, session: session)
    }

    func testLayoutListsTheWrappedRunUnionAfterScroll() throws {
        let session = ExactApp.shared.makeSession(label: "inline-frame-ios")
        defer { session.destroy() }
        let p = session.presenter
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 600, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "style": ["font_size": 16.0, "line_height": "20px"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 600, "h": 1000],
            ["op": "frame", "id": 2, "x": 24, "y": 24, "w": 260, "h": 60],
        ]))
        var style = InlineStyle()
        try style.set("font_size", .number(16))
        try style.set("line_height", .string("20px"))
        p.applyParagraph(2, [
            InlineText(id: 3, parent: 2, props: ["text": "Before this, please read "], style: style, handlers: [], paints: true),
            InlineText(id: 4, parent: 2, props: ["text": "see the earlier fix in pull request twelve", "id": "link"], style: style, handlers: [], paints: true),
            InlineText(id: 5, parent: 2, props: ["text": " after it."], style: style, handlers: [], paints: true),
        ])
        window.layoutIfNeeded()
        p.viewport.contentSize = CGSize(width: 600, height: 1000)
        let paragraph = try XCTUnwrap(p.views[2]), run = try XCTUnwrap(p.inlineText(4))
        let agent = Agent(session: session)
        for top in [CGFloat(0), 25] {
            p.viewport.setContentOffset(CGPoint(x: 0, y: top), animated: false)
            let fragments = paragraph.inlineRects(run)
            XCTAssertEqual(fragments.count, 2)
            let expected = fragments.map { agent.box(paragraph, region: $0) }.reduce(CGRect.null) { $0.union($1) }
            let rows = try XCTUnwrap(agent.layout()["nodes"] as? [[String: Any]])
            let row = try XCTUnwrap(rows.first { ($0["id"] as? Int) == 4 }, "layout lists inline runs")
            for (key, want) in zip(["x", "y", "w", "h"], [expected.minX, expected.minY, expected.width, expected.height]) {
                XCTAssertEqual(try XCTUnwrap(row[key] as? Double), Double(want), accuracy: 0.01)
            }
        }
    }
}
#endif
