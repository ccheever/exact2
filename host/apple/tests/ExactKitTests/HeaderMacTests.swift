// @ref LLP 1115.002 D2 — a route's `header` as the window's title bar and
// toolbar on the Mac: which header (the shared rule's, the selected route's
// or the one under a modal route), what the window's title is, that its
// buttons are toolbar items pressing the authored buttons, that the bar's
// own Back stands for the header's, that a header the bar cannot show whole
// stays painted, that a declared window toolbar wins, and that the header's
// box leaves layout while the bar shows it.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class HeaderMacTests: XCTestCase {
    private var window: NSWindow!
    private var session: ExactSession?
    override func tearDown() {
        window?.close(); window = nil
        session?.destroy(); session = nil
    }

    private func view(_ id: Int, _ kind: String = "view", _ props: [String: String] = [:], press: Bool = false) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": kind, "props": props, "handlers": press ? ["press"] : []],
         ["op": "frame", "id": id, "x": 0, "y": 0, "w": 60, "h": 30]]
    }
    private func children(_ id: Int, _ ids: [Int]) -> [String: Any] { ["op": "children", "id": id, "ids": ids] }
    private func heading(_ id: Int, _ text: String, level: Int) -> [[String: Any]] {
        view(id, "text", ["text": text, "accessibilityRole": "heading", "accessibilityHeadingLevel": "\(level)"])
    }
    /// A custom button holding one symbol: a bar's usual item.
    private func symbolButton(_ id: Int, _ label: String, _ symbol: String, _ props: [String: String] = [:]) -> [[String: Any]] {
        view(id, "button", ["accessibilityLabel": label].merging(props) { _, new in new }, press: true)
            + view(id * 10, "image", ["symbolName": symbol]) + [children(id, [id * 10])]
    }

    /// A navigation root (1) with two routes. Library (2, key "a") holds a
    /// header (10): a Filter button (11), its level-1 heading (12), an Add
    /// button (13) and a native Save button (14, `filled`); then its page
    /// (15). Detail (3, key "b"), pushed, holds a header (20): the stack's
    /// Back control (21), its heading (22) and a Share button (23, text);
    /// then its page (25). `extra` is applied in the same batch, before the
    /// roots are set.
    private func fixture(selected: String = "a", extra: [[String: Any]] = []) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 700, height: 400), styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.title = "Shelf"
        window.contentView = p.viewport
        p.buttonFace = { id in
            var face = ButtonFace()
            if id == 14 { face.title = "Save"; face.style = "filled" }
            return face
        }
        var ops: [[String: Any]] = view(1, "view", ["navigationBack": "back", "navigationKey": selected])
        ops += view(2, "view", ["navigationKey": "a"]) + view(10, "view", ["semanticTag": "header"])
        ops += symbolButton(11, "Filter", "line.3.horizontal.decrease") + heading(12, "Library", level: 1) + symbolButton(13, "Add Book", "plus", ["testId": "add"])
        ops += view(14, "control", ["type": "button", "accessibilityRole": "button", "buttonStyle": "filled"], press: true) + view(15)
        ops += [children(10, [11, 12, 13, 14]), children(2, [10, 15])]
        ops += view(3, "view", ["navigationKey": "b"]) + view(20, "view", ["semanticTag": "header"])
        ops += symbolButton(21, "Back", "chevron.backward", ["id": "back"]) + heading(22, "Detail", level: 2)
        ops += view(23, "button", [:], press: true) + view(231, "text", ["text": "Share"]) + [children(23, [231])] + view(25)
        ops += [children(20, [21, 22, 23]), children(3, [20, 25])]
        ops += extra
        ops += [children(1, [2, 3]), ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        return p
    }
    private func select(_ key: String, in p: Presenter) {
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["navigationKey": key]]]))
    }
    private func node(_ p: Presenter, _ id: UInt32) throws -> NodeView { try XCTUnwrap(p.views[id]) }
    private func identifiers() -> [String] { window.toolbar?.items.map(\.itemIdentifier.rawValue) ?? [] }
    private let back = WindowToolbarHost.backIdentifier.rawValue

    func testARIAUnavailableToolbarViewerRemainsPressable() throws {
        let p = fixture(extra: [["op": "props", "id": 13, "set": ["accessibilityDisabled": "true"]]])
        XCTAssertTrue(p.toolbar.attach(to: window))
        window.orderFrontRegardless()
        window.contentView?.superview?.layoutSubtreeIfNeeded()
        func find(_ element: NSAccessibilityProtocol) -> NSAccessibilityProtocol? {
            if element.accessibilityLabel() == "Add Book" { return element }
            for case let child as NSAccessibilityProtocol in element.accessibilityChildren() ?? [] {
                if let found = find(child) { return found }
            }
            return nil
        }
        let viewer = try XCTUnwrap(find(window))
        let item = try XCTUnwrap(p.toolbar.items[13])
        XCTAssertNil(item.view, "keep AppKit's standard toolbar presentation")
        XCTAssertTrue(item.isEnabled, "ARIA does not disable input")
        XCTAssertFalse(viewer.isAccessibilityEnabled(), "the exposed viewer carries ARIA disabled")
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        _ = viewer.accessibilityPerformPress()
        XCTAssertEqual(pressed, [13])
        p.apply(wireBatch([["op": "props", "id": 13, "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertTrue(try XCTUnwrap(find(window)).isAccessibilityEnabled())
        p.toolbar.detach()
    }

    func testARoutesHeaderIsTheWindowsTitleAndItsButtonsAreToolbarItems() throws {
        let p = fixture()
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        // Mounting claims no chrome: the header paints, the window keeps its title.
        XCTAssertNil(window.toolbar)
        XCTAssertFalse(try node(p, 10).isHidden)
        XCTAssertEqual(window.title, "Shelf")
        XCTAssertEqual(p.covers.delivered, [:])

        XCTAssertTrue(p.toolbar.attach(to: window))
        let toolbar = try XCTUnwrap(window.toolbar)
        XCTAssertEqual(toolbar.displayMode, .iconOnly)
        // The screen's title is the window's; a level-1 heading is a title like any other on the Mac.
        XCTAssertEqual(window.title, "Library")
        // Back first, then the button before the heading as a navigational item, then those after it.
        XCTAssertEqual(identifiers(), [back, "exact.command.11", "exact.command.13", "exact.command.14"])
        let filter = try XCTUnwrap(p.toolbar.items[11]), add = try XCTUnwrap(p.toolbar.items[13]), save = try XCTUnwrap(p.toolbar.items[14])
        XCTAssertTrue(filter.isNavigational, "a button before the heading leads the bar")
        XCTAssertFalse(add.isNavigational)
        XCTAssertEqual([filter.label, add.label, save.label], ["Filter", "Add Book", "Save"])
        XCTAssertNotNil(add.image, "its symbol")
        XCTAssertEqual(add.title, "", "a symbol-only button is an icon item")
        XCTAssertEqual(save.title, "Save", "a button with a title shows it")
        if #available(macOS 26.0, *), LinkedDesign.liquidGlass {
            XCTAssertEqual(save.style, .prominent, "a filled button is the prominent item")
            XCTAssertEqual(add.style, .plain)
        }
        XCTAssertFalse(p.toolbar.backItem.isEnabled, "nowhere to go back to at the stack's root")
        // The header is the platform's: hidden, and its box out of layout.
        XCTAssertTrue(try node(p, 10).isHidden)
        XCTAssertTrue(p.toolbar.hides(try node(p, 10)))
        XCTAssertEqual(p.covers.delivered, [10: .whole])
        // An item presses its authored button, by a person or by name.
        XCTAssertTrue(NSApp.sendAction(try XCTUnwrap(add.action), to: add.target, from: add))
        XCTAssertEqual(p.toolbar.activate(try node(p, 13)), true)
        XCTAssertEqual(pressed, [13, 13])
        XCTAssertNil(p.toolbar.activate(try node(p, 15)), "a node that is no part of the bar")
        // A batch that only places a box (a scroll's, an animation's) reads nothing of the bar and leaves it standing.
        let placed = wireBatch([["op": "frame", "id": 15, "x": 0, "y": 0, "w": 700, "h": 400]])
        XCTAssertFalse(p.toolbar.concerns(placed))
        XCTAssertTrue(p.toolbar.concerns(wireBatch([["op": "props", "id": 13, "set": ["accessibilityLabel": "Add Book"]]])))
        p.apply(placed)
        XCTAssertTrue(try node(p, 10).isHidden)
        XCTAssertTrue(p.toolbar.projected)
        XCTAssertEqual(identifiers(), [back, "exact.command.11", "exact.command.13", "exact.command.14"])
        // What `state` says of it.
        let summary = try XCTUnwrap(p.toolbar.summary["header"] as? [String: Any])
        XCTAssertEqual(summary["title"] as? String, "Library")
        XCTAssertEqual((summary["items"] as? [[String: Any]])?.map { $0["place"] as? String }, ["leading", "trailing", "trailing"])
        XCTAssertEqual((summary["items"] as? [[String: Any]])?.compactMap { $0["testId"] as? String }, ["add"])

        // A push: the pushed route's header is the bar, and the bar's own Back stands for the header's.
        select("b", in: p)
        XCTAssertTrue(window.toolbar === toolbar, "one toolbar through every route")
        XCTAssertEqual(window.title, "Detail")
        XCTAssertEqual(identifiers(), [back, "exact.command.23"], "the header's Back control is no second Back")
        XCTAssertEqual(try XCTUnwrap(p.toolbar.items[23]).title, "Share")
        XCTAssertTrue(try node(p, 20).isHidden)
        XCTAssertEqual(p.covers.delivered, [20: .whole], "the route beneath is no longer the bar's")
        XCTAssertTrue(p.toolbar.backItem.isEnabled)
        XCTAssertTrue(NSApp.sendAction(try XCTUnwrap(p.toolbar.backItem.action), to: p.toolbar.backItem.target, from: p.toolbar.backItem))
        XCTAssertEqual(pressed.last, 21, "Back presses the authored control, hidden as it is")
        XCTAssertEqual(p.toolbar.activate(try node(p, 21)), true, "and a driver's tap on that control is the bar's Back")
        XCTAssertEqual(pressed.suffix(2), [21, 21])

        // The title follows the heading's words; an authored document title is over it, and the app's name under it.
        p.apply(wireBatch([["op": "props", "id": 22, "set": ["text": "Piranesi"]]]))
        XCTAssertEqual(window.title, "Piranesi")
        p.headTitle("My Document")
        XCTAssertEqual(window.title, "My Document", "the authored document title wins")
        p.headTitle(nil)
        XCTAssertEqual(window.title, "Piranesi")
        p.apply(wireBatch([["op": "props", "id": 22, "set": ["text": ""]]]))
        XCTAssertEqual(window.title, "Shelf", "a heading with no words leaves the app's name")

        // A disabled button's item is disabled; one hidden as authored has no item.
        select("a", in: p)
        p.apply(wireBatch([["op": "props", "id": 13, "set": ["disabled": "true"]]]))
        XCTAssertFalse(try XCTUnwrap(p.toolbar.items[13]).isEnabled)
        XCTAssertEqual(p.toolbar.activate(try node(p, 13)), false)
        p.apply(wireBatch([["op": "style", "id": 11, "style": ["display": "none"]]]))
        XCTAssertEqual(identifiers(), [back, "exact.command.13", "exact.command.14"])

        // The owner lets the toolbar go: the header paints again and its box is its own.
        p.toolbar.detach()
        XCTAssertNil(window.toolbar)
        XCTAssertFalse(try node(p, 10).isHidden)
        XCTAssertEqual(p.covers.delivered, [:])
        XCTAssertEqual(window.title, "Shelf")
    }

    func testAHeaderTheBarCannotShowWholeStaysPaintedWhole() throws {
        // Each of these in Library's header is something a toolbar has no place for.
        let cases: [(String, [[String: Any]], [Int])] = [
            ("a picture", view(16, "image", ["imageSource": "assets/logo.png"]), [11, 12, 13, 14, 16]),
            ("a second line of text", view(16, "text", ["text": "3 books"]), [11, 12, 13, 14, 16]),
            ("a field", view(16, "input", ["type": "text"]), [11, 12, 13, 14, 16]),
            ("a button drawn as a filled box", view(16, "button", ["accessibilityLabel": "Profile"], press: true) + view(160) + view(161, "text", ["text": "AB"]) + [children(160, [161]), children(16, [160])], [11, 12, 13, 14, 16]),
            ("a pressable around the heading", view(16, "view", [:], press: true) + [children(16, [12])], [11, 16, 13, 14]),
        ]
        for (what, extra, header) in cases {
            let p = fixture(extra: extra + [children(10, header)])
            XCTAssertTrue(p.toolbar.attach(to: window))
            XCTAssertEqual(window.title, "Shelf", what)
            XCTAssertEqual(identifiers(), [back], "\(what): none of its buttons is an item")
            XCTAssertFalse(try node(p, 10).isHidden, "\(what): the header paints")
            XCTAssertEqual(p.covers.delivered, [:], what)
            XCTAssertNil(p.toolbar.summary["header"] as? [String: Any], what)
            XCTAssertTrue((p.toolbar.summary["headerKept"] as? String)?.contains("kept as authored") == true, "\(what): said, and why")
            XCTAssertNil(p.toolbar.activate(try node(p, 13)), "\(what): its buttons are the page's to press")
            // The route beside it projects as before.
            select("b", in: p)
            XCTAssertEqual(window.title, "Detail", what)
            // And once the header holds only what a bar shows, it is the bar.
            select("a", in: p)
            p.apply(wireBatch([children(10, [11, 12, 13, 14])]))
            XCTAssertEqual(window.title, "Library", what)
            XCTAssertEqual(identifiers(), [back, "exact.command.11", "exact.command.13", "exact.command.14"], what)
            XCTAssertTrue(try node(p, 10).isHidden, what)
            p.toolbar.detach()
            window.close()
        }
        // What is not shown as authored (`display: none`) is nothing to place.
        let p = fixture(extra: view(16, "image", ["imageSource": "assets/logo.png"]) + [["op": "style", "id": 16, "style": ["display": "none"]], children(10, [11, 12, 13, 14, 16])])
        XCTAssertTrue(p.toolbar.attach(to: window))
        XCTAssertEqual(window.title, "Library")
        p.toolbar.detach()
        window.close()
        // A box that only groups (a row around the trailing buttons) is no part of the bar: what it holds is.
        let q = fixture(extra: view(16) + [children(16, [13, 14]), children(10, [11, 12, 16])])
        XCTAssertTrue(q.toolbar.attach(to: window))
        XCTAssertEqual(window.title, "Library")
        XCTAssertEqual(identifiers(), [back, "exact.command.11", "exact.command.13", "exact.command.14"])
        q.toolbar.detach()
    }

    /// `appearance: none` written on the header keeps it the page's (LLP
    /// 1075.003 §9.10, the shared rule's `AUTHORED`); on one of its buttons
    /// the whole header paints, since the window's bar shows a header whole
    /// or not at all. Implied (not written), it is no opt-out.
    func testAHeaderOrAButtonThatWritesAppearanceNoneStaysPainted() throws {
        for (id, written) in [(10, true), (13, true), (13, false)] {
            let style: [String: Any] = written ? ["appearance": "none", "written": ["appearance"]] : ["appearance": "none"]
            let p = fixture(extra: [["op": "style", "id": id, "style": style]])
            XCTAssertTrue(p.toolbar.attach(to: window))
            let what = "#\(id) \(written ? "written" : "implied")"
            if written {
                XCTAssertEqual(window.title, "Shelf", what)
                XCTAssertFalse(try node(p, 10).isHidden, "\(what): the header paints")
                XCTAssertNil(p.toolbar.activate(try node(p, 13)), "\(what): its buttons are the page's to press")
            } else {
                XCTAssertEqual(window.title, "Library", what)
                XCTAssertTrue(try node(p, 10).isHidden, what)
            }
            p.toolbar.detach()
            window.close()
        }
    }

    /// A route that says it has no bar keeps its header in the page, and a
    /// route with no header leaves the window the app's name.
    func testARouteThatAsksForNoBarOrHasNoHeaderProjectsNothing() throws {
        let p = fixture(extra: [["op": "props", "id": 2, "set": ["navigationBar": "none"]]])
        XCTAssertTrue(p.toolbar.attach(to: window))
        XCTAssertEqual(window.title, "Shelf")
        XCTAssertFalse(try node(p, 10).isHidden)
        XCTAssertEqual(identifiers(), [], "no bar asked for: no Back item and none of the header's")
        XCTAssertEqual(p.covers.delivered, [:])
        p.apply(wireBatch([["op": "props", "id": 2, "set": [:], "clear": ["navigationBar"]]]))
        XCTAssertEqual(window.title, "Library")
        // The header goes: the title is the app's again, and Back stays.
        p.apply(wireBatch([children(2, [15])]))
        XCTAssertEqual(window.title, "Shelf")
        XCTAssertEqual(identifiers(), [back])
        XCTAssertEqual(p.covers.delivered, [:])
        p.toolbar.detach()
    }

    /// A route whose first child is no header is answered before its
    /// subtree is read: a feed's rows are not handed to the rule at each
    /// projection, as they were several times a scroll batch (the Bluesky
    /// clone's fling hitch, 2026-10-13). A header first is walked, and only
    /// its own subtree.
    func testARouteWhoseFirstChildIsNoHeaderIsAnsweredWithoutWalkingIt() throws {
        let rows = Array(31..<71)
        let feed = view(30) + rows.flatMap { view($0, "text", ["text": "row \($0)"]) } + [children(30, rows), children(2, [30, 10, 15])]
        let p = fixture(extra: feed)
        let before = HeaderRule.walked
        XCTAssertNil(HeaderRule(route: try node(p, 2), back: "back", backIsPlatforms: false, projected: true))
        XCTAssertEqual(HeaderRule.walked, before, "nothing of the feed is walked")
        XCTAssertNotNil(HeaderRule(route: try node(p, 3), back: "back", backIsPlatforms: false, projected: true))
        XCTAssertEqual(HeaderRule.walked - before, 6, "the header (20), Back and its symbol (21, 210), the heading (22), Share and its text (23, 231); not the page (25)")
    }

    /// A modal route is a sheet (D7.4, `RouteSheetMacTests`): the window
    /// keeps the title and the items of the route it stands over, which
    /// cannot be pressed under it, and the modal route's own header is its
    /// sheet's: the heading its title row, the stack's Back control its
    /// cancelling button, and what the sheet has no row for (Share) still
    /// in the header.
    func testAModalRouteIsASheetAndTheWindowKeepsTheRouteBeneath() throws {
        let p = fixture(extra: [["op": "props", "id": 3, "set": ["navigationPresentation": "modal"]]])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertTrue(p.toolbar.attach(to: window))
        XCTAssertEqual(window.title, "Library")
        select("b", in: p)
        XCTAssertEqual(window.title, "Library", "a sheet does not rename its window")
        XCTAssertEqual(identifiers(), [back, "exact.command.11", "exact.command.13", "exact.command.14"])
        XCTAssertTrue(try node(p, 10).isHidden)
        let sheet = try XCTUnwrap(window.attachedSheet), chrome = try XCTUnwrap(sheet.contentView as? RouteChrome)
        XCTAssertTrue(try node(p, 3).window === sheet, "the route is in its sheet")
        XCTAssertEqual(chrome.title.stringValue, "Detail")
        XCTAssertEqual([chrome.cancel.isHidden, chrome.confirm.isHidden], [false, true], "Back cancels; nothing in it confirms")
        XCTAssertEqual(chrome.cancel.title, "Back", "a symbol's button says its name")
        XCTAssertFalse(try node(p, 20).isHidden, "what the sheet has no row for stays in the header")
        XCTAssertEqual([try node(p, 21).isHidden, try node(p, 22).isHidden, try node(p, 23).isHidden], [true, true, false])
        XCTAssertEqual(p.covers.delivered, [10: .whole, 21: .whole, 22: .whole])
        let add = try XCTUnwrap(p.toolbar.items[13])
        XCTAssertFalse(p.toolbar.validateToolbarItem(add), "the route beneath is inert under it")
        XCTAssertFalse(add.isEnabled)
        XCTAssertFalse(p.toolbar.backItem.isEnabled, "a modal route's close is its own")
        XCTAssertNil(p.toolbar.activate(try node(p, 23)), "its buttons are in the sheet")
        XCTAssertEqual(p.dialogs.activate(try node(p, 21)), true, "the sheet's row presses the authored button")
        XCTAssertEqual(pressed, [21])
        select("a", in: p)
        XCTAssertNil(window.attachedSheet, "the sheet leaves with the route")
        XCTAssertEqual([try node(p, 21).isHidden, try node(p, 22).isHidden], [false, false], "the header is as authored again")
        XCTAssertEqual(p.covers.delivered, [10: .whole])
        XCTAssertTrue(p.toolbar.validateToolbarItem(add))
        XCTAssertTrue(add.isEnabled)
        p.toolbar.detach()
    }

    /// A toolbar the author declared for the window is the bar: the route's
    /// header is not projected into it, whatever the header holds.
    func testADeclaredWindowToolbarWinsOverTheHeader() throws {
        let declared = view(30, "view", ["accessibilityRole": "toolbar", "toolbarPlacement": "window"]) + symbolButton(31, "Compose", "square.and.pencil") + [children(30, [31]), children(2, [10, 15, 30])]
        let p = fixture(extra: declared)
        XCTAssertTrue(p.toolbar.attach(to: window))
        XCTAssertEqual(identifiers(), [back, "exact.command.31"])
        XCTAssertEqual(window.title, "Shelf", "the declared toolbar has no heading: the window's own title")
        XCTAssertFalse(try node(p, 10).isHidden, "the header paints, whole")
        XCTAssertTrue(try node(p, 30).isHidden, "the declared toolbar is the window's")
        XCTAssertEqual(p.covers.delivered, [30: .whole])
        // The route without one projects its header as any other.
        select("b", in: p)
        XCTAssertEqual(window.title, "Detail")
        XCTAssertEqual(identifiers(), [back, "exact.command.23"])
        XCTAssertEqual(p.covers.delivered, [20: .whole])
        select("a", in: p)
        XCTAssertEqual(identifiers(), [back, "exact.command.31"])
        XCTAssertEqual(window.title, "Shelf")
        XCTAssertEqual(p.covers.delivered, [30: .whole])
        p.toolbar.detach()
    }

    /// What a header button invokes, as HTML resolves it, decides whether
    /// the window's bar can hold it: a button whose `commandfor` hides a form
    /// popover is a command, though its `popovertarget` names that form; one
    /// whose invocation opens the form keeps the header painted, as the form
    /// is placed against the button's box; a command-only `toggle-popover`
    /// of a menu is the bar's menu item.
    func testWhatAHeaderButtonInvokesDecidesItsPlaceInTheBar() throws {
        /// Whether the header is the window's bar (titled by its heading), and the bar's items.
        func attach(_ props: [String: String]) -> (inBar: Bool, identifiers: [String]) {
            let extra = symbolButton(16, "More", "ellipsis.circle", props)
                + view(40, "view", ["popover": "auto", "id": "more", "accessibilityRole": "menu"])
                + view(41, "button", ["popovertarget": "more", "popovertargetaction": "hide"], press: true) + view(411, "text", ["text": "Rename"])
                + view(50, "view", ["popover": "auto", "id": "form"]) + view(51, "input", ["value": ""])
                + [children(41, [411]), children(40, [41]), children(50, [51]), children(10, [11, 12, 13, 14, 16]), children(2, [10, 15, 40, 50])]
            let p = fixture(extra: extra)
            defer { window.close() }
            XCTAssertTrue(p.toolbar.attach(to: window))
            return (window.title == "Library", identifiers())
        }
        XCTAssertTrue(attach(["popovertarget": "form", "commandfor": "form", "command": "hide-popover"]).inBar, "its commandfor governs: a command")
        XCTAssertFalse(attach(["popovertarget": "form"]).inBar, "it opens the form, which needs the button's box")
        XCTAssertTrue(attach(["commandfor": "more", "command": "toggle-popover"]).identifiers.contains("exact.menu.16"), "a command-only menu item")
    }

    /// A header's button that opens a menu is AppKit's menu item: the menu
    /// is the item's own to pop up, filled from the popover's rows as it
    /// opens, after the button's own press; a chosen row presses its
    /// authored row. A popover that is no menu needs the button's box, so
    /// its header stays painted.
    func testAButtonThatOpensAMenuIsAMenuItem() throws {
        // More (16) opens a menu (40) of two rows; it also hears `press`.
        let menu = symbolButton(16, "More", "ellipsis.circle", ["popovertarget": "more", "testId": "more"])
            + view(40, "view", ["popover": "auto", "id": "more", "accessibilityRole": "menu"])
            + view(41, "button", ["popovertarget": "more", "popovertargetaction": "hide"], press: true) + view(411, "text", ["text": "Rename"]) + [children(41, [411])]
            + view(42, "button", ["popovertarget": "more", "popovertargetaction": "hide", "disabled": "true"], press: true) + view(421, "text", ["text": "Delete"]) + [children(42, [421])]
            + [children(40, [41, 42]), children(10, [11, 12, 13, 14, 16]), children(2, [10, 15, 40])]
        let p = fixture(extra: menu)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertTrue(p.toolbar.attach(to: window))
        XCTAssertEqual(window.title, "Library")
        XCTAssertEqual(identifiers(), [back, "exact.command.11", "exact.command.13", "exact.command.14", "exact.menu.16"])
        let item = try XCTUnwrap(p.toolbar.items[16] as? NSMenuToolbarItem)
        XCTAssertEqual(item.label, "More")
        XCTAssertNotNil(item.image)
        XCTAssertTrue(item.showsIndicator, "AppKit's own mark of a menu, as SwiftUI's Menu has in a toolbar")
        XCTAssertNil(item.action, "AppKit shows the menu: nothing of Exact's is its click")
        XCTAssertTrue(item.isEnabled)
        XCTAssertFalse(p.toolbar.commands.contains(16), "a menu item is no command for the menu bar")
        XCTAssertTrue(p.toolbar.commands.contains(13))
        XCTAssertEqual(item.menu.numberOfItems, 0, "nothing is read until it opens")
        // As AppKit opens it.
        item.menu.delegate?.menuNeedsUpdate?(item.menu)
        XCTAssertEqual(pressed, [16], "the button's own press runs first")
        XCTAssertEqual(item.menu.items.map(\.title), ["Rename", "Delete"])
        XCTAssertEqual(item.menu.items.map(\.isEnabled), [true, false])
        let rename = item.menu.items[0]
        XCTAssertTrue(NSApp.sendAction(try XCTUnwrap(rename.action), to: rename.target, from: rename))
        // Pressed on the next turn, once the menu's tracking has unwound.
        let limit = Date().addingTimeInterval(5)
        while pressed.count < 2, Date() < limit { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertEqual(pressed, [16, 41])
        // Opened again it is read again.
        p.apply(wireBatch([["op": "props", "id": 411, "set": ["text": "Rename…"]]]))
        item.menu.delegate?.menuNeedsUpdate?(item.menu)
        XCTAssertEqual(item.menu.items.map(\.title), ["Rename…", "Delete"])
        // A key's search for an equivalent opens nothing and presses nothing.
        pressed = []
        var target: AnyObject?, action: Selector?
        let key = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0, windowNumber: window.windowNumber, context: nil, characters: "r", charactersIgnoringModifiers: "r", isARepeat: false, keyCode: 15))
        XCTAssertEqual(item.menu.delegate?.menuHasKeyEquivalent?(item.menu, for: key, target: &target, action: &action), false)
        XCTAssertEqual(pressed, [])
        p.toolbar.detach()
        window.close()

        // A popover that is no menu (it holds a field) is placed against its button's box: the header keeps it.
        let form = symbolButton(16, "Rename", "pencil", ["popovertarget": "rename"])
            + view(40, "view", ["popover": "auto", "id": "rename"]) + view(41, "input", ["type": "text"])
            + [children(40, [41]), children(10, [11, 12, 13, 14, 16]), children(2, [10, 15, 40])]
        let q = fixture(extra: form)
        XCTAssertTrue(q.toolbar.attach(to: window))
        XCTAssertEqual(window.title, "Shelf")
        XCTAssertFalse(try node(q, 10).isHidden)
        XCTAssertTrue((q.toolbar.summary["headerKept"] as? String)?.contains("neither a menu nor an alert") == true)
        q.toolbar.detach()
        window.close()

        // A confirmation in the alert's shape is a sheet on the window, which needs no box: a plain item asks.
        let ask = symbolButton(16, "Delete", "trash", ["popovertarget": "sure"])
            + view(40, "view", ["popover": "auto", "id": "sure", "accessibilityRole": "alertdialog", "accessibilityLabel": "Delete Book?"])
            + view(41, "button", ["popovertarget": "sure", "popovertargetaction": "hide"], press: true) + view(411, "text", ["text": "Delete"]) + [children(41, [411])]
            + view(42, "button", ["popovertarget": "sure", "popovertargetaction": "hide"]) + view(421, "text", ["text": "Cancel"]) + [children(42, [421])]
            + view(43, "text", ["text": "This can't be undone."])
            + [children(40, [43, 41, 42]), children(10, [11, 12, 13, 14, 16]), children(2, [10, 15, 40])]
        let r = fixture(extra: ask)
        XCTAssertTrue(r.toolbar.attach(to: window))
        XCTAssertEqual(window.title, "Library")
        XCTAssertEqual(identifiers().last, "exact.command.16")
        XCTAssertFalse(r.toolbar.items[16] is NSMenuToolbarItem)
        r.toolbar.detach()
    }

    /// A header's tablist of text tabs is the bar's centred group: a segment
    /// a tab, pressing its authored tab, showing the tab the app selected.
    func testATablistOfTextTabsIsTheBarsCentredSegmentedGroup() throws {
        func tab(_ id: Int, _ title: String, selected: Bool = false) -> [[String: Any]] {
            view(id, "button", ["accessibilityRole": "tab", "accessibilitySelected": selected ? "true" : "false", "testId": "tab-\(title)"], press: true)
                + view(id * 10, "text", ["text": title]) + [children(id, [id * 10])]
        }
        let tablist = view(17, "view", ["accessibilityRole": "tablist", "accessibilityLabel": "Filter"])
            + tab(171, "All", selected: true) + tab(172, "Unread") + tab(173, "Flagged")
            + [children(17, [171, 172, 173]), children(10, [11, 12, 17, 13, 14])]
        let p = fixture(extra: tablist)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        // As authored the page makes its own segmented control of it.
        XCTAssertNotNil(p.segments.control(of: 17))

        XCTAssertTrue(p.toolbar.attach(to: window))
        let toolbar = try XCTUnwrap(window.toolbar)
        XCTAssertEqual(window.title, "Library")
        let group = try XCTUnwrap(p.toolbar.segments.item)
        // SwiftUI's own order: the centred group, a flexible space, then the trailing items.
        XCTAssertEqual(identifiers(), [back, "exact.command.11", group.itemIdentifier.rawValue, NSToolbarItem.Identifier.flexibleSpace.rawValue, "exact.command.13", "exact.command.14"])
        XCTAssertEqual(toolbar.centeredItemIdentifiers, [group.itemIdentifier])
        XCTAssertEqual(group.selectionMode, .selectOne)
        XCTAssertEqual(group.subitems.map(\.title), ["All", "Unread", "Flagged"])
        XCTAssertEqual(group.label, "Filter", "the tablist's name is the group's")
        XCTAssertEqual(group.selectedIndex, 0)
        XCTAssertNil(p.segments.control(of: 17), "the bar shows it: the page makes no control of it")
        XCTAssertTrue(p.toolbar.adopts(try node(p, 17)))
        XCTAssertFalse(try node(p, 172).isHidden, "its tabs are hidden by the header alone")

        // A person's choice presses the tab. The app has not answered, so the group shows the app's tab still.
        group.selectedIndex = 1
        XCTAssertTrue(NSApp.sendAction(try XCTUnwrap(group.action), to: group.target, from: group))
        XCTAssertEqual(pressed, [172])
        XCTAssertEqual(group.selectedIndex, 0, "the selection is the app's")
        // The app answers.
        p.apply(wireBatch([["op": "props", "id": 171, "set": ["accessibilitySelected": "false"]], ["op": "props", "id": 172, "set": ["accessibilitySelected": "true"]]]))
        XCTAssertTrue(p.toolbar.segments.item === group, "the same group while its tabs are the same")
        XCTAssertEqual(group.selectedIndex, 1)
        // By name.
        XCTAssertEqual(p.toolbar.activate(try node(p, 173)), true)
        XCTAssertEqual(pressed, [172, 173])
        // A disabled tab's segment is disabled and chooses nothing.
        p.apply(wireBatch([["op": "props", "id": 173, "set": ["disabled": "true"]]]))
        XCTAssertEqual(group.subitems.map(\.isEnabled), [true, true, false])
        XCTAssertEqual(p.toolbar.activate(try node(p, 173)), false)
        group.selectedIndex = 2
        XCTAssertTrue(NSApp.sendAction(try XCTUnwrap(group.action), to: group.target, from: group))
        XCTAssertEqual(pressed, [172, 173])
        XCTAssertEqual(group.selectedIndex, 1)
        // What `state` says of it.
        let summary = try XCTUnwrap((p.toolbar.summary["header"] as? [String: Any])?["segments"] as? [String: Any])
        XCTAssertEqual(summary["selected"] as? Int, 1)
        XCTAssertEqual((summary["tabs"] as? [[String: Any]])?.compactMap { $0["testId"] as? String }, ["tab-All", "tab-Unread", "tab-Flagged"])

        // New words make a new group: a group's segments are made with it.
        p.apply(wireBatch([["op": "props", "id": 1720, "set": ["text": "New"]]]))
        let renamed = try XCTUnwrap(p.toolbar.segments.item)
        XCTAssertFalse(renamed === group)
        XCTAssertEqual(renamed.subitems.map(\.title), ["All", "New", "Flagged"])
        XCTAssertEqual(toolbar.centeredItemIdentifiers, [renamed.itemIdentifier])
        XCTAssertFalse(identifiers().contains(group.itemIdentifier.rawValue))
        XCTAssertEqual(renamed.selectedIndex, 1)

        // A tab that shows a symbol is no segment of a toolbar's group: the header stays painted, whole.
        p.apply(wireBatch(view(1721, "image", ["symbolName": "tray"]) + [children(172, [1721, 1720])]))
        XCTAssertEqual(window.title, "Shelf")
        XCTAssertFalse(try node(p, 10).isHidden)
        XCTAssertEqual(identifiers(), [back])
        XCTAssertTrue(toolbar.centeredItemIdentifiers.isEmpty)
        XCTAssertTrue((p.toolbar.summary["headerKept"] as? String)?.contains("tablist") == true)
        XCTAssertFalse(p.toolbar.adopts(try node(p, 17)))
        p.apply(wireBatch([children(172, [1720])]))
        XCTAssertEqual(window.title, "Library")

        // Let go: the page's own control is made of it again.
        p.toolbar.detach()
        XCTAssertNotNil(p.segments.control(of: 17))
    }

    /// A header's search input is the bar's search item, and the field in
    /// it is the input's own: its value, its typing, its focus and its test
    /// id are the authored node's, in a field AppKit draws and places.
    func testASearchInputIsTheBarsSearchItemHoldingTheInputsOwnField() throws {
        let input: [[String: Any]] = [
            ["op": "create", "id": 18, "kind": "input", "props": ["type": "search", "placeholder": "Search", "testId": "search", "accessibilityLabel": "Search books", "value": "Pi"],
             "handlers": ["input", "change", "focus", "blur"]],
            ["op": "frame", "id": 18, "x": 0, "y": 0, "w": 200, "h": 24],
            children(10, [11, 12, 13, 14, 18]),
        ]
        let p = fixture(extra: input)
        var typed: [String] = [], focused: [UInt32] = []
        p.onInput = { _, value in typed.append(value) }
        p.onFocus = { focused.append($0) }
        let node = try self.node(p, 18)
        let page = try XCTUnwrap(node.field as? SearchField)
        XCTAssertTrue(page.superview === node)
        XCTAssertEqual(page.stringValue, "Pi")
        XCTAssertNil(node.pageField)

        // Shown, off every screen, never key and never active: AppKit lays a toolbar out for a window that is shown.
        window.setFrameOrigin(NSPoint(x: -20000, y: -20000))
        window.orderFrontRegardless()
        XCTAssertTrue(p.toolbar.attach(to: window))
        XCTAssertEqual(window.title, "Library")
        // SwiftUI's own order: the trailing items, a flexible space, the search item last.
        XCTAssertEqual(identifiers(), [back, "exact.command.11", "exact.command.13", "exact.command.14", NSToolbarItem.Identifier.flexibleSpace.rawValue, "exact.search.18"])
        let item = try XCTUnwrap(p.toolbar.search.item)
        XCTAssertTrue(window.toolbar?.items.last === item)
        // The field in the bar is the node's own field: its class, the node its delegate, the value it held.
        let field = try XCTUnwrap(node.field as? SearchField)
        XCTAssertTrue(item.searchField === field)
        XCTAssertFalse(field === page, "one never styled by the page, so the item draws it as its own")
        XCTAssertTrue(node.pageField === page, "the page's field waits in the node")
        XCTAssertTrue((field as NSTextField).delegate === node)
        XCTAssertEqual(field.stringValue, "Pi")
        XCTAssertEqual(field.placeholderString, "Search")
        XCTAssertEqual(field.accessibilityIdentifier(), "search")
        XCTAssertEqual(field.accessibilityLabel(), "Search books")
        XCTAssertEqual(item.label, "Search books", "the input's name is the item's")
        XCTAssertTrue(field.isBezeled, "AppKit's own field: nothing of the page's look is written to it")
        XCTAssertTrue(p.keyTarget(field) === node, "the focus in the bar's field is the input's")

        // The app's value goes to it; what is typed in it is the input's `input`.
        p.apply(wireBatch([["op": "props", "id": 18, "set": ["value": "Pira"]]]))
        XCTAssertEqual(field.stringValue, "Pira")
        XCTAssertTrue(item.searchField === field, "the same field across batches")
        field.stringValue = "Dune"
        NotificationCenter.default.post(name: NSControl.textDidChangeNotification, object: field)
        XCTAssertEqual(typed, ["Dune"])
        // A style or a placeholder the page writes does not restyle AppKit's field (the app's answer rides with it).
        let font = field.font
        p.apply(wireBatch([["op": "style", "id": 18, "style": ["font_size": 30]], ["op": "props", "id": 18, "set": ["placeholder": "Find a book", "value": "Dune"]]]))
        XCTAssertEqual(field.font, font)
        XCTAssertTrue(field.isBezeled)
        XCTAssertEqual(field.placeholderString, "Find a book")
        XCTAssertNil(field.placeholderAttributedString)

        // The caret goes to it AppKit's way, by name or by an action's `focus()`; its `focus` is the input's.
        XCTAssertEqual(p.toolbar.activate(node), true)
        let limit = Date().addingTimeInterval(5)
        while field.currentEditor() == nil, Date() < limit { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        let editor = try XCTUnwrap(field.currentEditor())
        XCTAssertTrue(editor is FieldEditor, "it edits in the session's editor, as in the page")
        XCTAssertTrue(p.keyTarget(window.firstResponder) === node)
        XCTAssertEqual(focused, [18])
        let summary = try XCTUnwrap((p.toolbar.summary["header"] as? [String: Any])?["search"] as? [String: Any])
        XCTAssertEqual(summary["testId"] as? String, "search")
        XCTAssertEqual(summary["value"] as? String, "Dune")
        XCTAssertEqual(summary["editing"] as? Bool, true)
        window.makeFirstResponder(nil)

        // Another route's bar: the field goes back to the page, holding what the bar's held, in the page's look.
        select("b", in: p)
        XCTAssertFalse(identifiers().contains("exact.search.18"))
        XCTAssertNil(p.toolbar.search.item)
        XCTAssertNil(node.pageField)
        XCTAssertTrue(node.field === page)
        XCTAssertEqual(page.stringValue, "Dune")
        XCTAssertEqual(page.frame.size, node.bounds.size, "laid out in its node again")
        XCTAssertEqual(page.placeholderAttributedString?.string, "Find a book", "what the page wrote while the bar had the field is the page's field's now")
        // And back.
        select("a", in: p)
        let again = try XCTUnwrap(p.toolbar.search.item)
        XCTAssertTrue(again.searchField === node.field)
        XCTAssertEqual(again.searchField.stringValue, "Dune")

        // A search input the author draws (`appearance: none`) is theirs to draw: the header stays painted, whole.
        p.apply(wireBatch([["op": "style", "id": 18, "style": ["appearance": "none"]]]))
        XCTAssertEqual(window.title, "Shelf")
        XCTAssertFalse(try self.node(p, 10).isHidden)
        XCTAssertNil(node.pageField)
        XCTAssertEqual(identifiers(), [back])
        XCTAssertTrue((p.toolbar.summary["headerKept"] as? String)?.contains("search field") == true)
        XCTAssertFalse(node.field is NSSearchField, "its field is the plain one an author draws around")
        p.apply(wireBatch([["op": "style", "id": 18, "style": [:]], ["op": "props", "id": 18, "set": ["placeholder": "Search"]]]))
        XCTAssertEqual(window.title, "Library")
        XCTAssertNotNil(node.pageField)

        // Let go by the owner: the input edits in its page again.
        p.toolbar.detach()
        XCTAssertNil(node.pageField)
        XCTAssertTrue(node.field?.superview === node)
    }

    /// A `destructive` button is an item like any other. In the page a
    /// native destructive button refuses the click that would bring its
    /// window forward, which the driver's window always needs
    /// (`NativeButtonMac.acceptsFirstMouse`); an item's press is no click,
    /// so a tap naming the button presses it.
    func testADestructiveButtonsItemIsPressedByName() throws {
        let delete = view(19, "control", ["type": "button", "accessibilityRole": "button", "destructive": "true", "accessibilityLabel": "Delete", "testId": "delete"], press: true)
        let p = fixture(extra: delete + [children(10, [11, 12, 13, 14, 19])])
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        XCTAssertTrue(p.toolbar.attach(to: window))
        XCTAssertEqual(identifiers().last, "exact.command.19")
        let item = try XCTUnwrap(p.toolbar.items[19])
        XCTAssertEqual(item.label, "Delete")
        XCTAssertTrue(p.toolbar.validateToolbarItem(item))
        XCTAssertEqual(p.toolbar.activate(try node(p, 19)), true)
        XCTAssertTrue(NSApp.sendAction(try XCTUnwrap(item.action), to: item.target, from: item))
        XCTAssertEqual(pressed, [19, 19])
        p.toolbar.detach()
    }

    // MARK: A session, booted

    private let app = """
    routes nav
      library "/"
        book "/book"

    component App
      state added = 0
      action add
        added = added + 1
      action open
        nav = go(nav, "/book")
      action back
        nav = back(nav)
      view
        main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%" position="relative"
          for e in stack(nav) key=e.id
            column navigationKey=`${e.id}` position="absolute" inset=0 testId=`route-${e.name}`
              if e.name == "library"
                header display="flex" align-items="center" padding="8px 16px" testId="header"
                  text "Library" role="heading" aria-level=1 flex=1
                  button press=add aria-label="Add Book" testId="add"
                    image "symbol:add"
                column flex=1 testId="page"
                  button press=open testId="open"
                    text "Open"
                  text `added ${added}` testId="added"
              if e.name == "book"
                header display="flex" align-items="center" padding="8px 16px" testId="book-header"
                  button id="back" press=back testId="back"
                    text "Back"
                  text "Piranesi" role="heading" aria-level=2
                column flex=1 testId="book-page"
                  text "272 pages"

    """

    private func boot(size: CGSize) throws -> ExactSession {
        _ = NSApplication.shared
        let dir = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("../../target/header-test-" + UUID().uuidString).standardizedFileURL
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let contract = dir.appendingPathComponent("app.contract"), plan = dir.appendingPathComponent("app.plan")
        try app.write(to: contract, atomically: true, encoding: .utf8)
        let compiler = Process(), said = Pipe()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", contract.path, "-o", plan.path]
        compiler.standardError = said
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0, String(decoding: said.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self))
        let session = ExactApp.shared.makeSession(label: "header-" + UUID().uuidString)
        self.session = session
        XCTAssertNil(session.boot(plan: try Data(contentsOf: plan), size: size).error)
        return session
    }
    private func named(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId }, testId)
    }

    /// The kernel behind it: the page is laid out without the header while
    /// the bar shows it, an item's press runs the app's action, and Back in
    /// the bar goes back by the header's own control.
    func testTheProjectedHeadersBoxLeavesLayoutAndItsItemsRunTheApp() throws {
        let session = try boot(size: CGSize(width: 600, height: 400))
        let p = session.presenter
        let view = ExactView(session: session)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.title = "Shelf"
        window.contentView = view
        window.layoutIfNeeded()
        let header = try named("header"), page = try named("page")
        XCTAssertGreaterThan(header.frame.height, 0, "as authored the header takes its row")
        XCTAssertEqual(page.frame.minY, header.frame.maxY)

        XCTAssertTrue(view.attachWindowToolbar(to: window))
        window.layoutIfNeeded()
        XCTAssertEqual(window.title, "Library")
        XCTAssertTrue(header.isHidden)
        XCTAssertEqual(p.covers.delivered, [header.id: .whole])
        XCTAssertEqual(page.frame.minY, 0, "the page starts where the header did")
        XCTAssertEqual(page.frame.height, view.bounds.height, "and has the route's whole height")
        let add = try XCTUnwrap(p.toolbar.items[try named("add").id])
        XCTAssertEqual(add.label, "Add Book")
        XCTAssertTrue(NSApp.sendAction(try XCTUnwrap(add.action), to: add.target, from: add))
        XCTAssertEqual(try named("added").accessibleText, "added 1")

        // Pushed: the book's title, one Back, and the bar's Back goes back by the authored control.
        session.apply(session.runtime.press(try named("open").id, now: session.now()))
        XCTAssertEqual(window.title, "Piranesi")
        XCTAssertEqual(identifiers(), [back])
        XCTAssertEqual(try named("book-page").frame.minY, 0)
        XCTAssertEqual(p.covers.delivered, [try named("book-header").id: .whole])
        XCTAssertTrue(p.toolbar.validateToolbarItem(p.toolbar.backItem))
        XCTAssertTrue(NSApp.sendAction(try XCTUnwrap(p.toolbar.backItem.action), to: p.toolbar.backItem.target, from: p.toolbar.backItem))
        XCTAssertEqual(window.title, "Library")
        XCTAssertEqual(p.covers.delivered, [header.id: .whole])
        XCTAssertEqual(page.frame.minY, 0)

        // Let go by the owner: the header is the page's again, laid out as written.
        view.detachWindowToolbar()
        XCTAssertFalse(header.isHidden)
        XCTAssertEqual(p.covers.delivered, [:])
        XCTAssertGreaterThan(header.frame.height, 0)
        XCTAssertEqual(page.frame.minY, header.frame.maxY)
        XCTAssertEqual(window.title, "Shelf")
    }
}
#endif
