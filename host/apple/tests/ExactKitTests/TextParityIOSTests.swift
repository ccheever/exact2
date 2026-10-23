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

    @MainActor func testMixedSizesAndLineHeightsKeepChromesLineBoxes() async {
        let engine = TextEngine(resolve: { _ in nil })
        let native16 = engine.font(size: 16, weight: 400, family: 0, italic: false).ascender
        let cases: [(base: CGFloat, baseHeight: CGFloat?, span: CGFloat, spanHeight: CGFloat?, height: CGFloat, baseline: CGFloat)] = [
            (16, nil, 13, nil, 18, native16), (13, nil, 16, nil, 18, native16),
            (16, 18, 13, nil, 18, 15), (13, nil, 16, 18, 18, 15),
            (16, nil, 13, 16, 18, native16), (16, nil, 13, 20, 20, native16),
            (16, 24, 13, nil, 24, 18),
        ]
        for c in cases {
            let base = Run(text: "H", size: c.base, weight: 400, family: 0, italic: false,
                           lineHeight: c.baseHeight, letterSpacing: 0)
            var span = Run(text: "xg\n", size: c.span, weight: 400, family: 0, italic: false,
                           lineHeight: c.spanHeight, letterSpacing: 0)
            let firstSpan = span; span.text = "xg"
            let spec = Spec(runs: [base, firstSpan, base, span], align: 0, lineClamp: 0,
                            color: [0, 0, 0, 255], whiteSpace: 1, strut: base)
            let source = RegionTextSource.capture(spec, engine: engine)
            let workers = await Task.detached {
                [false, true].map { RegionWorkerLayout.shape(source, width: 500, compact: $0).metadata }
            }.value
            let ordinary = engine.paragraph(spec, width: 500)
            let flow = engine.paragraph(spec, width: 500, flow: [TextFlowShape(kind: 0, x: 0, y: 1000, a: 10)])
            XCTAssertFalse(flow.flowIncomplete)
            print("mixed line boxes \(c): ordinary=\(ordinary.height), flow=\(flow.height), workers=\(workers.map(\.height))")
            for p in [ordinary, flow] {
                XCTAssertEqual(p.height, c.height * 2, "\(c)")
                XCTAssertEqual(p.lineBottoms, [c.height, c.height * 2], "\(c)")
                XCTAssertEqual(p.firstBaseline, c.baseline, accuracy: 1e-6, "\(c)")
                XCTAssertEqual(p.baselines[1] - p.baselines[0], c.height, accuracy: 1e-6, "\(c)")
            }
            for p in workers {
                XCTAssertEqual(p.height, c.height * 2, "\(c)")
                XCTAssertEqual(p.lineBottoms, [c.height, c.height * 2], "\(c)")
                XCTAssertEqual(p.baselines, ordinary.baselines, "worker placement matches the painter")
            }
        }
    }

    func testIntrinsicWidthsAreChromes() { assertChromeWidths() }
}
#endif
