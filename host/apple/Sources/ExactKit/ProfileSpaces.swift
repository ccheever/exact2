// @ref LLP 1100 D3 — bind ICC paths to immutable bytes at the owning runtime's
// batch boundary. Views and text workers never consult another session's assets.
import CoreGraphics
import Foundation
import CryptoKit

enum ProfileSpaces {
    nonisolated(unsafe) private static var cache: [String: CGColorSpace] = [:]
    private static let lock = NSLock()

    static func bind(_ name: String, resolver: AssetResolver?) -> String {
        guard name.hasPrefix("icc:"), let data = resolver?.bytes(String(name.dropFirst(4))) else { return name }
        let key = "icc-sha256:" + SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        lock.lock(); defer { lock.unlock() }
        if cache[key] == nil { cache[key] = CGColorSpace(iccData: data as CFData) }
        return key
    }

    static func space(_ name: String) -> CGColorSpace? {
        if name.hasPrefix("cg:") { return CGColorSpace(name: String(name.dropFirst(3)) as CFString) }
        lock.lock(); defer { lock.unlock() }
        return cache[name]
    }

    static func intent(_ name: String?) -> CGColorRenderingIntent {
        switch name {
        case "perceptual": return .perceptual
        case "absolute-colorimetric": return .absoluteColorimetric
        case "saturation": return .saturation
        default: return .relativeColorimetric
        }
    }
}
