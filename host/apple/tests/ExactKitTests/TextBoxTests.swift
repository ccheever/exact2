// CSS `text-box` on CoreText (Inline Layout 3 §4.2): a paragraph is cut to
// its root inline box's metrics, a flowed line keeps its clearance, and a
// registered region's worker cuts as the ordinary paragraph does.
import XCTest
import CoreText
@testable import ExactKit

final class TextBoxTests: XCTestCase {
    private let engine = TextEngine(resolve: { _ in nil })
    private func spec(_ text: String, size: CGFloat = 16, root: CGFloat? = nil, trim: Int = 3, over: Int = 1, under: Int = 1) -> Spec {
        var s = Spec(runs: [Run(text: text, size: size, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)],
                     align: 0, lineClamp: 0, color: [0, 0, 0, 255],
                     strut: root.map { Run(text: "", size: $0, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0) })
        s.textBoxTrim = trim; s.textBoxOver = over; s.textBoxUnder = under
        return s
    }
    private func cap(_ size: CGFloat) -> CGFloat {
        CTFontGetCapHeight(engine.font(size: size, weight: 400, family: 0, italic: false) as CTFont)
    }

    /// Text that is all a 10 pt span in a 20 pt paragraph is cut to the
    /// 20 pt cap height and baseline, as CSS cuts the root inline box.
    func testTheRootInlineBoxsFontSetsTheTrim() {
        let p = engine.paragraph(spec("TTI", size: 10, root: 20), width: 400)
        XCTAssertEqual(p.firstBaseline, cap(20), accuracy: 0.01, "the 20 pt cap height, not the 10 pt span's")
        XCTAssertEqual(p.height, cap(20), accuracy: 0.01, "the box ends at the baseline")
    }

    /// A line flow pushed below a tall obstacle keeps its clearance: the cut
    /// is only its own space above the cap height, and its fragment moves
    /// with it.
    func testAFlowedLineKeepsItsClearance() {
        let wall = TextFlowShape(kind: 2, x: 0, y: 0, a: 400, b: 30_000)
        var plainSpec = spec("word"); plainSpec.textBoxTrim = 0
        let plain = engine.layoutFlow(engine.paragraph(plainSpec, width: .infinity).shape!, width: 400, flow: [wall])
        let trimmed = engine.layoutFlow(engine.paragraph(spec("word"), width: .infinity).shape!, width: 400, flow: [wall])
        let cut = CGFloat(plain.fragments[0].y - trimmed.fragments[0].y)
        XCTAssertGreaterThan(cut, 0)
        XCTAssertLessThan(cut, 20)
        XCTAssertGreaterThan(trimmed.firstBaseline, 30_000, "the baseline stays below the wall")
        XCTAssertEqual(trimmed.firstBaseline, plain.firstBaseline - cut, accuracy: 0.01)
    }

    /// A line shorter than its glyphs is cut by negative amounts: the
    /// paragraph grows, and its last line ends at the grown height.
    func testTheLastLineEndsAtTheTrimmedHeight() {
        var s = spec("TTI\nTTI", size: 16); s.whiteSpace = 1
        s.runs[0].lineHeight = 4
        let p = engine.paragraph(s, width: 400)
        XCTAssertGreaterThan(p.height, 8, "the cuts are negative")
        XCTAssertEqual(p.lineBottoms.count, 2)
        XCTAssertEqual(p.lineBottoms.last ?? -1, p.height, accuracy: 0.01)
    }

    /// A registered region's worker cuts a paragraph as the ordinary one is
    /// cut, each trim and edge, on one line and several, with short lines,
    /// and not at all when empty.
    func testARegionsWorkerTrimsAsTheParagraphDoes() {
        for text in ["TTI", "TTI\nTTI\nTTI", ""] {
            for (trim, over, under, lineHeight) in [(1, 1, 1, nil), (2, 0, 0, nil), (3, 2, 1, nil), (3, 0, 0, nil), (3, 1, 1, CGFloat(4))] {
                var s = spec(text, size: 12, root: 16, trim: trim, over: over, under: under); s.whiteSpace = 1
                s.runs[0].lineHeight = lineHeight; s.strut?.lineHeight = lineHeight
                let ordinary = engine.paragraph(s, width: 400)
                let source = RegionTextSource.capture(s, engine: engine)
                let done = DispatchSemaphore(value: 0)
                var worker: RegionParagraph?
                DispatchQueue(label: "text-box-region").async {
                    worker = RegionWorkerLayout.shape(source, width: 400).metadata
                    done.signal()
                }
                done.wait()
                let says = "\(trim) \(over) \(under) \(text.count)"
                XCTAssertEqual(worker?.height ?? -1, ordinary.height, accuracy: 0.01, says)
                if text.isEmpty { continue }
                XCTAssertEqual(worker?.baselines.count, ordinary.baselines.count, says)
                XCTAssertEqual(worker?.lineBottoms.last ?? -1, worker?.height ?? -2, accuracy: 0.01, says)
                for (a, b) in zip(worker?.baselines ?? [], ordinary.baselines) { XCTAssertEqual(a, b, accuracy: 0.01, says) }
            }
        }
    }
}
