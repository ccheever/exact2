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

    func testRevealScrollsAnActionUnderTheKeyboardAboveItUnderTheDefaultWidget() throws {
        let (session, window) = session("reveal-keyboard-default", widget: nil, scroller: false)
        defer { window.isHidden = true; session.destroy() }
        let p = session.presenter
        // A keyboard whose top is 340 points above the window's bottom: the
        // default `resizes-visual` makes its overlap the viewport's inset.
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

    func testWithoutAKeyboardAnActionInsideTheViewportStaysPut() throws {
        let (session, window) = session("reveal-keyboard-none", widget: nil, scroller: true)
        defer { window.isHidden = true; session.destroy() }
        let reply = Agent(session: session).reveal(["id": 3])
        XCTAssertEqual(reply["scrolled"] as? Bool, false, "\(reply)")
        XCTAssertEqual(try middle(session), 720, accuracy: 0.5)
    }
}
#endif
