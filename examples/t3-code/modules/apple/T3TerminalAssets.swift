#if os(macOS)
import Foundation
import WebKit

/// The terminal page's files (terminal-host/build.mjs writes them into the app's assets): the
/// `t3-terminal` scheme serves exactly these names and nothing else, so the page loads its
/// script, WASM and font from the app bundle with no network (MIT reference, see LICENSE-T3:
/// apps/web/src/terminal/ghostty/runtime.ts:46-67,190-219 and surface.ts:60-72 fetch them by
/// `?url`). The asset lookup follows T3Notifications.soundURL: `EXACT_ASSETS` in a development
/// run, else the bundle's `assets/`.
final class T3TerminalAssets: NSObject, WKURLSchemeHandler {
    static let scheme = "t3-terminal"
    static let pageURL = URL(string: "t3-terminal://host/terminal-host.html")!
    /// Scheme path → (asset file, MIME type).
    static let files: [String: (String, String)] = [
        "terminal-host.html": ("terminal-host.html", "text/html; charset=utf-8"),
        "terminal-host.js": ("terminal-host.js", "text/javascript; charset=utf-8"),
        "ghostty-vt.wasm": ("terminal-ghostty-vt.wasm", "application/wasm"),
        "ghostty-write-pty.wasm": ("terminal-ghostty-write-pty.wasm", "application/wasm"),
        "SymbolsNerdFontMono-Regular.woff2": ("terminal-symbols-nerd-font-mono.woff2", "font/woff2"),
    ]

    static func fileURL(_ file: String, assets: String? = ProcessInfo.processInfo.environment["EXACT_ASSETS"], resources: URL? = Bundle.main.resourceURL) -> URL? {
        var candidates: [URL] = []
        if let root = assets, root.hasPrefix("/") { candidates.append(URL(fileURLWithPath: root).appendingPathComponent("assets/\(file)")) }
        if let resources { candidates.append(resources.appendingPathComponent("assets/\(file)")) }
        return candidates.first(where: { FileManager.default.fileExists(atPath: $0.path) })
    }

    /// The bytes and MIME type for a scheme URL, or nil for anything that is not one of the files.
    static func resolve(_ url: URL?) -> (Data, String)? {
        guard let url, url.scheme == scheme, url.host == "host" else { return nil }
        let name = String(url.path.dropFirst())
        guard let (file, mime) = files[name], let path = fileURL(file), let data = try? Data(contentsOf: path) else { return nil }
        return (data, mime)
    }

    private(set) var served: [String] = []
    /// Requests and navigations refused: anything that is not one of the page's files.
    var refusedURLs: [String] = []

    func webView(_ webView: WKWebView, start task: WKURLSchemeTask) {
        guard let (data, mime) = Self.resolve(task.request.url) else {
            refusedURLs.append(task.request.url?.absoluteString ?? "?")
            task.didFailWithError(URLError(.fileDoesNotExist))
            return
        }
        served.append(task.request.url?.lastPathComponent ?? "")
        let response = HTTPURLResponse(url: task.request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: [
            "Content-Type": mime, "Content-Length": "\(data.count)", "Cache-Control": "no-store",
            "Access-Control-Allow-Origin": "t3-terminal://host",
        ])!
        task.didReceive(response)
        task.didReceive(data)
        task.didFinish()
    }

    func webView(_ webView: WKWebView, stop task: WKURLSchemeTask) {}
}
#endif
