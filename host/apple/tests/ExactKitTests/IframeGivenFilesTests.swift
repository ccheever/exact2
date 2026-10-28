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
    /// A page iOS shows as its web view's own document (its viewport asks for
    /// the device's width), and one it wraps, which posts what it read.
    private static let page = "<!doctype html><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>given</title><p>given"
    private static let wrapped = "<!doctype html><title>wrapped</title><script>(async () => { const f = new URLSearchParams(location.search).get('file'); let out; try { const r = await fetch(f); out = { status: r.status, bytes: (await r.arrayBuffer()).byteLength }; } catch (e) { out = { error: String(e) }; } parent.postMessage(JSON.stringify(out), '*'); })()</script>"
    private static let size = 20 * 1024 * 1024 + 1
    private static let odd = ["report #2.pdf", "a&b.pdf", "a+b.pdf", "100%.pdf", "my file.pdf"]
    private static let documents = ["feed.rss", "a.svg", "a.xhtml", "a.xsl", "a.html"]
    /// A script each document type runs, telling its parent its origin.
    private static let script = "<script xmlns=\"http://www.w3.org/1999/xhtml\">parent.postMessage('origin=' + self.origin, '*')</script>"

    private var base: URL!
    private var saved: URL!

    override func setUpWithError() throws {
        base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let fm = FileManager.default
        for directory in ["bundle/assets", "data", "cache/dir", "tmp", "elsewhere"] {
            try fm.createDirectory(at: base.appendingPathComponent(directory), withIntermediateDirectories: true)
        }
        try Data(Self.page.utf8).write(to: base.appendingPathComponent("bundle/assets/given.html"))
        try Data(Self.wrapped.utf8).write(to: base.appendingPathComponent("bundle/assets/wrapped.html"))
        // Past readFile's 16 MiB, with known bytes where the range reads.
        var big = Data(count: Self.size)
        big.replaceSubrange(10..<20, with: Data("ABCDEFGHIJ".utf8))
        try big.write(to: path("cache/big.bin"))
        try Data("0123456789".utf8).write(to: path("cache/digits.txt"))
        try Data().write(to: path("cache/empty.txt"))
        try Data("<p>a note".utf8).write(to: path("tmp/note.html"))
        for name in Self.odd { try Data(name.utf8).write(to: path("cache/\(name)")) }
        try Data("<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>t</title>\(Self.script)</channel></rss>".utf8).write(to: path("cache/feed.rss"))
        try Data("<svg xmlns=\"http://www.w3.org/2000/svg\"><script>parent.postMessage('origin=' + self.origin, '*')</script></svg>".utf8).write(to: path("cache/a.svg"))
        try Data("<html xmlns=\"http://www.w3.org/1999/xhtml\"><body>\(Self.script)</body></html>".utf8).write(to: path("cache/a.xhtml"))
        try Data("<?xml version=\"1.0\"?><x>\(Self.script)</x>".utf8).write(to: path("cache/a.xsl"))
        try Data("<!doctype html>\(Self.script)".utf8).write(to: path("cache/a.html"))
        try Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==")!.write(to: path("cache/dot.png"))
        try Data("secret".utf8).write(to: path("data/secret.txt"))
        try Data("elsewhere".utf8).write(to: path("elsewhere/big.bin"))
        // Links inside a root: to a file, and through a directory.
        try fm.createSymbolicLink(at: path("cache/link.txt"), withDestinationURL: path("data/secret.txt"))
        try fm.createSymbolicLink(at: path("cache/up"), withDestinationURL: path("data"))
        saved = ExactApp.shared.assetRoot
        ExactApp.shared.assetRoot = base.appendingPathComponent("bundle")
    }

    override func tearDownWithError() throws {
        ExactApp.shared.assetRoot = saved
        try? FileManager.default.removeItem(at: base)
    }

    private func path(_ name: String) -> URL { base.appendingPathComponent(name) }

    func testAPageReadsTheFilesItsQueryNames() throws {
        let (session, node) = try frames(1)[0]
        defer { session.destroy() }
        let encoded = Self.odd.map { "file=" + encode("app:/cache/\($0)") }.joined(separator: "&")
        show(session, node, "assets/given.html?file=app:/cache/big.bin&\(encoded)&file=app:/cache/empty.txt&file=app:/cache/digits.txt&keep=a+b#top")
        let read = try run(node, """
            const q = new URLSearchParams(location.search), [big, ...rest] = q.getAll('file'), digits = rest.pop(), empty = rest.pop();
            const whole = await fetch(big), bytes = (await whole.arrayBuffer()).byteLength;
            const part = await fetch(big, { headers: { Range: 'bytes=10-19' } });
            const head = await fetch(big, { method: 'HEAD' });
            const range = async r => { const res = await fetch(digits, { headers: { Range: r } }); return [res.status, await res.text()]; };
            return JSON.stringify({ big, keep: q.get('keep'), hash: location.hash, status: whole.status, bytes, part: part.status,
              contentRange: part.headers.get('content-range'), text: await part.text(), head: head.headers.get('content-length'),
              sandbox: whole.headers.get('content-security-policy'),
              odd: await Promise.all(rest.map(async f => (await (await fetch(f)).arrayBuffer()).byteLength)),
              empty: await fetch(empty).then(async r => [r.status, (await r.arrayBuffer()).byteLength]),
              ranges: [await range('bytes=-3'), await range('bytes=7-'), await range('bytes=99-'), await range('bytes=0-1,4-5'), await range('bytes=x')],
              post: await fetch(big, { method: 'POST', body: 'x' }).then(r => r.status, () => 'refused') });
            """)
        XCTAssertTrue((read["big"] as? String)?.hasPrefix("exact-app://app/.exact/files/") == true, "\(read)")
        XCTAssertEqual(read["keep"] as? String, "a b", "the other values stay as written")
        XCTAssertEqual(read["hash"] as? String, "#top")
        XCTAssertEqual(read["status"] as? Int, 200)
        XCTAssertEqual(read["bytes"] as? Int, Self.size)
        XCTAssertEqual(read["part"] as? Int, 206)
        XCTAssertEqual(read["contentRange"] as? String, "bytes 10-19/\(Self.size)")
        XCTAssertEqual(read["text"] as? String, "ABCDEFGHIJ")
        XCTAssertEqual(read["head"] as? String, String(Self.size))
        XCTAssertEqual(read["sandbox"] as? String, "sandbox allow-scripts allow-forms allow-modals allow-popups allow-downloads")
        XCTAssertEqual(read["odd"] as? [Int], Self.odd.map { Data($0.utf8).count }, "names a URL would split, encoded")
        XCTAssertEqual(read["empty"] as? [Int], [200, 0])
        let ranges = try XCTUnwrap(read["ranges"] as? [[Any]]).map { "\($0[0]) \($0[1])" }
        XCTAssertEqual(ranges, ["206 789", "206 789", "200 0123456789", "200 0123456789", "200 0123456789"])
        XCTAssertEqual(read["post"] as? Int, 405)
    }

    /// Any other value stays as written: the page loads, with no `error`.
    func testWhatCannotBeGivenStaysAsWritten() throws {
        let (session, node) = try frames(1)[0]
        defer { session.destroy() }
        for value in ["app:/data/secret.txt", "app:/cache/../data/secret.txt", "app:/cache/missing.bin", "app:/cache/dir",
                      "app:/cache/link.txt", "app:/cache/up/secret.txt", "app:/cache"] {
            for written in [value, encode(value)] {
                show(session, node, "assets/given.html?file=\(written)")
                XCTAssertEqual(try run(node, "return JSON.stringify({ file: new URLSearchParams(location.search).get('file') })")["file"] as? String, value, written)
            }
        }
        XCTAssertEqual(slot(session, "failed") as? String, "", "no error reached the app")
    }

    /// A token is the page's only name for its file, and lasts as long as the
    /// frame shows the source it was made for.
    func testAGivenFileIsItsFramesAloneAndEndsWithItsSource() throws {
        let both = try frames(2)
        defer { both.forEach { $0.0.destroy() } }
        let (session, node) = both[0], (other, otherNode) = both[1]
        show(session, node, "assets/given.html?file=app:/cache/big.bin")
        let file = try XCTUnwrap(try run(node, "return JSON.stringify({ file: new URLSearchParams(location.search).get('file') })")["file"] as? String)
        let status = { (node: NodeView, url: String) in try self.run(node, "return JSON.stringify({ status: await fetch(\(self.quoted(url))).then(r => r.status, () => 'refused') })")["status"] as? Int }
        XCTAssertEqual(try status(node, "exact-app://app/.exact/files/\(UUID().uuidString.lowercased())"), 404, "a guessed token")
        show(other, otherNode, "assets/given.html")
        XCTAssertEqual(try status(otherNode, file), 404, "another frame's page")
        // The root replaced by a link after the page was given the file: its
        // pinned root is read, never the link.
        try FileManager.default.moveItem(at: path("cache"), to: path("cache-was"))
        try FileManager.default.createSymbolicLink(at: path("cache"), withDestinationURL: path("elsewhere"))
        XCTAssertEqual(try run(node, "return JSON.stringify({ bytes: (await (await fetch(\(quoted(file)))).arrayBuffer()).byteLength })")["bytes"] as? Int, Self.size)
        // A new serve then finds a root that is a link, and gives nothing.
        show(session, node, "assets/given.html?file=app:/cache/big.bin&again=1")
        XCTAssertEqual(try run(node, "return JSON.stringify({ file: new URLSearchParams(location.search).get('file') })")["file"] as? String, "app:/cache/big.bin")
        try FileManager.default.removeItem(at: path("cache"))
        try FileManager.default.moveItem(at: path("cache-was"), to: path("cache"))
        // A new source ends the old one's files, as a new frame does.
        show(session, node, "assets/given.html?file=app:/tmp/note.html")
        XCTAssertEqual(try status(node, file), 404, "after a new source")
        session.webviews.destroy(id: node.id)
        let remade = try XCTUnwrap(session.webviews.create(owner: node))
        node.web = remade
        show(session, node, "assets/given.html")
        XCTAssertEqual(try status(node, file), 404, "after the frame was made again")
    }

    /// Every given document runs in an opaque origin, whatever its type; a
    /// type this Mac has no document for (`.xsl` may be one) is not framed,
    /// as framing it would download it.
    func testAGivenDocumentNeverRunsAsTheAppsOrigin() throws {
        let (session, node) = try frames(1)[0]
        defer { session.destroy() }
        show(session, node, "assets/given.html?" + Self.documents.map { "file=app:/cache/\($0)" }.joined(separator: "&"))
        let read = try run(node, """
            const origins = [];
            for (const file of new URLSearchParams(location.search).getAll('file')) {
              const response = await fetch(file), header = response.headers.get('content-security-policy');
              if (response.headers.get('content-type') === 'application/octet-stream') { origins.push('no document ' + header); continue; }
              const frame = document.createElement('iframe');
              const origin = await new Promise(resolve => {
                const heard = e => { if (e.source === frame.contentWindow) { removeEventListener('message', heard); resolve(String(e.data)); } };
                addEventListener('message', heard);
                setTimeout(() => resolve('no script ran'), 3000);
                frame.src = file;
                document.body.append(frame);
              });
              frame.remove();
              origins.push(origin + ' ' + header);
            }
            return JSON.stringify({ origins });
            """)
        let sandbox = "sandbox allow-scripts allow-forms allow-modals allow-popups allow-downloads"
        let origins = try XCTUnwrap(read["origins"] as? [String], "\(read)")
        XCTAssertEqual(origins.count, Self.documents.count)
        for (name, origin) in zip(Self.documents, origins) {
            XCTAssertTrue(origin == "origin=null \(sandbox)" || origin == "no document \(sandbox)", "\(name): \(origin)")
        }
        XCTAssertEqual(origins.filter { $0.hasPrefix("origin=null") }.count >= 4, true, "the HTML, SVG, XHTML and RSS documents ran, opaque: \(origins)")
    }

    /// A page in an opaque sandbox cannot read a given file; it can still
    /// show one, as an `img` loads across origins.
    func testAnOpaqueSandboxPageShowsButCannotReadAGivenFile() throws {
        let (session, node) = try frames(1)[0]
        defer { session.destroy() }
        node.props["sandbox"] = "allow-scripts"
        show(session, node, "assets/given.html?file=app:/cache/dot.png")
        let read = try run(node, """
            const file = new URLSearchParams(location.search).get('file');
            const img = await new Promise(r => { const i = new Image(); i.onload = () => r('load'); i.onerror = () => r('error'); i.src = file; });
            return JSON.stringify({ origin: self.origin, fetch: await fetch(file).then(r => r.status, () => 'refused'), img });
            """)
        XCTAssertEqual(read["origin"] as? String, "null")
        XCTAssertEqual(read["fetch"] as? String, "refused")
        XCTAssertEqual(read["img"] as? String, "load")
    }

    /// A page iOS wraps (no device-width viewport) is given its file too.
    func testAWrappedPageIsGivenItsFile() throws {
        let (session, node) = try frames(1)[0]
        defer { session.destroy() }
        show(session, node, "assets/wrapped.html?file=app:/cache/big.bin")
        #if os(iOS)
        XCTAssertEqual((node.web as? WKWebView)?.url?.path.hasPrefix("/.exact/frame/"), true, "iOS shows the page in its wrapper")
        #endif
        let end = Date(timeIntervalSinceNow: 30)
        while Date() < end, !said(session).contains("bytes") { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
        let posted = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(said(session).utf8)) as? [String: Any])
        XCTAssertEqual(posted["status"] as? Int, 200, "\(posted)")
        XCTAssertEqual(posted["bytes"] as? Int, Self.size)
    }

    /// Sessions each showing the fixture's one frame, the app's files at this
    /// test's roots: each boot learns its runtime's roots, so these come after.
    private func frames(_ count: Int) throws -> [(ExactSession, NodeView)] {
        guard let module = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let made = try (0..<count).map { n -> (ExactSession, NodeView) in
            let session = ExactApp.shared.makeSession(label: "given-files-\(n)")
            session.webviews.moduleLocation = { module }
            XCTAssertNil(session.boot(plan: try fixturePlan("WEB_DOCUMENTS"), size: CGSize(width: 300, height: 200)).error)
            return (session, try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "pdf" }))
        }
        AppFiles.learn(["roots": ["data": path("data").path, "cache": path("cache").path, "tmp": path("tmp").path]])
        return made
    }

    /// The frame's new page, loaded: a `load` its last document reports late
    /// (a refused first source's blank one) is not this page's.
    private func show(_ session: ExactSession, _ node: NodeView, _ src: String) {
        let before = (node.web as? WKWebView)?.url
        let loaded = expectation(description: src)
        loaded.assertForOverFulfill = false
        session.presenter.onLoad = { _ in loaded.fulfill() }
        node.props["src"] = src
        session.webviews.update(node)
        wait(for: [loaded], timeout: 30)
        session.presenter.onLoad = nil
        let end = Date(timeIntervalSinceNow: 30)
        while Date() < end {
            if let web = node.web as? WKWebView, web.url != before, web.url?.scheme == "exact-app", !web.isLoading { break }
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02))
        }
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

    private func slot(_ session: ExactSession, _ name: String) -> Any? {
        let state = (try? JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"state"}"#).utf8))) as? [String: Any]
        return (state?["slots"] as? [String: Any])?[name]
    }

    private func said(_ session: ExactSession) -> String { slot(session, "said") as? String ?? "" }

    /// As `encodeURIComponent` encodes it.
    private func encode(_ text: String) -> String {
        text.addingPercentEncoding(withAllowedCharacters: CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.!~*'()")) ?? text
    }

    private func quoted(_ text: String) -> String {
        String(decoding: try! JSONSerialization.data(withJSONObject: text, options: .fragmentsAllowed), as: UTF8.self)
    }
}
#endif
