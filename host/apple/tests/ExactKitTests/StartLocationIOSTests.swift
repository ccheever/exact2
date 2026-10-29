#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A launch no link started opens at the manifest's `start_url`
/// (`ExactSession.startLocation`), and a session already booted is not sent
/// there again (the dev menu's restart boots where the launch opened).
final class StartLocationIOSTests: XCTestCase {
    func testALaunchOpensAtTheStartLocationAndABootedSessionStaysWhereItIs() throws {
        let was = ExactSession.startLocation
        ExactSession.startLocation = "/detail"
        defer { ExactSession.startLocation = was }
        let session = ExactApp.shared.makeSession(label: "start-location")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try fixturePlan("HEADLESS_BACK"), size: CGSize(width: 390, height: 844)).error)
        let top = { session.presenter.views.values.first { $0.props["testId"] == "root" }?.props["navigationKey"] }
        XCTAssertTrue(session.presenter.views.values.contains { $0.props["testId"] == "page-detail" }, "the launch opened at the start location")
        XCTAssertTrue(session.navigate("/more"))
        let more = try XCTUnwrap(top())
        session.openStartLocation()
        XCTAssertEqual(top(), more, "a booted session stays where it is")
    }
}
#endif
