#if os(iOS)
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists
import UIKit

/// LLP 1116.005 D4: a large title survives a content swap under the bar
/// (`scripts/fixtures/still-title.contract`, chat-rooms' rooms screen). UIKit
/// moves the large title into the content scroll view it follows; once a
/// failed load's reason came between the header and the list, nothing
/// collapsed the title, UIKit was handed no scroller, and the title stayed in
/// the list at its top, clipped from sight below the reason: a blank band
/// under the bar.
final class StillTitleIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func fixture() throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_STILL_TITLE_PLAN"], "build.mjs --test --ios compiles the still-title fixture")
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "still-title")
        self.session = session
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.5)
        return session
    }

    private func find(_ testId: String) -> NodeView? {
        session?.presenter.views.values.first { $0.props["testId"] == testId && $0.window != nil }
    }
    private func node(_ testId: String) throws -> NodeView { try XCTUnwrap(find(testId), "no \(testId)") }

    private func tap(_ testId: String) throws {
        _ = Agent(session: try XCTUnwrap(session)).tap(["id": Int(try node(testId).id)])
        spin(0.6)
    }

    /// Pixels unlike the route's backdrop in the bar's large-title band (its
    /// bottom 52 points, from its leading margin), as the window's layers
    /// hold them, clips included.
    private func titleInk(_ nav: UINavigationController) throws -> Int {
        let window = try XCTUnwrap(self.window)
        let bar = nav.navigationBar.convert(nav.navigationBar.bounds, to: window)
        let band = CGRect(x: bar.minX + 8, y: bar.maxY - 52, width: bar.width - 16, height: 52)
        let w = Int(band.width), h = Int(band.height)
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                  space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
              let raw = ctx.data else { return 0 }
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: 1, y: -1)
        ctx.translateBy(x: -band.minX, y: -band.minY)
        window.layer.render(in: ctx)
        let p = raw.bindMemory(to: UInt8.self, capacity: w * h * 4)
        // The band's trailing end is past "Rooms": the backdrop there.
        let corner = ((h / 2) * w + (w - 4)) * 4
        let bg = (Int(p[corner]), Int(p[corner + 1]), Int(p[corner + 2]))
        var ink = 0
        for i in 0..<(w * h) {
            let o = i * 4
            if abs(Int(p[o]) - bg.0) + abs(Int(p[o + 1]) - bg.1) + abs(Int(p[o + 2]) - bg.2) > 120 { ink += 1 }
        }
        return ink
    }

    func testTheLargeTitleStaysAsTheContentUnderTheBarChangesInPlace() throws {
        let session = try fixture()
        let nav = try XCTUnwrap(session.presenter.navigation.primaryNavigation)
        let route = try XCTUnwrap(nav.topViewController)
        XCTAssertEqual(route.navigationItem.title, "Rooms")
        XCTAssertEqual(route.navigationItem.largeTitleDisplayMode, .always)
        XCTAssertTrue(route.contentScrollView(for: .top) is GroupedScroller, "the title collapses with the list")
        XCTAssertGreaterThan(try titleInk(nav), 100, "'Rooms' is drawn large under the bar")

        // A failed load's reason between the header and the list: nothing
        // collapses the title, and it stays where it was, drawn.
        try tap("fail")
        XCTAssertNotNil(find("rooms-error"), "the reason shows")
        XCTAssertNil(route.contentScrollView(for: .top), "the list no longer follows the header, so nothing collapses the title")
        XCTAssertEqual(route.navigationItem.largeTitleDisplayMode, .always)
        XCTAssertGreaterThan(try titleInk(nav), 100, "'Rooms' is still drawn with the reason above the list")

        // The list back under the header takes the title again.
        try tap("recover")
        XCTAssertNil(find("rooms-error"))
        XCTAssertTrue(route.contentScrollView(for: .top) is GroupedScroller, "the list collapses the title again")
        XCTAssertGreaterThan(try titleInk(nav), 100, "'Rooms' is drawn once the list is back")

        // An error view in the list's place: the title stays.
        try tap("lose")
        XCTAssertNotNil(find("rooms-gone"), "the error view took the list's place")
        XCTAssertGreaterThan(try titleInk(nav), 100, "'Rooms' is drawn over the error view")
    }
}
#endif
