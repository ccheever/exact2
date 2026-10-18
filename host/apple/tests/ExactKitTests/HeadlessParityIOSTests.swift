#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class HeadlessParityIOSTests: XCTestCase {
    func testHeadlessAndPresentedLayoutsUseTheSamePageFacts() throws {
        let plan = try fixturePlan("HEADLESS"), size = CGSize(width: 200, height: 400)
        let presented = ExactApp.shared.makeSession(label: "headless-parity")
        defer { presented.destroy() }
        presented.presenter.viewport.frame = CGRect(origin: .zero, size: size)
        let date = presented.dateNow()
        let facts = SessionFacts(displayScale: PageFacts.displayScale(presented.presenter.viewport),
            preferences: DisplayPreferences.bits(systemDark: presented.presenter.viewport.traitCollection.userInterfaceStyle == .dark, view: presented.presenter.viewport), rootFontSize: PageFacts.rootFontSize,
            date: Date(timeIntervalSince1970: date.epoch / 1000),
            timeZone: try XCTUnwrap(TimeZone(identifier: presented.launchPlace.timeZone)),
            locale: presented.launchPlace.locale, seed: presented.launchPlace.seed)
        XCTAssertEqual(facts.rootFontSize, 17, "the iPhone's default body size")
        let headless = try HeadlessSession(plan: plan, size: size, facts: facts)
        defer { headless.close() }
        XCTAssertNil(presented.boot(plan: plan, size: size).error)
        let snapshot = try headless.snapshot()
        for name in ["box", "label", "padded", "body", "hairline"] {
            let node = try XCTUnwrap(snapshot.nodes.values.first { $0.props["testId"] == name })
            let view = try XCTUnwrap(presented.presenter.views.values.first { $0.props["testId"] == name })
            // The kernel uses Float; UIKit snaps a hairline to Double(1 / scale).
            for (laidOut, presented) in zip([node.frame.minX, node.frame.minY, node.frame.width, node.frame.height],
                                            [view.frame.minX, view.frame.minY, view.frame.width, view.frame.height]) {
                XCTAssertEqual(laidOut, presented, accuracy: 0.000001, name)
            }
            XCTAssertEqual(node.contentBox, view.contentBox(), name)
            if node.kind == "text" {
                let paragraph = try XCTUnwrap(view.paragraphLayout())
                XCTAssertEqual(node.lines.map(\.origin.y), paragraph.baselines, name)
                XCTAssertEqual(node.lines.count, paragraph.lines.count, name)
            }
        }
        let body = try XCTUnwrap(snapshot.nodes.values.first { $0.props["testId"] == "body" })
        XCTAssertEqual(body.style["font_size"]?.number, 17)
        XCTAssertEqual(body.frame.width, 170)
    }
}
#endif
