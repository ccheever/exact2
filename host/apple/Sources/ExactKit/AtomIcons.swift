// An inline atom's icon from an image source (LLP 1045.000 D2): an asset, an
// `app:/` file, a `data:` URL or an http(s) URL, read as an `image` reads it
// (`RasterInput.open`) and decoded at the pill's size off the main thread, an
// SVG as the image host draws one (`exact_svg_image_scene`, `SvgIsland`).
// The pill's picture asks for it as it draws; until it is here the pill keeps
// the icon's room empty, and its field is restyled when it comes, which draws
// the pill again. One that cannot be read is logged with the step that
// failed, and its pill then keeps no room. A picture does not follow the
// appearance, as a browser's `<img>` does not: an app gives a face another
// source for another scheme.
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
    static let queue = DispatchQueue(label: "exact.atom-icons", qos: .userInitiated, attributes: .concurrent)

    /// `source`'s picture at most `pixels` on its longest side for a pill
    /// in `node`'s field, or nil while it is read (the field is restyled
    /// once it is) or when it cannot be.
    package static func image(_ source: String, pixels: Int, node: NodeView?) -> CGImage? {
        let session = node?.presenter?.session
        return image(source, pixels: pixels, resolver: session?.app.resolver, log: { [weak session] in session?.log($0) }) { [weak node] in node?.restyleMarkup() }
    }

    /// `source`'s picture, or nil while it is read or when it cannot be;
    /// `ready` is called on main once it is read, and `log` with the reason
    /// when it cannot be.
    static func image(_ source: String, pixels: Int, resolver: AssetResolver?, log: @escaping (String) -> Void = { _ in },
                      ready: @escaping () -> Void) -> CGImage? {
        let key = "\(pixels) \(source)"
        lock.lock()
        if let known = decoded[key] { lock.unlock(); return known }
        if waiting[key] != nil { waiting[key]?.append(ready); lock.unlock(); return nil }
        // A `data:` URL is its own bytes; any other source is the app's to
        // resolve, asked again once its field is in a session.
        guard resolver != nil || source.hasPrefix("data:") else { lock.unlock(); return nil }
        waiting[key] = [ready]
        lock.unlock()
        queue.async { read(key, source, pixels: max(1, pixels), resolver: resolver, log: log, attempt: 0) }
        return nil
    }

    /// Read `source` for `key` and tell its fields. A decode or a drawing
    /// that gives nothing from bytes it read is asked again after each of
    /// the image loader's delays (`RasterLoader.declineDelays`), as a busy
    /// system can decline one; a file that is not there or a source that
    /// does not parse is not.
    private static func read(_ key: String, _ source: String, pixels: Int, resolver: AssetResolver?, log: @escaping (String) -> Void, attempt: Int) {
        var image: CGImage?, failure: Failure?
        do { image = try decode(source, pixels: pixels, resolver: resolver) } catch {
            failure = error as? Failure ?? .read(error)
        }
        let named = source.hasPrefix("data:") ? "a data: URL" : RasterRetry.withoutQuery(source)
        if let failure, failure.declined, attempt < RasterLoader.declineDelays.count {
            let delay = RasterLoader.declineDelays[attempt]
            DispatchQueue.main.async { log("atom icon queued: \(named): \(failure); again in \(Int(delay * 1000)) ms") }
            queue.asyncAfter(deadline: .now() + delay) { read(key, source, pixels: pixels, resolver: resolver, log: log, attempt: attempt + 1) }
            return
        }
        lock.lock()
        decoded[key] = .some(image)
        let readies = waiting.removeValue(forKey: key) ?? []
        lock.unlock()
        DispatchQueue.main.async {
            if let failure { log("atom icon refused: \(named): \(failure) (LLP 1045.000 D2)") }
            readies.forEach { $0() }
        }
    }

    /// Whether `source` at `pixels` was read and could not be: its pill
    /// keeps no room for it.
    package static func failed(_ source: String, pixels: Int) -> Bool {
        lock.lock(); defer { lock.unlock() }
        if case .some(.none) = decoded["\(pixels) \(source)"] { return true }
        return false
    }

    /// Forget every picture (a test's).
    package static func reset() { lock.lock(); decoded = [:]; lock.unlock() }

    /// Why a picture cannot be drawn: the step that failed.
    enum Failure: Error, CustomStringConvertible {
        case data, read(Error), decode, svg(String), drawn
        /// Bytes it read gave no picture: a busy system can decline a decode.
        var declined: Bool {
            switch self {
            case .decode, .drawn: return true
            default: return false
            }
        }
        var description: String {
            switch self {
            case .data: return "a data: URL over \(RasterInput.dataLimit) bytes or not one"
            case .read(let error): return "read: \(error)"
            case .decode: return "ImageIO decoded no picture"
            case .svg(let why): return "SVG: \(why)"
            case .drawn: return "its SVG scene drew no picture"
            }
        }
    }

    private static func decode(_ source: String, pixels: Int, resolver: AssetResolver?) throws -> CGImage {
        let bytes: Data
        if source.hasPrefix("data:") {
            guard source.utf8.count <= RasterInput.dataLimit, let data = RasterInput.dataURL(source) else { throw Failure.data }
            bytes = data
        } else {
            do { bytes = try RasterInput.open(source, resolver: resolver!).bytes() } catch { throw Failure.read(error) }
        }
        if try RasterMetadata.read(prefix: bytes.prefix(RasterMetadata.headerLimit), encodedBytes: bytes.count).svg {
            return try svg(bytes, pixels: pixels)
        }
        guard let image = CGImageSourceCreateWithData(bytes as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              let thumbnail = CGImageSourceCreateThumbnailAtIndex(image, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true,
                                                                             kCGImageSourceCreateThumbnailWithTransform: true,
                                                                             kCGImageSourceThumbnailMaxPixelSize: pixels] as CFDictionary)
        else { throw Failure.decode }
        return thumbnail
    }

    /// An SVG drawn into a square of `pixels`, its view box fitted as an
    /// `image` fits it at that size.
    private static func svg(_ bytes: Data, pixels: Int) throws -> CGImage {
        var length = 0, flags: UInt32 = 0
        let side = Double(pixels)
        let buffer = bytes.withUnsafeBytes { raw in
            exact_svg_image_scene(raw.baseAddress?.assumingMemoryBound(to: UInt8.self), bytes.count, side, side, &length, &flags)
        }
        guard let buffer else { throw Failure.svg("no scene") }
        defer { exact_svg_image_free(buffer, length) }
        let data = Data(bytes: buffer, count: length)
        guard flags & 2 == 0 else { throw Failure.svg(String(decoding: data, as: UTF8.self)) }
        guard let spec = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw Failure.svg("its scene is not JSON") }
        let numbers = (spec["t"] as? [Any])?.compactMap { ($0 as? NSNumber)?.doubleValue } ?? []
        guard numbers.count == 6 else { throw Failure.svg("its scene has no transform") }
        let transform = CGAffineTransform(a: numbers[0], b: numbers[1], c: numbers[2], d: numbers[3], tx: numbers[4], ty: numbers[5])
        let render = {
            SvgIsland.render(spec["els"] as? [Any] ?? [], rect: CGRect(x: 0, y: 0, width: side, height: side), w: pixels, h: pixels, k: 1,
                             transform: transform, flip: true, dark: false, fonts: nil)?.makeImage()
        }
        // Text in it is drawn where the session's fonts are.
        guard let image = flags & 1 != 0 ? DispatchQueue.main.sync(execute: render) : render() else { throw Failure.drawn }
        return image
    }
}
