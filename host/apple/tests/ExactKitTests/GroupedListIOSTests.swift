#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// LLP 1084 on UIKit: a grouped list (`listStyle`) is a UICollectionView
/// with a list layout in the list's box over its hidden authored scroll;
/// its rows are list cells configured from the kernel's model (D5); a tap
/// selects, highlights and presses; a toggle flips its control; a custom
/// row's views are carried into their cell and given back before a batch.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class GroupedListIOSTests: XCTestCase {
    private var window: UIWindow!

    private typealias Row = GroupedListModel.Row
    private func model(_ style: String = "inset-grouped", custom: Bool = false, dark: Bool = false) -> GroupedListModel {
        GroupedListModel(style: style, sections: [
            .init(view: 2, header: "Account", footer: "Who can see you.", rows: [
                Row(view: 10, symbol: "person.circle", title: "Profile", accessory: "disclosure", pressable: true),
                Row(view: 11, title: "Notifications", secondary: "On", accessory: "disclosure", pressable: true),
                Row(view: 12, title: "Read Receipts", accessory: "toggle", target: 13),
            ]),
            .init(view: 3, header: nil, footer: nil, rows: [
                Row(view: 20, title: "Dark", accessory: dark ? "checkmark" : "none", pressable: true),
                custom ? Row(view: 21, custom: true, pressable: true) : Row(view: 21, title: "Delete", pressable: true, destructive: true),
            ]),
        ])
    }

    private func presenter(_ m: @escaping () -> GroupedListModel, on given: Presenter? = nil, height: Double = 874) -> Presenter {
        ExactGroupedLists.install()
        let p = given ?? Presenter()
        host(p).model = { $0 == 1 ? m() : nil }
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        let view = { (id: Int, kind: String, props: [String: String]) -> [String: Any] in
            ["op": "create", "id": id, "kind": kind, "props": props, "handlers": kind == "button" ? ["press"] : [],
             "style": ["text_color": [0, 0, 0, 255], "border_width_bottom": 1.0]]
        }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "list", "props": ["listStyle": "inset-grouped", "testId": "list"], "style": ["overflow_y": "scroll"]],
            view(2, "view", ["semanticTag": "section"]), view(3, "view", ["semanticTag": "section"]),
            view(4, "view", [:]), view(5, "view", [:]),
            ["op": "create", "id": 13, "kind": "control", "props": ["type": "checkbox", "accessibilityRole": "switch", "checked": "true", "testId": "toggle"], "handlers": ["input"], "style": [:]],
        ]
        for (id, kind) in [(10, "button"), (11, "button"), (12, "view"), (20, "button"), (21, "button")] {
            ops.append(view(id, kind, ["testId": "row\(id)"]))
        }
        ops += [["op": "children", "id": 1, "ids": [2, 3]], ["op": "children", "id": 2, "ids": [4]], ["op": "children", "id": 3, "ids": [5]],
                ["op": "children", "id": 4, "ids": [10, 11, 12]], ["op": "children", "id": 5, "ids": [20, 21]],
                ["op": "children", "id": 12, "ids": [13]], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": height],
                ["op": "frame", "id": 21, "x": 16.0, "y": 52.0, "w": 354.0, "h": 61.0]]
        p.apply(wireBatch(ops))
        return p
    }

    private func host(_ p: Presenter) -> GroupedListHost { p.groupedLists as! GroupedListHost }
    private func list(_ p: Presenter) throws -> GroupedListView { try XCTUnwrap(host(p).lists[1]) }
    private func cell(_ p: Presenter, _ id: UInt32) throws -> UICollectionViewListCell {
        let l = try list(p)
        l.collection.layoutIfNeeded()
        return try XCTUnwrap(l.cell(id) as? UICollectionViewListCell, "row \(id) has a cell")
    }

    func testItIsAUICollectionViewListInTheListsBoxOverItsHiddenScroll() throws {
        let p = presenter { self.model() }
        let l = try list(p)
        XCTAssertTrue(l.collection.superview === p.views[1])
        XCTAssertEqual(l.collection.frame, p.views[1]?.bounds)
        XCTAssertEqual(p.views[1]?.scroll?.isHidden, true, "the authored rows are beneath, hidden")
        XCTAssertEqual(l.collection.numberOfSections, 2)
        XCTAssertEqual(l.collection.numberOfItems(inSection: 0), 3)
        XCTAssertTrue(l.collection.collectionViewLayout is UICollectionViewCompositionalLayout)
        let seen = try XCTUnwrap(host(p).observation(try XCTUnwrap(p.views[1])))
        XCTAssertEqual(seen["view"] as? String, "UICollectionView")
        XCTAssertEqual(seen["rows"] as? Int, 5)
    }

    /// LLP 1115.000 D9.1: the collection view is what a finger drags, so it
    /// puts the keyboard away as the list's own scroll would: following the
    /// drag with nothing written, and as the author says otherwise.
    func testADragOnTheListTakesTheKeyboardAsItsScrollWould() throws {
        let p = presenter { self.model() }
        let l = try list(p)
        XCTAssertEqual(l.collection.keyboardDismissMode, .interactive, "unsaid: the keyboard follows the drag")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["keyboardDismissMode": "on-drag"], "clear": []]]))
        XCTAssertEqual(l.collection.keyboardDismissMode, .onDrag, "an authored mode reaches the list that shows")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["keyboardDismissMode": "none"], "clear": []]]))
        XCTAssertEqual(l.collection.keyboardDismissMode, UIScrollView.KeyboardDismissMode.none)
    }

    /// LLP 1115.000 D11.3: pull-to-refresh is on the list that shows. The
    /// control was on the list's own scroll, which a grouped list hides
    /// under its collection view, so nothing could be pulled.
    func testPullToRefreshIsOnTheListThatShows() throws {
        let p = presenter { self.model() }
        let l = try list(p), owner = try XCTUnwrap(p.views[1]), hidden = try XCTUnwrap(owner.scroll)
        var refreshed: [UInt32] = []
        p.onRefresh = { refreshed.append($0) }
        owner.handlers = ["refresh"]
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["refreshing": "false"], "clear": []]]))
        let control = try XCTUnwrap(l.collection.refreshControl, "the collection view, which a finger pulls, carries the control")
        XCTAssertNil(hidden.refreshControl, "the hidden scroll beneath carries none")
        XCTAssertEqual(control.actions(forTarget: owner, forControlEvent: .valueChanged), ["pulledToRefresh"], "a pull is the list's `refresh`")
        owner.pulledToRefresh()
        XCTAssertEqual(refreshed, [1])
        // The app's `refreshing` going false ends the spinner, as on a scroll.
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["refreshing": "true"], "clear": []]]))
        control.beginRefreshing()
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["refreshing": "false"], "clear": []]]))
        XCTAssertFalse(control.isRefreshing)
        // No handler, no control.
        owner.handlers = []
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["refreshing": "false"], "clear": []]]))
        XCTAssertNil(l.collection.refreshControl)
    }

    func testARowIsAListCellOfTheModelsParts() throws {
        let p = presenter { self.model() }
        let profile = try cell(p, 10)
        let c = try XCTUnwrap(profile.contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(c.text, "Profile")
        XCTAssertNotNil(c.image, "the symbol")
        XCTAssertEqual(profile.accessories.count, 1, "the disclosure indicator")
        XCTAssertEqual(profile.accessibilityIdentifier, "row10")
        let value = try XCTUnwrap(try cell(p, 11).contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(value.secondaryText, "On")
        let delete = try XCTUnwrap(try cell(p, 21).contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(delete.textProperties.color, .systemRed, "destructive")
        // UIKit draws headers and footers from the section's texts.
        let l = try list(p)
        XCTAssertNotNil(l.collection.supplementaryView(forElementKind: UICollectionView.elementKindSectionHeader, at: IndexPath(item: 0, section: 0)))
    }

    /// A row's margins are the list's, not the safe area's: the last row of
    /// a settings list flung under the home indicator grew by the inset and
    /// shrank back a pixel a layout pass until UIKit's feedback-loop check
    /// stopped the app (the Bluesky clone, 2026-10-07).
    func testARowsMarginsDoNotTakeTheSafeArea() throws {
        var custom = false
        let p = presenter { self.model(custom: custom) }
        for id: UInt32 in [10, 21] {
            let c = try cell(p, id)
            XCTAssertFalse(c.insetsLayoutMarginsFromSafeArea, "row \(id)")
            XCTAssertFalse(c.contentView.insetsLayoutMarginsFromSafeArea, "row \(id)'s content")
        }
        let l = try list(p)
        let header = try XCTUnwrap(l.collection.supplementaryView(forElementKind: UICollectionView.elementKindSectionHeader, at: IndexPath(item: 0, section: 0)) as? UICollectionViewListCell)
        XCTAssertFalse(header.contentView.insetsLayoutMarginsFromSafeArea, "a header's content")
        let footer = try XCTUnwrap(l.collection.supplementaryView(forElementKind: UICollectionView.elementKindSectionFooter, at: IndexPath(item: 0, section: 0)) as? UICollectionViewListCell)
        XCTAssertFalse(footer.contentView.insetsLayoutMarginsFromSafeArea, "a footer's content")
        custom = true
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "row21"], "clear": [String]()]]))
        XCTAssertFalse(try cell(p, 21).contentView.insetsLayoutMarginsFromSafeArea, "a custom row's content")
    }

    func testATapHighlightsAndPressesTheRowOnce() throws {
        let p = presenter { self.model() }
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let l = try list(p)
        _ = try cell(p, 10)
        XCTAssertTrue(l.collectionView(l.collection, shouldHighlightItemAt: IndexPath(item: 0, section: 0)))
        XCTAssertFalse(l.collectionView(l.collection, shouldHighlightItemAt: IndexPath(item: 2, section: 0)), "a `row` is not a button")
        XCTAssertEqual(host(p).activate(try XCTUnwrap(p.views[10]))?["native"] as? String, "grouped-list")
        XCTAssertEqual(pressed, [10])
        XCTAssertNotNil(host(p).activate(try XCTUnwrap(p.views[12]))?["error"], "nothing to press")
        XCTAssertEqual(pressed, [10])
        XCTAssertTrue(host(p).draws(10) && host(p).draws(13), "a row and its toggle's control")
        // Scrolled away, a row is refused as a finger would miss it.
        l.collection.contentInset.bottom = 2000
        l.collection.setContentOffset(CGPoint(x: 0, y: 1500), animated: false)
        l.collection.layoutIfNeeded()
        XCTAssertNotNil(host(p).activate(try XCTUnwrap(p.views[10]))?["error"])
        XCTAssertEqual(pressed, [10])
    }

    /// LLP 1115.000 D13.2: in a grouped row a radio is the row's checkmark
    /// accessory, as a list of choices in Settings has it, and a tap on the
    /// row checks the radio: `input` and `change` carry its value, and the
    /// checkmark follows the bound `checked`. It had been a custom row
    /// holding Safari's 16-point circle.
    func testARadiosRowShowsTheCheckmarkAndATapOnTheRowChecksIt() throws {
        ExactGroupedLists.install()
        let p = Presenter()
        var plan = "free"
        host(p).model = { $0 == 1 ? GroupedListModel(sections: [.init(view: 2, header: nil, footer: nil, rows: [
            Row(view: 30, title: "Free", accessory: "radio", target: 31, checked: plan == "free"),
            Row(view: 32, title: "Pro", accessory: "radio", target: 33, checked: plan == "pro"),
            Row(view: 34, title: "Team", accessory: "radio", target: 35, checked: plan == "team"),
        ])]) : nil }
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        var heard: [String] = []
        p.onControlValue = { id, value, input, change in
            if input { heard.append("input \(id) \(value)") }
            if change { heard.append("change \(id) \(value)") }
        }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "list", "props": ["listStyle": "inset-grouped"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["semanticTag": "section"], "style": [:]],
            ["op": "create", "id": 4, "kind": "view", "props": [:], "style": [:]],
        ]
        for (row, radio, value) in [(30, 31, "free"), (32, 33, "pro"), (34, 35, "team")] {
            var props = ["type": "radio", "name": "plan", "value": value, "checked": value == plan ? "true" : "false", "testId": "plan-\(value)"]
            if value == "team" { props["disabled"] = "true" }
            ops += [["op": "create", "id": row, "kind": "view", "props": ["testId": "row-\(value)"], "style": [:]],
                    ["op": "create", "id": radio, "kind": "control", "props": props, "handlers": ["input", "change"], "style": [:]],
                    ["op": "children", "id": row, "ids": [radio]]]
        }
        ops += [["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [4]], ["op": "children", "id": 4, "ids": [30, 32, 34]],
                ["op": "roots", "ids": [1]], ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0]]
        p.apply(wireBatch(ops))
        let l = try list(p)
        XCTAssertEqual(try cell(p, 30).accessories.map(\.accessoryType), [.checkmark], "the chosen row: the platform's checkmark")
        XCTAssertEqual(try cell(p, 32).accessories.count, 0, "the others: nothing")
        XCTAssertEqual((try cell(p, 30).contentConfiguration as? UIListContentConfiguration)?.text, "Free", "a standard cell, not the row's own views")
        XCTAssertTrue(l.collectionView(l.collection, shouldHighlightItemAt: IndexPath(item: 1, section: 0)), "the row takes the tap, as a choice's row does")
        XCTAssertFalse(l.collectionView(l.collection, shouldHighlightItemAt: IndexPath(item: 2, section: 0)), "a disabled radio's row does not")
        // A tap on the row, and one that names the radio, are the same tap.
        XCTAssertEqual(host(p).activate(try XCTUnwrap(p.views[32]))?["native"] as? String, "grouped-list")
        XCTAssertEqual(heard, ["input 33 pro", "change 33 pro"], "the radio's own events, with its value")
        XCTAssertNotNil(host(p).activate(try XCTUnwrap(p.views[34]))?["error"], "disabled")
        // The action commits it: the checkmark follows the bound `checked`.
        plan = "pro"
        p.apply(wireBatch([["op": "props", "id": 31, "set": ["checked": "false"]], ["op": "props", "id": 33, "set": ["checked": "true"]]]))
        XCTAssertEqual(try cell(p, 32).accessories.map(\.accessoryType), [.checkmark])
        XCTAssertEqual(try cell(p, 30).accessories.count, 0)
        heard = []
        XCTAssertEqual(host(p).activate(try XCTUnwrap(p.views[33]))?["native"] as? String, "grouped-list", "the radio by its own testId")
        XCTAssertEqual(heard, [], "already checked: nothing fires")
        XCTAssertEqual(host(p).activate(try XCTUnwrap(p.views[31]))?["native"] as? String, "grouped-list")
        XCTAssertEqual(heard, ["input 31 free", "change 31 free"])
    }

    func testAToggleFlipsItsControl() throws {
        let p = presenter { self.model() }
        var flips: [(UInt32, Bool)] = []
        p.onChecked = { flips.append(($0, $1)) }
        let toggle = try cell(p, 12)
        XCTAssertEqual(toggle.accessories.count, 1)
        let s = try XCTUnwrap(self.switches(toggle).first)
        XCTAssertTrue(s.isOn, "the control's `checked`")
        XCTAssertEqual(s.accessibilityIdentifier, "toggle")
        // A finger at its middle reaches the switch the cell shows, though
        // the hidden row's own switch is laid out under it.
        let middle = s.convert(CGPoint(x: s.bounds.midX, y: s.bounds.midY), to: window)
        let scroll = try XCTUnwrap(p.views[1]?.scroll)
        let at = scroll.convert(middle, from: window)
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0],
                           ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0],
                           ["op": "frame", "id": 12, "x": 0.0, "y": Double(at.y - 26), "w": 402.0, "h": 52.0],
                           ["op": "frame", "id": 13, "x": Double(at.x - 31.5), "y": 0.0, "w": 63.0, "h": 52.0]]))
        let authored = try XCTUnwrap(p.controls.controls[13] as? UISwitch)
        XCTAssertTrue(authored !== s && scroll.isHidden)
        XCTAssertTrue(authored.convert(authored.bounds, to: window).contains(middle), "the hidden row's switch is under the accessory")
        let hit = try XCTUnwrap(window.hitTest(middle, with: nil))
        XCTAssertTrue(hit === s || hit.isDescendant(of: s), "hit \(hit)")
        s.setOn(false, animated: false)
        s.sendActions(for: .valueChanged)
        XCTAssertEqual(flips.map(\.0), [13])
        XCTAssertEqual(flips.map(\.1), [false])
        XCTAssertTrue(s.isOn, "the committed state is authoritative: nothing committed it")
        // The agent's tap on the control flips the switch the cell shows.
        XCTAssertEqual(host(p).activate(try XCTUnwrap(p.views[13]))?["native"] as? String, "grouped-list")
        XCTAssertEqual(flips.map(\.1), [false, false])
    }

    func testToggleAccessoryCarriesARIAUnavailableAndStaysOperable() throws {
        let p = presenter { self.model() }
        let accessory = try XCTUnwrap(switches(try cell(p, 12)).first)
        let authored = try XCTUnwrap(p.controls.controls[13] as? UISwitch)
        XCTAssertTrue(accessory !== authored)
        XCTAssertFalse(accessory.accessibilityTraits.contains(.notEnabled))

        p.apply(wireBatch([["op": "props", "id": 13, "set": ["accessibilityDisabled": "true"]]]))
        XCTAssertTrue(authored.accessibilityTraits.contains(.notEnabled))
        XCTAssertTrue(accessory.accessibilityTraits.contains(.notEnabled), "VoiceOver reaches the accessory")
        XCTAssertTrue(accessory.isEnabled)
        var flips: [Bool] = []
        p.onChecked = { _, checked in flips.append(checked) }
        accessory.setOn(false, animated: false)
        accessory.sendActions(for: .valueChanged)
        XCTAssertEqual(flips, [false], "ARIA state preserves the exposed switch's input")
        XCTAssertTrue(accessory.accessibilityTraits.contains(.notEnabled), "refresh after input keeps the ARIA state")

        p.apply(wireBatch([["op": "props", "id": 13, "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertFalse(accessory.accessibilityTraits.contains(.notEnabled))
        p.apply(wireBatch([["op": "props", "id": 12, "set": ["accessibilityDisabled": "true"]],
                           ["op": "props", "id": 13, "clear": ["accessibilityDisabled"]]]))
        XCTAssertTrue(accessory.accessibilityTraits.contains(.notEnabled), "the authored row's state is inherited")
        p.apply(wireBatch([["op": "props", "id": 12, "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertFalse(accessory.accessibilityTraits.contains(.notEnabled), "an ancestor change refreshes the accessory")
    }

    /// LLP 1080.000 D4: a real finger aimed at what a list draws lands on
    /// UIKit's view, not the hidden authored node: a row's cell, its
    /// toggle's switch, its detail button; never the row for a control
    /// that is not shown.
    func testARealTouchAimsAtTheCellOrAccessoryUIKitDraws() throws {
        var detail = false
        let p = presenter {
            var m = self.model()
            if detail { m.sections[0].rows[2].accessory = "detail" }
            return m
        }
        let l = try list(p)
        func aimed(_ id: UInt32) throws -> UIView? {
            guard case .view(let view, let port)? = host(p).shown(try XCTUnwrap(p.views[id])) else { return nil }
            XCTAssertTrue(port === l.collection)
            return view
        }
        func refusal(_ id: UInt32) throws -> String? {
            guard case .refused(let why)? = host(p).shown(try XCTUnwrap(p.views[id])) else { return nil }
            return why
        }
        let toggle = try cell(p, 12)
        let s = try XCTUnwrap(switches(toggle).first)
        XCTAssertTrue(try aimed(13) === s, "the toggle's control: its switch")
        XCTAssertTrue(try aimed(10) === (try cell(p, 10)), "a row: its cell")
        XCTAssertNil(host(p).shown(try XCTUnwrap(p.views[4])), "a node no list draws: the ordinary aim")
        // The dispatch log tells one row's switch from another's, and from
        // its cell: the node alone is the list's for all of them.
        XCTAssertEqual(GroupedListHost.part(s.subviews.first ?? s) as? [String: AnyHashable], ["row": 12, "part": "switch"])
        XCTAssertEqual(GroupedListHost.part(toggle.contentView) as? [String: AnyHashable], ["row": 12, "part": "cell"])
        XCTAssertNil(GroupedListHost.part(l.collection))
        // A switch not shown is refused, never the row in its place.
        s.removeFromSuperview()
        XCTAssertEqual(try refusal(13), "its switch is not shown")
        // A detail button's control: UIKit's accessory, a control in the
        // cell outside its content.
        detail = true
        p.apply(wireBatch([["op": "props", "id": 12, "set": ["testId": "row12"], "clear": [String]()]]))
        let info = try XCTUnwrap(try aimed(13) as? UIControl)
        let row = try cell(p, 12)
        XCTAssertTrue(info.isDescendant(of: row) && !info.isDescendant(of: row.contentView))
        // Scrolled off the list's port, a row has no cell to aim at.
        l.collection.contentInset.bottom = 2000
        l.collection.setContentOffset(CGPoint(x: 0, y: 1500), animated: false)
        l.collection.layoutIfNeeded()
        XCTAssertEqual(try refusal(10), "its cell is outside the list's port; scroll it into view first")
    }

    /// A tap that names the list never selects the row its middle holds
    /// (LLP 1012 §1; Astra, round 1): every cell is in the list's node, so
    /// the row comes from the cell's projection, and the tap is refused,
    /// naming it, as a real touch's aim is.
    func testATapNamingTheListNeverSelectsTheRowAtItsMiddle() throws {
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "grouped-addressed")
        defer { session.destroy() }
        let p = presenter({ self.model() }, on: session.presenter, height: 200)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        _ = try cell(p, 10)
        let reply = Agent(session: session).tap(["id": 1])
        let error = try XCTUnwrap(reply["error"] as? String, "refused: \(reply)")
        XCTAssertTrue(error.contains("would press row #"), error)
        XCTAssertTrue([10, 11, 12].contains(reply["pressing"] as? Int ?? 0), "\(reply)")
        XCTAssertEqual(pressed, [], "no row was pressed")
    }

    /// A custom row's own views are carried into its cell and take the
    /// ordinary path: a tap that names the row presses it (round 2).
    func testATapNamingACustomRowPressesIt() throws {
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "grouped-custom")
        defer { session.destroy() }
        let p = presenter({ self.model(custom: true) }, on: session.presenter)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        _ = try cell(p, 21)
        let reply = Agent(session: session).tap(["id": 21])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(pressed, [21], "the custom row: \(reply)")
    }

    func testASwitchFollowsItsControlsStateAndTarget() throws {
        var target: UInt32 = 13
        let p = presenter {
            var m = self.model()
            m.sections[0].rows[2].target = target
            return m
        }
        var flips: [UInt32] = []
        p.onChecked = { id, _ in flips.append(id) }
        let s = try XCTUnwrap(switches(try cell(p, 12)).first)
        p.apply(wireBatch([["op": "props", "id": 13, "set": ["disabled": "true"], "clear": [String]()]]))
        XCTAssertFalse(s.isEnabled, "the control disabled: the switch too, though the row is unchanged")
        XCTAssertNotNil(host(p).activate(try XCTUnwrap(p.views[13]))?["error"])
        p.apply(wireBatch([["op": "props", "id": 13, "set": [String: String](), "clear": ["disabled"]]]))
        XCTAssertTrue(s.isEnabled)
        // A `when` replaced the control: the same switch flips the new one.
        target = 14
        p.apply(wireBatch([["op": "create", "id": 14, "kind": "control", "props": ["type": "checkbox", "accessibilityRole": "switch"], "handlers": ["input"], "style": [:]],
                           ["op": "children", "id": 12, "ids": [14]], ["op": "destroy", "id": 13]]))
        XCTAssertFalse(s.isOn, "the new control's `checked`")
        s.setOn(true, animated: false)
        s.sendActions(for: .valueChanged)
        XCTAssertEqual(flips, [14])
    }

    private func switches(_ view: UIView) -> [UISwitch] {
        (view as? UISwitch).map { [$0] } ?? view.subviews.flatMap(switches)
    }

    func testACustomRowsViewsAreCarriedAndGivenBackBeforeABatch() throws {
        let p = presenter { self.model(custom: true) }
        let row = try XCTUnwrap(p.views[21])
        let cell = try cell(p, 21)
        XCTAssertTrue(row.superview === cell.contentView, "carried into its cell")
        XCTAssertEqual(row.frame.minX, 16, "at its place in its group")
        XCTAssertEqual(cell.bounds.height, 60, accuracy: 0.5, "the row less the separator the cell draws")
        XCTAssertTrue(host(p).projects(row))
        host(p).prepare()
        XCTAssertTrue(row.superview === p.views[5]?.container, "back where the presenter put it")
        XCTAssertEqual(row.frame, CGRect(x: 16, y: 52, width: 354, height: 61))
        XCTAssertFalse(host(p).projects(row))
    }

    /// A carried row of stacked lines keeps the cell's margins (team-kanban's
    /// cards and household-list's items, 2026-10-10, whose titles touched the
    /// card and buttons the separator): the sheet keeps the stack 15 under the
    /// row's top and 15 over its separator, as a hand-built cell pinned to its
    /// content view's margins has it, and the cell shows the row so, as tall
    /// as the row less the separator it draws instead.
    func testACarriedStackKeepsTheCellsMargins() throws {
        let p = presenter { self.model(custom: true) }
        let text = { (id: Int, text: String) -> [String: Any] in
            ["op": "create", "id": id, "kind": "text", "props": ["text": text], "style": ["text_color": [0, 0, 0, 255]]]
        }
        // The kernel's boxes for `row` > `column gap=8` of a title, a second
        // line and a row of buttons: 15, 22, 8, 20, 8, 34, 15 and the separator.
        p.apply(wireBatch([
            ["op": "create", "id": 44, "kind": "view", "props": [:], "style": [:]],
            text(40, "Ada task one"), text(41, "Ada"),
            ["op": "create", "id": 42, "kind": "view", "props": [:], "style": [:]],
            ["op": "create", "id": 43, "kind": "button", "props": ["testId": "move"], "handlers": ["press"], "style": [:]],
            ["op": "children", "id": 42, "ids": [43]], ["op": "children", "id": 44, "ids": [40, 41, 42]],
            ["op": "children", "id": 21, "ids": [44]],
            ["op": "frame", "id": 21, "x": 16.0, "y": 52.0, "w": 354.0, "h": 123.0],
            ["op": "frame", "id": 44, "x": 0.0, "y": 15.0, "w": 338.0, "h": 92.0],
            ["op": "frame", "id": 40, "x": 0.0, "y": 0.0, "w": 120.0, "h": 22.0],
            ["op": "frame", "id": 41, "x": 0.0, "y": 30.0, "w": 40.0, "h": 20.0],
            ["op": "frame", "id": 42, "x": 0.0, "y": 58.0, "w": 200.0, "h": 34.0],
            ["op": "frame", "id": 43, "x": 0.0, "y": 0.0, "w": 100.0, "h": 34.0],
        ]))
        let cell = try cell(p, 21)
        let row = try XCTUnwrap(p.views[21])
        XCTAssertTrue(row.superview === cell.contentView, "carried")
        XCTAssertEqual(cell.bounds.height, 122, accuracy: 0.5, "the row less the separator the cell draws")
        let first = try XCTUnwrap(p.views[40]), last = try XCTUnwrap(p.views[42])
        let (top, bottom) = (first.convert(first.bounds, to: cell), last.convert(last.bounds, to: cell))
        let margins = cell.contentView.directionalLayoutMargins
        XCTAssertGreaterThanOrEqual(top.minY, 15, "the first line 15 under the card's top: \(top)")
        XCTAssertGreaterThanOrEqual(cell.bounds.height - bottom.maxY, 15, "the buttons 15 over the separator: \(bottom) in \(cell.bounds)")
        XCTAssertGreaterThanOrEqual(top.minX, 16, "the text's margin")
        XCTAssertGreaterThanOrEqual(top.minY, margins.top, "at least UIKit's own content margin")
        XCTAssertGreaterThanOrEqual(cell.bounds.height - bottom.maxY, margins.bottom, "at least UIKit's own content margin")
    }

    /// The symbol's tint is the sheet's (D7), the author's over its own: the
    /// shown symbol the model named, each side of a light-dark() pair for its
    /// appearance, and a row whose tint changes is configured again.
    func testASymbolTakesItsAuthoredTintForEachAppearance() throws {
        let p = presenter { self.model() }
        let before = try XCTUnwrap(try cell(p, 10).contentConfiguration as? UIListContentConfiguration)
        XCTAssertNil(before.imageProperties.tintColor, "no tint authored: UIKit's")
        // A hidden image first, then the symbol (the model's), the title, then the chevron.
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "image", "props": ["imageSource": "symbol:sf/xmark"], "style": ["display": "none", "tint_color": [255, 0, 0, 255]]],
            ["op": "create", "id": 31, "kind": "image", "props": ["imageSource": "symbol:sf/person.circle"],
             "style": ["tint_color": [[0, 0, 0, 255], [255, 255, 255, 255]]]],
            ["op": "create", "id": 33, "kind": "text", "props": ["text": "Profile"], "style": [:]],
            ["op": "create", "id": 32, "kind": "image", "props": ["imageSource": "symbol:forward-chevron"], "style": [:]],
            ["op": "children", "id": 10, "ids": [30, 31, 33, 32]],
        ]))
        func rgb(_ style: UIUserInterfaceStyle) throws -> [CGFloat] {
            let tint = try XCTUnwrap((try cell(p, 10).contentConfiguration as? UIListContentConfiguration)?.imageProperties.tintColor)
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            tint.resolvedColor(with: UITraitCollection(userInterfaceStyle: style)).getRed(&r, green: &g, blue: &b, alpha: &a)
            return [r, g, b, a]
        }
        XCTAssertEqual(try rgb(.light), [0, 0, 0, 1], "the shown symbol's light side, not the hidden image's red")
        XCTAssertEqual(try rgb(.dark), [1, 1, 1, 1], "and its dark side, with no batch")
        // A style-only change to the symbol's tint configures the row again.
        p.apply(wireBatch([["op": "style", "id": 31, "style": ["tint_color": [[0, 0, 255, 255], [255, 0, 0, 255]]]]]))
        XCTAssertEqual(try rgb(.light), [0, 0, 1, 1])
        XCTAssertEqual(try rgb(.dark), [1, 0, 0, 1])
    }

    /// LLP 1116.005 D2: the cell is the field. A touch in a custom row's
    /// cell that no control of the row takes (the cell's margin, a text
    /// under the field) goes to the row's one field, published as the cell's;
    /// a button in the row keeps its touch, and a disabled field takes none.
    func testATouchInAFieldsCellGoesToTheField() throws {
        ExactGroupedLists.install()
        let p = Presenter()
        host(p).model = { $0 == 1 ? GroupedListModel(style: "inset-grouped", sections: [.init(view: 2, header: nil, footer: nil, rows: [Row(view: 30, custom: true)])]) : nil }
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "list", "props": ["listStyle": "inset-grouped"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["semanticTag": "section"], "style": [:]],
            ["op": "create", "id": 4, "kind": "view", "props": [:], "style": [:]],
            ["op": "create", "id": 30, "kind": "view", "props": ["testId": "row"], "style": [:]],
            ["op": "create", "id": 31, "kind": "input", "props": ["testId": "name", "placeholder": "Name"], "handlers": ["input"], "style": [:]],
            ["op": "create", "id": 32, "kind": "text", "props": ["text": "Required"], "style": [:]],
            ["op": "create", "id": 33, "kind": "button", "props": ["testId": "clear"], "handlers": ["press"], "style": [:]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [4]], ["op": "children", "id": 4, "ids": [30]],
            ["op": "children", "id": 30, "ids": [31, 32, 33]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0],
            ["op": "frame", "id": 30, "x": 16.0, "y": 35.0, "w": 370.0, "h": 74.0],
            ["op": "frame", "id": 31, "x": 16.0, "y": 15.0, "w": 260.0, "h": 22.0],
            ["op": "frame", "id": 32, "x": 16.0, "y": 41.0, "w": 260.0, "h": 18.0],
            ["op": "frame", "id": 33, "x": 300.0, "y": 15.0, "w": 54.0, "h": 44.0],
            ["op": "fieldContent", "id": 31, "rect": [0.0, 0.0, 260.0, 22.0], "cell": true],
        ]))
        let row = try cell(p, 30)
        let field = try XCTUnwrap(p.views[31]?.field)
        XCTAssertEqual(field.borderStyle, .none, "borderless in its cell")
        func hit(_ point: CGPoint, in view: UIView? = nil) -> UIView? { row.hitTest(view.map { row.convert(point, from: $0) } ?? point, with: nil) }
        XCTAssertTrue(hit(CGPoint(x: 4, y: 4)) === field, "the cell's margin")
        XCTAssertTrue(hit(CGPoint(x: 10, y: 9), in: p.views[32]) === field, "a text under the field")
        XCTAssertTrue(hit(CGPoint(x: 10, y: 11), in: p.views[31]) === field, "the field itself")
        XCTAssertFalse(hit(CGPoint(x: 27, y: 22), in: p.views[33]) === field, "a button in the row keeps its own touch")
        p.apply(wireBatch([["op": "props", "id": 31, "set": ["testId": "name", "placeholder": "Name", "disabled": "true"], "clear": [String]()]]))
        XCTAssertFalse(hit(CGPoint(x: 4, y: 4)) === field, "a disabled field takes no touch from its cell")
    }

    /// LLP 1116.004 D2 (polls, round 6): the sheet's accent is `AccentColor`,
    /// the platform's tint, and a symbol that follows it is UIKit's: the cell
    /// tints it with its own `tintColor`, which the root's accent sets. The
    /// sheet's fixed blue had drawn a vote's checkmark blue under a teal accent.
    func testASymbolInTheAccentTakesTheCellsTint() throws {
        let p = presenter { self.model() }
        p.apply(wireBatch([
            ["op": "create", "id": 31, "kind": "image", "props": ["imageSource": "symbol:sf/person.circle"],
             "style": ["tint_color": ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]]],
            ["op": "create", "id": 33, "kind": "text", "props": ["text": "Profile"], "style": [:]],
            ["op": "children", "id": 10, "ids": [31, 33]],
        ]))
        let c = try XCTUnwrap(try cell(p, 10).contentConfiguration as? UIListContentConfiguration)
        XCTAssertNil(c.imageProperties.tintColor, "the accent is the cell's tint, not a colour of its own")
        window.tintColor = .systemTeal
        let light = UITraitCollection(userInterfaceStyle: .light)
        XCTAssertEqual(try cell(p, 10).tintColor.resolvedColor(with: light), UIColor.systemTeal.resolvedColor(with: light), "which the app's accent sets")
    }

    /// Outside a batch (a sheet's dismissal replaying geometry, a subtree's
    /// appearance), the projections sync on the next turn with no batch to
    /// carry them (LLP 1079's amendment of 2026-10-04): the collection takes
    /// the list's new box and the switch its control as it now stands.
    func testAnOutOfBatchChangeReachesTheProjectionOnTheNextTurn() throws {
        let p = presenter { self.model() }
        let l = try list(p)
        let toggle = try XCTUnwrap(switches(try cell(p, 12)).first)
        XCTAssertTrue(toggle.isOn)
        p.views[13]?.props["checked"] = "false"
        p.applyGeometry(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 600.0]]).ops[0])
        p.requestProjectionSync() // coalesced with the replay's
        XCTAssertEqual(l.collection.frame.width, 402, "not inline")
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(l.collection.frame, CGRect(x: 0, y: 0, width: 300, height: 600))
        XCTAssertFalse(toggle.isOn)
    }

    /// A carried custom row given a new box outside a batch keeps it, and is
    /// carried into its cell again on the next turn.
    func testAReplayedBoxReachesACarriedRow() throws {
        let p = presenter { self.model(custom: true) }
        let row = try XCTUnwrap(p.views[21])
        let before = try cell(p, 21)
        XCTAssertTrue(row.superview === before.contentView, "carried")
        p.applyGeometry(wireBatch([["op": "frame", "id": 21, "x": 16.0, "y": 52.0, "w": 354.0, "h": 80.0]]).ops[0])
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let after = try cell(p, 21)
        XCTAssertEqual(row.frame.height, 80, "the replayed box, not the one saved at the last carry")
        XCTAssertTrue(row.superview === after.contentView, "carried again")
    }

    /// A card-less section (`section background-color="transparent"`): its
    /// rows sit on the list's background, as Signal's profile header does,
    /// and a section that gains its card back is configured again.
    func testACardlessSectionsCellsAreClear() throws {
        var card = false
        let p = presenter {
            var m = self.model()
            m.sections[1].card = card
            return m
        }
        XCTAssertEqual(try cell(p, 20).backgroundConfiguration?.backgroundColor, .clear, "a standard row: no card")
        XCTAssertNotEqual(try cell(p, 10).backgroundConfiguration?.backgroundColor, .clear, "the other section keeps its card")
        // Pressed, a pressable row still highlights.
        let pressed = try cell(p, 20)
        var state = pressed.configurationState
        state.isHighlighted = true
        pressed.configurationUpdateHandler?(pressed, state)
        XCTAssertNotEqual(pressed.backgroundConfiguration?.backgroundColor, .clear, "highlighted while pressed")
        state.isHighlighted = false
        pressed.configurationUpdateHandler?(pressed, state)
        XCTAssertEqual(pressed.backgroundConfiguration?.backgroundColor, .clear)
        // An otherwise unchanged standard row is configured again when its
        // section gains its card back, and loses it again.
        card = true
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["testId": "s1"], "clear": [String]()]]))
        XCTAssertNotEqual(try cell(p, 20).backgroundConfiguration?.backgroundColor, .clear, "the card is back")
        card = false
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["testId": "s1b"], "clear": [String]()]]))
        XCTAssertEqual(try cell(p, 20).backgroundConfiguration?.backgroundColor, .clear, "and gone again")
    }

    /// LLP 1084 §6.4: an authored space is above the later section, above
    /// its header when it has one; nil keeps UIKit's gaps.
    func testAnAuthoredSpaceSitsAboveTheSectionAndItsHeader() throws {
        var space: (CGFloat?, CGFloat?) = (nil, nil)
        let p = presenter {
            var m = self.model()
            m.sections[0].spaceAbove = space.0
            m.sections[1].spaceAbove = space.1
            return m
        }
        let l = try list(p)
        func layout() -> (header: CGRect, footer: CGRect, first: CGRect, later: CGRect) {
            l.collection.layoutIfNeeded()
            let attributes = l.collection.collectionViewLayout
            let header = attributes.layoutAttributesForSupplementaryView(ofKind: UICollectionView.elementKindSectionHeader, at: IndexPath(item: 0, section: 0))?.frame ?? .null
            let footer = attributes.layoutAttributesForSupplementaryView(ofKind: UICollectionView.elementKindSectionFooter, at: IndexPath(item: 0, section: 0))?.frame ?? .null
            let first = attributes.layoutAttributesForItem(at: IndexPath(item: 0, section: 0))?.frame ?? .null
            let later = attributes.layoutAttributesForItem(at: IndexPath(item: 0, section: 1))?.frame ?? .null
            return (header, footer, first, later)
        }
        let before = layout()
        space = (12, 20)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["testId": "spaced"], "clear": [String]()]]))
        let after = layout()
        XCTAssertEqual(after.header.minY, 12, accuracy: 0.5, "above the header, not between it and its rows")
        XCTAssertEqual(after.first.minY - after.header.maxY, before.first.minY - before.header.maxY, accuracy: 0.5, "the header keeps its rows' gap")
        XCTAssertEqual(after.later.minY - after.footer.maxY, 20, accuracy: 0.5, "20 between the footer and the next section")
    }

    /// The space under a last section is under its footer, and follows an update.
    func testAnAuthoredSpaceUnderTheLastSectionIsUnderItsFooter() throws {
        var below: CGFloat? = nil
        let p = presenter {
            var m = self.model()
            m.sections[1].footer = "The end."
            m.spaceBelow = below
            return m
        }
        let l = try list(p)
        func extent() -> CGFloat { l.collection.layoutIfNeeded(); return l.collection.contentSize.height + l.collection.contentInset.bottom }
        let before = extent()
        below = 60
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["testId": "below60"], "clear": [String]()]]))
        XCTAssertEqual(extent() - before, 60, accuracy: 0.5, "60 under the footer")
        below = 20
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["testId": "below20"], "clear": [String]()]]))
        XCTAssertEqual(extent() - before, 20, accuracy: 0.5, "and 20 after an update")
    }

    /// The list's own padding is room before its first section and after its
    /// last, as a scroll's is: a tab bar's under a settings list.
    func testTheListsPaddingIsRoomAboveAndBelowItsSections() throws {
        let p = presenter { self.model() }
        // A port shorter than the rows, so a resting offset of 0 would stay
        // legal under the new inset and only the follow moves the rows.
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 200.0]]))
        let l = try list(p)
        l.collection.layoutIfNeeded()
        XCTAssertGreaterThan(l.collection.contentSize.height, l.collection.bounds.height, "taller than its port")
        let before = l.collection.contentInset, indicator = l.collection.verticalScrollIndicatorInsets
        // Where the first row shows in the port, at rest.
        func shown() throws -> CGFloat {
            l.collection.layoutIfNeeded()
            return try XCTUnwrap(l.cell(10)).frame.minY - l.collection.contentOffset.y
        }
        let top = try shown()
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "padding_top": 12.0, "padding_bottom": 83.0]]]))
        XCTAssertEqual(l.collection.contentInset.top - before.top, 12, accuracy: 0.5)
        XCTAssertEqual(l.collection.contentInset.bottom - before.bottom, 83, accuracy: 0.5)
        XCTAssertEqual(try shown() - top, 12, accuracy: 0.5, "the room shows at rest, not only past the top")
        XCTAssertEqual(l.collection.verticalScrollIndicatorInsets.top - indicator.top, 12, accuracy: 0.5)
        XCTAssertEqual(l.collection.verticalScrollIndicatorInsets.bottom - indicator.bottom, 83, accuracy: 0.5, "the indicator keeps out of it")
        XCTAssertEqual(l.collection.contentOffset.y + l.collection.adjustedContentInset.top, 0, accuracy: 0.5, "at its top")
        // A reader among the rows keeps them where they are as the padding goes.
        l.collection.contentOffset.y = 60
        let reading = try shown()
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll"]]]))
        XCTAssertEqual(l.collection.contentInset, before, "and none once it is gone")
        XCTAssertEqual(try shown(), reading, accuracy: 0.5, "the rows stay put")
    }

    /// A row moved between a card-less section and a carded one, with
    /// neither section's card changing, is configured for its new section.
    func testARowMovedIntoACardlessSectionLosesItsCard() throws {
        var moved = false
        let p = presenter {
            var m = self.model()
            m.sections[1].card = false
            if moved {
                let row = m.sections[0].rows.removeFirst()
                m.sections[1].rows.insert(row, at: 0)
            }
            return m
        }
        XCTAssertNotEqual(try cell(p, 10).backgroundConfiguration?.backgroundColor, .clear)
        moved = true
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["testId": "s0"], "clear": [String]()]]))
        XCTAssertEqual(try cell(p, 10).backgroundConfiguration?.backgroundColor, .clear, "in the card-less section now")
    }

    func testCustomRowsGoBackInTheirOrder() throws {
        let p = presenter {
            var m = self.model(custom: true)
            m.sections[0].rows[0] = Row(view: 10, custom: true, pressable: true)
            m.sections[0].rows[2] = Row(view: 12, custom: true)
            m.sections[1].rows[0] = Row(view: 20, custom: true, pressable: true)
            return m
        }
        for id: UInt32 in [10, 12, 20, 21] { _ = try cell(p, id) }
        XCTAssertTrue(p.views[10]?.superview !== p.views[4]?.container, "carried")
        host(p).prepare()
        let order = { (id: UInt32) in p.views[id]?.container.subviews.compactMap { ($0 as? NodeView)?.id } }
        XCTAssertEqual(order(4), [10, 11, 12], "the standard row between them keeps its place")
        XCTAssertEqual(order(5), [20, 21])
    }

    func testAnInertRowOrSectionTakesNoTap() throws {
        let p = presenter { self.model() }
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let l = try list(p)
        _ = try cell(p, 20)
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["inert": "true"], "clear": [String]()]]))
        XCTAssertFalse(l.collectionView(l.collection, shouldHighlightItemAt: IndexPath(item: 0, section: 1)))
        XCTAssertNotNil(host(p).activate(try XCTUnwrap(p.views[20]))?["error"])
        XCTAssertFalse(try cell(p, 20).isUserInteractionEnabled, "an inert section's cell takes no touch")
        XCTAssertTrue(try cell(p, 20).accessibilityElementsHidden)
        p.apply(wireBatch([["op": "props", "id": 3, "set": [String: String](), "clear": ["inert"]]]))
        XCTAssertNotNil(host(p).activate(try XCTUnwrap(p.views[20]))?["tapped"])
        XCTAssertEqual(pressed, [20])
    }

    func testACarriedCustomRowKeepsItsSectionsInertness() throws {
        let p = presenter { self.model(custom: true) }
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["inert": "true"], "clear": [String]()]]))
        let custom = try cell(p, 21)
        XCTAssertTrue(p.views[21]?.superview === custom.contentView, "carried")
        XCTAssertFalse(custom.isUserInteractionEnabled, "the section is inert though the cell is not its ancestor")
        XCTAssertTrue(custom.accessibilityElementsHidden)
        XCTAssertTrue(host(p).scroller(for: 21) === (try list(p)).collection, "a custom row's wheel scrolls its list")
    }

    func testARowThatTurnsCustomAndBackAndAHeaderThatChanges() throws {
        var custom = false, header = "Account"
        let p = presenter {
            var m = self.model(custom: custom)
            m.sections[0].header = header
            return m
        }
        let row = try XCTUnwrap(p.views[21])
        XCTAssertNotNil(try cell(p, 21).contentConfiguration as? UIListContentConfiguration)
        custom = true
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "row21"], "clear": [String]()]]))
        XCTAssertTrue(row.superview === (try cell(p, 21)).contentView, "custom now: carried")
        XCTAssertFalse(host(p).draws(21), "the ordinary tap path finds a custom row's views")
        custom = false; header = "Profile"
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "row21"], "clear": [String]()]]))
        XCTAssertTrue(row.superview === p.views[5]?.container, "standard again: given back, and not carried")
        XCTAssertNotNil(try cell(p, 21).contentConfiguration as? UIListContentConfiguration)
        let l = try list(p)
        l.collection.layoutIfNeeded()
        let shown = l.collection.supplementaryView(forElementKind: UICollectionView.elementKindSectionHeader, at: IndexPath(item: 0, section: 0)) as? UICollectionViewListCell
        XCTAssertEqual((shown?.contentConfiguration as? UIListContentConfiguration)?.text, "Profile")
    }

    /// LLP 1116.003 (r5 password generator: `tap history-0` refused, a
    /// different UIKit view "covering its middle" each time): the agent's
    /// reveal scrolls the list UIKit draws to a row with no cell yet, by
    /// that row's cell, and carries a custom row's views into it, so the tap
    /// lands on them. The hidden authored scroll, which moves nothing on
    /// screen, stays where it is.
    func testRevealScrollsTheListToARowWithNoCellYet() throws {
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "grouped-reveal")
        defer { session.destroy() }
        let p = presenter({ self.model(custom: true) }, on: session.presenter, height: 150)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let l = try list(p)
        l.collection.layoutIfNeeded()
        XCTAssertNil(l.cell(21), "below the list's port, the custom row has no cell")
        let row = try XCTUnwrap(p.views[21])
        XCTAssertFalse(row.isDescendant(of: l.collection), "its views wait in the hidden scroll")
        let agent = Agent(session: session)
        let reply = agent.reveal(["id": 21])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["scrolled"] as? Bool, true, "\(reply)")
        let cell = try XCTUnwrap(l.cell(21), "the list scrolled to it by its cell: \(reply)")
        XCTAssertTrue(row.isDescendant(of: cell), "and carried its views in")
        let port = l.collection.bounds.inset(by: l.collection.adjustedContentInset)
        let mid = row.convert(CGPoint(x: row.bounds.midX, y: row.bounds.midY), to: l.collection)
        XCTAssertTrue(port.contains(mid), "its middle is in the list's port: \(mid) in \(port)")
        XCTAssertEqual(p.views[1]?.scroll?.contentOffset.y ?? 0, 0, "the hidden scroll stays")
        XCTAssertNil(agent.tap(["id": 21])["error"])
        XCTAssertEqual(pressed, [21], "the tap pressed the row it revealed")
        // A standard row, scrolled away again: by its cell too, then selected.
        l.collection.setContentOffset(CGPoint(x: 0, y: -l.collection.adjustedContentInset.top), animated: false)
        l.collection.layoutIfNeeded()
        XCTAssertNil(l.cell(20))
        XCTAssertEqual(agent.reveal(["id": 20])["scrolled"] as? Bool, true)
        XCTAssertNotNil(l.cell(20))
        let selected = agent.tap(["id": 20])
        XCTAssertNil(selected["error"], "\(selected)")
        XCTAssertEqual(pressed, [21, 20])
    }

    func testAWheelOnARowScrollsTheListUIKitDraws() throws {
        let p = presenter { self.model() }
        let l = try list(p)
        l.collection.contentInset.bottom = 2000
        l.collection.layoutIfNeeded()
        let scroller = try XCTUnwrap(host(p).scroller(for: 21))
        XCTAssertTrue(scroller === l.collection, "the row's list, not its hidden scroll")
        Agent.scroll(from: scroller, dx: 0, dy: 300)
        XCTAssertEqual(l.collection.contentOffset.y + l.collection.adjustedContentInset.top, 300, accuracy: 0.5)
        XCTAssertEqual(p.views[1]?.scroll?.contentOffset.y ?? 0, 0, "the hidden scroll stays")
    }

    func testASwitchWhoseFlipChangesItsRowKeepsItsSuperviewUntilTheActionReturns() throws {
        var title = "Read Receipts"
        let p = presenter {
            var m = self.model()
            m.sections[0].rows[2].title = title
            return m
        }
        let s = try XCTUnwrap(switches(try cell(p, 12)).first)
        p.onChecked = { _, _ in
            title = "Receipts Off"
            p.apply(wireBatch([["op": "props", "id": 13, "set": ["checked": "false"], "clear": [String]()]]))
            XCTAssertNotNil(s.superview, "not taken out inside its own action")
        }
        s.setOn(false, animated: false)
        s.sendActions(for: .valueChanged)
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        let c = try XCTUnwrap(try cell(p, 12).contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(c.text, "Receipts Off", "reconfigured once the action returned")
        XCTAssertFalse(s.isOn)
    }

    /// A grouped section holding a custom row that names a trailing swipe
    /// action (row 21: content 22, Delete 23) above a standard row.
    private func swipeRow(scrolls: Bool = false, _ onPress: @escaping (UInt32) -> Void = { _ in }) -> Presenter {
        ExactGroupedLists.install()
        let p = Presenter()
        host(p).model = { $0 == 1 ? GroupedListModel(sections: [
            .init(view: 2, header: nil, footer: nil, rows: [Row(view: 21, custom: true), Row(view: 30, title: "Plain")]),
        ]) : nil }
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.onPress = onPress
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "list", "props": ["listStyle": "inset-grouped"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["semanticTag": "section"], "style": [:]],
            ["op": "create", "id": 4, "kind": "view", "props": [:], "style": [:]],
            ["op": "create", "id": 21, "kind": "view", "props": ["swipeContent": "body", "swipeTrailing": "delete", "testId": "swipe-row"],
             "handlers": scrolls ? ["scroll"] : [String](),
             "style": ["overflow_x": "scroll", "overflow_y": "hidden", "padding_right": 16.0, "border_width_bottom": 1.0]],
            ["op": "create", "id": 22, "kind": "view", "props": ["id": "body"], "style": [:]],
            ["op": "create", "id": 23, "kind": "button", "props": ["id": "delete", "accessibilityLabel": "Delete Milk", "destructive": "true"], "handlers": ["press"], "style": [:]],
            ["op": "create", "id": 30, "kind": "view", "props": [:], "style": [:]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [4]], ["op": "children", "id": 4, "ids": [21, 30]],
            ["op": "children", "id": 21, "ids": [22, 23]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0],
            // The sheet's 16 in and 16 of padding: the content is 370 of a 386 × 53 row.
            ["op": "frame", "id": 21, "x": 16.0, "y": 0.0, "w": 386.0, "h": 53.0],
            ["op": "frame", "id": 22, "x": 0.0, "y": 9.0, "w": 370.0, "h": 34.0],
            ["op": "frame", "id": 23, "x": 370.0, "y": 9.0, "w": 60.0, "h": 34.0],
            ["op": "frame", "id": 30, "x": 16.0, "y": 52.0, "w": 354.0, "h": 52.0],
        ]))
        return p
    }

    /// LLP 1116.001 D1: a custom row that names swipe actions (the
    /// `list-rows` recipe's scroll, in a grouped section) swipes as a
    /// hand-built list's cell does: UIKit's actions on its list cell, a full
    /// swipe the first. Its content need not fill the row (the sheet's
    /// inset and padding), no table is projected, and its controls stay
    /// hidden while the row is carried (grocery-list, round 3: the row was
    /// refused and swiped 27 points by its scroll, the action clipped).
    func testASwipeRowTakesItsListCellsOwnActions() throws {
        var pressed: [UInt32] = []
        let p = swipeRow { pressed.append($0) }
        let owner = try XCTUnwrap(p.views[21]), delete = try XCTUnwrap(p.views[23])
        XCTAssertNil(p.swipeActions.refusal(of: owner), "a grouped row's content need not fill it")
        XCTAssertTrue(delete.isHidden, "the cell's action stands for the control")
        XCTAssertNil(owner.scroll, "no scroll to pan: the cell's swipe takes the touch")
        XCTAssertTrue(p.swipeActions.ownsAction(23))
        let c = try cell(p, 21)
        XCTAssertTrue(owner.isDescendant(of: c.contentView), "carried into its list cell")
        p.swipeActions.touch(owner)
        XCTAssertNil(p.swipeActions.cell(of: owner), "no table projected: the list cell is UIKit's own")
        XCTAssertFalse(owner.subviews.contains { $0 is UITableView })

        let l = try list(p)
        let trailing = try XCTUnwrap(l.swipeActions(at: IndexPath(item: 0, section: 0), leading: false))
        XCTAssertEqual(trailing.actions.count, 1)
        XCTAssertEqual(trailing.actions.first?.style, .destructive)
        XCTAssertEqual(trailing.actions.first?.accessibilityLabel, "Delete Milk")
        XCTAssertTrue(trailing.performsFirstActionWithFullSwipe, "a full swipe deletes")
        XCTAssertNil(l.swipeActions(at: IndexPath(item: 0, section: 0), leading: true), "it names no leading action")
        XCTAssertNil(l.swipeActions(at: IndexPath(item: 1, section: 0), leading: false), "a standard row has none")
        var completed: Bool?
        try XCTUnwrap(trailing.actions.first).handler(try XCTUnwrap(trailing.actions.first), UIView(), { completed = $0 })
        XCTAssertEqual(completed, true)
        XCTAssertEqual(pressed, [23], "the action is the control's press")

        // A batch gives the row back and carries it again; the control
        // stays the cell's action, and the agent's tap on it, unrevealed,
        // performs it as assistive technology would.
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "swipe-row"], "clear": [String]()]]))
        XCTAssertTrue(delete.isHidden)
        XCTAssertTrue(owner.isDescendant(of: try cell(p, 21).contentView))
        XCTAssertEqual(p.swipeActions.perform(23), true)
        XCTAssertEqual(pressed, [23, 23])
        // An inert section offers no action.
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["inert": "true"], "clear": [String]()]]))
        XCTAssertNil(l.swipeActions(at: IndexPath(item: 0, section: 0), leading: false), "inert: nothing offered")
        XCTAssertEqual(p.swipeActions.perform(23), false)
    }

    /// LLP 1116.008 D4 (grocery-list, round 10): the row's own horizontal snap
    /// scroll never takes a pan the list cell's swipe should. At rest it is no
    /// view at all (above); where something made it one (an authored `scroll`
    /// handler or position, the agent's wheel) it is stilled and at its start
    /// while the cell is UIKit's, and every batch gives it back and stills it
    /// again, so UIKit's swipe sees the first pan of a short drag as a native
    /// list's does (the grader's "short swipe" was Maestro's nine-point drag
    /// from the checkbox, which a hand-built UIKit list takes no differently).
    func testAGroupedSwipeRowsOwnScrollNeverPansWhileItsCellSwipes() throws {
        let p = swipeRow(scrolls: true)
        let owner = try XCTUnwrap(p.views[21])
        let scroll = try XCTUnwrap(owner.scroll, "a `scroll` handler wants the scroll view")
        XCTAssertFalse(scroll.isScrollEnabled, "its pan is off: the cell's swipe takes the touch")
        XCTAssertEqual(scroll.contentOffset.x, -scroll.adjustedContentInset.left, "at its start")
        XCTAssertNil(p.swipeActions.refusal(of: owner))
        XCTAssertTrue(owner.isDescendant(of: try cell(p, 21).contentView), "the cell is UIKit's")
        for _ in 0..<2 {
            p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "swipe-row"], "clear": [String]()]]))
            XCTAssertFalse(try XCTUnwrap(owner.scroll).isScrollEnabled, "stilled again after each batch")
        }
        XCTAssertNotNil(try list(p).swipeActions(at: IndexPath(item: 0, section: 0), leading: false))
        // A row that stops swiping is the scroll's again, as the web's.
        p.apply(wireBatch([["op": "props", "id": 21, "set": [String: String](), "clear": ["swipeContent"]]]))
        XCTAssertTrue(try XCTUnwrap(owner.scroll).isScrollEnabled)
    }

    /// LLP 1116.004 D2 (grocery-list, round 6): a contact in a swipe row
    /// that leaves the slop toward a side with actions is the swipe's, never
    /// a press, whether or not UIKit's swipe goes on to begin. Maestro's
    /// "fast" swipe from the row's checkbox moved nine points, short of
    /// UIKit's threshold, and the checkbox took it as a tap. The row's claim
    /// cancels the touch beneath and stands in no one's way; a drag toward
    /// a side with no actions, or up the list, stays the press's.
    func testASidewaysDragTowardTheRowsActionsIsTheSwipesNotAPress() throws {
        let p = swipeRow()
        let owner = try XCTUnwrap(p.views[21])
        XCTAssertTrue(owner.isDescendant(of: try cell(p, 21).contentView))
        let claims = { (owner.gestureRecognizers ?? []).compactMap { $0 as? SwipeActionsHost.SwipeClaim } }
        let claim = try XCTUnwrap(claims().first, "the row claims its sideways touches")
        XCTAssertTrue(claim.cancelsTouchesInView, "claiming cancels the press beneath")
        XCTAssertFalse(claim.delaysTouchesEnded, "a tap's lift is not held back")
        let swipe = UIPanGestureRecognizer()
        XCTAssertFalse(claim.canPrevent(swipe), "UIKit's swipe goes on")
        XCTAssertFalse(claim.canBePrevented(by: swipe))
        let sides = claim.sides()
        XCTAssertTrue(sides.left, "a drag to the left reveals the trailing Delete")
        XCTAssertFalse(sides.right, "no leading action")
        let decide = { (dx: CGFloat, dy: CGFloat) in
            SwipeActionsHost.SwipeClaim.decide(dx: dx, dy: dy, slop: CGFloat(Gesture.slop), left: sides.left, right: sides.right)
        }
        XCTAssertEqual(decide(-9, 0), .claim, "Maestro's swipe from the checkbox to the screen's tenth")
        XCTAssertEqual(decide(-48, 2), .claim, "a flick that lifts before UIKit's swipe begins")
        XCTAssertEqual(decide(-3, 1), .wait, "a tap's wobble is still a tap")
        XCTAssertEqual(decide(9, 0), .fail, "toward no action: the press stays, as a UIKit cell's")
        XCTAssertEqual(decide(-3, 9), .fail, "up the list: the scroll's")
        XCTAssertTrue(SwipeActionsHost.SwipeClaim.drags(UISwitch()), "a switch keeps its own sideways drag")
        // A batch keeps the one claim; a row that stops swiping drops it.
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "swipe-row"], "clear": [String]()]]))
        XCTAssertEqual(claims().count, 1)
        p.apply(wireBatch([["op": "props", "id": 21, "set": [String: String](), "clear": ["swipeContent"]]]))
        XCTAssertTrue(claims().isEmpty)
    }

    func testABatchUpdatesTheRowsAndAGoneListGoes() throws {
        var dark = false
        let p = presenter { self.model(dark: dark) }
        XCTAssertTrue(try cell(p, 20).accessories.isEmpty)
        dark = true
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["testId": "row20"], "clear": [String]()]]))
        XCTAssertEqual(try cell(p, 20).accessories.count, 1, "the checkmark")
        p.apply(wireBatch([["op": "roots", "ids": []], ["op": "destroy", "id": 1]]))
        XCTAssertNil(host(p).lists[1])
    }

    /// LLP 1116.002 D2, round 4's countdowns: the commit that lands while a
    /// sheet covers the list (the saved event's refresh, before `then`
    /// closes the form) inserts a row and a sibling section, the "next up"
    /// card, and grows a row; the sheet holds their boxes until it closes.
    /// All of it shows on the batch that closes the sheet, not after the
    /// screen is left: a cell first sized to nothing is one UIKit never
    /// shows again, and a box given back to a row that a layout pass in that
    /// batch carried again is the row's, not one the next `restore` undoes.
    func testWhatACommitInsertsUnderASheetShowsWhenItCloses() throws {
        var m = model(custom: true)
        let p = presenter { m }
        // The list's route where navigation hosts one: a controller's view in
        // a scene's window, which UIKit presents a sheet over.
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let screen = scene.map { UIWindow(windowScene: $0) } ?? UIWindow()
        screen.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        let owner = UIViewController(), background = UIViewController()
        screen.rootViewController = owner
        screen.makeKeyAndVisible()
        window.isHidden = true
        window = screen
        addTeardownBlock { owner.presentedViewController?.dismiss(animated: false); screen.isHidden = true }
        p.viewport.frame = owner.view.bounds
        owner.view.addSubview(p.viewport)
        owner.addChild(background)
        background.view.frame = owner.view.bounds
        owner.view.addSubview(background.view)
        background.didMove(toParent: owner)
        let route = try XCTUnwrap(p.views[1])
        background.view.addSubview(route)
        // The sheet's route, and the document's first root once the list's
        // route has left it (the closing batch changes its viewport fit).
        p.apply(wireBatch([
            ["op": "create", "id": 90, "kind": "view", "props": ["navigationPresentation": "modal"], "style": [:]],
            ["op": "create", "id": 95, "kind": "view", "props": [:], "style": [:]],
            ["op": "roots", "ids": [95]],
        ]))
        p.modals.present(try XCTUnwrap(p.views[90]), navigation: UIViewController(), from: background, node: route, preceding: nil, owner: owner)
        XCTAssertEqual(p.modals.routes.count, 1, "the sheet is up")

        // The refresh, under the sheet: a "next up" section first, a new row
        // under the others, and row 21 grown. The sheet holds every box.
        m.sections.insert(.init(view: 69, header: nil, footer: nil, rows: [Row(view: 71, custom: true)]), at: 0)
        m.sections[2].rows.append(Row(view: 80, custom: true, pressable: true))
        let view = { (id: Int, kind: String, props: [String: String]) -> [String: Any] in
            ["op": "create", "id": id, "kind": kind, "props": props, "style": ["text_color": [0, 0, 0, 255], "border_width_bottom": 1.0]]
        }
        p.apply(wireBatch([
            view(69, "view", ["semanticTag": "section"]), view(70, "view", [:]), view(71, "view", ["testId": "next-up"]),
            view(72, "text", ["text": "NEXT UP"]), view(80, "button", ["testId": "row80"]), view(81, "text", ["text": "Holiday"]),
            ["op": "children", "id": 1, "ids": [69, 2, 3]], ["op": "children", "id": 69, "ids": [70]], ["op": "children", "id": 70, "ids": [71]],
            ["op": "children", "id": 71, "ids": [72]], ["op": "children", "id": 5, "ids": [20, 21, 80]], ["op": "children", "id": 80, "ids": [81]],
            ["op": "frame", "id": 69, "x": 0.0, "y": 0.0, "w": 402.0, "h": 120.0],
            ["op": "frame", "id": 70, "x": 16.0, "y": 10.0, "w": 370.0, "h": 101.0],
            ["op": "frame", "id": 71, "x": 0.0, "y": 0.0, "w": 370.0, "h": 101.0],
            ["op": "frame", "id": 72, "x": 16.0, "y": 20.0, "w": 100.0, "h": 20.0],
            ["op": "frame", "id": 21, "x": 16.0, "y": 52.0, "w": 354.0, "h": 91.0],
            ["op": "frame", "id": 80, "x": 16.0, "y": 143.0, "w": 354.0, "h": 73.0],
            ["op": "frame", "id": 81, "x": 16.0, "y": 26.0, "w": 100.0, "h": 21.0],
        ]))
        for id: UInt32 in [71, 80, 21] { XCTAssertTrue(p.modals.holdsGeometry(id), "the sheet holds #\(id)'s box") }
        XCTAssertEqual(p.views[80]?.frame.height, 0, "no box yet")

        // The batch that closes it, as navigation's sync does mid-batch; UIKit
        // laid the list out in that batch, carrying its shown rows again.
        let l = try list(p)
        p.onViewportFit = {
            l.mount()
            p.modals.closeTop(animated: false)
        }
        p.apply(wireBatch([["op": "props", "id": 95, "set": ["viewportFit": "cover"], "clear": [String]()]]))
        p.onViewportFit = nil
        XCTAssertTrue(p.modals.routes.isEmpty, "the sheet closed")
        func shown(_ id: UInt32, _ height: CGFloat, _ what: String) throws {
            let cell = try self.cell(p, id), row = try XCTUnwrap(p.views[id])
            XCTAssertTrue(row.superview === cell.contentView, "\(what): carried into its cell")
            XCTAssertEqual(cell.bounds.height, height, accuracy: 0.5, "\(what): the row less its separator")
            XCTAssertTrue(l.collection.bounds.intersects(cell.frame), "\(what): in the list's port")
        }
        try shown(71, 100, "the next-up card")
        try shown(80, 72, "the new row")
        try shown(21, 90, "the grown row")
        // The next batch gives the rows back and carries them again at
        // their boxes, the ones the sheet held.
        p.apply(wireBatch([["op": "props", "id": 95, "set": ["testId": "root"], "clear": [String]()]]))
        XCTAssertEqual(p.views[21]?.frame.height ?? 0, 91, accuracy: 0.5, "the replayed box, kept")
        try shown(21, 90, "the grown row, after a batch")
        try shown(80, 72, "the new row, after a batch")
    }

    /// LLP 1116.003 D7, round 5's password-generator: at launch the list
    /// lays out before its box settles, so UIKit configures and shows every
    /// row of a long list, then keeps the cells that leave its port for
    /// when they come back. A batch that leaves the list alone gives the
    /// rows home and carries them again only into the cells shown, so a kept
    /// cell came back on a scroll without its row, blank until scrolled away
    /// and back. A cell UIKit shows again is given its row as it shows.
    func testAKeptCellShowsItsRowWhenItComesBack() throws {
        var m = model(custom: true)
        let ids = (100..<112).map { UInt32($0) }
        m.sections.append(.init(view: 6, header: "Recent", footer: nil, rows: ids.map { Row(view: $0, custom: true, pressable: true) }))
        let p = presenter({ m }, height: 3000)
        let view = { (id: Int, kind: String, props: [String: String]) -> [String: Any] in
            ["op": "create", "id": id, "kind": kind, "props": props, "style": ["text_color": [0, 0, 0, 255], "border_width_bottom": 1.0]]
        }
        var ops: [[String: Any]] = [view(6, "view", ["semanticTag": "section"]), view(7, "view", [:]),
                                    ["op": "create", "id": 95, "kind": "view", "props": [:], "style": [:]]]
        for (i, id) in ids.enumerated() {
            ops += [view(Int(id), "button", ["testId": "row\(id)"]), view(Int(id) + 100, "text", ["text": "Row \(i)"]),
                    ["op": "children", "id": Int(id), "ids": [Int(id) + 100]],
                    ["op": "frame", "id": Int(id), "x": 16.0, "y": Double(i) * 100, "w": 354.0, "h": 101.0],
                    ["op": "frame", "id": Int(id) + 100, "x": 16.0, "y": 30.0, "w": 200.0, "h": 21.0]]
        }
        ops += [["op": "children", "id": 1, "ids": [2, 3, 6]], ["op": "children", "id": 6, "ids": [7]],
                ["op": "children", "id": 7, "ids": ids.map { Int($0) }], ["op": "roots", "ids": [1, 95]]]
        p.apply(wireBatch(ops))
        let l = try list(p)
        l.collection.layoutIfNeeded()
        for id in ids { XCTAssertTrue(p.views[id]?.superview === l.cell(id)?.contentView, "row \(id) shown in the tall box") }
        // The box settles; the later rows leave the port.
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 600.0]]))
        l.collection.layoutIfNeeded()
        // A batch that touches nothing in the list.
        p.apply(wireBatch([["op": "props", "id": 95, "set": ["testId": "elsewhere"], "clear": [String]()]]))
        // Scrolled to the end, every row shown is in its cell.
        let c = l.collection
        c.setContentOffset(CGPoint(x: 0, y: max(0, c.contentSize.height + c.adjustedContentInset.bottom - c.bounds.height)), animated: false)
        c.layoutIfNeeded()
        var seen = 0
        for case let cell as GroupedCell in c.visibleCells {
            guard let id = cell.row, ids.contains(id) else { continue }
            seen += 1
            XCTAssertTrue(p.views[id]?.superview === cell.contentView, "row \(id): shown in its cell, not blank")
        }
        XCTAssertGreaterThan(seen, 3, "the recent rows are in the port")
    }

    /// The second half of password-generator's blank rows: a carried row's
    /// paragraph drawn while the row was home under the hidden scroll paints
    /// nothing, and may find no worker free. Its cell shows on a scroll of
    /// the list, outside any batch's text refresh; the list's scroll drives
    /// the text as its authored scroll's would, and a row carried in asks
    /// for it, so what shows is painted.
    func testACarriedRowsTextIsPaintedWhenItsCellShows() throws {
        let session = ExactApp.shared.makeSession(label: "grouped-text")
        defer { session.destroy(); RegionTextExecutor.queue.isSuspended = false }
        var m = model(custom: true)
        let ids = (100..<112).map { UInt32($0) }
        m.sections.append(.init(view: 6, header: nil, footer: nil, rows: ids.map { Row(view: $0, custom: true) }))
        let p = presenter({ m }, on: session.presenter, height: 600)
        let view = { (id: Int, kind: String) -> [String: Any] in
            ["op": "create", "id": id, "kind": kind, "props": [:], "style": ["text_color": [0, 0, 0, 255], "border_width_bottom": 1.0]]
        }
        var ops: [[String: Any]] = [view(6, "view"), view(7, "view")]
        for (i, id) in ids.enumerated() {
            ops += [view(Int(id), "view"),
                    ["op": "create", "id": Int(id) + 100, "kind": "text", "props": ["text": "Recent password \(i)"], "style": ["font_size": 17.0]],
                    ["op": "children", "id": Int(id), "ids": [Int(id) + 100]],
                    ["op": "frame", "id": Int(id), "x": 16.0, "y": Double(i) * 100, "w": 354.0, "h": 101.0],
                    ["op": "frame", "id": Int(id) + 100, "x": 16.0, "y": 30.0, "w": 300.0, "h": 22.0]]
        }
        ops += [["op": "children", "id": 1, "ids": [2, 3, 6]], ["op": "children", "id": 6, "ids": [7]],
                ["op": "children", "id": 7, "ids": ids.map { Int($0) }]]
        p.apply(wireBatch(ops))
        let l = try list(p)
        l.collection.layoutIfNeeded()
        // The last row's text has no pixels, and no pump is running.
        let last = try XCTUnwrap(ids.last), text = try XCTUnwrap(p.views[last + 100])
        XCTAssertNil(l.cell(last), "the last row is far below the port")
        RegionTextExecutor.queue.waitUntilAllOperationsAreFinished()
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        text.dropTextRaster()
        p.scrollPump.reset()
        // A finger scrolls the list to its end (the rows' sizes are learned
        // as they show, so the end moves until the last one has).
        let c = l.collection
        for _ in 0..<8 where l.cell(last).map({ !c.bounds.contains($0.frame) }) ?? true {
            c.setContentOffset(CGPoint(x: 0, y: max(0, c.contentSize.height + c.adjustedContentInset.bottom - c.bounds.height)), animated: false)
            c.layoutIfNeeded()
        }
        let cell = try XCTUnwrap(l.cell(last), "its cell shows")
        XCTAssertTrue(text.isDescendant(of: cell.contentView), "carried in")
        XCTAssertTrue(p.textIsVisible(text))
        let end = Date().addingTimeInterval(1)
        while !text.textRasterReady && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(text.textRasterReady, "what shows is painted, not left blank")
    }

    /// LLP 1116.003 D7, round 5's chat-rooms: a room's last message, its
    /// subtitle line written `line-clamp=1`, grew the row to 13 lines, as
    /// UIKit wraps a cell's text unless told. The model's look is the
    /// author's clamp, and the cell keeps to it; a text that says nothing
    /// keeps UIKit's.
    func testAStandardCellsTextKeepsItsLineClamp() throws {
        let message = String(repeating: "A long last message that says a great deal. ", count: 11)
        var look = GroupedListModel.Look(lines: 1, ellipsis: true)
        let p = presenter {
            var m = self.model()
            m.sections[0].rows[1] = Row(view: 11, title: "General", secondary: message, subtitle: true, accessory: "disclosure",
                                        pressable: true, secondaryLook: look)
            return m
        }
        let clamped = try cell(p, 11)
        let c = try XCTUnwrap(clamped.contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(c.secondaryTextProperties.numberOfLines, 1)
        XCTAssertEqual(c.secondaryTextProperties.lineBreakMode, .byTruncatingTail, "an ellipsis, as CSS's clamp draws")
        XCTAssertEqual(c.textProperties.numberOfLines, 0, "the title says nothing: UIKit's")
        let short = clamped.bounds.height
        XCTAssertLessThan(short, 80, "one line under the title")
        look = .init()
        p.apply(wireBatch([["op": "props", "id": 11, "set": ["testId": "row11"], "clear": [String]()]]))
        let wrapped = try cell(p, 11)
        XCTAssertEqual((wrapped.contentConfiguration as? UIListContentConfiguration)?.secondaryTextProperties.numberOfLines, 0)
        XCTAssertGreaterThan(wrapped.bounds.height, short * 3, "unclamped, every line")
    }

    /// LLP 1116.003 D7, round 5's unit-converter: its validation message, a
    /// footer's text written `color="-exact-system-red"`, was red on the web
    /// and the footer's grey on iOS. Author > platform (LLP 1115): a footer,
    /// a header and a standard cell draw the colour and font their author
    /// gave the text, a destructive row's included; what is unsaid stays
    /// UIKit's.
    func testAnAuthoredColourAndFontReachTheTextsUIKitDraws() throws {
        let session = ExactApp.shared.makeSession(label: "grouped-looks")
        defer { session.destroy() }
        typealias Look = GroupedListModel.Look
        let red: BatchValue = ["sys": "systemRedColor", "c": [[255, 56, 60, 255], [255, 66, 69, 255]]]
        let p = presenter({
            var m = self.model()
            m.sections[0].footerLook = Look(color: red)
            m.sections[0].headerLook = Look(font: .init(size: 13, weight: 400))
            m.sections[0].rows[0].titleLook = Look(color: [255, 0, 0, 255], font: .init(size: 20, weight: 700))
            m.sections[1].rows[1].titleLook = Look(color: [0, 128, 0, 255])
            return m
        }, on: session.presenter)
        let l = try list(p)
        l.collection.layoutIfNeeded()
        let light = UITraitCollection(userInterfaceStyle: .light), dark = UITraitCollection(userInterfaceStyle: .dark)
        func rgba(_ color: UIColor?, _ traits: UITraitCollection) -> [Int] {
            var c: (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
            color?.resolvedColor(with: traits).getRed(&c.0, green: &c.1, blue: &c.2, alpha: &c.3)
            return [c.0, c.1, c.2, c.3].map { Int(($0 * 255).rounded()) }
        }
        func label(_ kind: String) throws -> (UICollectionViewListCell, UIListContentConfiguration) {
            let view = try XCTUnwrap(l.collection.supplementaryView(forElementKind: kind, at: IndexPath(item: 0, section: 0)) as? UICollectionViewListCell)
            return (view, try XCTUnwrap(view.contentConfiguration as? UIListContentConfiguration))
        }
        let (footer, f) = try label(UICollectionView.elementKindSectionFooter)
        XCTAssertEqual(rgba(f.textProperties.color, light), rgba(.systemRed, light), "the footer's red, the platform's red")
        XCTAssertEqual(rgba(f.textProperties.color, dark), rgba(.systemRed, dark), "in each appearance")
        XCTAssertEqual(f.textProperties.font, footer.defaultContentConfiguration().textProperties.font, "its font unsaid: UIKit's")
        let (header, h) = try label(UICollectionView.elementKindSectionHeader)
        XCTAssertEqual(h.textProperties.font.pointSize, 13, "the header's own size")
        XCTAssertEqual(rgba(h.textProperties.color, light), rgba(header.defaultContentConfiguration().textProperties.color, light), "its colour unsaid")
        let profile = try XCTUnwrap(try cell(p, 10).contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(rgba(profile.textProperties.color, light), [255, 0, 0, 255])
        XCTAssertEqual(profile.textProperties.font.pointSize, 20)
        XCTAssertGreaterThanOrEqual(TextEngine.controlWeight(profile.textProperties.font), 700, "bold, as written")
        let delete = try XCTUnwrap(try cell(p, 21).contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(rgba(delete.textProperties.color, light), [0, 128, 0, 255], "the author's over a destructive row's red")
        let plain = try cell(p, 11)
        let notes = try XCTUnwrap(plain.contentConfiguration as? UIListContentConfiguration)
        let platform = UIListContentConfiguration.valueCell()
        XCTAssertEqual(rgba(notes.textProperties.color, light), rgba(platform.textProperties.color, light), "a title that says nothing")
        XCTAssertEqual(rgba(notes.secondaryTextProperties.color, light), rgba(platform.secondaryTextProperties.color, light))
        XCTAssertEqual(notes.textProperties.font, platform.textProperties.font)
    }

    /// LLP 1116.004 D2 (chat-rooms, round 6): a page with no navigation
    /// container keeps its viewport inside the safe area, and a grouped list
    /// at its bottom left the root's white under the home indicator. Where
    /// the list's box meets an edge of a viewport short of its container's,
    /// the list's background goes on to the container's edge, behind the
    /// viewport; not while the keyboard ends the viewport.
    func testAListAtTheViewportsEdgeDrawsItsBackgroundToTheScreensEdge() throws {
        let p = presenter({ self.model() }, height: 840)
        let grey: [[Int]] = [[242, 242, 247, 255], [0, 0, 0, 255]]
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["background_color": grey]]]))
        // The home indicator's 34 points under the viewport.
        p.viewport.frame = CGRect(x: 0, y: 0, width: 402, height: 840)
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 840.0]]))
        let skirts = { self.window.subviews.filter { $0 !== p.viewport && !$0.isHidden && $0.frame.maxY == 874 } }
        let skirt = try XCTUnwrap(skirts().first, "the list's background reaches the screen's bottom edge")
        XCTAssertEqual(skirt.frame, CGRect(x: 0, y: 0, width: 402, height: 874))
        XCTAssertLessThan(try XCTUnwrap(window.subviews.firstIndex(of: skirt)), try XCTUnwrap(window.subviews.firstIndex(of: p.viewport)),
                          "behind the viewport: only what lies past its edge shows")
        XCTAssertFalse(skirt.isUserInteractionEnabled)
        func rgb(_ style: UIUserInterfaceStyle) throws -> [CGFloat] {
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            try XCTUnwrap(skirt.backgroundColor).resolvedColor(with: UITraitCollection(userInterfaceStyle: style)).getRed(&r, green: &g, blue: &b, alpha: &a)
            return [r, g, b, a].map { ($0 * 255).rounded() }
        }
        XCTAssertEqual(try rgb(.light), [242, 242, 247, 255], "the list's grey, in light")
        XCTAssertEqual(try rgb(.dark), [0, 0, 0, 255], "and in dark")
        // The keyboard ends the viewport: the keys cover that edge.
        p.keyboardInset = 300
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 840.0]]))
        XCTAssertTrue(skirts().isEmpty)
        p.keyboardInset = 0
        // A list short of the viewport's edge draws nothing past it.
        p.apply(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 600.0]]))
        XCTAssertTrue(skirts().isEmpty)
    }

    /// LLP 1115.000 D11.3 over `scripts/fixtures/navigation-bars.contract`'s
    /// `rows` route, a grouped list in a stack: a tapped row that pushes a
    /// screen stays selected under it and lets go as the reader comes back,
    /// as a table view's row does (`clearsSelectionOnViewWillAppear`); a row
    /// that pushes nothing is still selected while its press runs, then lets
    /// go. Before, every row was deselected before its press ran.
    func testATappedRowStaysSelectedThroughThePushAndLetsGoOnReturn() throws {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_NAVIGATION_BARS_PLAN"], "build.mjs --test --ios compiles the fixtures' plans")))
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "rows-selection")
        defer { session.destroy(); window.isHidden = true }
        let view = ExactView(session: session), root = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = root
        window.makeKeyAndVisible()
        root.view.addSubview(view)
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = root.view.bounds
        view.layoutIfNeeded()
        let spin = { (seconds: Double) in RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
        let p = session.presenter, nav = try XCTUnwrap(p.navigation.primaryNavigation)
        func node(_ testId: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == testId }, "no \(testId)") }
        func settle(_ count: Int, _ what: String) {
            let deadline = Date().addingTimeInterval(5)
            while !(nav.viewControllers.count == count && nav.transitionCoordinator == nil), Date() < deadline { spin(0.02) }
            XCTAssertEqual(nav.viewControllers.count, count, what)
            spin(0.3)
        }
        spin(0.3)
        _ = Agent(session: session).tap(["id": Int(try node("start-open-rows").id)])
        settle(2, "the list's route pushed")
        let list = try XCTUnwrap(host(p).lists[try node("rows-list").id], "the route's grouped list")
        list.collection.layoutIfNeeded()
        let selected = { list.collection.indexPathsForSelectedItems ?? [] }
        // A row that pushes nothing: selected while its press runs, then not.
        var duringPress: [IndexPath] = [], presses = 0
        let onPress = p.onPress
        p.onPress = { id in duringPress = selected(); presses += 1; onPress?(id) }
        XCTAssertEqual(host(p).activate(try node("rows-stay"))?["native"] as? String, "grouped-list")
        XCTAssertEqual(presses, 1)
        XCTAssertEqual(duringPress, [IndexPath(item: 1, section: 0)], "the row is selected while its press runs")
        XCTAssertEqual(selected(), [], "a row that pushed nothing lets go")
        // A row that pushes a screen stays selected under it.
        XCTAssertEqual(host(p).activate(try node("rows-open-detail"))?["native"] as? String, "grouped-list")
        settle(3, "the row pushed its screen")
        XCTAssertEqual(selected(), [IndexPath(item: 0, section: 0)], "the tapped row stays selected through the push")
        // Back, as UIKit's back button pops: the row lets go as its screen returns.
        nav.popViewController(animated: false)
        settle(2, "back on the list")
        XCTAssertEqual(selected(), [], "and lets go on the way back")
    }
}
#endif
