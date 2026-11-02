#if os(iOS)
import UIKit
import WebKit
import CoreText
import XCTest
@testable import ExactKit

final class BaselinePrecisionIOSTests: XCTestCase {
    private final class Loaded: NSObject, WKNavigationDelegate {
        let done: XCTestExpectation
        init(_ done: XCTestExpectation) { self.done = done }
        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { done.fulfill() }
    }

    func testPaintedGlyphPlacementMatchesAHandBuiltLabel() throws {
        let engine = TextEngine(resolve: { _ in nil })
        for scale in [2, 3] as [CGFloat] {
            for size in [13, 16, 17] as [CGFloat] {
                let paragraph = self.paragraph(engine, "Hxg", size: size)
                let run = Run(text: "Hxg", size: size, weight: 400, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
                let spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run)
                let label = UILabel(); label.font = engine.font(size: size, weight: 400, family: 0, italic: false)
                label.text = "Hxg"; label.textColor = .black
                label.traitOverrides.displayScale = scale; label.updateTraitsIfNeeded()
                label.frame = CGRect(origin: .zero, size: label.sizeThatFits(CGSize(width: 100, height: 100)))
                let format = UIGraphicsImageRendererFormat(); format.scale = scale; format.opaque = true; format.preferredRange = .standard
                func render(_ paint: (CGContext) -> Void) -> UIImage {
                    UIGraphicsImageRenderer(size: CGSize(width: 100, height: 40), format: format).image { context in
                        UIColor.white.setFill(); context.fill(CGRect(x: 0, y: 0, width: 100, height: 40))
                        context.cgContext.translateBy(x: 10, y: 10)
                        paint(context.cgContext)
                    }
                }
                let exact = render { TextEngine.draw(paragraph, spec: spec, in: CGRect(x: 0, y: 0, width: 80, height: 30), context: $0) }
                let native = render { label.layer.render(in: $0) }
                func inkRows(_ image: UIImage) throws -> ClosedRange<Int> {
                    let cg = try XCTUnwrap(image.cgImage), data = try XCTUnwrap(cg.dataProvider?.data)
                    let bytes = CFDataGetBytePtr(data)!
                    let rows = (0..<cg.height).filter { y in
                        (0..<cg.width).contains { x in bytes[y * cg.bytesPerRow + x * cg.bitsPerPixel / 8] < 240 }
                    }
                    return try XCTUnwrap(rows.first)...XCTUnwrap(rows.last)
                }
                let job = TextRasterJob(source: engine.attributed(spec), ranges: paragraph.lines.map { CTLineGetStringRange($0) },
                                        baselines: paragraph.baselines, flush: 0, box: CGRect(x: 0, y: 0, width: 80, height: 30),
                                        size: CGSize(width: 80, height: 30), scale: scale)
                let raster = try XCTUnwrap(job.render(lines: paragraph.lines))
                let worker = render { _ in UIImage(cgImage: raster.image, scale: scale, orientation: .up).draw(in: raster.frame) }
                let actual = try inkRows(exact), expected = try inkRows(native)
                XCTAssertEqual(try inkRows(worker), actual, "the worker and direct painter place the same ink")
                print("painted baseline \(scale)x \(size)pt: Exact ink=\(actual) UILabel ink=\(expected) normal heights=\(paragraph.height)/\(label.bounds.height)")
                XCTAssertEqual(actual.lowerBound, expected.lowerBound, "first ink row at \(scale)x, \(size)pt")
                // UILabel paints one more descender row in these two 2x cases.
                let extra = scale == 2 && size != 17 ? 1 : 0
                XCTAssertEqual(actual.upperBound, expected.upperBound - extra)
                XCTAssertEqual(paragraph.height, [13: CGFloat(16), 16: 18, 17: 20][Int(size)], "Chrome's line height retains UIKit's glyph placement")
                if let directory = ProcessInfo.processInfo.environment["EXACT_PIXEL_CAPTURES"] {
                    for (name, image) in [("exact", exact), ("label", native)] {
                        try image.pngData()?.write(to: URL(fileURLWithPath: directory).appendingPathComponent("baseline-\(Int(scale))x-\(Int(size))-\(name).png"))
                    }
                }
            }
        }
    }

    func testIntrinsicWidthsKeepFractionsBesideUILabelAndWebKitLineBoxes() throws {
        let sizes: [CGFloat] = [11.3, 13, 14, 16, 17, 20]
        let engine = TextEngine(resolve: { _ in nil })
        let loaded = expectation(description: "WebKit line boxes")
        let delegate = Loaded(loaded), web = WKWebView(frame: CGRect(x: 0, y: 0, width: 300, height: 600))
        web.navigationDelegate = delegate
        let rows = sizes.enumerated().map { index, size in
            "<div><span id='t\(index)' style='display:inline-block;white-space:nowrap;font:400 \(size)px -apple-system'>Hxg<i style='display:inline-block;width:0;height:0;vertical-align:baseline'></i></span></div>"
        }.joined()
        web.loadHTMLString("<meta name='viewport' content='width=device-width,initial-scale=1'><body style='margin:0'>\(rows)</body>", baseURL: nil)
        wait(for: [loaded], timeout: 20)
        let measured = expectation(description: "WebKit measurements")
        var browser: [[String: Double]] = []
        var failure: Error?
        web.evaluateJavaScript("Array.from(document.querySelectorAll('span')).map(s=>{let r=s.getBoundingClientRect(),b=s.firstElementChild.getBoundingClientRect();return {width:r.width,height:r.height,baseline:b.bottom-r.top}})") { value, error in
            browser = value as? [[String: Double]] ?? []; failure = error; measured.fulfill()
        }
        wait(for: [measured], timeout: 20)
        XCTAssertNil(failure); XCTAssertEqual(browser.count, sizes.count)
        guard browser.count == sizes.count else { return }
        for (index, size) in sizes.enumerated() {
            let font = engine.font(size: size, weight: 400, family: 0, italic: false)
            let label = UILabel(); label.font = font; label.text = "Hxg"
            let labelSize = label.sizeThatFits(CGSize(width: 300, height: 600))
            let container = UIView(frame: CGRect(x: 0, y: 0, width: 300, height: 100)), marker = UIView()
            for view in [label, marker] { view.translatesAutoresizingMaskIntoConstraints = false; container.addSubview(view) }
            NSLayoutConstraint.activate([
                label.topAnchor.constraint(equalTo: container.topAnchor), label.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                label.widthAnchor.constraint(equalToConstant: labelSize.width), label.heightAnchor.constraint(equalToConstant: labelSize.height),
                marker.topAnchor.constraint(equalTo: label.firstBaselineAnchor), marker.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                marker.widthAnchor.constraint(equalToConstant: 0), marker.heightAnchor.constraint(equalToConstant: 0),
            ])
            container.layoutIfNeeded()
            let paragraph = self.paragraph(engine, "Hxg", size: size)
            let raw = CGFloat(CTLineGetTypographicBounds(try XCTUnwrap(paragraph.lines.first), nil, nil, nil))
            XCTAssertGreaterThanOrEqual(paragraph.width, raw)
            XCTAssertLessThan(paragraph.width - raw, 1.0 / 64 + 1e-6, "intrinsic width keeps the browser's layout precision")
            XCTAssertEqual(Double(paragraph.width), try XCTUnwrap(browser[index]["width"]), accuracy: 1.0 / 64)
            XCTAssertEqual(self.paragraph(engine, "Hxg", size: size, lineHeight: 21.25).height, 21.25)
            print("baseline size=\(size) face=\(font.fontName) Exact=[\(paragraph.width),\(paragraph.height),\(paragraph.firstBaseline)] UILabel=[\(labelSize.width),\(labelSize.height),\(marker.frame.minY)] WebKit=\(browser[index])")
        }
        withExtendedLifetime(delegate) {}
    }
}
#endif
