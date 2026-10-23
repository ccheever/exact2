#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class SymbolReloadIOSTests: XCTestCase {
    func testAReloadBeforeIntrinsicDeliveryKeepsTheSymbolsSize() throws {
        let p = Presenter()
        defer { p.reset() }
        let delivered = expectation(description: "the reloaded symbol reports its natural size")
        let expected = try XCTUnwrap(SymbolMeasure.size("camera", points: 20, weight: 400))
        p.onIntrinsic = { sizes in
            XCTAssertEqual(sizes.count, 1)
            XCTAssertEqual(sizes.first?.0, 1)
            XCTAssertEqual(sizes.first?.1, expected)
            delivered.fulfill()
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "image", "props": ["imageSource": "symbol:sf/camera", "symbolName": "camera"], "style": ["font_size": 20]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 0.0, "h": 0.0],
        ]))
        p.assetChanged("symbol:sf/camera")
        p.assetChanged("symbol:sf/camera")
        wait(for: [delivered], timeout: 2)
    }
}
#endif
