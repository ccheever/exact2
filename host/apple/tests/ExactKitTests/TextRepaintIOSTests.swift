#if os(iOS)
import XCTest
@testable import ExactKit
import UIKit

/// LLP 1116.005 D4: a text that changes or leaves is repainted on the commit
/// that changes it, inside a stack that scrolls (`scripts/fixtures/repaint.contract`).
/// Stopwatch's status line became a collapsed space on Reset: its box laid
/// out to nothing, which neither the text pump nor UIKit's `draw(_:)` visits,
/// and its old raster stayed on screen beside 00:00.00, starting at the
/// empty box's corner, though the accessibility tree had dropped it.
final class TextRepaintIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; window?.isHidden = true; window = nil; super.tearDown() }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func booted() throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_REPAINT_PLAN"], "build.mjs --test --ios compiles the repaint fixture")
        let session = ExactApp.shared.makeSession(label: "text-repaint")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        return session
    }

    private func node(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId && $0.window != nil }, "no \(testId)")
    }

    /// The page's pixels in `rect` (its own space) as its layers hold them,
    /// a text's raster layer included, over nothing: alpha is ink.
    private func pixels(_ page: NodeView, _ rect: CGRect) -> [UInt8] {
        let w = Int(rect.width.rounded(.up)), h = Int(rect.height.rounded(.up))
        guard w > 0, h > 0, let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
              let raw = ctx.data else { return [] }
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: 1, y: -1)
        ctx.translateBy(x: -rect.minX, y: -rect.minY)
        page.layer.render(in: ctx)
        return Array(UnsafeBufferPointer(start: raw.bindMemory(to: UInt8.self, capacity: w * h * 4), count: w * h * 4))
    }
    private func ink(_ page: NodeView, _ rect: CGRect) -> Int {
        let p = pixels(page, rect)
        return stride(from: 3, to: p.count, by: 4).filter { p[$0] > 16 }.count
    }

    func testATextThatChangesOrLeavesIsRepaintedOnTheCommit() throws {
        let session = try booted()
        let p = session.presenter
        let page = try node("page"), status = try node("status"), count = try node("count"), lap = try node("lap")
        XCTAssertNotNil(page.scroll, "the stack scrolls")
        p.paintVisibleText()
        spin(0.2)
        // Each line's box in the page, a little wider than its text.
        let rects = [status, count, lap].map { page.convert($0.bounds, from: $0).insetBy(dx: -8, dy: -2) }
        XCTAssertGreaterThan(ink(page, rects[0]), 50, "'3 laps' is painted before Reset")
        XCTAssertGreaterThan(ink(page, rects[2]), 50, "'Lap 3' is painted before Reset")
        let countBefore = pixels(page, rects[1])

        _ = Agent(session: session).tap(["id": Int(try node("reset").id)])
        spin(0.3)
        p.paintVisibleText()
        spin(0.2)

        // The status line is a collapsed space now: its box has no width.
        XCTAssertEqual(status.bounds.width, 0, "a collapsed space lays out to nothing")
        XCTAssertNil(status.textRasterLayer?.superlayer, "the empty status line keeps no raster layer")
        XCTAssertEqual(ink(page, rects[0]), 0, "'3 laps' is gone from the screen with the text")
        // The `if`'s line left the page, and its pixels with it.
        XCTAssertNil(lap.window, "the `if` removed 'Lap 3'")
        XCTAssertEqual(ink(page, rects[2]), 0, "'Lap 3' is gone from the screen")
        // The count shows its new text, not its old pixels.
        XCTAssertGreaterThan(ink(page, rects[1]), 50, "the count is painted after Reset")
        XCTAssertNotEqual(pixels(page, rects[1]), countBefore, "the count's pixels are its new text's")
        if let key = count.textRasterKey { XCTAssertEqual(key.spec.runs.map(\.text).joined(), "0 in all") }
    }
}
#endif
