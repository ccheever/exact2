#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

final class RowLifetimeIOSTests: XCTestCase {
    func testAHostedSessionReleasesItsCollectionWithAnUpdatePending() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "session teardown with a visible collection update")
        }
        ExactGroupedLists.install()
        weak var collection: UICollectionView?
        weak var feed: GroupedListView?
        weak var source: AnyObject?
        var completed = false
        try autoreleasepool {
            let session = ExactApp.shared.makeSession(label: "row-session-teardown")
            let view = ExactView(session: session), controller = UIViewController()
            let window = UIWindow(windowScene: scene)
            window.rootViewController = controller
            window.makeKeyAndVisible()
            controller.view.addSubview(view)
            view.frame = controller.view.bounds
            XCTAssertNil(session.boot(plan: try fixturePlan("PAGE_EDGES_GROUPED"), size: view.bounds.size).error)
            view.fit(); window.layoutIfNeeded()
            let p = session.presenter
            let node = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "list" })
            let host = try XCTUnwrap(p.groupedLists as? GroupedListHost)
            let list = try XCTUnwrap(host.lists[node.id])
            collection = list.collection; feed = list
            let data = try XCTUnwrap(list.collection.dataSource as? UICollectionViewDiffableDataSource<UInt32, UInt32>)
            source = data
            list.collection.layoutIfNeeded()
            var snapshot = data.snapshot()
            let first = try XCTUnwrap(snapshot.itemIdentifiers.first), last = try XCTUnwrap(snapshot.itemIdentifiers.last)
            snapshot.moveItem(first, afterItem: last)
            data.apply(snapshot, animatingDifferences: true) { completed = true }
            XCTAssertFalse(completed, "destroy while UIKit still owes the collection update's completion")
            session.destroy()
            XCTAssertEqual(session.state, .destroyed)
            window.isHidden = true
            window.rootViewController = nil
        }
        let began = Date(), until = began.addingTimeInterval(5)
        while (collection != nil || !completed), Date() < until {
            autoreleasepool { _ = RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        }
        print("hosted teardown after \(Date().timeIntervalSince(began)) s: feed=\(feed != nil) collection=\(collection != nil) source=\(source != nil) completed=\(completed)")
        XCTAssertTrue(completed)
        XCTAssertNil(feed)
        XCTAssertNil(source)
        XCTAssertNil(collection, "the unchanged five-second release condition")
    }
}
#endif
