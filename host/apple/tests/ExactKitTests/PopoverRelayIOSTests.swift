#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

/// A painted popover the agent lifts into the top layer (LLP 1021 D4) is out
/// of the page whose recognizers would hear its touches: its layer relays a
/// touch's pointer events, long press and double tap to the popover's
/// authored ancestors (`MountRelay`), and the agent's walks reach them too.
final class PopoverRelayIOSTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() }
    private final class SheetTouch: UITouch {
        let target: UIView
        var point: CGPoint
        init(_ target: UIView, at point: CGPoint) { self.target = target; self.point = point; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { target.convert(point, to: view) }
    }

    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func page(rowKind: String = "view") -> ExactSession {
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
            ["op": "create", "id": 3, "kind": rowKind, "props": rowKind == "text" ? ["text": "Plain content"] : [:], "handlers": ["pan"]],
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
    /// unless one in it is nearer; a press takes the same authored step.
    func testATouchInThePopoverReachesItsAuthoredAncestorsUnlessANearerNodeListens() throws {
        let p = page().presenter
        let root = try XCTUnwrap(p.views[1]), popover = try XCTUnwrap(p.views[2]), row = try XCTUnwrap(p.views[3])
        let relay = try XCTUnwrap(try observer(popover).relay)
        XCTAssertFalse(row.isDescendant(of: root), "the popover left its authored parent")
        XCTAssertTrue(row.isDescendant(of: relay.mount))
        XCTAssertTrue(p.pointerParent(of: popover) === root)
        XCTAssertTrue(p.listenerParent(of: popover) === root, "a press bubbles out to the authored parent")
        XCTAssertTrue(relay.listener(row) { $0.wantsPointer } === root)
        XCTAssertTrue(relay.listener(row) { $0.handlers.contains("dblclick") } === root)
        XCTAssertTrue(relay.listener(row) { $0.handlers.contains("contextmenu") } === root)
        XCTAssertTrue(row.contextMenuSource === root, "selectable text in it leaves a long press to the ancestor's menu")

        popover.handlers = ["pointerdown", "dblclick", "contextmenu"]
        XCTAssertNil(relay.listener(row) { $0.wantsPointer }, "the popover's own observer sends the bubbling pointer events")
        XCTAssertNil(relay.listener(row) { $0.handlers.contains("dblclick") })
        XCTAssertNil(relay.listener(row) { $0.handlers.contains("contextmenu") })
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
                let reply = agent.tap(["id": 3, event: true])
                XCTAssertNil(reply["error"], "\(reply)")
                XCTAssertEqual(reply["tapped"] as? Int, Int(target.id), "\(reply)")
            }
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

    func testAPressInThePopoverUsesItsContentBoxAndNearestAuthoredAncestor() throws {
        let session = page(rowKind: "text"), p = session.presenter, agent = Agent(session: session)
        let root = try XCTUnwrap(p.views[1]), popover = try XCTUnwrap(p.views[2]), row = try XCTUnwrap(p.views[3])
        root.handlers.formUnion(["press", "focus"])
        row.handlers = []
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"]],
            ["op": "children", "id": 4, "ids": [1]], ["op": "roots", "ids": [4]],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 400, "h": 800],
        ]))
        // The lifted content remains reachable when its authored parent's box is elsewhere.
        root.frame.size = CGSize(width: 10, height: 10)
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        let point = CGPoint(x: row.bounds.midX, y: row.bounds.midY)
        let touch = SheetTouch(row, at: point)
        XCTAssertTrue(row.activationTarget(at: touch.location(in: nil)) === root)
        row.touchesBegan([touch], with: nil)
        XCTAssertTrue(root.pressed)
        touch.point.x += 2
        row.touchesMoved([touch], with: nil)
        XCTAssertTrue(root.pressed)
        row.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [1], "only the nearest pressable ancestor hears it")
        XCTAssertFalse(root.pressed)
        XCTAssertFalse(root.isFirstResponder, "the focus stays in the popover")
        XCTAssertTrue(p.menus.isOpen(popover))

        touch.point = point
        row.touchesBegan([touch], with: nil)
        XCTAssertTrue(root.pressed)
        touch.point = CGPoint(x: row.bounds.maxX + 100, y: row.bounds.maxY + 100)
        row.touchesMoved([touch], with: nil)
        XCTAssertTrue(root.pressed)
        row.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [1], "a release outside the content cancels the press")

        touch.point = point
        row.handlers.insert("press")
        XCTAssertTrue(row.activationTarget(at: touch.location(in: nil)) === row)
        row.touchesBegan([touch], with: nil)
        XCTAssertTrue(row.pressed)
        XCTAssertFalse(root.pressed)
        row.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [1, 3])
        row.handlers = []

        // The agent resolves both its addressed tap and its captured lift by the same walk.
        let win = try XCTUnwrap(window)
        if case .node(let target)? = agent.pressReach(row, at: touch.location(in: nil), in: win) {
            XCTAssertTrue(target === root)
        } else { XCTFail("the agent's tap must reach the ancestor") }
        let reach = agent.fingerReach(row, at: touch.location(in: nil))
        XCTAssertTrue(agent.stillPresses(reach.action, from: reach.reached) === root)
        let down = try XCTUnwrap(agent.recognizedPan("down", ["id": 3]))
        XCTAssertNil(down["error"], "\(down)")
        let up = try XCTUnwrap(agent.recognizedPan("up", [:]))
        XCTAssertEqual(up["pressed"] as? Int, 1, "\(up)")
        XCTAssertEqual(presses, [1, 3, 1])
        _ = agent.recognizedPan("down", ["id": 3])
        _ = agent.recognizedPan("move", ["dx": 300.0, "dy": 300.0])
        let outside = try XCTUnwrap(agent.recognizedPan("up", [:]))
        XCTAssertNil(outside["pressed"], "\(outside)")
        XCTAssertEqual(presses, [1, 3, 1])

        root.props["inert"] = "true"
        XCTAssertNil(row.activationTarget(at: touch.location(in: nil)))
        XCTAssertNil(agent.stillPresses(reach.action, from: reach.reached))
        row.touchesBegan([touch], with: nil)
        XCTAssertFalse(root.pressed)
        row.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [1, 3, 1])
        XCTAssertTrue(p.menus.isOpen(popover))
    }

    func testPlacedContentUsesTheSameLandingBoxForTouchAndAgentPresses() throws {
        let session = page(), p = session.presenter, agent = Agent(session: session)
        let ancestor = try XCTUnwrap(p.views[1]), row = try XCTUnwrap(p.views[3])
        ancestor.handlers = ["press"]
        ancestor.applyStyle(["press_scale": 0.8])
        row.handlers = []
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "canvas"],
            ["op": "children", "id": 2, "ids": [4]],
            ["op": "children", "id": 4, "ids": [3]],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 200, "h": 160],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 40, "h": 40],
        ]))
        let canvas = try XCTUnwrap(p.views[4]), win = try XCTUnwrap(window)
        row.placement = [1, 0, 50, 0, 1, 50, 0, 0, 1, 0]
        row.alpha = 0
        let drawn = canvas.convert(CGPoint(x: 60, y: 60), to: nil)
        let touch = SheetTouch(row, at: row.convert(drawn, from: nil))
        XCTAssertFalse(row.bounds.contains(row.convert(drawn, from: nil)))
        XCTAssertTrue(row.bounds.contains(row.local(drawn)))
        XCTAssertTrue(row.pressInside(touch))
        XCTAssertTrue(win.hitTest(drawn, with: nil) === row)
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        row.touchesBegan([touch], with: nil)
        XCTAssertTrue(ancestor.pressed)
        row.touchesMoved([touch], with: nil)
        XCTAssertEqual(ancestor.press.to, 0.8, accuracy: 0.001, "feedback stays down inside the drawn content")
        row.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [1])
        XCTAssertFalse(ancestor.pressed)
        XCTAssertTrue(row.activationTarget(at: drawn) === ancestor)
        if case .node(let target)? = agent.pressReach(row, at: drawn, in: win) {
            XCTAssertTrue(target === ancestor)
        } else { XCTFail("the addressed tap must reach the ancestor through the placed content") }
        let down = try XCTUnwrap(agent.recognizedPan("down", ["id": 3]))
        XCTAssertNil(down["error"], "\(down)")
        let up = try XCTUnwrap(agent.recognizedPan("up", [:]))
        XCTAssertEqual(up["pressed"] as? Int, 1)
        XCTAssertEqual(presses, [1, 1])

        let old = canvas.convert(CGPoint(x: 10, y: 10), to: nil)
        XCTAssertTrue(row.bounds.contains(row.convert(old, from: nil)))
        XCTAssertFalse(row.bounds.contains(row.local(old)))
        XCTAssertNil(row.activationTarget(at: old))
        touch.point = row.convert(drawn, from: nil)
        row.touchesBegan([touch], with: nil)
        touch.point = row.convert(old, from: nil)
        row.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [1, 1], "the old, unplaced box cannot accept the lift")
    }

    func testCapturedPopoverPressDoesNotSurviveClosingAndReopening() throws {
        let session = page(), p = session.presenter, agent = Agent(session: session)
        let ancestor = try XCTUnwrap(p.views[1]), row = try XCTUnwrap(p.views[3]), pop = try XCTUnwrap(p.views[2])
        ancestor.handlers = ["press", "pointerup"]
        row.handlers = []
        var presses: [UInt32] = [], ups: [UInt32] = []
        p.onPress = { presses.append($0) }
        p.onPointer = { id, phase, _ in if phase == .up { ups.append(id) } }
        let down = try XCTUnwrap(agent.recognizedPan("down", ["id": 3]))
        XCTAssertNil(down["error"], "\(down)")
        XCTAssertTrue(agent.panContact?.reach?.action?.node === ancestor)
        p.menus.action("hidePopover", ["choices"])
        p.menus.action("showPopover", ["choices"])
        XCTAssertTrue(p.menus.isOpen(pop))
        let up = try XCTUnwrap(agent.recognizedPan("up", [:]))
        XCTAssertNil(up["pressed"] as? Int)
        XCTAssertEqual(presses, [])
        XCTAssertEqual(ups, [1], "closing cancels the press, not pointerup delivery")
        _ = agent.recognizedPan("down", ["id": 3])
        let fresh = try XCTUnwrap(agent.recognizedPan("up", [:]))
        XCTAssertEqual(fresh["pressed"] as? Int, 1)
        XCTAssertEqual(presses, [1])
    }

    func testBubbledPressBypassesSurfaceInputAndSurfaceOnlyAncestors() throws {
        let session = page(), p = session.presenter, agent = Agent(session: session)
        let root = try XCTUnwrap(p.views[1]), row = try XCTUnwrap(p.views[3])
        row.handlers = []
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"]],
            ["op": "children", "id": 4, "ids": [1]], ["op": "roots", "ids": [4]],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 10, "h": 10],
        ]))
        let outer = try XCTUnwrap(p.views[4]), canvas = PressCanvasProbe()
        root.canvasInput = canvas
        root.props["action"] = "primary"
        root.frame.size = CGSize(width: 10, height: 10)
        XCTAssertTrue(root.isSurfaceControl)
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        let touch = SheetTouch(row, at: CGPoint(x: 20, y: 20)), win = try XCTUnwrap(window)
        for hasHandler in [true, false] {
            root.handlers = hasHandler ? ["press"] : []
            let target = hasHandler ? root : outer
            presses = []
            XCTAssertTrue(row.activationTarget(at: touch.location(in: nil)) === target)
            row.touchesBegan([touch], with: nil)
            XCTAssertTrue(target.pressed)
            XCTAssertFalse(row.pressContact?.routesToCanvas ?? true)
            row.touchesMoved([touch], with: nil)
            XCTAssertTrue(target.pressed)
            row.touchesEnded([touch], with: nil)
            XCTAssertEqual(presses, [target.id])
            XCTAssertFalse(target.pressed)
            row.touchesBegan([touch], with: nil)
            row.touchesCancelled([touch], with: nil)
            XCTAssertFalse(target.pressed)
            row.touchesBegan([touch], with: nil)
            target.touchesCancelled([], with: nil)
            XCTAssertFalse(target.pressed)
            row.touchesEnded([touch], with: nil)
            row.touchesBegan([touch], with: nil)
            row.cancelPressContactsInSubtree()
            XCTAssertFalse(target.pressed)
            row.touchesEnded([touch], with: nil)
            XCTAssertEqual(presses, [target.id], "cancellation dispatches no press")
            XCTAssertTrue(row.activate(at: touch.location(in: nil)) === target)
            XCTAssertEqual(presses, [target.id, target.id], "recognized activation bypasses surface controls too")
            if case .node(let reached)? = agent.pressReach(row, at: touch.location(in: nil), in: win) {
                XCTAssertTrue(reached === target)
            } else { XCTFail("the agent must reach the same pressable ancestor") }
            let down = try XCTUnwrap(agent.recognizedPan("down", ["id": 3]))
            XCTAssertNil(down["error"], "\(down)")
            let up = try XCTUnwrap(agent.recognizedPan("up", [:]))
            XCTAssertEqual(up["pressed"] as? Int, Int(target.id), "\(up)")
            XCTAssertEqual(presses, [target.id, target.id, target.id])
            XCTAssertEqual(canvas.phases, [], "no phase of a bubbled contact belongs to the ancestor's canvas")
            if hasHandler {
                let reach = agent.fingerReach(row, at: touch.location(in: nil))
                root.handlers = []
                XCTAssertNil(agent.stillPresses(reach.action, from: reach.reached), "a captured target now offering only a surface control cannot take the bubbled lift")
            }
        }
        let direct = SheetTouch(root, at: CGPoint(x: 5, y: 5))
        XCTAssertTrue(root.activationTarget(at: direct.location(in: nil)) === root, "a surface control still takes its own touches")
        root.touchesBegan([direct], with: nil)
        root.touchesMoved([direct], with: nil)
        root.touchesEnded([direct], with: nil)
        root.touchesBegan([direct], with: nil)
        root.touchesCancelled([direct], with: nil)
        XCTAssertEqual(canvas.phases, ["down", "move", "up", "down", "cancel"])
    }

    func testBubbledPressDoesNotHitTestTheAncestorsInlineLink() throws {
        let session = page(), p = session.presenter
        let root = try XCTUnwrap(p.views[1]), row = try XCTUnwrap(p.views[3])
        root.handlers = []; row.handlers = []
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "text", "handlers": ["press"]],
            ["op": "paragraph", "id": 4, "runs": [
                ["id": 5, "parent": 4, "paint": true, "props": ["text": "Link behind the popover", "href": "https://example.test/wrong"],
                 "handlers": [], "style": ["font_size": 18.0]]]],
            ["op": "children", "id": 4, "ids": [1]], ["op": "roots", "ids": [4]],
            ["op": "frame", "id": 4, "x": 0, "y": 0, "w": 400, "h": 800],
        ]))
        let ancestor = try XCTUnwrap(p.views[4]), run = try XCTUnwrap(p.inlineText(5))
        let rect = try XCTUnwrap(ancestor.inlineRects(run).first)
        let touch = SheetTouch(row, at: CGPoint(x: 20, y: 20))
        let point = try XCTUnwrap(ancestor.superview).convert(touch.location(in: nil), from: nil)
        ancestor.frame.origin = CGPoint(x: point.x - rect.midX, y: point.y - rect.midY)
        XCTAssertEqual(ancestor.inlineActivationTarget(at: ancestor.local(touch.location(in: nil)))?.id, 5,
                       "the same window point really hits the ancestor's inline link")
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        row.touchesBegan([touch], with: nil)
        XCTAssertTrue(ancestor.pressContact?.bubbled == true)
        XCTAssertTrue(ancestor.pressed)
        XCTAssertNil(ancestor.inlinePressed)
        XCTAssertNil(ancestor.linkPressed)
        row.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [4], "the ancestor's own press wins over unrelated content beneath the presentation")
        XCTAssertFalse(ancestor.pressed)
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
private final class PressCanvasProbe: CanvasInput {
    var phases: [String] = []
    func touches(_ values: Set<UITouch>, phase: String, source: NodeView, event: UIEvent?) -> Bool {
        phases.append(phase)
        return true
    }
    func presses(_ presses: Set<UIPress>, down: Bool, source: NodeView) -> Bool { false }
    func blur() {}
}
#endif
