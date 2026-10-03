// @ref LLP 1100 D1, D3 — a profile colour's space: a Core Graphics name
// (`cg:kCGColorSpaceDCIP3`) or an app's ICC file (`icc:assets/brand.icc`).
import CoreGraphics
import Foundation

enum ProfileSpaces {
    /// Where an app's ICC files are read: the session that last drew one.
    nonisolated(unsafe) static weak var resolver: AssetResolver?
    nonisolated(unsafe) private static var cache: [String: CGColorSpace] = [:]
    private static let lock = NSLock()

    /// The space a wire name says, or nil for a wide space's name or an ICC
    /// file Core Graphics refuses.
    static func space(_ name: String) -> CGColorSpace? {
        if let cg = name.strip("cg:") { return CGColorSpace(name: cg as CFString) }
        guard let src = name.strip("icc:") else { return nil }
        lock.lock(); defer { lock.unlock() }
        if let known = cache[src] { return known }
        guard let data = resolver?.bytes(src), let space = CGColorSpace(iccData: data as CFData) else {
            FileHandle.standardError.write(Data("exact: color-profile \(src): not an ICC profile Core Graphics reads\n".utf8))
            return nil
        }
        cache[src] = space
        return space
    }
}

private extension String {
    func strip(_ prefix: String) -> String? { hasPrefix(prefix) ? String(dropFirst(prefix.count)) : nil }
}
