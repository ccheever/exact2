#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class PageScrollMacTests: XCTestCase {
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
        p.apply(Batch(ops: [
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
