#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

/// The iOS agent at a world's canvas, on `SurfaceControlTests`' fixture.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class SurfaceControlIOSTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
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
        XCTAssertFalse(jump.focusedByTouch, "a touch keeps the focus Tab or focus(id) gave it the app's")
        _ = jump.resignFirstResponder()
        XCTAssertTrue(jump.focusSurfacePointer())
        XCTAssertTrue(jump.isFirstResponder && jump.focusedByTouch, "the touch's focus")
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
