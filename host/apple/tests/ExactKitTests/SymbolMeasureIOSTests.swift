import CExact
import XCTest
@testable import ExactKit

/// The kernel's symbol measurer (`exact_set_symbol_measure`): a glyph's box,
/// an em square for a name the OS has no glyph for, and no room at 0 points,
/// which is a size, not a refusal to measure. UIKit and AppKit alike.
final class SymbolMeasureIOSTests: XCTestCase {
    private func measure(_ name: String, points: Float) -> (UInt8, [Float]) {
        var out: [Float] = [-1, -1]
        let bytes = Array(name.utf8)
        let answered = bytes.withUnsafeBufferPointer { name in
            out.withUnsafeMutableBufferPointer { SymbolMeasure.measure(nil, name.baseAddress, name.count, points, 400, $0.baseAddress) }
        }
        return (answered, out)
    }

    func testAGlyphIsMeasuredAtItsPointSize() {
        let (answered, size) = measure("gearshape", points: 20)
        XCTAssertEqual(answered, 1)
        let glyph = SymbolMeasure.size("gearshape", points: 20, weight: 400)
        XCTAssertEqual(size, [Float(glyph!.width), Float(glyph!.height)])
        XCTAssertGreaterThan(size[0], 0)
    }

    func testANameWithNoGlyphIsAnEmSquare() {
        XCTAssertEqual(measure("no.such.symbol.example", points: 20).1, [20, 20])
    }

    func testZeroPointsTakeNoRoom() {
        let (answered, size) = measure("gearshape", points: 0)
        XCTAssertEqual(answered, 1, "a known size, so the layout does not keep the last one")
        XCTAssertEqual(size, [0, 0])
    }
}
