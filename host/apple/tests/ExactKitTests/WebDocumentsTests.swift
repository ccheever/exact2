import XCTest
import PDFKit
import WebKit
import CoreGraphics
@testable import ExactKit
#if os(macOS)
import AppKit
#endif

#if os(macOS) || os(iOS)
final class WebDocumentsTests: XCTestCase {
    // scripts/fixtures/web-documents.contract: an iframe of an app file and its load and error actions.
    private func plan() throws -> Data { try fixturePlan("WEB_DOCUMENTS") }

    #if os(macOS)
    func testExplicitIframeFocusReachesTheWebViewAndRespectsInertAncestors() throws {
        _ = NSApplication.shared
        let presenter = Presenter()
        presenter.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "iframe", "props": ["id": "guest"]],
            ["op": "create", "id": 3, "kind": "input", "props": ["id": "outside", "value": "outside"]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 240],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 400, "h": 160],
            ["op": "frame", "id": 3, "x": 0, "y": 180, "w": 200, "h": 30],
        ]))
        let guest = try XCTUnwrap(presenter.views[2])
        let outside = try XCTUnwrap(presenter.views[3]?.field)
        let web = WKWebView(frame: guest.bounds)
        guest.addSubview(web)
        guest.web = web
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = presenter.viewport
        defer { window.close() }
        final class Loaded: NSObject, WKNavigationDelegate {
            let ready: XCTestExpectation
            init(_ ready: XCTestExpectation) { self.ready = ready }
            func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { ready.fulfill() }
        }
        let ready = expectation(description: "embedded document loaded")
        let loaded = Loaded(ready)
        web.navigationDelegate = loaded
        web.loadHTMLString("<input id='draft' value='retained'>", baseURL: nil)
        wait(for: [ready], timeout: 30)
        defer { web.navigationDelegate = nil }
        func webHasFocus() -> Bool {
            window.firstResponder === web || (window.firstResponder as? NSView)?.isDescendant(of: web) == true
        }
        XCTAssertTrue(window.makeFirstResponder(web), "the platform web view can receive focus")
        XCTAssertTrue(webHasFocus())
        presenter.focusElement(["outside"])
        XCTAssertTrue(window.firstResponder === outside.currentEditor())
        presenter.focusElement(["guest"])
        XCTAssertTrue(webHasFocus(), "focus(iframeId) must reach its embedded web view")
        presenter.focusElement(["outside"])
        presenter.apply(wireBatch([["op": "props", "id": 1, "set": ["inert": "true"]]]))
        let before = window.firstResponder
        presenter.focusElement(["guest"])
        XCTAssertTrue(window.firstResponder === before, "an inert ancestor refuses iframe focus")
        presenter.apply(wireBatch([["op": "props", "id": 1, "set": ["inert": "false"]]]))
        presenter.focusElement(["outside"])
        let editor = window.firstResponder
        presenter.focusElement(["guest"], selectText: true)
        XCTAssertTrue(window.firstResponder === editor, "selectText remains limited to text controls")
        presenter.focusElement(["guest"])
        XCTAssertTrue(webHasFocus())
    }
    #endif

    func testAppTextFilePreservesMIMEAndLiteralMarkup() throws {
        guard let path = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let session = ExactApp.shared.makeSession(label: "app-text-frame")
        session.webviews.moduleLocation = { path }
        defer { session.destroy() }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let literal = "<b>literal</b><script>window.executed = true</script>"
        try Data(literal.utf8).write(to: directory.appendingPathComponent("notes.txt"))
        XCTAssertNil(session.boot(plan: try plan(), size: CGSize(width: 300, height: 200)).error)
        AppFiles.learn(["roots": ["data": directory.path]])
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "pdf" })
        let loaded = expectation(description: "plain text document loaded")
        session.presenter.onLoad = { _ in loaded.fulfill() }
        node.props["sandbox"] = "allow-scripts"
        node.props["src"] = "app:/data/notes.txt"
        session.webviews.update(node)
        wait(for: [loaded], timeout: 30)
        let checked = expectation(description: "literal text, with no executed script")
        let web = try XCTUnwrap(node.web as? WKWebView)
        web.evaluateJavaScript("[document.contentType, document.body.innerText, window.executed === true]", in: nil, in: .page) { result in
            switch result {
            case .success(let value):
                let values = value as? [Any]
                XCTAssertEqual(values?[0] as? String, "text/plain")
                XCTAssertEqual(values?[1] as? String, literal)
                XCTAssertEqual(values?[2] as? Bool, false)
            case .failure(let error): XCTFail("\(error)")
            }
            checked.fulfill()
        }
        wait(for: [checked], timeout: 30)
    }

    func testAppFilePDFPixelsAndMissingFileError() throws {
        guard let path = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let session = ExactApp.shared.makeSession(label: "app-file-frame")
        session.webviews.moduleLocation = { path }
        defer { session.destroy() }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("doc.pdf")
        var bounds = CGRect(x: 0, y: 0, width: 300, height: 200)
        let pdf = try XCTUnwrap(CGContext(file as CFURL, mediaBox: &bounds, nil))
        pdf.beginPDFPage(nil)
        pdf.setFillColor(CGColor(red: 1, green: 0, blue: 0, alpha: 1))
        pdf.fill(CGRect(x: 40, y: 40, width: 220, height: 120))
        pdf.endPDFPage(); pdf.closePDF()
        XCTAssertNil(session.boot(plan: try plan(), size: bounds.size).error)
        // Boot learns the runtime's roots. This test owns one file root.
        AppFiles.learn(["roots": ["data": directory.path]])
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "pdf" })
        // The first read may already have started before this test supplied its root.
        node.props["src"] = "app:/data/doc.pdf#page=1"
        session.webviews.update(node)
        #if os(macOS)
        let window = NSWindow(contentRect: bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = session.presenter.viewport
        window.orderFrontRegardless()
        defer { window.close() }
        #endif
        let loaded = expectation(description: "the app file's PDF document")
        session.presenter.onLoad = { _ in loaded.fulfill() }
        wait(for: [loaded], timeout: 30)
        // iOS runs in hostless XCTest, with no UIApplication/window scene; its PDF captures are black.
        // The iOS app drive in issues/closed/20261009-app-file-iframe-loading.md qualifies pixels there.
        #if os(macOS)
        // An unsandboxed local PDF is PDFKit's view (LLP 1020.002 D10); the
        // agent's picture of the frame is the arm's in-process drawing of it.
        XCTAssertTrue(node.web is PDFView, "\(String(describing: node.web))")
        let cg = try XCTUnwrap(session.webviews.snapshots()[node.id]?.cgImage(forProposedRect: nil, context: nil, hints: nil), "no PDF snapshot")
        var pixels = [UInt8](repeating: 0, count: 100 * 100 * 4)
        let red = pixels.withUnsafeMutableBytes { bytes -> Int in
            let context = CGContext(data: bytes.baseAddress, width: 100, height: 100, bitsPerComponent: 8,
                bytesPerRow: 400, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            context.draw(cg, in: CGRect(x: 0, y: 0, width: 100, height: 100))
            return stride(from: 0, to: bytes.count, by: 4).filter { bytes[$0] > 200 && bytes[$0 + 1] < 60 && bytes[$0 + 2] < 60 }.count
        }
        XCTAssertGreaterThan(red, 100, "the PDF's red rectangle must be visible")
        #endif
        let failed = expectation(description: "missing file settles")
        node.props["src"] = "app:/data/missing.pdf"
        session.presenter.onLoad = { _ in XCTFail("a missing app file must not load") }
        session.webviews.update(node)
        let deadline = Date(timeIntervalSinceNow: 30)
        while Date() < deadline {
            let state = session.agent("{\"op\":\"state\"}")
            if state.contains("missing.pdf") { failed.fulfill(); break }
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05))
        }
        wait(for: [failed], timeout: 1)
    }
}
#endif
