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

    /// The field wire the UIKit and AppKit fields sent before it moved to ExactCore,
    /// byte for byte (`runner/src/runner/field.rs` reads it), and the watch's replacement.
    func testTheFieldWireIsTheFieldsPayloads() {
        XCTAssertEqual(FieldWire.selection(start: 1, end: 3, direction: "forward", value: "a,b"), "1,3,forward,a,b")
        XCTAssertEqual(FieldWire.input(type: "insertText", data: "é", value: "xé", start: 2, end: 2),
                       "insertText,1,0,2,é2,2,none,xé")
        XCTAssertEqual(FieldWire.input(type: "deleteContentBackward", data: nil, cancelable: false, composing: true, value: "", start: 0, end: 0),
                       "deleteContentBackward,0,1,-1,0,0,none,")
        XCTAssertEqual(FieldWire.replacement("Palo 😀"), "insertReplacementText,0,0,9,Palo 😀7,7,none,Palo 😀")
    }

    /// A native control's title is read as the turn applies it, so a view's body
    /// reads a value, not the runtime.
    func testANativeControlCarriesItsTitle() throws {
        let s = try session("BUTTONS", live: false)
        defer { s.close() }
        let controls = try s.snapshot().nodes.values.filter { $0.kind == "control" }
        XCTAssertFalse(controls.isEmpty)
        XCTAssertTrue(controls.contains { !($0.controlTitle ?? "").isEmpty })
    }

    /// A native button's text is the control's, which no op carries: its title
    /// follows the text when the button's box stays the same.
    func testANativeControlsTitleFollowsItsText() throws {
        let s = try session("HEADLESS_LIVE", live: false)
        defer { s.close() }
        let flip = try node("flip", in: s.snapshot())
        XCTAssertEqual(flip.controlTitle, "Play")
        let pressed = try s.press(flip.id, at: 0)
        XCTAssertEqual(try node("flip", in: pressed).frame, flip.frame)
        XCTAssertEqual(try node("flip", in: pressed).controlTitle, "Stop")
    }

    /// A link to one of the app's routes goes there when no root hears `navigate`.
    func testALinkToARouteGoesThereWithNoNavigateHandler() throws {
        let s = try session("HEADLESS_BACK", live: false)
        defer { s.close() }
        let home = try XCTUnwrap(node("root", in: s.snapshot()).props["navigationKey"])
        XCTAssertNil(try s.follow("https://example.com/", at: 0), "not a route: the host's to open")
        let followed = try XCTUnwrap(s.follow("/detail", at: 0))
        XCTAssertNotEqual(try node("root", in: followed).props["navigationKey"], home)
        XCTAssertNoThrow(try node("detail-back", in: followed))
    }

    /// The keys of the stack, base first, after pressing each of `opens`.
    private func stack(_ s: HeadlessSession, _ opens: [String]) throws -> [String] {
        var keys = [try XCTUnwrap(node("root", in: s.snapshot()).props["navigationKey"])]
        for open in opens {
            try s.press(node(open, in: s.snapshot()).id, at: 0)
            keys.append(try XCTUnwrap(node("root", in: s.snapshot()).props["navigationKey"]))
        }
        return keys
    }

    /// A pop to the base stops at a route whose own Back keeps it there: the
    /// route beneath's Back is not pressed.
    func testAPopOfSeveralLevelsStopsWhereARouteStays() throws {
        let s = try session("HEADLESS_BACK", live: false)
        defer { s.close() }
        let keys = try stack(s, ["open", "open-more"])
        let root = try node("root", in: s.snapshot()).id
        let popped = try XCTUnwrap(s.back(root: root, routes: [keys[2], keys[1]], at: 0))
        XCTAssertEqual(try text("pressed", in: popped), "more")
        XCTAssertEqual(popped.nodes[root]?.props["navigationKey"], keys[2])
    }

    /// A pop to the base takes each level: the top by the router's own back, the
    /// next by its own control.
    func testAPopOfSeveralLevelsTakesEachLevel() throws {
        let s = try session("HEADLESS_BACK", live: false)
        defer { s.close() }
        let keys = try stack(s, ["open", "open-more-going"])
        let root = try node("root", in: s.snapshot()).id
        let popped = try XCTUnwrap(s.back(root: root, routes: [keys[2], keys[1]], at: 0))
        XCTAssertEqual(try text("pressed", in: popped), "detail")
        XCTAssertEqual(popped.nodes[root]?.props["navigationKey"], keys[0])
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
