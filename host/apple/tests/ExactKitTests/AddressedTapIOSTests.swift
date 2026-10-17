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

    /// A field and a checkbox take a touch themselves (UIKit's controls), so
    /// the agent's walks press no ancestor from them, as a real touch does
    /// not; text beside them still presses it, and a native button's action
    /// still presses the nearest handler.
    func testAFieldOrAControlTakesTheTapAndPressesNoAncestor() throws {
        let (session, agent, presses) = rows(), p = session.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 21, "kind": "input", "props": ["value": "draft"]],
            ["op": "create", "id": 22, "kind": "control", "props": ["type": "checkbox"]],
            ["op": "create", "id": 23, "kind": "control", "props": ["type": "button"]],
            ["op": "create", "id": 24, "kind": "textarea", "props": ["value": "notes"]],
            ["op": "create", "id": 25, "kind": "text", "props": ["text": "Beside the controls"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [21, 22, 23, 24, 25]],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 400, "h": 500],
        ] + (21...25).map { ["op": "frame", "id": $0, "x": 20, "y": ($0 - 21) * 60 + 20, "w": 240, "h": 40] }))
        let ancestor = try XCTUnwrap(p.views[2]), win = try XCTUnwrap(window)
        func point(_ node: NodeView) -> CGPoint { node.convert(CGPoint(x: node.bounds.midX, y: node.bounds.midY), to: nil) }
        for id: UInt32 in [21, 22, 24] {
            let node = try XCTUnwrap(p.views[id])
            XCTAssertNil(node.activationTarget(at: point(node)), "\(id)")
            XCTAssertNil(node.activate(at: point(node)), "\(id)")
            XCTAssertNil(agent.fingerReach(node, at: point(node)).action, "\(id)")
            if case .control(let owner)? = agent.pressReach(node, at: point(node), in: win) {
                XCTAssertTrue(owner === node, "\(id)")
            } else { XCTFail("the addressed walk stops at \(id)") }
        }
        XCTAssertEqual(presses(), [], "no ancestor press from a field or a control")
        let plain = try XCTUnwrap(p.views[25]), button = try XCTUnwrap(p.views[23])
        XCTAssertTrue(plain.activate(at: point(plain)) === ancestor)
        XCTAssertTrue(button.activationTarget(at: point(button)) === ancestor, "a native button's action presses the nearest handler")
        XCTAssertEqual(presses(), [2])
    }

    func testACheckboxPaddingPressesItsAncestorAsARealTouchDoes() throws {
        let (session, agent, presses) = rows(), p = session.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 21, "kind": "control", "props": ["type": "checkbox"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [21]],
            ["op": "frame", "id": 21, "x": 20, "y": 20, "w": 240, "h": 40],
        ]))
        p.viewport.layoutIfNeeded()
        let node = try XCTUnwrap(p.views[21]), ancestor = try XCTUnwrap(p.views[2]), win = try XCTUnwrap(window)
        let padding = CGPoint(x: 10, y: node.bounds.midY), point = node.convert(padding, to: nil)
        XCTAssertTrue(win.hitTest(point, with: nil) === node, "the padding lands on the node, outside the centred checkbox's hit area")
        XCTAssertTrue(node.activationTarget(at: point) === ancestor)
        XCTAssertTrue(node.activate(at: point) === ancestor)
        XCTAssertEqual(presses(), [2])
        let reach = agent.fingerReach(node, at: point)
        XCTAssertTrue(reach.action?.node === ancestor)
        XCTAssertTrue(agent.stillPresses(reach) === ancestor)
        if case .node(let target)? = agent.pressReach(node, at: point, in: win) {
            XCTAssertTrue(target === ancestor)
        } else { XCTFail("the addressed tap on padding must reach the ancestor") }
        XCTAssertEqual(agent.lift(reach, at: point, held: "")["pressed"] as? Int, 2)
        XCTAssertEqual(presses(), [2])
        let touch = PaddingTouch(node, at: padding)
        node.touchesBegan([touch], with: nil)
        XCTAssertTrue(ancestor.pressed)
        node.touchesEnded([touch], with: nil)
        XCTAssertFalse(ancestor.pressed)
        XCTAssertEqual(presses(), [2], "the real touch and the agent press the same ancestor")

        let centre = node.convert(CGPoint(x: node.bounds.midX, y: node.bounds.midY), to: nil)
        let control = try XCTUnwrap(p.controls.controls[21])
        XCTAssertTrue(win.hitTest(centre, with: nil) === control)
        XCTAssertNil(node.activationTarget(at: centre))
        XCTAssertNil(node.activate(at: centre))
        let centreReach = agent.fingerReach(control, at: centre)
        XCTAssertNil(centreReach.action)
        if case .control(let owner)? = agent.pressReach(node, at: centre, in: win) {
            XCTAssertTrue(owner === node)
        } else { XCTFail("the centre belongs to the native checkbox") }
        XCTAssertTrue(agent.lift(centreReach, at: centre, held: "")["pressed"] is NSNull)
        XCTAssertEqual(presses(), [])

        let paddingReply = agent.tap(["id": 21, "at": [10.0, 20.0]])
        XCTAssertNil(paddingReply["error"], "\(paddingReply)")
        XCTAssertEqual(paddingReply["pressed"] as? Int, 2)
        XCTAssertEqual(presses(), [2])
        XCTAssertNil(paddingReply["native"], "a tap on padding is no host activation of the checkbox")
        let viewportPoint = p.client(point)
        let viewportReply = agent.tap(["id": 21, "x": Double(viewportPoint.x), "y": Double(viewportPoint.y)])
        XCTAssertNil(viewportReply["error"], "\(viewportReply)")
        XCTAssertEqual(viewportReply["pressed"] as? Int, 2)
        XCTAssertEqual(presses(), [2])
        XCTAssertNil(viewportReply["native"], "viewport coordinates on padding must use the same point as native hit testing")
        for request: [String: Any] in [["id": 21, "at": [120.0, 20.0]], ["id": 21]] {
            let reply = agent.tap(request)
            XCTAssertNil(reply["error"], "\(reply)")
            XCTAssertEqual(reply["native"] as? String, "control", "an explicit centre and a tap without a point are the checkbox's own")
            XCTAssertEqual(presses(), [])
        }
    }

    private final class PaddingTouch: UITouch {
        let target: UIView, point: CGPoint
        init(_ target: UIView, at point: CGPoint) { self.target = target; self.point = point; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { target.convert(point, to: view) }
    }

    func testAControlWithItsOwnPressTakesItBeforeConsumingTheTap() throws {
        let (session, agent, presses) = rows(), p = session.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 21, "kind": "input", "props": ["value": "draft"], "handlers": ["press"]],
            ["op": "create", "id": 22, "kind": "control", "props": ["type": "checkbox"], "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [21, 22]],
            ["op": "frame", "id": 21, "x": 20, "y": 20, "w": 240, "h": 40],
            ["op": "frame", "id": 22, "x": 20, "y": 80, "w": 240, "h": 40],
        ]))
        let win = try XCTUnwrap(window)
        for id: UInt32 in [21, 22] {
            let node = try XCTUnwrap(p.views[id])
            let point = node.convert(CGPoint(x: node.bounds.midX, y: node.bounds.midY), to: nil)
            XCTAssertTrue(node.activationTarget(at: point) === node, "\(id)")
            XCTAssertTrue(node.activate(at: point) === node, "\(id)")
            XCTAssertEqual(presses(), [id])
            XCTAssertTrue(agent.stillPresses(FingerReach(reached: HeldNode(node), action: HeldNode(node))) === node)
            if case .node(let target)? = agent.pressReach(node, at: point, in: win) {
                XCTAssertTrue(target === node)
            } else { XCTFail("the control's own handler takes the tap: \(id)") }
            // The agent toggles a checkbox through its own action (host activation), as before.
            guard id == 21 else { continue }
            let reply = agent.tap(["id": Int(id)])
            XCTAssertNil(reply["error"], "\(reply)")
            XCTAssertEqual(reply["pressed"] as? Int, Int(id))
            XCTAssertEqual(presses(), [id], "the ancestor never presses")
        }
    }

    /// What a press there would deliver counts, a text editor's focus too;
    /// a 2-point strip around a child that covers the rest is found (Astra,
    /// round 1); a point on an inline run presses the run.
    func testAnEditorAThinStripAndAnInlineRunAtAPoint() throws {
        let (session, agent, presses) = rows()
        let p = session.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "handlers": ["press"], "props": ["testId": "note-row"]],
            ["op": "create", "id": 21, "kind": "textarea", "props": ["id": "note", "value": "hi"], "handlers": ["input"]],
            ["op": "create", "id": 22, "kind": "view", "handlers": ["press"], "props": ["testId": "ring"]],
            ["op": "create", "id": 23, "kind": "view", "handlers": ["press"], "props": ["testId": "ring-core"]],
            ["op": "create", "id": 24, "kind": "view", "handlers": ["press"], "props": ["testId": "para-row"]],
            ["op": "create", "id": 25, "kind": "text", "style": ["font_size": 16, "line_height": "20px"]],
            ["op": "paragraph", "id": 25, "runs": [
                ["id": 26, "parent": 25, "paint": true, "props": ["text": "Read "], "style": [:], "handlers": []],
                ["id": 27, "parent": 25, "paint": true, "props": ["text": "the guide"], "style": [:], "handlers": ["press"]],
            ]],
            ["op": "children", "id": 1, "ids": [2, 4, 6, 8, 20, 22, 24]],
            ["op": "children", "id": 20, "ids": [21]],
            ["op": "children", "id": 22, "ids": [23]],
            ["op": "children", "id": 24, "ids": [25]],
            ["op": "frame", "id": 20, "x": 0.0, "y": 600.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 21, "x": 100.0, "y": 10.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 22, "x": 0.0, "y": 680.0, "w": 100.0, "h": 100.0],
            ["op": "frame", "id": 23, "x": 2.0, "y": 2.0, "w": 96.0, "h": 96.0],
            ["op": "frame", "id": 24, "x": 120.0, "y": 680.0, "w": 280.0, "h": 40.0],
            ["op": "frame", "id": 25, "x": 0.0, "y": 0.0, "w": 280.0, "h": 40.0],
        ]))
        p.viewport.layoutIfNeeded()
        let note = agent.tap(["id": 20])
        XCTAssertNil(note["error"], "\(note)")
        XCTAssertEqual(presses(), [20], "the row beside its editor: \(note)")
        XCTAssertEqual((note["avoided"] as? [String: Any])?["pressing"] as? Int, 21, "\(note)")
        XCTAssertNotEqual(p.views[21]?.textArea?.isFirstResponder, true, "the editor did not take the focus")
        let ring = agent.tap(["id": 22])
        XCTAssertNil(ring["error"], "a 2-point strip reaches the ring: \(ring)")
        XCTAssertEqual(presses(), [22])
        let owner = try XCTUnwrap(p.views[25]), run = try XCTUnwrap(p.inlineText(27))
        let fragment = try XCTUnwrap(owner.inlineRects(run).first)
        let point = agent.tap(["id": 24, "at": [Double(fragment.midX), Double(fragment.midY)]])
        XCTAssertEqual(point["pressed"] as? Int, 27, "a point on the run presses the run: \(point)")
        XCTAssertEqual(presses(), [27])
    }

    /// A real touch's aim (`--touch platform`) takes the same rule.
    func testTheTouchRunnersAimTakesTheSameRule() throws {
        let (_, agent, _) = rows()
        let plain = agent.aim(["id": 2, "aim": ["press": true]])
        if (plain["error"] as? String)?.contains("foreground") == true { throw missing("foreground", "the test host is in the background: a real touch's aim refuses it") }
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
