import Foundation
import XCTest
#if os(macOS)
import AppKit
#endif
@testable import ExactKit

final class GenerationAcceptanceTests: XCTestCase {
    func testAChangedSessionRefusesTheWholeCompositionBeforeDurableCommit() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let app = ExactApp.shared, plan = try fixturePlan("HEADLESS")
        let sessions = [app.makeSession(label: "generation-a"), app.makeSession(label: "generation-b")]
        defer { for session in sessions { session.destroy() } }
        for session in sessions {
            session.presenter.viewport.frame = CGRect(x: 0, y: 0, width: 200, height: 400)
            XCTAssertNil(session.boot(size: CGSize(width: 200, height: 400)).error)
            XCTAssertTrue(session.apply(plan))
            XCTAssertNotNil(session.prepare(plan, resolver: app.resolver))
        }
        let session = sessions[0]
        let box = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "box" })
        session.apply(session.runtime.press(box.id, now: session.now()))
        func slots() throws -> [[String: String]] {
            try sessions.map { session in
                let data = Data(session.agent(#"{"op":"state"}"#).utf8)
                let state = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
                return try XCTUnwrap(state["slots"] as? [String: String])
            }
        }
        let before = try slots()
        XCTAssertEqual(before.map { $0["suffix"] }, ["delta epsilon zeta", "beta gamma"])
        var durableCommits = 0
        let refused = Runtime.commitTogether(sessions.map(\.runtime)) { durableCommits += 1; return true }
        XCTAssertNil(refused.batches)
        XCTAssertTrue(refused.pending)
        XCTAssertEqual(durableCommits, 0)
        XCTAssertEqual(try slots(), before)
        for session in sessions { session.runtime.discardPlan() }
        XCTAssertTrue(app.applyGeneration(ExactGeneration(plan: plan, assets: app.resolver)) {
            XCTAssertTrue(Thread.isMainThread)
            durableCommits += 1; return true
        })
        XCTAssertEqual(durableCommits, 1)
        XCTAssertEqual(try slots(), before)
    }
}
