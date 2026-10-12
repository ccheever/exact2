import CoreGraphics
import ImageIO
import XCTest
@testable import ExactKit

/// LLP 1053 G2: per-side border colours as the node view paints them, held
/// to Chrome's pictures of the parity page (`scripts/fixtures/borders.contract`,
/// the web host at 1×): each case's box, over the page's colour, within an
/// antialiasing band. The styles are what the host's Rust side sends for
/// the page — every side's colour resolved (`currentcolor` against `color`),
/// `light-dark()` a pair the view resolves by its appearance.
enum BorderParity {
    typealias RGBA = [UInt8]
    struct Case { let name: String; let x: Int; let y: Int; let style: NodeStyle }

    static let fixtures = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().appendingPathComponent("../../../../scripts/fixtures").standardized

    private static func c(_ hex: UInt32) -> BatchValue {
        [Double(hex >> 24), Double(hex >> 16 & 255), Double(hex >> 8 & 255), Double(hex & 255)].batch
    }
    private static func pair(_ light: UInt32, _ dark: UInt32) -> BatchValue { .array([c(light), c(dark)]) }
    private static let (R, G, B, Y) = (c(0xe53935ff), c(0x43a047ff), c(0x1e88e5ff), c(0xfdd835ff))

    private static func box(_ widths: [Double], _ colors: [BatchValue], radii: [Double], background: Bool) -> NodeStyle {
        var s: NodeStyle = [:]
        for (i, side) in ["top", "right", "bottom", "left"].enumerated() {
            s["border_width_" + side] = .number(widths[i]); s["border_color_" + side] = colors[i]
        }
        for (i, corner) in ["top_left", "top_right", "bottom_right", "bottom_left"].enumerated() { s["border_radius_" + corner] = .number(radii[i]) }
        if background { s["background_color"] = c(0xeeeeeeff) }
        return s
    }

    /// The page's twelve cases; `flipped` is after the `flip` tap.
    static func cases(flipped: Bool) -> [Case] {
        let rgby = [R, G, B, Y], ten: [Double] = [10, 10, 10, 10], black = c(0x000000ff)
        let styles: [(String, NodeStyle)] = [
            ("square4", box(ten, rgby, radii: [0, 0, 0, 0], background: true)),
            ("round4", box(ten, rgby, radii: [24, 24, 24, 24], background: true)),
            ("unequal", box([4, 12, 20, 8], rgby, radii: [0, 0, 0, 0], background: false)),
            ("unequal-round", box([4, 12, 20, 8], rgby, radii: [30, 30, 30, 30], background: false)),
            ("radii", box([6, 6, 6, 6], rgby, radii: [40, 10, 35, 0], background: true)),
            ("ring", box([2, 10, 2, 10], Array(repeating: c(0x6a1b9aff), count: 4), radii: [20, 20, 20, 20], background: false)),
            ("transparent", box(ten, [R, c(0), B, c(0)], radii: [20, 20, 20, 20], background: true)),
            ("translucent", box([12, 12, 12, 12], [c(0x1e88e580), c(0xe5393580), c(0xe5393580), c(0x1e88e580)], radii: [18, 18, 18, 18], background: true)),
            ("quote", box([0, 0, 0, 6], [black, black, black, R], radii: [12, 12, 12, 12], background: true)),
            ("current", box(ten, flipped ? [G, G, B, B] : [G, G, R, R], radii: [16, 16, 16, 16], background: false)),
            ("scheme", box(ten, [pair(0xe53935ff, 0x80deeaff), pair(0x43a047ff, 0xffcc80ff), B, pair(0xfdd835ff, 0xce93d8ff)], radii: [20, 20, 20, 20], background: false)),
            ("hairline", box([1, 2, 3, 4], rgby, radii: [12, 12, 12, 12], background: false)),
        ]
        return styles.enumerated().map { i, s in Case(name: s.0, x: [20, 145, 270][i % 3], y: [20, 115, 210, 305][i / 3], style: s.1) }
    }

    static func patterns() -> [Case] {
        let clear = c(0), purple = c(0x6a1b9aff)
        let rows: [(String, [Double], [BatchValue], [Double], [String])] = [
            ("dot1", [1, 1, 1, 1], [R, R, R, R], [0, 0, 0, 0], Array(repeating: "dotted", count: 4)),
            ("dot2", [2, 2, 2, 2], [R, R, R, R], [0, 0, 0, 0], Array(repeating: "dotted", count: 4)),
            ("dot4", [4, 4, 4, 4], [R, R, R, R], [0, 0, 0, 0], Array(repeating: "dotted", count: 4)),
            ("dash1", [1, 1, 1, 1], [G, G, G, G], [0, 0, 0, 0], Array(repeating: "dashed", count: 4)),
            ("dash2", [2, 2, 2, 2], [G, G, G, G], [0, 0, 0, 0], Array(repeating: "dashed", count: 4)),
            ("dash4", [4, 4, 4, 4], [G, G, G, G], [0, 0, 0, 0], Array(repeating: "dashed", count: 4)),
            ("round-dot", [4, 4, 4, 4], [B, B, B, B], [20, 20, 20, 20], Array(repeating: "dotted", count: 4)),
            ("round-dash", [4, 4, 4, 4], [B, B, B, B], [20, 20, 20, 20], Array(repeating: "dashed", count: 4)),
            ("mixed", [4, 4, 4, 4], [R, G, B, Y], [16, 16, 16, 16], ["dotted", "solid", "dashed", "solid"]),
            ("unequal-dot", [1, 2, 4, 2], [purple, purple, purple, purple], [12, 12, 12, 12], Array(repeating: "dotted", count: 4)),
            ("underline-dot", [0, 0, 1, 0], [clear, clear, R, clear], [0, 0, 0, 0], ["none", "none", "dotted", "none"]),
            ("underline-dash", [0, 0, 4, 0], [clear, clear, G, clear], [0, 0, 0, 0], ["none", "none", "dashed", "none"]),
        ]
        return rows.enumerated().map { i, row in
            var style = box(row.1, row.2, radii: row.3, background: false)
            for (side, kind) in zip(["top", "right", "bottom", "left"], row.4) { style["border_style_" + side] = .string(kind) }
            return Case(name: row.0, x: [20, 145, 270][i % 3], y: [20, 115, 210, 305][i / 3], style: style)
        }
    }

    /// An image's straight sRGB bytes.
    static func pixels(_ image: CGImage) -> (Int, Int, [UInt8]) {
        let (w, h) = (image.width, image.height)
        var bytes = [UInt8](repeating: 0, count: w * h * 4)
        let ctx = CGContext(data: &bytes, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        return (w, h, bytes)
    }

    static func chrome(_ name: String) -> (Int, Int, [UInt8]) {
        let url = fixtures.appendingPathComponent(name)
        let source = CGImageSourceCreateWithURL(url as CFURL, nil)!
        return pixels(CGImageSourceCreateImageAtIndex(source, 0, nil)!)
    }

    /// Mean absolute channel difference (0–255) and the share of pixels
    /// where a channel differs by more than 48, over one 100×70 box. `ours`
    /// is the box alone, opaque, at 1×.
    static func compare(_ ours: [UInt8], _ reference: (Int, Int, [UInt8]), at x: Int, _ y: Int) -> (Double, Double) {
        var (sum, over) = (0, 0)
        for j in 0..<70 {
            for i in 0..<100 {
                let a = (j * 100 + i) * 4, b = ((y + j) * reference.0 + x + i) * 4
                var beyond = false
                for k in 0..<3 {
                    let d = abs(Int(ours[a + k]) - Int(reference.2[b + k]))
                    sum += d; beyond = beyond || d > 48
                }
                over += beyond ? 1 : 0
            }
        }
        return (Double(sum) / (3 * 7000), 100 * Double(over) / 7000)
    }

    /// Every case against one of Chrome's pictures; the failures, named.
    static func check(flipped: Bool, dark: Bool, band: (mean: Double, over: Double) = (2, 2), render: (NodeStyle, Bool, CGColor) -> [UInt8]) -> [String] {
        let reference = chrome(dark ? "borders.web-dark.png" : "borders.web.png")
        let page = dark ? CGColor(srgbRed: 0x12 / 255, green: 0x12 / 255, blue: 0x12 / 255, alpha: 1) : CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 1)
        var failures: [String] = []
        for c in cases(flipped: flipped) {
            let (mean, over) = compare(render(c.style, dark, page), reference, at: c.x, c.y)
            print("border parity \(dark ? "dark" : "light") \(c.name): mean \(String(format: "%.2f", mean))/255, \(String(format: "%.2f", over))% beyond 48")
            // Measured 2026-09-25 against Chrome; the band is that with room.
            if mean > band.mean || over > band.over { failures.append("\(c.name) \(dark ? "dark" : "light"): mean \(mean), \(over)% beyond 48") }
        }
        return failures
    }

    // CoreGraphics and Skia differ on the one-pixel dots and curved joins.
    // The 1× fixture's measured worst case stays under 5/255 mean and 5%.
    static func checkPatterns(band: (mean: Double, over: Double) = (5, 5), render: (NodeStyle, Bool, CGColor) -> [UInt8]) -> [String] {
        let reference = chrome("border-patterns.web.png")
        let page = CGColor(gray: 1, alpha: 1)
        return patterns().compactMap { c in
            let pixels = render(c.style, false, page)
            let (mean, over) = compare(pixels, reference, at: c.x, c.y)
            print("border pattern parity \(c.name): mean \(String(format: "%.2f", mean))/255, \(String(format: "%.2f", over))% beyond 48")
            if mean > band.mean || over > band.over { return "\(c.name): mean \(mean), \(over)% beyond 48" }
            if c.name == "underline-dot" && !underlineHasInkAndGaps(pixels) { return "\(c.name): missing ink or gaps" }
            if ["dot1", "dash1"].contains(c.name) && !thinBoxEdgesHaveInkAndGaps(pixels) { return "\(c.name): missing ink or gaps on an edge" }
            if c.name == "dot1" && !thinDotPhaseMatchesChrome(pixels, reference: reference, at: c.x, c.y) { return "\(c.name): wrong dot phase" }
            return nil
        }
    }

    static func underlineHasInkAndGaps(_ pixels: [UInt8]) -> Bool {
        let greens = (10..<90).map { pixels[(69 * 100 + $0) * 4 + 1] }
        return greens.filter { $0 < 160 }.count >= 20 && greens.filter { $0 > 245 }.count >= 20
    }

    static func thinBoxEdges() -> [[(Int, Int)]] {
        [(10..<90).map { ($0, 0) }, (10..<60).map { (99, $0) },
         (10..<90).map { ($0, 69) }, (10..<60).map { (0, $0) }]
    }

    static func thinBoxEdgesHaveInkAndGaps(_ pixels: [UInt8]) -> Bool {
        thinBoxEdges().allSatisfy { edge in
            let lows = edge.map { point -> UInt8 in
                let i = (point.1 * 100 + point.0) * 4
                return min(pixels[i], pixels[i + 1], pixels[i + 2])
            }
            return lows.filter { $0 < 160 }.count >= 10 && lows.filter { $0 > 245 }.count >= 10
        }
    }

    static func thinDotPhaseMatchesChrome(_ pixels: [UInt8], reference: (Int, Int, [UInt8]), at x: Int, _ y: Int) -> Bool {
        thinBoxEdges().allSatisfy { edge in
            edge.filter { point in
                (pixels[(point.1 * 100 + point.0) * 4 + 1] < 160) !=
                    (reference.2[((y + point.1) * reference.0 + x + point.0) * 4 + 1] < 160)
            }.count <= 4
        }
    }

    /// A 1× opaque bitmap context over the page's colour, y down.
    static func canvas(_ page: CGColor) -> CGContext {
        let ctx = CGContext(data: nil, width: 100, height: 70, bitsPerComponent: 8, bytesPerRow: 400,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.setFillColor(page); ctx.fill(CGRect(x: 0, y: 0, width: 100, height: 70))
        ctx.translateBy(x: 0, y: 70); ctx.scaleBy(x: 1, y: -1)
        return ctx
    }
    static func bytes(_ ctx: CGContext) -> [UInt8] { pixels(ctx.makeImage()!).2 }
}

private extension Array where Element == Double {
    var batch: BatchValue { .array(map { .number($0) }) }
}
