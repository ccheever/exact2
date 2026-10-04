#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

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

    private func presenter(_ m: @escaping () -> GroupedListModel) -> Presenter {
        let p = Presenter()
        p.groupedList = { $0 == 1 ? m() : nil }
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
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0],
                ["op": "frame", "id": 21, "x": 16.0, "y": 52.0, "w": 354.0, "h": 61.0]]
        p.apply(wireBatch(ops))
        return p
    }

    private func list(_ p: Presenter) throws -> GroupedListView { try XCTUnwrap(p.groupedLists.lists[1]) }
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
        let seen = try XCTUnwrap(p.groupedLists.observation(try XCTUnwrap(p.views[1])))
        XCTAssertEqual(seen["view"] as? String, "UICollectionView")
        XCTAssertEqual(seen["rows"] as? Int, 5)
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

    func testATapHighlightsAndPressesTheRowOnce() throws {
        let p = presenter { self.model() }
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let l = try list(p)
        _ = try cell(p, 10)
        XCTAssertTrue(l.collectionView(l.collection, shouldHighlightItemAt: IndexPath(item: 0, section: 0)))
        XCTAssertFalse(l.collectionView(l.collection, shouldHighlightItemAt: IndexPath(item: 2, section: 0)), "a `row` is not a button")
        XCTAssertEqual(p.groupedLists.activate(try XCTUnwrap(p.views[10]))?["native"] as? String, "grouped-list")
        XCTAssertEqual(pressed, [10])
        XCTAssertNotNil(p.groupedLists.activate(try XCTUnwrap(p.views[12]))?["error"], "nothing to press")
        XCTAssertEqual(pressed, [10])
        XCTAssertTrue(p.groupedLists.draws(10) && p.groupedLists.draws(13), "a row and its toggle's control")
        // Scrolled away, a row is refused as a finger would miss it.
        l.collection.contentInset.bottom = 2000
        l.collection.setContentOffset(CGPoint(x: 0, y: 1500), animated: false)
        l.collection.layoutIfNeeded()
        XCTAssertNotNil(p.groupedLists.activate(try XCTUnwrap(p.views[10]))?["error"])
        XCTAssertEqual(pressed, [10])
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
        s.setOn(false, animated: false)
        s.sendActions(for: .valueChanged)
        XCTAssertEqual(flips.map(\.0), [13])
        XCTAssertEqual(flips.map(\.1), [false])
        XCTAssertTrue(s.isOn, "the committed state is authoritative: nothing committed it")
        // The agent's tap on the control flips the switch the cell shows.
        XCTAssertEqual(p.groupedLists.activate(try XCTUnwrap(p.views[13]))?["native"] as? String, "grouped-list")
        XCTAssertEqual(flips.map(\.1), [false, false])
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
        XCTAssertNotNil(p.groupedLists.activate(try XCTUnwrap(p.views[13]))?["error"])
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
        XCTAssertTrue(p.groupedLists.projects(row))
        p.groupedLists.prepare()
        XCTAssertTrue(row.superview === p.views[5]?.container, "back where the presenter put it")
        XCTAssertEqual(row.frame, CGRect(x: 16, y: 52, width: 354, height: 61))
        XCTAssertFalse(p.groupedLists.projects(row))
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
        p.groupedLists.prepare()
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
        XCTAssertNotNil(p.groupedLists.activate(try XCTUnwrap(p.views[20]))?["error"])
        XCTAssertFalse(try cell(p, 20).isUserInteractionEnabled, "an inert section's cell takes no touch")
        XCTAssertTrue(try cell(p, 20).accessibilityElementsHidden)
        p.apply(wireBatch([["op": "props", "id": 3, "set": [String: String](), "clear": ["inert"]]]))
        XCTAssertNotNil(p.groupedLists.activate(try XCTUnwrap(p.views[20]))?["tapped"])
        XCTAssertEqual(pressed, [20])
    }

    func testACarriedCustomRowKeepsItsSectionsInertness() throws {
        let p = presenter { self.model(custom: true) }
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["inert": "true"], "clear": [String]()]]))
        let custom = try cell(p, 21)
        XCTAssertTrue(p.views[21]?.superview === custom.contentView, "carried")
        XCTAssertFalse(custom.isUserInteractionEnabled, "the section is inert though the cell is not its ancestor")
        XCTAssertTrue(custom.accessibilityElementsHidden)
        XCTAssertTrue(p.groupedLists.scroller(for: 21) === (try list(p)).collection, "a custom row's wheel scrolls its list")
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
        XCTAssertFalse(p.groupedLists.draws(21), "the ordinary tap path finds a custom row's views")
        custom = false; header = "Profile"
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "row21"], "clear": [String]()]]))
        XCTAssertTrue(row.superview === p.views[5]?.container, "standard again: given back, and not carried")
        XCTAssertNotNil(try cell(p, 21).contentConfiguration as? UIListContentConfiguration)
        let l = try list(p)
        l.collection.layoutIfNeeded()
        let shown = l.collection.supplementaryView(forElementKind: UICollectionView.elementKindSectionHeader, at: IndexPath(item: 0, section: 0)) as? UICollectionViewListCell
        XCTAssertEqual((shown?.contentConfiguration as? UIListContentConfiguration)?.text, "Profile")
    }

    func testAWheelOnARowScrollsTheListUIKitDraws() throws {
        let p = presenter { self.model() }
        let l = try list(p)
        l.collection.contentInset.bottom = 2000
        l.collection.layoutIfNeeded()
        let scroller = try XCTUnwrap(p.groupedLists.scroller(for: 21))
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

    func testABatchUpdatesTheRowsAndAGoneListGoes() throws {
        var dark = false
        let p = presenter { self.model(dark: dark) }
        XCTAssertTrue(try cell(p, 20).accessories.isEmpty)
        dark = true
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["testId": "row20"], "clear": [String]()]]))
        XCTAssertEqual(try cell(p, 20).accessories.count, 1, "the checkmark")
        p.apply(wireBatch([["op": "roots", "ids": []], ["op": "destroy", "id": 1]]))
        XCTAssertNil(p.groupedLists.lists[1])
    }
}
#endif
