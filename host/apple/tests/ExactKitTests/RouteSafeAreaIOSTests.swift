#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class RouteSafeAreaIOSTests: XCTestCase {
    /// Model the contribution UIKit supplies separately from authored insets.
    private final class InsetsScroll: UIScrollView {
        var automatic = UIEdgeInsets(top: 140, left: 20, bottom: 34, right: 40)
        var insetWritten: (() -> Void)?
        override var adjustedContentInset: UIEdgeInsets {
            .init(top: contentInset.top + automatic.top, left: contentInset.left + automatic.left,
                  bottom: contentInset.bottom + automatic.bottom, right: contentInset.right + automatic.right)
        }
        // Keep UIKit's Objective-C/KVO dispatch when the test writes through Swift.
        override dynamic var contentInset: UIEdgeInsets {
            didSet { insetWritten?() }
        }
        func changeTopAsUIKitDoes(_ top: CGFloat) {
            let anchor = contentOffset.y + adjustedContentInset.top
            automatic.top = top
            contentOffset.y = anchor - adjustedContentInset.top
        }
    }

    private final class Fixture {
        let presenter = Presenter()
        let route: RouteController
        let node: NodeView
        let scroll = InsetsScroll(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        init(bottomOnly: Bool = false) {
            let root = NodeView(id: 1, kind: "view", presenter: presenter)
            root.props["navigationKey"] = "test"
            route = RouteController(root)
            node = NodeView(id: 2, kind: "view", presenter: presenter)
            presenter.views[1] = root; presenter.views[2] = node
            presenter.root.addSubview(root); root.addSubview(node); node.addSubview(scroll)
            node.mountNativeScrollView(scroll)
            scroll.contentInsetAdjustmentBehavior = .automatic
            scroll.contentSize = CGSize(width: 1600, height: 1600)
            if bottomOnly { route.edgeScroll = node }
            else { route.collapseScroll = node; route.collapseScrollView = scroll }
        }
        func policy(_ value: String) {
            route.node.props["safeArea"] = value
            route.safeArea.sync(route)
        }
    }

    func testCollapseAndExpansionKeepTheAnchorUIKitAlreadyPreserved() {
        let f = Fixture()
        f.policy("top")
        f.scroll.contentOffset.y = 200
        for top: CGFloat in [90, 140] {
            let before = f.scroll.contentOffset.y + f.scroll.adjustedContentInset.top
            f.scroll.changeTopAsUIKitDoes(top)
            f.route.safeArea.sync(f.route)
            XCTAssertEqual(f.scroll.contentOffset.y + f.scroll.adjustedContentInset.top, before, accuracy: 0.01)
            XCTAssertEqual(f.scroll.adjustedContentInset.top, 0, accuracy: 0.01)
        }
    }

    func testOptingOutAtTheEndClampsToTheNewRange() {
        let f = Fixture()
        f.scroll.automatic.top = 100
        f.scroll.contentOffset.y = 834
        f.policy("top")
        XCTAssertEqual(f.scroll.contentOffset.y, 834, accuracy: 0.01)
        XCTAssertLessThanOrEqual(f.scroll.contentOffset.y, f.scroll.contentSize.height + f.scroll.adjustedContentInset.bottom - f.scroll.bounds.height)
    }

    func testEveryEdgeIsCancelledOnBothKindsOfPrimaryScroller() {
        for bottomOnly in [false, true] {
            let f = Fixture(bottomOnly: bottomOnly)
            f.policy("none")
            XCTAssertEqual(f.scroll.adjustedContentInset, .zero)
            XCTAssertEqual(f.scroll.contentInsetAdjustmentBehavior, .automatic)
            f.policy("auto")
            XCTAssertEqual(f.scroll.contentInset, .zero)
            f.policy("left right")
            XCTAssertEqual(f.scroll.adjustedContentInset.left, 0)
            XCTAssertEqual(f.scroll.adjustedContentInset.right, 0)
            XCTAssertEqual(f.scroll.adjustedContentInset.top, 140)
            XCTAssertEqual(f.scroll.adjustedContentInset.bottom, 34)
        }
    }

    func testTopIsCancelledOnABottomOnlyScroller() {
        let f = Fixture(bottomOnly: true)
        f.policy("top")
        XCTAssertEqual(f.scroll.adjustedContentInset.top, 0)
        XCTAssertEqual(f.scroll.adjustedContentInset.bottom, 34)
    }

    func testInvalidRuntimeValuesAreAutoAsAWhole() {
        let f = Fixture()
        for value in ["none bottom", "auto top", "top top", "Top", "top,bottom", "", "top unknown"] {
            f.route.node.props["safeArea"] = value
            for edge in ["top", "right", "bottom", "left"] {
                XCTAssertFalse(f.route.ignoresSafeArea(edge), "\(value): \(edge)")
            }
            f.route.safeArea.sync(f.route)
            XCTAssertEqual(f.scroll.contentInset, .zero, value)
        }
    }

    func testRefreshRestoringItsSnapshotDoesNotRestoreAnOldCorrection() {
        for band: CGFloat in [0, 12] {
            let f = Fixture()
            let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
            let host = UIViewController()
            window.rootViewController = host
            f.presenter.root.frame = window.bounds
            f.route.node.frame = window.bounds; f.node.frame = window.bounds
            host.view.addSubview(f.presenter.root)
            window.makeKeyAndVisible(); host.view.layoutIfNeeded()
            defer { window.isHidden = true }
            f.node.handlers.insert("refresh"); f.node.props["refreshing"] = "true"
            f.policy("top")
            let refresh = UIRefreshControl()
            f.scroll.refreshControl = refresh
            let snapshot = f.scroll.contentInset
            refresh.beginRefreshing()
            XCTAssertTrue(refresh.isRefreshing, "UIKit starts refresh only while the control is in a window")
            f.scroll.contentOffset.y = 200
            f.scroll.changeTopAsUIKitDoes(90)
            f.route.safeArea.sync(f.route)
            XCTAssertEqual(f.scroll.contentInset.top, -90)
            // A control bar changes its own contribution while the refresh runs.
            f.node.more.bandInset.top = band
            f.scroll.contentInset.top += band
            f.scroll.verticalScrollIndicatorInsets.top += band
            refresh.endRefreshing()
            f.scroll.contentInset = snapshot
            XCTAssertEqual(f.scroll.adjustedContentInset.top, band, accuracy: 0.01, "the native write itself requests reconciliation")
            f.route.safeArea.sync(f.route)
            XCTAssertEqual(f.scroll.contentInset.top, band - 90, accuracy: 0.01)
            // A late frame of that animation can arrive after an authored policy change.
            f.policy("auto")
            f.scroll.contentInset = snapshot
            XCTAssertEqual(f.scroll.contentInset.top, band, accuracy: 0.01)
        }
    }

    func testAReentrantBackendChangeReleasesTheOldAndCorrectsTheNewScroller() {
        let f = Fixture()
        let next = InsetsScroll(frame: f.scroll.frame)
        next.contentSize = f.scroll.contentSize
        f.scroll.insetWritten = {
            f.scroll.insetWritten = nil
            f.node.addSubview(next)
            f.node.mountNativeScrollView(next)
            next.contentInsetAdjustmentBehavior = .automatic
            f.route.collapseScrollView = next
            f.route.safeArea.sync(f.route)
        }
        f.policy("top")
        XCTAssertEqual(f.scroll.contentInset, .zero)
        XCTAssertEqual(next.adjustedContentInset.top, 0)
    }

    func testAReentrantInsetWriterKeepsItsIndicatorContribution() {
        let f = Fixture()
        f.scroll.insetWritten = {
            f.scroll.insetWritten = nil
            f.scroll.contentInset.bottom += 11
            f.scroll.verticalScrollIndicatorInsets.bottom += 11
            f.scroll.horizontalScrollIndicatorInsets.bottom += 11
            f.route.safeArea.sync(f.route)
        }
        f.policy("top")
        XCTAssertEqual(f.scroll.contentInset.bottom, 11)
        XCTAssertEqual(f.scroll.verticalScrollIndicatorInsets.bottom, 11)
        XCTAssertEqual(f.scroll.horizontalScrollIndicatorInsets.bottom, 11)
    }

    func testRetirementRequestedByASetterReleasesAllCorrections() {
        let f = Fixture()
        f.scroll.insetWritten = {
            f.scroll.insetWritten = nil
            f.route.safeArea.end()
            f.route.safeArea.sync(f.route)
        }
        f.policy("none")
        XCTAssertEqual(f.scroll.contentInset, .zero)
        XCTAssertEqual(f.scroll.verticalScrollIndicatorInsets, .zero)
        XCTAssertEqual(f.scroll.horizontalScrollIndicatorInsets, .zero)
        f.route.safeArea.sync(f.route)
        XCTAssertEqual(f.scroll.contentInset, .zero, "a retired route cannot reacquire its insets")
    }

    func testUnrelatedInsetsAndIndicatorsSurvivePolicyChanges() {
        let f = Fixture()
        let base = UIEdgeInsets(top: 8, left: 4, bottom: 23, right: 6)
        f.scroll.contentInset = base
        f.scroll.verticalScrollIndicatorInsets = base
        f.scroll.horizontalScrollIndicatorInsets = base
        f.policy("none")
        XCTAssertEqual(f.scroll.adjustedContentInset, base)
        f.scroll.contentInset.bottom += 11
        f.scroll.verticalScrollIndicatorInsets.bottom += 11
        f.policy("auto")
        XCTAssertEqual(f.scroll.contentInset.top, 8)
        XCTAssertEqual(f.scroll.contentInset.left, 4)
        XCTAssertEqual(f.scroll.contentInset.bottom, 34)
        XCTAssertEqual(f.scroll.contentInset.right, 6)
        XCTAssertEqual(f.scroll.verticalScrollIndicatorInsets.bottom, 34)
        XCTAssertEqual(f.scroll.horizontalScrollIndicatorInsets, base)
    }
}
#endif
