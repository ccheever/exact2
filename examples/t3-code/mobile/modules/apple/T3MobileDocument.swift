#if os(iOS)
// AttachmentDocument365aa87982: read at most1MiB+1 even when a server ignores Range.
// @ref llp/1109.005-composer-and-transcript.decision.md#media-presentation
import Foundation

final class T3MobileDocument {
    static let limit = 1024 * 1024
    private let dataRoot: URL
    private var reads: [UUID: Task<Void, Never>] = [:]
    private var alive = true

    init(dataRoot: URL) { self.dataRoot = dataRoot }
    func destroy() {
        alive = false
        for task in reads.values { task.cancel() }
        reads.removeAll()
    }
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let key = UUID()
        reads[key] = Task { @MainActor [weak self] in
            guard let self else { return }
            defer { reads.removeValue(forKey: key) }
            do {
                guard alive, let raw = request["sourceJSON"] as? String else { throw CancellationError() }
                let source = try T3MobileMediaSource(raw)
                let bytes = try await read(source, noCache: request["retry"] as? Bool ?? false)
                try Task.checkCancellation()
                guard alive else { throw CancellationError() }
                reply(["ok": true, "generation": generation,
                    "value": ["identifier": source.identifier, "base64": bytes.base64EncodedString()]])
            } catch {
                reply(["ok": false, "generation": generation, "error": [
                    "kind": error is CancellationError ? "superseded" : "Attachment",
                    "message": error is CancellationError ? "Preview cancelled." : error.localizedDescription]])
            }
        }
    }

    private func read(_ source: T3MobileMediaSource, noCache: Bool) async throws -> Data {
        if let url = source.url {
            let configuration = URLSessionConfiguration.ephemeral
            configuration.httpCookieStorage = nil
            configuration.urlCredentialStorage = nil
            configuration.timeoutIntervalForRequest = 30
            configuration.timeoutIntervalForResource = 60
            let session = URLSession(configuration: configuration)
            defer { session.invalidateAndCancel() }
            var request = URLRequest(url: url)
            request.setValue("bytes=0-\(Self.limit)", forHTTPHeaderField: "Range")
            if noCache { request.setValue("no-cache", forHTTPHeaderField: "Cache-Control") }
            let (stream, response) = try await session.bytes(for: request)
            guard let response = response as? HTTPURLResponse, (200..<300).contains(response.statusCode) else {
                throw T3MobileMediaFiles.error("The file could not be loaded. Reconnect and try again.")
            }
            var bytes = Data()
            bytes.reserveCapacity(Self.limit + 1)
            for try await byte in stream {
                try Task.checkCancellation()
                bytes.append(byte)
                if bytes.count == Self.limit + 1 { break }
            }
            return bytes
        }
        let root = dataRoot.appendingPathComponent(source.source == "draft-image" ? "snapshots/drafts" : "composer-files", isDirectory: true).resolvingSymlinksInPath()
        let file = root.appendingPathComponent(source.id.lowercased()).resolvingSymlinksInPath()
        guard file.deletingLastPathComponent() == root,
              try file.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile == true else {
            throw T3MobileMediaFiles.error("This attachment is no longer available. Attach the file again.")
        }
        let handle = try FileHandle(forReadingFrom: file)
        defer { try? handle.close() }
        try Task.checkCancellation()
        // Empty text is a valid document. No whole-file allocation or mutable lease is needed.
        return try handle.read(upToCount: Self.limit + 1) ?? Data()
    }
}
#endif
