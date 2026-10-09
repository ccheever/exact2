// @ref llp/1109.005-composer-and-transcript.decision.md#incoming-share-inbox
// App-owned ingress for pinned365aa87982 features/sharing.
// GAP 006: Share Extension packaging is unavailable; there is no system ingress yet.
import Foundation
import CryptoKit

/// Synchronous operations run on the containing module's serial queue. No paths or bytes
/// cross the producer boundary; a later draft adoption must copy these owned references.
final class T3MobileIncomingShares {
    struct Payload: Codable {
        let shareType: String
        let mimeType: String?
        let value: String
        let originalName: String?
    }
    struct Attachment: Codable {
        let id: String
        let kind: String
        let name: String
        let mimeType: String
        let sizeBytes: Int
    }
    struct Entry: Codable {
        let schemaVersion: Int
        let id: String
        let createdAt: String
        let text: String
        let attachments: [Attachment]
        let warnings: [String]
    }
    private let directory: URL
    private let cleanupRoots: [URL]
    private let manager = FileManager.default
    private let write: (Data, URL) throws -> Void
    private static let images = Set(["image/png", "image/jpeg", "image/gif", "image/webp"])
    private static let kinds = Set(["image", "file", "audio", "video"])
    private static let imageLimit = 10 * 1024 * 1024
    private static let fileLimit = 50 * 1024 * 1024

    init(directory: URL, cleanupRoots: [URL] = [], write: @escaping (Data, URL) throws -> Void = {
        try $0.write(to: $1, options: .atomic)
    }) {
        self.directory = directory
        self.cleanupRoots = cleanupRoots
        self.write = write
    }
    private func fail(_ message: String) -> NSError {
        NSError(domain: "T3MobileIncomingShares", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }
    /// Matches JSON.stringify([{shareType,mimeType,value}]) at the pinned mobile source.
    /// Names do not change identity; payload order does. Native acknowledgement uses this ID.
    static func identity(_ payloads: [Payload]) throws -> String {
        func json(_ value: Any) throws -> String {
            String(decoding: try JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed, .withoutEscapingSlashes]), as: UTF8.self)
        }
        let rows = try payloads.map {
            "{\"shareType\":\(try json($0.shareType)),\"mimeType\":\(try json($0.mimeType as Any? ?? NSNull())),\"value\":\(try json($0.value))}"
        }
        let fingerprint = Data(("[" + rows.joined(separator: ",") + "]").utf8)
        return "share-" + SHA256.hash(data: fingerprint).map { String(format: "%02x", $0) }.joined()
    }
    private static func validID(_ id: String) -> Bool {
        id.count == 70 && id.hasPrefix("share-") && id.dropFirst(6).allSatisfy { "0123456789abcdef".contains($0) }
    }
    private func folder(_ id: String) throws -> URL {
        guard Self.validID(id) else { throw fail("The incoming share identity is invalid.") }
        return directory.appendingPathComponent(id, isDirectory: true)
    }
    private func read(_ id: String) throws -> Entry? {
        let root = try folder(id), path = root.appendingPathComponent("entry.json")
        guard manager.fileExists(atPath: path.path) else { return nil }
        let entry = try JSONDecoder().decode(Entry.self, from: Data(contentsOf: path))
        guard entry.schemaVersion == 1, entry.id == id,
              ISO8601DateFormatter().date(from: entry.createdAt) != nil, entry.attachments.count <= 100,
              !entry.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !entry.attachments.isEmpty else {
            throw fail("The saved incoming share is invalid.")
        }
        var ids = Set<String>()
        for attachment in entry.attachments {
            guard UUID(uuidString: attachment.id) != nil, ids.insert(attachment.id.lowercased()).inserted,
                  ["image", "file"].contains(attachment.kind), !attachment.name.isEmpty,
                  attachment.sizeBytes > 0, attachment.sizeBytes <= (attachment.kind == "image" ? Self.imageLimit : Self.fileLimit),
                  attachment.kind != "image" || Self.images.contains(attachment.mimeType) else {
                throw fail("A saved incoming attachment is invalid.")
            }
        }
        return entry
    }
    func entries() throws -> [Entry] {
        guard manager.fileExists(atPath: directory.path) else { return [] }
        let entries = try manager.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
            .filter { Self.validID($0.lastPathComponent) }
            .compactMap { try read($0.lastPathComponent) }
        return entries.sorted { $0.createdAt == $1.createdAt ? $0.id < $1.id : $0.createdAt > $1.createdAt }
    }
    /// Reload the saved metadata and actual owned bytes. A dangling or changed file is
    /// an error, never an invented attachment. Consumers cannot supply arbitrary paths.
    func attachmentURL(shareID: String, attachmentID: String) throws -> URL {
        guard let entry = try read(shareID), let attachment = entry.attachments.first(where: { $0.id == attachmentID }) else {
            throw fail("The shared attachment is no longer available.")
        }
        let root = try folder(shareID), file = root.appendingPathComponent(attachment.id)
        guard Self.ownedFile(file, roots: [root]),
              try file.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey]).isRegularFile == true,
              try file.resourceValues(forKeys: [.fileSizeKey]).fileSize == attachment.sizeBytes else {
            throw fail("The shared attachment bytes are unavailable.")
        }
        return file
    }
    /// The extension's copied payload remains recoverable until the entry and every
    /// accepted byte copy are durable. A failed ACK leaves the same content-derived ID.
    @discardableResult
    func ingest(_ payloads: [Payload], createdAt: Date = Date(), acknowledge: (String) throws -> Void) throws -> Entry? {
        guard !payloads.isEmpty else { return nil }
        let id = try Self.identity(payloads), root = try folder(id)
        if let existing = try read(id) {
            // Do not acknowledge a persisted entry whose referenced bytes went missing.
            for attachment in existing.attachments { _ = try attachmentURL(shareID: id, attachmentID: attachment.id) }
            cleanup(payloads)
            try acknowledge(id)
            return existing
        }
        // An interrupted pre-commit import owns this directory but has no entry. It
        // can be replaced only while the original native payload is still present.
        if manager.fileExists(atPath: root.path) { try manager.removeItem(at: root) }
        try manager.createDirectory(at: root, withIntermediateDirectories: true)
        var attachments: [Attachment] = [], warnings: [String] = [], texts: [String] = []
        var seenText = Set<String>(), limitWarned = false, committed = false
        defer { if !committed { try? manager.removeItem(at: root) } }
        for payload in payloads {
            if ["text", "url"].contains(payload.shareType) {
                let text = payload.value.trimmingCharacters(in: .whitespacesAndNewlines)
                if !text.isEmpty && seenText.insert(text).inserted { texts.append(text) }
                continue
            }
            guard Self.kinds.contains(payload.shareType) else { continue }
            if attachments.count >= 100 {
                if !limitWarned { warnings.append("Only the first 100 shared \(payload.shareType == "image" ? "images" : "files") were attached."); limitWarned = true }
                continue
            }
            let image = payload.shareType == "image"
            let mime = (payload.mimeType ?? (image ? "image/png" : "application/octet-stream")).lowercased()
            let source = Self.localFile(payload.value)
            let name = payload.originalName?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false
                ? payload.originalName! : source?.lastPathComponent.isEmpty == false ? source!.lastPathComponent : "shared-\(image ? "image" : "file")-\(attachments.count + 1)"
            if image && !Self.images.contains(mime) { warnings.append("'\(name)' is not a supported image type."); continue }
            guard let source else { warnings.append("Could not read '\(name)'."); continue }
            let access = source.startAccessingSecurityScopedResource()
            defer { if access { source.stopAccessingSecurityScopedResource() } }
            let limit = image ? Self.imageLimit : Self.fileLimit
            let attachmentID = UUID().uuidString.lowercased(), target = root.appendingPathComponent(attachmentID)
            do {
                let properties = try source.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey])
                guard properties.isRegularFile == true, let size = properties.fileSize, size > 0 else {
                    warnings.append("'\(name)' is empty or could not be read."); continue
                }
                guard size <= limit else { warnings.append("'\(name)' exceeds the \(image ? 10 : 50) MB attachment limit."); continue }
                try manager.copyItem(at: source, to: target)
                // Measure the actual copy rather than trusting sender metadata.
                let stored = try target.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey])
                guard stored.isRegularFile == true, let storedSize = stored.fileSize, storedSize > 0, storedSize <= limit,
                      Self.ownedFile(target, roots: [root]) else {
                    try? manager.removeItem(at: target)
                    warnings.append("'\(name)' is empty, too large or could not be read."); continue
                }
                attachments.append(Attachment(id: attachmentID, kind: image ? "image" : "file", name: name, mimeType: mime, sizeBytes: storedSize))
            } catch {
                try? manager.removeItem(at: target)
                warnings.append("Could not read '\(name)'.")
            }
        }
        let entry = Entry(schemaVersion: 1, id: id, createdAt: ISO8601DateFormatter().string(from: createdAt),
                          text: texts.joined(separator: "\n\n"), attachments: attachments, warnings: warnings)
        guard !entry.text.isEmpty || !attachments.isEmpty else {
            cleanup(payloads)
            try acknowledge(id)
            throw fail(warnings.first ?? "The shared content is not supported by the composer.")
        }
        try write(JSONEncoder().encode(entry), root.appendingPathComponent("entry.json"))
        committed = true
        cleanup(payloads)
        try acknowledge(id)
        return entry
    }
    private func cleanup(_ payloads: [Payload]) {
        for payload in payloads where Self.kinds.contains(payload.shareType) {
            guard let file = Self.localFile(payload.value), Self.ownedFile(file, roots: cleanupRoots),
                  !Self.ownedFile(file, roots: [directory]) else { continue }
            try? manager.removeItem(at: file)
        }
    }
    private static func localFile(_ raw: String) -> URL? {
        guard let url = URL(string: raw), url.isFileURL, url.host == nil || url.host == "" || url.host == "localhost",
              let decoded = URLComponents(string: raw)?.percentEncodedPath.removingPercentEncoding,
              !decoded.split(separator: "/").contains(".."), !decoded.contains("\0") else { return nil }
        return url
    }
    /// Resolve symlinks as well as /private aliases. A lexical prefix alone permits
    /// a copied link to delete a sender's document outside the App Group.
    static func ownedFile(_ file: URL, roots: [URL]) -> Bool {
        guard let local = localFile(file.absoluteString) else { return false }
        let path = local.resolvingSymlinksInPath().standardizedFileURL.path
        return roots.contains { root in
            guard let localRoot = localFile(root.absoluteString) else { return false }
            let prefix = localRoot.resolvingSymlinksInPath().standardizedFileURL.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            let rootPath = "/" + prefix + "/"
            return path.hasPrefix(rootPath) && path.count > rootPath.count
        }
    }
}
