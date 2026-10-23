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

    func testStructuralReadsFollowTheSheetsPresentationBetweenEndpoints() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else { throw missing("scene", "native sheet motion requires a scene") }
        let s = ExactApp.shared.makeSession(label: "dialog-animation-geometry")
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
        let content = try XCTUnwrap(p.views.values.first { $0.props["id"] == "content" })
        let dialog = try XCTUnwrap(p.views.values.first { $0.props["id"] == "form" })
        until("native geometry rests") { !FrameClock.shared.wants(p.nativeGeometry) }
        let start = content.convert(content.bounds, to: p.viewport)
        var finished = false
        UIView.animate(withDuration: 0.8, delay: 0, options: [.curveLinear]) {
            sheet.view.transform = CGAffineTransform(translationX: 0, y: 160)
            sheet.view.setNeedsLayout(); sheet.view.layoutIfNeeded()
        } completion: { _ in finished = true }
        until("the sheet is between endpoints") {
            guard let layer = content.layer.presentation(), let port = p.viewport.layer.presentation() else { return false }
            let delta = layer.convert(layer.bounds, to: port).minY - start.minY
            return delta > 30 && delta < 120
        }
        let layer = try XCTUnwrap(content.layer.presentation()), port = try XCTUnwrap(p.viewport.layer.presentation())
        let visible = p.viewportPoints(layer.convert(layer.bounds, to: port))
        let destination = p.viewportPoints(content.convert(content.bounds, to: p.viewport))
        XCTAssertGreaterThan(abs(visible.minY - destination.minY), 30)
        let point = CGPoint(x: visible.midX, y: visible.midY)
        let sample = PointerSample(x: 0, y: 0, buttons: 0, pressure: 0, type: "mouse", id: 1, clientX: Double(point.x), clientY: Double(point.y))
        s.apply(s.runtime.pointer(dialog.id, .move, sample, now: s.now()))
        let state = slots(s), frame = (state["rect"] as? String ?? "").split(separator: ",").compactMap { Double($0) }
        XCTAssertEqual(frame.count, 4)
        if frame.count == 4 {
            for (got, want) in zip(frame, [visible.minX, visible.minY, visible.width, visible.height]) { XCTAssertEqual(got, want, accuracy: 2) }
        }
        XCTAssertEqual(state["hit"] as? String, "content")
        XCTAssertTrue(FrameClock.shared.wants(p.nativeGeometry), "native movement owns geometry frames")
        until("the native animation ends") { finished }
        until("native geometry releases its frames") { !FrameClock.shared.wants(p.nativeGeometry) }
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
