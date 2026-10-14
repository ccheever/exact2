// CSS `text-wrap-style: balance` on Apple (LLP 1001 §6): the lines before
// the first forced break break at the narrowest width that keeps their
// count, up to six lines, as Chrome's do; the box measures as greedy lines.
// The expected lines and widths are Chrome's for 16px Arial (a Range per
// line, the textrows issue).
import XCTest
import CoreText
@testable import ExactKit

final class TextBalanceTests: XCTestCase {
    private let engine: TextEngine = {
        let engine = TextEngine(resolve: { _ in nil })
        let arial = CTFontDescriptorCreateWithNameAndSize("ArialMT" as CFString, 16)
        engine.catalog = [9: [RegisteredFace(family: "Arial", generic: nil, weight: 400, italic: false, descriptor: arial)]]
        return engine
    }()
    private let sentence = "Settle every thread in this section and keep the newest one open"

    private func spec(_ text: String, balance: Bool, align: Int = 0, whiteSpace: Int = 0, clamp: Int = 0) -> Spec {
        var s = Spec(runs: [Run(text: text, size: 16, weight: 400, family: 9, italic: false, lineHeight: nil, letterSpacing: 0)],
                     align: align, lineClamp: clamp, color: [0, 0, 0, 255], whiteSpace: whiteSpace)
        s.balance = balance
        return s
    }
    private func lines(_ p: Paragraph, _ text: String) -> [String] {
        p.lines.map {
            let r = CTLineGetStringRange($0)
            return (text as NSString).substring(with: NSRange(location: r.location, length: r.length))
        }
    }
    private func inked(_ p: Paragraph) -> [CGFloat] {
        p.lines.map { CGFloat(CTLineGetTypographicBounds($0, nil, nil, nil) - CTLineGetTrailingWhitespaceWidth($0)) }
    }

    func testTheIssuesSentenceBalancesAsChromeDoes() {
        let greedy = engine.paragraph(spec(sentence, balance: false), width: 180)
        XCTAssertEqual(lines(greedy, sentence), ["Settle every thread in ", "this section and keep the ", "newest one open"])
        let balanced = engine.paragraph(spec(sentence, balance: true), width: 180)
        XCTAssertEqual(lines(balanced, sentence), ["Settle every thread in ", "this section and keep ", "the newest one open"])
        for (width, chrome) in zip(inked(balanced), [151.2, 150.33, 148.56]) {
            XCTAssertEqual(width, chrome, accuracy: 0.5)
        }
        XCTAssertEqual(balanced.width, greedy.width, "balancing moves breaks, never the box")
        XCTAssertEqual(balanced.height, greedy.height)
        // Centred, the lines centre in the box's width, not the balanced one.
        let centred = engine.paragraph(spec(sentence, balance: true, align: 1), width: 180)
        XCTAssertEqual(lines(centred, sentence), lines(balanced, sentence))
        XCTAssertEqual(CGFloat(CTLineGetPenOffsetForFlush(centred.lines[0], 0.5, 180)), 14.39, accuracy: 0.5)
        // Justified, all but the last fill the box, as Chrome's do.
        let justified = engine.paragraph(spec(sentence, balance: true, align: 3), width: 180)
        XCTAssertEqual(lines(justified, sentence), lines(balanced, sentence))
        XCTAssertEqual(inked(justified)[0], 180, accuracy: 0.5)
    }

    func testRecolouringBalancedTextKeepsItsMeasuredWidth() {
        let original = engine.paragraph(spec(sentence, balance: true), width: 180)
        var red = spec(sentence, balance: true)
        red.color = [255, 0, 0, 255]
        let recoloured = engine.paragraph(red, width: 180)
        XCTAssertEqual(lines(recoloured, sentence), lines(original, sentence))
        XCTAssertEqual(recoloured.width, original.width)
        XCTAssertEqual(recoloured.height, original.height)
        let fromRanges = engine.layout(original.shape!, width: 180,
                                       ranges: original.lines.map { CTLineGetStringRange($0) })
        XCTAssertEqual(fromRanges.width, original.width)
        withExtendedLifetime(original) {}
    }

    /// Chrome balances only the lines before the first forced break; the
    /// rest break greedily at the box's width.
    func testOnlyTheLinesBeforeTheFirstForcedBreakBalance() {
        let text = "Settle every thread in this section\nand keep the newest one open now please ok"
        let p = engine.paragraph(spec(text, balance: true, whiteSpace: 3), width: 180)
        XCTAssertEqual(lines(p, text), ["Settle every thread ", "in this section\n", "and keep the newest one ", "open now please ok"])
    }

    /// Seven lines, or a clamp, stay greedy, as in Chrome.
    func testMoreThanSixLinesAndAClampStayGreedy() {
        let seven = "a b c d e f g h i j k l m n o p q r s t u v w x y z aa bb cc dd ee ff gg hh ii jj kk ll mm nn"
        let greedy = lines(engine.paragraph(spec(seven, balance: false), width: 100), seven)
        XCTAssertEqual(greedy.count, 7)
        XCTAssertEqual(lines(engine.paragraph(spec(seven, balance: true), width: 100), seven), greedy)
        let six = "a b c d e f g h i j k l m n o p q r s t u v w x y z aa bb cc dd ee ff gg hh ii jj kk"
        XCTAssertEqual(lines(engine.paragraph(spec(six, balance: true), width: 100), six).last, "gg hh ii jj kk", "Chrome's last line")
        let clamped = engine.paragraph(spec(sentence, balance: true, clamp: 3), width: 180)
        XCTAssertEqual(lines(clamped, sentence)[1], "this section and keep the ")
    }
}
