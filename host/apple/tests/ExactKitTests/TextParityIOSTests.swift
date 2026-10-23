#if os(iOS)
import XCTest
import UIKit
import CoreText
import CExact
@testable import ExactKit

/// Chrome system-ui at scale 1, as the web scroll fixture lays it out.
/// FieldChromeIOS keeps UIFont.lineHeight for native editors.
final class TextParityIOSTests: XCTestCase {
    func testRuntimeInstalledFamilyUsesItsOwnFace() throws {
        guard !TextEngine.installedFaces("Menlo").isEmpty else { throw missing("fonts", "Menlo unavailable") }
        let engine = TextEngine(resolve: { _ in nil })
        let name = Array("Menlo".utf8)
        name.withUnsafeBufferPointer { n in
            var face = ExactFontFace()
            face.family = n.baseAddress; face.family_len = n.count; face.stack = 8
            withUnsafePointer(to: &face) { row in
                var catalog = ExactFontCatalog()
                catalog.faces = row; catalog.count = 1
                withUnsafePointer(to: &catalog) { engine.install($0) }
            }
        }
        let font = engine.font(size: 20, weight: 700, family: 8, italic: false) as CTFont
        XCTAssertEqual(CTFontCopyFamilyName(font) as String, "Menlo")
    }
    func testNormalLineHeightIsTheWebHostsSystemUI() {
        let engine = TextEngine(resolve: { _ in nil })
        let cases: [(CGFloat, CGFloat, CGFloat)] = [
            (11, 13, 11), (12, 15, 12), (13, 16, 13), (14, 17, 14),
            (15, 18, 15), (16, 18, 15), (17, 20, 16), (20, 23, 19),
            (24, 28, 23), (28, 33, 27), (34, 40, 33),
        ]
        for (size, height, baseline) in cases {
            let p = paragraph(engine, "Hxg\nHxg", size: size)
            XCTAssertEqual(p.height, height * 2, "\(size)px")
            XCTAssertEqual(p.firstBaseline, baseline, "\(size)px")
            XCTAssertEqual(p.baselines[1] - p.baselines[0], height, "\(size)px")
        }
    }

    func testFortyNormal14pxRowsKeepTheSharedScrollLimit() {
        let engine = TextEngine(resolve: { _ in nil })
        let rows = (0..<40).map { paragraph(engine, "Row \($0)", size: 14).height }.reduce(0, +)
        XCTAssertEqual(rows + 39 * 4 + 16 - 200, 652)
    }

    func testIntrinsicWidthsAreChromes() { assertChromeWidths() }
}
#endif
