#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// A tap addressed by id presses what it names (LLP 1012 §1 `tap`): a row
/// with a press of its own is pressed itself, beside the link card its middle
/// holds, never the card; a box without a press never presses a control
/// inside it; `at` keeps a point's semantics (the Bluesky clone's likes and
/// reposts on real people's posts, 2026-10-09).
final class AddressedTapIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func rows() -> (ExactSession, Agent, () -> [UInt32]) {
        let session = ExactApp.shared.makeSession(label: "addressed-tap")
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
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            // A post that opens its thread, a link card at its middle.
            ["op": "create", "id": 2, "kind": "view", "handlers": ["press"], "props": ["testId": "post-0"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["press"], "props": ["testId": "card-0"]],
            // A box with no press of its own, Like at its middle.
            ["op": "create", "id": 4, "kind": "view", "props": ["testId": "wrap-0"]],
            ["op": "create", "id": 5, "kind": "button", "handlers": ["press"], "props": ["testId": "like-0"]],
            // A row whose child covers it: no point of it presses it.
            ["op": "create", "id": 6, "kind": "view", "handlers": ["press"], "props": ["testId": "post-1"]],
            ["op": "create", "id": 7, "kind": "view", "handlers": ["press"], "props": ["testId": "cover-1"]],
            // A disabled row: a press of its own that does nothing, Repost at its middle.
            ["op": "create", "id": 8, "kind": "view", "handlers": ["press"], "props": ["testId": "post-2", "disabled": "true"]],
            ["op": "create", "id": 9, "kind": "button", "handlers": ["press"], "props": ["testId": "repost-2"]],
            ["op": "children", "id": 1, "ids": [2, 4, 6, 8]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 6, "ids": [7]],
            ["op": "children", "id": 8, "ids": [9]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 160.0],
            ["op": "frame", "id": 3, "x": 100.0, "y": 40.0, "w": 200.0, "h": 80.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 200.0, "w": 400.0, "h": 100.0],
            ["op": "frame", "id": 5, "x": 150.0, "y": 25.0, "w": 100.0, "h": 50.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": 340.0, "w": 400.0, "h": 100.0],
            ["op": "frame", "id": 7, "x": 0.0, "y": 0.0, "w": 400.0, "h": 100.0],
            ["op": "frame", "id": 8, "x": 0.0, "y": 480.0, "w": 400.0, "h": 100.0],
            ["op": "frame", "id": 9, "x": 150.0, "y": 25.0, "w": 100.0, "h": 50.0],
        ]))
        p.viewport.layoutIfNeeded()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        return (session, Agent(session: session), { defer { pressed = [] }; return pressed })
    }

    func testARowWithItsOwnPressIsPressedBesideTheCardAtItsMiddle() throws {
        let (_, agent, presses) = rows()
        let reply = agent.tap(["id": 2])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["pressed"] as? Int, 2, "the row, not the card its middle holds: \(reply)")
        XCTAssertEqual(presses(), [2])
        let avoided = try XCTUnwrap(reply["avoided"] as? [String: Any], "the reply says why it landed off the middle: \(reply)")
        XCTAssertEqual(avoided["pressing"] as? Int, 3)
        let at = try XCTUnwrap(reply["at"] as? [Double])
        XCTAssertFalse(CGRect(x: 100, y: 40, width: 200, height: 80).contains(CGPoint(x: at[0], y: at[1])), "outside the card: \(at)")
        // The card, named, is the card; a point keeps the point's semantics.
        XCTAssertEqual(agent.tap(["id": 3])["pressed"] as? Int, 3)
        XCTAssertEqual(presses(), [3])
        let point = agent.tap(["id": 2, "at": [200.0, 80.0]])
        XCTAssertEqual(point["pressed"] as? Int, 3, "at the row's middle, a finger reaches the card: \(point)")
        XCTAssertNil(point["avoided"])
        XCTAssertEqual(presses(), [3])
        XCTAssertNotNil(agent.tap(["id": 2, "at": [420.0, 80.0]])["error"], "a point outside the box is refused")
        XCTAssertEqual(presses(), [])
    }

    func testABoxWithoutAPressNeverPressesTheControlInsideIt() throws {
        let (_, agent, presses) = rows()
        let reply = agent.tap(["id": 4])
        let error = try XCTUnwrap(reply["error"] as? String, "refused: \(reply)")
        XCTAssertTrue(error.contains("would press #5 inside it"), error)
        XCTAssertEqual(reply["pressing"] as? Int, 5)
        XCTAssertEqual(reply["addressed"] as? Int, 4)
        XCTAssertEqual(presses(), [], "nothing is pressed")
        XCTAssertEqual(agent.tap(["id": 4, "at": [200.0, 50.0]])["pressed"] as? Int, 5, "a point: what is there")
        XCTAssertEqual(presses(), [5])
    }

    func testARowNoPointOfWhichReachesItIsRefused() throws {
        let (_, agent, presses) = rows()
        let reply = agent.tap(["id": 6])
        let error = try XCTUnwrap(reply["error"] as? String, "refused: \(reply)")
        XCTAssertTrue(error.contains("would press #7 inside it") && error.contains("no point of #6"), error)
        XCTAssertEqual(presses(), [])
    }

    func testADisabledRowPressesNothingNotTheButtonAtItsMiddle() throws {
        let (_, agent, presses) = rows()
        let reply = agent.tap(["id": 8])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertTrue(reply["pressed"] is NSNull, "a disabled row's tap presses nothing: \(reply)")
        XCTAssertEqual(presses(), [])
    }

    /// What a press there would deliver counts, a text editor's focus too;
    /// a 2-point strip around a child that covers the rest is found (Astra,
    /// round 1); a point on an inline run presses the run.
    func testAnEditorAThinStripAndAnInlineRunAtAPoint() throws {
        let (session, agent, presses) = rows()
        let p = session.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "handlers": ["press"], "props": ["testId": "note-row"]],
            ["op": "create", "id": 21, "kind": "textarea", "props": ["id": "note", "value": "hi"], "handlers": ["input"]],
            ["op": "create", "id": 22, "kind": "view", "handlers": ["press"], "props": ["testId": "ring"]],
            ["op": "create", "id": 23, "kind": "view", "handlers": ["press"], "props": ["testId": "ring-core"]],
            ["op": "create", "id": 24, "kind": "view", "handlers": ["press"], "props": ["testId": "para-row"]],
            ["op": "create", "id": 25, "kind": "text", "style": ["font_size": 16, "line_height": "20px"]],
            ["op": "paragraph", "id": 25, "runs": [
                ["id": 26, "parent": 25, "paint": true, "props": ["text": "Read "], "style": [:], "handlers": []],
                ["id": 27, "parent": 25, "paint": true, "props": ["text": "the guide"], "style": [:], "handlers": ["press"]],
            ]],
            ["op": "children", "id": 1, "ids": [2, 4, 6, 8, 20, 22, 24]],
            ["op": "children", "id": 20, "ids": [21]],
            ["op": "children", "id": 22, "ids": [23]],
            ["op": "children", "id": 24, "ids": [25]],
            ["op": "frame", "id": 20, "x": 0.0, "y": 600.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 21, "x": 100.0, "y": 10.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 22, "x": 0.0, "y": 680.0, "w": 100.0, "h": 100.0],
            ["op": "frame", "id": 23, "x": 2.0, "y": 2.0, "w": 96.0, "h": 96.0],
            ["op": "frame", "id": 24, "x": 120.0, "y": 680.0, "w": 280.0, "h": 40.0],
            ["op": "frame", "id": 25, "x": 0.0, "y": 0.0, "w": 280.0, "h": 40.0],
        ]))
        p.viewport.layoutIfNeeded()
        let note = agent.tap(["id": 20])
        XCTAssertNil(note["error"], "\(note)")
        XCTAssertEqual(presses(), [20], "the row beside its editor: \(note)")
        XCTAssertEqual((note["avoided"] as? [String: Any])?["pressing"] as? Int, 21, "\(note)")
        XCTAssertNotEqual(p.views[21]?.textArea?.isFirstResponder, true, "the editor did not take the focus")
        let ring = agent.tap(["id": 22])
        XCTAssertNil(ring["error"], "a 2-point strip reaches the ring: \(ring)")
        XCTAssertEqual(presses(), [22])
        let owner = try XCTUnwrap(p.views[25]), run = try XCTUnwrap(p.inlineText(27))
        let fragment = try XCTUnwrap(owner.inlineRects(run).first)
        let point = agent.tap(["id": 24, "at": [Double(fragment.midX), Double(fragment.midY)]])
        XCTAssertEqual(point["pressed"] as? Int, 27, "a point on the run presses the run: \(point)")
        XCTAssertEqual(presses(), [27])
    }

    /// A real touch's aim (`--touch platform`) takes the same rule.
    func testTheTouchRunnersAimTakesTheSameRule() throws {
        let (_, agent, _) = rows()
        let plain = agent.aim(["id": 2, "aim": ["press": true]])
        if (plain["error"] as? String)?.contains("foreground") == true { throw missing("foreground", "the test host is in the background: a real touch's aim refuses it") }
        let aim = try XCTUnwrap(plain["aim"] as? [String: Any], "\(plain)")
        let at = try XCTUnwrap(aim["at"] as? [Double])
        XCTAssertFalse(CGRect(x: 100, y: 40, width: 200, height: 80).contains(CGPoint(x: at[0], y: at[1])), "beside the card: \(at)")
        XCTAssertEqual(aim["hit"] as? Int, 2)
        XCTAssertEqual((aim["avoided"] as? [String: Any])?["pressing"] as? Int, 3)
        XCTAssertEqual((agent.aim(["id": 4, "aim": ["press": true]])["pressing"]) as? Int, 5, "refused, naming Like")
        XCTAssertEqual((agent.aim(["id": 2, "aim": ["at": [200.0, 80.0]]])["aim"] as? [String: Any])?["hit"] as? Int, 3, "a point: the card")
        XCTAssertEqual((agent.aim(["id": 2, "aim": true])["aim"] as? [String: Any])?["hit"] as? Int, 3, "a drag's aim is the middle still")
    }

    /// A tap on a native module's own control reaches no control of its
    /// (UIKit has no public touch synthesis): refused, naming `--touch
    /// platform`, unless Contract hears it, a pointer listener around the
    /// native view, which gets its pointer events, the reply saying what
    /// went unreached.
    func testATapOnAModulesControlIsRefusedUnlessContractHearsIt() throws {
        let session = ExactApp.shared.makeSession(label: "module-tap")
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
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "native", "props": ["testId": "transport"]],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["pointerdown"], "props": ["testId": "listening"]],
            ["op": "create", "id": 4, "kind": "native", "props": ["testId": "heard"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 100.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 200.0, "w": 400.0, "h": 100.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 100.0],
        ]))
        p.viewport.layoutIfNeeded()
        // What a module's view holds: a control of its own.
        for id in [UInt32(2), 4] {
            let control = UIButton(type: .system)
            control.frame = CGRect(x: 0, y: 0, width: 400, height: 100)
            try XCTUnwrap(p.views[id]).addSubview(control)
        }
        let agent = Agent(session: session)
        let refused = agent.tap(["id": 2, "at": [200.0, 50.0]])
        XCTAssertTrue((refused["error"] as? String)?.contains("unsupported") == true, "\(refused)")
        XCTAssertTrue((refused["error"] as? String)?.contains("--touch platform") == true, "\(refused)")
        let heard = agent.tap(["id": 4, "at": [200.0, 50.0]])
        XCTAssertNil(heard["error"], "\(heard)")
        XCTAssertNotNil(heard["unreached"], "says the module's control heard nothing: \(heard)")
    }

    /// Under `--touch platform` a menu's row is UIKit's, found by its
    /// popover and place (LLP 1080.000 §13): UIKit puts no identifier on the
    /// element it shows, so the row's popover must be the menu UIKit was last
    /// given to present, and its rows must read as one run of the shown
    /// rows. A duplicate title or a label that is another's prefix picks
    /// nothing else; another popover's row, however alike its rows read, a
    /// menu shown twice and a menu that scrolls past the screen are refused.
    func testAMenuRowIsItsPlaceInThePresentedMenu() throws {
        let (session, agent, _) = rows()
        let window = try XCTUnwrap(self.window)
        let p = session.presenter
        let node = { (id: UInt32) in p.views[id]! }
        let present = { (ids: [UInt32]) in p.menus.presentedMenu = Dictionary(uniqueKeysWithValues: ids.map { ($0, node($0).incarnation) }) }
        p.menus.given[2] = [GivenRow(node(3), title: "Rename", subtitle: nil), GivenRow(node(5), title: "Delete", subtitle: nil), GivenRow(node(7), title: "Delete", subtitle: nil)]
        p.menus.given[4] = [GivenRow(node(9), title: "Copy", subtitle: nil), GivenRow(node(6), title: "Copy", subtitle: "link")]
        // Popover 8 gives the same rows as popover 2.
        p.menus.given[8] = [GivenRow(node(1), title: "Rename", subtitle: nil), GivenRow(node(4), title: "Delete", subtitle: nil), GivenRow(node(2), title: "Delete", subtitle: nil)]
        func menu(_ labels: [String], at origin: CGPoint, scrolls: Bool = false) -> UIScrollView {
            let list = UIScrollView(frame: CGRect(origin: origin, size: CGSize(width: 200, height: CGFloat(labels.count) * 40)))
            // Added last row first: a list's subviews are not in reading order.
            for (i, label) in labels.enumerated().reversed() {
                let cell = UIView(frame: CGRect(x: 0, y: CGFloat(i) * 40, width: 200, height: 40))
                cell.isAccessibilityElement = true
                cell.accessibilityLabel = label
                list.addSubview(cell)
            }
            list.contentSize = CGSize(width: 200, height: CGFloat(labels.count + (scrolls ? 4 : 0)) * 40)
            window.addSubview(list)
            return list
        }
        let why = { (r: Result<CGRect, AimRefusal>) -> String in if case .failure(let f) = r { return f.why }; return "" }
        XCTAssertTrue(why(agent.shownRow(node(5), of: node(2), in: window)).contains("not the menu UIKit shows"), "no menu presented")
        present([2])
        XCTAssertTrue(why(agent.shownRow(node(5), of: node(2), in: window)).contains("does not show its rows"), "presented, not yet shown")
        XCTAssertFalse(agent.menuShown(in: window))
        let a = menu(["Rename", "Delete", "Delete"], at: CGPoint(x: 20, y: 100))
        XCTAssertTrue(agent.menuShown(in: window))
        XCTAssertEqual(try agent.shownRow(node(7), of: node(2), in: window).get(), CGRect(x: 20, y: 180, width: 200, height: 40), "the second Delete, by its place")
        XCTAssertEqual(try agent.shownRow(node(5), of: node(2), in: window).get(), CGRect(x: 20, y: 140, width: 200, height: 40))
        // Popover 2's menu was dismissed and popover 8's, reading the same, is up: 2's row is not in it.
        present([8])
        XCTAssertTrue(why(agent.shownRow(node(5), of: node(2), in: window)).contains("not the menu UIKit shows"), "a dismissed menu's row")
        XCTAssertEqual(try agent.shownRow(node(2), of: node(8), in: window).get(), CGRect(x: 20, y: 180, width: 200, height: 40))
        // Two popovers of the presented menu (a menu and its submenu) that read the same.
        present([2, 8])
        XCTAssertTrue(why(agent.shownRow(node(5), of: node(2), in: window)).contains("read the same"))
        present([4])
        let b = menu(["Copy", "Copy, link"], at: CGPoint(x: 20, y: 400))
        XCTAssertEqual(try agent.shownRow(node(9), of: node(4), in: window).get(), CGRect(x: 20, y: 400, width: 200, height: 40), "Copy, not Copy, link")
        XCTAssertEqual(try agent.shownRow(node(6), of: node(4), in: window).get(), CGRect(x: 20, y: 440, width: 200, height: 40))
        present([2])
        let twin = menu(["Rename", "Delete", "Delete"], at: CGPoint(x: 220, y: 100))
        XCTAssertTrue(why(agent.shownRow(node(3), of: node(2), in: window)).contains("2 places"), "the menu shown twice")
        for v in [a, b, twin] { v.removeFromSuperview() }
        XCTAssertFalse(agent.menuShown(in: window))
        // A menu that scrolls shows a slice of its rows: one shown is picked;
        // one scrolled out is refused, naming the way that works.
        let long = menu(["Rename", "Delete"], at: CGPoint(x: 20, y: 100), scrolls: true)
        XCTAssertTrue(agent.menuShown(in: window))
        XCTAssertEqual(try agent.shownRow(node(5), of: node(2), in: window).get(), CGRect(x: 20, y: 140, width: 200, height: 40))
        XCTAssertTrue(why(agent.shownRow(node(7), of: node(2), in: window)).contains("scrolled out"))
        long.removeFromSuperview()
        let tail = menu(["Delete"], at: CGPoint(x: 20, y: 100), scrolls: true)
        XCTAssertTrue(why(agent.shownRow(node(7), of: node(2), in: window)).contains("2 places"), "one Delete shown, two it could be")
        tail.removeFromSuperview()
        // A row the node no longer is (a reused view): not in the presented menu.
        p.menus.given[2] = [GivenRow(node(3), title: "Rename", subtitle: nil)]
        XCTAssertTrue(why(agent.shownRow(node(5), of: node(2), in: window)).contains("not the menu UIKit shows"))
    }
}
#endif
