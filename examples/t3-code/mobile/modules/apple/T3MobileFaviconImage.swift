import Foundation
import ImageIO
import UniformTypeIdentifiers

/// Pinned favicon image policy, using the already bounded response instead of a second download.
/// Small served bytes are preserved, including SVG/animation; only oversized bitmaps are decoded.
/// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
enum T3MobileFaviconImage {
    static let maxSourceBytes = 4 * 1024 * 1024
    static let maxDataURLLength = 32 * 1024

    enum Failure: String, LocalizedError {
        case cancelled = "Project icon request was cancelled."
        case tooLarge = "Project icon is too large to decode."
        case noType = "Project icon has no image type."
        case cacheLimit = "Project icon exceeds the cache limit."
        case decode = "Project icon could not be decoded."
        case encode = "Project icon thumbnail could not be encoded."
        var errorDescription: String? { rawValue }
    }

    private static let inlineTypes: Set<String> = [
        "image/png", "image/jpeg", "image/gif", "image/webp", "image/avif", "image/svg+xml",
        "image/x-icon", "image/vnd.microsoft.icon",
    ]
    private static let mediaTypes: [String: String] = [
        "avif": "image/avif", "gif": "image/gif", "ico": "image/x-icon", "jpeg": "image/jpeg",
        "jpg": "image/jpeg", "png": "image/png", "svg": "image/svg+xml", "webp": "image/webp",
        "avi": "video/x-msvideo", "m4v": "video/mp4", "mkv": "video/x-matroska",
        "mov": "video/quicktime", "mp4": "video/mp4", "ogv": "video/ogg", "webm": "video/webm",
    ]

    static func mimeType(contentType: String?, url: String) -> String? {
        let content = contentType?.components(separatedBy: ";").first?
            .trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if let content, content.hasPrefix("image/") { return content }
        var source = url.trimmingCharacters(in: .whitespacesAndNewlines)
        if source.hasPrefix("<"), source.hasSuffix(">") { source = String(source.dropFirst().dropLast()) }
        if let match = source.range(of: "^data:((?:image|video)/[A-Za-z0-9_.+-]+)[;,]", options: [.regularExpression, .caseInsensitive]) {
            return String(source[match].dropFirst(5).dropLast()).lowercased()
        }
        var path = source.components(separatedBy: CharacterSet(charactersIn: "?#")).first ?? ""
        if source.range(of: "^(?:https?:|file:|//)", options: [.regularExpression, .caseInsensitive]) != nil {
            // WHATWG special URLs treat backslashes as path separators before URL parsing.
            var special = source.replacingOccurrences(of: "\\", with: "/")
                .components(separatedBy: CharacterSet(charactersIn: "\t\n\r")).joined()
            if let colon = special.firstIndex(of: ":"), ["http", "https"].contains(special[..<colon].lowercased()) {
                let scheme = special[..<colon].lowercased(), rest = String(special[special.index(after: colon)...])
                if scheme == "https", !rest.hasPrefix("//") {
                    special = "https://media.invalid" + (rest.hasPrefix("/") ? rest : "/" + rest)
                } else {
                    special = scheme + "://" + rest.drop(while: { $0 == "/" })
                }
            }
            guard let parsed = URL(string: special, relativeTo: URL(string: "https://media.invalid"))?.absoluteURL,
                  let components = URLComponents(url: parsed, resolvingAgainstBaseURL: true) else { return nil }
            if ["http", "https"].contains(components.scheme?.lowercased() ?? ""), components.host?.isEmpty != false { return nil }
            path = components.percentEncodedPath
        }
        path = path.removingPercentEncoding ?? path
        let basename = path.components(separatedBy: CharacterSet(charactersIn: "/\\")).last ?? ""
        guard let dot = basename.lastIndex(of: ".") else { return nil }
        return mediaTypes[String(basename[basename.index(after: dot)...]).lowercased()]
    }

    static func dataURL(bytes: Data, contentType: String?, url: String,
                        cancelled: () -> Bool = { false }) throws -> String {
        func checkCancellation() throws { if cancelled() { throw Failure.cancelled } }
        try checkCancellation()
        guard let mime = mimeType(contentType: contentType, url: url) else { throw Failure.noType }
        guard bytes.count <= maxSourceBytes else { throw Failure.tooLarge }
        if let inline = inline(bytes, mime: mime) {
            try checkCancellation()
            return inline
        }
        guard mime != "image/svg+xml" else { throw Failure.cacheLimit }
        guard let source = CGImageSourceCreateWithData(bytes as CFData,
                    [kCGImageSourceShouldCache: false] as CFDictionary) else { throw Failure.decode }
        for edge in [96, 48] {
            try checkCancellation()
            guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: edge,
                kCGImageSourceShouldCacheImmediately: true,
            ] as CFDictionary), image.width <= edge, image.height <= edge else { throw Failure.decode }
            try checkCancellation()
            let alpha = [.first, .last, .premultipliedFirst, .premultipliedLast, .alphaOnly].contains(image.alphaInfo)
            let type = alpha ? UTType.png : UTType.jpeg
            let output = NSMutableData()
            guard let destination = CGImageDestinationCreateWithData(output, type.identifier as CFString, 1, nil) else { throw Failure.encode }
            // SDWebImage's default disk coder uses full quality; retain alpha in PNG.
            CGImageDestinationAddImage(destination, image, [kCGImageDestinationLossyCompressionQuality: 1.0] as CFDictionary)
            guard CGImageDestinationFinalize(destination) else { throw Failure.encode }
            try checkCancellation()
            if let result = inline(output as Data, mime: alpha ? "image/png" : "image/jpeg") { return result }
        }
        throw Failure.cacheLimit
    }

    private static func inline(_ data: Data, mime: String) -> String? {
        guard !data.isEmpty, inlineTypes.contains(mime) else { return nil }
        let prefix = "data:\(mime);base64,"
        guard prefix.utf8.count + ((data.count + 2) / 3) * 4 <= maxDataURLLength else { return nil }
        return prefix + data.base64EncodedString()
    }
}
