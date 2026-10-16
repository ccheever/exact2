#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A painted popover the agent lifts into the top layer (LLP 1021 D4) is out
/// of the page whose recognizers would hear its touches: its layer relays a
/// touch's pointer events, long press and double tap to the popover's
/// authored ancestors (`MountRelay`), and the agent's walks reach them too.
final class PopoverRelayIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func page() -> ExactSession {
        let session = ExactApp.shared.makeSession(label: "popover-relay")
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
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointerup", "dblclick", "contextmenu"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "choices", "popover": "auto"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["pan"]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 20.0, "y": 100.0, "w": 200.0, "h": 160.0],
            ["op": "frame", "id": 3, "x": 20.0, "y": 20.0, "w": 160.0, "h": 40.0],
        ]))
        p.viewport.layoutIfNeeded()
        // showPopover enters agentShow's painted presentation without changing process-wide agent mode.
        p.menus.action("showPopover", ["choices"])
        return session
    }

    private func observer(_ popover: NodeView) throws -> PointerRecognizer {
        try XCTUnwrap(popover.superview?.gestureRecognizers?.compactMap { $0 as? PointerRecognizer }.first,
                      "the top layer observes touches for the popover's authored ancestors")
    }

    /// The relay gives a touch to the nearest listener outside the popover,
    /// unless one in it is nearer; a press stays in the top layer.
    func testATouchInThePopoverReachesItsAuthoredAncestorsUnlessANearerNodeListens() throws {
        let p = page().presenter
        let root = try XCTUnwrap(p.views[1]), popover = try XCTUnwrap(p.views[2]), row = try XCTUnwrap(p.views[3])
        let relay = try XCTUnwrap(try observer(popover).relay)
        XCTAssertFalse(row.isDescendant(of: root), "the popover left its authored parent")
        XCTAssertTrue(row.isDescendant(of: relay.mount))
        XCTAssertTrue(p.pointerParent(of: popover) === root)
        XCTAssertTrue(p.touchParent(of: popover) === relay.mount, "press bubbling stays in the top layer")
        XCTAssertTrue(relay.listener(row) { $0.wantsPointer } === root)
        XCTAssertTrue(relay.listener(row) { $0.handlers.contains("dblclick") } === root)
        root.props["disabled"] = "true"
        XCTAssertTrue(relay.listener(row, contextMenu: true) { $0.handlers.contains("contextmenu") } === root)
        XCTAssertTrue(row.contextMenuSource === root, "disabled on the ancestor container leaves its menu available")
        root.props.removeValue(forKey: "disabled")

        popover.handlers = ["pointerdown", "dblclick", "contextmenu"]
        XCTAssertNil(relay.listener(row) { $0.wantsPointer }, "the popover's own observer sends the bubbling pointer events")
        XCTAssertNil(relay.listener(row) { $0.handlers.contains("dblclick") })
        XCTAssertNil(relay.listener(row, contextMenu: true) { $0.handlers.contains("contextmenu") })
        XCTAssertTrue(row.contextMenuSource === popover)
    }

    /// The agent's `dblclick`, `contextmenu` and pan reach what a finger does.
    func testTheAgentsTapAndPanReachTheSameListeners() throws {
        let session = page(), p = session.presenter, agent = Agent(session: session)
        let root = try XCTUnwrap(p.views[1]), popover = try XCTUnwrap(p.views[2])
        var doubled: [UInt32] = [], contexts: [UInt32] = [], pointers: [String] = []
        p.onDblclick = { doubled.append($0) }
        p.onContextmenu = { contexts.append($0) }
        p.onPointer = { id, kind, _ in pointers.append("\(id) \(kind == .down ? "down" : "up")") }
        p.onPanSample = nil
        for target in [root, popover] {
            if target === popover { popover.handlers = ["pointerdown", "pointerup", "dblclick", "contextmenu"] }
            for event in ["dblclick", "contextmenu"] {
                target.props["disabled"] = event == "contextmenu" ? "true" : nil
                let reply = agent.tap(["id": 3, event: true])
                XCTAssertNil(reply["error"], "\(reply)")
                XCTAssertEqual(reply["tapped"] as? Int, Int(target.id), "\(reply)")
            }
            target.props.removeValue(forKey: "disabled")
            let down = try XCTUnwrap(agent.recognizedPan("down", ["id": 3]))
            XCTAssertNil(down["error"], "\(down)")
            XCTAssertTrue(agent.panContact?.pointer?.node === target)
            _ = agent.recognizedPan("cancel", [:])
        }
        XCTAssertEqual(doubled, [1, 2])
        XCTAssertEqual(contexts, [1, 2])
        // The popover's down and up bubble to its authored parent, as a finger's do (`pointerParent`).
        XCTAssertEqual(pointers, ["1 down", "1 up", "2 down", "1 down", "2 up", "1 up"])
    }

    /// One relay per open popover, gone when it closes or the menus reset.
    func testTheRelayStaysWithItsEntryAndLeavesOnDismissalAndReset() throws {
        let p = page().presenter
        let root = try XCTUnwrap(p.views[1]), popover = try XCTUnwrap(p.views[2]), row = try XCTUnwrap(p.views[3])
        let layer = try XCTUnwrap(popover.superview), pointer = try observer(popover)
        XCTAssertNotNil(pointer.relay)
        XCTAssertFalse(pointer.cancelsTouchesInView)
        XCTAssertFalse(pointer.delaysTouchesBegan)
        XCTAssertFalse(pointer.delaysTouchesEnded)
        XCTAssertNil(layer.hitTest(.zero, with: nil), "the layer takes no touch outside its content")
        p.menus.sync()
        XCTAssertTrue(try observer(popover) === pointer, "sync keeps the entry's observer")
        XCTAssertEqual(layer.gestureRecognizers?.filter { $0 is PointerRecognizer }.count, 1)
        p.menus.lightDismiss(row)
        XCTAssertTrue(p.menus.isOpen(popover), "a touch inside keeps the popover")
        p.menus.lightDismiss(root)
        XCTAssertFalse(p.menus.isOpen(popover))
        XCTAssertNil(layer.superview)
        XCTAssertNil(pointer.relay, "dropping the entry releases its relay")
        XCTAssertTrue(popover.superview === root)

        p.menus.action("showPopover", ["choices"])
        let reopened = try observer(popover)
        XCTAssertFalse(reopened === pointer)
        XCTAssertNotNil(reopened.relay)
        p.menus.reset()
        XCTAssertNil(reopened.relay, "reset drops each entry too")
        XCTAssertTrue(popover.superview === root)
    }
}
#endif
