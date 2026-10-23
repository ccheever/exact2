#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

final class RowLifetimeIOSTests: GroupedListFixture {
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }

    func testAGroupedListsCollectionAndFeedReleaseAfterThePresenterResets() throws {
        weak var collection: UICollectionView?
        weak var feed: GroupedListView?
        weak var source: AnyObject?
        weak var layout: UICollectionViewLayout?
        try autoreleasepool {
            let model = model()
            let p = presenter { model }
            let list = try list(p)
            collection = list.collection; feed = list
            source = list.collection.dataSource; layout = list.collection.collectionViewLayout
            list.collection.layoutIfNeeded()
            p.reset()
            p.viewport.removeFromSuperview()
            window.isHidden = true; window = nil
        }
        let began = Date(), until = began.addingTimeInterval(5)
        while collection != nil, Date() < until {
            autoreleasepool { _ = RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        }
        print("grouped teardown after \(Date().timeIntervalSince(began)) s: feed=\(feed != nil) collection=\(collection != nil) source=\(source != nil) layout=\(layout != nil)")
        XCTAssertNil(feed)
        XCTAssertNil(collection, "the unchanged five-second release condition")
    }
}
#endif
