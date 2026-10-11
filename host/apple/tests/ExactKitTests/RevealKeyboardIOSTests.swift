#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1086.000.000 D2: the agent's `reveal` counts what a person sees as in
/// view, so a form's lower action under the software keyboard is scrolled
/// above it before a tap, as a finger would scroll the form, instead of
/// being left there for the tap to refuse.
final class RevealKeyboardIOSTests: XCTestCase {
    private func session(_ label: String, widget: String?, scroller: Bool) -> (ExactSession, UIWindow) {
        let session = ExactApp.shared.makeSession(label: label)
        let p = session.presenter
        p.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        let window = UIWindow(frame: p.viewport.frame)
        window.addSubview(p.viewport)
        window.isHidden = false
        // The action (3, with a child of its own so it is a view) sits at
        // y 700–740: in the root page, or in a 2000-point scroller (2).
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view"]]
        if let widget { ops.append(["op": "props", "id": 1, "set": ["interactiveWidget": widget]]) }
        ops += [
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 4, "kind": "view"],
            ["op": "children", "id": 3, "ids": [4]],
        ]
        if scroller {
            ops += [
                ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
                ["op": "create", "id": 5, "kind": "view"],
                ["op": "children", "id": 5, "ids": [3]],
                ["op": "children", "id": 2, "ids": [5]],
                ["op": "children", "id": 1, "ids": [2]],
            ]
        } else {
            ops.append(["op": "children", "id": 1, "ids": [3]])
        }
        ops += [
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 3, "x": 16.0, "y": 700.0, "w": 368.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 368.0, "h": 40.0],
        ]
        if scroller {
            ops += [
                ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
                ["op": "content", "id": 2, "w": 400.0, "h": 2000.0],
                ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 400.0, "h": 2000.0],
            ]
        }
        p.apply(wireBatch(ops))
        return (session, window)
    }

    /// The action's middle, in the window.
    private func middle(_ session: ExactSession) throws -> CGFloat {
        let v = try XCTUnwrap(session.presenter.views[3])
        return v.convert(CGPoint(x: v.bounds.midX, y: v.bounds.midY), to: nil).y
    }

    func testRevealScrollsAnActionUnderTheKeyboardAboveItUnderResizesVisual() throws {
        let (session, window) = session("reveal-keyboard-visual", widget: "resizes-visual", scroller: false)
        defer { window.isHidden = true; session.destroy() }
        let p = session.presenter
        // A keyboard whose top is 340 points above the window's bottom: the
        // web's `resizes-visual`, the opt-out, makes its overlap the viewport's inset.
        p.applyKeyboard(top: 460, duration: 0, curve: 0)
        XCTAssertEqual(p.viewport.contentInset.bottom, 340)
        XCTAssertEqual(try middle(session), 720, accuracy: 0.5, "under the keyboard, inside the viewport's bounds")
        let reply = Agent(session: session).reveal(["id": 3])
        XCTAssertNil(reply["error"])
        XCTAssertEqual(reply["scrolled"] as? Bool, true, "\(reply)")
        XCTAssertLessThan(try middle(session), 460, "above the keyboard: \(reply)")
        // Already in view: nothing moves.
        let again = Agent(session: session).reveal(["id": 3])
        XCTAssertEqual(again["scrolled"] as? Bool, false, "\(again)")
    }

    func testRevealUsesTheKeyboardsTopUnderOverlaysContent() throws {
        let (session, window) = session("reveal-keyboard-overlays", widget: "overlays-content", scroller: true)
        defer { window.isHidden = true; session.destroy() }
        let p = session.presenter
        p.applyKeyboard(top: 460, duration: 0, curve: 0)
        XCTAssertEqual(p.viewport.contentInset.bottom, 0, "overlays-content insets nothing")
        let reply = Agent(session: session).reveal(["id": 3])
        XCTAssertEqual(reply["scrolled"] as? Bool, true, "\(reply)")
        XCTAssertLessThan(try middle(session), 460, "above the keyboard: \(reply)")
    }

    /// A target wider than its horizontal scroller, its middle off the left:
    /// centring brings the middle in, where aligning its far edge (the
    /// right, past the port) would push it further out.
    func testRevealCentresATargetWiderThanItsScroller() throws {
        let session = ExactApp.shared.makeSession(label: "reveal-wide")
        let p = session.presenter
        p.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        let window = UIWindow(frame: p.viewport.frame)
        window.addSubview(p.viewport)
        window.isHidden = false
        defer { window.isHidden = true; session.destroy() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_x": "scroll"]],
            ["op": "create", "id": 5, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 4, "kind": "view"],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 5, "ids": [3]],
            ["op": "children", "id": 2, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 100.0],
            ["op": "content", "id": 2, "w": 2000.0, "h": 100.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 2000.0, "h": 100.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 1500.0, "h": 100.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 1500.0, "h": 100.0],
        ]))
        let scroller = try XCTUnwrap(p.views[2]?.scroll)
        scroller.contentOffset.x = 1000
        let v = try XCTUnwrap(p.views[3])
        let reply = Agent(session: session).reveal(["id": 3])
        XCTAssertEqual(reply["scrolled"] as? Bool, true, "\(reply)")
        let middle = v.convert(CGPoint(x: v.bounds.midX, y: v.bounds.midY), to: nil).x
        XCTAssertTrue((0..<400).contains(middle), "its middle is in view: \(middle), \(reply)")
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    /// LLP 1116 D2: keyboard avoidance is the default, as in a hand-built
    /// app. A root that names no `interactive-widget` ends the viewport at
    /// the keyboard's top, so the routes end above the keys, a scroller's
    /// last rows and the field in it reachable there; `resizes-visual` opts
    /// out to the web's inset.
    func testTheDefaultEndsTheViewportAtTheKeyboardsTop() throws {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: "keyboard-default")
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        defer { session.destroy(); window.isHidden = true }
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        let p = session.presenter
        XCTAssertNil(p.interactiveWidget, "the fixture's root names none")
        XCTAssertEqual(p.keyboardPolicy, "resizes-content")
        XCTAssertEqual((Agent(session: session).stateSections()["keyboard"] as? [String: Any])?["policy"] as? String, "resizes-content", "the agent reports it")
        let route = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "route-home" })
        let bottom = { route.convert(route.bounds, to: nil).maxY }
        XCTAssertEqual(bottom(), 874, accuracy: 0.5, "the whole window without a keyboard")
        p.applyKeyboard(top: 574, duration: 0, curve: 0)
        spin(0.3)
        XCTAssertEqual(p.viewport.frame.maxY, 574, accuracy: 0.5, "the viewport ends at the keys")
        XCTAssertEqual(p.viewport.contentInset.bottom, 0, "nothing to pan")
        XCTAssertEqual(p.keyboardInset, 300, accuracy: 0.5, "the overlap, against the viewport without it")
        XCTAssertEqual(bottom(), 574, accuracy: 0.5, "the route ends above the keys")
        p.applyKeyboard(top: nil, duration: 0, curve: 0)
        spin(0.3)
        XCTAssertEqual(bottom(), 874, accuracy: 0.5, "and takes the window back")
        // The opt-out: the web's inset, the layout viewport left alone.
        let root = try XCTUnwrap(p.root.subviews.first as? NodeView)
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["interactiveWidget": "resizes-visual"]]]))
        XCTAssertEqual(p.keyboardPolicy, "resizes-visual")
        p.applyKeyboard(top: 574, duration: 0, curve: 0)
        spin(0.3)
        XCTAssertEqual(p.viewport.contentInset.bottom, 300, accuracy: 0.5, "inset by the overlap")
        XCTAssertEqual(bottom(), 874, accuracy: 0.5, "laid out as without a keyboard")
        p.applyKeyboard(top: nil, duration: 0, curve: 0)
    }

    func testWithoutAKeyboardAnActionInsideTheViewportStaysPut() throws {
        let (session, window) = session("reveal-keyboard-none", widget: nil, scroller: true)
        defer { window.isHidden = true; session.destroy() }
        let reply = Agent(session: session).reveal(["id": 3])
        XCTAssertEqual(reply["scrolled"] as? Bool, false, "\(reply)")
        XCTAssertEqual(try middle(session), 720, accuracy: 0.5)
    }
}
#endif
