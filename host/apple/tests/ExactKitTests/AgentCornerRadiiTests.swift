import XCTest
@testable import ExactKit
#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif

final class AgentCornerRadiiTests: XCTestCase {
    func testPresentedGeometryUsesAnimatedBoundsAndPresentedScrollOrigin() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        let p = session.presenter
        let size = CGRect(x: 0, y: 0, width: 320, height: 400)
        #if os(macOS)
        let window = NSWindow(contentRect: size, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        window.orderFrontRegardless()
        defer { window.close() }
        #else
        let window = UIWindow(frame: size)
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        #endif
        p.viewport.frame = size
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 320, "h": 1000],
            ["op": "frame", "id": 2, "x": 20, "y": 200, "w": 200, "h": 100],
        ]))
        let view = try XCTUnwrap(p.views[2])
        #if os(macOS)
        view.wantsLayer = true
        p.viewport.contentView.wantsLayer = true
        p.viewport.contentView.scroll(to: CGPoint(x: 0, y: 100))
        let layer = try XCTUnwrap(view.layer), port = try XCTUnwrap(p.viewport.contentView.layer)
        #else
        p.viewport.contentSize = CGSize(width: 320, height: 1000)
        p.viewport.contentOffset = CGPoint(x: 0, y: 100)
        let layer = view.layer, port = p.viewport.layer
        #endif
        func hold(_ layer: CALayer, key: String, from: NSValue, to: NSValue) {
            let motion = CABasicAnimation(keyPath: key)
            motion.fromValue = from; motion.toValue = to
            motion.duration = 1; motion.speed = 0; motion.timeOffset = 0.5
            motion.fillMode = .both; motion.isRemovedOnCompletion = false
            layer.add(motion, forKey: key)
        }
        #if os(macOS)
        hold(layer, key: "bounds", from: NSValue(rect: CGRect(x: 0, y: 0, width: 100, height: 100)), to: NSValue(rect: layer.bounds))
        hold(port, key: "bounds.origin", from: NSValue(point: .zero), to: NSValue(point: CGPoint(x: 0, y: 100)))
        #else
        hold(layer, key: "bounds", from: NSValue(cgRect: CGRect(x: 0, y: 0, width: 100, height: 100)), to: NSValue(cgRect: layer.bounds))
        hold(port, key: "bounds.origin", from: NSValue(cgPoint: .zero), to: NSValue(cgPoint: CGPoint(x: 0, y: 100)))
        #endif
        CATransaction.flush()
        let deadline = Date(timeIntervalSinceNow: 3)
        while (layer.presentation() == nil || port.presentation() == nil), Date() < deadline {
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01))
        }
        let agent = Agent(session: session), result = agent.geometry(view)
        let target = try XCTUnwrap(result["target"] as? [String: Double])
        let presented = try XCTUnwrap(result["presented"] as? [String: Double], "\(result)")
        XCTAssertEqual(target["w"] ?? .nan, 200, accuracy: 0.1)
        XCTAssertEqual(presented["w"] ?? .nan, 150, accuracy: 0.1)
        XCTAssertEqual((presented["y"] ?? .nan) - (target["y"] ?? .nan), 50, accuracy: 0.1)
        let fragment = agent.geometry(view, region: CGRect(x: 0, y: 0, width: 10, height: 10))
        XCTAssertNotNil((fragment["presented"] as? [String: Any])?["unavailable"])
    }

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
