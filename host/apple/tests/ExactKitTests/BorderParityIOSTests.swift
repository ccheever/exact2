#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// The parity page's border cases as UIKit renders them (`BorderParity`):
///   bun host/apple/build.mjs --test --ios
final class BorderParityIOSTests: XCTestCase {
    private func render(_ style: NodeStyle, dark: Bool, page: CGColor) -> [UInt8] {
        let p = Presenter()
        let node = NodeView(id: 1, kind: "view", presenter: p)
        p.views[node.id] = node
        // An appearance is a window's (`setScheme` sets the window's style).
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 100, height: 70))
        window.overrideUserInterfaceStyle = dark ? .dark : .light
        window.addSubview(node)
        node.frame = CGRect(x: 0, y: 0, width: 100, height: 70)
        node.contentScaleFactor = 1
        node.applyStyle(style)
        node.layer.displayIfNeeded()
        let ctx = BorderParity.canvas(page)
        node.layer.render(in: ctx)
        return BorderParity.bytes(ctx)
    }

    func testEveryBorderCaseMatchesChrome() {
        var failures = BorderParity.check(flipped: false, dark: false) { render($0, dark: $1, page: $2) }
        failures += BorderParity.check(flipped: true, dark: true) { render($0, dark: $1, page: $2) }
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }

    func testPatternedBordersMatchChrome() {
        let failures = BorderParity.checkPatterns { render($0, dark: $1, page: $2) }
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }

    func testPatternedBorderLeavesGaps() {
        for pattern in ["dotted", "dashed"] {
            var style = BorderParity.cases(flipped: false)[0].style
            style["background_color"] = .array([.number(255), .number(255), .number(255), .number(255)])
            for side in ["top", "right", "bottom", "left"] {
                style["border_width_" + side] = .number(4)
                style["border_color_" + side] = .array([.number(255), .number(0), .number(0), .number(255)])
                style["border_style_" + side] = .string(pattern)
            }
            let pixels = render(style, dark: false, page: CGColor(gray: 1, alpha: 1))
            let rows = (0..<70).map { y in (10..<90).map { pixels[(y * 100 + $0) * 4 + 1] } }
            let borderRows = rows.filter { $0.filter { $0 < 100 }.count > 4 }
            XCTAssertFalse(borderRows.isEmpty, "\(pattern) has ink")
            XCTAssertTrue(borderRows.allSatisfy { $0.contains(where: { $0 > 245 }) }, "\(pattern) leaves gaps")
        }
    }

    /// LLP 1066: the gradient page's cases (`GradientParity`), light then dark.
    func testEveryGradientCaseMatchesChrome() {
        GradientParity.checkLengthResize()
        var failures = GradientParity.check(dark: false) { render($0, dark: $1, page: $2) }
        failures += GradientParity.check(dark: true) { render($0, dark: $1, page: $2) }
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }
}
#endif
