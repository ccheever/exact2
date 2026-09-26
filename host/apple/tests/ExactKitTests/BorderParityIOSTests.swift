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
}
#endif
