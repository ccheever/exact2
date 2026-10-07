// CSS overflow-wrap on Apple: lines break at UAX #14 opportunities (the last
// that fits); a word that cannot fit on a line by itself overflows under
// `normal` and breaks inside under `break-word` and `anywhere`; min-content
// counts only `anywhere`'s extra breaks (CSS Text 3 §5.5).
import XCTest
import CoreText
@testable import ExactKit

final class OverflowWrapTests: XCTestCase {
    private let engine = TextEngine(resolve: { _ in nil })
    private let url = "https://www.nps.gov/goga/planyourvisit/muir-woods.htm"

    private func spec(_ text: String, wrap: Int) -> Spec {
        var s = Spec(runs: [Run(text: text, size: 17, weight: 400, family: 0, italic: false, lineHeight: 20, letterSpacing: 0)],
                     align: 0, lineClamp: 0, color: [0, 0, 0, 255])
        s.overflowWrap = wrap
        return s
    }
    private func lines(_ s: Spec, width: CGFloat) -> [String] {
        let text = s.runs.map(\.text).joined() as NSString
        return engine.paragraph(s, width: width).lines.map {
            let r = CTLineGetStringRange($0)
            return text.substring(with: NSRange(location: r.location, length: r.length))
        }
    }
    private func width(_ text: String) -> CGFloat {
        CGFloat(CTLineGetTypographicBounds(engine.paragraph(spec(text, wrap: 0), width: .infinity).lines[0], nil, nil, nil))
    }

    /// The bubble's text at 280. Apple's UAX #14 tailoring breaks a URL
    /// after each solidus (as Safari does), so it wraps there; a piece wider
    /// than the box overflows under `normal` and breaks inside under
    /// `break-word` and `anywhere`, so no line is wider than the box.
    func testAnOverlongWordBreaksInsideUnderBreakWordOnly() {
        let text = "Look " + url
        let lined = lines(spec(text, wrap: 1), width: 280)
        XCTAssertEqual(lined.joined(), text)
        XCTAssertEqual(lined.first, "Look https://www.nps.gov/goga/", "the last opportunity that fits")
        let long = "Look " + String(repeating: "a", count: 60)
        let normal = lines(spec(long, wrap: 0), width: 200)
        XCTAssertEqual(normal.first, "Look ", "the word goes to a line of its own")
        XCTAssertGreaterThan(width(normal[1]), 200, "and overflows under normal: \(normal)")
        for wrap in [1, 2] {
            let broken = lines(spec(long, wrap: wrap), width: 200)
            XCTAssertEqual(broken.joined(), long)
            for line in broken {
                XCTAssertLessThanOrEqual(width(line.trimmingCharacters(in: .whitespaces)), 200.5, "wrap \(wrap): \(broken)")
            }
        }
        XCTAssertEqual(lines(spec(long, wrap: 1), width: 200).first, "Look ", "break-word still prefers the opportunity")
    }

    /// Min-content's pieces are UAX #14's, not only spaces: the URL's
    /// solidus and hyphen are opportunities, so its min-content is the
    /// widest piece, not the whole URL; `anywhere` adds every grapheme,
    /// `break-word` adds none.
    func testMinContentCutsAtUnicodeOpportunitiesAndOnlyAnywhereAddsMore() {
        let pieces = engine.unbreakablePieces(url)
        XCTAssertGreaterThan(pieces.count, 2, "\(pieces)")
        XCTAssertEqual(pieces.joined(), url)
        let widest = pieces.map { CSSLineBox.layoutWidth(width($0)) }.max()!
        XCTAssertEqual(engine.minContentWidth(spec(url, wrap: 0)), widest, accuracy: 0.5)
        XCTAssertEqual(engine.minContentWidth(spec(url, wrap: 1)), widest, accuracy: 0.5)
        XCTAssertLessThan(widest, CSSLineBox.layoutWidth(width(url)) / 2)
        XCTAssertLessThan(engine.minContentWidth(spec(url, wrap: 2)), 20)
        XCTAssertEqual(engine.unbreakablePieces("two\u{00A0}words here"), ["two\u{00A0}words", "here"], "a no-break space joins")
    }

    /// A forced break still ends its line, whatever fits after it: CR LF as
    /// one, the spaces before it hanging, the spaces after it the next line's.
    func testForcedBreaksStillEndLines() {
        for wrap in [0, 1] {
            var s = spec("a\nb c", wrap: wrap); s.whiteSpace = 1
            XCTAssertEqual(lines(s, width: 300), ["a\n", "b c"])
            s = spec("aa hello\r\nworld", wrap: wrap); s.whiteSpace = 1
            XCTAssertEqual(lines(s, width: 400), ["aa hello\r\n", "world"], "CR LF is one break")
            s = spec("hi\n   world", wrap: wrap); s.whiteSpace = 1
            XCTAssertEqual(lines(s, width: 400), ["hi\n", "   world"], "the next line keeps its indent")
            s = spec("aa hello   \nworld", wrap: wrap); s.whiteSpace = 1
            let w = width("aa hello") + 2
            XCTAssertEqual(lines(s, width: w), ["aa hello   \n", "world"], "spaces before a forced break hang")
        }
    }

    /// A no-break space is content: it neither hangs at a break nor drops
    /// from min-content; a soft hyphen's piece counts its visible hyphen.
    func testNoBreakSpacesStayAndSoftHyphensCount() {
        let w = width("hi hello") + 1
        XCTAssertEqual(lines(spec("hi hello\u{00A0}", wrap: 1), width: w).count, 2, "the NBSP does not hang")
        XCTAssertEqual(engine.unbreakablePieces("hello\u{00A0}"), ["hello\u{00A0}"])
        XCTAssertEqual(engine.unbreakablePieces("\u{00A0}hello"), ["\u{00A0}hello"])
        XCTAssertEqual(engine.unbreakablePieces("WWW\u{00AD}q"), ["WWW-", "q"])
        XCTAssertGreaterThanOrEqual(engine.minContentWidth(spec("WWW\u{00AD}q", wrap: 0)), CSSLineBox.layoutWidth(width("WWW-")) - 0.01)
        // A soft hyphen followed by a space, a tab or a forced break is not
        // where its line breaks: no hyphen shows, none is counted.
        for after in [" ", "\t", "\n"] {
            XCTAssertEqual(engine.unbreakablePieces("WWW\u{00AD}\(after)q").first, "WWW\u{00AD}", "after \(after.unicodeScalars.first!.value)")
        }
    }

    /// A soft hyphen's break that cannot show its hyphen gives way to an
    /// earlier opportunity measured against the whole room, not the room
    /// less the hyphen.
    func testAnEarlierOpportunityKeepsTheWholeRoom() {
        for wrap in [0, 1] {
            let text = "hi WWW \u{00AD}q"
            let w = width("hi WWW") + 1
            XCTAssertEqual(lines(spec(text, wrap: wrap), width: w).first, "hi WWW ", "wrap \(wrap)")
        }
    }

    /// Other space separators hang as U+0020 does, out of the fit and out of
    /// min-content; an ideographic space is one.
    func testOtherSpaceSeparatorsHang() {
        let w = width("hi hello") + 1
        XCTAssertEqual(lines(spec("hi hello\u{3000}x", wrap: 1), width: w), ["hi hello\u{3000}", "x"])
        XCTAssertEqual(engine.unbreakablePieces("hello\u{3000}x"), ["hello", "x"])
    }

    /// Alternating tabs and spaces are one forward pass, not a scan per opportunity.
    func testContentEndsAreLinear() {
        let text = String(repeating: "\t ", count: 20_000) as NSString
        let started = Date()
        let boundaries = engine.lineBoundaries(text, length: text.length)
        let (content, _) = TextEngine.breakEnds(text, boundaries: boundaries, length: text.length)
        XCTAssertEqual(content.count, boundaries.count)
        XCTAssertLessThan(Date().timeIntervalSince(started), 1)
    }

    /// The region worker breaks as the ordinary paragraph does, forced
    /// breaks and emergency breaks included.
    func testRegionWorkerBreaksAsTheParagraphDoes() {
        for wrap in [0, 1] {
            for text in ["Look " + url, "hi\n   world", "aa hello\r\nworld", "aa hello   \nworld", "Look " + String(repeating: "a", count: 60), "WWW\u{00AD}q", "hi WWW \u{00AD}q"] {
                var s = spec(text, wrap: wrap); s.whiteSpace = 1
                let source = RegionTextSource.capture(s, engine: engine)
                for width: CGFloat in [50, 83.25, 200, 280] {
                    let ordinary = engine.paragraph(s, width: width).lines.map { CTLineGetStringRange($0) }
                    let done = DispatchSemaphore(value: 0)
                    var worker: [NSRange] = []
                    DispatchQueue(label: "overflow-wrap-region").async {
                        worker = RegionWorkerLayout.shape(source, width: width).lines.map { let r = CTLineGetStringRange($0); return NSRange(location: r.location, length: r.length) }
                        done.signal()
                    }
                    done.wait()
                    XCTAssertEqual(worker, ordinary.map { NSRange(location: $0.location, length: $0.length) }, "\(wrap) \(width) \(text)")
                }
            }
        }
    }
}
