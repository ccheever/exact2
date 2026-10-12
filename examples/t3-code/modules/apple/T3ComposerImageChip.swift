#if os(macOS)
import AppKit
import ImageIO

/// An image chip in the prompt (ImageChipButton, contextChipParts.tsx): its
/// icon is the draft image's thumbnail (object-cover, rounded-sm) and its
/// accent the image's average colour over a 16×16 sample, weighted by alpha
/// (averageImageColor), mixed into ink, border and fill as ContextChip's
/// tokens do. Without the image the chip keeps the default image accent.
enum T3ComposerImageChip {
    struct Entry { let thumbnail: CGImage; let palette: [String] }
    private static var cache: [String: Entry] = [:]
    private static var missing: Set<String> = []

    /// The draft image `id` in `directory` (SnapShot's drafts/, where Attach files puts picked images too).
    static func entry(directory: URL?, id: String) -> Entry? {
        guard let directory, UUID(uuidString: id) != nil else { return nil }
        let path = directory.appendingPathComponent("\(id.lowercased()).png").path
        if let hit = cache[path] { return hit }
        if missing.contains(path) { return nil }
        guard let made = load(path) else { missing.insert(path); return nil }
        cache[path] = made
        return made
    }

    static func load(_ path: String) -> Entry? {
        guard let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true,
                  kCGImageSourceCreateThumbnailWithTransform: true, kCGImageSourceThumbnailMaxPixelSize: 256] as CFDictionary) else { return nil }
        let side = min(image.width, image.height)
        guard side > 0, let square = image.cropping(to: CGRect(x: (image.width - side) / 2, y: (image.height - side) / 2, width: side, height: side)) else { return nil }
        return Entry(thumbnail: square, palette: average(image).map { palette(accent: oklab(srgb: $0)) } ?? palette(accent: oklch(0.62, 0.16, 16)))
    }

    /// averageImageColor: the image drawn into 16×16, channels weighted by alpha; nil when fully transparent.
    static func average(_ image: CGImage) -> (Double, Double, Double)? {
        var pixels = [UInt8](repeating: 0, count: 16 * 16 * 4)
        let drawn = pixels.withUnsafeMutableBytes { buffer -> Bool in
            guard let context = CGContext(data: buffer.baseAddress, width: 16, height: 16, bitsPerComponent: 8, bytesPerRow: 64,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
            context.interpolationQuality = .medium
            context.draw(image, in: CGRect(x: 0, y: 0, width: 16, height: 16))
            return true
        }
        guard drawn else { return nil }
        // Premultiplied: a channel's value times its alpha is the stored value times 255.
        var red = 0.0, green = 0.0, blue = 0.0, alpha = 0.0
        for index in stride(from: 0, to: pixels.count, by: 4) {
            red += Double(pixels[index]) * 255; green += Double(pixels[index + 1]) * 255; blue += Double(pixels[index + 2]) * 255
            alpha += Double(pixels[index + 3])
        }
        guard alpha > 0 else { return nil }
        return ((red / alpha).rounded(), (green / alpha).rounded(), (blue / alpha).rounded())
    }

    // MARK: Colour (CSS color-mix in oklab)

    typealias Lab = (l: Double, a: Double, b: Double)
    static func oklch(_ l: Double, _ c: Double, _ h: Double) -> Lab { (l, c * cos(h * .pi / 180), c * sin(h * .pi / 180)) }
    static func oklab(srgb: (Double, Double, Double)) -> Lab {
        let lin = { (v: Double) -> Double in let x = v / 255; return x <= 0.04045 ? x / 12.92 : pow((x + 0.055) / 1.055, 2.4) }
        let r = lin(srgb.0), g = lin(srgb.1), b = lin(srgb.2)
        let l = cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
        let m = cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
        let s = cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
        return (0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s, 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
                0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s)
    }
    static func srgb(_ lab: Lab) -> (Int, Int, Int) {
        let l = pow(lab.l + 0.3963377774 * lab.a + 0.2158037573 * lab.b, 3), m = pow(lab.l - 0.1055613458 * lab.a - 0.0638541728 * lab.b, 3)
        let s = pow(lab.l - 0.0894841775 * lab.a - 1.2914855480 * lab.b, 3)
        let encode = { (x: Double) -> Int in
            let v = x <= 0.0031308 ? 12.92 * x : 1.055 * pow(x, 1 / 2.4) - 0.055
            return max(0, min(255, Int((v * 255).rounded())))
        }
        return (encode(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s), encode(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
                encode(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s))
    }
    /// color-mix(in oklab, accent p, other) with premultiplied alpha; returns `#rrggbb` or `#rrggbbaa`.
    static func mix(_ accent: Lab, _ p: Double, _ other: Lab, alpha: Double = 1) -> String {
        let total = p + (1 - p) * alpha
        let lab: Lab = ((accent.l * p + other.l * (1 - p) * alpha) / total, (accent.a * p + other.a * (1 - p) * alpha) / total,
                        (accent.b * p + other.b * (1 - p) * alpha) / total)
        return hex(srgb(lab), alpha: total)
    }
    static func hex(_ rgb: (Int, Int, Int), alpha: Double = 1) -> String {
        let base = String(format: "#%02x%02x%02x", rgb.0, rgb.1, rgb.2)
        return alpha >= 1 ? base : base + String(format: "%02x", Int((alpha * 255).rounded()))
    }

    /// ContextChip: [light ink, dark ink, fill, light border, dark border] — ink
    /// mixes 22% accent into --contrast-foreground, the border 34% into
    /// --contrast-border, the fill is the accent at 11%.
    static func palette(accent: Lab) -> [String] {
        let lightInk: Lab = (0.274, 0.00165715, -0.00576662), darkInk: Lab = (0.97, 0, 0), lightBorder = oklab(srgb: (228, 228, 231)), white = oklab(srgb: (255, 255, 255))
        return [mix(accent, 0.22, lightInk), mix(accent, 0.22, darkInk), hex(srgb(accent), alpha: 0.11),
                mix(accent, 0.34, lightBorder), mix(accent, 0.34, white, alpha: 0.06)]
    }
}
#endif
