// swiftc modules/apple/T3MobileFaviconImage.swift tests/FaviconImageTests.swift -o /tmp/t3-favicon-image-tests
import Foundation
import ImageIO
import UniformTypeIdentifiers

@main struct FaviconImageTests {
    static func main() {
        var failures = [String](), checks = 0
        func check(_ condition: Bool, _ message: String) { checks += 1; if !condition { failures.append(message) } }
        func test(_ name: String, _ run: () throws -> Void) {
            do { try run() } catch { check(false, "\(name): \(error)") }
        }
        func rejects(_ failure: T3MobileFaviconImage.Failure, _ name: String, _ run: () throws -> String) {
            do { _ = try run(); check(false, name + " accepted") }
            catch { check(error as? T3MobileFaviconImage.Failure == failure, name + " returned \(error)") }
        }
        func encode(_ data: Data, _ mime: String? = "image/png", _ url: String = "https://example.test/icon") throws -> String {
            try T3MobileFaviconImage.dataURL(bytes: data, contentType: mime, url: url)
        }
        for (header, url, expected) in [
            (" Image/PNG ; charset=utf-8", "https://example.test/a.gif", "image/png"),
            ("image/tiff", "https://example.test/a.png", "image/tiff"),
            ("application/octet-stream", "https://example.test/a%2EPNG?token=.gif#x", "image/png"),
            ("text/plain", "//example.test/icons/a.JpEg", "image/jpeg"),
            ("", " <https://example.test/a.svg> ", "image/svg+xml"),
            ("", "file:///tmp/a.webp", "image/webp"),
            ("", "C:\\icons\\a.gif", "image/gif"),
            ("", "relative/a%.ico?key=1", "image/x-icon"),
            ("", "data:IMAGE/AVIF;base64,AA==", "image/avif"),
            ("", "https://example.test/a.m4v", "video/mp4"),
            ("", "a.mkv", "video/x-matroska"),
            ("", "a.mov", "video/quicktime"),
            ("", "a.webm", "video/webm"),
            ("", "a.ogv", "video/ogg"),
            ("", "a.avi", "video/x-msvideo"),
            ("", "https://example.test/a.avif", "image/avif"),
            ("", "https:a.png", "image/png"),
            ("", "https:/a.png", "image/png"),
            ("", "http:example.test/a.svg", "image/svg+xml"),
            ("", "https://example.test/a.p\nng", "image/png"),
            ("", "https://example.test/a%ZZ.png", "image/png"),
            ("", "https://example.test/%E0%A4%A.png", "image/png"),
        ] {
            check(T3MobileFaviconImage.mimeType(contentType: header, url: url) == expected, "MIME \(header) / \(url)")
        }
        for url in ["https://example.test/a?file=.png", "https://example.test/.png/no-extension", "a.png/", "https://[bad/icon.png", "a.png%00", "a.pngx"] {
            check(T3MobileFaviconImage.mimeType(contentType: nil, url: url) == nil, "unknown MIME \(url)")
        }
        let raw = Data([0, 255, 16, 37, 128])
        for mime in ["image/png", "image/jpeg", "image/gif", "image/webp", "image/avif", "image/svg+xml", "image/x-icon", "image/vnd.microsoft.icon"] {
            test("raw \(mime)") {
                check(try encode(raw, mime) == "data:\(mime);base64,AP8QJYA=", "raw \(mime) preserves served bytes without decoding")
            }
        }
        test("raw SVG") {
            let svg = Data("<svg xmlns='http://www.w3.org/2000/svg'><rect fill='red' width='1' height='1'/></svg>".utf8)
            check(try encode(svg, nil, "https://example.test/a.svg") == "data:image/svg+xml;base64," + svg.base64EncodedString(), "small SVG preserves XML")
        }
        let maxPNG = (T3MobileFaviconImage.maxDataURLLength - "data:image/png;base64,".count) / 4 * 3
        test("inline boundary") {
            let value = try encode(Data(repeating: 42, count: maxPNG))
            check(value.count <= 32768 && value.count > 32764, "largest inline PNG encoded boundary")
        }
        rejects(.decode, "above inline limit invalid bitmap") { try encode(Data(repeating: 42, count: maxPNG + 1)) }
        rejects(.cacheLimit, "oversized SVG") { try encode(Data(repeating: 32, count: 25_000), "image/svg+xml") }
        rejects(.cacheLimit, "empty SVG") { try encode(Data(), "image/svg+xml") }
        rejects(.decode, "empty bitmap") { try encode(Data()) }
        rejects(.decode, "unknown image type does not inline") { try encode(raw, "image/bmp") }
        rejects(.noType, "no MIME or extension") { try encode(raw, nil) }
        rejects(.tooLarge, "source bound") { try encode(Data(repeating: 0, count: 4 * 1024 * 1024 + 1)) }
        rejects(.decode, "exact source boundary reaches decode") { try encode(Data(repeating: 0, count: 4 * 1024 * 1024)) }
        rejects(.cancelled, "cancel before raw processing") {
            try T3MobileFaviconImage.dataURL(bytes: raw, contentType: "image/png", url: "", cancelled: { true })
        }
        var inlineChecks = 0
        rejects(.cancelled, "cancel before raw return") {
            try T3MobileFaviconImage.dataURL(bytes: raw, contentType: "image/png", url: "", cancelled: { inlineChecks += 1; return inlineChecks > 1 })
        }
        test("opaque thumbnail") {
            let source = try fixture(width: 384, height: 192, alpha: false)
            check(source.count > maxPNG, "opaque fixture forces downscale")
            let value = try encode(source)
            let image = try decode(value)
            check(value.hasPrefix("data:image/jpeg;base64,/9j/"), "opaque thumbnail is JPEG")
            check(image.width == 96 && image.height == 48, "opaque thumbnail preserves aspect at96")
            check(value.utf8.count <= 32768, "opaque thumbnail respects inline size")
            check(![.first, .last, .premultipliedFirst, .premultipliedLast].contains(image.alphaInfo), "opaque thumbnail has no alpha")
            let mislabeled = try encode(source, "image/tiff")
            check(mislabeled.hasPrefix("data:image/jpeg;base64,"), "unaccepted inline MIME still decodes actual bitmap")
        }
        test("alpha thumbnail") {
            let source = try fixture(width: 384, height: 192, alpha: true)
            check(source.count > maxPNG, "alpha fixture forces downscale")
            let value = try encode(source), image = try decode(value)
            check(value.hasPrefix("data:image/png;base64,iVBORw0KGgo"), "alpha thumbnail is PNG")
            check(image.width == 96 && image.height == 48, "alpha thumbnail preserves aspect at96")
            check(try hasTransparentPixels(image), "alpha thumbnail retains actual transparent pixels")
            check(value.utf8.count <= 32768, "alpha thumbnail respects inline size")
        }
        test("96 to48 fallback") {
            let source = try fixture(width: 96, height: 96, alpha: true)
            check(source.count > maxPNG, "random96 RGBA requires fallback")
            let value = try encode(source), image = try decode(value)
            check(image.width == 48 && image.height == 48, "large96 PNG retries at48")
            check(try hasTransparentPixels(image), "48 fallback retains alpha")
            check(value.utf8.count <= 32768, "48 fallback respects complete URL limit")
        }
        test("cancel after decode and encoding") {
            let source = try fixture(width: 384, height: 192, alpha: true)
            for cancelAt in [2, 3, 4] {
                var calls = 0
                rejects(.cancelled, "cancel processing checkpoint \(cancelAt)") {
                    try T3MobileFaviconImage.dataURL(bytes: source, contentType: "image/png", url: "", cancelled: { calls += 1; return calls >= cancelAt })
                }
            }
        }
        print("Favicon image: \(checks - failures.count)/\(checks) checks passed")
        for failure in failures { print("FAIL: \(failure)") }
        if !failures.isEmpty { exit(1) }
    }

    enum FixtureError: Error { case image, encode, decode }

    /// Fixed PRNG, actual pixel image and production ImageIO encoding; no external fixture files.
    static func fixture(width: Int, height: Int, alpha: Bool) throws -> Data {
        var seed: UInt32 = 0xA137F03D
        func byte() -> UInt8 { seed = seed &* 1_664_525 &+ 1_013_904_223; return UInt8(truncatingIfNeeded: seed >> 24) }
        var pixels = [UInt8](); pixels.reserveCapacity(width * height * 4)
        for _ in 0..<(width * height) {
            let a: UInt8 = alpha ? max(1, byte()) : 255
            pixels.append(UInt8(UInt16(byte()) * UInt16(a) / 255))
            pixels.append(UInt8(UInt16(byte()) * UInt16(a) / 255))
            pixels.append(UInt8(UInt16(byte()) * UInt16(a) / 255))
            pixels.append(a)
        }
        let info = alpha ? CGImageAlphaInfo.premultipliedLast : .noneSkipLast
        guard let provider = CGDataProvider(data: Data(pixels) as CFData),
              let image = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
                  bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: info.rawValue),
                  provider: provider, decode: nil, shouldInterpolate: true, intent: .defaultIntent) else { throw FixtureError.image }
        let output = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(output, UTType.png.identifier as CFString, 1, nil) else { throw FixtureError.encode }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else { throw FixtureError.encode }
        return output as Data
    }

    static func decode(_ url: String) throws -> CGImage {
        guard let comma = url.firstIndex(of: ","), let bytes = Data(base64Encoded: String(url[url.index(after: comma)...])),
              let source = CGImageSourceCreateWithData(bytes as CFData, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { throw FixtureError.decode }
        return image
    }

    static func hasTransparentPixels(_ image: CGImage) throws -> Bool {
        let count = image.width * image.height * 4
        var pixels = [UInt8](repeating: 0, count: count)
        return try pixels.withUnsafeMutableBytes { memory in
            guard let context = CGContext(data: memory.baseAddress, width: image.width, height: image.height,
                bitsPerComponent: 8, bytesPerRow: image.width * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { throw FixtureError.image }
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
            return stride(from: 3, to: count, by: 4).contains { memory[$0] < 255 }
        }
    }
}
