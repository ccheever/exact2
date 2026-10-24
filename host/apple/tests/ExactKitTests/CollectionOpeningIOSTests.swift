#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class CollectionOpeningIOSTests: XCTestCase {
    func testOpeningReportsOnlyTheVisibleWindowBeforeLaterFillSlices() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        defer { p.reset(); window.isHidden = true }
        var wires: [Data] = []
        p.collections.onFeedback = { wires.append($0) }
        p.apply(wireBatch([
            ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 100,
                "totalExtent": 5000, "pending": false, "rows": [["view": 10, "root": 10, "epoch": 1, "index": 0]], "correction": NSNull()]]],
            ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 10, "kind": "view"],
            ["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 300.0, "h": 50.0],
            ["op": "content", "id": 1, "w": 300.0, "h": 5000.0],
        ]))
        p.collections.flush()
        func limit(_ wire: Data) -> UInt32 {
            wire.subdata(in: 88..<92).withUnsafeBytes { $0.loadUnaligned(as: UInt32.self) }
        }
        XCTAssertEqual(limit(try XCTUnwrap(wires.first)), 0, "opening rescues what shows without building the offscreen lead")
        wires.removeAll()
        p.collections.changed(1)
        p.collections.flush()
        XCTAssertTrue(wires.isEmpty, "unchanged geometry with no pending lead needs no report")
        p.collections.fillSlice(1, limit: 3)
        XCTAssertEqual(limit(try XCTUnwrap(wires.first)), 3, "a later slice can build its bounded lead")
    }
}
#endif
