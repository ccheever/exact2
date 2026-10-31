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

    /// A view leaving a symbol clears the symbol's size at once: a second
    /// load in the same turn must not drop the clear (the raster may never report).
    func testLeavingASymbolClearsItsSizeThoughASecondLoadFollows() throws {
        let p = Presenter()
        defer { p.reset() }
        var reports: [CGSize?] = []
        let measured = expectation(description: "the symbol reports its size")
        p.onIntrinsic = { sizes in
            reports += sizes.filter { $0.0 == 1 }.map(\.1)
            if reports.count == 1 { measured.fulfill() }
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "image", "props": ["imageSource": "symbol:sf/camera", "symbolName": "camera"], "style": ["font_size": 20]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 0.0, "h": 0.0],
        ]))
        wait(for: [measured], timeout: 2)
        let view = try XCTUnwrap(p.views[1])
        view.loadImage("a.png")
        view.loadImage("b.png")
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        XCTAssertEqual(reports.count, 2)
        XCTAssertEqual(reports.last, .some(nil))
    }
}
#endif
