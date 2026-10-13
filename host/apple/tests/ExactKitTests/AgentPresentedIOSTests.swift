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
}
#endif
