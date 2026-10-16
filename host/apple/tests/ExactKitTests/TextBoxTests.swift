// CSS `text-box` on CoreText (Inline Layout 3 §4.2): a paragraph is cut to
// its root inline box's metrics, a flowed line keeps its clearance, and a
// registered region's worker cuts as the ordinary paragraph does.
import XCTest
import CoreText
import CExact
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

    func testEmptyParagraphHasNoLineBox() {
        let painter = TextEngine.pair(resolve: { _ in nil })
        for whiteSpace in 0...4 {
            for lineHeight: CGFloat? in [nil, 48] {
                var s = spec("", trim: 0)
                s.whiteSpace = whiteSpace
                s.runs[0].lineHeight = lineHeight
                s.strut = s.runs[0]
                let variants = [[], s.runs, s.runs + s.runs]
                for width: CGFloat in [0, 200, .infinity] {
                    for runs in variants {
                        s.runs = runs
                        let p = engine.paragraph(s, width: width)
                        XCTAssertEqual(p.width, 0)
                        XCTAssertEqual(p.height, 0)
                        XCTAssertTrue(p.lines.isEmpty)
                        XCTAssertTrue(p.baselines.isEmpty)
                        XCTAssertTrue(p.lineBottoms.isEmpty)
                    }
                }
                var run = ExactTextRun()
                run.font_size = 16; run.font_weight = 400
                run.has_line_height = lineHeight == nil ? 0 : 1
                run.line_height = Float(lineHeight ?? 0)
                for count in 0...2 {
                    Array(repeating: run, count: count).withUnsafeBufferPointer { runs in
                        var request = ExactMeasureRequest()
                        request.runs = runs.baseAddress; request.count = runs.count; request.strut = run
                        request.white_space = UInt8(whiteSpace)
                        request.height = EXACT_MAX_CONTENT
                        for width in [EXACT_MIN_CONTENT, EXACT_MAX_CONTENT, 0, 200] {
                            request.width = width
                            for measurer in [engine, painter.measuring] {
                                for _ in 0..<2 {
                                    let metrics = measurer.measure(request)
                                    XCTAssertEqual(metrics.width, 0)
                                    XCTAssertEqual(metrics.height, 0)
                                    XCTAssertEqual(metrics.baseline, -1, "no line has no baseline at the C seam")
                                }
                            }
                            var key: [UInt8] = []
                            let hash = TextAnswerKey.write(request, into: &key)
                            let offer = width == EXACT_MIN_CONTENT ? TextAnswerKey.minContent
                                : width == EXACT_MAX_CONTENT ? TextAnswerKey.maxContent : TextAnswerKey.offer(width: CGFloat(width))
                            XCTAssertEqual(painter.answers?.metrics(hash: hash, key: key, offer: offer)?.baseline, -1)
                        }
                    }
                }
            }
        }
        var s = spec("\n", trim: 0)
        s.whiteSpace = 1
        s.runs[0].lineHeight = 24
        XCTAssertEqual(engine.paragraph(s, width: 200).height, 24, "a preserved break still makes a line")
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
                if text.isEmpty {
                    XCTAssertEqual(worker?.firstBaseline, -1, "the region reader must publish no baseline")
                    continue
                }
                XCTAssertEqual(worker?.baselines.count, ordinary.baselines.count, says)
                XCTAssertEqual(worker?.lineBottoms.last ?? -1, worker?.height ?? -2, accuracy: 0.01, says)
                for (a, b) in zip(worker?.baselines ?? [], ordinary.baselines) { XCTAssertEqual(a, b, accuracy: 0.01, says) }
            }
        }
    }
}
