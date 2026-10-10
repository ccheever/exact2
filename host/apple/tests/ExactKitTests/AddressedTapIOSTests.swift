#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// A tap addressed by id presses what it names (LLP 1012 §1 `tap`): a row
/// with a press of its own is pressed itself, beside the link card its middle
/// holds, never the card; a box without a press never presses a control
/// inside it; `at` keeps a point's semantics (the Bluesky clone's likes and
/// reposts on real people's posts, 2026-10-09).
final class AddressedTapIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func rows() -> (ExactSession, Agent, () -> [UInt32]) {
        let session = ExactApp.shared.makeSession(label: "addressed-tap")
        self.session = session
        let p = session.presenter
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        p.apply(wireBatch([
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
        ]))
        p.viewport.layoutIfNeeded()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        return (session, Agent(session: session), { defer { pressed = [] }; return pressed })
    }

    func testARowWithItsOwnPressIsPressedBesideTheCardAtItsMiddle() throws {
        let (_, agent, presses) = rows()
        let reply = agent.tap(["id": 2])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["pressed"] as? Int, 2, "the row, not the card its middle holds: \(reply)")
        XCTAssertEqual(presses(), [2])
        let avoided = try XCTUnwrap(reply["avoided"] as? [String: Any], "the reply says why it landed off the middle: \(reply)")
        XCTAssertEqual(avoided["pressing"] as? Int, 3)
        let at = try XCTUnwrap(reply["at"] as? [Double])
        XCTAssertFalse(CGRect(x: 100, y: 40, width: 200, height: 80).contains(CGPoint(x: at[0], y: at[1])), "outside the card: \(at)")
        // The card, named, is the card; a point keeps the point's semantics.
        XCTAssertEqual(agent.tap(["id": 3])["pressed"] as? Int, 3)
        XCTAssertEqual(presses(), [3])
        let point = agent.tap(["id": 2, "at": [200.0, 80.0]])
        XCTAssertEqual(point["pressed"] as? Int, 3, "at the row's middle, a finger reaches the card: \(point)")
        XCTAssertNil(point["avoided"])
        XCTAssertEqual(presses(), [3])
        XCTAssertNotNil(agent.tap(["id": 2, "at": [420.0, 80.0]])["error"], "a point outside the box is refused")
        XCTAssertEqual(presses(), [])
    }

    func testABoxWithoutAPressNeverPressesTheControlInsideIt() throws {
        let (_, agent, presses) = rows()
        let reply = agent.tap(["id": 4])
        let error = try XCTUnwrap(reply["error"] as? String, "refused: \(reply)")
        XCTAssertTrue(error.contains("would press #5 inside it"), error)
        XCTAssertEqual(reply["pressing"] as? Int, 5)
        XCTAssertEqual(reply["addressed"] as? Int, 4)
        XCTAssertEqual(presses(), [], "nothing is pressed")
        XCTAssertEqual(agent.tap(["id": 4, "at": [200.0, 50.0]])["pressed"] as? Int, 5, "a point: what is there")
        XCTAssertEqual(presses(), [5])
    }

    func testARowNoPointOfWhichReachesItIsRefused() throws {
        let (_, agent, presses) = rows()
        let reply = agent.tap(["id": 6])
        let error = try XCTUnwrap(reply["error"] as? String, "refused: \(reply)")
        XCTAssertTrue(error.contains("would press #7 inside it") && error.contains("no point of #6"), error)
        XCTAssertEqual(presses(), [])
    }

    func testADisabledRowPressesNothingNotTheButtonAtItsMiddle() throws {
        let (_, agent, presses) = rows()
        let reply = agent.tap(["id": 8])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertTrue(reply["pressed"] is NSNull, "a disabled row's tap presses nothing: \(reply)")
        XCTAssertEqual(presses(), [])
    }

    /// A real touch's aim (`--touch platform`) takes the same rule.
    func testTheTouchRunnersAimTakesTheSameRule() throws {
        let (_, agent, _) = rows()
        let plain = agent.aim(["id": 2, "aim": ["press": true]])
        if (plain["error"] as? String)?.contains("foreground") == true { return } // a test host in the background aims nowhere
        let aim = try XCTUnwrap(plain["aim"] as? [String: Any], "\(plain)")
        let at = try XCTUnwrap(aim["at"] as? [Double])
        XCTAssertFalse(CGRect(x: 100, y: 40, width: 200, height: 80).contains(CGPoint(x: at[0], y: at[1])), "beside the card: \(at)")
        XCTAssertEqual(aim["hit"] as? Int, 2)
        XCTAssertEqual((aim["avoided"] as? [String: Any])?["pressing"] as? Int, 3)
        XCTAssertEqual((agent.aim(["id": 4, "aim": ["press": true]])["pressing"]) as? Int, 5, "refused, naming Like")
        XCTAssertEqual((agent.aim(["id": 2, "aim": ["at": [200.0, 80.0]]])["aim"] as? [String: Any])?["hit"] as? Int, 3, "a point: the card")
        XCTAssertEqual((agent.aim(["id": 2, "aim": true])["aim"] as? [String: Any])?["hit"] as? Int, 3, "a drag's aim is the middle still")
    }
}
#endif
