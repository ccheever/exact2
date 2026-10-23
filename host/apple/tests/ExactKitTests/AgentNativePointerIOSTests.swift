#if os(iOS)
import UIKit
import XCTest
@testable import ExactGroupedLists
@testable import ExactKit

final class AgentNativePointerIOSTests: XCTestCase {
    private var session: ExactSession!
    private var window: UIWindow!

    override func tearDown() {
        session?.destroy(); session = nil
        window?.isHidden = true; window = nil
        super.tearDown()
    }

    private func mount(_ ops: [[String: Any]]) -> Agent {
        session = ExactApp.shared.makeSession(label: "native-pointer")
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.rootViewController = UIViewController()
        window.rootViewController!.view.addSubview(p.viewport)
        p.viewport.frame = window.bounds
        window.makeKeyAndVisible()
        p.apply(wireBatch(ops))
        window.layoutIfNeeded()
        return Agent(session: session)
    }

    private func nativeSwitch() -> Agent {
        mount([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup"]],
            ["op": "create", "id": 2, "kind": "control", "props": ["type": "checkbox", "accessibilityRole": "switch", "checked": "false"], "handlers": ["press", "change"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 40.0, "y": 60.0, "w": 60.0, "h": 44.0],
        ])
    }

    func testANativeSwitchBubblesPointersBeforeItsAction() throws {
        let agent = nativeSwitch(), p = session.presenter
        let control = try XCTUnwrap(p.controls.controls[2] as? UISwitch)
        var events: [String] = []
        p.onPointer = { id, kind, sample in
            events.append("\(id):\(kind == .down ? "down" : "up")")
            XCTAssertFalse(control.isOn, "pointer handlers precede the flip")
            XCTAssertEqual(sample.type, "touch")
            XCTAssertEqual(sample.buttons, kind == .down ? 1 : 0)
        }
        let reply = agent.tap(["id": 2])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["native"] as? String, "control")
        XCTAssertEqual(events, ["1:down", "1:up"])
        XCTAssertTrue(control.isOn)
    }

    func testPositionedControlTapsKeepTheirRequestedPointerCoordinates() throws {
        let agent = nativeSwitch(), p = session.presenter
        let control = try XCTUnwrap(p.controls.controls[2] as? UISwitch), node = try XCTUnwrap(p.views[2])
        let point = control.convert(CGPoint(x: 5, y: 6), to: nil)
        let local = node.convert(point, from: nil), client = p.client(point), box = agent.box(node)
        let requests: [([String: Any], CGPoint)] = [
            (["id": 2, "at": [Double(local.x), Double(local.y)]], client),
            (["id": 2, "x": Double(client.x), "y": Double(client.y)], client),
            (["id": 2, "x": Double(client.x)], CGPoint(x: client.x, y: box.midY)),
            (["id": 2, "y": Double(client.y)], CGPoint(x: box.midX, y: client.y)),
        ]
        var samples: [PointerSample] = []
        p.onPointer = { _, _, sample in samples.append(sample) }
        for (request, expected) in requests {
            samples = []
            let reply = agent.tap(request)
            XCTAssertNil(reply["error"], "\(reply)")
            XCTAssertEqual(reply["native"] as? String, "control")
            XCTAssertEqual(samples.count, 2)
            for sample in samples {
                XCTAssertEqual(sample.clientX, Double(expected.x), accuracy: 0.01, "\(request)")
                XCTAssertEqual(sample.clientY, Double(expected.y), accuracy: 0.01, "\(request)")
            }
        }
    }

    func testAPointerHandlerCanDisableANativeControlBeforeItActivates() throws {
        let agent = nativeSwitch(), p = session.presenter
        let control = try XCTUnwrap(p.controls.controls[2] as? UISwitch)
        var events: [String] = []
        p.onPointer = { _, kind, _ in
            events.append(kind == .down ? "down" : "up")
            if kind == .down {
                p.apply(wireBatch([["op": "props", "id": 2, "set": ["disabled": "true"]]]))
            }
        }
        p.onChecked = { _, _ in events.append("change") }
        _ = agent.tap(["id": 2])
        XCTAssertEqual(events, ["down", "up"])
        XCTAssertFalse(control.isOn, "a pointer handler disabled the switch before activation")
    }

    func testAPointerHandlerCanMakeAControlInertOrDetachItBeforeActivation() throws {
        for detach in [false, true] {
            let agent = nativeSwitch(), p = session.presenter
            let control = try XCTUnwrap(p.controls.controls[2] as? UISwitch)
            var downs = 0
            p.onPointer = { _, kind, _ in
                if kind == .down {
                    downs += 1
                    if detach { control.removeFromSuperview() }
                    else { p.apply(wireBatch([["op": "props", "id": 1, "set": ["inert": "true"]]])) }
                }
            }
            _ = agent.tap(["id": 2])
            XCTAssertEqual(downs, 1)
            XCTAssertTrue(control.isEnabled, "inertness and attachment are separate from UIControl.isEnabled")
            XCTAssertFalse(control.isOn, detach ? "the control left the window" : "the parent became inert")
            session.destroy(); session = nil
            window.isHidden = true; window = nil
        }
    }

    func testAnUnrevealedSwipeActionBubblesPointersBeforePress() throws {
        let agent = mount([
            ["op": "create", "id": 1, "kind": "view", "props": ["swipeContent": "body", "swipeTrailing": "delete"],
             "handlers": ["pointerdown", "pointerup"], "style": ["overflow_x": "scroll", "overflow_y": "hidden"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "props": ["id": "body"]],
            ["op": "create", "id": 4, "kind": "button", "props": ["id": "delete", "accessibilityLabel": "Delete"], "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3, 4]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 390.0, "h": 80.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 80.0],
            ["op": "frame", "id": 4, "x": 300.0, "y": 0.0, "w": 90.0, "h": 80.0],
            ["op": "content", "id": 1, "w": 390.0, "h": 80.0],
        ])
        let p = session.presenter
        var events: [String] = []
        p.onPointer = { id, kind, _ in events.append("\(id):\(kind == .down ? "down" : "up")") }
        p.onPress = { events.append("\($0):press") }
        let reply = agent.tap(["id": 4])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["native"] as? String, "swipe-action")
        XCTAssertEqual(events, ["1:down", "1:up", "4:press"])
    }

    func testASegmentsTapBubblesPointersBeforePress() throws {
        let agent = mount([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["accessibilityRole": "tablist"]],
            ["op": "create", "id": 3, "kind": "button", "props": ["accessibilityRole": "tab"], "handlers": ["press"]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "One"]],
            ["op": "create", "id": 5, "kind": "button", "props": ["accessibilityRole": "tab"], "handlers": ["press"]],
            ["op": "create", "id": 6, "kind": "text", "props": ["text": "Two"]],
            ["op": "children", "id": 5, "ids": [6]],
            ["op": "children", "id": 3, "ids": [4]], ["op": "children", "id": 2, "ids": [3, 5]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 20.0, "y": 60.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 200.0, "h": 40.0],
        ])
        let p = session.presenter
        XCTAssertNotNil(p.segments.control(of: 2))
        var events: [String] = []
        p.onPointer = { id, kind, _ in events.append("\(id):\(kind == .down ? "down" : "up")") }
        p.onPress = { events.append("\($0):press") }
        let reply = agent.tap(["id": 3])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(events, ["1:down", "1:up", "3:press"])
    }
}
final class AgentGroupedPointerIOSTests: GroupedListFixture {
    func testADisabledRowIsNotReportedAsChangedDuringPointerHandlers() throws {
        let session = ExactApp.shared.makeSession(label: "disabled-grouped-pointer")
        defer { session.destroy(); window?.isHidden = true }
        var model = self.model()
        model.sections[0].rows[0].disabled = true
        let p = presenter(on: session.presenter) { model }
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["disabled": "true"]],
                           ["op": "props", "id": 13, "set": ["disabled": "true"]]]))
        _ = try cell(p, 10); _ = try cell(p, 12)
        for (id, reason) in [(UInt32(10), "the row is disabled or not a button"), (UInt32(13), "the switch is disabled")] {
            var called = false
            let reply = host(p).activate(try XCTUnwrap(p.views[id]), before: { _ in called = true; return true })
            XCTAssertFalse(called, "an already disabled target runs no pointer handler")
            XCTAssertEqual(reply?["error"] as? String, "tap #\(id): \(reason)")
        }
    }

    func testAProjectedRowBubblesPointersBeforePress() throws {
        let session = ExactApp.shared.makeSession(label: "grouped-pointer")
        defer { session.destroy(); window?.isHidden = true }
        let p = presenter(handlers: ["pointerdown", "pointerup"], on: session.presenter) { self.model() }
        _ = try cell(p, 10)
        var events: [String] = []
        p.onPointer = { id, kind, _ in events.append("\(id):\(kind == .down ? "down" : "up")") }
        p.onPress = { events.append("\($0):press") }
        let reply = Agent(session: session).tap(["id": 10])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["native"] as? String, "grouped-list")
        XCTAssertEqual(events, ["1:down", "1:up", "10:press"])
    }
}
#endif
