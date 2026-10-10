#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// A click addressed by id presses what it names (LLP 1012 §1 `tap`), as on
/// iOS (`AddressedTapIOSTests`): the real click lands beside the card a
/// row's middle holds; a box without a press never presses the control inside
/// it; `at` keeps a point's semantics.
///
/// The iOS test's rows: a post with a card at its middle, a box with Like at
/// its middle, a row its child covers, a disabled row with Repost at its middle.
private let FIXTURE: [[String: Any]] = [
    ["op": "create", "id": 1, "kind": "view"],
    // A post that opens its thread, a link card at its middle.
    ["op": "create", "id": 2, "kind": "view", "handlers": ["press"], "props": ["testId": "post-0"]],
    ["op": "create", "id": 3, "kind": "view", "handlers": ["press"], "props": ["testId": "card-0"]],
    // A box with no press of its own, Like at its middle.
    ["op": "create", "id": 4, "kind": "view", "props": ["testId": "wrap-0"]],
    ["op": "create", "id": 5, "kind": "button", "handlers": ["press"], "props": ["testId": "like-0"]],
    // A row whose child covers it: no point of it presses it.
    ["op": "create", "id": 6, "kind": "view", "handlers": ["press"], "props": ["testId": "post-1"]],
    ["op": "create", "id": 7, "kind": "view", "handlers": ["press"], "props": ["testId": "cover-1"]],
    // A disabled row: a press of its own that does nothing, Repost at its middle.
    ["op": "create", "id": 8, "kind": "view", "handlers": ["press"], "props": ["testId": "post-2", "disabled": "true"]],
    ["op": "create", "id": 9, "kind": "button", "handlers": ["press"], "props": ["testId": "repost-2"]],
    ["op": "children", "id": 1, "ids": [2, 4, 6, 8]],
    ["op": "children", "id": 2, "ids": [3]],
    ["op": "children", "id": 4, "ids": [5]],
    ["op": "children", "id": 6, "ids": [7]],
    ["op": "children", "id": 8, "ids": [9]],
    ["op": "roots", "ids": [1]],
    ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
    ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 160.0],
    ["op": "frame", "id": 3, "x": 100.0, "y": 40.0, "w": 200.0, "h": 80.0],
    ["op": "frame", "id": 4, "x": 0.0, "y": 200.0, "w": 400.0, "h": 100.0],
    ["op": "frame", "id": 5, "x": 150.0, "y": 25.0, "w": 100.0, "h": 50.0],
    ["op": "frame", "id": 6, "x": 0.0, "y": 340.0, "w": 400.0, "h": 100.0],
    ["op": "frame", "id": 7, "x": 0.0, "y": 0.0, "w": 400.0, "h": 100.0],
    ["op": "frame", "id": 8, "x": 0.0, "y": 480.0, "w": 400.0, "h": 100.0],
    ["op": "frame", "id": 9, "x": 150.0, "y": 25.0, "w": 100.0, "h": 50.0],
]

@MainActor
final class AddressedTapMacTests: XCTestCase {
    private var window: NSWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.close(); window = nil; super.tearDown() }

    private func rows() -> (Agent, () -> [UInt32]) {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "addressed-tap-mac")
        self.session = session
        let p = session.presenter
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 800)
        let window = NSWindow(contentRect: NSRect(x: 240, y: 60, width: 400, height: 800), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        window.orderFrontRegardless()
        self.window = window
        p.apply(wireBatch(FIXTURE))
        p.viewport.layoutSubtreeIfNeeded()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        return (Agent(session: session), { defer { pressed = [] }; return pressed })
    }

    func testAClickOnARowLandsBesideTheCardAndABoxWithoutAPressIsRefused() throws {
        let (agent, presses) = rows()
        let reply = agent.tap(["id": 2])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(presses(), [2], "the row, not the card its middle holds: \(reply)")
        XCTAssertEqual((reply["avoided"] as? [String: Any])?["pressing"] as? Int, 3, "\(reply)")
        XCTAssertEqual(agent.tap(["id": 2, "at": [200.0, 80.0]])["error"] as? String, nil)
        XCTAssertEqual(presses(), [3], "a point: the card a click there reaches")
        let refused = agent.tap(["id": 4])
        XCTAssertTrue((refused["error"] as? String)?.contains("would press #5 inside it") == true, "\(refused)")
        XCTAssertEqual(refused["pressing"] as? Int, 5)
        let covered = agent.tap(["id": 6])
        XCTAssertTrue((covered["error"] as? String)?.contains("no point of #6") == true, "\(covered)")
        XCTAssertEqual(presses(), [], "nothing is pressed by a refusal")
        XCTAssertNil(agent.tap(["id": 8])["error"])
        XCTAssertEqual(presses(), [], "a disabled row's click presses nothing, not the button at its middle")
        // A 2-point strip around a child that covers the rest is found (Astra, round 1).
        session?.presenter.apply(wireBatch([
            ["op": "create", "id": 22, "kind": "view", "handlers": ["press"], "props": ["testId": "ring"]],
            ["op": "create", "id": 23, "kind": "view", "handlers": ["press"], "props": ["testId": "ring-core"]],
            ["op": "children", "id": 1, "ids": [2, 4, 6, 8, 22]], ["op": "children", "id": 22, "ids": [23]],
            ["op": "frame", "id": 22, "x": 0.0, "y": 600.0, "w": 100.0, "h": 100.0],
            ["op": "frame", "id": 23, "x": 2.0, "y": 2.0, "w": 96.0, "h": 96.0],
        ]))
        session?.presenter.viewport.layoutSubtreeIfNeeded()
        let ring = agent.tap(["id": 22])
        XCTAssertNil(ring["error"], "\(ring)")
        XCTAssertEqual(presses(), [22])
        // An inline run's `at` is from its own fragment, not its paragraph's
        // top left, where another run is (round 2).
        session?.presenter.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "text", "style": ["font_size": 16, "line_height": "20px"]],
            ["op": "paragraph", "id": 30, "runs": [
                ["id": 31, "parent": 30, "paint": true, "props": ["text": "first link "], "style": [:], "handlers": ["press"]],
                ["id": 32, "parent": 30, "paint": true, "props": ["text": "second link"], "style": [:], "handlers": ["press"]],
            ]],
            ["op": "children", "id": 1, "ids": [2, 4, 6, 8, 22, 30]],
            ["op": "frame", "id": 30, "x": 120.0, "y": 600.0, "w": 280.0, "h": 40.0],
        ]))
        session?.presenter.viewport.layoutSubtreeIfNeeded()
        let wire = try JSONSerialization.jsonObject(with: JSONSerialization.data(withJSONObject: ["id": 32, "at": [2, 2]])) as! [String: Any]
        let second = agent.tap(wire)
        XCTAssertNil(second["error"], "\(second)")
        XCTAssertEqual(presses(), [32], "the second run, at a point in it: \(second)")
    }
}
#endif
