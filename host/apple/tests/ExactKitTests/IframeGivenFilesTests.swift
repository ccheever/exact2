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
        try Data("inner".utf8).write(to: path("cache/dir/inner.txt"))
        try Data("not this".utf8).write(to: path("data/inner.txt"))
        let pdf = NSMutableData()
        var box = CGRect(x: 0, y: 0, width: 300, height: 300)
        let context = try XCTUnwrap(CGContext(consumer: CGDataConsumer(data: pdf as CFMutableData)!, mediaBox: &box, nil))
        context.beginPDFPage(nil)
        context.setFillColor(CGColor(red: 1, green: 0, blue: 0, alpha: 1))
        context.fill(box)
        context.endPDFPage()
        context.closePDF()
        try (pdf as Data).write(to: path("cache/red.pdf"))
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
              type: whole.headers.get('content-type'), sandbox: [whole, part, head].map(r => r.headers.get('content-security-policy')),
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
        XCTAssertEqual(read["type"] as? String, "application/octet-stream")
        XCTAssertEqual(read["sandbox"] as? [String], Array(repeating: "sandbox allow-scripts allow-forms allow-modals allow-popups allow-downloads", count: 3), "200, 206 and HEAD")
        XCTAssertEqual(read["odd"] as? [Int], Self.odd.map { Data($0.utf8).count }, "names a URL would split, encoded")
        XCTAssertEqual(read["empty"] as? [Int], [200, 0])
        let ranges = try XCTUnwrap(read["ranges"] as? [[Any]]).map { "\($0[0]) \($0[1])" }
        XCTAssertEqual(ranges, ["206 789", "206 789", "200 0123456789", "200 0123456789", "200 0123456789"])
        XCTAssertEqual(read["post"] as? Int, 405)
    }

    /// Any other `app:/` value is emptied: the page loads, with no `error`.
    func testWhatCannotBeGivenIsEmptied() throws {
        let (session, node) = try frames(1)[0]
        defer { session.destroy() }
        var n = 0
        for value in ["app:/data/secret.txt", "app:/cache/../data/secret.txt", "app:/cache/missing.bin", "app:/cache/dir",
                      "app:/cache/link.txt", "app:/cache/up/secret.txt", "app:/cache"] {
            for written in [value, encode(value)] {
                n += 1
                show(session, node, "assets/given.html?file=\(written)&n=\(n)")
                XCTAssertEqual(try run(node, "return JSON.stringify({ search: location.search })")["search"] as? String, "?file=&n=\(n)", written)
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
        show(session, node, "assets/given.html?file=app:/cache/big.bin&file=app:/cache/digits.txt&file=app:/cache/dir/inner.txt")
        let files = try XCTUnwrap(try run(node, "return JSON.stringify({ files: new URLSearchParams(location.search).getAll('file') })")["files"] as? [String])
        let file = files[0]
        let status = { (node: NodeView, url: String) in try self.run(node, "return JSON.stringify({ status: await fetch(\(self.quoted(url)), { cache: 'no-store' }).then(r => r.status, () => 'refused') })")["status"] as? Int }
        XCTAssertEqual(try status(node, "exact-app://app/.exact/files/\(UUID().uuidString.lowercased())"), 404, "a guessed token")
        show(other, otherNode, "assets/given.html")
        XCTAssertEqual(try status(otherNode, file), 404, "another frame's page")
        // A file, and a folder on its way, replaced by links after the page
        // was given them: each request opens its path again, following none.
        try FileManager.default.removeItem(at: path("cache/digits.txt"))
        try FileManager.default.createSymbolicLink(at: path("cache/digits.txt"), withDestinationURL: path("data/secret.txt"))
        XCTAssertEqual(try status(node, files[1]), 404, "a file replaced by a link")
        try FileManager.default.moveItem(at: path("cache/dir"), to: path("cache/dir-was"))
        try FileManager.default.createSymbolicLink(at: path("cache/dir"), withDestinationURL: path("data"))
        XCTAssertEqual(try status(node, files[2]), 404, "a folder on its way replaced by a link")
        try FileManager.default.removeItem(at: path("cache/dir"))
        try FileManager.default.moveItem(at: path("cache/dir-was"), to: path("cache/dir"))
        XCTAssertEqual(try status(node, files[2]), 200, "the folder back")
        // The root replaced by a link after the page was given the file: its
        // pinned root is read, never the link.
        try FileManager.default.moveItem(at: path("cache"), to: path("cache-was"))
        try FileManager.default.createSymbolicLink(at: path("cache"), withDestinationURL: path("elsewhere"))
        XCTAssertEqual(try run(node, "return JSON.stringify({ bytes: (await (await fetch(\(quoted(file)))).arrayBuffer()).byteLength })")["bytes"] as? Int, Self.size)
        // A new serve then finds a root that is a link, and gives nothing.
        show(session, node, "assets/given.html?file=app:/cache/big.bin&again=1")
        XCTAssertEqual(try run(node, "return JSON.stringify({ search: location.search })")["search"] as? String, "?file=&again=1")
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

    /// A file a browser would run as a document is given as bytes, so it
    /// never runs as the app; every response but a PDF's carries the sandbox
    /// as well.
    func testAGivenFileIsTypedSoItCannotRunAsTheApp() throws {
        let (session, node) = try frames(1)[0]
        defer { session.destroy() }
        let names = Self.documents + ["dot.png", "red.pdf"]
        show(session, node, "assets/given.html?" + names.map { "file=app:/cache/\($0)" }.joined(separator: "&"))
        let read = try run(node, """
            const out = [];
            for (const file of new URLSearchParams(location.search).getAll('file')) {
              const r = await fetch(file);
              out.push(r.headers.get('content-type') + ' ' + r.headers.get('content-security-policy'));
            }
            return JSON.stringify({ out });
            """)
        let sandbox = "sandbox allow-scripts allow-forms allow-modals allow-popups allow-downloads"
        XCTAssertEqual(read["out"] as? [String], Self.documents.map { _ in "application/octet-stream \(sandbox)" } + ["image/png \(sandbox)", "application/pdf null"], "\(read)")
    }

    /// A page that frames a given PDF shows it: WebKit's PDF view draws.
    func testAFramedGivenPDFDraws() throws {
        let (session, node) = try frames(1)[0]
        defer { session.destroy() }
        #if os(macOS)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = session.presenter.viewport
        window.orderFrontRegardless()
        defer { window.close() }
        #endif
        show(session, node, "assets/given.html?file=app:/cache/red.pdf")
        let read = try run(node, """
            document.body.style.margin = '0';
            document.body.replaceChildren();
            const frame = document.createElement('iframe');
            frame.style.cssText = 'border:0;width:300px;height:200px';
            await new Promise(resolve => { frame.onload = resolve; frame.src = new URLSearchParams(location.search).get('file'); document.body.append(frame); });
            await new Promise(resolve => setTimeout(resolve, 1500));
            const shown = frame.contentDocument;
            // macOS shows it in WebKit's PDF view (an <embed>); iOS as an image of its pages.
            return JSON.stringify({ type: shown.contentType, viewer: !!shown.querySelector('embed') || (shown.querySelector('img')?.naturalWidth ?? 0) > 0 });
            """)
        XCTAssertEqual(read["type"] as? String, "application/pdf", "\(read)")
        XCTAssertEqual(read["viewer"] as? Bool, true, "WebKit shows the PDF in the frame: \(read)")
        #if os(macOS)
        // The picture: the PDF's red page fills the frame (it drew nothing under the sandbox).
        let image = try XCTUnwrap(session.webviews.snapshots()[node.id]?.cgImage(forProposedRect: nil, context: nil, hints: nil), "no snapshot")
        var pixels = [UInt8](repeating: 0, count: 100 * 100 * 4)
        let red = pixels.withUnsafeMutableBytes { bytes -> Int in
            let context = CGContext(data: bytes.baseAddress, width: 100, height: 100, bitsPerComponent: 8, bytesPerRow: 400,
                                    space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            context.draw(image, in: CGRect(x: 0, y: 0, width: 100, height: 100))
            return stride(from: 0, to: bytes.count, by: 4).filter { bytes[$0] > 200 && bytes[$0 + 1] < 60 && bytes[$0 + 2] < 60 }.count
        }
        XCTAssertGreaterThan(red, 3000, "the PDF's red page")
        #endif
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
