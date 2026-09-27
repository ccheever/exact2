#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// The parity page's border cases as AppKit draws them (`BorderParity`).
final class BorderParityMacTests: XCTestCase {
    private func render(_ style: NodeStyle, dark: Bool, page: CGColor) -> [UInt8] {
        _ = NSApplication.shared
        let p = Presenter()
        let node = NodeView(id: 1, kind: "view", presenter: p)
        node.frame = NSRect(x: 0, y: 0, width: 100, height: 70)
        node.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        p.root.addSubview(node); p.views[1] = node
        node.applyStyle(style)
        let ctx = BorderParity.canvas(page)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
        // What the layer's clip-path mask does on screen; `draw` alone does not.
        if let path = node.clipPath { ctx.addPath(path); ctx.clip() }
        node.draw(node.bounds)
        NSGraphicsContext.restoreGraphicsState()
        return BorderParity.bytes(ctx)
    }

    func testEveryBorderCaseMatchesChrome() {
        var failures = BorderParity.check(flipped: false, dark: false) { render($0, dark: $1, page: $2) }
        failures += BorderParity.check(flipped: true, dark: true) { render($0, dark: $1, page: $2) }
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }

    /// LLP 1066: the gradient page's cases (`GradientParity`), light then dark.
    func testEveryGradientCaseMatchesChrome() {
        var failures = GradientParity.check(dark: false) { render($0, dark: $1, page: $2) }
        failures += GradientParity.check(dark: true) { render($0, dark: $1, page: $2) }
        XCTAssert(failures.isEmpty, failures.joined(separator: "\n"))
    }
}
#endif
