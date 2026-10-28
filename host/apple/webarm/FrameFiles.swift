// A bundled page's given files (@ref LLP 1020.001 D7): each `app:/cache` or
// `app:/tmp` file the page's query names is served to that frame's web view at
// `exact-app://app/.exact/files/<token>`, read-only, until the frame is served
// again or unmounts. The token is the page's only name for the file.
import Foundation
import WebKit

final class FrameFiles {
    static let prefix = ".exact/files/"
    /// Reads off the main thread: a piece at a time, each handed to WebKit on
    /// the main thread before the next is read, so the app's process never
    /// holds a file whole (WebKit keeps what the page has not read yet).
    static let reads = DispatchQueue(label: "exact.web.files", qos: .userInitiated, attributes: .concurrent)
    static let piece = 1 << 20
    /// Every given file's response: opened as a document, a file a person
    /// brought (an HTML page, an SVG, any XML type) runs in an opaque origin,
    /// never as the app's own, whatever type its extension maps to.
    static let sandbox = "sandbox allow-scripts allow-forms allow-modals allow-popups allow-downloads"
    /// What `exact_web_set_files` resolved for the source: each `app:/` path
    /// the page's query names, to its root's directory.
    var supplied: [String: String] = [:]
    /// The current serve's tokens: the pinned root and the names below it.
    private var given: [String: (root: Int32, parts: [String])] = [:]
    /// Roots opened at the current serve, by path, closed at the next.
    private var roots: [String: Int32] = [:]

    /// `page` with each query value naming a file the page may be given
    /// replaced by a new token's URL; any other value stays as written, and
    /// the host log says why. Every serve calls it, so the last serve's
    /// tokens end.
    func give(_ page: URL?, source: String?) -> URL? {
        close()
        guard let page, !supplied.isEmpty, var parts = URLComponents(url: page, resolvingAgainstBaseURL: false),
              let query = parts.percentEncodedQuery else { return page }
        var tokens: [String: String?] = [:]
        parts.percentEncodedQuery = query.split(separator: "&", omittingEmptySubsequences: false).map { pair -> String in
            guard let equals = pair.firstIndex(of: "="), let path = FrameFiles.value(pair[pair.index(after: equals)...]),
                  let rootPath = supplied[path] else { return String(pair) }
            if tokens[path] == nil { tokens[path] = token(path, rootPath, source: source) }
            guard let token = tokens[path] ?? nil else { return String(pair) }
            let url = "\(AssetScheme.origin)/\(FrameFiles.prefix)\(token)"
            return "\(pair[..<equals])=\(url.addingPercentEncoding(withAllowedCharacters: FrameFiles.unreserved) ?? url)"
        }.joined(separator: "&")
        return parts.url ?? page
    }

    /// A token for the regular file at `path` below its root, which is
    /// pinned now; nil, said in the log, when there is none.
    private func token(_ path: String, _ rootPath: String, source: String?) -> String? {
        let names = path.split(separator: "/", omittingEmptySubsequences: false).dropFirst(2).map(String.init)
        if roots[rootPath] == nil { roots[rootPath] = Darwin.open(rootPath, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC) }
        guard let root = roots[rootPath], root >= 0, let file = FrameFiles.open(below: dup(root), names) else {
            FileHandle.standardError.write(Data("exact: iframe \(source ?? ""): \(path) is not given (no such regular file)\n".utf8))
            return nil
        }
        try? file.close()
        let token = UUID().uuidString.lowercased()
        given[token] = (root, names)
        return token
    }

    /// A new descriptor of a given file's pinned root, and the names below
    /// it, while its token lasts; the caller closes the descriptor.
    func file(_ name: String) -> (root: Int32, parts: [String])? {
        guard let entry = given[String(name.dropFirst(FrameFiles.prefix.count))] else { return nil }
        let root = dup(entry.root)
        return root >= 0 ? (root, entry.parts) : nil
    }

    private func close() {
        given = [:]
        for root in roots.values where root >= 0 { Darwin.close(root) }
        roots = [:]
    }

    func end() {
        close()
        supplied = [:]
    }

    deinit { close() }

    /// The regular file at `names` below the directory `dir`, which this
    /// closes, following no link on the way (`app_fs_unix.rs` opens app
    /// files so): nil when any part is missing, a link, or not a directory,
    /// or the last is not a regular file.
    static func open(below dir: Int32, _ names: [String]) -> FileHandle? {
        guard dir >= 0 else { return nil }
        var current = dir
        for (index, name) in names.enumerated() {
            let last = index == names.count - 1
            let next = openat(current, name, (last ? O_RDONLY | O_NONBLOCK : O_RDONLY | O_DIRECTORY) | O_NOFOLLOW | O_CLOEXEC)
            Darwin.close(current)
            guard next >= 0 else { return nil }
            current = next
        }
        var info = stat()
        guard !names.isEmpty, fstat(current, &info) == 0, info.st_mode & S_IFMT == S_IFREG else {
            Darwin.close(current)
            return nil
        }
        return FileHandle(fileDescriptor: current, closeOnDealloc: true)
    }

    /// A query value as a form reads it (`URLSearchParams`): `+` is a space.
    static func value(_ raw: Substring) -> String? {
        raw.replacingOccurrences(of: "+", with: " ").removingPercentEncoding
    }

    static let unreserved = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
}

extension AssetScheme {
    /// A given file: one `bytes=` range, or the whole file in pieces; a
    /// `HEAD` gets the headers alone. Every call on the task is made on the
    /// main thread, where WebKit's stop arrives, so a stopped task is never
    /// answered again.
    func serveGiven(_ task: WKURLSchemeTask, _ url: URL, _ file: (root: Int32, parts: [String])) {
        let id = ObjectIdentifier(task), method = (task.request.httpMethod ?? "GET").uppercased()
        guard method == "GET" || method == "HEAD" else {
            Darwin.close(file.root)
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
        FrameFiles.reads.async { [self] in
            guard let handle = FrameFiles.open(below: file.root, file.parts), let end = try? handle.seekToEnd() else {
                _ = step { self.refuse(task, url, given: "is no longer a regular file below its root") }
                return
            }
            defer { try? handle.close() }
            let size = Int(end)
            var from = 0, to = size - 1, status = 200
            if let range, let bytes = AssetScheme.bytes(range, count: size) { (from, to, status) = (bytes.0, bytes.1, 206) }
            let length = max(0, to - from + 1)
            var headers = ["Content-Type": AssetScheme.type((file.parts.last.map { ($0 as NSString).pathExtension }) ?? ""),
                           "Content-Length": String(length), "Accept-Ranges": "bytes", "Cache-Control": "no-store",
                           "X-Content-Type-Options": "nosniff", "Content-Security-Policy": FrameFiles.sandbox]
            if status == 206 { headers["Content-Range"] = "bytes \(from)-\(to)/\(size)" }
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
