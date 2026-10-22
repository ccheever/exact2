// `storage.fs.compressImage`'s SVG input on Apple (LLP 1133.006 D1): ImageIO
// has no SVG decoder, so the storage worker asks this, the renderer an `img`
// of an SVG draws with (RasterImage.swift `decodeSVG`, LLP 1011.001), for a
// PNG of the file at its natural size, which the Rust side computed and
// bounded. Nothing is dark, as a picture follows no scheme. Text draws on
// the main thread, where the fonts are.
import CExact
import Foundation
import ImageIO

enum SvgRasterizer {
    /// Hand the renderer to the Rust side, once a process.
    static let installed: Void = { exact_set_svg_renderer(render) }()

    private static let render: @convention(c) (UnsafePointer<UInt8>?, Int, UInt32, UInt32, UnsafeMutablePointer<Int>?) -> UnsafeMutablePointer<UInt8>? = { bytes, len, w, h, outLen in
        guard let bytes, let outLen, let png = png(Data(bytes: bytes, count: len), width: Int(w), height: Int(h)) else { return nil }
        let out = malloc(max(png.count, 1))!.assumingMemoryBound(to: UInt8.self)
        png.copyBytes(to: out, count: png.count)
        outLen.pointee = png.count
        return out
    }

    /// The file drawn at `width` × `height` pixels (CSS pixels at scale 1), as a PNG.
    static func png(_ bytes: Data, width: Int, height: Int) -> Data? {
        var length = 0, flags: UInt32 = 0
        let buffer = bytes.withUnsafeBytes { raw in
            exact_svg_image_scene(raw.baseAddress?.assumingMemoryBound(to: UInt8.self), bytes.count, Double(width), Double(height), &length, &flags)
        }
        guard let buffer else { return nil }
        defer { exact_svg_image_free(buffer, length) }
        guard flags & 2 == 0, let spec = try? JSONSerialization.jsonObject(with: Data(bytes: buffer, count: length)) as? [String: Any] else { return nil }
        let els = spec["els"] as? [Any] ?? []
        let numbers = (spec["t"] as? [Any])?.compactMap { ($0 as? NSNumber)?.doubleValue } ?? []
        let draw = { () -> CGContext? in
            // No view box area: an empty picture, as the browser's.
            guard numbers.count == 6 else {
                return CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                                 space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
            }
            let transform = CGAffineTransform(a: numbers[0], b: numbers[1], c: numbers[2], d: numbers[3], tx: numbers[4], ty: numbers[5])
            return SvgIsland.render(els, rect: CGRect(x: 0, y: 0, width: width, height: height), w: width, h: height, k: 1,
                                    transform: transform, flip: true, dark: false, fonts: nil)
        }
        let context = flags & 1 != 0 && !Thread.isMainThread ? DispatchQueue.main.sync(execute: draw) : draw()
        guard let image = context?.makeImage() else { return nil }
        let out = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(out, "public.png" as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(destination, image, nil)
        return CGImageDestinationFinalize(destination) ? out as Data : nil
    }
}
