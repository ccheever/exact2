import XCTest
@testable import ExactKit

final class RouteFollowTests: XCTestCase {
    private final class Delegate: ExactSessionDelegate {
        var opened: [String] = []
        func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
            if name == "openURL", let href = args.first as? String { opened.append(href) }
        }
    }

    func testLinksAndIncomingURLsFollowRoutesWithoutANavigateHandler() throws {
        let delegate = Delegate()
        let session = ExactApp.shared.makeSession(delegate: delegate, label: "route-follow")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try fixturePlan("PUSHED_TITLE"), size: CGSize(width: 390, height: 844)).error)
        XCTAssertNil(session.navigationRoot)
        func hasRoute(_ name: String) -> Bool {
            session.presenter.views.values.contains { $0.props["testId"] == "route-" + name }
        }
        XCTAssertTrue(session.follow("/short"))
        XCTAssertTrue(hasRoute("short"))
        XCTAssertTrue(session.follow("/short"), "following the selected route is accepted")
        XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: "exacttest://inline?x=1#part"))))
        XCTAssertTrue(hasRoute("inline"))
        XCTAssertTrue(session.follow("/assets/readme.md"))
        XCTAssertEqual(delegate.opened, ["/assets/readme.md"], "a non-route is the containing app's to open")
        XCTAssertFalse(session.navigate("/missing"))
    }
}
