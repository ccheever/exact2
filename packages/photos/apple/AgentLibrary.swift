// The agent's photo library (LLP 1133.019 §10): what @exact/photos reads and
// adds to under the agent, where it asks PhotoKit for nothing. Its photos
// are this checkout's `fixture/agent-1.jpg` to `agent-6.jpg` (a drive runs
// on this Mac or its simulators), with fixed creation dates, agent-1 the
// newest; agent-3 is stored turned (EXIF orientation 6) and agent-5 is in
// Display P3. An add joins it, its file kept under the session's
// `app:/tmp/photos-library/`.
// What the app may see of it is the access the runner's status gives
// (`full`, `limited`, `none`) and, under `limited`, the selection (the
// agent's `selection`, else `agent-1` to `agent-3`) with the app's own adds.
import CryptoKit
import Foundation
import ImageIO
import UniformTypeIdentifiers

final class AgentLibrary {
    struct Asset {
        let id: String
        let file: URL
        /// Its size as Photos shows it, orientation applied.
        let width: Int, height: Int
        /// Ms since the epoch.
        let created: Double
        /// Added by the app in this session.
        let own: Bool
        let type: String
    }

    /// The fixtures, beside this file's directory in the checkout.
    private static let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("fixture")
    static let defaultSelection = ["agent-1", "agent-2", "agent-3"]

    private(set) var assets: [Asset] = []
    var access = "none"
    var selection = AgentLibrary.defaultSelection

    init() {
        // 2026-0(7-n)-1n 10:(7n):00 UTC, as the fixtures' EXIF dates say.
        for n in 1...6 {
            let file = Self.fixtures.appendingPathComponent("agent-\(n).jpg")
            var parts = DateComponents()
            parts.year = 2026; parts.month = 7 - n; parts.day = 10 + n; parts.hour = 10; parts.minute = 7 * n
            parts.timeZone = TimeZone(identifier: "UTC")
            let created = (Calendar(identifier: .gregorian).date(from: parts)?.timeIntervalSince1970 ?? 0) * 1000
            if let (w, h, type) = Self.size(file) {
                assets.append(Asset(id: "agent-\(n)", file: file, width: w, height: h, created: created, own: false, type: type))
            }
        }
    }

    /// The ids the app may read now.
    var visible: Set<String> {
        switch access {
        case "full": return Set(assets.map(\.id))
        case "limited": return Set(selection).union(assets.filter(\.own).map(\.id))
        default: return []
        }
    }

    func asset(_ id: String) -> Asset? { visible.contains(id) ? assets.first { $0.id == id } : nil }

    /// The newest `limit` the app may read.
    func newest(_ limit: Int) -> [Asset] {
        let seen = visible
        return Array(assets.filter { seen.contains($0.id) }.sorted { $0.created > $1.created }.prefix(limit))
    }

    /// An add: the file, kept in `directory`, joins the library at the
    /// session's `now`. Nil for bytes that are no image.
    func add(_ data: Data, directory: URL, now: Double) -> Asset? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil), let uti = CGImageSourceGetType(source),
              let type = UTType(uti as String), type.conforms(to: .image), CGImageSourceGetCount(source) > 0 else { return nil }
        let id = "agent-\(assets.count + 1)"
        let file = directory.appendingPathComponent("\(id).\(type.preferredFilenameExtension ?? "img")")
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try data.write(to: file)
        } catch { return nil }
        guard let (w, h, mime) = Self.size(file) else { return nil }
        // The session's clock counts from 0 under the agent, so an add is dated
        // a day after the newest fixture, plus that clock: newer than all of them,
        // as a photo saved today is.
        let newest = assets.filter { !$0.own }.map(\.created).max() ?? 0
        let asset = Asset(id: id, file: file, width: w, height: h, created: newest + 86_400_000 + now, own: true, type: mime)
        assets.append(asset)
        return asset
    }

    /// `asset` drawn upright for a box of `width` by `height` device pixels
    /// at `fit`: no larger than it is.
    func picture(_ asset: Asset, width: Int, height: Int, fit: String) -> CGImage? {
        guard let source = CGImageSourceCreateWithURL(asset.file as CFURL, nil) else { return nil }
        let w = Double(asset.width), h = Double(asset.height)
        let x = Double(width) / w, y = Double(height) / h
        let factor = fit == "contain" || fit == "scale-down" ? min(x, y) : max(x, y)
        let longest = min(max(w, h), (max(w, h) * factor).rounded(.up))
        let options: [CFString: Any] = [kCGImageSourceCreateThumbnailFromImageAlways: true,
                                        kCGImageSourceCreateThumbnailWithTransform: true,
                                        kCGImageSourceThumbnailMaxPixelSize: max(1, Int(longest))]
        return CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
    }

    /// A file's upright size and media type.
    static func size(_ file: URL) -> (Int, Int, String)? {
        guard let source = CGImageSourceCreateWithURL(file as CFURL, nil),
              let p = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              var w = p[kCGImagePropertyPixelWidth] as? Int, var h = p[kCGImagePropertyPixelHeight] as? Int else { return nil }
        if let o = p[kCGImagePropertyOrientation] as? Int, o >= 5 { swap(&w, &h) }
        let type = (CGImageSourceGetType(source)).flatMap { UTType($0 as String)?.preferredMIMEType } ?? "image/jpeg"
        return (w, h, type)
    }

    static func sha256(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
}
