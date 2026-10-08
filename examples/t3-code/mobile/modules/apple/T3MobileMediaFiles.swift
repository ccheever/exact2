#if os(iOS)
// Adapted from T3 Code (MIT), 365aa87982 T3NativeFilePresentation.prepareFile/localAttachmentPreview.
// @ref llp/1109.005-composer-and-transcript.decision.md#media-presentation
import Foundation
import ImageIO
import CoreGraphics
import UniformTypeIdentifiers

struct T3MobileMediaSource {
    let identifier: String
    let name: String
    let kind: String
    let source: String
    let id: String
    let url: URL?

    init(_ text: String) throws {
        guard let value = try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
              let identifier = value["identifier"] as? String, !identifier.isEmpty,
              let name = value["name"] as? String, let kind = value["kind"] as? String,
              ["image", "video", "file"].contains(kind), let source = value["source"] as? String,
              ["remote", "draft-image", "draft-file"].contains(source), let id = value["id"] as? String, !id.isEmpty else {
            throw T3MobileMediaFiles.error("That preview is no longer available.")
        }
        self.identifier = identifier; self.name = name; self.kind = kind; self.source = source; self.id = id
        if source == "remote" {
            guard let text = value["url"] as? String, let url = URL(string: text),
                  ["https", "http"].contains(url.scheme?.lowercased() ?? ""), url.host != nil,
                  url.user == nil, url.password == nil else { throw T3MobileMediaFiles.error("The media URL is invalid.") }
            self.url = url
        } else {
            guard UUID(uuidString: id) != nil else { throw T3MobileMediaFiles.error("This attachment is no longer available. Attach the file again.") }
            url = nil
        }
    }
}

/// Each preview/share leases its own original-byte copy. Removing a draft cannot remove an open lease.
final class T3MobileMediaFiles {
    private let dataRoot: URL
    init(dataRoot: URL) { self.dataRoot = dataRoot }
    static func error(_ message: String) -> Error { NSError(domain: "T3MobileMedia", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
    static func release(_ file: URL?) { if let file { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) } }

    func prepare(_ source: T3MobileMediaSource) async throws -> URL {
        try Task.checkCancellation()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("t3-media-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        do {
            let original = directory.appendingPathComponent("original")
            if let url = source.url {
                let configuration = URLSessionConfiguration.ephemeral
                configuration.httpCookieStorage = nil; configuration.urlCredentialStorage = nil
                configuration.timeoutIntervalForRequest = 30; configuration.timeoutIntervalForResource = 60
                let session = URLSession(configuration: configuration)
                defer { session.invalidateAndCancel() }
                let (file, response) = try await session.download(from: url)
                guard let response = response as? HTTPURLResponse, (200..<300).contains(response.statusCode) else {
                    throw Self.error("The file could not be loaded. Check the connection and try again.")
                }
                try FileManager.default.moveItem(at: file, to: original)
            } else {
                let root = dataRoot.appendingPathComponent(source.source == "draft-image" ? "snapshots/drafts" : "composer-files", isDirectory: true).resolvingSymlinksInPath()
                let file = root.appendingPathComponent(source.id.lowercased()).resolvingSymlinksInPath()
                guard file.deletingLastPathComponent() == root, FileManager.default.isReadableFile(atPath: file.path) else {
                    throw Self.error("This attachment is no longer available. Attach the file again.")
                }
                try FileManager.default.copyItem(at: file, to: original)
            }
            try Task.checkCancellation()
            let resource = try original.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey])
            guard resource.isRegularFile == true, let bytes = resource.fileSize, bytes > 0, bytes <= 50 * 1024 * 1024 else {
                throw Self.error("This file is empty or exceeds the 50 MB attachment limit.")
            }
            // Pinned Quick Look adapter detects image/PDF content before choosing an extension.
            let filename = URL(fileURLWithPath: source.name).lastPathComponent as NSString
            let originalExtension = filename.pathExtension
            var type: UTType?
            if let image = CGImageSourceCreateWithURL(original as CFURL, nil), CGImageSourceGetCount(image) > 0,
               let identifier = CGImageSourceGetType(image) { type = UTType(identifier as String) }
            else if CGPDFDocument(original as CFURL) != nil { type = .pdf }
            else if originalExtension.lowercased() == "svg" { type = .svg }
            let ext: String
            if let type { ext = UTType(filenameExtension: originalExtension) == type ? originalExtension : type.preferredFilenameExtension ?? originalExtension }
            else { ext = originalExtension }
            var name = String(filename.deletingPathExtension.prefix(60)).components(separatedBy: .controlCharacters).joined(separator: "_")
            while name.utf8.count > 200 { name.removeLast() }
            if name.isEmpty { name = "Preview" }
            let file = directory.appendingPathComponent(ext.isEmpty ? name : "\(name).\(ext)")
            try FileManager.default.moveItem(at: original, to: file)
            return file
        } catch { try? FileManager.default.removeItem(at: directory); throw error }
    }
}
#endif
