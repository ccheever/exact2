#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1059 D2a on AppKit: a tablist projected to `NSSegmentedControl`
/// reports the control's height as its automatic minimum, as on iOS, so an
/// unsized row of text tabs does not squash the control.
final class SegmentsMacTests: XCTestCase {
    private func drain() {
        let delivered = expectation(description: "intrinsic size delivered after the batch")
        DispatchQueue.main.async { delivered.fulfill() }
        wait(for: [delivered], timeout: 2)
    }

    func testSegmentsReportNativeHeightWithoutRepeatedOrStaleMeasurements() throws {
        _ = NSApplication.shared
        let p = Presenter()
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false
        w.contentView = p.viewport
        defer { p.reset(); w.close() }
        var reports: [CGSize?] = []
        p.onIntrinsic = { sizes in
            for (id, size) in sizes where id == 10 { reports.append(size) }
        }
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "tablist"]],
            ["op": "create", "id": 11, "kind": "button", "props": ["accessibilityRole": "tab", "accessibilitySelected": "true"], "handlers": ["press"]],
            ["op": "create", "id": 12, "kind": "button", "props": ["accessibilityRole": "tab"], "handlers": ["press"]],
            ["op": "create", "id": 14, "kind": "text", "props": ["text": "One"]],
            ["op": "create", "id": 16, "kind": "text", "props": ["text": "Two"]],
            ["op": "children", "id": 11, "ids": [14]],
            ["op": "children", "id": 12, "ids": [16]],
            ["op": "children", "id": 10, "ids": [11, 12]],
            ["op": "roots", "ids": [10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 400.0, "h": 16.0],
        ]))
        let owner = try XCTUnwrap(p.views[10])
        let control = try XCTUnwrap(p.segments.control(of: 10))
        drain()
        let size = try XCTUnwrap(reports.last ?? nil)
        XCTAssertEqual(size, control.intrinsicContentSize, "the control's own size, not the box's")
        XCTAssertGreaterThan(size.height, 16, "an unsized text row must not squash the native control")
        XCTAssertEqual(control.frame, owner.contentBox(), "the kernel still owns the final box")
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0,
                            "w": 400.0, "h": Double(size.height)]]))
        drain()
        let count = reports.count
        p.segments.sync()
        drain()
        XCTAssertEqual(reports.count, count, "unchanged native sizes are not published again")
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0,
                            "w": 300.0, "h": Double(size.height)]]))
        drain()
        XCTAssertEqual(reports.count, count, "its height does not follow the box's width, so a resize does not remeasure")
        XCTAssertEqual(control.frame, owner.contentBox())
        // The control fills the content box: a border-box minimum also holds
        // the padding and border around it; a content-box one does not.
        p.apply(wireBatch([["op": "style", "id": 10,
                            "style": ["box_sizing": "border-box", "padding_top": 6.0, "padding_bottom": 4.0]]]))
        drain()
        XCTAssertEqual((reports.last ?? nil)?.height, size.height + 10)
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0,
                            "w": 300.0, "h": Double(size.height + 10)]]))
        XCTAssertEqual(control.frame, owner.contentBox())
        XCTAssertEqual(control.frame.height, size.height, "the padding does not squash the control")
        p.apply(wireBatch([["op": "style", "id": 10,
                            "style": ["box_sizing": "content-box", "padding_top": 6.0, "padding_bottom": 4.0]]]))
        drain()
        XCTAssertEqual((reports.last ?? nil)?.height, size.height)
        // Measured at zero width too: tabs replaced there still reserve it.
        p.apply(wireBatch([["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 0.0, "h": 0.0],
                           ["op": "children", "id": 10, "ids": [12, 11]]]))
        drain()
        XCTAssertEqual((reports.last ?? nil)?.height, size.height, "a zero-width tablist keeps its native minimum")
        let replaced = try XCTUnwrap(p.segments.control(of: 10))
        p.apply(wireBatch([["op": "props", "id": 10,
                            "set": ["accessibilityRole": "tablist", "accessibilityOrientation": "vertical"]]]))
        drain()
        XCTAssertNil(reports.last ?? nil, "leaving the projection clears its native minimum")
        XCTAssertNil(control.superview)
        XCTAssertNil(replaced.superview)
        XCTAssertFalse(try XCTUnwrap(p.views[11]).isHidden)
    }
}
#endif
