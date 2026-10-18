#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// A screenshot shows what a person sees (LLP 1116.012 D3): an alert, an
/// action sheet, a popover or a presented route's sheet is a view of the
/// window beside the viewport, so the viewport's own picture left it out
/// (grocery-list, round 14: two captures missed the clear alert). With one
/// up the capture is the window's, as `window` draws it, and says what is
/// presented; with none it is the viewport's, as it was. `presentedViewController`
/// is set at once, in a scene's window or a bare one (a hostless bundle has no
/// scene, and a presentation completes only in one), so no test here waits for it.
final class AgentPresentedIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    private var root: UIViewController?
    override func tearDown() { unmount(); super.tearDown() }
    private func unmount() {
        session?.destroy(); session = nil
        window?.isHidden = true; window = nil; root = nil
    }

    /// An agent whose viewport sits below the window's top, so the viewport's
    /// picture and the window's differ in height.
    private func mounted() -> Agent {
        unmount()
        let session = ExactApp.shared.makeSession(label: "agent-presented")
        self.session = session
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        let root = UIViewController()
        window.rootViewController = root
        window.makeKeyAndVisible()
        self.window = window; self.root = root
        let p = session.presenter
        p.viewport.frame = CGRect(x: 0, y: 100, width: 400, height: 700)
        root.view.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 700.0],
        ]))
        p.viewport.layoutIfNeeded()
        return Agent(session: session)
    }
    private func shot(_ agent: Agent, _ extra: [String: Any] = [:]) -> [String: Any] {
        var request: [String: Any] = ["path": NSTemporaryDirectory() + "agent-presented-\(UUID().uuidString).png"]
        request.merge(extra) { $1 }
        return agent.screenshot(request)
    }

    func testAScreenshotIsTheViewportsWithNothingPresentedAndTheWindowsWithAnAlertOrSheetUp() throws {
        let agent = mounted()
        XCTAssertNil(agent.presentedOverApp())
        var r = shot(agent)
        XCTAssertNil(r["window"], "no window drawn")
        XCTAssertNil(r["presented"])
        XCTAssertEqual(r["w"] as? Double, 400); XCTAssertEqual(r["h"] as? Double, 700, "the viewport's height")
        r = shot(agent, ["window": true])
        XCTAssertEqual(r["window"] as? Bool, true); XCTAssertEqual(r["h"] as? Double, 800, "`window` is the window's")
        XCTAssertNil(r["presented"], "nothing was up")

        // An alert is the window's.
        let alert = UIAlertController(title: "Clear checked?", message: nil, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Clear", style: .destructive))
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        try XCTUnwrap(root).present(alert, animated: false)
        XCTAssertEqual(agent.presentedOverApp(), "alert")
        r = shot(agent)
        XCTAssertEqual(r["window"] as? Bool, true, "the capture is the window's")
        XCTAssertEqual(r["presented"] as? String, "alert")
        XCTAssertEqual(r["w"] as? Double, 400); XCTAssertEqual(r["h"] as? Double, 800, "the window's height, not the viewport's")
        // A film's frames keep one size: the viewport's.
        r = shot(agent, ["film": true])
        XCTAssertNil(r["window"]); XCTAssertEqual(r["h"] as? Double, 700)

        // An action sheet.
        let again = mounted()
        let chooser = UIAlertController(title: "Open in", message: nil, preferredStyle: .actionSheet)
        chooser.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        try XCTUnwrap(root).present(chooser, animated: false)
        XCTAssertEqual(again.presentedOverApp(), "action sheet")
        XCTAssertEqual(shot(again)["h"] as? Double, 800)

        // A presented route's sheet. (An alert over one reads as the alert, the innermost: that needs a scene's window.)
        let third = mounted()
        let sheet = UIViewController()
        sheet.modalPresentationStyle = .pageSheet
        try XCTUnwrap(root).present(sheet, animated: false)
        XCTAssertEqual(third.presentedOverApp(), "sheet")
        r = shot(third)
        XCTAssertEqual(r["presented"] as? String, "sheet"); XCTAssertEqual(r["h"] as? Double, 800)
    }

    /// A native button that hides the popover `confirm`.
    private func button(_ id: Int, press: Bool, y: Double) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": "control", "handlers": press ? ["press"] : [],
          "props": ["type": "button", "accessibilityRole": "button", "popovertarget": "confirm", "popovertargetaction": "hide"],
          "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
         ["op": "frame", "id": id, "x": 0.0, "y": y, "w": 80.0, "h": 40.0]]
    }

    /// A real touch (`--touch platform`) on a button of a confirmation aims
    /// at the button UIKit's alert shows for it, never at the hidden
    /// authored node, which every alert covered ("UIView covers its middle":
    /// a euxy row's rename prompt could not be driven).
    func testARealTouchAimsAtTheAlertsButtonForAConfirmationsRow() throws {
        let agent = mounted()
        let p = try XCTUnwrap(session).presenter
        let titles: [UInt32: String] = [3: "Keep", 9: "Cancel"]
        p.buttonFace = { id in var f = ButtonFace(); f.title = titles[id]; return f }
        p.apply(wireBatch([
            ["op": "create", "id": 2, "kind": "view", "handlers": [], "style": ["text_color": [0, 0, 0, 255]],
             "props": ["id": "confirm", "popover": "auto", "accessibilityRole": "alertdialog", "accessibilityLabel": "Keep?"]],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 200.0],
            ["op": "create", "id": 10, "kind": "text", "props": ["text": "A message."], "handlers": [], "style": ["text_color": [0, 0, 0, 255]]],
        ] + button(3, press: true, y: 40) + button(9, press: false, y: 80) + [
            ["op": "children", "id": 2, "ids": [10, 3, 9]], ["op": "roots", "ids": [1, 2]],
        ]))
        p.menus.action("showPopover", ["confirm"])
        let alert = try XCTUnwrap(root?.presentedViewController as? UIAlertController)
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "a presentation completes only in a scene's window")
        }
        let end = Date().addingTimeInterval(3)
        while p.menus.inTransition, Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
        alert.view.layoutIfNeeded()
        for (id, title) in [(9, "Cancel"), (3, "Keep")] {
            let reply = agent.aim(["id": id])
            if (reply["error"] as? String)?.contains("foreground") == true { throw missing("foreground", "the test host is in the background: a real touch's aim refuses it") }
            let aim = try XCTUnwrap(reply["aim"] as? [String: Any], "\(reply)")
            let native = try XCTUnwrap(aim["native"] as? [String: Any], "\(aim)")
            XCTAssertEqual(native["kind"] as? String, "alertAction")
            XCTAssertEqual(native["label"] as? String, title)
            let w = try XCTUnwrap(aim["window"] as? [Double])
            var hit = try XCTUnwrap(window).hitTest(CGPoint(x: w[0], y: w[1]), with: nil)
            while let view = hit, view.accessibilityLabel != title { hit = view.superview }
            XCTAssertNotNil(hit, "\(title): the point is on UIKit's \(title) button")
            XCTAssertTrue(hit?.isDescendant(of: alert.view) == true, "\(title): in the alert")
        }
    }
}
#endif
