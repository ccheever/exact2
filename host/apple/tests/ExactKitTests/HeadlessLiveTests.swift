import XCTest
import ExactCore

/// A running app's headless session (LLP 1126.002 D3), as the watch drives it.
final class HeadlessLiveTests: XCTestCase {
    private func node(_ testId: String, in snapshot: HeadlessSession.Snapshot) throws -> HeadlessSession.Node {
        try XCTUnwrap(snapshot.nodes.values.first { $0.props["testId"] == testId }, testId)
    }
    private func text(_ testId: String, in snapshot: HeadlessSession.Snapshot) throws -> String {
        try node(testId, in: snapshot).lines.map(\.text).joined()
    }

    /// Back from the root's active route, as the watch's stack asks for it.
    private func back(_ session: HeadlessSession) throws -> HeadlessSession.Snapshot? {
        let root = try node("root", in: session.snapshot())
        return try session.back(root: root.id, route: XCTUnwrap(root.props["navigationKey"]), at: 0)
    }

    private func session(_ key: String, live: Bool) throws -> HeadlessSession {
        let clock = live ? HeadlessSession.liveNow() : 0
        return try HeadlessSession(plan: fixturePlan(key), size: CGSize(width: 200, height: 400),
                                   facts: SessionFacts(displayScale: 2, preferences: [], clock: clock), live: live)
    }

    /// After twenty minutes wrist-down, the resume's one advance fires a 1 s
    /// ticker once, not once per missed beat.
    func testMissedBeatsFireOnceOnTheLiveClock() throws {
        let live = try session("HEADLESS_LIVE", live: true)
        defer { live.close() }
        let due = try XCTUnwrap(live.timerDue)
        XCTAssertEqual(try text("ticks", in: live.advance(at: due + 20 * 60_000)), "1")

        let seek = try session("HEADLESS_LIVE", live: false)
        defer { seek.close() }
        XCTAssertEqual(try text("ticks", in: seek.advance(at: 5_000)), "5", "the seek clock fires each beat")
    }

    /// Two routes each hold an `id="back"`: Back from the pushed one presses its
    /// own, not the base route's, whose view id is lower.
    func testBackPressesTheActiveRoutesOwnControl() throws {
        let s = try session("HEADLESS_BACK", live: false)
        defer { s.close() }
        try s.press(node("open", in: s.snapshot()).id)
        XCTAssertEqual(try text("pressed", in: XCTUnwrap(back(s))), "detail")
    }

    /// A pushed route with no Back of its own declares none: Back pops it and
    /// never presses the base route's control.
    func testBackFromARouteWithoutItsOwnControlPops() throws {
        let s = try session("HEADLESS_BACK", live: false)
        defer { s.close() }
        let home = try XCTUnwrap(node("root", in: s.snapshot()).props["navigationKey"])
        try s.press(node("open-plain", in: s.snapshot()).id)
        XCTAssertNotEqual(try node("root", in: s.snapshot()).props["navigationKey"], home)
        let popped = try XCTUnwrap(back(s))
        XCTAssertEqual(try text("pressed", in: popped), "none")
        XCTAssertEqual(try node("root", in: popped).props["navigationKey"], home)
    }
}
