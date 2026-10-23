#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// Stage 14 B on iOS: a popover is laid out in the viewport, a block the
/// kernel implies, not in its positioned ancestor. Harmony's Pins popover
/// with no width (`top-layer-pins.contract`), opened as itself, is the size
/// the web gives it, and the runner is told it by its block.
final class TopLayerBlockIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func booted() throws -> ExactSession {
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_TOP_LAYER_PINS_PLAN"], "build.mjs --test compiles tests/fixtures/top-layer-pins.contract")))
        let session = ExactApp.shared.makeSession(label: "top-layer-pins")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        return session
    }

    /// Its cards on one line, 368 by 64 as Chrome lays the same contract
    /// out, where the header 100 points wide had trimmed it to one a line.
    func testAPopoverWithNoWidthIsTheWebsSizeUnderANarrowHeader() throws {
        let session = try booted(), p = session.presenter
        func node(_ id: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == id }, id) }
        let pins = try node("pins")
        p.press(try node("open").id)
        let end = Date().addingTimeInterval(3)
        while !(p.menus.isOpen(pins) && pins.bounds.width == 368), Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(p.menus.isOpen(pins), "opened as itself")
        XCTAssertEqual(pins.bounds.size, CGSize(width: 368, height: 64))
        XCTAssertEqual(p.owners.blockRects[pins.id], CGRect(x: 0, y: 0, width: 368, height: 64), "its block: no margins")
        let told = try XCTUnwrap(p.openTopLayer.frames.first { $0.0 == pins.id })
        XCTAssertEqual(told.1.size, CGSize(width: 368, height: 64), "told by its block, where UIKit shows it")
    }
}
#endif
