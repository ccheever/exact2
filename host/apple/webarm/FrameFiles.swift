// A bundled page's given files (@ref LLP 1020.001 D7): each `app:/cache` or
// `app:/tmp` path the host resolved in the page's query is served to that
// frame's web view at `exact-app://app/.exact/files/<token>`, read-only, until
// the frame is served again or unmounts. The token is the page's only name
// for the file.
import Foundation
import WebKit

final class FrameFiles {
    static let prefix = ".exact/files/"
    /// Reads off the main thread; each piece waits until WebKit has the last,
    /// so a large file is never held whole.
    static let reads = DispatchQueue(label: "exact.web.files", qos: .userInitiated, attributes: .concurrent)
    static let piece = 1 << 20
    /// What `exact_web_set_files` resolved for the source: each `app:/` path
    /// the page's query names, to its file.
    var supplied: [String: URL] = [:]
    /// What the current serve gave the page, by token.
    private var given: [String: URL] = [:]

    /// `page` with each query value the host resolved replaced by a new
    /// token's URL. Every serve calls it, so the last serve's tokens end.
    func give(_ page: URL?) -> URL? {
        given = [:]
        guard let page, !supplied.isEmpty, var parts = URLComponents(url: page, resolvingAgainstBaseURL: false),
              let query = parts.percentEncodedQuery else { return page }
        var tokens: [String: String] = [:]
        parts.percentEncodedQuery = query.split(separator: "&", omittingEmptySubsequences: false).map { pair -> String in
            guard let equals = pair.firstIndex(of: "="), let path = FrameFiles.value(pair[pair.index(after: equals)...]),
                  let file = supplied[path] else { return String(pair) }
            let token = tokens[path] ?? UUID().uuidString.lowercased()
            tokens[path] = token
            given[token] = file
            let url = "\(AssetScheme.origin)/\(FrameFiles.prefix)\(token)"
            return "\(pair[..<equals])=\(url.addingPercentEncoding(withAllowedCharacters: FrameFiles.unreserved) ?? url)"
        }.joined(separator: "&")
        return parts.url ?? page
    }

    /// The file a `.exact/files/<token>` name was given for, while it lasts.
    func file(_ name: String) -> URL? { given[String(name.dropFirst(FrameFiles.prefix.count))] }

    func end() {
        given = [:]
        supplied = [:]
    }

    /// A query value as a form reads it (`URLSearchParams`): `+` is a space.
    static func value(_ raw: Substring) -> String? {
        raw.replacingOccurrences(of: "+", with: " ").removingPercentEncoding
    }

    static let unreserved = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")

    /// A type whose document runs script when a page frames or opens it.
    static func scripted(_ type: String) -> Bool {
        let bare = type.split(separator: ";", maxSplits: 1)[0].trimmingCharacters(in: .whitespaces).lowercased()
        return ["text/html", "application/xhtml+xml", "image/svg+xml", "application/xml", "text/xml"].contains(bare)
    }
}

extension AssetScheme {
    /// A given file: one `bytes=` range, or the whole file in pieces; a
    /// `HEAD` gets the headers alone. Every call on the task is made on the
    /// main thread, where WebKit's stop arrives, so a stopped task is never
    /// answered again.
    func serveGiven(_ task: WKURLSchemeTask, _ url: URL, _ file: URL) {
        let id = ObjectIdentifier(task), method = (task.request.httpMethod ?? "GET").uppercased()
        guard method == "GET" || method == "HEAD" else {
            task.didReceive(HTTPURLResponse(url: url, statusCode: 405, httpVersion: "HTTP/1.1", headerFields: ["Allow": "GET, HEAD", "Content-Length": "0"])!)
            return task.didFinish()
        }
        let range = task.request.value(forHTTPHeaderField: "Range")
        let step = { [weak self] (body: () -> Void) -> Bool in
            DispatchQueue.main.sync {
                guard let self, self.stopped.remove(id) == nil else { return false }
                body()
                return true
            }
        }
        FrameFiles.reads.async {
            let handle: FileHandle, size: Int
            do {
                handle = try FileHandle(forReadingFrom: file)
                size = Int(try handle.seekToEnd())
            } catch {
                _ = step { self.refuse(task, url, given: "could not be read (\(error.localizedDescription))") }
                return
            }
            defer { try? handle.close() }
            var from = 0, to = size - 1, status = 200
            if let range, let bytes = AssetScheme.bytes(range, count: size) { (from, to, status) = (bytes.0, bytes.1, 206) }
            let length = max(0, to - from + 1), type = AssetScheme.type(file.pathExtension)
            var headers = ["Content-Type": type, "Content-Length": String(length), "Accept-Ranges": "bytes",
                           "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff"]
            if status == 206 { headers["Content-Range"] = "bytes \(from)-\(to)/\(size)" }
            // Opened as a document, a file the page was given never runs as
            // the app's own origin.
            if FrameFiles.scripted(type) { headers["Content-Security-Policy"] = "sandbox allow-scripts allow-forms allow-modals allow-popups allow-downloads" }
            guard step({ task.didReceive(HTTPURLResponse(url: url, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!) }) else { return }
            do {
                var left = method == "GET" ? length : 0
                try handle.seek(toOffset: UInt64(from))
                while left > 0 {
                    guard let data = try handle.read(upToCount: min(left, FrameFiles.piece)), !data.isEmpty else {
                        throw CocoaError(.fileReadCorruptFile)
                    }
                    left -= data.count
                    guard step({ task.didReceive(data) }) else { return }
                }
                _ = step { task.didFinish() }
            } catch {
                _ = step { task.didFailWithError(error) }
            }
        }
    }
}
