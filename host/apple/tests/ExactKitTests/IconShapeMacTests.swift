// LLP 1121 stage 14 C1 (theme2's 07:01 and 07:02 rulings): on the Mac a
// template-path icon (an image node with a `symbolPath`, no SF Symbol) is
// filled as its paths at the node's own fractional frame, not drawn by an
// NSImageView resampling a template image off the pixel grid.
//   bun host/apple/build.mjs --test
//   cd host/apple && EXACT_CONTRACT=$PWD/../../target/debug/contract xcrun xctest -XCTest ExactKitTests.IconShapeMacTests <bundle>
#if os(macOS)
import AppKit
import XCTest
import ExactCore
@testable import ExactKit

final class IconShapeMacTests: XCTestCase {
    private var window: NSWindow!
    private var windows: [NSWindow] = []
    override func tearDown() { MainActor.assumeIsolated { windows.forEach { $0.close() }; windows = []; window = nil } }

    // MARK: Icons, written once as segments: the host is sent their `d`, the
    // reference builds its CGPaths from the same numbers through Core Graphics.

    private enum Seg { case m(CGFloat, CGFloat), l(CGFloat, CGFloat), c(CGFloat, CGFloat, CGFloat, CGFloat, CGFloat, CGFloat), z }
    /// An icon: each `;` path's segments (its subpaths one after another).
    private typealias Icon = [[Seg]]

    /// A circle as four cubics, clockwise on screen (y down), or the other way.
    private static func circle(_ cx: CGFloat, _ cy: CGFloat, _ r: CGFloat, reversed: Bool = false) -> [Seg] {
        let k = 0.5523 * r
        if !reversed {
            return [.m(cx + r, cy), .c(cx + r, cy + k, cx + k, cy + r, cx, cy + r), .c(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
                    .c(cx - r, cy - k, cx - k, cy - r, cx, cy - r), .c(cx + k, cy - r, cx + r, cy - k, cx + r, cy), .z]
        }
        return [.m(cx + r, cy), .c(cx + r, cy - k, cx + k, cy - r, cx, cy - r), .c(cx - k, cy - r, cx - r, cy - k, cx - r, cy),
                .c(cx - r, cy + k, cx - k, cy + r, cx, cy + r), .c(cx + k, cy + r, cx + r, cy + k, cx + r, cy), .z]
    }
    private static func rect(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat) -> [Seg] {
        [.m(x, y), .l(x + w, y), .l(x + w, y + h), .l(x, y + h), .z]
    }
    /// The probe's glyph, nonzero: a ring (its hole wound the other way) as
    /// one path; a bar and a corner flag as another, overlapping it, so each
    /// path is filled on its own and the flag says which way up it is.
    private static let ring: Icon = [circle(12, 12, 9) + circle(12, 12, 5.5, reversed: true),
                                     rect(10.3, 1.2, 3.4, 21.6) + [.m(16, 2), .l(22, 2), .l(22, 8), .z]]
    /// Stage 14 C2's glyph, even-odd: the ring's hole wound the same way, so
    /// only the even-odd rule leaves it clear, beside the same bar and flag.
    private static let evenRing: Icon = [circle(12, 12, 9) + circle(12, 12, 5.5),
                                         rect(10.3, 1.2, 3.4, 21.6) + [.m(16, 2), .l(22, 2), .l(22, 8), .z]]
    /// Another icon's d (the wrong-d mutation): a plus and a dot.
    private static let plus: Icon = [rect(3, 10, 18, 4) + rect(10, 3, 4, 18), circle(19, 19, 2.5)]

    private static func d(_ icon: Icon) -> String {
        func f(_ v: CGFloat) -> String { String(format: "%g", Double(v)) }
        return icon.map { path in
            path.map { seg -> String in
                switch seg {
                case .m(let x, let y): return "M \(f(x)) \(f(y))"
                case .l(let x, let y): return "L \(f(x)) \(f(y))"
                case .c(let a, let b, let c, let d, let x, let y): return "C \(f(a)) \(f(b)) \(f(c)) \(f(d)) \(f(x)) \(f(y))"
                case .z: return "Z"
                }
            }.joined(separator: " ")
        }.joined(separator: "; ")
    }

    /// The reference: each path built by Core Graphics' own calls, the 24
    /// viewBox scaled into `frame` (a square), filled nonzero on its own into
    /// a bitmap `scale` pixels a point, its first row at the top. No ExactKit
    /// or IconPath code draws it.
    private static func reference(_ icon: Icon, frame: CGRect, canvas: CGSize, scale: CGFloat, rule: CGPathFillRule = .winding) -> [UInt8] {
        let w = Int(canvas.width * scale), h = Int(canvas.height * scale)
        let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpaceCreateDeviceRGB(),
                            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.translateBy(x: 0, y: CGFloat(h)); ctx.scaleBy(x: scale, y: -scale)
        ctx.translateBy(x: frame.minX, y: frame.minY); ctx.scaleBy(x: frame.width / 24, y: frame.height / 24)
        ctx.setFillColor(CGColor(red: 0, green: 0, blue: 0, alpha: 1))
        for path in icon {
            let p = CGMutablePath()
            for seg in path {
                switch seg {
                case .m(let x, let y): p.move(to: CGPoint(x: x, y: y))
                case .l(let x, let y): p.addLine(to: CGPoint(x: x, y: y))
                case .c(let a, let b, let c, let d, let x, let y): p.addCurve(to: CGPoint(x: x, y: y), control1: CGPoint(x: a, y: b), control2: CGPoint(x: c, y: d))
                case .z: p.closeSubpath()
                }
            }
            ctx.addPath(p); ctx.fillPath(using: rule)
        }
        let data = ctx.data!.assumingMemoryBound(to: UInt8.self)
        return (0..<(w * h)).map { data[$0 * 4 + 3] }
    }

    // MARK: The host's node

    private let canvas = CGSize(width: 48, height: 48)

    private func props(_ icon: Icon) -> [String: Any] { props(Self.d(icon)) }
    private func props(_ d: String) -> [String: Any] {
        return ["imageSource": "symbol:icon||0 0 24 24|\(d)|icons/probe.svg", "symbolName": "", "symbolPath": d, "symbolViewBox": "0 0 24 24"]
    }

    /// A page holding image node 2, a path icon, in black at `frame`.
    private func presenter(_ icon: Icon, frame: CGRect, style: [String: Any] = [:], extra: [String: Any] = [:],
                           window make: (NSRect) -> NSWindow = { NSWindow(contentRect: $0, styleMask: [.titled], backing: .buffered, defer: false) }) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        window = make(NSRect(origin: .zero, size: canvas))
        window.isReleasedWhenClosed = false; windows.append(window)
        window.appearance = NSAppearance(named: .aqua)
        window.contentView = p.viewport
        // (An NSNull in `style` leaves that row unsaid.)
        let style = ["font_size": Double(frame.width), "text_color": [0, 0, 0, 255]].merging(style) { $1 }.filter { !($0.value is NSNull) }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": [:]],
            ["op": "create", "id": 2, "kind": "image", "props": props(icon).merging(extra) { $1 }, "handlers": [],
             "style": style],
            ["op": "frame", "id": 2, "x": Double(frame.minX), "y": Double(frame.minY), "w": Double(frame.width), "h": Double(frame.height)],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": Double(canvas.width), "h": Double(canvas.height)],
        ]))
        return p
    }

    private func move(_ p: Presenter, to frame: CGRect) {
        p.apply(wireBatch([["op": "frame", "id": 2, "x": Double(frame.minX), "y": Double(frame.minY), "w": Double(frame.width), "h": Double(frame.height)]]))
    }

    /// `view`'s coverage (alpha) as the host captures it, `scale` pixels a point.
    private func render(_ view: NSView, scale: CGFloat) -> [UInt8] {
        view.window?.contentView?.layoutSubtreeIfNeeded()
        (view as? NodeView)?.presenter?.views[2]?.layoutSymbol()
        let w = Int(view.bounds.width * scale), h = Int(view.bounds.height * scale)
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                                   isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: w * 4, bitsPerPixel: 32)!
        rep.size = view.bounds.size
        Capture.draw(view, to: rep)
        let data = rep.bitmapData!
        return (0..<(w * h)).map { data[$0 * 4 + 3] }
    }

    /// The old way, for contrast: the path's template image (`IconPath.image`,
    /// an em square) in an NSImageView scaling it to the frame, tinted black.
    private func renderOld(_ icon: Icon, frame: CGRect, scale: CGFloat) -> [UInt8] {
        final class Flipped: NSView { override var isFlipped: Bool { true } }
        let page = Flipped(frame: CGRect(origin: .zero, size: canvas))
        let view = NSImageView(frame: frame)
        view.image = IconPath.image(Self.d(icon), viewBox: "0 0 24 24", size: frame.size)
        view.imageScaling = .scaleProportionallyUpOrDown; view.contentTintColor = .black
        page.addSubview(view)
        return render(page, scale: scale)
    }

    // MARK: theme2's checks (05:12 edge rule, 05:15 centroid, spread, coverage)

    private struct Measure: CustomStringConvertible {
        var differing = 0, fringe = 0, outside = 0
        var centroid = 0.0, spread = 0.0, coverage = 0.0
        var hostInk = 0.0, refInk = 0.0
        var equal: Bool { differing == 0 }
        var failures: [String] {
            var out: [String] = []
            if outside > 0 { out.append("outside") }
            if centroid > 0.25 { out.append("centroid") }
            if spread > 0.25 { out.append("spread") }
            if coverage > 0.05 { out.append("coverage") }
            return out
        }
        var passes: Bool { failures.isEmpty }
        var description: String {
            String(format: "differing %d (fringe %d, outside %d), centroid %.3f px, spread %.3f px, coverage %+.2f%% (host ink %.1f of %.1f)",
                   differing, fringe, outside, centroid, spread, coverage * 100 * (hostInk >= refInk ? 1 : -1), hostInk, refInk)
        }
    }

    private static func moments(_ a: [UInt8], width: Int) -> (ink: Double, x: Double, y: Double, spread: Double) {
        var t = 0.0, sx = 0.0, sy = 0.0
        for (i, v) in a.enumerated() where v > 0 {
            let w = Double(v) / 255, x = Double(i % width) + 0.5, y = Double(i / width) + 0.5
            t += w; sx += w * x; sy += w * y
        }
        let cx = sx / t, cy = sy / t
        var r = 0.0
        for (i, v) in a.enumerated() where v > 0 {
            let w = Double(v) / 255, x = Double(i % width) + 0.5 - cx, y = Double(i / width) + 0.5 - cy
            r += w * (x * x + y * y)
        }
        return (t, cx, cy, (r / t).squareRoot())
    }

    /// `host` against `ref`: every differing pixel must be on the reference's
    /// anti-aliased fringe (coverage strictly between none and full, dilated
    /// 1 px); centroid and spread within 0.25 px; total coverage within 5%.
    private static func measure(_ host: [UInt8], _ ref: [UInt8], width: Int) -> Measure {
        let height = ref.count / width
        var fringe = [Bool](repeating: false, count: ref.count)
        for (i, v) in ref.enumerated() where v > 0 && v < 255 {
            let x = i % width, y = i / width
            for dy in -1...1 { for dx in -1...1 {
                let nx = x + dx, ny = y + dy
                if nx >= 0, ny >= 0, nx < width, ny < height { fringe[ny * width + nx] = true }
            } }
        }
        var m = Measure()
        for i in 0..<ref.count where host[i] != ref[i] {
            m.differing += 1
            if fringe[i] { m.fringe += 1 } else { m.outside += 1 }
        }
        let h = moments(host, width: width), r = moments(ref, width: width)
        m.centroid = ((h.x - r.x) * (h.x - r.x) + (h.y - r.y) * (h.y - r.y)).squareRoot()
        m.spread = abs(h.spread - r.spread)
        m.coverage = abs(h.ink - r.ink) / r.ink
        m.hostInk = h.ink; m.refInk = r.ink
        return m
    }

    private static let origins: [CGPoint] = [CGPoint(x: 0, y: 0), CGPoint(x: 0.25, y: 0.25), CGPoint(x: 0.5, y: 0.5),
                                             CGPoint(x: 0.67, y: 0.69), CGPoint(x: 0.84, y: 0.84), CGPoint(x: 0, y: 0.38)]
    private static func frame(_ o: CGPoint, side: CGFloat = 18) -> CGRect { CGRect(x: 12 + o.x, y: 12 + o.y, width: side, height: side) }

    // MARK: Tests

    /// (1) A real node, through `updateSymbol` and `layoutSymbol`, at the
    /// probe's six origins at 1x and 2x, against the independent reference:
    /// identical at a whole-pixel origin, the edge, centroid, spread and
    /// coverage checks at a fractional one. The old NSImageView draw, for
    /// contrast, fails them off the grid.
    @MainActor func testAPathIconIsItsPathFilledAtItsFrame() throws {
        let p = presenter(Self.ring, frame: Self.frame(.zero))
        let node = try XCTUnwrap(p.views[2]), page = try XCTUnwrap(p.views[1])
        XCTAssertNotNil(node.symbolShape, "a path icon's leaf fills its paths")
        XCTAssertNil(node.symbolView, "and no image view draws it")
        XCTAssertEqual(node.symbolShape?.shapes.count, 2, "one shape layer for each `;` path")
        XCTAssertEqual(node.symbolShape?.shapes.first?.fillRule, .nonZero)
        for scale in [CGFloat(1), 2] {
            let width = Int(canvas.width * scale)
            for o in Self.origins {
                let frame = Self.frame(o)
                move(p, to: frame)
                let ref = Self.reference(Self.ring, frame: frame, canvas: canvas, scale: scale)
                let m = Self.measure(render(page, scale: scale), ref, width: width)
                let old = Self.measure(renderOld(Self.ring, frame: frame, scale: scale), ref, width: width)
                let whole = (o.x * scale).rounded() == o.x * scale && (o.y * scale).rounded() == o.y * scale
                print(String(format: "C1 probe %.0fx origin (%.2f, %.2f)%@: host %@ | old NSImageView %@ [%@]", scale, o.x, o.y, whole ? " whole" : "",
                             m.description, old.description, old.failures.joined(separator: ",")))
                if whole {
                    XCTAssertTrue(m.equal, "\(scale)x \(o): every pixel equal at a whole-pixel origin, \(m)")
                } else {
                    XCTAssertTrue(m.passes, "\(scale)x \(o): \(m.failures) \(m)")
                    if scale == 1 { XCTAssertFalse(old.passes, "\(scale)x \(o): the old NSImageView draw fails off the grid, \(old)") }
                }
            }
        }
    }

    /// Stage 14 C2: an even-odd icon (`symbolFillRule`) fills each path
    /// even-odd at its frame, at the six origins at 1x and 2x, against the
    /// independent reference filled even-odd: identical at a whole-pixel
    /// origin, the checks at a fractional one. Filled nonzero, its hole is
    /// ink the checks catch.
    @MainActor func testAnEvenOddPathIconIsItsPathFilledEvenOddAtItsFrame() throws {
        let p = presenter(Self.evenRing, frame: Self.frame(.zero), extra: ["symbolFillRule": "evenodd"])
        let node = try XCTUnwrap(p.views[2]), page = try XCTUnwrap(p.views[1])
        XCTAssertEqual(node.symbolShape?.shapes.map(\.fillRule), [.evenOdd, .evenOdd], "each `;` path even-odd")
        for scale in [CGFloat(1), 2] {
            let width = Int(canvas.width * scale)
            for o in Self.origins {
                let frame = Self.frame(o)
                move(p, to: frame)
                let ref = Self.reference(Self.evenRing, frame: frame, canvas: canvas, scale: scale, rule: .evenOdd)
                let m = Self.measure(render(page, scale: scale), ref, width: width)
                let whole = (o.x * scale).rounded() == o.x * scale && (o.y * scale).rounded() == o.y * scale
                print(String(format: "C2 even-odd %.0fx origin (%.2f, %.2f)%@: host %@", scale, o.x, o.y, whole ? " whole" : "", m.description))
                if whole { XCTAssertTrue(m.equal, "\(scale)x \(o): every pixel equal at a whole-pixel origin, \(m)") }
                else { XCTAssertTrue(m.passes, "\(scale)x \(o): \(m.failures) \(m)") }
            }
        }
        // The rule unsaid (nonzero): the hole fills, outside the reference's fringe.
        let o = CGPoint(x: 0.67, y: 0.69), frame = Self.frame(o)
        let nonzero = presenter(Self.evenRing, frame: frame)
        let ref = Self.reference(Self.evenRing, frame: frame, canvas: canvas, scale: 2, rule: .evenOdd)
        let m = Self.measure(render(try XCTUnwrap(nonzero.views[1]), scale: 2), ref, width: Int(canvas.width * 2))
        print("C2 mutation nonzero: \(m) -> fails \(m.failures)")
        XCTAssertTrue(m.failures.contains("outside"), "filled nonzero, the hole is caught: \(m)")
    }

    /// (1) The checks catch what they are for: a 1 px shift (centroid), the
    /// icon 10% larger about its centre (spread) and another icon's d
    /// (outside the fringe), each on the real node, at 1x and 2x.
    @MainActor func testTheChecksCatchAShiftAScaleAndAnotherIcon() throws {
        let o = CGPoint(x: 0.67, y: 0.69), frame = Self.frame(o)
        for scale in [CGFloat(1), 2] {
            let width = Int(canvas.width * scale)
            let ref = Self.reference(Self.ring, frame: frame, canvas: canvas, scale: scale)
            func run(_ name: String, _ icon: Icon, _ at: CGRect, caughtBy check: String) throws {
                let p = presenter(icon, frame: at)
                let m = Self.measure(render(try XCTUnwrap(p.views[1]), scale: scale), ref, width: width)
                print(String(format: "C1 mutation %.0fx %@: %@ -> fails %@", scale, name, m.description, m.failures.joined(separator: ",")))
                XCTAssertTrue(m.failures.contains(check), "\(scale)x \(name) is caught by \(check): \(m)")
                p.reset()
            }
            try run("1 px shift", Self.ring, frame.offsetBy(dx: 1 / scale, dy: 0), caughtBy: "centroid")
            try run("10% scale", Self.ring, frame.insetBy(dx: -0.9, dy: -0.9), caughtBy: "spread")
            try run("another d", Self.plus, frame, caughtBy: "outside")
        }
    }

    private func rgb(_ c: CGColor?) -> [Int] {
        guard let c = c.flatMap({ NSColor(cgColor: $0)?.usingColorSpace(.sRGB) }) else { return [] }
        return [c.redComponent, c.greenComponent, c.blueComponent].map { Int(($0 * 255).rounded()) }
    }

    /// (2) The layer's fill follows light and dark, and a selected row drawn
    /// emphasized, from the tint source the image view's tint had.
    @MainActor func testTheFillFollowsTheAppearanceAndASelectedRow() throws {
        // Light opaque, dark at 40%: the colour and the leaf's alpha follow each way.
        let pair: [Any] = [[200, 30, 40, 255], [90, 200, 250, 102]]
        let p = presenter(Self.ring, frame: Self.frame(.zero), style: ["text_color": pair])
        let node = try XCTUnwrap(p.views[2]), shape = try XCTUnwrap(node.symbolShape?.shapes.first), leaf = try XCTUnwrap(node.symbolShape)
        XCTAssertEqual(rgb(shape.fillColor), [200, 30, 40], "light")
        XCTAssertEqual(leaf.alphaValue, 1, accuracy: 0.002, "light: opaque")
        window.appearance = NSAppearance(named: .darkAqua)
        node.viewDidChangeEffectiveAppearance()
        XCTAssertEqual(rgb(shape.fillColor), [90, 200, 250], "dark")
        XCTAssertEqual(leaf.alphaValue, 0.4, accuracy: 0.002, "dark: its 40%")
        window.appearance = NSAppearance(named: .aqua)
        node.viewDidChangeEffectiveAppearance()
        XCTAssertEqual(rgb(shape.fillColor), [200, 30, 40], "light again")
        XCTAssertEqual(leaf.alphaValue, 1, accuracy: 0.002, "light again: opaque, not a stale 40%")
        // An unsaid colour is the label's, resolved for the appearance it draws in.
        let unsaid = presenter(Self.ring, frame: Self.frame(.zero), style: ["text_color": NSNull()])
        let plain = try XCTUnwrap(unsaid.views[2]), plainShape = try XCTUnwrap(plain.symbolShape?.shapes.first)
        let light = rgb(plainShape.fillColor)
        window.appearance = NSAppearance(named: .darkAqua)
        plain.viewDidChangeEffectiveAppearance()
        let dark = rgb(plainShape.fillColor)
        print("C1 unsaid fill light \(light), dark \(dark)")
        XCTAssertNotEqual(light, dark, "the unsaid label colour is resolved again in dark")
        // A selected row drawn emphasized: the selected item's ink, as AppKit
        // drew a template cell's image; a colour that names a role follows it.
        window.appearance = NSAppearance(named: .aqua)
        let role: [String: Any] = ["sys": "labelColor", "c": [[0, 0, 0, 255], [255, 255, 255, 255]]]
        let row = presenter(Self.ring, frame: Self.frame(.zero), style: ["text_color": role])
        let cell = try XCTUnwrap(row.views[2]), cellShape = try XCTUnwrap(cell.symbolShape?.shapes.first)
        XCTAssertEqual(rgb(cellShape.fillColor), [0, 0, 0])
        // The label colour's alpha against the selected item's, each resolved here.
        var selected: CGColor?, labelAlpha: CGFloat = 0
        cell.effectiveAppearance.performAsCurrentDrawingAppearance {
            selected = NSColor.alternateSelectedControlTextColor.cgColor; labelAlpha = NSColor.labelColor.cgColor.alpha
        }
        let cellLeaf = try XCTUnwrap(cell.symbolShape)
        XCTAssertNotEqual(labelAlpha, try XCTUnwrap(selected).alpha, "the two inks' alphas differ, so a stale one shows")
        XCTAssertEqual(cellLeaf.alphaValue, labelAlpha, accuracy: 0.002, "the label colour's alpha")
        cell.rowEmphasisChanged(true)
        XCTAssertEqual(rgb(cellShape.fillColor), rgb(selected), "selected and emphasized")
        XCTAssertEqual(cellLeaf.alphaValue, try XCTUnwrap(selected).alpha, accuracy: 0.002, "the selected item's alpha")
        cell.rowEmphasisChanged(false)
        XCTAssertEqual(rgb(cellShape.fillColor), [0, 0, 0], "not emphasized")
        XCTAssertEqual(cellLeaf.alphaValue, labelAlpha, accuracy: 0.002, "the label colour's alpha again")
        // A colour its author fixed stays the author's in the row (TableInkMac).
        node.rowEmphasisChanged(true)
        XCTAssertEqual(rgb(shape.fillColor), [200, 30, 40], "a fixed colour stays")
        // The image view draws the same in that row (the ink it stands for).
        let image = NSImageView(frame: CGRect(x: 0, y: 0, width: 18, height: 18))
        image.image = IconPath.image(Self.d(Self.ring), viewBox: "0 0 24 24", size: CGSize(width: 18, height: 18))
        image.contentTintColor = .black; image.cell?.backgroundStyle = .emphasized
        let rep = try XCTUnwrap(image.bitmapImageRepForCachingDisplay(in: image.bounds))
        image.cacheDisplay(in: image.bounds, to: rep)
        let drawn = try XCTUnwrap(rep.colorAt(x: rep.pixelsWide / 2, y: 2)?.usingColorSpace(.sRGB))
        XCTAssertEqual([drawn.redComponent, drawn.greenComponent, drawn.blueComponent].map { Int(($0 * 255).rounded()) }, rgb(selected),
                       "an emphasized template cell image is the selected item's ink whatever tinted it")
    }

    /// (3) A node whose source goes path, SF Symbol, path again keeps one
    /// leaf in its clip, the right kind each time.
    @MainActor func testASourceThatChangesKindSwapsItsOneLeaf() throws {
        let p = presenter(Self.ring, frame: Self.frame(.zero))
        let node = try XCTUnwrap(p.views[2])
        func leaves() throws -> [NSView] { try XCTUnwrap(node.symbolClip).subviews }
        let clip = try XCTUnwrap(node.symbolClip)
        XCTAssertEqual(try leaves().count, 1); XCTAssertTrue(try leaves().first is IconShapeView)
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["imageSource": "symbol:sf/heart.fill", "symbolName": "heart.fill"], "clear": ["symbolPath", "symbolViewBox"]]]))
        XCTAssertEqual(try leaves().count, 1); XCTAssertTrue(try leaves().first is NSImageView)
        XCTAssertNil(node.symbolShape); XCTAssertNotNil(node.symbolView?.image)
        XCTAssertTrue(node.symbolClip === clip, "the clip stays")
        p.apply(wireBatch([["op": "props", "id": 2, "set": props(Self.plus), "clear": []]]))
        XCTAssertEqual(try leaves().count, 1); XCTAssertTrue(try leaves().first is IconShapeView)
        XCTAssertNil(node.symbolView); XCTAssertEqual(node.symbolShape?.shapes.count, 2)
        XCTAssertEqual(node.subviews.filter { $0 === clip }.count, 1)
        XCTAssertEqual(node.subviews.filter { $0.subviews.contains(where: { $0 is IconShapeView || $0 is NSImageView }) }.count, 1, "one clip")
    }

    /// The node's accessibility is the image's, unchanged: a path icon and
    /// an SF Symbol with the same props answer alike (both went through the
    /// same `updateRoleAccessibility` on main), and the leaf is never an element.
    @MainActor func testAPathIconKeepsItsAccessibility() throws {
        for extra: [String: Any] in [["accessibilityLabel": "Reply"], [:], ["accessibilityLabel": "Reply", "accessibilityElementsHidden": "true"]] {
            let p = presenter(Self.ring, frame: Self.frame(.zero), extra: extra)
            p.apply(wireBatch([
                ["op": "create", "id": 3, "kind": "image", "props": ["imageSource": "symbol:sf/heart.fill", "symbolName": "heart.fill"].merging(extra) { $1 }, "handlers": [], "style": ["font_size": 18.0]],
                ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 18.0, "h": 18.0], ["op": "children", "id": 1, "ids": [2, 3]],
            ]))
            p.syncAccessibility()
            let path = try XCTUnwrap(p.views[2]), sf = try XCTUnwrap(p.views[3])
            func answers(_ v: NodeView) -> [String] {
                [String(v.isAccessibilityElement()), v.accessibilityRole()?.rawValue ?? "-", v.accessibilityLabel() ?? "-", String(v.isAccessibilityHidden())]
            }
            print("C1 accessibility \(extra): path \(answers(path)), sf \(answers(sf))")
            XCTAssertEqual(answers(path), answers(sf), "\(extra)")
            if extra["accessibilityLabel"] != nil, extra["accessibilityElementsHidden"] == nil {
                XCTAssertEqual(answers(path), ["true", "AXImage", "Reply", "false"])
            }
            let leaf = try XCTUnwrap(path.symbolShape)
            XCTAssertFalse(leaf.isAccessibilityElement())
            XCTAssertFalse((path.accessibilityChildren() ?? []).contains { $0 as AnyObject === leaf || $0 as AnyObject === path.symbolClip })
            p.reset()
        }
    }

    /// Intrinsic size and the image a control face reads are as before: the
    /// node's image is still the path's em-square template.
    @MainActor func testTheImageAndItsSizeStay() throws {
        let p = presenter(Self.ring, frame: Self.frame(.zero))
        let node = try XCTUnwrap(p.views[2])
        let image = try XCTUnwrap(node.image)
        XCTAssertEqual(image.size, CGSize(width: 18, height: 18))
        XCTAssertTrue(image.isTemplate)
        XCTAssertTrue(node.faceImage === image, "a native control's face is the template image, as before")
        XCTAssertTrue(node.symbolFound)
    }

    // MARK: Round 1

    /// Coverage of a whole-pixel 1x render at (x, y) points.
    private func alpha(_ a: [UInt8], _ x: Int, _ y: Int) -> Int { Int(a[y * Int(canvas.width) + x]) }

    /// A translucent tint is applied once over the union of the paths, as
    /// the template image was tinted once: where two `;` paths overlap the
    /// ink is no darker, and the whole matches the union filled once at
    /// that alpha (an independent Core Graphics fill). For a written
    /// rgba(0,0,0,.5) and for the label colour (alpha about .85).
    @MainActor func testATranslucentTintIsAppliedOnceOverOverlappingPaths() throws {
        let two: Icon = [Self.rect(2, 2, 12, 12), Self.rect(8, 8, 12, 12)]
        let frame = CGRect(x: 12, y: 12, width: 24, height: 24)
        let label: [String: Any] = ["sys": "labelColor", "c": [[0, 0, 0, 217], [255, 255, 255, 217]]]
        // The alphas expected, found without the host: the literal's, and the
        // system label colour's resolved in the window's (light) appearance.
        var labelAlpha: CGFloat = 0
        try XCTUnwrap(NSAppearance(named: .aqua)).performAsCurrentDrawingAppearance { labelAlpha = NSColor.labelColor.cgColor.alpha }
        XCTAssertGreaterThan(labelAlpha, 0.5); XCTAssertLessThan(labelAlpha, 1)
        for (name, ink, expected) in [("rgba .5", [0, 0, 0, 128] as Any, CGFloat(128) / 255), ("labelColor", label as Any, labelAlpha)] {
            let p = presenter(two, frame: frame, style: ["text_color": ink])
            let node = try XCTUnwrap(p.views[2]), leaf = try XCTUnwrap(node.symbolShape)
            let opacity = expected
            XCTAssertEqual(leaf.alphaValue, expected, accuracy: 0.002, "\(name): the tint's alpha is the leaf's layer opacity")
            XCTAssertEqual(CGFloat(try XCTUnwrap(leaf.layer).opacity), expected, accuracy: 0.002)
            XCTAssertEqual(leaf.shapes.first?.fillColor?.alpha, 1, "\(name): each path is filled opaque")
            let host = render(try XCTUnwrap(p.views[1]), scale: 1)
            let overlap = alpha(host, 12 + 11, 12 + 11), single = alpha(host, 12 + 4, 12 + 4), other = alpha(host, 12 + 17, 12 + 17)
            print("C1 r1 \(name): opacity \(opacity), overlap \(overlap), first only \(single), second only \(other)")
            XCTAssertEqual(overlap, Int((expected * 255).rounded()), accuracy: 1, "\(name): inked at the tint's alpha")
            XCTAssertGreaterThan(single, 0, "\(name): ink is drawn"); XCTAssertGreaterThan(other, 0)
            XCTAssertEqual(overlap, single, "\(name): the overlap is no darker")
            XCTAssertEqual(overlap, other, "\(name)")
            // The union filled once at that alpha.
            let w = Int(canvas.width), h = Int(canvas.height)
            let ctx = try XCTUnwrap(CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpaceCreateDeviceRGB(),
                                              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            ctx.translateBy(x: 0, y: CGFloat(h)); ctx.scaleBy(x: 1, y: -1)
            ctx.setFillColor(CGColor(red: 0, green: 0, blue: 0, alpha: opacity))
            ctx.addRect(CGRect(x: 14, y: 14, width: 12, height: 12)); ctx.addRect(CGRect(x: 20, y: 20, width: 12, height: 12)); ctx.fillPath(using: .winding)
            let data = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
            var worst = 0
            for i in 0..<(w * h) { worst = max(worst, abs(Int(host[i]) - Int(data[i * 4 + 3]))) }
            print("C1 r1 \(name): worst alpha difference from the union filled once: \(worst)")
            XCTAssertLessThanOrEqual(worst, 1, "\(name): the union filled once at that alpha")
            p.reset()
        }
    }

    /// A path that runs past its viewBox is clipped to the placed em square,
    /// as the template image's bounds clipped it: a 48x24 `contain` box
    /// keeps its side gutters empty.
    @MainActor func testAPathIsClippedToItsPlacedEmSquare() throws {
        let p = presenter([], frame: CGRect(x: 0, y: 12, width: 48, height: 24), style: ["font_size": 24.0, "object_fit": "contain"])
        p.apply(wireBatch([["op": "props", "id": 2, "set": props("M -12 8 L 36 8 L 36 16 L -12 16 Z"), "clear": []]]))
        let host = render(try XCTUnwrap(p.views[1]), scale: 1)
        var gutter = 0, square = 0
        for y in 12..<36 { for x in 0..<48 {
            if x < 12 || x >= 36 { gutter = max(gutter, alpha(host, x, y)) } else { square = max(square, alpha(host, x, y)) }
        } }
        print("C1 r1 gutters: most ink in the gutters \(gutter), in the square \(square)")
        XCTAssertEqual(gutter, 0, "the gutters stay empty")
        XCTAssertEqual(square, 255, "the bar inks the square")
        XCTAssertEqual(alpha(host, 12, 24), 255, "from its left edge")
        XCTAssertEqual(alpha(host, 35, 24), 255, "to its right edge")
    }

    /// The paths are placed in the em square's own coordinates: an icon
    /// inked only in its viewBox's top-left quarter, in boxes whose placed
    /// square starts away from the content box's corner (across, then down),
    /// lands where the square's corner puts it, computed here by hand.
    @MainActor func testAnAsymmetricPathLandsInItsPlacedSquare() throws {
        let corner = "M 0 0 L 6 0 L 6 6 L 0 6 Z"
        // A 40x24 box at (4, 12): the 24 square starts 8 across, at (12, 12) on the page;
        // a 24x40 box at (12, 4): 8 down, at (12, 12) too. The quarter is 6 x 6 there.
        for box in [CGRect(x: 4, y: 12, width: 40, height: 24), CGRect(x: 12, y: 4, width: 24, height: 40)] {
            let p = presenter([], frame: box, style: ["font_size": 24.0, "object_fit": "contain"])
            p.apply(wireBatch([["op": "props", "id": 2, "set": props(corner), "clear": []]]))
            let host = render(try XCTUnwrap(p.views[1]), scale: 1)
            var wrong: [String] = []
            for y in 0..<Int(canvas.height) { for x in 0..<Int(canvas.width) {
                let want = (12..<18).contains(x) && (12..<18).contains(y) ? 255 : 0
                if alpha(host, x, y) != want { wrong.append("(\(x),\(y))=\(alpha(host, x, y))") }
            } }
            print("C1 r2 corner in \(box): \(wrong.count) pixels off")
            XCTAssertEqual(wrong, [], "\(box): ink exactly at (12,12)-(18,18)")
            p.reset()
        }
    }

    /// A path icon is coloured by its computed `color`, alpha and all
    /// (docs/agent-pitfalls.md: a `symbol:` image ignores `-exact-tint-color`):
    /// `color="-exact-secondary-label"` gives the leaf's alpha and the drawn
    /// pixels' alpha NSColor.secondaryLabelColor's in light and in dark, each
    /// resolved here; a tint with no colour draws the text colour.
    @MainActor func testAPathIconTakesItsColourNotItsTint() throws {
        let square: Icon = [Self.rect(0, 0, 24, 24)]
        let frame = CGRect(x: 12, y: 12, width: 24, height: 24)
        func resolved(_ color: NSColor, _ name: NSAppearance.Name) throws -> CGColor {
            var out: CGColor?
            try XCTUnwrap(NSAppearance(named: name)).performAsCurrentDrawingAppearance { out = color.cgColor }
            return try XCTUnwrap(out)
        }
        let secondary: [String: Any] = ["sys": "secondaryLabelColor", "c": [[60, 60, 67, 153], [235, 235, 245, 153]]]
        let teal: [Any] = [[48, 176, 199, 255], [64, 200, 224, 255]]
        for (name, style, color) in [("color secondary label", ["text_color": secondary] as [String: Any], NSColor.secondaryLabelColor),
                                     ("tint only", ["text_color": NSNull(), "tint_color": teal] as [String: Any], NSColor.textColor)] {
            let p = presenter(square, frame: frame, style: style)
            let node = try XCTUnwrap(p.views[2]), leaf = try XCTUnwrap(node.symbolShape), page = try XCTUnwrap(p.views[1])
            for appearance in [NSAppearance.Name.aqua, .darkAqua] {
                window.appearance = NSAppearance(named: appearance)
                node.viewDidChangeEffectiveAppearance()
                let want = try resolved(color, appearance)
                let drawn = alpha(render(page, scale: 1), 24, 24)
                print("C1 colour \(name) \(appearance.rawValue): expected alpha \(want.alpha), leaf \(leaf.alphaValue), pixel \(drawn), fill \(rgb(leaf.shapes.first?.fillColor)) vs \(rgb(want))")
                XCTAssertEqual(leaf.alphaValue, want.alpha, accuracy: 0.002, "\(name) \(appearance.rawValue): the leaf's alpha")
                XCTAssertEqual(drawn, Int((want.alpha * 255).rounded()), accuracy: 1, "\(name) \(appearance.rawValue): the pixels' alpha")
                XCTAssertEqual(rgb(leaf.shapes.first?.fillColor), rgb(want.copy(alpha: 1)), "\(name) \(appearance.rawValue): its colour, not its tint")
            }
            p.reset()
        }
    }

    /// A window whose backing scale changes: the shape layers' contentsScale
    /// follows (AppKit's `viewDidChangeBackingProperties`, sent here by hand,
    /// since a test can't move a window to another screen).
    @MainActor func testTheContentsScaleFollowsTheBackingScale() throws {
        final class Scaled: NSWindow {
            static var scale: CGFloat = 3
            override var backingScaleFactor: CGFloat { Scaled.scale }
        }
        Scaled.scale = 3
        let p = presenter(Self.ring, frame: Self.frame(.zero), window: { Scaled(contentRect: $0, styleMask: [.titled], backing: .buffered, defer: false) })
        let leaf = try XCTUnwrap(p.views[2]?.symbolShape)
        XCTAssertEqual(leaf.shapes.map(\.contentsScale), [3, 3])
        XCTAssertEqual(leaf.group.contentsScale, 3)
        Scaled.scale = 1
        leaf.viewDidChangeBackingProperties()
        XCTAssertEqual(leaf.shapes.map(\.contentsScale), [1, 1])
        XCTAssertEqual(leaf.group.contentsScale, 1)
    }
}
#endif
