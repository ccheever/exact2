#if os(macOS)
import AppKit
import XCTest
import ObjectiveC
@testable import ExactKit

final class PageScrollMacTests: XCTestCase {
    private static var preferredStyle: NSScroller.Style = .overlay
    @objc private class func forcedScrollerStyle() -> NSScroller.Style { preferredStyle }

    func testPreferredStyleChangesRetileAndRefitThePage() throws {
        _ = NSApplication.shared
        let original = try XCTUnwrap(class_getClassMethod(NSScroller.self, #selector(getter: NSScroller.preferredScrollerStyle)))
        let replacement = try XCTUnwrap(class_getClassMethod(Self.self, #selector(Self.forcedScrollerStyle)))
        method_exchangeImplementations(original, replacement)
        defer { method_exchangeImplementations(original, replacement) }
        Self.preferredStyle = .overlay
        let p = Presenter()
        p.viewport.frame.size = NSSize(width: 600, height: 400)
        let child = FlippedView(frame: NSRect(x: 0, y: 0, width: 600, height: 1000))
        p.root.addSubview(child)
        p.fitDocument()
        var sizes: [NSSize] = []
        p.viewport.visibleSizeChanged = {
            sizes.append(p.viewportSize)
            child.frame.size.width = p.viewportSize.width
            p.fitDocument()
        }
        for style in [NSScroller.Style.legacy, .overlay, .legacy] {
            Self.preferredStyle = style
            NotificationCenter.default.post(name: NSScroller.preferredScrollerStyleDidChangeNotification, object: nil)
            XCTAssertEqual(p.viewport.scrollerStyle, style)
            XCTAssertEqual(child.frame.width, p.viewportSize.width)
            XCTAssertEqual(p.root.frame.width, p.viewportSize.width)
            XCTAssertTrue(p.viewport.horizontalScroller!.isHidden, "a fitting page has no sideways scroll")
            XCTAssertEqual(p.viewportSize.width < 600, style == .legacy)
        }
        XCTAssertFalse(sizes.isEmpty)
    }

    func testOverflowingRootUsesTheVisibleViewportAtLaunchAndAfterResize() throws {
        _ = NSApplication.shared
        let original = try XCTUnwrap(class_getClassMethod(NSScroller.self, #selector(getter: NSScroller.preferredScrollerStyle)))
        let replacement = try XCTUnwrap(class_getClassMethod(Self.self, #selector(Self.forcedScrollerStyle)))
        method_exchangeImplementations(original, replacement)
        defer { method_exchangeImplementations(original, replacement) }
        Self.preferredStyle = .legacy
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = """
        shape Viewport
          width: number
          height: number
        component App
          resource viewport = exactViewport() as shape Viewport
          view
            main width="100%" height="100%" padding=20
              text `${viewport.width}x${viewport.height}` testId="size"

        """
        let contract = dir.appendingPathComponent("app.contract"), plan = dir.appendingPathComponent("app.plan")
        try source.write(to: contract, atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", contract.path, "-o", plan.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let session = ExactApp.shared.makeSession(label: "page-viewport")
        let initial = NSSize(width: 1000, height: 700)
        XCTAssertNil(session.boot(plan: try Data(contentsOf: plan), size: initial).error)
        let view = ExactView(session: session)
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: initial), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = view
        defer { window.close(); session.destroy() }
        let p = session.presenter
        for style in [NSScroller.Style.overlay, .legacy] {
            Self.preferredStyle = style
            NotificationCenter.default.post(name: NSScroller.preferredScrollerStyleDidChangeNotification, object: nil)
            let visible = p.viewportSize
            let label = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "size" })
            XCTAssertEqual(label.props["text"], "\(Int(visible.width))x\(Int(visible.height))", "style changes update the resource without a resize")
        }
        for size in [initial, NSSize(width: 800, height: 600), initial] {
            window.setContentSize(size)
            view.layoutSubtreeIfNeeded()
            let visible = p.viewportSize
            let label = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "size" })
            XCTAssertEqual(label.props["text"], "\(Int(visible.width))x\(Int(visible.height))")
            if ExactEnv.agentMode { XCTAssertEqual(visible, size, "the agent keeps overlay even with overflow") }
            XCTAssertEqual(p.root.subviews.first?.frame.width, visible.width + 40)
        }
    }

    private func fixture(xOverflow: Bool = false, yOverflow: Bool = false) -> (Presenter, NSView) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.scrollerStyle = .legacy
        p.viewport.frame = NSRect(x: 0, y: 0, width: 960, height: 900)
        let child = FlippedView(frame: .zero)
        p.root.addSubview(child)
        // Resolve the initial real single-axis gutter before testing a resize.
        for _ in 0..<3 {
            child.frame.size = NSSize(width: xOverflow ? 1200 : p.viewportSize.width,
                                      height: yOverflow ? 1100 : p.viewportSize.height)
            p.fitDocument()
        }
        return (p, child)
    }

    func testTwoShrinksDoNotOfferTransientGuttersForPreviouslyFittingDocument() {
        let (p, child) = fixture()
        for size in [NSSize(width: 956, height: 897), NSSize(width: 952, height: 894)] {
            let authored = child.frame
            p.viewport.setFrameSize(size)
            XCTAssertEqual(p.viewportSize, size, "baseline offers a false narrow viewport before fitting its old document")
            XCTAssertEqual(child.frame, authored, "prefit changes the document wrapper, never an authored root")
            XCTAssertTrue(p.viewport.verticalScroller!.isHidden)
            XCTAssertTrue(p.viewport.horizontalScroller!.isHidden)
            child.frame.size = size
            p.fitDocument()
        }
    }

    func testSingleAxisOverflowKeepsNeededGutterWithoutCreatingOtherGutter() {
        for x in [false, true] {
            let (p, child) = fixture(xOverflow: x, yOverflow: !x)
            let before = p.viewportSize
            let authored = child.frame
            p.viewport.setFrameSize(NSSize(width: 956, height: 897))
            XCTAssertEqual(p.viewportSize, NSSize(width: before.width - 4, height: before.height - 3))
            XCTAssertEqual(child.frame, authored)
            XCTAssertEqual(p.viewport.horizontalScroller!.isHidden, !x)
            XCTAssertEqual(p.viewport.verticalScroller!.isHidden, x)
            XCTAssertEqual(x ? p.root.frame.width : p.root.frame.height, x ? 1200 : 1100)
        }
    }

    func testBothOverflowAndNonzeroScrollRemainOwnedByAppKit() {
        let (p, child) = fixture(xOverflow: true, yOverflow: true)
        p.viewport.contentView.scroll(to: NSPoint(x: 31, y: 47))
        p.viewport.reflectScrolledClipView(p.viewport.contentView)
        let before = p.viewportSize
        p.viewport.setFrameSize(NSSize(width: 956, height: 897))
        XCTAssertEqual(p.viewportSize, NSSize(width: before.width - 4, height: before.height - 3))
        XCTAssertEqual(p.root.frame.size, NSSize(width: 1200, height: 1100))
        XCTAssertEqual(child.frame.size, p.root.frame.size)
        XCTAssertEqual(p.viewport.contentView.bounds.origin, NSPoint(x: 31, y: 47))
    }

    func testNewRealOverflowAfterAcceptedWidthMayStillCorrectViewport() {
        let (p, child) = fixture()
        p.viewport.setFrameSize(NSSize(width: 952, height: 894))
        XCTAssertEqual(p.viewportSize, NSSize(width: 952, height: 894))
        // This represents a real new layout result, not the baseline's spurious
        // 939-wide first sample triggering the width-sensitive model early.
        child.frame.size = NSSize(width: 952, height: 1100)
        p.fitDocument()
        XCTAssertFalse(p.viewport.verticalScroller!.isHidden)
        XCTAssertEqual(child.frame.height, 1100)
        XCTAssertLessThan(p.viewportSize.width, 952, "a newly necessary gutter must not be suppressed")
        child.frame.size.width = p.viewportSize.width
        p.fitDocument()
        XCTAssertEqual(p.root.frame.height, 1100)
        XCTAssertFalse(p.viewport.verticalScroller!.isHidden)
        XCTAssertTrue(p.viewport.horizontalScroller!.isHidden)
    }

    func testStyleChangeAndUnacceptedDocumentChangeDoNotReuseCertificate() {
        let (p, _) = fixture()
        let old = p.root.frame
        p.viewport.scrollerStyle = .overlay
        p.viewport.setFrameSize(NSSize(width: 956, height: 897))
        XCTAssertEqual(p.root.frame, old, "a changed style requires ordinary layout")
        let (q, _) = fixture()
        q.root.setFrameSize(NSSize(width: 970, height: 910))
        q.viewport.setFrameSize(NSSize(width: 956, height: 897))
        XCTAssertEqual(q.root.frame.size, NSSize(width: 970, height: 910))
    }

    func testResetAndReplacementDocumentCannotUseOldCertificate() {
        let (p, _) = fixture()
        p.reset()
        let old = p.root.frame
        p.viewport.setFrameSize(NSSize(width: 956, height: 897))
        XCTAssertEqual(p.root.frame, old)
        let (q, _) = fixture()
        let replacement = FlippedView(frame: q.root.frame)
        q.viewport.documentView = replacement
        let prior = replacement.frame
        q.viewport.setFrameSize(NSSize(width: 956, height: 897))
        XCTAssertEqual(replacement.frame, prior)
    }

    func testReentrantGeometryDuringBatchDoesNotPrefitAheadOfBatch() {
        let (p, _) = fixture()
        var called = false
        p.onViewportFit = {
            called = true
            let old = p.root.frame
            p.viewport.setFrameSize(NSSize(width: 956, height: 897))
            XCTAssertEqual(p.root.frame, old)
            XCTAssertTrue(p.deferGeometry {})
        }
        p.apply(batchFixture(ops: [
            ["op": "create", "id": 1, "kind": "view", "props": ["viewportFit": "cover"]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 960.0, "h": 900.0],
            ["op": "roots", "ids": [1]]
        ], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertTrue(called)
    }

    func testNonzeroInsetsContinueToBePinnedToZero() {
        let (p, _) = fixture()
        p.viewport.contentInsets = NSEdgeInsets(top: 7, left: 11, bottom: 13, right: 17)
        p.viewport.setFrameSize(NSSize(width: 956, height: 897))
        let value = p.viewport.contentInsets
        XCTAssertEqual([value.top, value.left, value.bottom, value.right], [0, 0, 0, 0])
        XCTAssertEqual(p.viewport.scrollerStyle, .legacy)
    }
}
#endif
