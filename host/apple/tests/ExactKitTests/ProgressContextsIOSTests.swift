#if os(iOS)
import UIKit
import XCTest
import ExactGroupedLists
@testable import ExactKit

final class ProgressContextsIOSTests: XCTestCase {
    private var session: ExactSession?
    private var window: UIWindow?
    private func until(_ message: String, _ predicate: () -> Bool) {
        let deadline = Date(timeIntervalSinceNow: 5)
        while !predicate(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertTrue(predicate(), message)
    }
    override func tearDown() {
        session?.destroy(); session = nil
        window?.isHidden = true; window = nil
        super.tearDown()
    }
    private func node(_ s: ExactSession, _ name: String) throws -> NodeView {
        try XCTUnwrap(s.presenter.views.values.first { $0.props["testId"] == name }, name)
    }
    private func assertAccent(_ s: ExactSession, _ name: String, file: StaticString = #filePath, line: UInt = #line) throws {
        let owner = try node(s, name)
        let spinner = try XCTUnwrap(s.presenter.controls.spinners[owner.id], name, file: file, line: line)
        XCTAssertNotNil(spinner.window, name, file: file, line: line)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        spinner.color.getRed(&r, green: &g, blue: &b, alpha: &a)
        for (got, want) in zip([r, g, b, a].map { Double($0) * 255 }, [0.0, 200, 0, 255]) {
            XCTAssertEqual(got, want, accuracy: 0.01, name, file: file, line: line)
        }
    }

    func testPresentedRoutesDialogsPopoversAndCarriedRowsKeepTheirAuthoredAccent() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else { throw missing("scene", "sheets require a scene window") }
        ExactGroupedLists.install()
        let s = ExactApp.shared.makeSession(label: "progress-contexts")
        session = s
        let view = ExactView(session: s), host = UIViewController(), w = UIWindow(windowScene: scene)
        w.frame = w.screen.bounds; w.rootViewController = host; w.makeKeyAndVisible(); host.view.addSubview(view)
        window = w
        XCTAssertNil(s.boot(plan: try fixturePlan("PROGRESS_CONTEXTS"), size: w.bounds.size).error)
        view.frame = host.view.bounds; view.layoutIfNeeded()
        if s.firstDrawMs == nil { s.firstDrawMs = ExactEnv.wall() }
        let p = s.presenter
        defer { p.menus.reset(); host.dismiss(animated: false) }
        until("the custom row is carried") { (try? self.node(s, "busy-carried"))?.window != nil }
        try assertAccent(s, "busy-start")
        try assertAccent(s, "busy-carried")
        let carried = try node(s, "busy-carried")
        XCTAssertTrue(sequence(first: carried as UIView, next: { $0.superview }).contains { $0 is UICollectionViewCell }, "the spinner is carried into a native cell")

        p.dialogCommand("showModal", "form")
        until("the dialog sheet is up") { host.presentedViewController != nil && !p.menus.inTransition }
        XCTAssertFalse(try node(s, "busy-dialog").isDescendant(of: try node(s, "app")))
        try assertAccent(s, "busy-dialog")
        p.dialogCommand("close", "form")
        until("the dialog sheet is gone") { host.presentedViewController == nil && !p.menus.inTransition }

        p.menus.action("showPopover", ["popup"])
        until("the popover is up") { (try? self.node(s, "busy-popover"))?.window != nil && !p.menus.inTransition }
        try assertAccent(s, "busy-popover")
        p.menus.action("hidePopover", ["popup"])

        XCTAssertTrue(s.follow("/detail"))
        until("the pushed route is up") { p.navigation.primaryNavigation?.viewControllers.count == 2 && !p.navigation.inTransition }
        try assertAccent(s, "busy-detail")
        XCTAssertTrue(s.follow("/sheet"))
        until("the modal route is up") { p.navigation.presentedNavigations.count == 1 && !p.modals.inTransition && (try? self.node(s, "busy-sheet"))?.window != nil }
        try assertAccent(s, "busy-sheet")
    }
}
#endif
