#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class DialogGeometryIOSTests: XCTestCase {
    private var session: ExactSession?
    private var window: UIWindow?
    override func tearDown() {
        session?.destroy(); session = nil
        window?.isHidden = true; window = nil
        super.tearDown()
    }
    private func until(_ message: String, _ predicate: () -> Bool) {
        let end = Date(timeIntervalSinceNow: 5)
        while !predicate(), Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertTrue(predicate(), message)
    }
    private func slots(_ s: ExactSession) -> [String: Any] {
        let json = s.runtime.agent("{\"op\":\"state\"}")
        return ((try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any])?["slots"] as? [String: Any] ?? [:]
    }

    func testScrollingTheSheetDeliversIntersectionWithoutAnotherInput() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else { throw missing("scene", "native sheet scrolling requires a scene") }
        let s = ExactApp.shared.makeSession(label: "dialog-geometry")
        session = s
        let view = ExactView(session: s), host = UIViewController(), w = UIWindow(windowScene: scene)
        w.frame = w.screen.bounds; w.rootViewController = host; w.makeKeyAndVisible(); host.view.addSubview(view)
        window = w
        XCTAssertNil(s.boot(plan: try fixturePlan("DIALOG_SHEET_READS"), size: w.bounds.size).error)
        view.frame = host.view.bounds; view.layoutIfNeeded()
        if s.firstDrawMs == nil { s.firstDrawMs = ExactEnv.wall() }
        let p = s.presenter
        defer { p.menus.reset(); host.dismiss(animated: false) }
        p.dialogCommand("showModal", "form")
        until("the sheet appeared") { host.presentedViewController is DialogSheetController && !p.menus.inTransition }
        let sheet = try XCTUnwrap(host.presentedViewController as? DialogSheetController)
        until("initial observer delivered") { self.slots(s)["observations"] as? Double ?? 0 > 0 }
        XCTAssertEqual(slots(s)["seen"] as? Bool, false)
        // Reading slots does not sample UIKit geometry or send a runner input.
        sheet.scroller.setContentOffset(CGPoint(x: 0, y: 1300), animated: false)
        until("scrolling alone enters the target") { self.slots(s)["seen"] as? Bool == true }
        sheet.scroller.setContentOffset(.zero, animated: false)
        until("scrolling alone leaves the target") { self.slots(s)["seen"] as? Bool == false }
    }
}
#endif
