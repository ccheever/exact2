#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// `check` on UIKit (LLP 1116.003 D8): the observations the driver judges —
/// a paragraph's ink and whether it fits its box, a control covered by
/// another node, a target the author sized small, a view past the viewport's
/// side, a card's fill without an edge, two controls overlapping — read from
/// the presented view tree.
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
        var face = ButtonFace(); face.title = "Prominent"; face.style = "bordered-prominent"
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
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6, 7, 8, 9, 10]],
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
        ])
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
        XCTAssertEqual(try item(9)["control"] as? String, "press")
        XCTAssertEqual(try item(10)["x"] as? Double, 96, "b starts inside a: the driver finds them overlapping")
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
}
#endif
