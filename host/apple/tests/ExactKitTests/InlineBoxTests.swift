// LLP 1011.002: an image in a paragraph's text on CoreText. Its run is a
// U+FFFC whose run delegate is the box: the line takes its width, grows to
// hold it, and the measurer says where it went.
import XCTest
import CoreText
import CExact
@testable import ExactKit

final class InlineBoxTests: XCTestCase {
    private let engine = TextEngine(resolve: { _ in nil })
    private func run(_ text: String) -> Run {
        Run(text: text, size: 16, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
    }
    private func spec(_ box: InlineBox?) -> Spec {
        var runs = [run("ab")]
        if let box { var image = run("\u{FFFC}"); image.box = box; image.hidden = true; runs.append(image) }
        runs.append(run("cd"))
        return Spec(runs: runs, align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run(""))
    }
    private var font: CTFont { engine.font(size: 16, weight: 400, family: 0, italic: false) as CTFont }

    func testABoxTakesItsWidthAndGrowsItsLine() {
        let plain = engine.paragraph(spec(nil), width: .infinity)
        let p = engine.paragraph(spec(InlineBox(width: 30, height: 40, align: 0, shift: 0)), width: .infinity)
        XCTAssertEqual(p.width, plain.width + 30, accuracy: 0.05)
        // `baseline`: its bottom on the baseline, so the line is 40 above it.
        XCTAssertEqual(p.firstBaseline, 40, accuracy: 0.01)
        XCTAssertGreaterThan(p.height, 40)
        XCTAssertLessThan(p.height, 40 + plain.height)
        // A box shorter than the text keeps the line's height.
        let small = engine.paragraph(spec(InlineBox(width: 8, height: 8, align: 0, shift: 0)), width: .infinity)
        XCTAssertEqual(small.height, plain.height)
    }

    func testVerticalAlignPlacesTheBoxAgainstTheBaseline() {
        let (above, below) = CSSLineBox.content(font)
        XCTAssertEqual(InlineBox(width: 10, height: 20, align: 0, shift: 0).ascent(font), 20)
        XCTAssertEqual(InlineBox(width: 10, height: 20, align: 1, shift: 0).ascent(font), (20 + CTFontGetXHeight(font)) / 2)
        XCTAssertEqual(InlineBox(width: 10, height: 20, align: 2, shift: 0).ascent(font), above)
        XCTAssertEqual(InlineBox(width: 10, height: 20, align: 3, shift: 0).ascent(font), 20 - below)
        XCTAssertEqual(InlineBox(width: 10, height: 20, align: 0, shift: -4).ascent(font), 16)
    }

    func testTheMeasurerSaysWhereEachBoxWent() {
        let box = InlineBox(width: 30, height: 12, align: 0, shift: 3)
        let texts = ["ab", "\u{FFFC}", "cd"].map { Array($0.utf8) }
        let origins: [CGFloat] = texts[0].withUnsafeBufferPointer { a in
            texts[1].withUnsafeBufferPointer { b in
                texts[2].withUnsafeBufferPointer { c in
                    var runs = [a, b, c].map { bytes -> ExactTextRun in
                        var r = ExactTextRun()
                        r.text = bytes.baseAddress; r.len = bytes.count
                        r.font_size = 16; r.font_weight = 400; r.font_stretch = 100
                        return r
                    }
                    runs[1].inline_box = 1; runs[1].box_width = 30; runs[1].box_height = 12; runs[1].box_shift = 3
                    var strut = ExactTextRun()
                    strut.font_size = 16; strut.font_weight = 400; strut.font_stretch = 100
                    return runs.withUnsafeBufferPointer { buffer in
                        var request = ExactMeasureRequest()
                        request.runs = buffer.baseAddress; request.count = buffer.count; request.strut = strut
                        request.width = 300; request.height = EXACT_MAX_CONTENT
                        _ = engine.measure(request)
                        return engine.inlineBoxOrigins(request)
                    }
                }
            }
        }
        let p = engine.paragraph(spec(box), width: 300)
        let ab = engine.paragraph(Spec(runs: [run("ab")], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run("")), width: .infinity)
        XCTAssertEqual(origins.count, 2)
        XCTAssertEqual(origins[0], ab.width, accuracy: 0.05)
        // Raised 3: its top 15 above the baseline.
        XCTAssertEqual(origins[1], p.firstBaseline - 15, accuracy: 0.01)
    }
}
