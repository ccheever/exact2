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
