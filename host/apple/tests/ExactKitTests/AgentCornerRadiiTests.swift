import XCTest
@testable import ExactKit
#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif

final class AgentCornerRadiiTests: XCTestCase {
    func testLayoutReportsReducedEllipticalRadiiInLocalPoints() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let session = ExactApp.shared.makeSession(label: "corner-radii")
        defer { session.destroy() }
        let p = session.presenter
        let bounds = CGRect(x: 0, y: 0, width: 320, height: 400)
        #if os(iOS) || os(tvOS)
        let window = UIWindow(frame: bounds)
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        #else
        let window = NSWindow(contentRect: bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = p.viewport
        defer { window.orderOut(nil) }
        #endif
        p.viewport.frame = bounds
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["id": "root"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "rounded"], "style": [
                "border_radius_top_left": ["pct": 80], "border_radius_top_right": 80,
                "border_radius_bottom_right": 20, "border_radius_bottom_left": 10]],
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [255, 0, 0, 255],
                "border_radius_top_left": 60, "border_radius_top_right": 60,
                "border_radius_bottom_right": 60, "border_radius_bottom_left": 60]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 320, "h": 400],
            ["op": "frame", "id": 2, "x": 10, "y": 20, "w": 100, "h": 50],
            ["op": "frame", "id": 3, "x": 10, "y": 100, "w": 80, "h": 40],
        ]))
        let agent = Agent(session: session)
        func radii(_ id: Int) throws -> [[Double]] {
            let rows = try XCTUnwrap(agent.layout()["nodes"] as? [[String: Any]])
            let node = try XCTUnwrap(rows.first { $0["id"] as? Int == id })
            return try XCTUnwrap(node["radii"] as? [[Double]])
        }
        let geometry = agent.geometry(try XCTUnwrap(p.views[2]))
        let target = try XCTUnwrap(geometry["target"] as? [String: Any])
        XCTAssertEqual(target["w"] as? Double, 100)
        #if os(macOS)
        XCTAssertNotNil((geometry["presented"] as? [String: Any])?["unavailable"], "a model is never substituted for a missing presentation tree")
        #endif
        XCTAssertNil(agent.layout()["layoutVersion"], "the protocol has no per-layout version")
        XCTAssertEqual(try radii(1), Array(repeating: [0, 0], count: 4))
        // The right edge's 80 + 20 is twice its height: all axes halve.
        XCTAssertEqual(try radii(2), [[40, 20], [40, 40], [10, 10], [5, 5]])
        XCTAssertEqual(try radii(3), Array(repeating: [20, 20], count: 4))
        #if os(iOS) || os(tvOS)
        XCTAssertNotNil(p.flats.leaves[3], "the layer-only box must be covered too")
        #endif
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 10, "y": 20, "w": 200, "h": 100]]))
        let resized = try radii(2)
        XCTAssertEqual(resized[0][0], 400.0 / 3, accuracy: 0.00001)
        XCTAssertEqual(resized[0][1], 200.0 / 3, accuracy: 0.00001)
        XCTAssertEqual(resized[1][0], 200.0 / 3, accuracy: 0.00001)
    }
    #if os(iOS) || os(tvOS)
    func testLayoutReportsTheDefaultPopoverCornerAndAuthoredOverride() throws {
        let session = ExactApp.shared.makeSession(label: "popover-corner")
        defer { session.destroy() }
        let p = session.presenter, window = UIWindow(frame: CGRect(x: 0, y: 0, width: 320, height: 400))
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["popover": "auto", "id": "p",
                "defaultSurface": "true", "defaultSurfaceRadius": "30"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 80],
        ]))
        let node = try XCTUnwrap(p.views[1]), agent = Agent(session: session)
        XCTAssertNotNil(node.materialView)
        func radii() throws -> [[Double]] {
            let rows = try XCTUnwrap(agent.layout()["nodes"] as? [[String: Any]])
            return try XCTUnwrap(rows.first { $0["id"] as? Int == 1 }?["radii"] as? [[Double]])
        }
        XCTAssertEqual(try radii(), Array(repeating: [30, 30], count: 4))
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["border_radius": 12]]]))
        XCTAssertEqual(try radii(), Array(repeating: [12, 12], count: 4))
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["border_radius": 0]]]))
        XCTAssertEqual(try radii(), Array(repeating: [0, 0], count: 4), "an authored square overrides the platform")
    }
    #endif

}
