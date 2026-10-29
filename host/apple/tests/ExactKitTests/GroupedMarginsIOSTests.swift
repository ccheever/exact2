#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

final class GroupedPaddingIOSTests: GroupedListFixture {
    func testHorizontalPaddingInsetsSectionsAndIsNotReportedAsACardMargin() throws {
        for appearance in ["plain", "grouped", "inset-grouped"] {
            for direction in ["ltr", "rtl"] {
                let p = presenter(style: ["direction": direction]) { self.model(appearance, custom: true) }
                // Without a scene the list layout's environment stays left to
                // right whatever the list's direction.
                guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
                    throw missing("scene", "a right-to-left list lays out right to left only in a scene's window")
                }
                window.isHidden = true
                window = UIWindow(windowScene: scene)
                window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
                window.addSubview(p.viewport)
                window.makeKeyAndVisible()
                defer { window.isHidden = true }
                let l = try list(p), collection = l.collection
                p.apply(wireBatch([contentOp(1, 402, 874, padding: [12, 0, 23, 0])]))
                func frames() throws -> [CGRect] {
                    collection.layoutIfNeeded()
                    return try [IndexPath(item: 0, section: 0), IndexPath(item: 1, section: 1)].map {
                        try XCTUnwrap(collection.layoutAttributesForItem(at: $0)).frame
                    } + [UICollectionView.elementKindSectionHeader, UICollectionView.elementKindSectionFooter].map {
                        try XCTUnwrap(collection.layoutAttributesForSupplementaryElement(ofKind: $0, at: IndexPath(item: 0, section: 0))).frame
                    }
                }
                let original = try frames(), margins = try XCTUnwrap(l.margins())
                let port = collection.frame
                // Other owners can add room for refresh and keyboard avoidance.
                collection.contentInset.top += 31
                collection.contentInset.bottom += 47
                let insets = collection.contentInset
                for (left, right) in [(230.0 / 3, 0.0), (31.0, 17.0), (0.0, 0.0)] {
                    // A resolved padding can change without a style or frame change.
                    p.apply(wireBatch([contentOp(1, 402, 874, padding: [12, right, 23, left])]))
                    let note = "\(appearance), \(direction), padding \(left)/\(right)"
                    let padded = try frames()
                    for (before, after) in zip(original, padded) {
                        XCTAssertEqual(after.minX, before.minX + left, accuracy: 0.5, note)
                        XCTAssertEqual(after.maxX, before.maxX - right, accuracy: 0.5, note)
                    }
                    let reported = try XCTUnwrap(l.margins())
                    for (before, after) in zip(margins, reported) {
                        XCTAssertEqual(after, before, accuracy: 0.5, "CSS padding is not a native margin: \(note)")
                    }
                    XCTAssertEqual(collection.frame, port, "padding leaves the scroll port intact")
                    XCTAssertEqual(collection.contentInset, insets, "horizontal padding preserves the other inset owners")
                    p.apply(wireBatch([]))
                    XCTAssertEqual(try frames(), padded, "a remount keeps the section geometry")
                }
            }
        }
    }

    /// A plain list's header is measured at the width it is drawn at, its padding taken off.
    func testAPlainHeaderWrappedByThePaddingIsAsTallAsItsText() throws {
        let heading = "Announcements, events and plans for the season ahead"
        let p = presenter { GroupedListModel(style: "plain", sections: [.init(view: 2, header: heading, footer: nil, rows: [Row(view: 10, title: "Profile", pressable: true)])]) }
        defer { window.isHidden = true }
        let collection = try list(p).collection, path = IndexPath(item: 0, section: 0)
        func header() throws -> (frame: CGRect, needs: CGFloat) {
            collection.layoutIfNeeded()
            let view = try XCTUnwrap(collection.supplementaryView(forElementKind: UICollectionView.elementKindSectionHeader, at: path))
            let needs = view.systemLayoutSizeFitting(CGSize(width: view.bounds.width, height: 0), withHorizontalFittingPriority: .required,
                                                     verticalFittingPriority: .fittingSizeLevel).height
            func labels(_ v: UIView) -> [UILabel] { (v as? UILabel).map { [$0] } ?? v.subviews.flatMap(labels) }
            let label = labels(view).first
            XCTAssertGreaterThan(label?.bounds.width ?? 0, view.bounds.width / 2, "the heading's text takes the header's width")
            return (view.frame, needs)
        }
        p.apply(wireBatch([contentOp(1, 402, 874, padding: [0, 0, 0, 0])]))
        let full = try header()
        p.apply(wireBatch([contentOp(1, 402, 874, padding: [0, 0, 0, 200])]))
        let padded = try header()
        XCTAssertEqual(padded.frame.minX, 200, accuracy: 0.5)
        XCTAssertEqual(padded.frame.minY, full.frame.minY, accuracy: 0.5, "the padding moves the heading sideways only")
        XCTAssertGreaterThan(padded.frame.height, full.frame.height + 1, "the heading wraps to more lines in the padded width")
        XCTAssertGreaterThanOrEqual(padded.frame.height + 0.5, padded.needs, "and its frame holds them")
        let row = try XCTUnwrap(collection.layoutAttributesForItem(at: path)).frame
        XCTAssertGreaterThanOrEqual(row.minY + 0.5, padded.frame.maxY, "the row starts under the heading")
    }
}

/// A grouped list's custom row starts where a standard row's text does: at
/// its cell's layout margin, which UIKit widens from 16 to 20 points on a
/// wide screen (the iPhone Duo's inner panel, an iPhone in landscape); on
/// the Duo's simulator it is 20 at both widths used here. The host tells
/// the kernel the margins UIKit chose, again when they change, and the
/// kernel lays the custom row out by them.
final class GroupedMarginsIOSTests: XCTestCase {
    func testACustomRowStartsAtItsCellsMarginAsTheWidthChanges() throws {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_GROUPED_REVEAL_PLAN"])
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "grouped-margins")
        defer { session.destroy() }
        let view = ExactView(session: session)
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "the list's margins need a hosted window scene")
        }
        // As wide as the Duo's inner panel, where UIKit's list margins are 20.
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 951, height: 669)
        let root = UIViewController()
        window.rootViewController = root
        window.makeKeyAndVisible()
        root.view.addSubview(view)
        view.frame = root.view.bounds
        defer { window.isHidden = true }
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: view.bounds.size).error)
        let p = session.presenter, agent = Agent(session: session)
        func settle() {
            window.layoutIfNeeded()
            let reply = agent.clock(["settle": true])
            XCTAssertEqual(reply["settled"] as? Bool, true, "\(reply)")
        }
        func tap(_ name: String) throws {
            let node = try XCTUnwrap(p.views.values.first { $0.props["testId"] == name }, name)
            XCTAssertNil(agent.reveal(["id": Int(node.id)])["error"])
            XCTAssertNil(agent.tap(["id": Int(node.id)])["error"])
            settle()
        }
        settle()
        // The fixture's custom row, a row holding a button, comes with the
        // section its sheet adds.
        try tap("add")
        try tap("save")
        XCTAssertNil(agent.clock(["data": true])["error"])
        settle()
        let host = try XCTUnwrap(p.groupedLists as? GroupedListHost)
        let button = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "remove" })
        XCTAssertNil(agent.reveal(["id": Int(button.id)])["error"])
        settle()
        let list = try XCTUnwrap(host.lists.values.first(where: \.carriesCustomRows))
        // Where UIKit's own cell configurations start their text (`UIListContentConfiguration`), and the custom row's content.
        func starts() throws -> (margin: CGFloat, row: CGFloat) {
            let cell = try XCTUnwrap(list.collection.visibleCells.first { button.isDescendant(of: $0) } as? UICollectionViewListCell,
                                     "the custom row is carried into its cell")
            return (cell.directionalLayoutMargins.leading, button.convert(button.bounds, to: cell).minX)
        }
        let wide = try starts()
        XCTAssertEqual(wide.margin, 20, "UIKit's cell margin on a screen this wide; the test needs one other than the sheet's 16")
        XCTAssertEqual(host.told[list.owner.id]?[2], wide.margin, "the kernel was told UIKit's margin")
        XCTAssertEqual(wide.row, wide.margin, accuracy: 0.5, "the custom row's content starts at the margin")
        // An iPhone's width, as a fold or a turn changes it, while a
        // collection's slice is in flight: the change lands with the slice,
        // first in line behind it, and a report owed before it (the kernel
        // told nothing yet) runs after. Sampled when it runs, the report
        // tells the margin that shows, not the one when it was owed.
        p.collections.onFill?(list.owner.id, Data())
        XCTAssertTrue(session.fillInFlight)
        session.whenIdle {
            window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
            view.frame = root.view.bounds
            view.layoutIfNeeded()
        }
        host.told[list.owner.id] = nil
        host.sync(changed: [])
        let deadline = Date().addingTimeInterval(5)
        while session.fillInFlight, Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
        XCTAssertFalse(session.fillInFlight, "the slice lands")
        settle()
        XCTAssertNil(agent.reveal(["id": Int(button.id)])["error"])
        settle()
        let narrow = try starts()
        XCTAssertEqual(host.told[list.owner.id]?[2], narrow.margin)
        XCTAssertEqual(narrow.row, narrow.margin, accuracy: 0.5, "the custom row follows the margin that shows")
        guard narrow.margin != wide.margin else {
            throw XCTSkip("UIKit's list margin is \(narrow.margin) at both widths on this device (the iPhone Duo's), so a report sampled when it was owed would pass too")
        }
        XCTAssertEqual(narrow.margin, 16, "UIKit's cell margin at an iPhone's width")
    }
}
#endif
