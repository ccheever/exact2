#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

final class NativeListScrollIOSTests: GroupedListFixture {
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
            XCTAssertEqual(measured.height, 80)
            XCTAssertEqual(measured.bounds.height, 80, accuracy: 0.5)
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
            XCTAssertEqual(try XCTUnwrap((try cell(p, 10) as? GroupedCell)?.height), 80)
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
        XCTAssertEqual(try XCTUnwrap((try cell(p, 21) as? GroupedCell)?.height), 600, accuracy: 0.5)
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

}
#endif
