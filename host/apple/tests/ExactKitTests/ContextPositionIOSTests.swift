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

    func testInlineTapUsesTheRunFragmentsInsteadOfTheParagraphMidpoint() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let session = ExactApp.shared.makeSession(label: "inline-tap-geometry")
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
        let agent = Agent(session: session)
        for (prefix, height, visible) in [("", 1000.0, true), (String(repeating: "line\n", count: 20), 500.0, false)] {
            p.apply(wireBatch([
                ["op": "create", "id": 10001, "kind": "view"],
                ["op": "create", "id": 10002, "kind": "text", "style": ["white_space": "pre-wrap", "font_size": 16, "line_height": "20px"]],
                ["op": "paragraph", "id": 10002, "runs": [
                    ["id": 10003, "parent": 10002, "paint": true, "props": ["text": prefix], "style": [:], "handlers": []],
                    ["id": 10004, "parent": 10002, "paint": true, "props": ["text": "target"], "style": [:], "handlers": ["press"]]
                ]],
                ["op": "children", "id": 10001, "ids": [10002]],
                ["op": "roots", "ids": [10001]],
                ["op": "frame", "id": 10001, "x": 0.0, "y": 0.0, "w": 400.0, "h": 1200.0],
                ["op": "frame", "id": 10002, "x": 0.0, "y": 0.0, "w": 300.0, "h": height]
            ]))
            let owner = try XCTUnwrap(p.views[10002]), run = try XCTUnwrap(p.inlineText(10004))
            let fragment = try XCTUnwrap(owner.inlineRects(run).first)
            XCTAssertEqual(fragment.midY < 300, visible, "the actual CoreText fragment defines visibility")
            XCTAssertEqual(agent.box(owner).midY < 300, !visible, "the paragraph midpoint gives the opposite answer")
            let reply = agent.tap(["id": NSNumber(value: 10004)])
            if visible {
                XCTAssertNil(reply["error"], "\(reply)")
                XCTAssertEqual(reply["tapped"] as? Int, 10004)
                owner.translate = CGPoint(x: 0, y: 400)
                owner.applyTransform()
                let moved = agent.tap(["id": NSNumber(value: 10004)])
                XCTAssertTrue((moved["error"] as? String)?.contains("outside") == true,
                              "presentation transforms move the fragment too: \(moved)")
                XCTAssertNil(moved["tapped"])
            } else {
                XCTAssertTrue((reply["error"] as? String)?.contains("outside") == true, "\(reply)")
                XCTAssertNil(reply["tapped"])
            }
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
