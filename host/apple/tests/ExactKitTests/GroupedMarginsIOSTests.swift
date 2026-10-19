#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

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
