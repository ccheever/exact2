import XCTest
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
@testable import ExactKit

final class DocumentOverflowTests: XCTestCase {
    func testTransformedDescendantsExtendTheDocumentUntilAClippingBox() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 200, "h": 200],
            ["op": "frame", "id": 2, "x": 0, "y": 150, "w": 100, "h": 100],
            ["op": "content", "id": 1, "w": 200, "h": 250],
            ["op": "present", "id": 2, "property": "translate", "x": 0, "y": 250]
        ]))
        XCTAssertEqual(p.root.frame.height, 500, accuracy: 0.01)
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["overflow_x": "hidden", "overflow_y": "hidden"]]]))
        XCTAssertEqual(p.root.frame.height, 200, accuracy: 0.01)
    }

    func testRootRotationBringsNegativeOverflowIntoTheDocument() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 200, height: 200)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 200, "h": 200],
            ["op": "frame", "id": 2, "x": 0, "y": -100, "w": 100, "h": 100],
            ["op": "content", "id": 1, "w": 200, "h": 200],
            ["op": "present", "id": 1, "property": "rotate", "x": 180]
        ]))
        XCTAssertEqual(p.root.frame.height, 300, accuracy: 0.01)
    }
}
