// @ref LLP 1043.000 §3 D5, D7 — real Swift bridge, native invalidation and painted ink.
import XCTest
import CoreText
import ImageIO
import CExact
@testable import ExactKit

final class TextFlowPresentationTests: XCTestCase {
    let engine = TextEngine(resolve: { _ in nil })
    private var root: URL {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    }
    func spec(_ text: String, family: Int = 0) -> Spec {
        Spec(runs: [Run(text: text, size: 16, weight: 400, family: family, italic: false,
                        lineHeight: 24, letterSpacing: 0)], align: 0, lineClamp: 0, color: [51, 75, 73, 255])
    }
    func testMeasurementUsesTheSameFlowedGeometry() {
        let text = String(repeating: "A quiet river opens a path through the garden. ", count: 10)
        let circle = TextFlowShape(kind: 0, x: 180, y: 96, a: 52)
        let expected = engine.paragraph(spec(text), width: 360, flow: [circle])
        let measured = Array(text.utf8).withUnsafeBufferPointer { bytes in
            var run = ExactTextRun()
            run.text = bytes.baseAddress; run.len = bytes.count; run.font_size = 16; run.font_weight = 400
            run.has_line_height = 1; run.line_height = 24
            return withUnsafePointer(to: &run) { pointer in
                TextFlowShape.withCShapes([circle]) { shapes in
                    var request = ExactMeasureRequest()
                    request.runs = pointer; request.count = 1; request.strut = pointer.pointee
                    request.strut.text = nil; request.strut.len = 0
                    request.width = 360; request.height = 300
                    // A warm ordinary answer must never shadow this flow request.
                    let plain = engine.measure(request)
                    XCTAssertEqual(engine.measure(request).height, plain.height)
                    request.exclusions = shapes.baseAddress; request.exclusion_count = shapes.count
                    XCTAssertNotEqual(CGFloat(plain.height), expected.height)
                    return engine.measure(request)
                }
            }
        }
        XCTAssertEqual(CGFloat(measured.width), expected.width, accuracy: 0.01)
        XCTAssertEqual(CGFloat(measured.height), expected.height, accuracy: 0.01)
        XCTAssertEqual(CGFloat(measured.baseline), expected.firstBaseline, accuracy: 0.01)
    }

    func testPresenterFlowClearsOnlyItsParagraphAndTranslatesPadding() throws {
        let session = ExactApp.shared.makeSession(label: "flow-batch")
        defer { session.destroy() }
        func apply(_ ops: [[String: Any]]) { session.apply(Batch(ops: ops, timers: false, motion: false, clock: nil, error: nil)) }
        let text = String(repeating: "The river leaves room for the light. ", count: 12)
        apply([
            ["op": "create", "id": 1, "kind": "text", "props": ["text": text],
             "style": ["font_size": 16.0, "line_height": "24px", "padding_left": 10.0, "padding_top": 12.0]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Unchanged neighbor"]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 370.0, "h": 500.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 500.0, "w": 370.0, "h": 40.0],
            ["op": "roots", "ids": [1, 2]],
        ])
        let node = try XCTUnwrap(session.presenter.views[1]), neighbor = try XCTUnwrap(session.presenter.views[2])
        let before = try XCTUnwrap(node.paragraphLayout()), unchanged = try XCTUnwrap(neighbor.paragraphLayout())
        apply([["op": "flow", "id": 1, "shapes": [["kind": "Circle", "cx": 190.0, "cy": 108.0, "r": 52.0]]]])
        let p = try XCTUnwrap(node.paragraphLayout())
        XCTAssertFalse(p === before)
        XCTAssertTrue(neighbor.paragraphLayout() === unchanged)
        let expected = session.text.paragraph(node.paragraphSpec(), width: 360, flow: [TextFlowShape(kind: 0, x: 180, y: 96, a: 52)])
        XCTAssertEqual(p.fragments.map(\.end), expected.fragments.map(\.end))
        XCTAssertEqual(p.origins, expected.origins)
        XCTAssertNoThrow(try JSONSerialization.data(withJSONObject: p.flowFacts))
        apply([["op": "flow", "id": 1, "shapes": []]])
        XCTAssertTrue(try XCTUnwrap(node.paragraphLayout()).fragments.isEmpty)
        XCTAssertTrue(node.flowShapes.isEmpty)
    }

    func testCollapsedSpacesAndSelectedSoftHyphenMatchPaintedAdvances() {
        for (text, width) in [("alpha   beta\t\tgamma", CGFloat(240)), ("abc\u{ad}def abc\u{ad}def", CGFloat(42))] {
            let shape = engine.paragraph(spec(text, family: 5), width: .infinity).shape!
            let p = engine.layoutFlow(shape, width: width, flow: [])
            XCTAssertFalse(p.fragments.isEmpty)
            for (i, f) in p.fragments.enumerated() {
                XCTAssertEqual(CTLineGetTypographicBounds(p.lines[i], nil, nil, nil), Double(f.width), accuracy: 0.03)
            }
            XCTAssertEqual(p.fragments.last?.end, text.utf8.count)
            if text.contains("\u{ad}") { XCTAssertEqual(CTLineGetGlyphCount(p.lines[0]), 4) }
        }
    }

    func testEmptyHardBreakFragmentKeepsItsLogicalCaret() {
        var input = spec("First\n\nThird")
        input.whiteSpace = 1
        let shape = engine.paragraph(input, width: 360).shape!
        let paragraph = engine.layoutFlow(shape, width: 360, flow: [])
        let empty = paragraph.fragments.indices.first { paragraph.fragments[$0].paint_start == paragraph.fragments[$0].paint_end }!
        XCTAssertEqual(paragraph.stringIndex(in: empty, at: 0), 6)
        XCTAssertEqual(paragraph.lineIndex(at: CGPoint(x: 0, y: 25), align: 0, width: 360), empty)
        let blank = engine.paragraph(spec(""), width: 360, flow: [TextFlowShape(kind: 0, x: 100, y: 50, a: 20)])
        XCTAssertEqual(blank.flowFacts["complete"] as? Bool, true)
    }

    func testDemoBallAtThreeClocksPaintsFragmentsOutsideTheExclusion() throws {
        let contract = try String(contentsOf: root.appendingPathComponent("apps/textflow/app.contract"), encoding: .utf8)
        let regex = try NSRegularExpression(pattern: #": "([^"]*)"\) testId="ball-prose""#)
        let match = try XCTUnwrap(regex.firstMatch(in: contract, range: NSRange(contract.startIndex..., in: contract)))
        let text = (contract as NSString).substring(with: match.range(at: 1)).replacingOccurrences(of: #"\n"#, with: "\n")
        var input = spec(text, family: 3)
        input.whiteSpace = 1
        for time in [0, 800, 1600] {
            let cx = Float(326 + Double(time / 16) * 2.4), cy = Float(181 + Double(time / 16) * 1.5)
            let circle = TextFlowShape(kind: 0, x: cx, y: cy, a: 66)
            let p = engine.paragraph(input, width: 656, flow: [circle])
            XCTAssertEqual(p.fragments.last?.end, text.utf8.count)
            for (i, f) in p.fragments.enumerated() {
                let width = CTLineGetTypographicBounds(p.lines[i], nil, nil, nil)
                let dy = max(0, max(Double(f.y - cy), Double(cy - f.y - 24)))
                if dy < 66 {
                    let half = sqrt(66 * 66 - dy * dy)
                    XCTAssertTrue(Double(p.origins[i]) + width <= Double(cx) - half + 0.03 || Double(p.origins[i]) >= Double(cx) + half - 0.03)
                }
            }
            let context = try XCTUnwrap(CGContext(data: nil, width: 656, height: 660, bitsPerComponent: 8,
                bytesPerRow: 656 * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            context.translateBy(x: 0, y: 660); context.scaleBy(x: 1, y: -1)
            context.setFillColor(CGColor(red: 252/255, green: 250/255, blue: 242/255, alpha: 1))
            context.fill(CGRect(x: 0, y: 0, width: 656, height: 660))
            TextEngine.draw(p, spec: input, in: CGRect(x: 0, y: 0, width: 656, height: 660), context: context)
            context.setFillColor(CGColor(red: 210/255, green: 120/255, blue: 77/255, alpha: 1))
            context.fillEllipse(in: CGRect(x: CGFloat(cx) - 56, y: CGFloat(cy) - 56, width: 112, height: 112))
            let directory = root.appendingPathComponent("target/textflow-scratch/m4")
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let output = directory.appendingPathComponent("apple-ball-\(time).png")
            let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(output as CFURL, "public.png" as CFString, 1, nil))
            CGImageDestinationAddImage(destination, try XCTUnwrap(context.makeImage()), nil)
            XCTAssertTrue(CGImageDestinationFinalize(destination))
            let pairs = zip(p.fragments, p.fragments.dropFirst()).filter { $0.line == $1.line }.count
            print("TEXTFLOW BALL clock=\(time) cx=\(cx) cy=\(cy) radius=66 fragments=\(p.fragments.count) split_bands=\(pairs) bands=\(p.fragments.last!.line + 1) height=\(p.height) end=\(p.fragments.last!.end) prepare=\(p.shape!.prepareCount)")
        }
    }
}
