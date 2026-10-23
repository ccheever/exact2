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
        let cases: [(CGFloat, CGFloat)] = [
            (11, 13), (12, 15), (13, 16), (14, 17),
            (15, 18), (16, 18), (17, 20), (20, 23),
            (24, 28), (28, 33), (34, 40),
        ]
        for (size, height) in cases {
            let font = engine.font(size: size, weight: 400, family: 0, italic: false)
            let p = paragraph(engine, "Hxg\nHxg", size: size)
            XCTAssertEqual(p.height, height * 2, "\(size)px")
            XCTAssertEqual(p.firstBaseline, font.ascender, accuracy: 1e-6, "UIKit's baseline at \(size)px")
            XCTAssertEqual(p.baselines[1] - p.baselines[0], height, accuracy: 1e-6, "Chrome's line advance at \(size)px")
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
