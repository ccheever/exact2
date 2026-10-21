#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

/// The iOS agent at a world's canvas, on `SurfaceControlTests`' fixture.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class SurfaceControlIOSTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4

    private final class CanvasTouch: UITouch {
        let target: UIView
        var point: CGPoint
        init(_ point: CGPoint, on target: UIView) { self.point = point; self.target = target; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { view.map { $0.convert(point, from: nil) } ?? point }
        override func previousLocation(in view: UIView?) -> CGPoint { location(in: view) }
    }

    func testCanvasKeepsBothTouchesThroughMovesAndUps() {
        let (s, canvas, _) = SurfaceControlTests().fixture(); defer { s.destroy() }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        s.presenter.viewport.frame = window.bounds
        s.presenter.root.frame = window.bounds
        window.addSubview(s.presenter.viewport); window.makeKeyAndVisible()
        let first = CanvasTouch(CGPoint(x: 130, y: 80), on: canvas)
        let second = CanvasTouch(CGPoint(x: 160, y: 110), on: canvas)
        controlEvents = []

        canvas.touchesBegan([first], with: nil)
        canvas.touchesBegan([second], with: nil)
        first.point.x += 10; second.point.x += 10
        canvas.touchesMoved([first, second], with: nil)
        canvas.touchesEnded([first], with: nil)
        canvas.touchesEnded([second], with: nil)

        let pointers = controlEvents.filter { $0["t"] as? String == "pointer" }
        XCTAssertEqual(pointers.compactMap { $0["phase"] as? String }, ["down", "down", "move", "move", "up", "up"])
        let idsByPhase = Dictionary(grouping: pointers, by: { $0["phase"] as? String ?? "" })
        for phase in ["down", "move", "up"] {
            XCTAssertEqual(Set(idsByPhase[phase, default: []].compactMap { $0["id"] as? Int }), Set([1, 2]), "both fingers reach \(phase)")
        }
        XCTAssertFalse(pointers.contains { $0["phase"] as? String == "cancel" })

        controlEvents = []
        let third = CanvasTouch(CGPoint(x: 130, y: 80), on: canvas)
        let fourth = CanvasTouch(CGPoint(x: 160, y: 110), on: canvas)
        canvas.touchesBegan([third, fourth], with: nil)
        canvas.touchesEnded([third, fourth], with: nil)
        XCTAssertEqual(controlEvents.compactMap { $0["phase"] as? String }, ["down", "down", "up", "up"], "one callback also delivers both contacts")
        withExtendedLifetime(window) {}
    }

    func testChildOfSurfaceControlForwardsMoveUpAndCancel() {
        let (s, _, jump) = SurfaceControlTests().fixture(); defer { s.destroy() }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        s.presenter.viewport.frame = window.bounds
        s.presenter.root.frame = window.bounds
        window.addSubview(s.presenter.viewport); window.makeKeyAndVisible()
        let child = NodeView(id: 102, kind: "text", presenter: s.presenter)
        child.frame = CGRect(x: 0, y: 0, width: 80, height: 40)
        jump.addSubview(child); s.presenter.views[102] = child
        controlEvents = []

        let released = CanvasTouch(CGPoint(x: 20, y: 20), on: child)
        child.touchesBegan([released], with: nil)
        released.point.x += 5
        child.touchesMoved([released], with: nil)
        child.touchesEnded([released], with: nil)

        let cancelled = CanvasTouch(CGPoint(x: 30, y: 20), on: child)
        child.touchesBegan([cancelled], with: nil)
        child.touchesCancelled([cancelled], with: nil)

        XCTAssertEqual(controlEvents.filter { $0["t"] as? String == "control" }
            .compactMap { $0["phase"] as? String }, ["down", "move", "up", "down", "cancel"])
        XCTAssertTrue(s.surfaceHost.entries[100]?.controls.isEmpty == true, "no control remains held")
        withExtendedLifetime(window) {}
    }

    func testOverflowChildOfSurfaceControlKeepsEditingOnRelease() throws {
        let (s, _, jump) = SurfaceControlTests().fixture(); defer { s.destroy() }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        s.presenter.viewport.frame = window.bounds
        s.presenter.root.frame = window.bounds
        window.addSubview(s.presenter.viewport); window.makeKeyAndVisible()
        let child = NodeView(id: 102, kind: "text", presenter: s.presenter)
        child.frame = CGRect(x: 110, y: 10, width: 40, height: 40)
        jump.addSubview(child); s.presenter.views[102] = child
        let input = NodeView(id: 103, kind: "input", presenter: s.presenter)
        input.frame = CGRect(x: 0, y: 150, width: 100, height: 34)
        s.presenter.root.addSubview(input); s.presenter.views[103] = input
        let field = try XCTUnwrap(input.field)
        XCTAssertTrue(field.becomeFirstResponder())
        let point = child.convert(CGPoint(x: 10, y: 10), to: nil)
        let hit = try XCTUnwrap(window.hitTest(window.convert(point, from: nil), with: nil))
        XCTAssertTrue(hit === child, "the overflow child must be UIKit's actual hit")
        controlEvents = []
        let touch = CanvasTouch(point, on: hit)
        child.touchesBegan([touch], with: nil)
        child.touchesEnded([touch], with: nil)
        XCTAssertEqual(controlEvents.filter { $0["t"] as? String == "control" }.compactMap { $0["phase"] as? String }, ["down", "up"])
        XCTAssertTrue(field.isFirstResponder, "the surface control's release leaves editing as an in-bounds release does")
        withExtendedLifetime(window) {}
    }

    /// b6 review B6: a chord the iOS agent types at a world's canvas is split
    /// before the keyboard's route; nothing taking it, the world hears its key.
    func testAgentChordAtACanvasReachesTheWorldByItsKey() {
        let (s, _, _) = SurfaceControlTests().fixture(); defer { s.destroy() }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        s.presenter.viewport.frame = window.bounds
        window.addSubview(s.presenter.viewport); window.makeKeyAndVisible()
        controlEvents = []
        let reply = s.agentInstance.type(["id": 100, "key": "Shift+KeyO"])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(controlEvents.compactMap { $0["code"] as? String }, ["KeyO", "KeyO"])
        withExtendedLifetime(window) {}
    }

    /// A HUD button over a world takes no focus from a tap, a finger's or
    /// the agent's, as UIKit's take none: the world keeps the keyboard and
    /// is never blurred, so held keys stay held. A native button's
    /// activation and the agent's tap on it do the same.
    func testATappedHUDButtonLeavesTheKeyboardWithItsWorld() throws {
        let (s, canvas, button) = SurfaceControlTests().fixture(); defer { s.destroy() }
        button.props["action"] = nil; button.handlers = ["press"]
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        s.presenter.viewport.frame = window.bounds
        s.presenter.root.frame = window.bounds
        window.addSubview(s.presenter.viewport); window.makeKeyAndVisible()
        XCTAssertTrue(canvas.becomeFirstResponder())
        controlEvents = []
        let touch: Set<UITouch> = [UITouch()]
        button.touchesBegan(touch, with: nil); button.touchesEnded(touch, with: nil)
        let reply = s.agentInstance.tap(["op": "tap", "id": Int(button.id)])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertFalse(button.isFirstResponder)
        let native = NodeView(id: 103, kind: "control", presenter: s.presenter)
        native.props["type"] = "button"; native.handlers = ["press"]; native.frame = CGRect(x: 100, y: 0, width: 80, height: 44)
        canvas.addSubview(native); s.presenter.views[103] = native
        native.activateNative()
        XCTAssertNil(s.agentInstance.tap(["op": "tap", "id": 103])["error"])
        XCTAssertFalse(native.isFirstResponder)
        XCTAssertTrue(canvas.isFirstResponder, "movement keys still reach the world")
        XCTAssertFalse(controlEvents.contains { $0["t"] as? String == "blur" }, "the world is never blurred")
        withExtendedLifetime(window) {}
    }

    /// A surface control's focus from its touch (Jump, which forwards
    /// movement keys to its world) is touch focus, which a later autofocus
    /// may take, unless it held the focus already; and it stays through a HUD
    /// button's tap (Pause): a press ends only text editing.
    func testASurfaceControlsTouchFocusStaysThroughAHUDTap() throws {
        let (s, canvas, jump) = SurfaceControlTests().fixture(); defer { s.destroy() }
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        s.presenter.viewport.frame = window.bounds
        s.presenter.root.frame = window.bounds
        window.addSubview(s.presenter.viewport); window.makeKeyAndVisible()
        let pause = NodeView(id: 103, kind: "button", presenter: s.presenter)
        pause.handlers = ["press"]; pause.frame = CGRect(x: 100, y: 100, width: 80, height: 44)
        canvas.addSubview(pause); s.presenter.views[103] = pause
        XCTAssertTrue(jump.becomeFirstResponder())
        XCTAssertTrue(jump.focusSurfacePointer())
        XCTAssertFalse(jump.focusedBySurfaceTouch, "a touch keeps the focus Tab or focus(id) gave it the app's")
        _ = jump.resignFirstResponder()
        XCTAssertTrue(jump.focusSurfacePointer())
        XCTAssertTrue(jump.isFirstResponder && jump.focusedBySurfaceTouch, "the touch's focus")
        let touch: Set<UITouch> = [UITouch()]
        pause.touchesBegan(touch, with: nil); pause.touchesEnded(touch, with: nil)
        XCTAssertNil(s.agentInstance.tap(["op": "tap", "id": 103])["error"])
        XCTAssertTrue(jump.isFirstResponder, "movement keys still reach the world through Jump")
        withExtendedLifetime(window) {}
    }

    /// A HUD button that asked for focus (a tabindex) keeps the focus its
    /// tap gave it, as any node that asked does: nothing hands it back.
    func testAHUDButtonThatAskedForFocusKeepsIt() throws {
        let (s, canvas, button) = SurfaceControlTests().fixture(); defer { s.destroy() }
        button.props["action"] = nil; button.handlers = ["press"]; button.props["tabIndex"] = "0"
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        s.presenter.viewport.frame = window.bounds
        window.addSubview(s.presenter.viewport); window.makeKeyAndVisible()
        XCTAssertTrue(canvas.becomeFirstResponder())
        let touch: Set<UITouch> = [UITouch()]
        button.touchesBegan(touch, with: nil); button.touchesEnded(touch, with: nil)
        XCTAssertTrue(button.isFirstResponder)
        XCTAssertFalse(canvas.isFirstResponder)
        withExtendedLifetime(window) {}
    }
}
#endif
