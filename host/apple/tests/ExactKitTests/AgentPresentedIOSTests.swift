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

    func testANativeTreeCarriesTheRunnersPendingSnapshot() throws {
        let agent = mounted()
        let tree = agent.decorateTree(["nodes": []])
        let pending = try XCTUnwrap(tree["pending"] as? [[String: Any]], "the native tree must carry the snapshot used by missing-target diagnostics")
        XCTAssertEqual(pending.compactMap { $0["name"] as? String }, agent.pendingRequests().compactMap { $0["name"] as? String })
        XCTAssertTrue(pending.isEmpty, "this session starts with no source requests")
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


    // A real touch (`--touch platform`) on a row of a confirmation aims at
    // the button UIKit's alert shows for it, never at the hidden authored
    // node, which every alert covered ("UIView covers its middle": a euxy
    // row's rename prompt could not be driven).

    private var titles: [UInt32: String] = [:]
    /// The popover `confirm`, labelled `label`, with the text rows `texts`
    /// (10…), a native action row per title in `actions` (3…) and a
    /// hide-only Cancel (9), under the agent `mounted()` made.
    private func confirmation(label: String, texts: [String], actions: [String]) throws -> Presenter {
        let p = try XCTUnwrap(session).presenter
        titles = [9: "Cancel"]
        for (i, title) in actions.enumerated() { titles[UInt32(3 + i)] = title }
        p.buttonFace = { [unowned self] id in var f = ButtonFace(); f.title = self.titles[id]; return f }
        let ink: [String: Any] = ["text_color": [0, 0, 0, 255]]
        func button(_ id: Int, press: Bool) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "control", "handlers": press ? ["press"] : [],
              "props": ["type": "button", "accessibilityRole": "button", "popovertarget": "confirm", "popovertargetaction": "hide"],
              "style": ink.merging(["appearance": "auto"]) { $1 }],
             ["op": "frame", "id": id, "x": 0.0, "y": Double(40 * id), "w": 80.0, "h": 40.0]]
        }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 2, "kind": "view", "handlers": [], "style": ink,
             "props": ["id": "confirm", "popover": "auto", "accessibilityRole": "alertdialog", "accessibilityLabel": label]],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 600.0]]
        let textIds = texts.indices.map { 10 + $0 }, actionIds = actions.indices.map { 3 + $0 }
        for (id, text) in zip(textIds, texts) { ops.append(["op": "create", "id": id, "kind": "text", "props": ["text": text], "handlers": [], "style": ink]) }
        for id in actionIds { ops += button(id, press: true) }
        ops += button(9, press: false)
        ops += [["op": "children", "id": 2, "ids": textIds + actionIds + [9]], ["op": "roots", "ids": [1, 2]]]
        p.apply(wireBatch(ops))
        return p
    }
    private func settle(_ p: Presenter) {
        let end = Date().addingTimeInterval(3)
        while p.menus.inTransition, Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    /// The aim's reply for a plain real tap on `id`.
    private func aim(_ agent: Agent, _ id: Int) throws -> [String: Any] {
        let reply = agent.aim(["id": id])
        if (reply["error"] as? String)?.contains("foreground") == true { throw missing("foreground", "the test host is in the background: a real touch's aim refuses it") }
        return reply
    }
    /// The `UIAlertAction` whose button a finger at the aim's point presses.
    private func pressed(_ reply: [String: Any]) throws -> UIAlertAction? {
        let aim = try XCTUnwrap(reply["aim"] as? [String: Any], "\(reply)")
        let w = try XCTUnwrap(aim["window"] as? [Double])
        var hit = try XCTUnwrap(window).hitTest(CGPoint(x: w[0], y: w[1]), with: nil)
        while let view = hit, !NSStringFromClass(type(of: view)).hasPrefix("_UIAlertControllerActionView") { hit = view.superview }
        return hit?.value(forKey: "action") as? UIAlertAction
    }

    /// Two actions the alert titles alike, and the alert's own title the
    /// same: each row presses its own button, in the order the rows were given.
    func testARealTouchPressesTheAlertsOwnButtonForEachRowAlikeTitlesOrNot() throws {
        let agent = mounted()
        let p = try confirmation(label: "Photos", texts: ["Where should it go?"], actions: ["Photos", "Photos"])
        p.menus.action("showPopover", ["confirm"])
        let alert = try XCTUnwrap(root?.presentedViewController as? UIAlertController)
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "a presentation completes only in a scene's window")
        }
        settle(p)
        alert.view.layoutIfNeeded()
        XCTAssertEqual(alert.preferredStyle, .alert)
        for (id, index) in [(3, 0), (4, 1), (9, 2)] {
            let reply = try aim(agent, id)
            let native = try XCTUnwrap((reply["aim"] as? [String: Any])?["native"] as? [String: Any], "\(reply)")
            XCTAssertEqual(native["kind"] as? String, "alertAction")
            XCTAssertEqual(native["label"] as? String, titles[UInt32(id)])
            XCTAssertTrue(try pressed(reply) === alert.actions[index], "row \(id) presses the alert's button \(index)")
        }
    }

    /// The cancel's title is the one the alert shows: a cancel that now says
    /// something else ends the alert, as an action's does.
    func testACancelRetitledEndsTheAlert() throws {
        let agent = mounted()
        let p = try confirmation(label: "Keep?", texts: ["A message."], actions: ["Keep"])
        p.menus.action("showPopover", ["confirm"])
        let alert = try XCTUnwrap(root?.presentedViewController as? UIAlertController)
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "a presentation completes only in a scene's window")
        }
        settle(p)
        XCTAssertEqual(alert.actions.last?.title, "Cancel")
        XCTAssertEqual((try aim(agent, 9)["aim"] as? [String: Any]).flatMap { $0["native"] as? [String: Any] }?["label"] as? String, "Cancel")
        titles[9] = "Close"
        p.apply(facesBatch())
        XCTAssertFalse(p.menus.isOpen(try XCTUnwrap(p.views[2])), "the alert's Cancel no longer says what the page does")
        let end = Date().addingTimeInterval(3)
        while root?.presentedViewController != nil, Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
        XCTAssertNil(root?.presentedViewController, "and UIKit's alert is gone")
    }

    /// An alert still arriving or leaving is no place for a finger: the aim
    /// says so rather than aiming where its button will be, or was.
    func testAnAlertStillMovingRefusesTheAim() throws {
        let agent = mounted()
        let p = try confirmation(label: "Keep?", texts: ["A message."], actions: ["Keep"])
        p.menus.action("showPopover", ["confirm"])
        let alert = try XCTUnwrap(root?.presentedViewController as? UIAlertController)
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "a presentation completes only in a scene's window")
        }
        if alert.isBeingPresented {
            XCTAssertTrue((try aim(agent, 3)["error"] as? String)?.contains("still moving") == true, "arriving")
        }
        settle(p)
        XCTAssertNotNil(try aim(agent, 3)["aim"], "arrived")
        p.menus.action("hidePopover", ["confirm"])
        XCTAssertTrue((try aim(agent, 9)["error"] as? String)?.contains("still moving") == true, "leaving")
    }

    /// A chooser at regular width is a popover, which UIKit draws without its
    /// cancel: a tap on the cancel is refused, naming the tap outside.
    func testAPopoverChoosersCancelIsRefusedNamingTheTapOutside() throws {
        let agent = mounted()
        let p = try confirmation(label: "Open in", texts: [], actions: ["Maps", "Waze"])
        try XCTUnwrap(root).traitOverrides.horizontalSizeClass = .regular
        p.menus.action("showPopover", ["confirm"])
        let alert = try XCTUnwrap(root?.presentedViewController as? UIAlertController)
        guard UIApplication.shared.connectedScenes.contains(where: { $0 is UIWindowScene }) else {
            throw missing("scene", "a presentation completes only in a scene's window")
        }
        try XCTSkipUnless(p.menus.observation()?["presentation"] as? String == "popover", "the regular width took")
        settle(p)
        XCTAssertTrue((try aim(agent, 9)["error"] as? String)?.contains("tap outside") == true)
        XCTAssertTrue(try pressed(try aim(agent, 4)) === alert.actions[1], "an action still aims at its button")
    }
}
#endif
