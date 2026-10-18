// @ref LLP 1126.001 WP8 — keep only a state this generation's plan can decode.
import Foundation
import CommonCrypto

public struct ActivityStateCache {
    private let file: URL

    public init(root: URL, id: String) {
        let key = Data(id.utf8)
        var hash = [UInt8](repeating: 0, count: Int(CC_SHA256_DIGEST_LENGTH))
        _ = key.withUnsafeBytes { CC_SHA256($0.baseAddress, CC_LONG(key.count), &hash) }
        let name = hash.map { String(format: "%02x", $0) }.joined()
        file = root.appendingPathComponent("Library/Caches/activity-states/\(name).json")
    }

    /// A nil state or a failed decode tries the per-id last good state. Without
    /// one, the driver shows its placeholder. A failed cache write is returned
    /// to the caller; `outdated` is true only when a kept state is shown. No app data source runs here.
    public func resolve(_ state: Data?, accepts: (Data) -> Bool) throws -> (state: Data?, outdated: Bool) {
        if let state, accepts(state) {
            try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
            try state.write(to: file, options: .atomic)
            return (state, false)
        }
        if let kept = try? Data(contentsOf: file), accepts(kept) { return (kept, true) }
        return (nil, false)
    }
}
