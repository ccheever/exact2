#if os(iOS)
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists
import UIKit

/// LLP 1116.006 D1 over `scripts/fixtures/bar-band.contract`: team-kanban's
/// shape, a large-title header, a persona switcher (a segmented control) and
/// a line of text, then a grouped list. Round 8's switcher lay under the
/// large title, drawn over by it, and took no tap. As on a hand-built screen
/// (a control pinned to the safe area over a table), it lies below the bar,
/// its large title included, takes a tap, and follows the bar as the title
/// collapses with the list beneath it.
final class BarBandIOSTests: XCTestCase {
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
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_BAR_BAND_PLAN"], "build.mjs --test --ios compiles the bar-band fixture")
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "bar-band")
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

    private func node(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId && $0.window != nil }, "no \(testId)")
    }
    private func box(_ v: UIView) -> CGRect { v.convert(v.bounds, to: nil) }

    /// The switcher's segmented control and the text under it lie wholly
    /// below the bar, and a touch at the control's middle reaches it.
    private func assertBelow(_ nav: UINavigationController, _ control: UIView, file: StaticString = #filePath, line: UInt = #line) throws {
        let bar = box(nav.navigationBar).maxY
        XCTAssertGreaterThanOrEqual(box(control).minY, bar - 0.5, "the switcher below the bar (\(bar)): \(box(control))", file: file, line: line)
        XCTAssertGreaterThanOrEqual(box(try node("me")).minY, bar - 0.5, "the text below the bar too", file: file, line: line)
        let r = box(control), hit = window?.hitTest(CGPoint(x: r.midX, y: r.midY), with: nil)
        XCTAssertTrue(hit.map { $0 === control || $0.isDescendant(of: control) } ?? false, "the switcher takes a touch at its middle, not \(String(describing: hit))", file: file, line: line)
    }

    /// Pixels unlike the backdrop in the bar's large-title band (its bottom
    /// 52 points), as the window's layers hold them, clips included.
    private func titleInk(_ nav: UINavigationController) throws -> Int {
        let window = try XCTUnwrap(self.window)
        let bar = box(nav.navigationBar)
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
        let corner = ((h / 2) * w + (w - 4)) * 4
        let bg = (Int(p[corner]), Int(p[corner + 1]), Int(p[corner + 2]))
        return (0..<(w * h)).filter { i in
            let o = i * 4
            return abs(Int(p[o]) - bg.0) + abs(Int(p[o + 1]) - bg.1) + abs(Int(p[o + 2]) - bg.2) > 120
        }.count
    }

    func testTheContentBetweenTheBarAndTheListSitsBelowTheBarAndFollowsIt() throws {
        let session = try fixture()
        let p = session.presenter
        let nav = try XCTUnwrap(p.navigation.primaryNavigation)
        let route = try XCTUnwrap(nav.topViewController)
        XCTAssertEqual(route.navigationItem.title, "Team Board")
        XCTAssertEqual(route.navigationItem.largeTitleDisplayMode, .always)
        let control = try XCTUnwrap(p.segments.control(of: try node("persona").id), "the tablist is a segmented control")
        let list = try XCTUnwrap(p.navigation.shownScroll(try node("board")))
        XCTAssertTrue(route.contentScrollView(for: .top) === list, "the title collapses with the list, as UIKit follows a hand-built screen's table")

        // At rest the large title shows, and the switcher is under it, not
        // beneath it.
        let expanded = box(nav.navigationBar).maxY
        XCTAssertGreaterThan(expanded, route.view.window!.safeAreaInsets.top + 80, "the large title is shown: the bar ends at \(expanded)")
        try assertBelow(nav, control)
        XCTAssertEqual(box(control).minY, expanded + 8, accuracy: 1, "the row's own top padding under the bar, as authored")
        XCTAssertLessThan(box(list).minY, expanded + 120, "the list right under the switcher and the text")
        XCTAssertGreaterThan(try titleInk(nav), 100, "'Team Board' is drawn large in the bar, not taken into the list")

        // Its tap is the app's.
        _ = Agent(session: session).tap(["id": Int(try node("persona-grace").id)])
        spin(0.5)
        let me = try node("me").accessibleText
        XCTAssertTrue(me.contains("Grace"), "the switcher acts: \(me)")

        // The list scrolls; the title collapses with it, and the switcher
        // follows the bar up, still wholly below it.
        list.contentOffset.y = -list.adjustedContentInset.top + 300
        spin(0.6)
        let collapsed = box(nav.navigationBar).maxY
        XCTAssertLessThan(collapsed, expanded - 30, "the title collapsed: the bar ends at \(collapsed), not \(expanded)")
        try assertBelow(nav, control)
        XCTAssertEqual(box(control).minY, collapsed + 8, accuracy: 1, "the switcher followed the bar")

        // Back past the list's start, as a fling's bounce goes, the title
        // and the switcher are as they were (a hand-built screen's title
        // expands there too, not at the start itself).
        list.contentOffset.y = -list.adjustedContentInset.top - 30
        spin(0.4)
        list.contentOffset.y = -list.adjustedContentInset.top
        spin(0.6)
        XCTAssertEqual(box(nav.navigationBar).maxY, expanded, accuracy: 1, "the title expanded again")
        try assertBelow(nav, control)
        XCTAssertEqual(box(control).minY, expanded + 8, accuracy: 1)
    }
}
#endif
