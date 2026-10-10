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

    private func nativeButtonsPresenter(extraRows: Int = 0, intrinsic: (([(UInt32, CGSize?)]) -> Void)? = nil) -> Presenter {
        ExactGroupedLists.install()
        let p = Presenter()
        p.onIntrinsic = intrinsic
        host(p).model = { id in
            guard id == 1 else { return nil }
            return GroupedListModel(sections: [
                .init(view: 2, header: nil, footer: nil, rows: [Row(view: 10, custom: true), Row(view: 11, custom: true)]
                    + (0..<extraRows).map { Row(view: UInt32(30 + $0), custom: true) }),
            ])
        }
        p.buttonFace = { id in
            var face = ButtonFace()
            face.title = id == 10 ? "Direct" : id == 12 ? "Nested" : "Standalone"
            return face
        }
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        let create = { (id: Int, kind: String, props: [String: String]) -> [String: Any] in
            var style: [String: Any] = ["text_color": [0, 0, 0, 255]]
            if kind == "control" { style["appearance"] = "auto" }
            return ["op": "create", "id": id, "kind": kind, "props": props,
                    "handlers": kind == "control" ? ["press"] : [], "style": style]
        }
        let frame = { (id: Int, x: Double, y: Double, w: Double, h: Double) -> [String: Any] in
            ["op": "frame", "id": id, "x": x, "y": y, "w": w, "h": h]
        }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "list", "props": ["listStyle": "inset-grouped"], "style": ["overflow_y": "scroll"]],
            create(2, "view", ["semanticTag": "section"]), create(3, "view", [:]),
            create(10, "control", ["type": "button", "accessibilityRole": "button", "groupedRowSeparator": "true"]),
            create(11, "view", [:]),
            create(12, "control", ["type": "button", "accessibilityRole": "button"]),
            create(20, "control", ["type": "button", "accessibilityRole": "button"]),
            ["op": "children", "id": 1, "ids": [2]], ["op": "children", "id": 2, "ids": [3]],
            ["op": "children", "id": 11, "ids": [12]],
            ["op": "roots", "ids": [1, 20]],
            frame(1, 0, 0, 402, 874), frame(2, 0, 0, 402, Double(104 + extraRows * 52)),
            frame(3, 16, 0, 370, Double(104 + extraRows * 52)),
            frame(10, 0, 0, 370, 52), frame(11, 0, 52, 370, 52),
            frame(12, 16, 8, 120, 34), frame(20, 0, 740, 120, 34),
        ]
        for index in 0..<extraRows {
            ops.append(create(30 + index, "view", [:]))
            ops.append(frame(30 + index, 0, Double(104 + index * 52), 370, 52))
        }
        ops.append(["op": "children", "id": 3, "ids": [10, 11] + (0..<extraRows).map { 30 + $0 }])
        p.apply(wireBatch(ops))
        return p
    }

    private func groupedButtonRect(_ bounds: CGRect) -> CGRect {
        CGRect(x: bounds.minX + 16, y: bounds.minY, width: max(0, bounds.width - 16), height: bounds.height)
    }

    private func assertAlignmentRect(_ button: NativeButtonIOS, _ expected: CGRect,
                                     file: StaticString = #filePath, line: UInt = #line) {
        let actual = button.alignmentRect(forFrame: button.frame)
        XCTAssertEqual(actual.minX, expected.minX, accuracy: 0.001, file: file, line: line)
        XCTAssertEqual(actual.minY, expected.minY, accuracy: 0.001, file: file, line: line)
        XCTAssertEqual(actual.width, expected.width, accuracy: 0.001, file: file, line: line)
        XCTAssertEqual(actual.height, expected.height, accuracy: 0.001, file: file, line: line)
    }

    func testNativeButtonInsetFollowsGroupedMembershipThroughProjection() throws {
        let p = nativeButtonsPresenter()
        let row = try XCTUnwrap(p.views[10])
        let native = try XCTUnwrap(p.controls.controls[10] as? NativeButtonIOS)
        let nested = try XCTUnwrap(p.controls.controls[12] as? NativeButtonIOS)
        let standalone = try XCTUnwrap(p.controls.controls[20] as? NativeButtonIOS)
        let projected = try cell(p, 10)
        XCTAssertTrue(row.superview === projected.contentView)
        XCTAssertEqual(row.frame, CGRect(x: 0, y: 0, width: 370, height: 52), "the authored row remains the full cell slot")
        XCTAssertEqual(projected.bounds.height, 52, accuracy: 0.5)
        assertAlignmentRect(native, groupedButtonRect(row.bounds))
        assertAlignmentRect(nested, try XCTUnwrap(p.views[12]).bounds)
        assertAlignmentRect(standalone, try XCTUnwrap(p.views[20]).bounds)
        XCTAssertEqual(try XCTUnwrap(p.views[11]).frame.size, CGSize(width: 370, height: 52))

        // Cell chrome changes independently of the semantic 16-point slot inset.
        projected.contentView.layoutMargins = UIEdgeInsets(top: 5, left: 22, bottom: 7, right: 26)
        projected.setNeedsLayout(); projected.layoutIfNeeded()
        host(p).sync(changed: []); p.controls.sync()
        assertAlignmentRect(native, groupedButtonRect(row.bounds))
        host(p).prepare()
        XCTAssertTrue(row.superview === p.views[3]?.container)
        p.controls.sync()
        assertAlignmentRect(native, groupedButtonRect(row.bounds))

        p.apply(wireBatch([["op": "props", "id": 10, "set": ["groupedRowSeparator": "false"]]]))
        assertAlignmentRect(native, groupedButtonRect(row.bounds), line: #line)
        p.apply(wireBatch([["op": "props", "id": 10, "clear": ["groupedRowSeparator"]]]))
        assertAlignmentRect(native, row.bounds, line: #line)
    }

    func testProjectedNativeButtonRowsKeepAuthoredSizeAndClampNarrowSlots() throws {
        let p = nativeButtonsPresenter()
        let row = try XCTUnwrap(p.views[10])
        let native = try XCTUnwrap(p.controls.controls[10] as? NativeButtonIOS)
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 250.0, "h": 72.0]]))
        let projected = try cell(p, 10)
        XCTAssertEqual(row.frame.size, CGSize(width: 250, height: 72))
        XCTAssertEqual(projected.bounds.height, 72, accuracy: 0.5)
        assertAlignmentRect(native, CGRect(x: 16, y: 0, width: 234, height: 72))

        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 12.0, "h": 10.0]]))
        _ = try cell(p, 10)
        XCTAssertEqual(row.frame.size, CGSize(width: 12, height: 10), "the projection does not resize the authored slot")
        assertAlignmentRect(native, CGRect(x: 16, y: 0, width: 0, height: 10))
    }

    private func drainProjectionTurns() {
        let done = expectation(description: "projection and trait refresh")
        DispatchQueue.main.async { DispatchQueue.main.async { done.fulfill() } }
        wait(for: [done], timeout: 2)
    }

    func testOffscreenNativeRowKeepsInsetAndNeverPublishesProvisionalIntrinsicSizes() throws {
        var reports: [(UInt32, CGSize?)] = []
        let p = nativeButtonsPresenter(extraRows: 30) { reports.append(contentsOf: $0) }
        let row = try XCTUnwrap(p.views[10])
        let native = try XCTUnwrap(p.controls.controls[10] as? NativeButtonIOS)
        _ = try cell(p, 10)
        let collection = try list(p).collection
        collection.setContentOffset(CGPoint(x: 0, y: 800), animated: false)
        collection.layoutIfNeeded()
        XCTAssertNil(try list(p).cell(10), "the regression requires the native row to be offscreen")
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["testId": "unrelated-change"]]]))
        XCTAssertNil(try list(p).cell(10))
        XCTAssertTrue(row.superview === p.views[3]?.container, "an offscreen row remains in its authored hierarchy")
        assertAlignmentRect(native, groupedButtonRect(row.bounds))
        XCTAssertEqual(row.bounds.height, 52)
        drainProjectionTurns()
        XCTAssertTrue(reports.isEmpty, "native measurements are synchronous; projection publishes no raw or slot sizes")

        let cache = ButtonMeasureCache()
        cache.configure(window.traitCollection, in: p.viewport)
        let face = p.controls.face(10)
        let regular = cache.measure(face, widthKind: 0, width: row.bounds.width - 16, traits: window.traitCollection)
        window.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraExtraLarge
        window.layoutIfNeeded(); native.updateTraitsIfNeeded()
        p.controls.sync(); drainProjectionTurns()
        let large = cache.measure(face, widthKind: 0, width: row.bounds.width - 16, traits: window.traitCollection)
        XCTAssertGreaterThan(large.height, regular.height, "UIKit's real height-for-width answer grows with Dynamic Type")
        assertAlignmentRect(native, groupedButtonRect(row.bounds))
        XCTAssertEqual(row.bounds.height, 52, "the host waits for the kernel's measured frame")
        XCTAssertTrue(reports.isEmpty)

        // Apply the synchronous kernel answer while the row is still offscreen.
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 370.0, "h": Double(large.height)]]))
        XCTAssertNil(try list(p).cell(10))
        assertAlignmentRect(native, CGRect(x: 16, y: 0, width: 354, height: CGFloat(large.height)))
        collection.setContentOffset(.zero, animated: false); collection.layoutIfNeeded()
        let remounted = try cell(p, 10)
        XCTAssertTrue(row.superview === remounted.contentView)
        XCTAssertEqual(row.bounds.height, CGFloat(large.height), accuracy: 0.001)
        assertAlignmentRect(native, groupedButtonRect(row.bounds))
        p.apply(wireBatch([])); drainProjectionTurns()
        XCTAssertTrue(reports.isEmpty, "remounting cannot replace the kernel's answer with raw control dimensions")
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
        XCTAssertEqual(cell.bounds.height, 61, accuracy: 0.5, "the authored border remains inside the carried row")
        XCTAssertTrue(host(p).projects(row))
        host(p).prepare()
        XCTAssertTrue(row.superview === p.views[5]?.container, "back where the presenter put it")
        XCTAssertEqual(row.frame, CGRect(x: 16, y: 52, width: 354, height: 61))
        XCTAssertFalse(host(p).projects(row))
    }

    func testACarriedRowKeepsItsAuthoredBorderAndUsesUIKitSeparator() throws {
        let p = presenter { self.model(custom: true) }
        let row = try XCTUnwrap(p.views[21])
        row.props["groupedRowSeparator"] = "true"
        row.applyStyle(["border_width_bottom": 4, "border_color_bottom": [255, 0, 0, 255]])
        p.apply(wireBatch([["op": "frame", "id": 21, "x": 0.0, "y": 52.0, "w": 370.0, "h": 64.0]]))
        row.applyBoxLayer()
        XCTAssertNil(row.groupedSeparatorRect, "UIKit owns the carried row's system separator")
        XCTAssertEqual(try cell(p, 21).bounds.height, 64, accuracy: 0.5)
        XCTAssertEqual(row.style["border_width_bottom"]?.number, 4)
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
        XCTAssertNil(host(p).lists[1])
    }
}
#endif
