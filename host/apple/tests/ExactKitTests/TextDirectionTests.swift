import XCTest
import CoreText
#if os(macOS)
import AppKit
#else
import UIKit
#endif
@testable import ExactKit

final class TextDirectionTests: XCTestCase {
    func testCSSDirectionSetsTheBaseEvenWhenTheFirstStrongCharacterDisagrees() {
        let engine = TextEngine(resolve: { _ in nil })
        for text in ["אבג abc", "العربية abc"] {
            let latin = (text as NSString).range(of: "abc").location
            var spec = Spec(runs: [Run(text: text, size: 20, weight: 400, family: 0,
                italic: false, lineHeight: nil, letterSpacing: 0)], align: 0, lineClamp: 0, color: [0, 0, 0, 255])
            let ltr = engine.paragraph(spec, width: 300)
            XCTAssertLessThan(CTLineGetOffsetForStringIndex(ltr.lines[0], 0, nil),
                CTLineGetOffsetForStringIndex(ltr.lines[0], latin, nil), text)
            spec.direction = 1
            let rtl = engine.paragraph(spec, width: 300)
            XCTAssertGreaterThan(CTLineGetOffsetForStringIndex(rtl.lines[0], 0, nil),
                CTLineGetOffsetForStringIndex(rtl.lines[0], latin, nil), text)
            spec.direction = 0
            XCTAssertEqual(engine.paragraph(spec, width: 300).width, ltr.width)
        }
    }

    func testBaseDirectionPreservesOtherParagraphAttributesAndDoesNotOverrideRuns() {
        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = .center
        paragraph.firstLineHeadIndent = 12
        let string = NSMutableAttributedString(string: "אבג abc", attributes: [.paragraphStyle: paragraph])
        TextEngine.setBaseDirection(string, direction: 0)
        let style = string.attribute(.paragraphStyle, at: 0, effectiveRange: nil) as? NSParagraphStyle
        XCTAssertEqual(style?.baseWritingDirection, .leftToRight)
        XCTAssertEqual(style?.alignment, .center)
        XCTAssertEqual(style?.firstLineHeadIndent, 12)
        XCTAssertNil(string.attribute(.writingDirection, at: 0, effectiveRange: nil))
    }
}
