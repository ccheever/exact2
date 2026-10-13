#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1075.003 §9.11: `navigationDetent`'s words as UIKit detents,
/// `fit-content` resolving to the route's content height within the sheet's
/// maximum, and a sheet presented over the native fixture following its rows.
final class ModalDetentIOSTests: XCTestCase {
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

    private final class Context: NSObject, UISheetPresentationControllerDetentResolutionContext {
        let containerTraitCollection = UITraitCollection()
        let maximumDetentValue: CGFloat
        init(_ maximum: CGFloat) { maximumDetentValue = maximum }
    }

    func testTheWordsBecomeDetentsInOrder() {
        let detents = ModalHost.detents("fit-content 300 large nonsense") { 120 }
        XCTAssertEqual(detents.map(\.identifier.rawValue), ["fit-content-0", "authored-1", UISheetPresentationController.Detent.Identifier.large.rawValue])
        XCTAssertEqual(detents[1].resolvedValue(in: Context(700)), 300)
        XCTAssertTrue(ModalHost.detents(nil) { nil }.isEmpty)
    }

    func testFitContentIsTheContentHeightWithinTheMaximum() {
        var content: CGFloat? = 180.4
        let detent = ModalHost.detents("fit-content") { content }[0]
        XCTAssertEqual(detent.resolvedValue(in: Context(700)), 181, "rounded up to a whole point")
        content = 900
        XCTAssertEqual(detent.resolvedValue(in: Context(700)), 700, "clamped to the sheet's maximum")
        // Read at each resolution: a grown route resolves anew.
        content = 260
        XCTAssertEqual(detent.resolvedValue(in: Context(700)), 260)
        // Not measured yet: the sheet's maximum, as the default large sheet.
        content = nil
        XCTAssertEqual(detent.resolvedValue(in: Context(700)), 700)
        content = 0
        XCTAssertEqual(detent.resolvedValue(in: Context(700)), 700)
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// The native fixture booted in a window of the test host's scene: UIKit
    /// presents a sheet only from a window a scene holds.
    private func fixture(_ label: String) throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        self.host = host
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        // A sheet waits for the first drawn frame, which this window never draws.
        if session.firstDrawMs == nil { session.firstDrawMs = ExactEnv.wall() }
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    /// Pressed directly: the sheet's view is never laid out in this window,
    /// so a tap finds nothing to hit there.
    private func tap(_ session: ExactSession, _ testId: String) throws {
        session.presenter.press(try node(session, testId).id)
    }

    /// The sheet over the fixture, once presented.
    private func sheet() -> UIViewController? {
        var top = host?.presentedViewController
        while let next = top?.presentedViewController { top = next }
        return top
    }

    /// The fixture's menu: 16 points of padding around More, Fewer, its
    /// rows, Close and Close both, 44 points each.
    private func rows(_ n: Int) -> CGFloat { 16 + 44 * CGFloat(4 + n) + 16 }

    /// `invalidateDetents()` calls on any sheet while `body` runs. This
    /// window finishes no presentation transition (the sheet's view stays
    /// zero-sized), so the sheet's own height cannot be read here; what
    /// resizes it is the invalidation, counted by exchanging UIKit's method
    /// for the test's, which calls it.
    private func countingInvalidations(_ body: () throws -> Void) rethrows {
        let cls: AnyClass = UISheetPresentationController.self
        let original = class_getInstanceMethod(cls, #selector(UISheetPresentationController.invalidateDetents))!
        let counting = class_getInstanceMethod(cls, #selector(UISheetPresentationController.exactTestInvalidateDetents))!
        method_exchangeImplementations(original, counting)
        defer { method_exchangeImplementations(original, counting) }
        UISheetPresentationController.exactTestInvalidations = 0
        try body()
    }

    /// LLP 1115 D5: a sheet that declares no Back control still swipes down,
    /// and its dismissal goes back by the root's `navigate`, once.
    func testASheetWithNoBackControlStillSwipesDownAndGoesBackByNavigate() throws {
        try sheetSwipesDown(navigate: true)
    }

    /// The same with no `navigate` handler: the runner's own back, once.
    func testASheetWithNoBackControlAndNoNavigateHandlerIsTheRunnersOwnBack() throws {
        try sheetSwipesDown(navigate: false)
    }

    private func sheetSwipesDown(navigate: Bool) throws {
        let session = try fixture("sheet-back-always")
        let root = try node(session, "navigation")
        if !navigate { root.handlers.remove("navigate") }
        try tap(session, "open-note")
        until("the note is presented as a sheet") { sheet()?.sheetPresentationController != nil }
        let presented = try XCTUnwrap(sheet())
        let controller = try XCTUnwrap(presented.presentationController)
        let modals = session.presenter.modals
        XCTAssertTrue(modals.presentationControllerShouldDismiss(controller), "the swipe may dismiss it")
        // A finished swipe: UIKit dismisses the sheet, then tells its
        // delegate (this window finishes no transition, so it is told here).
        presented.presentingViewController?.dismiss(animated: false)
        modals.presentationControllerDidDismiss(controller)
        until("the router went back") {
            let text = session.agent(#"{"op":"state"}"#)
            let json = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any]
            let nav = (json?["slots"] as? [String: Any])?["nav"] as? [String: Any]
            return ((nav?["tabs"] as? [[String: Any]])?.first?["stack"] as? [Any])?.count == 1
        }
        let journal = session.agent(#"{"op":"logs","since":0}"#)
        XCTAssertEqual(journal.components(separatedBy: "(follow)").count - 1, navigate ? 1 : 0, "navigate once, or never: \(journal)")
        XCTAssertEqual(journal.components(separatedBy: "host back").count - 1, navigate ? 0 : 1, "the runner's own back without a handler: \(journal)")
        XCTAssertFalse(journal.contains("modal dismissal refused"))
    }

    /// Through the real presenter: a row added or taken away reaches the
    /// presented sheet, whose detents are invalidated (the `content` op
    /// reaching `ModalHost.contentChanged`), and its detent resolves to the
    /// new height; it is the same sheet throughout.
    func testAFitContentSheetFollowsItsRows() throws {
        let session = try fixture("detent-follows")
        try countingInvalidations {
            try tap(session, "menu")
            until("the menu is presented as a sheet") { sheet()?.sheetPresentationController != nil }
            let presented = try XCTUnwrap(sheet())
            let controller = try XCTUnwrap(presented.sheetPresentationController)
            XCTAssertEqual(controller.detents.map(\.identifier.rawValue), ["fit-content-0"])
            XCTAssertFalse(controller.prefersGrabberVisible, "one detent: no grabber")
            let detent = { controller.detents.first?.resolvedValue(in: Context(800)) }
            until("at its rows' height") { detent() == rows(1) }
            var seen = UISheetPresentationController.exactTestInvalidations
            for (press, n) in [("menu-more", 2), ("menu-more", 3), ("menu-fewer", 2)] {
                try tap(session, press)
                until("\(press): \(n) rows") { detent() == rows(n) }
                until("\(press) invalidates the detents") { UISheetPresentationController.exactTestInvalidations > seen }
                seen = UISheetPresentationController.exactTestInvalidations
            }
            XCTAssertTrue(sheet() === presented, "the same sheet, never presented again")
            // Taller than the sheet's maximum: the maximum.
            XCTAssertEqual(controller.detents.first?.resolvedValue(in: Context(100)), 100)
        }
    }

    /// `"fit-content large"`: the sheet starts at its rows, with the grabber;
    /// a content change invalidates the detents and keeps whichever is
    /// selected.
    func testFitContentLargeKeepsTheSelectedDetent() throws {
        let session = try fixture("detent-large")
        try tap(session, "menu-large")
        try countingInvalidations {
            try tap(session, "menu")
            until("the menu is presented as a sheet") { sheet()?.sheetPresentationController != nil }
            let controller = try XCTUnwrap(sheet()?.sheetPresentationController)
            XCTAssertEqual(controller.detents.map(\.identifier), [.init("fit-content-0"), .large])
            XCTAssertTrue(controller.prefersGrabberVisible)
            XCTAssertEqual(controller.selectedDetentIdentifier, .init("fit-content-0"), "it starts at its rows")
            let detent = { controller.detents.first?.resolvedValue(in: Context(800)) }
            until("at its rows' height") { detent() == rows(1) }
            var seen = UISheetPresentationController.exactTestInvalidations
            try tap(session, "menu-more")
            until("a row added") { detent() == rows(2) && UISheetPresentationController.exactTestInvalidations > seen }
            XCTAssertEqual(controller.selectedDetentIdentifier, .init("fit-content-0"))
            // Dragged to large (a drag sets the selection), a row added keeps it.
            controller.selectedDetentIdentifier = .large
            seen = UISheetPresentationController.exactTestInvalidations
            try tap(session, "menu-more")
            until("another row") { detent() == rows(3) && UISheetPresentationController.exactTestInvalidations > seen }
            XCTAssertEqual(controller.selectedDetentIdentifier, .large, "still large")
        }
    }
}

private extension UISheetPresentationController {
    static var exactTestInvalidations = 0
    /// Exchanged with `invalidateDetents()`: after the exchange this name
    /// runs UIKit's own.
    @objc func exactTestInvalidateDetents() {
        Self.exactTestInvalidations += 1
        exactTestInvalidateDetents()
    }
}
#endif
