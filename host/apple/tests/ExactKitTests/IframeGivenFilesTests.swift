import XCTest
import WebKit
@testable import ExactKit
#if os(macOS)
import AppKit
#endif

#if os(macOS) || os(iOS)
/// A bundled page given the app files its query names (LLP 1020.001 D7), in
/// scripts/fixtures/web-documents.contract's frame with its `src` set here.
final class IframeGivenFilesTests: XCTestCase {
    private static let page = "<!doctype html><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>given</title><p>given"
    private static let size = 20 * 1024 * 1024 + 1

    func testAPageReadsTheFilesItsQueryNamesOnlyWhileItsSourceLasts() throws {
        guard let module = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: base) }
        let bundle = base.appendingPathComponent("bundle"), roots = ["data", "cache", "tmp"].map { base.appendingPathComponent($0) }
        for directory in [bundle.appendingPathComponent("assets")] + roots {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
        try Data(Self.page.utf8).write(to: bundle.appendingPathComponent("assets/given.html"))
        // Past readFile's 16 MiB, with known bytes where the range reads.
        var big = Data(count: Self.size)
        big.replaceSubrange(10..<20, with: Data("ABCDEFGHIJ".utf8))
        try big.write(to: roots[1].appendingPathComponent("big.bin"))
        try Data("<p>a note".utf8).write(to: roots[2].appendingPathComponent("note.html"))
        try Data("secret".utf8).write(to: roots[0].appendingPathComponent("secret.txt"))
        let saved = ExactApp.shared.assetRoot
        ExactApp.shared.assetRoot = bundle
        defer { ExactApp.shared.assetRoot = saved }

        let (session, node) = try frame(module, "given-files")
        defer { session.destroy() }
        let (other, otherNode) = try frame(module, "given-files-other")
        defer { other.destroy() }
        // Each boot learns its runtime's roots: these are learned after both.
        AppFiles.learn(["roots": ["data": roots[0].path, "cache": roots[1].path, "tmp": roots[2].path]])
        show(session, node, "assets/given.html?file=app:/cache/big.bin&note=app%3A%2Ftmp%2Fnote.html&keep=a+b")
        let first = try run(node, """
            const q = new URLSearchParams(location.search), file = q.get('file');
            const whole = await fetch(file), bytes = (await whole.arrayBuffer()).byteLength;
            const part = await fetch(file, { headers: { Range: 'bytes=10-19' } });
            const head = await fetch(file, { method: 'HEAD' });
            const note = await fetch(q.get('note'));
            const guess = await fetch('exact-app://app/.exact/files/' + crypto.randomUUID()).then(r => r.status, () => 'refused');
            return JSON.stringify({ file, keep: q.get('keep'), status: whole.status, bytes, part: part.status,
              range: part.headers.get('content-range'), text: await part.text(), head: head.headers.get('content-length'),
              note: note.headers.get('content-type'), sandbox: note.headers.get('content-security-policy'), guess });
            """)
        let file = try XCTUnwrap(first["file"] as? String)
        XCTAssertTrue(file.hasPrefix("exact-app://app/.exact/files/"), "\(first)")
        XCTAssertEqual(first["keep"] as? String, "a b", "the other values stay as written")
        XCTAssertEqual(first["status"] as? Int, 200)
        XCTAssertEqual(first["bytes"] as? Int, Self.size)
        XCTAssertEqual(first["part"] as? Int, 206)
        XCTAssertEqual(first["range"] as? String, "bytes 10-19/\(Self.size)")
        XCTAssertEqual(first["text"] as? String, "ABCDEFGHIJ")
        XCTAssertEqual(first["head"] as? String, String(Self.size))
        XCTAssertEqual(first["note"] as? String, "text/html; charset=utf-8")
        XCTAssertEqual(first["sandbox"] as? String, "sandbox allow-scripts allow-forms allow-modals allow-popups allow-downloads",
                       "a given document never runs as the app's origin")
        XCTAssertEqual(first["guess"] as? Int, 404)

        // Another frame's page cannot read this frame's file.
        show(other, otherNode, "assets/given.html")
        XCTAssertEqual(try run(otherNode, "return JSON.stringify({ status: await fetch(\(quoted(file))).then(r => r.status, () => 'refused') })")["status"] as? Int, 404)

        // A new source ends the old one's files.
        show(session, node, "assets/given.html?file=app:/tmp/note.html")
        XCTAssertEqual(try run(node, "return JSON.stringify({ status: await fetch(\(quoted(file))).then(r => r.status, () => 'refused') })")["status"] as? Int, 404)

        // In an opaque sandbox the page cannot fetch it, as on the web.
        node.props["sandbox"] = "allow-scripts"
        show(session, node, "assets/given.html?file=app:/tmp/note.html&opaque=1")
        XCTAssertEqual(try run(node, "return JSON.stringify({ status: await fetch(new URLSearchParams(location.search).get('file')).then(r => r.status, () => 'refused') })")["status"] as? String, "refused")
        node.props["sandbox"] = nil

        // app:/data is never given: `error`, and no page.
        session.presenter.onLoad = { _ in XCTFail("a page named a refused file must not load") }
        node.props["src"] = "assets/given.html?file=app:/data/secret.txt"
        session.webviews.update(node)
        let refused = expectation(description: "the frame's error")
        let deadline = Date(timeIntervalSinceNow: 30)
        while Date() < deadline {
            if session.agent("{\"op\":\"state\"}").contains("app:/data/secret.txt: a page is given app:/cache and app:/tmp files only") { refused.fulfill(); break }
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05))
        }
        wait(for: [refused], timeout: 1)
        session.presenter.onLoad = nil
    }

    /// A session showing the fixture's one frame.
    private func frame(_ module: String, _ label: String) throws -> (ExactSession, NodeView) {
        let session = ExactApp.shared.makeSession(label: label)
        session.webviews.moduleLocation = { module }
        XCTAssertNil(session.boot(plan: try fixturePlan("WEB_DOCUMENTS"), size: CGSize(width: 300, height: 200)).error)
        return (session, try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "pdf" }))
    }

    private func show(_ session: ExactSession, _ node: NodeView, _ src: String) {
        let loaded = expectation(description: src)
        session.presenter.onLoad = { _ in loaded.fulfill() }
        node.props["src"] = src
        session.webviews.update(node)
        wait(for: [loaded], timeout: 30)
        session.presenter.onLoad = nil
    }

    /// The page's own script, in its world; it answers JSON.
    private func run(_ node: NodeView, _ script: String) throws -> [String: Any] {
        let web = try XCTUnwrap(node.web as? WKWebView)
        let done = expectation(description: "the page's script")
        var answer: [String: Any] = [:]
        web.callAsyncJavaScript(script, arguments: [:], in: nil, in: .page) { result in
            if case .success(let value) = result, let text = value as? String,
               let object = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any] {
                answer = object
            } else {
                XCTFail("\(result)")
            }
            done.fulfill()
        }
        wait(for: [done], timeout: 30)
        return answer
    }

    private func quoted(_ text: String) -> String {
        String(decoding: try! JSONSerialization.data(withJSONObject: text, options: .fragmentsAllowed), as: UTF8.self)
    }
}
#endif
