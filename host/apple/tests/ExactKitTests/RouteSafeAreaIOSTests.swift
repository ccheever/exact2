#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1123 D4's executor on a scroller whose UIKit contribution the test
/// sets, inside a view whose safe area it sets too.
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
        /// The safe area the route's view has: what the chrome would take on each edge.
        static let safe = UIEdgeInsets(top: 140, left: 20, bottom: 34, right: 40)
        let presenter = Presenter()
        let window: UIWindow
        private let host = UIViewController()
        private var device = UIEdgeInsets.zero
        let route: RouteController
        let node: NodeView
        let scroll = InsetsScroll(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        init(bottomOnly: Bool = false) {
            let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
            window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow()
            window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
            window.rootViewController = host
            window.makeKeyAndVisible()
            host.view.layoutIfNeeded()
            device = host.view.safeAreaInsets
            host.additionalSafeAreaInsets = UIEdgeInsets(top: Self.safe.top - device.top, left: Self.safe.left - device.left,
                                                         bottom: Self.safe.bottom - device.bottom, right: Self.safe.right - device.right)
            let root = NodeView(id: 1, kind: "view", presenter: presenter)
            root.props["navigationKey"] = "test"
            route = RouteController(root)
            node = NodeView(id: 2, kind: "view", presenter: presenter)
            presenter.views[1] = root; presenter.views[2] = node
            for view in [presenter.root, root, node] { view.frame = window.bounds }
            host.view.addSubview(presenter.root)
            presenter.root.addSubview(root); root.addSubview(node); node.addSubview(scroll)
            node.mountNativeScrollView(scroll)
            host.view.layoutIfNeeded()
            scroll.contentInsetAdjustmentBehavior = .automatic
            scroll.contentSize = CGSize(width: 1600, height: 1600)
            if bottomOnly { route.edgeScroll = node }
            else { route.collapseScroll = node; route.collapseScrollView = scroll }
        }
        deinit { window.isHidden = true }
        /// The view's safe area at the top, as a bar's height changes it.
        func safeTop(_ top: CGFloat) {
            host.additionalSafeAreaInsets.top = top - device.top
            host.view.layoutIfNeeded()
        }
        func policy(_ value: String) {
            route.node.props["safeArea"] = value
            route.safeArea.sync(route)
        }
    }

    func testTheFixtureGivesTheScrollerItsSafeArea() {
        let f = Fixture()
        XCTAssertEqual(f.node.safeAreaInsets, Fixture.safe)
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

    func testChangingClearancePreservesAnExistingPullPastTheStart() {
        let f = Fixture()
        f.scroll.automatic.top = 100
        f.scroll.contentOffset.y = -180
        f.policy("top")
        XCTAssertEqual(f.scroll.contentOffset.y, -80, accuracy: 0.01)
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

    /// While a control refreshes, UIKit adds its room to the scroller's own
    /// safe area and so to its contribution, not to the view's (iOS 27.0 and
    /// 27.1, measured with a pull): the room stays on an edge turned off.
    func testARefreshingControlKeepsItsRoomOnAnEdgeTurnedOff() {
        let f = Fixture()
        f.policy("top")
        XCTAssertEqual(f.scroll.adjustedContentInset.top, 0)
        f.scroll.automatic.top += 60
        f.route.safeArea.sync(f.route)
        XCTAssertEqual(f.scroll.adjustedContentInset.top, 60, accuracy: 0.01)
        XCTAssertEqual(f.scroll.contentInset.top, -140, accuracy: 0.01)
        f.safeTop(90)
        f.scroll.changeTopAsUIKitDoes(90 + 60)
        f.route.safeArea.sync(f.route)
        XCTAssertEqual(f.scroll.adjustedContentInset.top, 60, accuracy: 0.01, "a collapse while it refreshes")
        f.scroll.automatic.top -= 60
        f.route.safeArea.sync(f.route)
        XCTAssertEqual(f.scroll.adjustedContentInset.top, 0, accuracy: 0.01)
    }

    func testAReleasedScrollerIsHandedBackAndCorrectedAgainWhileItIsATarget() {
        let f = Fixture()
        f.policy("top")
        XCTAssertEqual(f.scroll.contentInset.top, -140)
        f.route.safeArea.release(f.scroll)
        XCTAssertEqual(f.scroll.contentInset, .zero)
        XCTAssertEqual(f.scroll.verticalScrollIndicatorInsets, .zero)
        f.route.safeArea.sync(f.route)
        XCTAssertEqual(f.scroll.adjustedContentInset.top, 0)
    }

    func testTheTitlesInsetsComeBackWithTheTop() {
        let f = Fixture()
        f.node.scrollOrigin = 140
        f.node.scrollCollapsed = 90
        f.policy("top")
        XCTAssertEqual(f.node.scrollOrigin, 0, "scrollTop 0 is the content's top")
        f.scroll.changeTopAsUIKitDoes(90)
        f.route.safeArea.sync(f.route)
        f.policy("auto")
        XCTAssertEqual(f.node.scrollOrigin, 140, "the expanded title's, though the title is collapsed")
        XCTAssertEqual(f.node.scrollCollapsed, 90)
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
