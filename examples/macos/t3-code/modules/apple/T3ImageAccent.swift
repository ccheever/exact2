#if os(macOS)
import AppKit

/// An image chip's accent (contextChipParts.tsx ImageChipButton averageImageColor;
/// T3 Code, MIT, see LICENSE-T3), lane r4-timeline: the image drawn at 16×16 and
/// averaged with each pixel weighted by its alpha, so transparent pixels do not
/// darken it. `{op: "imageAccent", url, key}` → `{accent: "r g b"}`, "" when the
/// image cannot be read (the chip keeps the image kind's own tone).
enum T3ImageAccent {
    private static var cache: [String: String] = [:]
    private static let lock = NSLock()

    static func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0, key = request["key"] as? String ?? ""
        let answer = { (accent: String) in reply(["ok": true, "generation": generation, "value": ["accent": accent]]) }
        guard let string = request["url"] as? String, let url = URL(string: string), ["http", "https"].contains(url.scheme ?? "") else { return answer("") }
        lock.lock(); let hit = key.isEmpty ? nil : cache[key]; lock.unlock()
        if let hit { return answer(hit) }
        var fetch = URLRequest(url: url)
        fetch.timeoutInterval = 5
        URLSession.shared.dataTask(with: fetch) { data, response, _ in
            let fine = (response as? HTTPURLResponse).map { (200..<300).contains($0.statusCode) } ?? false
            let accent = fine ? data.flatMap(average) ?? "" : ""
            if !key.isEmpty {
                lock.lock(); cache[key] = accent; if cache.count > 256 { cache.removeAll() }; lock.unlock()
            }
            answer(accent)
        }.resume()
    }

    /// The alpha-weighted mean colour of `data` drawn into 16×16, as "r g b".
    static func average(_ data: Data) -> String? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil), let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { return nil }
        let side = 16
        var pixels = [UInt8](repeating: 0, count: side * side * 4)
        let drawn: Bool = pixels.withUnsafeMutableBytes { buffer in
            guard let context = CGContext(data: buffer.baseAddress, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                          space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
            context.interpolationQuality = .medium
            context.draw(image, in: CGRect(x: 0, y: 0, width: side, height: side))
            return true
        }
        guard drawn else { return nil }
        // Premultiplied channels already carry r·a/255: their sum over Σa is Σ(r·a)/Σa.
        var red = 0.0, green = 0.0, blue = 0.0, alpha = 0.0
        for index in stride(from: 0, to: pixels.count, by: 4) {
            red += Double(pixels[index]); green += Double(pixels[index + 1]); blue += Double(pixels[index + 2]); alpha += Double(pixels[index + 3])
        }
        guard alpha > 0 else { return nil }
        let channel = { (sum: Double) in Int((sum * 255 / alpha).rounded()) }
        return "\(channel(red)) \(channel(green)) \(channel(blue))"
    }
}
#endif
