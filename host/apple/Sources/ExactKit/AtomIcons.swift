// An inline atom's icon from an image source (LLP 1045.000 D2): an asset, an
// `app:/` file, a `data:` URL or an http(s) URL, read as an `image` reads it
// (`RasterInput.open`) and decoded at the pill's size off the main thread, an
// SVG as the image host draws one (`exact_svg_image_scene`, `SvgIsland`).
// The pill's picture asks for it as it draws; until it is here the pill draws
// no icon, and its field is restyled when it comes, which draws the pill
// again. A picture does not follow the appearance, as a browser's `<img>`
// does not: an app gives a face another source for another scheme.
import CExact
import CoreGraphics
import Foundation
import ImageIO

package enum AtomIcons {
    private static let lock = NSLock()
    /// Each source's picture by its decode's longest side; a failed one is
    /// remembered as none, so it is not read again.
    nonisolated(unsafe) private static var decoded: [String: CGImage?] = [:]
    nonisolated(unsafe) private static var waiting: [String: [() -> Void]] = [:]
    private static let queue = DispatchQueue(label: "exact.atom-icons", qos: .userInitiated, attributes: .concurrent)

    /// `source`'s picture at most `pixels` on its longest side for a pill
    /// in `node`'s field, or nil while it is read (the field is restyled
    /// once it is) or when it cannot be.
    package static func image(_ source: String, pixels: Int, node: NodeView?) -> CGImage? {
        image(source, pixels: pixels, resolver: node?.presenter?.session?.app.resolver) { [weak node] in node?.restyleMarkup() }
    }

    static func image(_ source: String, pixels: Int, resolver: AssetResolver?, ready: @escaping () -> Void) -> CGImage? {
        let key = "\(pixels) \(source)"
        lock.lock()
        if let known = decoded[key] { lock.unlock(); return known }
        if waiting[key] != nil { waiting[key]?.append(ready); lock.unlock(); return nil }
        // A `data:` URL is its own bytes; any other source is the app's to resolve.
        guard resolver != nil || source.hasPrefix("data:") else { lock.unlock(); return nil }
        waiting[key] = [ready]
        lock.unlock()
        queue.async {
            let image = try? decode(source, pixels: max(1, pixels), resolver: resolver)
            lock.lock()
            decoded[key] = .some(image)
            let readies = waiting.removeValue(forKey: key) ?? []
            lock.unlock()
            DispatchQueue.main.async { readies.forEach { $0() } }
        }
        return nil
    }

    /// Forget every picture (a test's).
    package static func reset() { lock.lock(); decoded = [:]; lock.unlock() }

    private static func decode(_ source: String, pixels: Int, resolver: AssetResolver?) throws -> CGImage? {
        let bytes: Data
        if source.hasPrefix("data:") {
            guard source.utf8.count <= RasterInput.dataLimit, let data = RasterInput.dataURL(source) else { return nil }
            bytes = data
        } else if let resolver {
            bytes = try RasterInput.open(source, resolver: resolver).bytes()
        } else { return nil }
        if try RasterMetadata.read(prefix: bytes.prefix(RasterMetadata.headerLimit), encodedBytes: bytes.count).svg {
            return svg(bytes, pixels: pixels)
        }
        guard let image = CGImageSourceCreateWithData(bytes as CFData, [kCGImageSourceShouldCache: false] as CFDictionary) else { return nil }
        return CGImageSourceCreateThumbnailAtIndex(image, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true,
                                                              kCGImageSourceCreateThumbnailWithTransform: true,
                                                              kCGImageSourceThumbnailMaxPixelSize: pixels] as CFDictionary)
    }

    /// An SVG drawn into a square of `pixels`, its view box fitted as an
    /// `image` fits it at that size.
    private static func svg(_ bytes: Data, pixels: Int) -> CGImage? {
        var length = 0, flags: UInt32 = 0
        let side = Double(pixels)
        let buffer = bytes.withUnsafeBytes { raw in
            exact_svg_image_scene(raw.baseAddress?.assumingMemoryBound(to: UInt8.self), bytes.count, side, side, &length, &flags)
        }
        guard let buffer else { return nil }
        defer { exact_svg_image_free(buffer, length) }
        let data = Data(bytes: buffer, count: length)
        guard flags & 2 == 0, let spec = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        let numbers = (spec["t"] as? [Any])?.compactMap { ($0 as? NSNumber)?.doubleValue } ?? []
        guard numbers.count == 6 else { return nil }
        let transform = CGAffineTransform(a: numbers[0], b: numbers[1], c: numbers[2], d: numbers[3], tx: numbers[4], ty: numbers[5])
        let render = {
            SvgIsland.render(spec["els"] as? [Any] ?? [], rect: CGRect(x: 0, y: 0, width: side, height: side), w: pixels, h: pixels, k: 1,
                             transform: transform, flip: true, dark: false, fonts: nil)?.makeImage()
        }
        // Text in it is drawn where the session's fonts are.
        return flags & 1 != 0 ? DispatchQueue.main.sync(execute: render) : render()
    }
}
