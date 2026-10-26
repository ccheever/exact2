#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
import ExactCore
@testable import ExactKit

/// Stage 9 A (LLP 1121 D10): an icon token with no `sf` source and a
/// template SVG draws its own path on Apple, from the commands the Rust side
/// sends (`symbolPath`, `symbolViewBox`), as a template image that takes the
/// node's computed `color`; an `sf` still draws its symbol.
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
        if style["text_color"] != nil { props["symbolColorWritten"] = "1" }
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
        // A path icon fills its paths in its tint (stage 14 C1, IconShapeMac.swift).
        if let fill = v?.symbolShape?.shapes.first?.fillColor { return NSColor(cgColor: fill) }
        return (v?.symbolView as? NSImageView)?.contentTintColor
        #endif
    }

    @MainActor func testAnSvgOnlyIconDrawsItsPathInItsComputedColour() throws {
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
        XCTAssertEqual(rgb(tint(tinted)), rgb(tinted.symbolInk), "raster tint does not color a symbol")
        XCTAssertEqual(rgb(tint(coloured)), [232, 28, 36], "no tint written: its `color`")
        // A color that names the accent follows it, as an SF Symbol's does.
        let accent = presenter(icon(4, ["text_color": ["sys": "@tint"]]))
        XCTAssertEqual(rgb(tint(accent.views[4])), rgb(accent.views[4]?.inheritedTint), "the accent")
        // A token retheme changes its computed color.
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["font_size": 24.0, "text_color": [0, 122, 255, 255]]]]))
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
        // The em square placed by object-fit, as Linux places it (the widget snapshot's model).
        let box = CGRect(x: 0, y: 0, width: 96, height: 48)
        XCTAssertEqual(IconPath.placed(em: 24, fit: "fill", in: box), CGRect(x: 24, y: 0, width: 48, height: 48))
        XCTAssertEqual(IconPath.placed(em: 24, fit: "cover", in: box), CGRect(x: 0, y: -24, width: 96, height: 96))
        XCTAssertEqual(IconPath.placed(em: 24, fit: "none", in: box), CGRect(x: 36, y: 12, width: 24, height: 24))
        XCTAssertEqual(IconPath.placed(em: 24, fit: "scale-down", in: box).width, 24, "it fits: its own size")
        XCTAssertEqual(IconPath.placed(em: 64, fit: "scale-down", in: box).width, 48, "it doesn't: contained")
    }

    #if os(iOS)
    /// Drawn again at a box it fills, and its em square again once centred.
    @MainActor func testAPathIconFollowsItsBoxAndIsItsEmSquareCentred() throws {
        let p = presenter(icon(2, [:]))
        let v = try XCTUnwrap(p.views[2]), leaf = try XCTUnwrap(v.symbolView as? UIImageView)
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 60.0, "w": 96.0, "h": 96.0]]))
        v.layoutSymbol()
        XCTAssertEqual(leaf.image?.size.width, 96, "drawn at the box it fills")
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["font_size": 24.0, "object_fit": "none"]]]))
        v.layoutSymbol()
        XCTAssertEqual(leaf.contentMode, .center)
        XCTAssertEqual(leaf.image?.size.width, 24, "its em square, not the box's bitmap")
    }
    #endif

    /// The image's pixels: an asymmetric bar near its top inks its top rows
    /// only (right side up on both platforms), and two paths of opposite
    /// winding that overlap stay solid, each filled on its own.
    func testTheImageInksItsPathsRightSideUpEachOnItsOwn() throws {
        let bar = "M 0 2 L 24 2 L 24 6 L 0 6 Z"
        let rows = try inked(bar, at: [3, 4, 12, 20])
        XCTAssertEqual(rows, [true, true, false, false], "the bar at the top, nothing below")
        // An outer square clockwise and an inner one counter-clockwise, as two paths: solid where they overlap.
        let two = "M 2 2 L 22 2 L 22 22 L 2 22 Z; M 8 8 L 8 16 L 16 16 L 16 8 Z"
        XCTAssertEqual(try inked(two, at: [12]), [true], "the overlap is filled, not cut")
    }

    /// Stage 14 C2: an even-odd icon (`symbolFillRule`) leaves the hole of
    /// two squares wound the same way clear, as one path; nonzero fills it.
    func testAnEvenOddIconLeavesItsSameWoundHoleClear() throws {
        XCTAssertEqual(IconPath.rule("evenodd"), .evenOdd)
        XCTAssertEqual(IconPath.rule(nil), .winding)
        XCTAssertEqual(IconPath.rule("nonzero"), .winding)
        XCTAssertEqual(try inked(Self.frame, at: [4, 12], rule: .evenOdd), [true, false], "the band inked, the hole clear")
        XCTAssertEqual(try inked(Self.frame, at: [4, 12], rule: .winding), [true, true], "nonzero fills the hole")
    }

    /// Two squares wound the same way, as one path: a frame even-odd, a block nonzero.
    static let frame = "M 2 2 L 22 2 L 22 22 L 2 22 Z M 8 8 L 16 8 L 16 16 L 8 16 Z"

    /// The same, on a node: its image (iOS's leaf, the Mac's face image) and,
    /// on the Mac, its shape layers take the node's `symbolFillRule`.
    @MainActor func testAnEvenOddIconNodeDrawsItsRule() throws {
        var ops = icon(2, [:]) + icon(3, [:])
        for (i, rule) in [(0, "evenodd"), (2, nil)] as [(Int, String?)] {
            var props = ops[i]["props"] as! [String: Any]
            props["symbolPath"] = Self.frame
            if let rule { props["symbolFillRule"] = rule }
            ops[i]["props"] = props
        }
        let p = presenter(ops)
        let even = try XCTUnwrap(p.views[2]), plain = try XCTUnwrap(p.views[3])
        XCTAssertEqual(try Self.inked(try XCTUnwrap(even.iconPathImage(side: 24)), at: [4, 12]), [true, false])
        XCTAssertEqual(try Self.inked(try XCTUnwrap(plain.iconPathImage(side: 24)), at: [4, 12]), [true, true])
        // As drawn on the screen, at a fractional origin: the hole clear, the band inked.
        XCTAssertEqual(try drawn(p, even, at: CGPoint(x: 40.5, y: 60.25)), [true, false], "even-odd, drawn")
        XCTAssertEqual(try drawn(p, plain, at: CGPoint(x: 100.33, y: 60.67)), [true, true], "nonzero, drawn")
        #if os(macOS)
        XCTAssertEqual(even.symbolShape?.shapes.map(\.fillRule), [.evenOdd], "C1's shape layer fills even-odd")
        XCTAssertEqual(plain.symbolShape?.shapes.map(\.fillRule), [.nonZero])
        #endif
        // The rule alone changes, each way, through the batch: drawn again.
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["symbolFillRule"]]]))
        XCTAssertEqual(try drawn(p, even, at: CGPoint(x: 40.5, y: 60.25)), [true, true], "cleared: nonzero")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["symbolFillRule": "evenodd"]]]))
        XCTAssertEqual(try drawn(p, even, at: CGPoint(x: 40.5, y: 60.25)), [true, false], "said again: even-odd")
    }

    /// `node` (a 24-point icon) moved to `origin` and drawn as the screen
    /// shows it, 2 pixels a point: whether the middle column is inked 4 and
    /// 12 points down.
    @MainActor private func drawn(_ p: Presenter, _ node: NodeView, at origin: CGPoint) throws -> [Bool] {
        p.apply(wireBatch([["op": "frame", "id": Int(node.id), "x": Double(origin.x), "y": Double(origin.y), "w": 24.0, "h": 24.0]]))
        node.layoutSymbol()
        let scale: CGFloat = 2, side = 200
        let w = side * Int(scale)
        let ctx = try XCTUnwrap(CGContext(data: nil, width: w, height: w, bitsPerComponent: 8, bytesPerRow: w * 4,
                                          space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        #if os(iOS)
        ctx.translateBy(x: 0, y: CGFloat(w)); ctx.scaleBy(x: scale, y: -scale)
        p.viewport.layoutIfNeeded()
        p.viewport.layer.render(in: ctx)
        #else
        p.viewport.layoutSubtreeIfNeeded()
        let rep = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: w, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                                                 isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: w * 4, bitsPerPixel: 32))
        rep.size = p.viewport.bounds.size
        Capture.draw(p.viewport, to: rep) // as IconShapeMacTests captures C1's layers
        let cg = try XCTUnwrap(rep.cgImage)
        ctx.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: w))
        #endif
        let pixels = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
        // (Its buffer holds its top row first; the viewport is flipped on both.)
        // Inked: far from the page's ground, read at the page's corner (the
        // ground may be opaque, so not by alpha).
        func rgba(_ x: Int, _ y: Int) -> [Int] { (0..<4).map { Int(pixels[(y * w + x) * 4 + $0]) } }
        let ground = rgba(2, 2)
        let x = Int(((origin.x + 12) * scale).rounded(.down))
        return [4, 12].map { row in
            let y = Int(((origin.y + CGFloat(row)) * scale).rounded(.down))
            return zip(rgba(x, y), ground).map { abs($0 - $1) }.reduce(0, +) > 96
        }
    }

    /// Whether the image at 24 points inks the middle column at each row.
    private func inked(_ commands: String, at rows: [Int], rule: CGPathFillRule = .winding) throws -> [Bool] {
        try Self.inked(try XCTUnwrap(IconPath.image(commands, viewBox: "0 0 24 24", size: CGSize(width: 24, height: 24), rule: rule)), at: rows)
    }

    /// Whether `image`, drawn 24 pixels square, inks the middle column at each row.
    static func inked(_ image: IconImage, at rows: [Int]) throws -> [Bool] {
        let side = 24
        #if os(iOS)
        let cg = try XCTUnwrap(image.cgImage)
        #else
        var rect = CGRect(x: 0, y: 0, width: side, height: side)
        let cg = try XCTUnwrap(image.cgImage(forProposedRect: &rect, context: nil, hints: nil))
        #endif
        let ctx = try XCTUnwrap(CGContext(data: nil, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                          space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.draw(cg, in: CGRect(x: 0, y: 0, width: side, height: side))
        // (The context's own buffer holds its top row first.)
        let pixels = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
        return rows.map { row in pixels[(row * side + side / 2) * 4 + 3] > 128 }
    }
}
#endif
