#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
@testable import ExactKit

/// Stage 9 A (LLP 1121 D10): an icon token with no `sf` source and a
/// template SVG draws its own path on Apple, from the commands the Rust side
/// sends (`symbolPath`, `symbolViewBox`), as a template image that takes the
/// node's `-exact-tint-color` where written, else its `color`; an `sf` still
/// draws its symbol.
///   bun host/apple/build.mjs --test --ios   (and --test for the Mac)
final class IconPathTests: XCTestCase {
    #if os(iOS)
    private var window: UIWindow!
    override func tearDown() { MainActor.assumeIsolated { window = nil } }
    #else
    private var window: NSWindow!
    override func tearDown() { MainActor.assumeIsolated { window?.close(); window = nil } }
    #endif

    private let square = "M 2 2 L 22 2 L 22 22 L 2 22 Z"

    private func icon(_ id: Int, _ style: [String: Any], sf: String = "") -> [[String: Any]] {
        var props: [String: Any] = ["imageSource": "symbol:icon|\(sf)|0 0 24 24|M2 2 L22 2 L22 22 L2 22 Z|icons/x.svg", "symbolName": sf]
        if sf.isEmpty { props["symbolPath"] = square; props["symbolViewBox"] = "0 0 24 24" }
        return [["op": "create", "id": id, "kind": "image", "props": props, "handlers": [], "style": ["font_size": 24.0].merging(style) { $1 }],
                ["op": "frame", "id": id, "x": 0.0, "y": Double(id) * 30, "w": 24.0, "h": 24.0]]
    }

    private func presenter(_ ops: [[String: Any]]) -> Presenter {
        let p = Presenter()
        #if os(iOS)
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        #else
        _ = NSApplication.shared
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        #endif
        p.apply(wireBatch([["op": "create", "id": 1, "kind": "view", "style": [:]]] + ops
                          + [["op": "children", "id": 1, "ids": ops.compactMap { $0["op"] as? String == "create" ? $0["id"] : nil }], ["op": "roots", "ids": [1]],
                             ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0]]))
        return p
    }

    private func rgb(_ c: PlatformColor?) -> [Int] {
        guard let c else { return [] }
        #if os(iOS)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        c.getRed(&r, green: &g, blue: &b, alpha: &a)
        #else
        guard let s = c.usingColorSpace(.sRGB) else { return [] }
        let r = s.redComponent, g = s.greenComponent, b = s.blueComponent
        #endif
        return [r, g, b].map { Int(($0 * 255).rounded()) }
    }

    private func tint(_ v: NodeView?) -> PlatformColor? {
        #if os(iOS)
        return (v?.symbolView as? UIImageView)?.tintColor
        #else
        return (v?.symbolView as? NSImageView)?.contentTintColor
        #endif
    }

    @MainActor func testAnSvgOnlyIconDrawsItsPathTintedByItsTintElseItsColour() throws {
        let p = presenter(icon(2, ["tint_color": [52, 199, 89, 255]]) + icon(3, ["text_color": [232, 28, 36, 255]]))
        let tinted = try XCTUnwrap(p.views[2]), coloured = try XCTUnwrap(p.views[3])
        let image = try XCTUnwrap(tinted.image, "the path drawn, no SF Symbol needed")
        XCTAssertEqual(image.size.width, 24, "an em square")
        #if os(iOS)
        XCTAssertEqual(image.renderingMode, .alwaysTemplate, "it takes the tint")
        #else
        XCTAssertTrue(image.isTemplate, "it takes the tint")
        #endif
        XCTAssertTrue(tinted.symbolFound)
        XCTAssertEqual(rgb(tint(tinted)), [52, 199, 89], "`-exact-tint-color`")
        XCTAssertEqual(rgb(tint(coloured)), [232, 28, 36], "no tint written: its `color`")
        // A new tint (a token's retheme reaches it as any row) tints it again.
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["font_size": 24.0, "tint_color": [0, 122, 255, 255]]]]))
        XCTAssertEqual(rgb(tint(tinted)), [0, 122, 255])
    }

    @MainActor func testAnIconWithAnSFSourceStillDrawsItsSymbol() throws {
        let p = presenter(icon(2, [:], sf: "heart.fill"))
        let v = try XCTUnwrap(p.views[2])
        XCTAssertNotNil(v.image)
        XCTAssertNil(v.props["symbolPath"])
        XCTAssertTrue(v.symbolFound)
    }

    func testThePathFillsItsViewBoxInItsAspect() throws {
        let path = try XCTUnwrap(IconPath.path(square))
        XCTAssertEqual(path.boundingBox, CGRect(x: 2, y: 2, width: 20, height: 20))
        // A 48-wide box keeps a 24 viewBox square, centred.
        let t = IconPath.fitted(IconPath.viewBox("0 0 24 24"), in: CGRect(x: 0, y: 0, width: 48, height: 24))
        XCTAssertEqual(CGPoint(x: 0, y: 0).applying(t), CGPoint(x: 12, y: 0))
        XCTAssertNil(IconPath.path("Q 1 2"), "a verb it doesn't send")
    }
}
#endif
