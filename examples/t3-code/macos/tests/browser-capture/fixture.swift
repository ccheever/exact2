import AppKit
import Network
import WebKit
import XCTest

// browser-surface part 3 (capture): the shared loopback fixture and helpers of this directory's tests
// (annotate.swift, recording.swift, capture.swift; main.swift runs them). The fixture is part 1's
// (macos/tests/browser/main.swift). `EXACT_ASSETS` must name the app directory (examples/t3-code) so the
// page scripts load from its `assets/`; `T3_BROWSER_TEST_DIR` receives images and recordings.
final class CaptureFixture {
    private let listener: NWListener
    private(set) var paths: [String] = []
    /// path → (status, content type, body, delay in seconds, extra headers)
    var routes: [String: (Int, String, Data, Double, String)] = [:]
    private(set) var port: UInt16 = 0
    private let queue = DispatchQueue(label: "browser-capture-fixture")

    init() throws {
        listener = try NWListener(using: .tcp, on: .any)
        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
        listener.newConnectionHandler = { [weak self] connection in self?.serve(connection) }
        listener.start(queue: queue)
        _ = ready.wait(timeout: .now() + 5)
        port = listener.port?.rawValue ?? 0
    }
    var base: String { "http://127.0.0.1:\(port)" }
    var seen: [String] { queue.sync { paths } }
    func page(_ path: String, _ html: String, delay: Double = 0) { routes[path] = (200, "text/html", Data(html.utf8), delay, "") }
    func file(_ path: String, _ type: String, _ body: Data, headers: String = "") { routes[path] = (200, type, body, 0, headers) }

    private func serve(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, _, _ in
            guard let self, let data, let head = String(data: data, encoding: .utf8)?.split(separator: "\r\n").first else { return connection.cancel() }
            let path = String(head.split(separator: " ").dropFirst().first ?? "")
            self.paths.append(path)
            let route = String(path.split(separator: "?").first ?? "")
            let (status, type, body, delay, headers) = self.routes[route] ?? (404, "text/html", Data("<title>Not found</title>missing".utf8), 0, "")
            var response = Data("HTTP/1.1 \(status) \(status == 200 ? "OK" : "Not Found")\r\nContent-Type: \(type)\r\nContent-Length: \(body.count)\r\nCache-Control: no-store\r\n\(headers)Connection: close\r\n\r\n".utf8)
            response.append(body)
            self.queue.asyncAfter(deadline: .now() + delay) { connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() }) }
        }
    }
    deinit { listener.cancel() }
}

/// Run loop helpers shared by this directory's tests.
enum CaptureSpin {
    static func until(_ condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.01)) }
    }
    static func wait(_ seconds: TimeInterval) { until({ false }, timeout: seconds) }
    /// `evaluateJavaScript` in a content world (the page's by default), its result as a string.
    static func evaluate(_ web: WKWebView, _ script: String, world: WKContentWorld = .page) -> String {
        var value: String?
        web.evaluateJavaScript(script, in: nil, in: world) { result in
            switch result {
            case .success(let output): value = (output as? String) ?? "\(output)"
            case .failure(let error): value = "error: \(error.localizedDescription)"
            }
        }
        until({ value != nil }, timeout: 5)
        return value ?? "timeout"
    }
    /// A directory for a test's output files (`T3_BROWSER_TEST_DIR`, else a temporary one).
    static func outputDirectory(_ name: String) -> URL {
        let root = ProcessInfo.processInfo.environment["T3_BROWSER_TEST_DIR"].map { URL(fileURLWithPath: $0) } ?? FileManager.default.temporaryDirectory
        let url = root.appendingPathComponent(name, isDirectory: true)
        try? FileManager.default.removeItem(at: url)
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }
}
