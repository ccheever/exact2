#if os(iOS)
import XCTest
import UIKit
import CoreText
import CExact
@testable import ExactKit

/// Chrome with the simulator's SFUI.ttf loaded as a web font, at scale 1.
/// UIKit controls keep their own fonts and metrics (SessionControlTextIOS).
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
    func testNormalLineHeightUsesChromesRuleForTheSameFace() {
        let engine = TextEngine(resolve: { _ in nil })
        let cases: [(CGFloat, CGFloat, CGFloat)] = [
            (11, 13, 10), (12, 14, 11), (13, 15, 12), (14, 16, 13),
            (15, 18, 14), (16, 19, 15), (17, 20, 16), (20, 24, 19),
            (24, 29, 23), (28, 34, 27), (34, 40, 32),
        ]
        for (size, height, baseline) in cases {
            let font = engine.font(size: size, weight: 400, family: 0, italic: false) as CTFont
            print("SFUI \(size): UIKit \(CTFontGetAscent(font) + CTFontGetDescent(font) + CTFontGetLeading(font)), Chrome \(height)")
            let p = paragraph(engine, "Hxg\nHxg", size: size)
            XCTAssertEqual(p.height, height * 2, "\(size)px")
            XCTAssertEqual(p.firstBaseline, baseline, "\(size)px")
            XCTAssertEqual(p.baselines[1] - p.baselines[0], height, "\(size)px")
        }
    }

    func testIntrinsicWidthsAreChromes() { assertChromeWidths() }
}
#endif
