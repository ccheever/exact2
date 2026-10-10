#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// LLP 1084 on UIKit: a grouped list (`listStyle`) is a UICollectionView
/// with a list layout as the list's only scroller;
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

    private func presenter(height: Double = 874, handlers: [String] = [], props: [String: String] = [:],
                           on given: Presenter? = nil, _ m: @escaping () -> GroupedListModel) -> Presenter {
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
        let listProps = ["listStyle": "inset-grouped", "testId": "list"].merging(props) { _, value in value }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "list", "props": listProps, "handlers": handlers, "style": ["overflow_y": "scroll"]],
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

    func testTheCollectionIsTheListsOnlyScrollView() throws {
        let p = presenter { self.model() }
        let l = try list(p)
        XCTAssertTrue(l.collection.superview === p.views[1])
        XCTAssertEqual(l.collection.frame, p.views[1]?.bounds)
        XCTAssertNil(p.views[1]?.scroll, "no duplicate plain UIScrollView")
        XCTAssertTrue(p.views[1]?.scrollView === l.collection)
        XCTAssertTrue(p.views[2]?.isHidden == true && p.views[3]?.isHidden == true, "only the authored section roots are hidden")
        XCTAssertEqual(p.views[1]?.subviews.filter { $0 is UIScrollView }.count, 1)
        XCTAssertEqual(l.collection.numberOfSections, 2)
        XCTAssertEqual(l.collection.numberOfItems(inSection: 0), 3)
        XCTAssertTrue(l.collection.collectionViewLayout is UICollectionViewCompositionalLayout)
        let seen = try XCTUnwrap(host(p).observation(try XCTUnwrap(p.views[1])))
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
        let owner = try XCTUnwrap(p.views[1])
        let at = owner.convert(middle, from: window)
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0],
                           ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0],
                           ["op": "frame", "id": 12, "x": 0.0, "y": Double(at.y - 26), "w": 402.0, "h": 52.0],
                           ["op": "frame", "id": 13, "x": Double(at.x - 31.5), "y": 0.0, "w": 63.0, "h": 52.0]]))
        let authored = try XCTUnwrap(p.controls.controls[13] as? UISwitch)
        XCTAssertTrue(authored !== s && p.views[2]?.isHidden == true)
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
        let p = presenter(height: 200, on: session.presenter) { self.model() }
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
        let p = presenter(on: session.presenter) { self.model(custom: true) }
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

    func testAConfiguredCustomCellCarriesItsRestoredViewsWhenDisplayed() throws {
        let p = presenter { self.model(custom: true) }
        let l = try list(p), row = try XCTUnwrap(p.views[21])
        let prepared = try cell(p, 21)
        let path = try XCTUnwrap(l.collection.indexPath(for: prepared))
        XCTAssertTrue(row.superview === prepared.contentView)
        host(p).prepare()
        XCTAssertTrue(row.superview === p.views[5]?.container, "a batch restores even a prepared cell's views")
        XCTAssertFalse(row.superview === prepared.contentView)
        row.props["testId"] = "displayed-custom"
        // The public delegate callback is also delivered for an already
        // configured prefetched cell, without a cell-registration callback.
        l.collectionView(l.collection, willDisplay: prepared, forItemAt: path)
        XCTAssertTrue(row.superview === prepared.contentView)
        XCTAssertTrue(host(p).projects(row))
        XCTAssertEqual(row.frame, CGRect(x: 16, y: 0, width: 354, height: 61))
        XCTAssertEqual(prepared.accessibilityIdentifier, "displayed-custom")
        XCTAssertEqual((prepared as? GroupedCell)?.height, 60)
        host(p).prepare()
        XCTAssertTrue(row.superview === p.views[5]?.container, "display-time carrying still restores normally")
        p.applyGeometry(wireBatch([["op": "frame", "id": 21, "x": 16.0, "y": 52.0, "w": 354.0, "h": 81.0]]).ops[0])
        XCTAssertEqual((prepared as? GroupedCell)?.height, 60, "the prepared cell still has its older measured height")
        l.collectionView(l.collection, willDisplay: prepared, forItemAt: path)
        XCTAssertTrue(row.superview === prepared.contentView)
        XCTAssertEqual((prepared as? GroupedCell)?.height, 80, "display reads the current authored frame")
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(prepared.bounds.height, 80, accuracy: 0.5, "the existing projection sync settles the height outside the display callback")
    }

    func testUnchangedCustomRowsKeepTheirNativeGeometryAndRefreshLiveProjectionProps() throws {
        let p = presenter(height: 180) { self.model(custom: true) }
        p.apply(wireBatch([["op": "frame", "id": 21, "x": 16.0, "y": 52.0, "w": 354.0, "h": 240.0],
                           ["op": "props", "id": 1, "set": ["scrollTop": "140"]]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let l = try list(p), row = try XCTUnwrap(p.views[21])
        let shown = try XCTUnwrap(try cell(p, 21) as? GroupedCell)
        let offset = l.collection.contentOffset
        let nativeY = shown.convert(shown.bounds, to: l.collection).minY
        let before = shown.convert(shown.bounds, to: window)
        XCTAssertEqual(shown.height, 239)
        for index in 0..<4 {
            p.apply(wireBatch([["op": "props", "id": 21, "set": ["testId": "custom\(index)"], "clear": [String]()]]))
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            let current = try cell(p, 21)
            XCTAssertTrue(current === shown, "the live custom row keeps its native cell")
            XCTAssertTrue(row.superview === shown.contentView)
            XCTAssertEqual(current.accessibilityIdentifier, "custom\(index)")
            XCTAssertEqual(l.collection.contentOffset.y - offset.y,
                           current.convert(current.bounds, to: l.collection).minY - nativeY, accuracy: 0.5,
                           "native estimates may settle above the reader; anchoring counts that shift once")
            XCTAssertEqual(current.convert(current.bounds, to: window).minY, before.minY, accuracy: 0.5)
            XCTAssertEqual(current.bounds.height, before.height, accuracy: 0.5)
        }
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["inert": "true"]]]))
        XCTAssertFalse(shown.isUserInteractionEnabled, "remount reads the current authored section")
        XCTAssertTrue(shown.accessibilityElementsHidden)
        p.apply(wireBatch([["op": "props", "id": 3, "set": [String: String](), "clear": ["inert"]],
                           ["op": "frame", "id": 21, "x": 16.0, "y": 52.0, "w": 354.0, "h": 300.0]]))
        XCTAssertTrue(try cell(p, 21) === shown)
        XCTAssertTrue(shown.isUserInteractionEnabled)
        XCTAssertFalse(shown.accessibilityElementsHidden)
        XCTAssertEqual(shown.height, 299, "mount updates an authored box without changing the custom model")
        XCTAssertEqual(shown.bounds.height, 299, accuracy: 0.5)
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
    /// carry them (LLP 1079's amendment of 2026-10-04): the collection
    /// autoresizes with its owner, then the switch reads its current control.
    func testAnOutOfBatchChangeReachesTheProjectionOnTheNextTurn() throws {
        let p = presenter { self.model() }
        let l = try list(p)
        let toggle = try XCTUnwrap(switches(try cell(p, 12)).first)
        XCTAssertTrue(toggle.isOn)
        let projected = expectation(description: "the queued projection ran")
        UIView.performWithoutAnimation {
            p.views[13]?.props["checked"] = "false"
            p.applyGeometry(wireBatch([["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 600.0]]).ops[0])
            p.requestProjectionSync() // coalesced with the replay's
            XCTAssertEqual(l.collection.frame, p.views[1]?.bounds, "the only scroller autoresizes inline")
            DispatchQueue.main.async { projected.fulfill() }
            wait(for: [projected], timeout: 1)
        }
        XCTAssertEqual(l.collection.frame, CGRect(x: 0, y: 0, width: 300, height: 600))
        XCTAssertTrue(try XCTUnwrap(switches(try cell(p, 12)).first) === toggle)
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

    func testAWheelOnARowScrollsTheListUIKitDraws() throws {
        let p = presenter { self.model() }
        let l = try list(p)
        l.collection.contentInset.bottom = 2000
        l.collection.layoutIfNeeded()
        let scroller = try XCTUnwrap(host(p).scroller(for: 21))
        XCTAssertTrue(scroller === l.collection, "the row's list, not its hidden scroll")
        Agent.scroll(from: scroller, dx: 0, dy: 300)
        XCTAssertEqual(l.collection.contentOffset.y + l.collection.adjustedContentInset.top, 300, accuracy: 0.5)
        XCTAssertNil(p.views[1]?.scroll, "the wheel has no shadow scroll to update")
        XCTAssertTrue(p.views[1]?.scrollView === l.collection)
    }

    func testNativeEstimatedListScrollMatchesProjection() throws {
        let model = model()
        let p = presenter(height: 180, props: ["scrollTop": "75"]) { model }
        let exact = try list(p).collection
        let layout = UICollectionViewCompositionalLayout { index, environment in
            var c = UICollectionLayoutListConfiguration(appearance: .insetGrouped)
            let section = model.sections[index]
            c.headerMode = section.header == nil ? .none : .supplementary
            c.footerMode = section.footer == nil ? .none : .supplementary
            let result = NSCollectionLayoutSection.list(using: c, layoutEnvironment: environment)
            for item in result.boundarySupplementaryItems where item.elementKind == UICollectionView.elementKindSectionFooter {
                item.pinToVisibleBounds = false
            }
            return result
        }
        let native = UICollectionView(frame: CGRect(x: 0, y: 0, width: 402, height: 180), collectionViewLayout: layout)
        native.contentInsetAdjustmentBehavior = .never
        native.alwaysBounceVertical = true
        let rows = Dictionary(uniqueKeysWithValues: model.sections.flatMap { $0.rows }.map { ($0.view, $0) })
        let registration = UICollectionView.CellRegistration<UICollectionViewListCell, UInt32> { cell, _, id in
            guard let row = rows[id] else { return }
            var c: UIListContentConfiguration = row.secondary != nil ? .valueCell() : .cell()
            c.text = row.title
            c.secondaryText = row.secondary
            c.image = row.symbol.flatMap { UIImage(systemName: $0) }
            cell.contentConfiguration = c
            cell.insetsLayoutMarginsFromSafeArea = false
            cell.contentView.insetsLayoutMarginsFromSafeArea = false
            switch row.accessory {
            case "disclosure": cell.accessories = [.disclosureIndicator()]
            case "toggle":
                let toggle = UISwitch()
                toggle.isOn = true
                cell.accessories = [.customView(configuration: .init(customView: toggle, placement: .trailing()))]
            default: cell.accessories = []
            }
        }
        let header = UICollectionView.SupplementaryRegistration<UICollectionViewListCell>(elementKind: UICollectionView.elementKindSectionHeader) { cell, _, path in
            var c = cell.defaultContentConfiguration()
            c.text = model.sections[path.section].header
            cell.contentConfiguration = c
            cell.insetsLayoutMarginsFromSafeArea = false
            cell.contentView.insetsLayoutMarginsFromSafeArea = false
        }
        let footer = UICollectionView.SupplementaryRegistration<UICollectionViewListCell>(elementKind: UICollectionView.elementKindSectionFooter) { cell, _, path in
            var c = cell.defaultContentConfiguration()
            c.text = model.sections[path.section].footer
            cell.contentConfiguration = c
            cell.insetsLayoutMarginsFromSafeArea = false
            cell.contentView.insetsLayoutMarginsFromSafeArea = false
        }
        let source = UICollectionViewDiffableDataSource<UInt32, UInt32>(collectionView: native) { view, path, id in
            view.dequeueConfiguredReusableCell(using: registration, for: path, item: id)
        }
        source.supplementaryViewProvider = { view, kind, path in
            kind == UICollectionView.elementKindSectionHeader
                ? view.dequeueConfiguredReusableSupplementary(using: header, for: path)
                : view.dequeueConfiguredReusableSupplementary(using: footer, for: path)
        }
        var snapshot = NSDiffableDataSourceSnapshot<UInt32, UInt32>()
        for section in model.sections {
            snapshot.appendSections([section.view])
            snapshot.appendItems(section.rows.map(\.view), toSection: section.view)
        }
        source.apply(snapshot, animatingDifferences: false)
        try XCTUnwrap(p.views[1]).addSubview(native)
        native.layoutIfNeeded()
        native.setContentOffset(CGPoint(x: 0, y: 75), animated: false)
        exact.layoutIfNeeded()
        native.layoutIfNeeded()
        XCTAssertEqual(exact.contentOffset.y, 75, accuracy: 0.5)
        XCTAssertEqual(native.contentOffset.y, 75, accuracy: 0.5)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["scrollTop": "100"]]]))
        native.setContentOffset(CGPoint(x: 0, y: 100), animated: false)
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(exact.contentOffset.y, 100, accuracy: 0.5, "list-cell margins do not grow under the status bar")
        XCTAssertEqual(native.contentOffset.y, 100, accuracy: 0.5)
        XCTAssertEqual(exact.contentSize.height, native.contentSize.height, accuracy: 0.5)
        for section in model.sections.indices {
            for item in model.sections[section].rows.indices {
                let path = IndexPath(item: item, section: section)
                let projected = try XCTUnwrap(exact.layoutAttributesForItem(at: path)?.frame)
                let reference = try XCTUnwrap(native.layoutAttributesForItem(at: path)?.frame)
                XCTAssertEqual(projected.minY, reference.minY, accuracy: 0.5)
                XCTAssertEqual(projected.height, reference.height, accuracy: 0.5)
            }
        }
        withExtendedLifetime(source) {}
    }

    func testScrollTopAndEventsUseTheNativeExtent() throws {
        let p = presenter(height: 180, handlers: ["scroll"], props: ["scrollTop": "75"]) { self.model() }
        let owner = try XCTUnwrap(p.views[1]), l = try list(p)
        l.collection.layoutIfNeeded()
        XCTAssertEqual(l.collection.contentOffset.y + owner.scrollTopInset(l.collection), 75, accuracy: 0.5, "the initial write waits for native layout")
        var noted: [(UInt32?, Double)] = [], events: [(UInt32, [Double])] = []
        p.onScrolled = { id, _, top in noted.append((id, top)) }
        p.onScroll = { events.append(($0, $1)) }
        p.apply(wireBatch([
            ["op": "content", "id": 1, "w": 402.0, "h": 9000.0],
            ["op": "props", "id": 1, "set": ["scrollTop": "100"]],
        ]))
        XCTAssertEqual(l.collection.contentOffset.y + owner.scrollTopInset(l.collection), 100, accuracy: 0.5, "the write follows native layout")
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(l.collection.contentOffset.y + owner.scrollTopInset(l.collection), 100, accuracy: 0.5, "an unchanged snapshot cannot reset native estimates after the write")
        XCTAssertLessThan(l.collection.contentSize.height, 9000, "UIKit, rather than the kernel's sheet, owns the extent")
        XCTAssertEqual(noted.last?.0, 1)
        XCTAssertEqual(noted.last?.1 ?? -1, 100, accuracy: 0.5)
        let event = try XCTUnwrap(events.last)
        XCTAssertEqual(event.0, 1)
        XCTAssertEqual(event.1[1], 100, accuracy: 0.5)
        XCTAssertEqual(event.1[3] - event.1[1] - event.1[5],
                       Double(l.collection.contentSize.height + l.collection.adjustedContentInset.bottom - l.collection.bounds.height - l.collection.contentOffset.y), accuracy: 0.5)
        p.apply(wireBatch([]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(l.collection.contentOffset.y + owner.scrollTopInset(l.collection), 100, accuracy: 0.5, "a later remount keeps the authored position")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["scrollTop": "99999"]]]))
        let end = max(-l.collection.adjustedContentInset.top, l.collection.contentSize.height + l.collection.adjustedContentInset.bottom - l.collection.bounds.height)
        XCTAssertEqual(l.collection.contentOffset.y, end, accuracy: 0.5, "writes clamp to the actual native end")
    }

    func testMeasuringALeadingCustomRowKeepsTheNativeListAtItsStart() throws {
        for initial in [0.0, 52.0] {
            let p = presenter(height: 180) {
                var m = self.model()
                m.sections[0].header = nil
                m.sections[0].footer = nil
                m.sections[0].rows[0] = Row(view: 10, custom: true)
                return m
            }
            let owner = try XCTUnwrap(p.views[1]), l = try list(p)
            p.apply(wireBatch([["op": "frame", "id": 10, "x": 16.0, "y": 0.0, "w": 354.0, "h": initial]]))
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            XCTAssertEqual(l.collection.contentOffset.y + owner.scrollTopInset(l.collection), 0, accuracy: 0.5,
                           "the provisional custom row starts at CSS0, initial=\(initial)")
            p.apply(wireBatch([["op": "frame", "id": 10, "x": 16.0, "y": 0.0, "w": 354.0, "h": 80.0]]))
            XCTAssertEqual(l.collection.contentOffset.y + owner.scrollTopInset(l.collection), 0, accuracy: 0.5,
                           "measuring the first row does not move the list away from its start")
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            XCTAssertEqual(l.collection.contentOffset.y + owner.scrollTopInset(l.collection), 0, accuracy: 0.5)
            let measured = try XCTUnwrap(try cell(p, 10) as? GroupedCell)
            XCTAssertEqual(measured.height, 79)
            XCTAssertEqual(measured.bounds.height, 79, accuracy: 0.5)
            XCTAssertTrue(p.views[10]?.superview === measured.contentView)
        }
    }

    func testFractionalNativeInsetsDoNotTurnTheStartIntoAReadingAnchor() throws {
        for follow in [false, true] {
            let p = presenter(height: 180, props: follow ? ["scrollFollowEnd": "true"] : [:]) {
                var m = self.model()
                m.sections[0].header = nil
                m.sections[0].footer = nil
                m.sections[0].rows[0] = Row(view: 10, custom: true)
                return m
            }
            let l = try list(p)
            p.apply(wireBatch([["op": "frame", "id": 10, "x": 16.0, "y": 0.0, "w": 354.0, "h": 0.0]]))
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            l.collection.contentInset.top = 57.666668
            let scale = window.screen.scale
            let aligned = floor(l.collection.adjustedContentInset.top * scale) / scale
            l.collection.setContentOffset(CGPoint(x: 0, y: -aligned), animated: false)
            l.collection.layoutIfNeeded()
            let rounding = l.collection.contentOffset.y + l.collection.adjustedContentInset.top
            XCTAssertGreaterThan(rounding, 0, "pixel alignment leaves a positive fraction at the native start")
            XCTAssertLessThan(rounding, 0.5)
            p.apply(wireBatch([["op": "frame", "id": 10, "x": 16.0, "y": 0.0, "w": 354.0, "h": 80.0]]))
            XCTAssertEqual(l.collection.contentOffset.y + l.collection.adjustedContentInset.top, 0, accuracy: 0.5,
                           "measuring a leading row keeps the native start, follow=\(follow)")
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            XCTAssertEqual(l.collection.contentOffset.y + l.collection.adjustedContentInset.top, 0, accuracy: 0.5)
            p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "padding_top": 12.0]],
                               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 402.0, "h": 240.0]]))
            XCTAssertEqual(l.collection.bounds.height, 240)
            XCTAssertEqual(l.collection.contentOffset.y + l.collection.adjustedContentInset.top, 0, accuracy: 0.5,
                           "a changed native inset uses the current start, follow=\(follow)")
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            XCTAssertEqual(l.collection.contentOffset.y + l.collection.adjustedContentInset.top, 0, accuracy: 0.5)
            XCTAssertEqual(try XCTUnwrap((try cell(p, 10) as? GroupedCell)?.height), 79)
        }
    }

    func testPrependingAndResizingKeepTheVisibleNativeRowInPlace() throws {
        for follow in [false, true] {
            var model = model()
            let p = presenter(height: 180, props: follow ? ["scrollFollowEnd": "true"] : [:]) { model }
            let ids: [UInt32] = [10, 11, 12, 20, 21]
            model = GroupedListModel(sections: [.init(view: 3, header: nil, footer: nil, rows: ids.map { Row(view: $0, custom: true) })])
            var setup: [[String: Any]] = [["op": "children", "id": 4, "ids": [Int]()],
                                         ["op": "children", "id": 5, "ids": ids.map(Int.init)]]
            for (index, id) in ids.enumerated() {
                setup.append(["op": "frame", "id": id, "x": 16.0, "y": Double(index * 80), "w": 354.0, "h": 80.0])
            }
            p.apply(wireBatch(setup))
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            p.apply(wireBatch([["op": "props", "id": 1, "set": ["scrollTop": "110"]]]))
            let l = try list(p)
            func screenY() throws -> CGFloat {
                let row = try cell(p, 12)
                return row.convert(row.bounds, to: l.collection).minY - l.collection.contentOffset.y
            }
            let before = try screenY()
            XCTAssertGreaterThan(before, 0, "the surviving row is fully visible away from the end")
            model.sections[0].rows.insert(Row(view: 9, custom: true), at: 0)
            p.apply(wireBatch([
                ["op": "create", "id": 9, "kind": "view", "style": ["border_width_bottom": 1.0]],
                ["op": "children", "id": 5, "ids": ([9] + ids.map(Int.init))],
                ["op": "frame", "id": 9, "x": 16.0, "y": 0.0, "w": 354.0, "h": 80.0],
            ]))
            XCTAssertEqual(try screenY(), before, accuracy: 0.5, "prepend preserves the native reading position, follow=\(follow)")
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            XCTAssertEqual(try screenY(), before, accuracy: 0.5, "prepend stays settled, follow=\(follow)")
            let preferred = try XCTUnwrap(host(p).scrollAnchors(for: 1).first)
            let port = l.collection.bounds.inset(by: try XCTUnwrap(p.views[1]).scrollPortInsets(l.collection))
            XCTAssertTrue(port.contains(try XCTUnwrap(host(p).projectedRect(for: preferred.node, in: l.collection))),
                          "the preferred reading anchor is a whole native row")
            p.apply(wireBatch([["op": "frame", "id": 10, "x": 16.0, "y": 80.0, "w": 354.0, "h": 140.0]]))
            XCTAssertEqual(try screenY(), before, accuracy: 0.5, "a native height correction is counted once, follow=\(follow)")
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            XCTAssertEqual(try screenY(), before, accuracy: 0.5, "resize stays settled, follow=\(follow)")
        }
    }

    func testAppendingToTheNativeListFollowsItsActualEnd() throws {
        var model = model()
        let p = presenter(height: 180, props: ["scrollFollowEnd": "true"]) { model }
        let ids: [UInt32] = [10, 11, 12, 20, 21]
        model = GroupedListModel(sections: [.init(view: 3, header: nil, footer: nil, rows: ids.map { Row(view: $0, custom: true) })])
        var setup: [[String: Any]] = [["op": "children", "id": 4, "ids": [Int]()],
                                     ["op": "children", "id": 5, "ids": ids.map(Int.init)]]
        for (index, id) in ids.enumerated() {
            setup.append(["op": "frame", "id": id, "x": 16.0, "y": Double(index * 80), "w": 354.0, "h": 80.0])
        }
        p.apply(wireBatch(setup))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["scrollTop": "99999"]]]))
        let l = try list(p)
        func end() -> CGFloat {
            max(-l.collection.adjustedContentInset.top,
                l.collection.contentSize.height + l.collection.adjustedContentInset.bottom - l.collection.bounds.height)
        }
        let before = end()
        XCTAssertEqual(l.collection.contentOffset.y, before, accuracy: 0.5)
        model.sections[0].rows.append(Row(view: 22, custom: true))
        p.apply(wireBatch([
            ["op": "create", "id": 22, "kind": "view", "style": ["border_width_bottom": 1.0]],
            ["op": "children", "id": 5, "ids": ids.map(Int.init) + [22]],
            ["op": "frame", "id": 22, "x": 16.0, "y": 400.0, "w": 354.0, "h": 80.0],
        ]))
        XCTAssertGreaterThan(end(), before)
        XCTAssertEqual(l.collection.contentOffset.y, end(), accuracy: 0.5, "append follows the collection's extent")
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(l.collection.contentOffset.y, end(), accuracy: 0.5, "the actual end remains followed after native layout")
    }

    func testPaddingIsContentRoomInLogicalScrollMetrics() throws {
        let p = presenter(height: 180, handlers: ["scroll"]) { self.model() }
        let owner = try XCTUnwrap(p.views[1]), l = try list(p)
        var events: [[Double]] = [], facts: [(UInt32?, Double)] = []
        p.onScroll = { _, values in events.append(values) }
        p.onScrolled = { id, _, top in facts.append((id, top)) }
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["scrollTop": "1"]]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["scrollTop": "0"]]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let before = try XCTUnwrap(events.last)
        XCTAssertEqual(before[1], 0, accuracy: 0.5)
        XCTAssertEqual(before[5], Double(l.collection.bounds.height), accuracy: 0.5)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "padding_top": 12.0, "padding_bottom": 83.0]]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let padded = try XCTUnwrap(events.last)
        XCTAssertEqual(l.collection.contentOffset.y + owner.scrollTopInset(l.collection), 0, accuracy: 0.5)
        XCTAssertEqual(padded[1], 0, accuracy: 0.5, "padding keeps the CSS start at zero")
        XCTAssertEqual(facts.last?.0, 1)
        XCTAssertEqual(facts.last?.1 ?? -1, 0, accuracy: 0.5, "native layout facts use the same content origin")
        XCTAssertEqual(padded[5], before[5], accuracy: 0.5, "content padding does not reduce clientHeight")
        XCTAssertEqual(padded[3] - before[3], 95, accuracy: 0.5, "both padding edges increase scrollHeight")
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["id": "padded-row"]]]))
        p.scrollElementIntoView(["padded-row", "start", "nearest"])
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let revealed = try cell(p, 20)
        XCTAssertEqual(revealed.convert(revealed.bounds, to: l.collection).minY - l.collection.contentOffset.y,
                       0, accuracy: 0.5, "start aligns to the CSS port, not below content padding")
        XCTAssertEqual(try XCTUnwrap(events.last)[5], before[5], accuracy: 0.5)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["scrollTop": "30"]]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(try XCTUnwrap(events.last)[1], 30, accuracy: 0.5)
        l.collection.setContentOffset(CGPoint(x: 0, y: 100), animated: false)
        XCTAssertEqual(facts.last?.0, 1)
        XCTAssertEqual(facts.last?.1 ?? -1, 112, accuracy: 0.5, "a native offset includes the authored content room in kernel facts")
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(try XCTUnwrap(events.last)[1], 112, accuracy: 0.5)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "padding_top": 6.0, "padding_bottom": 9.0]],
                           ["op": "props", "id": 1, "set": ["scrollTop": "0"]]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let replaced = try XCTUnwrap(events.last)
        XCTAssertEqual(replaced[1], 0, accuracy: 0.5)
        XCTAssertEqual(replaced[5], before[5], accuracy: 0.5)
        XCTAssertEqual(replaced[3] - Double(l.collection.contentSize.height), 15, accuracy: 0.5, "changing padding replaces its room while native rows keep their current sizes")
        p.apply(wireBatch([]))
        XCTAssertEqual(l.collection.contentInset.top, 6, accuracy: 0.5, "remounting does not add the padding twice")
        XCTAssertEqual(l.collection.contentInset.bottom, 9, accuracy: 0.5)
    }

    func testChangingTopPaddingKeepsTheNativeReaderAndPublishesItsNewLogicalOffset() throws {
        let p = presenter(height: 180, handlers: ["scroll"]) { self.model() }
        let owner = try XCTUnwrap(p.views[1]), l = try list(p)
        var facts: [Double] = [], events: [[Double]] = []
        p.onScrolled = { id, _, top in if id == 1 { facts.append(top) } }
        p.onScroll = { _, values in events.append(values) }
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "padding_top": 12.0]],
                           ["op": "props", "id": 1, "set": ["scrollTop": "140"]]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let row = try XCTUnwrap(p.views[20])
        let shown = try cell(p, 20)
        let before = shown.convert(shown.bounds, to: window).minY
        XCTAssertEqual(l.collection.contentOffset.y + owner.scrollTopInset(l.collection), 140, accuracy: 0.5)
        XCTAssertEqual(try XCTUnwrap(facts.last), 140, accuracy: 0.5)
        XCTAssertEqual(try XCTUnwrap(events.last)[1], 140, accuracy: 0.5)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "padding_top": 24.0]]]))
        XCTAssertTrue(owner.scrollView === l.collection, "padding changes keep the same physical backend")
        XCTAssertEqual(try cell(p, 20).convert(shown.bounds, to: window).minY, before, accuracy: 0.5)
        XCTAssertEqual(l.collection.contentOffset.y + owner.scrollTopInset(l.collection), 152, accuracy: 0.5,
                       "anchoring counts the extra12 points of content room in CSS scrollTop")
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let settled = try cell(p, 20)
        XCTAssertEqual(settled.convert(settled.bounds, to: window).minY, before, accuracy: 0.5)
        let logicalTop = try XCTUnwrap(facts.last)
        XCTAssertEqual(logicalTop, 152, accuracy: 0.5, "kernel facts reflect the new content origin")
        XCTAssertEqual(try XCTUnwrap(events.last)[1], logicalTop, accuracy: 0.5)
        let projected = try XCTUnwrap(host(p).projectedRect(for: row, in: l.collection))
        let ownerY = owner.convert(owner.bounds, to: window).minY
        XCTAssertEqual(projected.minY + 24 - CGFloat(logicalTop) + ownerY, before, accuracy: 0.5,
                       "the native reading position agrees with the CSS content coordinate")
    }

    func testASameBatchResizeAndInsertionPrecedeTheScrollWrite() throws {
        var inserted = false
        let p = presenter(height: 180) {
            var rows = [Row(view: 21, custom: true)]
            if inserted { rows.append(Row(view: 22, title: "New", pressable: true)) }
            return GroupedListModel(sections: [.init(view: 3, header: nil, footer: nil, rows: rows)])
        }
        let l = try list(p)
        _ = try cell(p, 21)
        inserted = true
        p.apply(wireBatch([
            ["op": "create", "id": 22, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 5, "ids": [20, 21, 22]],
            ["op": "frame", "id": 21, "x": 16.0, "y": 52.0, "w": 354.0, "h": 600.0],
            ["op": "frame", "id": 22, "x": 16.0, "y": 652.0, "w": 354.0, "h": 52.0],
            ["op": "props", "id": 1, "set": ["scrollTop": "250"]],
        ]))
        XCTAssertEqual(l.collection.numberOfItems(inSection: 0), 2)
        XCTAssertEqual(l.collection.contentOffset.y, 250, accuracy: 0.5, "the write uses the resized native extent in the same batch")
        XCTAssertEqual(try XCTUnwrap((try cell(p, 21) as? GroupedCell)?.height), 599, accuracy: 0.5)
    }

    func testAResizedPortSettlesBeforeItsScrollWrite() throws {
        let p = presenter { self.model() }
        let l = try list(p)
        p.apply(wireBatch([
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 240.0, "h": 180.0],
            ["op": "props", "id": 1, "set": ["scrollTop": "99999"]],
        ]))
        l.collection.layoutIfNeeded()
        XCTAssertEqual(l.collection.bounds.size, CGSize(width: 240, height: 180))
        let end = max(-l.collection.adjustedContentInset.top, l.collection.contentSize.height + l.collection.adjustedContentInset.bottom - l.collection.bounds.height)
        XCTAssertGreaterThan(end, 0)
        XCTAssertEqual(l.collection.contentOffset.y, end, accuracy: 0.5)
    }

    func testChangingTheListProjectionRestoresAndReusesItsLogicalOwner() throws {
        let p = presenter { self.model(custom: true) }
        let owner = try XCTUnwrap(p.views[1]), original = try list(p)
        p.apply(wireBatch([["op": "props", "id": 1, "set": [String: String](), "clear": ["listStyle"]]]))
        let plain = try XCTUnwrap(owner.scroll)
        XCTAssertTrue(owner.scrollView === plain)
        XCTAssertNil(original.collection.superview)
        XCTAssertNil(host(p).lists[1])
        XCTAssertFalse(try XCTUnwrap(p.views[2]).hiddenByHost)
        XCTAssertFalse(try XCTUnwrap(p.views[3]).hiddenByHost)
        XCTAssertTrue(p.views[2]?.superview === plain)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["listStyle": "inset-grouped"]]]))
        let native = try list(p)
        XCTAssertTrue(p.views[1] === owner)
        XCTAssertNil(owner.scroll)
        XCTAssertTrue(owner.scrollView === native.collection)
        XCTAssertNil(plain.superview)
        XCTAssertEqual(owner.subviews.filter { $0 is UIScrollView }.count, 1)
        XCTAssertTrue(try XCTUnwrap(p.views[21]).superview === (try cell(p, 21)).contentView)
    }

    func testChangingTheScrollBackendPreservesItsPositionAndExplicitWritesWin() throws {
        let p = presenter(height: 180) { self.model() }
        let owner = try XCTUnwrap(p.views[1]), first = try list(p)
        p.apply(wireBatch([["op": "content", "id": 1, "w": 402.0, "h": 9000.0],
                           ["op": "props", "id": 1, "set": ["scrollTop": "140"]]]))
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(first.collection.contentOffset.y + owner.scrollTopInset(first.collection), 140, accuracy: 0.5)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "scroll_behavior": "smooth"]],
                           ["op": "props", "id": 1, "set": [String: String](), "clear": ["listStyle"]]]))
        let plain = try XCTUnwrap(owner.scroll)
        XCTAssertEqual(plain.contentSize.height, 9000, accuracy: 0.5)
        XCTAssertEqual(plain.contentOffset.y, 140, accuracy: 0.5, "the backend handoff is immediate even with smooth scrolling")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["listStyle": "inset-grouped"]]]))
        let second = try list(p)
        XCTAssertNil(owner.scroll)
        XCTAssertEqual(second.collection.contentOffset.y + owner.scrollTopInset(second.collection), 140, accuracy: 0.5)
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        XCTAssertEqual(second.collection.contentOffset.y + owner.scrollTopInset(second.collection), 140, accuracy: 0.5)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "scroll_behavior": "auto"]],
                           ["op": "props", "id": 1, "set": ["scrollTop": "35"], "clear": ["listStyle"]]]))
        XCTAssertEqual(try XCTUnwrap(owner.scroll).contentOffset.y, 35, accuracy: 0.5, "an authored write wins over the handoff")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["listStyle": "inset-grouped", "scrollTop": "80"]]]))
        let last = try list(p)
        XCTAssertEqual(last.collection.contentOffset.y + owner.scrollTopInset(last.collection), 80, accuracy: 0.5)
        XCTAssertEqual(owner.subviews.filter { $0 is UIScrollView }.count, 1)
    }

    func testAHiddenBackendReplacementKeepsTheLogicalPaddedPosition() throws {
        for (native, together) in [(true, false), (false, false), (true, true), (false, true)] {
            let p = presenter(height: 180) { self.model() }
            let owner = try XCTUnwrap(p.views[1])
            var setup: [[String: Any]] = [
                ["op": "style", "id": 1, "style": ["overflow_y": "scroll", "padding_top": 24.0]],
                ["op": "content", "id": 1, "w": 402.0, "h": 9000.0],
                ["op": "props", "id": 1, "set": ["scrollTop": "140"]],
            ]
            if !native { setup.append(["op": "props", "id": 1, "set": [String: String](), "clear": ["listStyle"]]) }
            p.apply(wireBatch(setup))
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            let original = try XCTUnwrap(owner.scrollView)
            XCTAssertEqual(original.contentOffset.y + owner.scrollTopInset(original), 140, accuracy: 0.5)
            let hide: [String: Any] = ["op": "style", "id": 1, "style": ["overflow_y": "scroll", "padding_top": 24.0, "display": "none"]]
            let replace: [String: Any] = native
                ? ["op": "props", "id": 1, "set": [String: String](), "clear": ["listStyle"]]
                : ["op": "props", "id": 1, "set": ["listStyle": "inset-grouped"]]
            if together { p.apply(wireBatch([hide, replace])) }
            else { p.apply(wireBatch([hide])); p.apply(wireBatch([replace])) }
            XCTAssertFalse(owner.scrollView === original)
            p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "padding_top": 24.0, "display": "block"]]]))
            let restored = try XCTUnwrap(owner.scrollView)
            XCTAssertEqual(restored.contentOffset.y + owner.scrollTopInset(restored), 140, accuracy: 0.5,
                           "hidden positions cross backend inset bases, native=\(native), together=\(together)")
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
            XCTAssertEqual(restored.contentOffset.y + owner.scrollTopInset(restored), 140, accuracy: 0.5)
        }
    }

    func testRevealingACarriedRowDoesNotReserveContentPaddingForAKeyboard() throws {
        let p = presenter(height: 180) { self.model(custom: true) }
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_y": "scroll", "padding_bottom": 83.0]],
                           ["op": "props", "id": 1, "set": ["scrollTop": "99999"]]]))
        let l = try list(p), row = try XCTUnwrap(p.views[21])
        _ = try cell(p, 21)
        let box = row.convert(row.bounds, to: l.collection)
        l.collection.setContentOffset(CGPoint(x: 0, y: box.midY - l.collection.bounds.height / 2), animated: false)
        l.collection.layoutIfNeeded()
        let before = l.collection.contentOffset
        let centered = row.convert(row.bounds, to: l.collection)
        XCTAssertGreaterThan(centered.minY - before.y, 8)
        XCTAssertLessThan(centered.maxY - before.y + 8, l.collection.bounds.height)
        XCTAssertGreaterThan(centered.maxY - before.y + 8, l.collection.bounds.height - l.collection.adjustedContentInset.bottom,
                             "the former content-inset port would falsely move this row")
        p.reveal(row)
        XCTAssertEqual(l.collection.contentOffset.y, before.y, accuracy: 0.5, "the whole carried row already fits the real port")
    }

    func testRefreshAndKeyboardStyleBelongToTheCollection() throws {
        let p = presenter(handlers: ["refresh"], props: ["keyboardDismissMode": "interactive"]) { self.model() }
        let l = try list(p)
        var refreshed: [UInt32] = []
        p.onRefresh = { refreshed.append($0) }
        let refresh = try XCTUnwrap(l.collection.refreshControl)
        XCTAssertEqual(l.collection.keyboardDismissMode, .interactive)
        let owner = try XCTUnwrap(p.views[1])
        let actions = try XCTUnwrap(refresh.actions(forTarget: owner, forControlEvent: .valueChanged))
        XCTAssertEqual(actions.count, 1)
        // The hostless UIKit test runner cannot dispatch selector actions via
        // sendActions; invoke the real registered action (as the title test does).
        for action in actions { _ = owner.perform(Selector(action)) }
        XCTAssertEqual(refreshed, [1])
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["refreshing": "true", "keyboardDismissMode": "on-drag"]]]))
        refresh.beginRefreshing()
        XCTAssertTrue(refresh.isRefreshing)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["refreshing": "false"]],
                           ["op": "style", "id": 1, "style": ["overflow_y": "scroll", "scrollbar_width": "none"]]]))
        XCTAssertFalse(refresh.isRefreshing)
        XCTAssertTrue(l.collection.refreshControl === refresh, "a batch keeps the same refresh control")
        XCTAssertEqual(l.collection.keyboardDismissMode, .onDrag)
        XCTAssertFalse(l.collection.showsVerticalScrollIndicator)
        XCTAssertNil(p.views[1]?.scroll)
    }

    func testCarriedRowsKeepNestedScrollsAndRestoreAuthoredRoots() throws {
        let p = presenter(height: 400) { self.model(custom: true) }
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "view", "props": ["id": "nested"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 31, "kind": "view", "props": ["id": "nested-content"]],
            ["op": "create", "id": 32, "kind": "view", "props": ["id": "nested-target"]],
            ["op": "children", "id": 21, "ids": [30]], ["op": "children", "id": 30, "ids": [31]],
            ["op": "children", "id": 31, "ids": [32]],
            ["op": "frame", "id": 21, "x": 16.0, "y": 52.0, "w": 354.0, "h": 800.0],
            ["op": "frame", "id": 30, "x": 0.0, "y": 80.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 31, "x": 0.0, "y": 0.0, "w": 200.0, "h": 300.0],
            ["op": "frame", "id": 32, "x": 0.0, "y": 260.0, "w": 100.0, "h": 20.0],
            ["op": "content", "id": 30, "w": 200.0, "h": 300.0],
        ]))
        let l = try list(p), row = try XCTUnwrap(p.views[21]), nested = try XCTUnwrap(p.views[30])
        let scroll = try XCTUnwrap(nested.scroll)
        XCTAssertTrue(nested.scrollView === scroll)
        XCTAssertTrue(row.superview === (try cell(p, 21)).contentView)
        let outer = l.collection.contentOffset
        Agent.scroll(from: scroll, dx: 0, dy: 40)
        XCTAssertEqual(scroll.contentOffset.y, 40, accuracy: 0.5)
        XCTAssertEqual(l.collection.contentOffset, outer)
        var scrolled: [UInt32] = []
        p.onScrolled = { id, _, _ in if let id { scrolled.append(id) } }
        p.scrollElementIntoView(["nested-target", "start", "nearest"])
        XCTAssertEqual(scroll.contentOffset.y, 260, accuracy: 0.5, "the inner port moves first")
        XCTAssertGreaterThan(l.collection.contentOffset.y, outer.y, "the native outer port then reveals the carried target")
        XCTAssertLessThan(try XCTUnwrap(scrolled.firstIndex(of: 30)), try XCTUnwrap(scrolled.firstIndex(of: 1)))
        let section = try XCTUnwrap(p.views[3])
        host(p).prepare()
        XCTAssertFalse(section.hiddenByHost)
        XCTAssertTrue(row.superview === p.views[5]?.container)
        section.isHidden = true
        host(p).sync(changed: [])
        host(p).reset()
        XCTAssertTrue(section.hiddenByHost, "removing the projection preserves a pre-existing host-hidden bit")
        XCTAssertNil(p.views[1]?.scrollView)
        XCTAssertNil(l.collection.superview)
    }

    func testProjectedRowsSupplyNativeGeometryAndReadingAnchors() throws {
        let p = presenter(height: 180) { self.model() }
        let l = try list(p), row = try XCTUnwrap(p.views[21])
        l.collection.layoutIfNeeded()
        let projected = try XCTUnwrap(host(p).projectedRect(for: row, in: l.collection))
        let estimated = try XCTUnwrap(l.collection.layoutAttributesForItem(at: IndexPath(item: 1, section: 1)))
        XCTAssertEqual(projected, estimated.frame, "an offscreen row uses UIKit's current estimate")
        XCTAssertNil(l.cell(21), "the target is offscreen")
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["id": "last-row"]]]))
        p.scrollElementIntoView(["last-row", "start", "nearest"])
        l.collection.layoutIfNeeded()
        XCTAssertGreaterThan(l.collection.contentOffset.y, 0)
        XCTAssertNotNil(l.cell(21), "reveal uses the native row frame, not the hidden authored row")
        let shown = try cell(p, 21)
        let realized = try XCTUnwrap(host(p).projectedRect(for: row, in: l.collection))
        XCTAssertEqual(realized, shown.convert(shown.bounds, to: l.collection), "realization replaces the estimate")
        let anchors = host(p).scrollAnchors(for: 1)
        XCTAssertFalse(anchors.isEmpty)
        XCTAssertTrue(anchors.contains { $0.node === row })
        let anchorPort = l.collection.bounds.inset(by: try XCTUnwrap(p.views[1]).scrollPortInsets(l.collection))
        let whole = anchors.filter { host(p).projectedRect(for: $0.node, in: l.collection).map(anchorPort.contains) == true }
        let partial = anchors.filter { host(p).projectedRect(for: $0.node, in: l.collection).map(anchorPort.contains) != true }
        XCTAssertEqual(anchors.map { $0.node.id }, (whole + partial).map { $0.node.id }, "whole rows precede partial fallbacks")
        XCTAssertEqual(whole.map(\.y), whole.map(\.y).sorted())
        XCTAssertEqual(partial.map(\.y), partial.map(\.y).sorted())
        RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        let settled = try cell(p, 21)
        let port = l.collection.bounds.inset(by: l.collection.adjustedContentInset).insetBy(dx: -0.5, dy: -0.5)
        XCTAssertTrue(port.contains(settled.convert(settled.bounds, to: l.collection)), "the revealed native row stays fully visible after self-sizing")
    }

    func testRevealingAListOwnerMovesItsOuterPortOnly() throws {
        let p = presenter(height: 180) { self.model() }
        let l = try list(p)
        p.apply(wireBatch([
            ["op": "create", "id": 100, "kind": "view", "props": ["id": "outer"], "style": ["overflow_y": "scroll"]],
            ["op": "props", "id": 1, "set": ["id": "grouped-owner"]],
            ["op": "children", "id": 100, "ids": [1]], ["op": "roots", "ids": [100]],
            ["op": "frame", "id": 100, "x": 0.0, "y": 0.0, "w": 402.0, "h": 200.0],
            ["op": "frame", "id": 1, "x": 0.0, "y": 600.0, "w": 402.0, "h": 180.0],
            ["op": "content", "id": 100, "w": 402.0, "h": 1000.0],
        ]))
        let outer = try XCTUnwrap(p.views[100]?.scroll), before = l.collection.contentOffset
        p.scrollElementIntoView(["grouped-owner", "start", "nearest"])
        XCTAssertEqual(outer.contentOffset.y, 600, accuracy: 0.5)
        XCTAssertEqual(l.collection.contentOffset, before, "the list's own port is not an ancestor of its owner")
    }

    func testAnOffscreenCustomTargetScrollsItsInnerPortBeforeRevealingTheRow() throws {
        let p = presenter(height: 180) { self.model(custom: true) }
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "view", "props": ["id": "inner"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 31, "kind": "view", "props": ["id": "inner-content"]],
            ["op": "create", "id": 32, "kind": "view", "props": ["id": "offscreen-target"]],
            ["op": "children", "id": 21, "ids": [30]], ["op": "children", "id": 30, "ids": [31]],
            ["op": "children", "id": 31, "ids": [32]],
            ["op": "frame", "id": 21, "x": 16.0, "y": 52.0, "w": 354.0, "h": 800.0],
            ["op": "frame", "id": 30, "x": 0.0, "y": 0.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 31, "x": 0.0, "y": 0.0, "w": 200.0, "h": 300.0],
            ["op": "frame", "id": 32, "x": 0.0, "y": 260.0, "w": 100.0, "h": 20.0],
            ["op": "content", "id": 30, "w": 200.0, "h": 300.0],
        ]))
        let l = try list(p), inner = try XCTUnwrap(p.views[30]?.scroll)
        XCTAssertNil(l.cell(21), "the custom target starts in the retained authored hierarchy")
        var scrolled: [UInt32] = []
        p.onScrolled = { id, _, _ in if let id { scrolled.append(id) } }
        p.scrollElementIntoView(["offscreen-target", "start", "nearest"])
        l.collection.layoutIfNeeded()
        XCTAssertEqual(inner.contentOffset.y, 260, accuracy: 0.5, "a native row box does not replace the inner target's authored geometry")
        XCTAssertGreaterThan(l.collection.contentOffset.y, 0)
        XCTAssertNotNil(l.cell(21))
        XCTAssertLessThan(try XCTUnwrap(scrolled.firstIndex(of: 30)), try XCTUnwrap(scrolled.firstIndex(of: 1)))
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
        XCTAssertNil(host(p).lists[1])
    }
}
#endif
