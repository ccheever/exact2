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
