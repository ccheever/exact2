import Foundation
import CExact
import XCTest
@testable import ExactKit

final class CalculatedLengthTests: XCTestCase {
    func testStyleCalculationsDecodeInEitherPropertyOrder() throws {
        let text = Array("min(50%, 10vw)".utf8)
        let handle = text.withUnsafeBufferPointer { exact_calc_parse($0.baseAddress, $0.count) }
        let rows = "\"border_radius_top_left\":{\"calc\":\(handle),\"env\":[0,0,0,0,300,600,1,1,0]}"
        for op in ["{\"op\":\"style\",\"id\":1,\"style\":{\(rows)}}",
                   "{\"style\":{\(rows)},\"id\":1,\"op\":\"style\"}"] {
            let batch = Batch.decode(Data("{\"ops\":[\(op)]}".utf8))
            XCTAssertNil(batch.error)
            let radius = try XCTUnwrap(batch.ops.first?.style["border_radius_top_left"])
            guard case .calculation = radius else { return XCTFail("style calculation was not bound") }
            XCTAssertEqual(radius.length(basis: 400), 30)
        }
    }

    func testSnapshotsSurviveOtherWindowsAndAreReleasedWithTheirValues() throws {
        let text = Array("min(50%, 10vw)".utf8)
        let handle = text.withUnsafeBufferPointer { exact_calc_parse($0.baseAddress, $0.count) }
        let decoder = JSONDecoder()
        func decode(_ width: Int) throws -> BatchValue {
            let json = "{\"radius\":{\"calc\":\(handle),\"env\":[0,0,0,0,\(width),600,1,1,0]}}"
            return try XCTUnwrap(decoder.decode(NodeStyle.self, from: Data(json.utf8))["radius"])
        }
        var first: BatchValue? = try decode(300)
        weak var owned: CalculatedLength?
        if case .calculation(let c) = first { owned = c }
        XCTAssertNotNil(owned)
        for width in 400..<1400 {
            let other = try decode(width)
            XCTAssertEqual(try XCTUnwrap(other.length(basis: 400)), min(200, CGFloat(width) / 10), accuracy: 0.0001)
            XCTAssertEqual(first?.length(basis: 400), 30)
        }
        first = nil
        XCTAssertNil(owned)
    }
}
