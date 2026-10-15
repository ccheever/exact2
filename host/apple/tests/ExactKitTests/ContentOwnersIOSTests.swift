#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1123 D2: every route is a content owner. Its root is laid out in its
/// controller's view, its host block (LLP 1115.002 D8.2), and the runner is
/// told where each route a window shows is: a presented one as a top-layer
/// box, as a dialog is, and any other by its placement in its parent, so
/// that it keeps its place among its siblings.
final class ContentOwnersIOSTests: XCTestCase {
    /// A touch resting on `target` at `point`, as UIKit hands a view one.
    private final class SheetTouch: UITouch {
        let target: UIView, point: CGPoint
        init(_ target: UIView, at point: CGPoint) { self.target = target; self.point = point; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { target.convert(point, to: view) }
    }

    private var window: UIWindow?
    private var host: UIViewController?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        host = nil
        super.tearDown()
    }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// A fixture booted in a window of the test host's scene, `size` large:
    /// UIKit presents a sheet only from a window a scene holds. The native
    /// fixture unless `plan` names another of the plans the test run builds.
    private func fixture(_ label: String, _ size: CGSize, plan key: String = "EXACT_FIXTURE_PLAN") throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env[key], "build.mjs --test --ios compiles \(key)")))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let frame = CGRect(origin: .zero, size: size)
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: frame)
        window.frame = frame
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        self.host = host
        XCTAssertNil(session.boot(plan: plan, size: size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        until("the stack is installed") { session.presenter.navigation.primaryOwner != nil }
        // A sheet waits for the first drawn frame, which this window never draws.
        if session.firstDrawMs == nil { session.firstDrawMs = ExactEnv.wall() }
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    /// A node's size as the kernel last laid it out (the agent's `frames`).
    private func laidOut(_ session: ExactSession, _ id: UInt32) -> CGSize? {
        guard let reply = try? JSONSerialization.jsonObject(with: Data(session.runtime.agent("{\"op\":\"frames\"}").utf8)) as? [String: Any],
              let row = (reply["nodes"] as? [[Any]])?.first(where: { ($0[0] as? NSNumber)?.uint32Value == id }),
              let width = row[4] as? NSNumber, let height = row[5] as? NSNumber else { return nil }
        return CGSize(width: width.doubleValue, height: height.doubleValue)
    }

    /// Where the runner is told a presented `route` is shown, if it is.
    private func shown(_ session: ExactSession, _ route: NodeView) -> CGRect? {
        session.presenter.openTopLayerFrames.first { $0.0 == route.id }?.1
    }

    /// Where the runner is told an ordinary `route` is shown in the page.
    private func placed(_ session: ExactSession, _ route: NodeView) -> BlockPlacement? {
        session.presenter.owners.told[route.id]?.placement
    }

    /// `view` in the local coordinates of the routes' owner, where an
    /// ordinary route's placement is.
    private func local(_ session: ExactSession, _ view: UIView) -> BlockPlacement? {
        let owner = session.presenter.navigation.container
        return owner.map { BlockPlacement(rect: view.convert(view.bounds, to: $0), within: $0.id) }
    }

    /// Where a person sees `view`, in viewport points.
    private func seen(_ session: ExactSession, _ view: UIView) -> CGRect {
        let p = session.presenter
        return view.convert(view.bounds, to: p.viewport).offsetBy(dx: -p.viewport.contentOffset.x, dy: -p.viewport.contentOffset.y)
    }

    /// The sheet just presented, put at `frame`, where UIKit's transition
    /// puts a page sheet: this window finishes no presentation transition
    /// (ModalDetentIOSTests).
    private func mountSheet(at frame: CGRect = CGRect(x: 0, y: 60, width: 402, height: 814)) throws -> UIViewController {
        let controller = try XCTUnwrap(host?.presentedViewController, "the sheet is presented")
        if controller.view.window == nil { window?.addSubview(controller.view) }
        controller.view.frame = frame
        controller.view.layoutIfNeeded()
        return controller
    }

    /// The route is laid out at its controller's size and reported where
    /// its controller's view is: at the stack's root and pushed over it,
    /// each by its placement in its parent and never in the top layer, back
    /// again, and in a sheet, which is a top-layer box where the runner's
    /// modal boundary begins, as the host's `inertBehind` draws it.
    func testEachRouteIsLaidOutInItsControllersViewAndReportedWhereItIsShown() throws {
        let session = try fixture("owner-routes", CGSize(width: 402, height: 874))
        let p = session.presenter
        let home = try node(session, "route-home")
        let homeView = try XCTUnwrap(p.navigation.controllers[home.id]?.view)
        XCTAssertEqual(laidOut(session, home.id), homeView.bounds.size)
        XCTAssertEqual(placed(session, home), local(session, homeView), "the stack's root is placed where it is shown")
        XCTAssertNil(shown(session, home), "and keeps its place among its siblings")
        // Its parent's own transform is the runner's to apply, not the placement's
        let root = try XCTUnwrap(p.navigation.container)
        let before = try XCTUnwrap(placed(session, home))
        root.transform = CGAffineTransform(translationX: 0, y: 40)
        homeView.setNeedsLayout(); homeView.layoutIfNeeded()
        p.owners.publish()
        XCTAssertEqual(placed(session, home), before, "the parent's translation is not in the placement")
        root.transform = .identity

        p.press(try node(session, "detail").id)
        let detail = try node(session, "route-detail")
        until("the pushed route is placed") { self.placed(session, detail) != nil && !p.navigation.transitioning }
        let detailView = try XCTUnwrap(p.navigation.controllers[detail.id]?.view)
        XCTAssertEqual(placed(session, detail), local(session, detailView))
        XCTAssertNil(shown(session, detail))
        XCTAssertEqual(laidOut(session, detail.id), detailView.bounds.size)
        p.press(try node(session, "back").id)
        until("back on the stack's root") { p.navigation.activeRoute?.node === home && !p.navigation.transitioning }
        XCTAssertEqual(placed(session, home), local(session, homeView))
        XCTAssertNil(p.openTopLayer.modal, "nothing is presented")

        p.press(try node(session, "sheet").id)
        let sheet = try node(session, "route-sheet")
        let controller = try mountSheet()
        let sheetView = try XCTUnwrap(p.navigation.controllers[sheet.id]?.view)
        until("the sheet's route takes its controller's size") { self.laidOut(session, sheet.id) == sheetView.bounds.size }
        XCTAssertEqual(shown(session, sheet), seen(session, sheetView), "the sheet's route is a top-layer box where it is shown")
        XCTAssertNil(placed(session, sheet), "with no placement")
        XCTAssertNil(shown(session, home), "the screen under it is not one")
        let open = p.openTopLayer
        XCTAssertEqual(open.modal, open.frames.firstIndex { $0.0 == sheet.id }, "the sheet's route begins the presentation")
        XCTAssertNotNil(open.modal)
    }

    /// A tab's route is read where UIKit shows it under a tab panel turned a
    /// quarter turn, then scaled to nothing
    /// (`scripts/fixtures/tab-owner.contract`): the tab container holds the
    /// route beside its panel, so neither reaches the card, and `frame`,
    /// both point reads and the intersection observer agree with the card's
    /// view.
    func testATabsRouteIsReadWhereUIKitShowsItUnderATurnedPanel() throws {
        let session = try fixture("owner-turned-tab", CGSize(width: 402, height: 874), plan: "EXACT_TAB_OWNER_PLAN")
        let p = session.presenter
        let card = try node(session, "card")
        let slots = { () -> [String: Any] in
            let state = try? JSONSerialization.jsonObject(with: Data(session.runtime.agent("{\"op\":\"state\"}").utf8)) as? [String: Any]
            return state?["slots"] as? [String: Any] ?? [:]
        }
        let read = { () -> [String] in
            p.press(try self.node(session, "read").id)
            let s = slots()
            return ["frame", "hit", "all", "seen"].map { s[$0] as? String ?? "" }
        }
        // The frame's numbers, which the runner composes in floating point
        let near = { (frame: String, rect: CGRect) -> Bool in
            let n = frame.split(separator: ",").compactMap { Double($0) }
            return n.count == 4 && zip(n, [rect.minX, rect.minY, rect.width, rect.height]).allSatisfy { abs($0 - $1) < 1e-6 }
        }
        let shown = seen(session, card)
        XCTAssertEqual(shown, CGRect(x: 16, y: 400, width: 100, height: 40), "the route fills the window")
        until("the observers have run") { slots()["beacon"] as? String == "true" && slots()["seen"] as? String != "unseen" }
        let before = try read()
        XCTAssertTrue(near(before[0], shown), before[0])
        XCTAssertEqual(before[1], "card")
        XCTAssertTrue(before[2].hasPrefix("card,"), before[2])
        XCTAssertEqual(before[3], "false:0", "the card is left of the observed half")

        p.press(try node(session, "turn").id)
        let panel = try XCTUnwrap(p.views.values.first { $0.props["id"] == "panel-first" })
        until("the panel is turned") { abs(panel.layer.transform.m12 - 1) < 1e-9 }
        until("the observers have seen the turn") { slots()["beacon"] as? String == "false" }
        XCTAssertEqual(seen(session, card), shown, "UIKit shows the card where it was")
        let after = try read()
        XCTAssertTrue(near(after[0], shown), after[0])
        XCTAssertEqual(Array(after[1...]), Array(before[1...]), "both point reads and the observer agree")

        p.press(try node(session, "flatten").id)
        until("the panel is flattened") { panel.layer.transform.m11 == 0 && panel.layer.transform.m12 == 0 && panel.layer.transform.m22 == 0 }
        until("the observers have seen it") { slots()["beacon"] as? String == "true" }
        XCTAssertEqual(seen(session, card), shown, "UIKit shows the card where it was")
        let flat = try read()
        XCTAssertTrue(near(flat[0], shown), flat[0])
        XCTAssertEqual(Array(flat[1...]), Array(before[1...]), "with no inverse needed")
    }

    /// The root no longer projecting its routes (its `navigationBack` gone)
    /// takes the stack down: the routes that stay give their blocks and
    /// placements back, and are laid out in their parent again, under
    /// `viewport-fit="cover"` as under `auto`.
    func testRoutesGiveTheirBlocksBackWhenTheStackGoes() throws {
        let session = try fixture("owner-teardown", CGSize(width: 402, height: 874))
        let p = session.presenter
        let root = try XCTUnwrap(p.navigation.container)
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["viewportFit": "cover"]]]))
        let home = try node(session, "route-home")
        until("the route has its block") { p.owners.told[home.id] != nil }
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": [String: String](), "clear": ["navigationBack"]]]))
        until("the stack is down") { p.navigation.primaryOwner == nil }
        XCTAssertTrue(p.owners.told.isEmpty, "every block given back: \(p.owners.told.keys.sorted())")
        XCTAssertTrue(home.isDescendant(of: root), "the route is its parent's again")
    }
}
#endif
