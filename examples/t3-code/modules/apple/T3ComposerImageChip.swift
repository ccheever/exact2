#if os(macOS)
import AppKit
import ImageIO

/// OS image facts only. The shared field chooses all chip colours and styling.
enum T3ComposerImageChip {
    static func average(at url: URL) -> [Int]? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true,
                  kCGImageSourceCreateThumbnailWithTransform: true, kCGImageSourceThumbnailMaxPixelSize: 256] as CFDictionary),
              let value = average(image) else { return nil }
        return [Int(value.0), Int(value.1), Int(value.2)]
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

}
#endif
