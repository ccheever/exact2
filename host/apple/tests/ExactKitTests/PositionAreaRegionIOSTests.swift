// A lifted popover sized in its position-area option's region (LLP 1021
// D2), as Chrome sizes the box: PositionAreaRegionMacTests' table on iOS, in
// a 400×600 window, from the same pages (tests/fixtures/position-area-region
// {,-rtl}.contract, compiled by build.mjs --test).
#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

final class PositionAreaRegionIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func booted(_ key: String, in scene: UIWindowScene) throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment[key], "build.mjs --test compiles tests/fixtures/position-area-region*.contract")
        let plan = try Data(contentsOf: URL(fileURLWithPath: path))
        let session = ExactApp.shared.makeSession(label: "position-area-region")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let window = UIWindow(windowScene: scene)
        // The page's area is Chrome's 400×600 viewport: below the status bar's band, which a scene's
        // window at the screen's top has as its top safe area.
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 600)
        window.frame.size.height += window.safeAreaInsets.top
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

    private func check(rtl: Bool, in scene: UIWindowScene) throws {
        let rows = rtl ? PositionAreaRegionTable.rows.map(PositionAreaRegionTable.mirrored) : PositionAreaRegionTable.rows
        let session = try booted(rtl ? "EXACT_POSITION_AREA_RTL_PLAN" : "EXACT_POSITION_AREA_PLAN", in: scene), p = session.presenter
        if rtl { p.documentDirection = "rtl" }
        func node(_ id: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == id }, id) }
        func settle(_ seconds: TimeInterval = 0.1) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
        for (i, row) in rows.enumerated() {
            let pop = try node("p\(i)")
            p.press(try node("a\(i)").id)
            let end = Date().addingTimeInterval(3)
            while !p.menus.isOpen(pop), Date() < end { settle(0.05) }
            settle(0.3)
            let layer = try XCTUnwrap(pop.superview, "lifted")
            XCTAssertEqual(layer.bounds.size, CGSize(width: 400, height: 600), "the page's area, Chrome's viewport")
            let got = pop.convert(pop.bounds, to: layer)
            let note = "\(rtl ? "rtl " : "")\(row.0),\(row.1) \(row.2) / \(row.3): \(got) want \(row.4)"
            for (a, b) in [(got.minX, row.4.minX), (got.minY, row.4.minY), (got.width, row.4.width), (got.height, row.4.height)] {
                XCTAssertEqual(a, b, accuracy: 0.5, note)
            }
            p.press(try node("a\(i)").id) // the invoker toggles it closed
            settle(0.2)
        }
    }

    func testALiftedPopoverIsSizedInItsOptionsRegionAsChromeSizesIt() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else { throw missing("scene", "a lifted popover's layer mounts in a scene's window") }
        try check(rtl: false, in: scene)
    }
    func testRightToLeftIsTheMirrorImageAsInChrome() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else { throw missing("scene", "a lifted popover's layer mounts in a scene's window") }
        try check(rtl: true, in: scene)
    }
}
#endif
