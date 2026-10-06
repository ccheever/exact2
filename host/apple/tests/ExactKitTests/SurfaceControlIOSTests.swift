#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// The iOS agent at a world's canvas, on `SurfaceControlTests`' fixture.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class SurfaceControlIOSTests: XCTestCase {
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
}
#endif
