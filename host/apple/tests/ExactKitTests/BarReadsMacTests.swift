// @ref LLP 1115.002 D2 — the window's bar is presented again only by a batch
// that touches what it read (ToolbarReadsMac.swift). The three cases that
// kept an exclusion by list ancestry from landing (a toolbar made or sitting
// in a row, a header button's popover living in a row, a native button's
// viewless title), a row fill that presents nothing, and a route change the
// bar never read, which the navigation's pass decides. `narrowed` is that
// exclusion, kept here to show what it skipped.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class BarReadsMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    private func view(_ id: Int, _ kind: String = "view", _ props: [String: String] = [:], press: Bool = false) -> [[String: Any]] {
        [["op": "create", "id": id, "kind": kind, "props": props, "handlers": press ? ["press"] : []],
         ["op": "frame", "id": id, "x": 0, "y": 0, "w": 60, "h": 30]]
    }
    private func children(_ id: Int, _ ids: [Int]) -> [String: Any] { ["op": "children", "id": id, "ids": ids] }
    private func symbolButton(_ id: Int, _ label: String, _ symbol: String, _ props: [String: String] = [:]) -> [[String: Any]] {
        view(id, "button", ["accessibilityLabel": label].merging(props) { _, new in new }, press: true)
            + view(id * 10, "image", ["symbolName": symbol]) + [children(id, [id * 10])]
    }
    private var saveTitle = "Save"

    /// A navigation root (1) showing route "a" (2), whose header (10) holds a
    /// Filter button (11), its heading (12), an Add button (13) and a native
    /// button (14, titled `saveTitle`); its page (15) holds a list (60) of one
    /// row (61). Route "b" (3) has its own header (20, heading 22). `header`
    /// and `row` are more children of the header and the row.
    private func fixture(header: [Int] = [], row: [Int] = [], extra: [[String: Any]] = []) throws -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 700, height: 400), styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.title = "Shelf"
        window.contentView = p.viewport
        p.buttonFace = { [unowned self] id in var face = ButtonFace(); if id == 14 { face.title = self.saveTitle }; return face }
        var ops: [[String: Any]] = view(1, "view", ["navigationBack": "back", "navigationKey": "a"])
        ops += view(2, "view", ["navigationKey": "a"]) + view(10, "view", ["semanticTag": "header"])
        ops += symbolButton(11, "Filter", "line.3.horizontal.decrease")
        ops += view(12, "text", ["text": "Library", "accessibilityRole": "heading", "accessibilityHeadingLevel": "1"])
        ops += symbolButton(13, "Add", "plus")
        ops += view(14, "control", ["type": "button", "accessibilityRole": "button"], press: true) + view(15)
        ops += view(60, "list") + view(61)
        ops += view(3, "view", ["navigationKey": "b"]) + view(20, "view", ["semanticTag": "header"])
        ops += view(22, "text", ["text": "Detail", "accessibilityRole": "heading", "accessibilityHeadingLevel": "1"]) + view(25)
        ops += extra
        ops += [children(61, row), children(60, [61]), children(15, [60]), children(10, [11, 12, 13, 14] + header), children(2, [10, 15]),
                children(20, [22]), children(3, [20, 25]), children(1, [2, 3]), ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        XCTAssertTrue(p.toolbar.attach(to: window))
        XCTAssertTrue(p.toolbar.projected, "the header is the bar")
        return p
    }

    /// The exclusion that did not land: today's op kinds, less ops on nodes
    /// the batch makes and ops inside a list's rows.
    private func narrowed(_ p: Presenter, _ batch: Batch) -> Bool {
        let made = Set(batch.ops.filter { $0.op == .create }.map(\.id))
        func inRow(_ id: UInt32) -> Bool {
            var ancestor = p.views[id]?.superview
            while let view = ancestor { if (view as? NodeView)?.kind == "list" { return true }; ancestor = view.superview }
            return false
        }
        return batch.ops.contains { p.toolbar.mayChangeAHeader(wireBatchOf($0)) && !made.contains($0.id) && !inRow($0.id) }
    }
    private func wireBatchOf(_ op: BatchOp) -> Batch { var b = wireBatch([]); b.ops = [op]; return b }
    private func ids() -> [String] { window.toolbar?.items.map(\.itemIdentifier.rawValue) ?? [] }

    /// (a) A declared toolbar made inside a row is the window's bar, and a
    /// change to its button, still in the row, reaches its item.
    func testADeclaredToolbarMadeInARowIsTheBarAndFollowsItsButton() throws {
        let p = try fixture()
        let made = wireBatch(view(70, "view", ["toolbarPlacement": "window", "accessibilityRole": "toolbar"])
            + view(71, "button", ["accessibilityLabel": "Pin"], press: true) + view(711, "text", ["text": "Pin"])
            + [children(71, [711]), children(70, [71]), children(61, [70])])
        XCTAssertFalse(narrowed(p, made), "the exclusion skipped it: every op is on a node it makes or in a row")
        XCTAssertTrue(p.toolbar.concerns(made), "a node made carrying toolbarPlacement, anywhere")
        p.apply(made)
        XCTAssertTrue(ids().contains("exact.command.71"), "\(ids())")
        let disabled = wireBatch([["op": "props", "id": 71, "set": ["disabled": "true"]]])
        XCTAssertFalse(narrowed(p, disabled), "the exclusion skipped it: in a row")
        XCTAssertTrue(p.toolbar.concerns(disabled))
        p.apply(disabled)
        XCTAssertEqual(p.toolbar.items[71]?.isEnabled, false)
        // A props op that adds the placement to a row's box that was there before.
        let q = try fixture(row: [72], extra: view(72, "view", ["accessibilityRole": "toolbar"]) + view(73, "button", ["accessibilityLabel": "Star"], press: true)
            + view(731, "text", ["text": "Star"]) + [children(73, [731]), children(72, [73])])
        let added = wireBatch([["op": "props", "id": 72, "set": ["toolbarPlacement": "window"]]])
        XCTAssertFalse(narrowed(q, added))
        XCTAssertTrue(q.toolbar.concerns(added))
        q.apply(added)
        XCTAssertTrue(ids().contains("exact.command.73"), "\(ids())")
    }

    /// (b) A header button opens a popover that lives in a row: the
    /// popover's `position_area` decides whether the bar can show the button,
    /// and so does which view its name finds.
    func testAPopoverLivingInARowDecidesItsHeaderButtonsPlace() throws {
        let rename = symbolButton(16, "Rename", "pencil", ["popovertarget": "rename"])
            + view(40, "view", ["popover": "auto", "id": "rename"]) + view(41, "input", ["type": "text"]) + [children(40, [41])]
        let p = try fixture(header: [16], row: [40], extra: rename)
        XCTAssertNotNil(p.toolbar.items[16], "AppKit anchors the form to its item")
        let placed = wireBatch([["op": "style", "id": 40, "style": ["position_area": "top", "margin_bottom": 12]]])
        XCTAssertFalse(narrowed(p, placed), "the exclusion skipped it: in a row")
        XCTAssertTrue(p.toolbar.concerns(placed))
        p.apply(placed)
        XCTAssertNil(p.toolbar.items[16], "authored placement needs the button's page box")
        XCTAssertFalse(try XCTUnwrap(p.views[10]).isHidden, "the header paints again")
        // The popover goes, and a box elsewhere in the row takes its name: the button opens that one now.
        let q = try fixture(header: [16], row: [40], extra: rename)
        XCTAssertNotNil(q.toolbar.items[16])
        let renamed = wireBatch(view(42, "view", ["popover": "auto", "id": "rename"]) + view(43, "input", ["type": "text"])
            + [["op": "style", "id": 42, "style": ["position_area": "top", "margin_bottom": 12]], children(42, [43]), ["op": "destroy", "id": 41], ["op": "destroy", "id": 40], children(61, [42])])
        XCTAssertFalse(narrowed(q, renamed))
        XCTAssertTrue(q.toolbar.concerns(renamed))
        q.apply(renamed)
        XCTAssertNil(q.toolbar.items[16], "the popover the name finds now needs the button's page box")
    }

    /// (c) A native header button's title is viewless: no op names it, only
    /// `Batch.controls`, which presents the bar while it shows a control.
    func testANativeHeaderButtonsNewTitleReachesItsItem() throws {
        let p = try fixture()
        XCTAssertEqual(p.toolbar.items[14]?.title, "Save")
        saveTitle = "Store"
        let faces = facesBatch()
        XCTAssertFalse(narrowed(p, faces), "no op to place")
        XCTAssertTrue(p.toolbar.concerns(faces))
        p.apply(faces)
        XCTAssertEqual(p.toolbar.items[14]?.title, "Store")
    }

    /// Rows filled into a list outside everything the bar read present
    /// nothing, nor does a word changed in a row; the heading's words do.
    func testARowFillOutsideTheBarsReadsDoesNotPresentIt() throws {
        let p = try fixture()
        let before = p.toolbar.presents
        var ops: [[String: Any]] = []
        for r in 0..<20 { ops += view(300 + r) + view(400 + r, "text", ["text": "row \(r)"]) + [children(300 + r, [400 + r])] }
        ops.append(children(60, [61] + (0..<20).map { 300 + $0 }))
        let fill = wireBatch(ops)
        XCTAssertFalse(p.toolbar.concerns(fill))
        p.apply(fill)
        p.apply(wireBatch([["op": "props", "id": 405, "set": ["text": "row five"]], ["op": "style", "id": 305, "style": ["opacity": 0.5]]]))
        XCTAssertEqual(p.toolbar.presents, before, "neither batch presents the bar")
        p.apply(wireBatch([["op": "props", "id": 12, "set": ["text": "Books"]]]))
        XCTAssertEqual(p.toolbar.presents, before + 1)
        XCTAssertEqual(window.title, "Books")
    }

    /// The route the bar shows is the navigation's to decide, from routes the
    /// bar never read: with the root on key "b", route "a" (never read)
    /// taking key "b" is the first route of that key, and the selected one,
    /// with no op on anything the bar read.
    func testARouteChangeTheBarNeverReadPresentsIt() throws {
        let p = try fixture()
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["navigationKey": "b"]]]))
        XCTAssertEqual(window.title, "Detail")
        let rekey = wireBatch([["op": "props", "id": 2, "set": ["navigationKey": "b"]]])
        XCTAssertFalse(p.toolbar.concerns(rekey), "route a was not read while b showed")
        let before = p.toolbar.presents
        p.apply(rekey)
        XCTAssertEqual(p.toolbar.presents, before + 1, "the navigation's pass moved the route: the bar follows")
        XCTAssertEqual(window.title, "Library")
    }
    // Round 1's findings (Astra, Grok).

    /// A header the rule refused is still read: its heading losing its role
    /// and then taking it back presents both times.
    func testAHeaderTheRuleRefusedIsStillReadAndTakenBackWhenItCanBe() throws {
        let p = try fixture()
        p.apply(wireBatch([["op": "props", "id": 12, "set": [:], "clear": ["accessibilityRole"]]]))
        XCTAssertFalse(p.toolbar.projected, "no heading: no header the bar can show")
        let restore = wireBatch([["op": "props", "id": 12, "set": ["accessibilityRole": "heading"]]])
        XCTAssertTrue(p.toolbar.concerns(restore), "the refused header's nodes are in the record")
        p.apply(restore)
        XCTAssertTrue(p.toolbar.projected)
        XCTAssertEqual(window.title, "Library")
    }

    /// A button's `popovertarget` names a view that is no popover yet: that
    /// view is read, so its taking `popover` (with an authored placement)
    /// presents, and the header paints again.
    func testANamedViewThatBecomesAPopoverIsRead() throws {
        let p = try fixture(header: [16], row: [40], extra: symbolButton(16, "Rename", "pencil", ["popovertarget": "rename"])
            + view(40, "view", ["id": "rename"]) + view(41, "input", ["type": "text"]) + [children(40, [41]),
            ["op": "style", "id": 40, "style": ["position_area": "top", "margin_bottom": 12]]])
        XCTAssertNotNil(p.toolbar.items[16], "it opens nothing yet: a command")
        let opened = wireBatch([["op": "props", "id": 40, "set": ["popover": "auto"]]])
        XCTAssertTrue(p.toolbar.concerns(opened))
        p.apply(opened)
        XCTAssertNil(p.toolbar.items[16], "now it opens a popover placed by its page box")
    }

    /// A readout labelled by a node in a row: that node's words are its item's
    /// name, so a change to them presents; and a name that resolved to nothing
    /// presents when a node takes it.
    func testAnExternalLabelsWordsAreRead() throws {
        let p = try fixture(header: [16], row: [90], extra: view(16, "text", ["text": "3", "accessibilityRole": "status", "accessibilityLabelledBy": "unread"])
            + view(90, "text", ["text": "Unread", "id": "unread"]))
        XCTAssertEqual(p.toolbar.items[16]?.label, "Unread")
        let renamed = wireBatch([["op": "props", "id": 90, "set": ["text": "New"]]])
        XCTAssertTrue(p.toolbar.concerns(renamed))
        p.apply(renamed)
        XCTAssertEqual(p.toolbar.items[16]?.label, "New")
        let q = try fixture(header: [16], extra: view(16, "text", ["text": "3", "accessibilityRole": "status", "accessibilityLabelledBy": "later"]))
        let named = wireBatch(view(91, "text", ["text": "Later", "id": "later"]) + [children(61, [91])])
        XCTAssertTrue(q.toolbar.concerns(named))
        q.apply(named)
        XCTAssertEqual(q.toolbar.items[16]?.label, "Later")
    }

    /// A readout's painted face carries its opacity: a presented opacity on it presents.
    func testAPresentedOpacityOnAReadoutPresents() throws {
        let p = try fixture(header: [16], extra: view(16, "text", ["text": "Syncing", "accessibilityRole": "status"]))
        let before = try XCTUnwrap((p.toolbar.items[16]?.view as? NSImageView)?.image?.tiffRepresentation)
        let faded = wireBatch([["op": "present", "id": 16, "property": "opacity", "x": 0.4, "y": 0, "w": 0, "h": 0]])
        XCTAssertTrue(p.toolbar.concerns(faded))
        p.apply(faded)
        XCTAssertNotEqual((p.toolbar.items[16]?.view as? NSImageView)?.image?.tiffRepresentation, before)
    }

    /// The system's colours changed (an accent the user chose): the bar,
    /// whose prominent items take their tint, presents again.
    func testSystemColoursPresentTheBar() throws {
        let p = try fixture()
        let before = p.toolbar.presents
        p.systemColorsChanged()
        XCTAssertEqual(p.toolbar.presents, before + 1)
    }

    /// The guard's present leaves the record the release path decides from
    /// as it was: a mark put in it survives a batch the record skipped.
    func testTheGuardLeavesTheRecordAsItWas() throws {
        let p = try fixture()
        p.toolbar.reads.names.insert("mark")
        let inputs = p.toolbar.inputs
        let skipped = wireBatch([["op": "style", "id": 61, "style": ["opacity": 0.9]]])
        XCTAssertFalse(p.toolbar.concerns(skipped))
        p.apply(skipped)
        XCTAssertTrue(p.toolbar.reads.names.contains("mark"), "the guard's own present recorded over it")
        XCTAssertEqual(p.toolbar.inputs, inputs)
    }
    // Round 2's findings (Astra, Grok). The guard is off in these: it would present the bar itself.
    private func unguarded(_ body: () throws -> Void) rethrows {
        let was = ShadowCheck.on
        ShadowCheck.on = false
        defer { ShadowCheck.on = was }
        try body()
    }

    /// A run in a paragraph taking the name a header button's `commandfor`
    /// gives: the invocation finds the run (no view), so the button opens no
    /// popover through its `popovertarget` any more; the run losing the name
    /// gives the menu back.
    func testARunTakingAButtonsNameIsRead() throws {
        try unguarded {
            let menu = symbolButton(16, "More", "ellipsis.circle", ["commandfor": "guard", "command": "show-popover", "popovertarget": "more"])
                + view(40, "view", ["popover": "auto", "id": "more", "accessibilityRole": "menu"])
                + view(41, "button", ["popovertarget": "more", "popovertargetaction": "hide"], press: true) + view(411, "text", ["text": "Rename"])
                + [children(41, [411]), children(40, [41])] + view(92, "text")
            let p = try fixture(header: [16], row: [40, 92], extra: menu)
            XCTAssertTrue(p.toolbar.items[16] is NSMenuToolbarItem, "the popovertarget's menu")
            func run(_ id: Int, _ props: [String: String]) -> [String: Any] { ["id": id, "parent": 92, "paint": true, "props": props, "style": ["font_size": 13.0]] }
            let named = wireBatch([["op": "paragraph", "id": 92, "runs": [run(93, ["text": "on guard", "id": "guard"])]]])
            XCTAssertTrue(p.toolbar.concerns(named))
            p.apply(named)
            XCTAssertNotNil(p.toolbar.items[16])
            XCTAssertFalse(p.toolbar.items[16] is NSMenuToolbarItem, "commandfor names a run: it opens nothing")
            let unnamed = wireBatch([["op": "paragraph", "id": 92, "runs": [run(93, ["text": "off guard"])]]])
            XCTAssertTrue(p.toolbar.concerns(unnamed))
            p.apply(unnamed)
            XCTAssertTrue(p.toolbar.items[16] is NSMenuToolbarItem)
        }
    }

    /// A second navigation root, in the page, shows a route holding a
    /// declared window toolbar; a sibling route it never showed taking that
    /// key hides it, with no op on anything the bar read: the header is the
    /// bar again.
    func testASecondNavigationRootHidingADeclaredToolbarPresents() throws {
        try unguarded {
            let nested = view(80, "view", ["navigationBack": "", "navigationKey": "y"]) + view(81, "view", ["navigationKey": "x"]) + view(82, "view", ["navigationKey": "y"])
                + view(83, "view", ["toolbarPlacement": "window", "accessibilityRole": "toolbar"])
                + view(84, "button", ["accessibilityLabel": "Pin"], press: true) + view(841, "text", ["text": "Pin"])
                + [children(84, [841]), children(83, [84]), children(82, [83]), children(80, [81, 82])]
            let bar = try fixture(row: [80], extra: nested)
            XCTAssertTrue(ids().contains("exact.command.84"), "the declaration is the bar: \(ids())")
            let rekey = wireBatch([["op": "props", "id": 81, "set": ["navigationKey": "y"]]])
            XCTAssertFalse(bar.toolbar.concerns(rekey), "route x was never read")
            bar.apply(rekey)
            XCTAssertFalse(ids().contains("exact.command.84"), "\(ids())")
            XCTAssertEqual(window.title, "Library", "the header is the bar again")
        }
    }

    /// The window turns dark: an authored `light-dark()` ink on a header
    /// button is resolved again, though no op says so.
    func testTheWindowTurningDarkResolvesTheBarsInkAgain() throws {
        try unguarded {
            let p = try fixture(extra: [["op": "style", "id": 13, "style": ["text_color": [[255, 0, 0, 255], [0, 0, 255, 255]], "written": ["text_color"]]]])
            window.appearance = NSAppearance(named: .aqua)
            p.toolbar.sync()
            let red = try XCTUnwrap((p.toolbar.items[13]?.view as? NSButton)?.contentTintColor?.usingColorSpace(.sRGB))
            XCTAssertGreaterThan(red.redComponent, 0.9)
            window.appearance = NSAppearance(named: .darkAqua)
            p.apply(wireBatch([]))
            let blue = try XCTUnwrap((p.toolbar.items[13]?.view as? NSButton)?.contentTintColor?.usingColorSpace(.sRGB))
            XCTAssertGreaterThan(blue.blueComponent, 0.9, "the dark half")
            XCTAssertLessThan(blue.redComponent, 0.1)
        }
    }

    /// An op that names no node presents: a theme's roles (the accent a
    /// prominent item bakes), the router's place (what Back goes to), and the
    /// document's language, after which the bar presents once more.
    func testOpsThatNameNoNodePresent() throws {
        try unguarded {
            let p = try fixture()
            for op: [String: Any] in [["op": "roles", "roles": ["AccentColor": [255, 0, 0, 255]]], ["op": "router", "location": "/books"]] {
                let batch = wireBatch([op])
                XCTAssertTrue(p.toolbar.concerns(batch), "\(op)")
                let before = p.toolbar.presents
                p.apply(batch)
                XCTAssertGreaterThan(p.toolbar.presents, before, "\(op)")
            }
            let before = p.toolbar.presents
            p.apply(wireBatch([["op": "language", "lang": "fr", "dir": "ltr"]]))
            XCTAssertEqual(p.documentLanguage, "fr")
            XCTAssertEqual(p.toolbar.presents, before + 2, "before and after the language moved")
            // Kinds no part of the bar reads stay skipped.
            XCTAssertFalse(p.toolbar.concerns(wireBatch([["op": "sound", "voices": []], ["op": "collections", "windows": []]])))
        }
    }
    // Round 3's findings (Astra): what the host hides and makes inert, with no op.

    /// A header button opens an `alertdialog` popover that lives under a
    /// second navigation root's route, placed by its page box: AppKit can
    /// show it as an alert, so the header is the bar. That root showing
    /// another route hides and inerts the alert's, which is no alert now, and
    /// its authored placement needs the button's page box: the header paints.
    func testASecondRootHidingAnAlertTheBarOpensPresents() throws {
        try unguarded {
            let closing = ["popovertarget": "ask", "popovertargetaction": "hide"]
            let nested = symbolButton(16, "Remove", "trash", ["popovertarget": "ask"])
                + view(80, "view", ["navigationBack": "", "navigationKey": "y"]) + view(81, "view", ["navigationKey": "x"]) + view(82, "view", ["navigationKey": "y"])
                + view(85, "view", ["popover": "auto", "id": "ask", "accessibilityRole": "alertdialog", "accessibilityLabel": "Remove Book?"])
                + [["op": "style", "id": 85, "style": ["position_area": "top", "margin_bottom": 12]]]
                + view(86, "text", ["text": "It leaves your library."])
                + view(87, "button", closing, press: true) + view(871, "text", ["text": "Remove"])
                + view(88, "button", closing) + view(881, "text", ["text": "Not Now"])
                + [children(87, [871]), children(88, [881]), children(85, [86, 87, 88]), children(82, [85]), children(80, [81, 82])]
            let p = try fixture(header: [16], row: [80], extra: nested)
            XCTAssertNotNil(p.toolbar.items[16], "an alert AppKit shows from the item")
            let rekey = wireBatch([["op": "props", "id": 81, "set": ["navigationKey": "y"]]])
            XCTAssertFalse(p.toolbar.concerns(rekey), "route x was never read")
            p.apply(rekey)
            XCTAssertNil(p.toolbar.items[16], "the alert's route is hidden: its placement needs the page box")
            XCTAssertFalse(try XCTUnwrap(p.views[10]).isHidden, "the header paints")
        }
    }

    /// A route that is itself the declared toolbar, under a second root:
    /// the bar hides it while it shows it, and the navigation's pass shows
    /// the selected route again on any batch. The bar hides it again.
    func testTheNavigationShowingAProjectedToolbarAgainPresents() throws {
        try unguarded {
            let nested = view(80, "view", ["navigationBack": "", "navigationKey": "y"]) + view(81, "view", ["navigationKey": "x"])
                + view(82, "view", ["navigationKey": "y", "toolbarPlacement": "window", "accessibilityRole": "toolbar"])
                + view(84, "button", ["accessibilityLabel": "Pin"], press: true) + view(841, "text", ["text": "Pin"]) + view(95, "text", ["text": "body"])
                + [children(84, [841]), children(82, [84]), children(80, [81, 82])]
            let p = try fixture(row: [80, 95], extra: nested)
            XCTAssertTrue(ids().contains("exact.command.84"), "\(ids())")
            XCTAssertTrue(try XCTUnwrap(p.views[82]).isHidden, "the bar shows it, so the page does not")
            let unrelated = wireBatch([["op": "props", "id": 95, "set": ["text": "body text"]]])
            XCTAssertFalse(p.toolbar.concerns(unrelated))
            p.apply(unrelated)
            XCTAssertTrue(try XCTUnwrap(p.views[82]).isHidden, "still the bar's, not painted twice")
        }
    }
}
#endif
