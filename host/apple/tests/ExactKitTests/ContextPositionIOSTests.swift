#if os(macOS) || os(iOS)
import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// A static wrapper's bounds are not an absolute context panel's containing
/// block. Exercise actual batch presentation, including coordinate conversion.
final class ContextPositionIOSTests: XCTestCase {
    func testAgentRefusesOffViewportTargetsBeforeActivation() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let session = ExactApp.shared.makeSession(label: "offscreen-tap")
        defer { session.destroy() }
        let p = session.presenter
        p.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        #if os(macOS)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        defer { window.close() }
        #else
        let window = UIWindow(frame: p.viewport.frame)
        window.addSubview(p.viewport)
        defer { window.isHidden = true }
        #endif
        p.apply(wireBatch([
            ["op": "create", "id": 10001, "kind": "view"],
            ["op": "create", "id": 10002, "kind": "button"],
            ["op": "children", "id": 10001, "ids": [10002]],
            ["op": "roots", "ids": [10001]],
            ["op": "frame", "id": 10001, "x": 0.0, "y": 0.0, "w": 400.0, "h": 900.0],
            ["op": "frame", "id": 10002, "x": 20.0, "y": 600.0, "w": 100.0, "h": 40.0]
        ]))
        let agent = Agent(session: session)
        for action: [String: Any] in [["id": 10002], ["id": 10002, "contextmenu": true], ["id": 10002, "dblclick": true]] {
            let reply = agent.tap(action)
            XCTAssertTrue((reply["error"] as? String)?.contains("outside") == true, "\(reply)")
            XCTAssertNil(reply["tapped"])
        }
    }

    func testContextPanelClampsToPositionedAncestorAcrossStaticWrappers() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 300)
        #if os(macOS)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        defer { window.close() }
        #else
        let window = UIWindow(frame: p.viewport.frame)
        window.addSubview(p.viewport)
        defer { window.isHidden = true }
        #endif
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["position_type": "relative"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "style": ["position_type": "absolute"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["contextTarget": "source", "contextMagnify": "false"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["id": "source"]],
            ["op": "children", "id": 1, "ids": [2, 5]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 400.0, "h": 50.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 100.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 20.0, "w": 100.0, "h": 50.0]
        ]))
        let panel = try XCTUnwrap(p.views[3])
        XCTAssertEqual(panel.convert(panel.bounds, to: p.views[1]).minY, 20, accuracy: 0.01)
        // A positioned wrapper really does constrain the preview.
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["position_type": "relative"]]]))
        XCTAssertEqual(panel.convert(panel.bounds, to: p.views[1]).minY, 100, accuracy: 0.01)
    }
}
#endif
