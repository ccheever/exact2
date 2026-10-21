// @ref LLP 1100 D3 — bind ICC paths to immutable bytes at the owning runtime's
// batch boundary. Views and text workers never consult another session's assets.
import CoreGraphics
import Foundation
import CommonCrypto

public enum ProfileSpaces {
    /// A decoded value owns its immutable profile beyond cache eviction.
    public final class Handle: Equatable {
        public let key: String
        public let space: CGColorSpace
        init(key: String, space: CGColorSpace) { self.key = key; self.space = space }
        public static func == (a: Handle, b: Handle) -> Bool { a.key == b.key }
    }
    nonisolated(unsafe) private static var cache: [String: Handle] = [:]
    nonisolated(unsafe) private static var order: [String] = []
    private static let lock = NSLock()
    private static let capacity = 64

    package static var cacheCount: Int { lock.lock(); defer { lock.unlock() }; return cache.count }

    package static func bind(_ name: String, resolver: AssetResolver?) -> Handle? {
        guard name.hasPrefix("icc:"), let data = resolver?.bytes(String(name.dropFirst(4))) else { return nil }
        var hash = [UInt8](repeating: 0, count: Int(CC_SHA256_DIGEST_LENGTH))
        _ = data.withUnsafeBytes { CC_SHA256($0.baseAddress, CC_LONG(data.count), &hash) }
        let key = "icc-sha256:" + hash.map { String(format: "%02x", $0) }.joined()
        lock.lock(); defer { lock.unlock() }
        if let found = cache[key] { return found }
        guard let space = CGColorSpace(iccData: data as CFData) else { return nil }
        let handle = Handle(key: key, space: space)
        if order.count == capacity { cache.removeValue(forKey: order.removeFirst()) }
        cache[key] = handle; order.append(key)
        return handle
    }

    package static func space(_ name: String) -> CGColorSpace? {
        guard name.hasPrefix("cg:") else { return nil }
        return CGColorSpace(name: String(name.dropFirst(3)) as CFString)
    }

    package static func intent(_ name: String?) -> CGColorRenderingIntent {
        switch name {
        case "perceptual": return .perceptual
        case "absolute-colorimetric": return .absoluteColorimetric
        case "saturation": return .saturation
        default: return .relativeColorimetric
        }
    }
}
