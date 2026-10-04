#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class PaintCaptureMacTests: XCTestCase {
    func testRanksNegativeChildrenAndOpacityAreComposedOnce() throws {
        _ = NSApplication.shared
        let p = Presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": 2, "kind": "view", "style": ["background_color": [255, 0, 0, 255]]],
            ["op": "create", "id": 3, "kind": "view", "style": ["background_color": [0, 0, 255, 255]]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 80, "h": 80],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 80, "h": 80],
            ["op": "frame", "id": 3, "x": 0, "y": 0, "w": 80, "h": 80],
            ["op": "rank", "id": 2, "rank": 1],
        ]))
        let root = try XCTUnwrap(p.views[1])
        let window = NSWindow(contentRect: root.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = root
        root.displayIfNeeded()
        let liveParent = p.views[2]?.layer?.superlayer
        func color() throws -> NSColor {
            let rep = try XCTUnwrap(Capture.paintOrderBitmap(of: root))
            return try XCTUnwrap(rep.colorAt(x: rep.pixelsWide / 2, y: rep.pixelsHigh / 2))
        }
        XCTAssertGreaterThan(try color().redComponent, 0.95)
        p.views[2]?.frame = CGRect(x: 0, y: 0, width: 80, height: 20)
        let oriented = try XCTUnwrap(Capture.paintOrderBitmap(of: root))
        XCTAssertGreaterThan(try XCTUnwrap(oriented.colorAt(x: 10, y: 10)).redComponent, 0.95)
        XCTAssertGreaterThan(try XCTUnwrap(oriented.colorAt(x: 10, y: oriented.pixelsHigh - 10)).blueComponent, 0.95)
        p.views[2]?.frame = root.bounds
        p.views[2]?.alphaValue = 0.5
        let mixed = try color()
        XCTAssertEqual(mixed.redComponent, 0.5, accuracy: 0.02)
        XCTAssertEqual(mixed.blueComponent, 0.5, accuracy: 0.02)
        p.views[2]?.setRank(-2)
        XCTAssertGreaterThan(try color().blueComponent, 0.95)
        p.views[3]?.isHidden = true
        let negative = try color()
        XCTAssertEqual(negative.redComponent, 1, accuracy: 0.02)
        XCTAssertEqual(negative.greenComponent, 0.5, accuracy: 0.02, "negative child still paints above its parent's background")
        XCTAssertEqual(root.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 3])
        XCTAssertTrue(p.views[2]?.layer?.superlayer === liveParent, "capture leaves live layers in place")
    }
}
#endif
