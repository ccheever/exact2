#if os(iOS)
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists
import UIKit

private final class CheckScrollDecoration: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        point.y < 100 ? super.hitTest(point, with: event) : nil
    }
}

/// `check` on UIKit (LLP 1116.003 D8): the observations the driver judges —
/// a paragraph's ink and whether it fits its box, a control covered by
/// another node, a target the author sized small, a view past the viewport's
/// side, a card's fill without an edge (and what is no card), a disabled
/// control's label, two controls overlapping — read from the presented view
/// tree.
final class AgentCheckIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func mounted(_ ops: [[String: Any]]) -> Agent {
        let session = ExactApp.shared.makeSession(label: "agent-check")
        self.session = session
        let p = session.presenter
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        p.apply(wireBatch(ops))
        p.viewport.layoutIfNeeded()
        return Agent(session: session)
    }

    func testFrameHealthControlsAreExcludedFromAppOcclusionFindings() throws {
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "button", "props": ["testId": "under-hud"], "handlers": ["press"]],
            ["op": "frame", "id": 1, "x": 20.0, "y": 20.0, "w": 100.0, "h": 44.0],
            ["op": "roots", "ids": [1]]])
        let session = try XCTUnwrap(session), window = try XCTUnwrap(window)
        let overlay = FrameHealthOverlay(health: session.frameHealth)
        overlay.frame = window.bounds
        window.rootViewController?.view.addSubview(overlay)
        overlay.setVisible(true)
        overlay.layoutIfNeeded()
        // The button under the card's drag handle, wherever the window's safe area puts the card (below the
        // status bar on a phone, so no fixed point lies on it)
        let at = overlay.handle.convert(CGPoint(x: overlay.handle.bounds.midX, y: overlay.handle.bounds.midY), to: window)
        session.presenter.apply(wireBatch([["op": "frame", "id": 1, "x": Double(at.x) - 50, "y": Double(at.y) - 22, "w": 100.0, "h": 44.0]]))
        session.presenter.viewport.layoutIfNeeded()
        XCTAssertTrue(window.hitTest(at, with: nil)?.isDescendant(of: overlay) == true)
        func observed() throws -> [String: Any] {
            let items = try XCTUnwrap(agent.check([:])["items"] as? [[String: Any]])
            return try XCTUnwrap(items.first { $0["id"] as? Int == 1 })
        }
        XCTAssertNil(try observed()["cover"], "host apparatus does not become an app defect")
        overlay.accessibilityIdentifier = "test.ordinary-overlay"
        XCTAssertNotNil(try observed()["cover"], "an ordinary covering control remains a finding")
    }

    /// A native button's title is the button's label (judged at 3:1 on its own fill, D3's ink rule) in a
    /// grouped list's cell as anywhere: the cell's part is reported for every view inside it, so it must not
    /// claim the title first (countdowns' indigo prominent button in dark read 3.5:1 against 4.5:1).
    func testANativeButtonsTitleInAGroupedRowIsItsLabel() throws {
        let saved = GroupedListsLink.part
        // As the grouped lists module reports a cell: row 99 for every view inside the row's node.
        GroupedListsLink.part = { view in
            var at = view
            while let v = at { if (v as? NodeView)?.props["testId"] == "row" { return ["row": 99, "part": "cell"] }; at = v.superview }
            return nil
        }
        defer { GroupedListsLink.part = saved }
        let session = ExactApp.shared.makeSession(label: "agent-check-row")
        self.session = session
        let p = session.presenter
        var face = ButtonFace(); face.title = "Prominent"; face.style = "bordered-prominent"; face.ios = "borderedProminent"; face.iosBefore26 = "borderedProminent"
        p.buttonFace = { _ in face }
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": 2, "kind": "view", "props": ["testId": "row"]],
            ["op": "create", "id": 3, "kind": "control", "handlers": ["press"], "props": ["type": "button", "accessibilityRole": "button", "testId": "prominent"],
             "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 3, "x": 16.0, "y": 8.0, "w": 140.0, "h": 44.0],
        ]))
        p.viewport.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        let reply = Agent(session: session).check([:])
        XCTAssertNil(reply["error"], "\(reply)")
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]])
        let title = try XCTUnwrap(items.first { $0["text"] as? String == "Prominent" }, "the title is observed: \(items)")
        XCTAssertEqual(title["label"] as? Bool, true, "a control's label, not the row's text: \(title)")
        XCTAssertEqual(title["of"] as? Int, 3, "of the button, not of row 99: \(title)")
        XCTAssertEqual(title["onAccent"] as? Bool, true, "UIKit's ink on the accent's fill (LLP 1116.019 D3): \(title)")
    }

    /// LLP 1116.008 D7: a `select` is UIKit's pop-up button, its value the button's title, drawn in the
    /// accent UIKit tints it with: the platform's ink for the control, judged at 3:1 as a button's title
    /// is, in a grouped list's cell as anywhere (countdowns' and expense-tracker's system indigo in dark
    /// read 3.9:1 against 4.5:1, as body text of the row). A text of the row's own stays the row's.
    func testASelectsValueInAGroupedRowIsItsControlsLabel() throws {
        let saved = GroupedListsLink.part
        GroupedListsLink.part = { view in
            var at = view
            while let v = at { if (v as? NodeView)?.props["testId"] == "row" { return ["row": 99, "part": "cell"] }; at = v.superview }
            return nil
        }
        defer { GroupedListsLink.part = saved }
        let session = ExactApp.shared.makeSession(label: "agent-check-select")
        self.session = session
        let p = session.presenter
        p.selectOptions = { _ in SelectMenu(options: [.init(value: "party", label: "Party", disabled: false), .init(value: "star", label: "Star", disabled: false)], chosen: 0) }
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        var batch = wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": 2, "kind": "view", "props": ["testId": "row"]],
            ["op": "create", "id": 3, "kind": "control", "props": ["type": "select", "testId": "emoji"], "handlers": ["input", "change"], "style": [:]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Emoji"], "style": [:]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [4, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 100.0, "w": 400.0, "h": 52.0],
            ["op": "frame", "id": 4, "x": 16.0, "y": 14.0, "w": 120.0, "h": 24.0],
            ["op": "frame", "id": 3, "x": 244.0, "y": 0.0, "w": 140.0, "h": 52.0],
        ])
        batch.controls = true
        p.apply(batch)
        p.viewport.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        let reply = Agent(session: session).check([:])
        XCTAssertNil(reply["error"], "\(reply)")
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]])
        let value = try XCTUnwrap(items.first { $0["text"] as? String == "Party" }, "the value is observed: \(items)")
        XCTAssertEqual(value["label"] as? Bool, true, "the control's label, not the row's text: \(value)")
        XCTAssertEqual(value["of"] as? Int, 3, "of the select, not of row 99: \(value)")
        let title = try XCTUnwrap(items.first { $0["text"] as? String == "Emoji" }, "the row's own text is observed: \(items)")
        XCTAssertNil(title["label"], "a text of the row's own stays body text: \(title)")
    }

    /// LLP 1116.020 D7: a text on a fill of the accent (a sent bubble) is observed as on it, so the judge
    /// holds white there to a filled button's 3:1; on a fill of its own colour it is not.
    func testATextOnTheAccentsFillIsObservedSo() throws {
        let tint: [String: Any] = ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": 2, "kind": "view", "style": ["background_color": tint]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Sent"], "style": ["text_color": [255, 255, 255, 255], "font_size": 17]],
            ["op": "create", "id": 4, "kind": "view", "style": ["background_color": [0, 136, 255, 255]]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Painted"], "style": ["text_color": [255, 255, 255, 255], "font_size": 17]],
            ["op": "children", "id": 1, "ids": [2, 4]], ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 4, "ids": [5]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 16.0, "y": 20.0, "w": 120.0, "h": 36.0],
            ["op": "frame", "id": 3, "x": 12.0, "y": 8.0, "w": 96.0, "h": 20.0],
            ["op": "frame", "id": 4, "x": 16.0, "y": 80.0, "w": 120.0, "h": 36.0],
            ["op": "frame", "id": 5, "x": 12.0, "y": 8.0, "w": 96.0, "h": 20.0],
        ])
        let items = try XCTUnwrap(agent.check([:])["items"] as? [[String: Any]])
        let sent = try XCTUnwrap(items.first { $0["text"] as? String == "Sent" }, "\(items)")
        XCTAssertEqual(sent["onAccentFill"] as? Bool, true, "\(sent)")
        let painted = try XCTUnwrap(items.first { $0["text"] as? String == "Painted" }, "\(items)")
        XCTAssertNil(painted["onAccentFill"], "\(painted)")
    }

    func testCheckObservesInkFitCoverTargetsOverflowCardsAndOverlap() throws {
        let white: [Double] = [255, 255, 255, 255]
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": white]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Faint", "testId": "faint"], "style": ["text_color": [187, 187, 187, 255], "font_size": 17]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "A long label that will not fit in its box"], "style": ["overflow_x": "hidden", "overflow_y": "hidden", "font_size": 17]],
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"], "props": ["testId": "under"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["testId": "cover"], "style": ["background_color": [200, 200, 200, 255]]],
            ["op": "create", "id": 6, "kind": "view", "handlers": ["press"], "props": ["testId": "tiny"]],
            ["op": "create", "id": 7, "kind": "view", "props": ["testId": "wide"]],
            ["op": "create", "id": 8, "kind": "view", "props": ["testId": "card"], "style": ["background_color": white, "border_radius_top_left": 12, "border_radius_top_right": 12, "border_radius_bottom_right": 12, "border_radius_bottom_left": 12]],
            ["op": "create", "id": 9, "kind": "view", "handlers": ["press"], "props": ["testId": "a"]],
            ["op": "create", "id": 10, "kind": "view", "handlers": ["press"], "props": ["testId": "b"]],
            ["op": "create", "id": 11, "kind": "text", "props": ["text": "In a card"], "style": ["font_size": 17]],
            ["op": "create", "id": 12, "kind": "view", "props": ["testId": "shape"], "style": ["background_color": white, "border_radius_top_left": 40, "border_radius_top_right": 40, "border_radius_bottom_right": 40, "border_radius_bottom_left": 40]],
            ["op": "create", "id": 13, "kind": "view", "handlers": ["press"], "props": ["testId": "previous", "disabled": "true"]],
            ["op": "create", "id": 14, "kind": "text", "props": ["text": "Previous"], "style": ["text_color": [187, 187, 187, 255], "font_size": 17]],
            ["op": "create", "id": 15, "kind": "view", "handlers": ["press"], "props": ["testId": "next"]],
            ["op": "create", "id": 16, "kind": "text", "props": ["text": "Next"], "style": ["font_size": 17]],
            ["op": "create", "id": 17, "kind": "list", "props": ["testId": "list"], "style": ["background_color": white, "border_radius_top_left": 12, "border_radius_top_right": 12, "border_radius_bottom_right": 12, "border_radius_bottom_left": 12]],
            ["op": "create", "id": 18, "kind": "text", "props": ["text": "In a list"], "style": ["font_size": 17]],
            ["op": "create", "id": 19, "kind": "view", "props": ["testId": "hiding"], "style": ["background_color": white, "border_radius_top_left": 12, "border_radius_top_right": 12, "border_radius_bottom_right": 12, "border_radius_bottom_left": 12]],
            ["op": "create", "id": 20, "kind": "text", "props": ["text": "Hidden"], "style": ["font_size": 17, "visibility": "hidden"]],
            ["op": "create", "id": 21, "kind": "view", "props": ["testId": "veil"], "style": ["background_color": white]],
            ["op": "create", "id": 22, "kind": "view", "props": ["testId": "veiled"], "style": ["background_color": white, "border_radius_top_left": 12, "border_radius_top_right": 12, "border_radius_bottom_right": 12, "border_radius_bottom_left": 12]],
            ["op": "create", "id": 23, "kind": "text", "props": ["text": "Veiled"], "style": ["font_size": 17]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13, 15, 17, 19, 21]],
            ["op": "children", "id": 19, "ids": [20]],
            ["op": "children", "id": 21, "ids": [22]],
            ["op": "children", "id": 22, "ids": [23]],
            ["op": "children", "id": 8, "ids": [11]],
            ["op": "children", "id": 17, "ids": [18]],
            ["op": "children", "id": 13, "ids": [14]],
            ["op": "children", "id": 15, "ids": [16]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 16.0, "y": 20.0, "w": 60.0, "h": 22.0],
            ["op": "frame", "id": 3, "x": 16.0, "y": 60.0, "w": 60.0, "h": 22.0],
            ["op": "frame", "id": 4, "x": 16.0, "y": 100.0, "w": 120.0, "h": 44.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 90.0, "w": 400.0, "h": 70.0],
            ["op": "frame", "id": 6, "x": 16.0, "y": 180.0, "w": 20.0, "h": 20.0],
            ["op": "frame", "id": 7, "x": 16.0, "y": 220.0, "w": 600.0, "h": 20.0],
            ["op": "frame", "id": 8, "x": 16.0, "y": 260.0, "w": 368.0, "h": 80.0],
            ["op": "frame", "id": 9, "x": 16.0, "y": 360.0, "w": 100.0, "h": 44.0],
            ["op": "frame", "id": 10, "x": 96.0, "y": 360.0, "w": 100.0, "h": 44.0],
            ["op": "frame", "id": 11, "x": 12.0, "y": 8.0, "w": 200.0, "h": 22.0],
            ["op": "frame", "id": 12, "x": 16.0, "y": 420.0, "w": 80.0, "h": 80.0],
            ["op": "frame", "id": 13, "x": 120.0, "y": 420.0, "w": 120.0, "h": 44.0],
            ["op": "frame", "id": 14, "x": 8.0, "y": 11.0, "w": 100.0, "h": 22.0],
            ["op": "frame", "id": 15, "x": 260.0, "y": 420.0, "w": 120.0, "h": 44.0],
            ["op": "frame", "id": 16, "x": 8.0, "y": 11.0, "w": 100.0, "h": 22.0],
            ["op": "frame", "id": 17, "x": 16.0, "y": 520.0, "w": 368.0, "h": 80.0],
            ["op": "frame", "id": 18, "x": 12.0, "y": 8.0, "w": 200.0, "h": 22.0],
            ["op": "frame", "id": 19, "x": 16.0, "y": 680.0, "w": 80.0, "h": 60.0],
            ["op": "frame", "id": 20, "x": 8.0, "y": 8.0, "w": 60.0, "h": 22.0],
            ["op": "frame", "id": 21, "x": 120.0, "y": 680.0, "w": 260.0, "h": 80.0],
            ["op": "frame", "id": 22, "x": 8.0, "y": 8.0, "w": 200.0, "h": 60.0],
            ["op": "frame", "id": 23, "x": 8.0, "y": 8.0, "w": 150.0, "h": 22.0],
        ])
        try XCTUnwrap(session).presenter.views[21]?.alpha = 0.5
        // UIKit's own disabled button, whose title is UIKit's label.
        let done = UIButton(type: .system)
        done.setTitle("Done", for: .normal)
        done.isEnabled = false
        done.frame = CGRect(x: 16, y: 620, width: 120, height: 44)
        try XCTUnwrap(session).presenter.views[1]?.addSubview(done)
        done.layoutIfNeeded()
        let reply = agent.check([:])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["target"] as? Int, 44)
        XCTAssertEqual(reply["scheme"] as? String, "light")
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]])
        func item(_ id: Int) throws -> [String: Any] { try XCTUnwrap(items.first { $0["id"] as? Int == id }, "#\(id) in \(items)") }
        let faint = try item(2)
        XCTAssertEqual(faint["text"] as? String, "Faint")
        XCTAssertEqual((faint["fg"] as? [Double])?.prefix(3).map { $0 }, [187, 187, 187], "the paragraph's ink: \(faint)")
        XCTAssertEqual(faint["size"] as? Double, 17)
        XCTAssertNil(faint["over"], "it fits")
        let clipped = try item(3)
        let over = try XCTUnwrap(clipped["over"] as? [Double], "a wrapped label taller than its box: \(clipped)")
        XCTAssertGreaterThan(over[1], 2)
        XCTAssertEqual(clipped["clips"] as? Bool, true)
        let under = try item(4)
        let cover = try XCTUnwrap(under["cover"] as? [String: Any], "the cover over its middle: \(under)")
        XCTAssertEqual(cover["by"] as? Int, 5)
        XCTAssertEqual(cover["kind"] as? String, "node")
        let tiny = try item(6)
        XCTAssertEqual(tiny["control"] as? String, "press")
        XCTAssertEqual(tiny["sized"] as? String, "author")
        XCTAssertEqual(tiny["w"] as? Double, 20)
        XCTAssertEqual(try item(7)["outX"] as? Double, 216, "600 wide from 16, 216 past a 400-wide viewport")
        let card = try item(8)
        XCTAssertEqual(card["fill"] as? [Double], [255, 255, 255, 1])
        XCTAssertEqual(card["radius"] as? Double, 12)
        XCTAssertNil(card["edge"], "no border, no shadow")
        XCTAssertNil(items.first { $0["id"] as? Int == 12 }, "a shape that holds no text or control is no card: \(items)")
        XCTAssertNil(items.first { $0["id"] as? Int == 17 }?["fill"], "a list's ground is the page under its rows, no card: \(items)")
        XCTAssertNil(items.first { $0["id"] as? Int == 19 }, "nor is a shape whose only text is hidden: \(items)")
        XCTAssertEqual(try item(22)["faded"] as? Bool, true, "a card in a half-opaque view mixes with what lies behind that view")
        XCTAssertNil(card["faded"])
        XCTAssertEqual(try item(14)["of"] as? Int, 13)
        XCTAssertEqual(try item(14)["inactive"] as? Bool, true, "a disabled control's label is an inactive component's text")
        XCTAssertNil(try item(16)["inactive"], "an enabled one's is judged")
        XCTAssertEqual(items.first { $0["text"] as? String == "Done" }?["inactive"] as? Bool, true, "UIKit's disabled button's title too: \(items)")
        XCTAssertEqual(try item(9)["control"] as? String, "press")
        XCTAssertEqual(try item(10)["x"] as? Double, 96, "b starts inside a: the driver finds them overlapping")
    }

    /// LLP 1116.009 D5 (focus-timer, recipe-finder, round 11): a box whose content fits its parent and whose written padding
    /// (content-box sizing) pushes it past the parent's content edge is observed, with the cause; border-box, a box that fits,
    /// padding the author did not write, and a parent that scrolls sideways on purpose are not. The presenter is never told a
    /// width, so the cause is read from the frames.
    func testAPaddedBoxPastItsParentIsObservedWithItsCause() throws {
        let fill: [Double] = [200, 200, 200, 255]
        let written: [String: Any] = ["written": ["padding_left", "padding_right"]]
        // A press handler keeps each box a view of its own (an inert leaf box is a layer in its parent's, LLP 1068 §6.1).
        func box(_ id: Int, _ style: [String: Any]) -> [String: Any] { ["op": "create", "id": id, "kind": "view", "handlers": ["press"], "style": style.merging(["background_color": fill]) { a, _ in a }] }
        let column: [String: Any] = ["display": "flex", "padding_left": 16.0, "padding_right": 16.0]
        let pads: [String: Any] = ["padding_left": 24.0, "padding_right": 24.0]
        let agent = mounted([
            box(1, [:]),
            box(10, column), box(2, pads.merging(written) { a, _ in a }),
            box(3, pads.merging(written) { a, _ in a }.merging(["box_sizing": "border-box"]) { a, _ in a }),
            box(4, pads.merging(written) { a, _ in a }), box(5, pads),
            box(11, [:]), box(6, pads.merging(written) { a, _ in a }),
            box(12, ["overflow_x": "auto", "written": ["overflow_x"]]), box(7, pads.merging(written) { a, _ in a }),
            box(13, ["overflow_x": "auto"]), box(8, pads.merging(written) { a, _ in a }), // a `scroll`'s own auto is the platform's
            ["op": "children", "id": 1, "ids": [10, 11, 12, 13]],
            ["op": "children", "id": 10, "ids": [2, 3, 4, 5]],
            ["op": "children", "id": 11, "ids": [6]],
            ["op": "children", "id": 12, "ids": [7]],
            ["op": "children", "id": 13, "ids": [8]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 16.0, "y": 0.0, "w": 416.0, "h": 40.0],   // 368 of content and 48 of padding in 368
            ["op": "frame", "id": 3, "x": 16.0, "y": 50.0, "w": 368.0, "h": 40.0],  // border-box: it fits
            ["op": "frame", "id": 4, "x": 16.0, "y": 100.0, "w": 200.0, "h": 40.0], // room to spare
            ["op": "frame", "id": 5, "x": 16.0, "y": 150.0, "w": 416.0, "h": 40.0], // the padding is not the author's
            ["op": "frame", "id": 11, "x": 0.0, "y": 420.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": 0.0, "w": 400.0, "h": 40.0],    // fits
            ["op": "frame", "id": 12, "x": 0.0, "y": 500.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 7, "x": 0.0, "y": 0.0, "w": 448.0, "h": 40.0],    // past, in a pager the author scrolls
            ["op": "frame", "id": 13, "x": 0.0, "y": 580.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 8, "x": 0.0, "y": 0.0, "w": 448.0, "h": 40.0],    // past, in a view that scrolls by default
            contentOp(10, 400, 400, padding: [0, 16, 0, 16]),
        ] + [(2, 416.0), (3, 368), (4, 200), (5, 416), (6, 400), (7, 448), (8, 448)].map { contentOp($0, $1, 40, padding: [0, 24, 0, 24]) })
        let reply = agent.check([:])
        XCTAssertNil(reply["error"], "\(reply)")
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]])
        let padded = try XCTUnwrap(items.first { $0["id"] as? Int == 2 }?["padded"] as? [String: Any], "#2 in \(items)")
        XCTAssertEqual(padded["by"] as? Int, 10)
        XCTAssertEqual(padded["dx"] as? Double, 48, "416 wide in a 368-wide content box")
        XCTAssertEqual(padded["pad"] as? Double, 48)
        XCTAssertNil(padded["width"], "the presenter is told no width")
        XCTAssertEqual((items.first { $0["id"] as? Int == 8 }?["padded"] as? [String: Any])?["by"] as? Int, 13, "#8 in \(items)")
        for id in [3, 4, 5, 6, 7] { XCTAssertNil(items.first { $0["id"] as? Int == id }?["padded"], "#\(id) is not padding past its parent") }
    }

    /// LLP 1116.020 D4 (focus-timer and trivia-quiz, round 18): a vertical scroll view holds no sideways overflow, its x
    /// `auto` by CSS's rule and not its author's, so a box past the screen's side inside it is observed; a scroll whose
    /// author wrote its `overflow-x` (a carousel, a swipe row) and an authored clip hold theirs.
    func testABoxPastTheScreenInAVerticalScrollIsObserved() throws {
        func box(_ id: Int, _ style: [String: Any]) -> [String: Any] { ["op": "create", "id": id, "kind": "view", "handlers": ["press"], "style": style] }
        let agent = mounted([
            box(1, [:]),
            box(10, ["overflow_x": "auto", "overflow_y": "auto"]), box(2, [:]),
            box(11, ["overflow_x": "auto", "overflow_y": "hidden", "written": ["overflow_x", "overflow_y"]]), box(3, [:]),
            box(12, ["overflow_x": "hidden", "written": ["overflow_x"]]), box(4, [:]),
            ["op": "children", "id": 1, "ids": [10, 11, 12]],
            ["op": "children", "id": 10, "ids": [2]],
            ["op": "children", "id": 11, "ids": [3]],
            ["op": "children", "id": 12, "ids": [4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 16.0, "y": 0.0, "w": 430.0, "h": 40.0],   // 46 past the screen in a vertical scroll
            ["op": "frame", "id": 11, "x": 0.0, "y": 220.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 3, "x": 16.0, "y": 0.0, "w": 430.0, "h": 40.0],   // in a strip that scrolls sideways
            ["op": "frame", "id": 12, "x": 0.0, "y": 300.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 4, "x": 16.0, "y": 0.0, "w": 430.0, "h": 40.0],   // in an authored clip
        ])
        let reply = agent.check([:])
        XCTAssertNil(reply["error"], "\(reply)")
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]])
        XCTAssertEqual(items.first { $0["id"] as? Int == 2 }?["outX"] as? Double, 46, "#2 in \(items)")
        for id in [3, 4] { XCTAssertNil(items.first { $0["id"] as? Int == id }?["outX"], "#\(id) is held: \(items)") }
    }

    /// Round 21's event-rsvp at the largest text size: a centred caption wider than its flex column runs past both of
    /// the column's sides (`spill`, the web's rule), and where its line is drawn (`ink`) is observed where that is not its
    /// box, so the driver finds two texts running into each other; one in a column that clips is held.
    func testATextPastItsColumnAndWhereItsLineIsDrawnAreObserved() throws {
        let caption: [String: Any] = ["font_size": 17, "text_align": "center"]
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["press"], "style": ["display": "flex"]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Going"], "style": caption],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Maybe"], "style": caption],
            ["op": "create", "id": 5, "kind": "view", "handlers": ["press"], "style": ["display": "flex", "overflow_x": "hidden", "overflow_y": "hidden"]],
            ["op": "create", "id": 6, "kind": "text", "props": ["text": "Held"], "style": caption],
            ["op": "children", "id": 1, "ids": [2, 4, 5]], ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 5, "ids": [6]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 40.0, "y": 100.0, "w": 20.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": -10.0, "y": 0.0, "w": 40.0, "h": 22.0],   // 10 past each side of its column
            ["op": "frame", "id": 4, "x": 16.0, "y": 200.0, "w": 368.0, "h": 22.0],  // a short word centred in a wide box
            ["op": "frame", "id": 5, "x": 40.0, "y": 300.0, "w": 20.0, "h": 40.0],
            ["op": "frame", "id": 6, "x": -10.0, "y": 0.0, "w": 40.0, "h": 22.0],
        ])
        let reply = agent.check([:])
        XCTAssertNil(reply["error"], "\(reply)")
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]])
        func item(_ id: Int) throws -> [String: Any] { try XCTUnwrap(items.first { $0["id"] as? Int == id }, "#\(id) in \(items)") }
        let spill = try XCTUnwrap(try item(3)["spill"] as? [String: Any], "\(items)")
        XCTAssertEqual(spill["by"] as? Int, 2)
        XCTAssertEqual(spill["dx"] as? Double, 10)
        let ink = try XCTUnwrap(try item(4)["ink"] as? [String: Any], "\(items)")
        let x = try XCTUnwrap(ink["x"] as? Double), w = try XCTUnwrap(ink["w"] as? Double)
        XCTAssertLessThan(w, 100, "the word, not its box: \(ink)")
        XCTAssertEqual(x + w / 2, 200, accuracy: 1, "centred in its box: \(ink)")
        XCTAssertNil(try item(6)["spill"], "a column that clips holds it")
    }

    /// LLP 1116.006 D5: a text stands on the nearest fill painted under it —
    /// its own box's (a rounded bubble, whose corners show the page), a
    /// bubble column's, the page's — in each scheme; and a row its scroller
    /// cuts is presented only to the scroller's edge, so it overlaps nothing
    /// past it (round 8's chat bubbles and contacts' rows).
    func testATextStandsOnTheFillUnderItAndARowItsScrollerCutsEndsThere() throws {
        let indigo: [[Double]] = [[88, 86, 214, 255], [142, 140, 240, 255]]
        let ink: [[Double]] = [[0, 0, 0, 255], [255, 255, 255, 255]]
        let round: [String: Any] = ["border_radius_top_left": 18, "border_radius_top_right": 18, "border_radius_bottom_right": 18, "border_radius_bottom_left": 18]
        let white: [Double] = [255, 255, 255, 255]
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Text on the page"], "style": ["text_color": ink, "font_size": 17]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Own box"], "style": round.merging(["background_color": indigo, "text_color": white, "font_size": 17, "padding_left": 12, "padding_top": 8]) { x, _ in x }],
            ["op": "create", "id": 4, "kind": "view", "style": round.merging(["background_color": indigo]) { x, _ in x }],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "In a bubble"], "style": ["text_color": white, "font_size": 17]],
            ["op": "create", "id": 6, "kind": "text", "props": ["text": "Label on a light bubble"], "style": round.merging(["background_color": [233, 233, 235, 255], "text_color": ink, "font_size": 17, "padding_left": 12, "padding_top": 8]) { x, _ in x }],
            ["op": "create", "id": 7, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 8, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 9, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 10, "kind": "view", "handlers": ["press"], "props": ["testId": "row3"]],
            ["op": "create", "id": 11, "kind": "view", "handlers": ["press"], "props": ["testId": "tab"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 6, 7, 11]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 7, "ids": [8, 9, 10]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 16.0, "y": 60.0, "w": 200.0, "h": 22.0],
            ["op": "frame", "id": 3, "x": 16.0, "y": 100.0, "w": 120.0, "h": 38.0],
            ["op": "frame", "id": 4, "x": 16.0, "y": 150.0, "w": 150.0, "h": 38.0],
            ["op": "frame", "id": 5, "x": 12.0, "y": 8.0, "w": 120.0, "h": 22.0],
            ["op": "frame", "id": 6, "x": 16.0, "y": 200.0, "w": 240.0, "h": 38.0],
            ["op": "frame", "id": 7, "x": 0.0, "y": 400.0, "w": 400.0, "h": 100.0],
            ["op": "content", "id": 7, "w": 400.0, "h": 132.0],
            ["op": "frame", "id": 8, "x": 0.0, "y": 0.0, "w": 400.0, "h": 44.0],
            ["op": "frame", "id": 9, "x": 0.0, "y": 44.0, "w": 400.0, "h": 44.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 88.0, "w": 400.0, "h": 44.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 500.0, "w": 400.0, "h": 44.0],
        ])
        func observe() throws -> [Int: [String: Any]] {
            let reply = agent.check([:])
            let items = try XCTUnwrap(reply["items"] as? [[String: Any]], "\(reply)")
            return Dictionary(items.compactMap { i in (i["id"] as? Int).map { ($0, i) } }) { a, _ in a }
        }
        func bg(_ item: [String: Any]?) -> [Double]? { item?["bg"] as? [Double] }
        let light = try observe()
        XCTAssertEqual(bg(light[2]), [255, 255, 255], "on the page, the platform's background: \(String(describing: light[2]))")
        XCTAssertEqual(light[2]?["on"] as? String, "page")
        XCTAssertEqual(bg(light[3]), [88, 86, 214], "its own box's fill, though the corners show the page: \(String(describing: light[3]))")
        XCTAssertEqual(light[3]?["on"] as? String, "own")
        XCTAssertEqual(bg(light[5]), [88, 86, 214], "the bubble's fill: \(String(describing: light[5]))")
        XCTAssertEqual(light[5]?["on"] as? Int, 4)
        XCTAssertEqual(bg(light[6]), [233, 233, 235])
        // The row past its scroller's bottom is presented to that edge: it overlaps nothing below.
        let row = try XCTUnwrap(light[10], "\(light)")
        let vis = try XCTUnwrap(row["vis"] as? [String: Any], "the scroller cuts it: \(row)")
        XCTAssertEqual(vis["y"] as? Double, 488)
        XCTAssertEqual(vis["h"] as? Double, 12)
        XCTAssertNil(row["cover"], "its scroller brings it in: \(row)")
        XCTAssertNil(light[11]?["vis"], "the tab is whole")

        window?.overrideUserInterfaceStyle = .dark
        window?.layoutIfNeeded()
        let dark = try observe()
        XCTAssertEqual(bg(dark[2]), [0, 0, 0], "the dark page: \(String(describing: dark[2]))")
        XCTAssertEqual(bg(dark[3]), [142, 140, 240], "\(String(describing: dark[3]))")
        XCTAssertEqual(bg(dark[5]), [142, 140, 240])
        XCTAssertEqual(bg(dark[6]), [233, 233, 235], "the fixed light bubble")
        XCTAssertEqual((dark[6]?["fg"] as? [Double])?.prefix(3).map { $0 }, [255, 255, 255], "white ink on it, which the driver finds")
    }

    /// LLP 1116.010 D2 (trivia, round 12): a card's fill is observed with the part of it that is presented. One past its
    /// scroller's edge (a Badge below the page's viewport) is not drawn, and the pixels there are the page's: the driver
    /// must not judge them as "a card that does not show".
    func testACardPastItsScrollersEdgeIsObservedWithNothingPresented() throws {
        let round: [String: Any] = ["border_radius_top_left": 12, "border_radius_top_right": 12, "border_radius_bottom_right": 12, "border_radius_bottom_left": 12]
        let card: [String: Any] = round.merging(["background_color": [255, 255, 255, 255]]) { x, _ in x }
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "inside"], "style": card],
            ["op": "create", "id": 4, "kind": "view", "props": ["testId": "past"], "style": card],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Inside"], "style": ["font_size": 17]],
            ["op": "create", "id": 6, "kind": "text", "props": ["text": "Past"], "style": ["font_size": 17]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3, 4]],
            ["op": "children", "id": 3, "ids": [5]],
            ["op": "children", "id": 4, "ids": [6]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 400.0, "w": 400.0, "h": 100.0],
            ["op": "content", "id": 2, "w": 400.0, "h": 200.0],
            ["op": "frame", "id": 3, "x": 16.0, "y": 20.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 16.0, "y": 140.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 5, "x": 12.0, "y": 9.0, "w": 76.0, "h": 22.0],
            ["op": "frame", "id": 6, "x": 12.0, "y": 9.0, "w": 76.0, "h": 22.0],
        ])
        let reply = agent.check([:])
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]], "\(reply)")
        func item(_ id: Int) throws -> [String: Any] { try XCTUnwrap(items.first { $0["id"] as? Int == id }, "#\(id) in \(items)") }
        XCTAssertNil(try item(3)["vis"], "the card inside its scroller is whole")
        let past = try item(4)
        XCTAssertNotNil(past["fill"], "it is a card: \(past)")
        let vis = try XCTUnwrap(past["vis"] as? [String: Any], "the scroller cuts it away: \(past)")
        XCTAssertEqual(vis["h"] as? Double, 0, "none of it is presented: \(past)")
    }

    /// In landscape the safe area takes 62 points from each side of the screen, and a route's content spans what is left: a
    /// box with square corners that reaches the safe area's sides is the page (a band across it, a route's list), not a card,
    /// as it is upright; one inset from them is a card.
    func testABoxAcrossTheSafeAreaIsThePageNotACard() throws {
        let fill: [String: Any] = ["background_color": [255, 255, 255, 255]]
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view", "style": fill],
            ["op": "create", "id": 2, "kind": "view", "props": ["testId": "band"], "style": fill],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Across"], "style": ["font_size": 17]],
            ["op": "create", "id": 4, "kind": "view", "props": ["testId": "card"], "style": fill],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Inset"], "style": ["font_size": 17]],
            ["op": "children", "id": 1, "ids": [2, 4]], ["op": "children", "id": 2, "ids": [3]], ["op": "children", "id": 4, "ids": [5]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 62.0, "y": 100.0, "w": 276.0, "h": 60.0],
            ["op": "frame", "id": 3, "x": 16.0, "y": 19.0, "w": 200.0, "h": 22.0],
            ["op": "frame", "id": 4, "x": 78.0, "y": 200.0, "w": 244.0, "h": 60.0],
            ["op": "frame", "id": 5, "x": 16.0, "y": 19.0, "w": 200.0, "h": 22.0],
        ])
        window?.rootViewController?.additionalSafeAreaInsets = UIEdgeInsets(top: 0, left: 62, bottom: 0, right: 62)
        window?.layoutIfNeeded()
        let items = try XCTUnwrap(agent.check([:])["items"] as? [[String: Any]])
        XCTAssertNil(items.first { $0["id"] as? Int == 2 }?["fill"], "the band reaches the safe area's sides: \(items)")
        XCTAssertNotNil(items.first { $0["id"] as? Int == 4 }?["fill"], "16 points in from them, a card: \(items)")
    }

    /// A cover and the scroller that clears it (LLP 1116.003): Shelf's
    /// Discover drew two false findings. Left, a scroller at its start whose
    /// row lies past its bottom, under the tab bar the agent paints below it:
    /// the scroller cuts the row away, so nothing covers it, and scrolling
    /// brings it in. Right, a scroller at its end: a row under a bar at its
    /// top is cleared by scrolling back, though the bar's own part the touch
    /// meets lies below the row's middle (a navigation bar's title); a row
    /// under a bar at its bottom is not, though the touch meets the bar's
    /// top half.
    func testACoverIsJudgedWholeAndARowPastItsScrollerIsNotCovered() throws {
        let gray: [Double] = [200, 200, 200, 255]
        let scrolls: [String: Any] = ["overflow_y": "scroll"]
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": 2, "kind": "view", "style": scrolls],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["press"], "props": ["testId": "past"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["testId": "tabs"], "style": ["background_color": gray]],
            ["op": "create", "id": 51, "kind": "view", "props": ["testId": "tab"], "style": ["background_color": gray]],
            ["op": "create", "id": 6, "kind": "view", "style": scrolls],
            ["op": "create", "id": 7, "kind": "view", "handlers": ["press"], "props": ["testId": "first"]],
            ["op": "create", "id": 8, "kind": "view", "handlers": ["press"], "props": ["testId": "last"]],
            ["op": "create", "id": 9, "kind": "view", "props": ["testId": "top-bar"], "style": ["background_color": gray]],
            ["op": "create", "id": 91, "kind": "view", "props": ["testId": "title"], "style": ["background_color": gray]],
            ["op": "create", "id": 10, "kind": "view", "props": ["testId": "bottom-bar"], "style": ["background_color": gray]],
            ["op": "create", "id": 101, "kind": "view", "props": ["testId": "bottom-item"], "style": ["background_color": gray]],
            ["op": "children", "id": 1, "ids": [2, 5, 6, 9, 10]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 5, "ids": [51]],
            ["op": "children", "id": 6, "ids": [7, 8]],
            ["op": "children", "id": 9, "ids": [91]],
            ["op": "children", "id": 10, "ids": [101]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 600.0],
            ["op": "content", "id": 2, "w": 200.0, "h": 1200.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 620.0, "w": 200.0, "h": 60.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 600.0, "w": 200.0, "h": 120.0],
            ["op": "frame", "id": 51, "x": 0.0, "y": 0.0, "w": 200.0, "h": 60.0],
            ["op": "frame", "id": 6, "x": 200.0, "y": 0.0, "w": 200.0, "h": 800.0],
            ["op": "content", "id": 6, "w": 200.0, "h": 1600.0],
            ["op": "frame", "id": 7, "x": 0.0, "y": 800.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 8, "x": 0.0, "y": 1540.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 9, "x": 200.0, "y": 0.0, "w": 200.0, "h": 120.0],
            ["op": "frame", "id": 91, "x": 0.0, "y": 10.0, "w": 200.0, "h": 110.0],
            ["op": "frame", "id": 10, "x": 200.0, "y": 700.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 101, "x": 0.0, "y": 0.0, "w": 200.0, "h": 60.0],
        ])
        let p = try XCTUnwrap(session).presenter
        let end = try XCTUnwrap(p.views[6]?.scroll)
        end.setContentOffset(CGPoint(x: 0, y: 800), animated: false)
        p.settlePump()
        p.viewport.layoutIfNeeded()
        let reply = agent.check([:])
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]], "\(reply)")
        func cover(_ id: Int) -> [String: Any]? { items.first { $0["id"] as? Int == id }?["cover"] as? [String: Any] }
        // Each touch meets what it is meant to: the tab, the title, the bottom bar's item.
        XCTAssertTrue(window?.hitTest(CGPoint(x: 100, y: 650), with: nil) === p.views[51])
        XCTAssertTrue(window?.hitTest(CGPoint(x: 300, y: 20), with: nil) === p.views[91])
        XCTAssertTrue(window?.hitTest(CGPoint(x: 300, y: 762), with: nil) === p.views[10])
        XCTAssertNil(cover(3), "a row past its scroller's bottom is cut away, and its scroller brings it in: \(items)")
        XCTAssertNil(cover(7), "under the top bar at the scroller's end, scrolling back clears it: \(items)")
        let last = try XCTUnwrap(cover(8), "under the bottom bar at the scroller's end, nothing clears it: \(items)")
        XCTAssertEqual(last["by"] as? Int, 10)
    }

    func testAPartlyClippedTopRowCanScrollClearOfItsOwnListsDecoration() throws {
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "row"], "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 800],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 400, "h": 800],
            ["op": "frame", "id": 3, "x": 0, "y": 440, "w": 400, "h": 100],
            ["op": "content", "id": 2, "w": 400, "h": 1200]
        ])
        let p = try XCTUnwrap(session).presenter, scroll = try XCTUnwrap(p.views[2]?.scroll)
        scroll.contentInset.top = 100
        scroll.contentOffset.y = 400
        let decoration = CheckScrollDecoration(frame: scroll.bounds)
        scroll.addSubview(decoration)
        let row = try XCTUnwrap(p.views[3]), point = row.convert(CGPoint(x: 200, y: 50), to: nil)
        XCTAssertTrue(window?.hitTest(point, with: nil) === decoration)
        let items = try XCTUnwrap(agent.check([:])["items"] as? [[String: Any]])
        let item = try XCTUnwrap(items.first { $0["id"] as? Int == 3 })
        XCTAssertNil(item["cover"], "scrolling toward the start brings the row below the inset: \(item)")
    }

    /// A row under iOS 26's floating bar (a bottom search field) is covered by the bar's glass, not by the container
    /// UIKit hangs it in, which is as large as the window and sits in the app's root node: judged on the root node, a
    /// row a short scroll clears from under the glass would need its middle carried past the screen's top. Left, a
    /// scroller that can still move: scrolling clears its row from under the glass. Right, a scroller at its end:
    /// nothing clears its last row, which the bar covers. Beside the glass, a text is neither covered nor cut.
    func testARowUnderTheFloatingBarIsCoveredByItsGlassAndScrollingClearsIt() throws {
        let scrolls: [String: Any] = ["overflow_y": "scroll"]
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": scrolls],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["press"], "props": ["testId": "row"]],
            ["op": "create", "id": 6, "kind": "view", "style": scrolls],
            ["op": "create", "id": 8, "kind": "view", "handlers": ["press"], "props": ["testId": "last"]],
            ["op": "create", "id": 9, "kind": "text", "props": ["text": "A"], "style": ["font_size": 12]],
            ["op": "children", "id": 1, "ids": [2, 6]],
            ["op": "children", "id": 2, "ids": [3, 9]],
            ["op": "children", "id": 6, "ids": [8]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 800.0],
            ["op": "content", "id": 2, "w": 200.0, "h": 1200.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 720.0, "w": 200.0, "h": 60.0],
            ["op": "frame", "id": 6, "x": 200.0, "y": 0.0, "w": 200.0, "h": 800.0],
            ["op": "content", "id": 6, "w": 200.0, "h": 1600.0],
            ["op": "frame", "id": 8, "x": 0.0, "y": 1540.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 9, "x": 2.0, "y": 740.0, "w": 16.0, "h": 16.0],
        ])
        let p = try XCTUnwrap(session).presenter
        try XCTUnwrap(p.views[6]?.scroll).setContentOffset(CGPoint(x: 0, y: 800), animated: false)
        p.settlePump()
        p.viewport.layoutIfNeeded()
        // UIKit's arrangement: the container and its hosting view as large as the window, the glass at the bottom
        let container = _UIFloatingBarContainerView(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        let host = FloatingBarHostingView(frame: container.bounds)
        let glass = UIPlatformGlassInteractionView(frame: CGRect(x: 20, y: 720, width: 360, height: 60))
        host.addSubview(glass)
        container.addSubview(host)
        try XCTUnwrap(p.views[1]).addSubview(container)
        XCTAssertTrue(window?.hitTest(CGPoint(x: 100, y: 750), with: nil) === host, "a touch on the glass meets the hosting view")
        XCTAssertTrue(window?.hitTest(CGPoint(x: 100, y: 400), with: nil)?.isDescendant(of: try XCTUnwrap(p.views[2])) == true, "elsewhere a touch passes the bar")
        let items = try XCTUnwrap(agent.check([:])["items"] as? [[String: Any]])
        func cover(_ id: Int) -> [String: Any]? { items.first { $0["id"] as? Int == id }?["cover"] as? [String: Any] }
        // 400 points of room: enough to carry the row's middle past the glass's top, 30 points up, and not past the
        // root node's, 750 up
        XCTAssertNotNil(items.first { $0["id"] as? Int == 3 }, "\(items)")
        XCTAssertNil(cover(3), "the row's middle is 30 points under the glass's top, and its scroller moves it clear: \(items)")
        let last = try XCTUnwrap(cover(8), "at the scroller's end nothing clears the last row: \(items)")
        XCTAssertEqual(last["kind"] as? String, "bar")
        XCTAssertEqual(last["name"] as? String, "UIKit's floating bar")
        let beside = try XCTUnwrap(items.first { $0["id"] as? Int == 9 }, "\(items)")
        XCTAssertNil(beside["vis"], "a text beside the glass, below its top, is presented whole: \(beside)")
        XCTAssertNil(beside["cover"], "\(beside)")
    }

    /// A touch the floating bar takes away from its glass (a toolbar it holds) is judged by what it met, not by glass
    /// elsewhere: measured from glass below the row, any scroll would have seemed to clear it. A bar inside a floating
    /// bar whose glass is found cuts nothing beside the glass.
    func testAFloatingBarsGlassCoversOnlyWhereItLies() throws {
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["press"], "props": ["testId": "row"]],
            ["op": "create", "id": 9, "kind": "text", "props": ["text": "A"], "style": ["font_size": 12]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3, 9]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "content", "id": 2, "w": 400.0, "h": 820.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 640.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 9, "x": 2.0, "y": 740.0, "w": 16.0, "h": 16.0],
        ])
        let p = try XCTUnwrap(session).presenter
        p.settlePump()
        p.viewport.layoutIfNeeded()
        let container = _UIFloatingBarContainerView(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        let host = FloatingBarHostingView(frame: container.bounds)
        host.addSubview(UIPlatformGlassInteractionView(frame: CGRect(x: 20, y: 720, width: 360, height: 60)))
        let toolbar = UIToolbar(frame: CGRect(x: 0, y: 640, width: 400, height: 60))
        container.addSubview(host)
        container.addSubview(toolbar)
        try XCTUnwrap(p.views[1]).addSubview(container)
        let hit = try XCTUnwrap(window?.hitTest(CGPoint(x: 200, y: 670), with: nil))
        XCTAssertTrue(hit.isDescendant(of: toolbar), "the row's middle meets the toolbar: \(hit)")
        let items = try XCTUnwrap(agent.check([:])["items"] as? [[String: Any]])
        // 20 points of room, short of the 30 that carry the row's middle past the toolbar's top
        let row = try XCTUnwrap(items.first { $0["id"] as? Int == 3 }, "\(items)")
        let cover = try XCTUnwrap(row["cover"] as? [String: Any], "\(row)")
        XCTAssertEqual(cover["kind"] as? String, "bar")
        XCTAssertEqual(cover["name"] as? String, "UIToolbar")
        let beside = try XCTUnwrap(items.first { $0["id"] as? Int == 9 }, "\(items)")
        XCTAssertNil(beside["vis"], "a text below the toolbar, beside the glass, is presented whole: \(beside)")
        XCTAssertNil(beside["cover"], "\(beside)")
    }

    /// LLP 1116.007 D3.8: what lies under UIKit's navigation bar (a row scrolled there, round 9's book-club and
    /// mood-journal) is covered, not presented: its `vis` is empty, or only the part below the bar's edge, so the
    /// driver judges no contrast or overlap there; a text wholly below the bar has none.
    func testATextUnderANavigationBarIsPresentedOnlyBelowIt() throws {
        let agent = mounted([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Under"], "style": ["font_size": 17]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Straddling"], "style": ["font_size": 17]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Below"], "style": ["font_size": 17]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 16.0, "y": 50.0, "w": 100.0, "h": 22.0],
            ["op": "frame", "id": 3, "x": 16.0, "y": 70.0, "w": 100.0, "h": 30.0],
            ["op": "frame", "id": 4, "x": 16.0, "y": 120.0, "w": 100.0, "h": 22.0],
        ])
        window?.rootViewController?.view.addSubview(UINavigationBar(frame: CGRect(x: 0, y: 40, width: 400, height: 44)))
        let reply = agent.check([:])
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]], "\(reply)")
        func vis(_ id: Int) -> [String: Any]? { items.first { $0["id"] as? Int == id }?["vis"] as? [String: Any] }
        XCTAssertEqual(vis(2)?["h"] as? Double, 0, "wholly under the bar (y 40 to 84): \(items)")
        XCTAssertEqual(vis(3)?["y"] as? Double, 84, "the part below the bar's edge: \(items)")
        XCTAssertEqual(vis(3)?["h"] as? Double, 16)
        XCTAssertNil(vis(4), "below the bar it is whole: \(items)")
    }

    /// A text scrolled above a top bar's inner edge, where a touch meets UIKit's edge effect, is covered by the bar, which
    /// scrolling back clears. UIKit sizes the edge effect far past the bar (chat's transcript in landscape with the keyboard
    /// up: 118 of its 128 points, from the screen's top over a bar 24 points down); judged on its box, the text was "under
    /// transcript-dad at rest, and nothing scrolls it clear". Any other view of the scroller's own over it still covers it.
    func testATextUnderABarIsCoveredByTheBarWhereATouchMeetsTheScrollersEdgeEffect() throws {
        for (view, cleared) in [(ScrollEdgeEffectView(), true), (UIView(), false)] {
            let agent = mounted([
                ["op": "create", "id": 1, "kind": "view"],
                ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
                ["op": "create", "id": 3, "kind": "text", "props": ["text": "Scrolled under the bar"], "style": ["font_size": 17]],
                ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
                ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
                ["op": "content", "id": 2, "w": 400.0, "h": 1600.0],
                ["op": "frame", "id": 3, "x": 16.0, "y": 220.0, "w": 300.0, "h": 22.0],
            ])
            let p = try XCTUnwrap(session).presenter
            let scroll = try XCTUnwrap(p.views[2]?.scroll)
            scroll.setContentOffset(CGPoint(x: 0, y: 200), animated: false) // the text at y 20 to 42 in the window
            p.settlePump()
            p.viewport.layoutIfNeeded()
            let bar = UINavigationBar(frame: CGRect(x: 0, y: 40, width: 400, height: 44))
            window?.rootViewController?.view.addSubview(bar)
            // The scroller's own view over its content: it stays where the scroller is.
            view.frame = CGRect(x: 0, y: scroll.contentOffset.y, width: 400, height: 700)
            scroll.addSubview(view)
            let text = try XCTUnwrap(p.views[3]).convert(try XCTUnwrap(p.views[3]).bounds, to: nil)
            XCTAssertLessThan(text.midY, bar.frame.minY, "its middle is above the bar, under nothing of it: \(text)")
            XCTAssertTrue(window?.hitTest(CGPoint(x: text.midX, y: text.midY), with: nil) === view, "a touch there meets \(view)")
            let items = try XCTUnwrap(agent.check([:])["items"] as? [[String: Any]])
            let item = try XCTUnwrap(items.first { $0["id"] as? Int == 3 }, "\(items)")
            XCTAssertEqual(item["cover"] == nil, cleared, cleared ? "scrolling back clears it from under the bar: \(item)" : "nothing clears it from under \(view): \(item)")
            session?.destroy(); session = nil; window?.isHidden = true; window = nil
        }
    }

    /// LLP 1116.009 D5 (round 11: book-club, expense-tracker, the grouped-form recipe's Quantity row, the dashboard
    /// pattern's add sheet): a native field's box is its kernel box and no taller, and a text under it in the same
    /// grouped row is not covered. A touch the row leaves to no control goes to the row's one field (the cell is the
    /// field, LLP 1116.005 D2), a widening of the field's target and nothing drawn over the text; `check` read it as
    /// the field covering "its middle". A text under a button of the row is still covered, and a finger's touch on the
    /// text still reaches the field.
    func testATextInAFieldsGroupedRowIsNotCoveredByTheFieldItsTouchFallsTo() throws {
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "agent-check-cell-field")
        self.session = session
        let p = session.presenter
        let host = try XCTUnwrap(p.groupedLists as? GroupedListHost)
        host.model = { $0 == 1 ? GroupedListModel(style: "inset-grouped", sections: [.init(view: 2, header: nil, footer: nil, rows: [.init(view: 30, custom: true)])]) : nil }
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        self.window = window
        p.viewport.frame = window.bounds
        window.rootViewController?.view.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "list", "props": ["listStyle": "inset-grouped"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["semanticTag": "section"], "style": [:]],
            ["op": "create", "id": 4, "kind": "view", "props": [:], "style": [:]],
            ["op": "create", "id": 30, "kind": "view", "props": ["testId": "row"], "style": [:]],
            ["op": "create", "id": 31, "kind": "input", "props": ["testId": "name", "placeholder": "Name"], "handlers": ["input"], "style": ["font_size": 17]],
            ["op": "create", "id": 32, "kind": "text", "props": ["text": "Required"], "style": ["font_size": 13]],
            ["op": "create", "id": 34, "kind": "text", "props": ["text": "Clear"], "style": ["font_size": 13]],
            ["op": "create", "id": 33, "kind": "button", "props": ["testId": "clear"], "handlers": ["press"], "style": [:]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [4]], ["op": "children", "id": 4, "ids": [30]],
            ["op": "children", "id": 30, "ids": [31, 32, 34, 33]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0],
            ["op": "frame", "id": 30, "x": 16.0, "y": 35.0, "w": 370.0, "h": 74.0],
            ["op": "frame", "id": 31, "x": 16.0, "y": 15.0, "w": 260.0, "h": 22.0],
            ["op": "frame", "id": 32, "x": 16.0, "y": 41.0, "w": 260.0, "h": 16.0],
            ["op": "frame", "id": 34, "x": 300.0, "y": 20.0, "w": 54.0, "h": 16.0],
            ["op": "frame", "id": 33, "x": 300.0, "y": 15.0, "w": 54.0, "h": 44.0],
            ["op": "fieldContent", "id": 31, "rect": [0.0, 0.0, 260.0, 22.0], "cell": true],
        ]))
        host.lists[1]?.collection.layoutIfNeeded()
        p.viewport.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        let node = try XCTUnwrap(p.views[31]), message = try XCTUnwrap(p.views[32]), editor = try XCTUnwrap(node.field)
        // The field is its kernel box and no taller: it draws nowhere near the message under it.
        XCTAssertEqual(editor.frame, node.bounds)
        XCTAssertEqual(node.bounds.size, CGSize(width: 260, height: 22))
        let drawn = editor.convert(editor.bounds, to: nil), under = message.convert(message.bounds, to: nil)
        XCTAssertFalse(drawn.intersects(under), "\(drawn) over \(under)")
        XCTAssertGreaterThanOrEqual(under.minY - drawn.maxY, 4)
        let middle = CGPoint(x: under.midX, y: under.midY)
        XCTAssertTrue(window.hitTest(middle, with: nil) === editor, "a touch on the text reaches the field: it is the cell's")
        let reply = Agent(session: session).check([:])
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]], "\(reply)")
        func item(_ id: Int) -> [String: Any]? { items.first { $0["id"] as? Int == id } }
        XCTAssertEqual(item(31)?["h"] as? Double, 22, "the field as observed is its kernel box: \(items)")
        XCTAssertEqual(item(32)?["text"] as? String, "Required", "\(items)")
        XCTAssertNil(item(32)?["cover"], "the text is not covered by the field its touch falls to: \(items)")
        XCTAssertNil(item(31)?["cover"], "\(items)")
        let over = try XCTUnwrap(item(34)?["cover"] as? [String: Any], "a text under the row's button is covered by it: \(items)")
        XCTAssertEqual(over["by"] as? Int, 33)
        XCTAssertTrue(window.hitTest(middle, with: nil) === editor, "the touch still falls to the field after the check")
    }
}

/// A stand-in with the name of iOS 26's edge-effect view, which `check` knows UIKit's by.
private final class ScrollEdgeEffectView: UIView {}

/// Stand-ins for iOS 26's floating bar, named as UIKit's are: the container, which takes no touch of its own; the
/// hosting view, which takes one only over the glass it draws; and the glass.
private final class _UIFloatingBarContainerView: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        let hit = super.hitTest(point, with: event)
        return hit === self ? nil : hit
    }
}
private final class FloatingBarHostingView: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        subviews.contains { $0.frame.contains(point) } ? self : nil
    }
}
private final class UIPlatformGlassInteractionView: UIView {}
#endif
