import PDFKit
import WebKit
import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#endif

#if os(macOS)
/// scripts/fixtures/iframe-pdf.contract: an `app:/` PDF frame is a PDF view
/// in the node, its hatch's platform object, and `tree` shows its pages; a
/// capture from an action writes the file and its `FrameCapture` reaches
/// the action (LLP 1020.002 D7, D10, §9).
final class IframeCaptureSessionTests: XCTestCase {
    func testAPDFFramesViewHatchTreeAndCapture() throws {
        guard let module = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "iframe-pdf")
        session.webviews.moduleLocation = { module }
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try fixturePlan("IFRAME_PDF"), size: CGSize(width: 640, height: 500)).error)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 500), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        window.orderFrontRegardless()
        defer { window.close() }
        let wait = { (seconds: TimeInterval, done: () -> Bool) in
            let end = Date(timeIntervalSinceNow: seconds)
            while !done() && Date() < end { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
        }
        let node = { (testId: String) in session.presenter.views.values.first { $0.props["testId"] == testId } }

        // Three pages, written where the frame's `src` names before it mounts.
        let file = try XCTUnwrap(AppFiles.url("app:/tmp/b6.pdf"))
        try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        let document = PDFDocument()
        for n in 0..<3 {
            let page = PDFPage()
            page.setBounds(CGRect(x: 0, y: 0, width: 612, height: 792), for: .mediaBox)
            document.insert(page, at: n)
        }
        XCTAssertTrue(document.write(to: file))
        // A hatch's moments, counted by the host before it calls the app's
        // module; no module is loaded here, so none is called.
        session.natives.hatchesConnected = true
        session.presenter.press(try XCTUnwrap(node("show")).id)
        let mounted = try XCTUnwrap(node("doc"))
        // A missing local PDF starts on the PDF's view, as named (its read fails later).
        XCTAssertTrue(try XCTUnwrap(node("gone")).web is PDFView, "\(String(describing: node("gone")?.web))")
        // A sandboxed local PDF stays WebKit's: a web view from its mount, never a PDF's view first (D10).
        XCTAssertTrue(try XCTUnwrap(node("boxed")).web is WKWebView, "\(String(describing: node("boxed")?.web))")
        mounted.props["hatch"] = "frame"
        session.presenter.elements.created(mounted)
        let calls = { () -> [String: Int] in
            let words = session.presenter.elements.observation(nil)["words"] as? [String: Any]
            return (words?["frame"] as? [String: Any])?["calls"] as? [String: Int] ?? [:]
        }
        XCTAssertEqual(calls()["built"], 1, "\(calls())")
        // The PDF's view from the mount (no web view first), its pages once the file came.
        wait(15) { (node("doc")?.web as? PDFView)?.document?.pageCount == 3 }
        let frame = try XCTUnwrap(node("doc"))
        let pdf = try XCTUnwrap(frame.web as? PDFView, "the node shows a PDF view: \(String(describing: frame.web))")
        XCTAssertEqual(pdf.document?.pageCount, 3)
        XCTAssertTrue(ElementHatches.platform(of: frame, session.presenter) === pdf, "the hatch's platform object is the PDF view")
        XCTAssertNil(calls()["changed"], "the hatch was built with the PDF view, which no other replaced: \(calls())")

        // A frame whose PDF is not there starts on the PDF's view, as named, and gets a web
        // view in its place when the read fails: that one is shown, in the window.
        wait(15) { (node("gone")?.web as? WKWebView)?.window != nil }
        XCTAssertNotNil((try XCTUnwrap(node("gone")).web as? WKWebView)?.window, "the failed PDF's web view, shown: \(String(describing: node("gone")?.web))")

        let tree = session.webviews.tree()
        let shown = (tree["nodes"] as? [[String: Any]])?.first { ($0["id"] as? NSNumber)?.uint32Value == frame.id }
        XCTAssertEqual(shown?["pdf"] as? Bool, true, "\(String(describing: shown))")
        XCTAssertEqual(shown?["pages"] as? Int, 3)

        let shot = try XCTUnwrap(AppFiles.url("app:/tmp/pdf-shot.png"))
        try? FileManager.default.removeItem(at: shot)
        session.presenter.press(try XCTUnwrap(node("snap")).id)
        let text = { node("shot")?.props["text"] ?? "" }
        wait(15) { !text().isEmpty }
        let scale = Int(window.backingScaleFactor)
        XCTAssertEqual(text(), "app:/tmp/pdf-shot.png \(200 * scale)x\(100 * scale) ", text())
        XCTAssertTrue(FileManager.default.fileExists(atPath: shot.path))

        // HTML again: a new web view, the hatch's platform object, and `changed` again.
        let page = try XCTUnwrap(AppFiles.url("app:/tmp/b6.html"))
        try Data("<!doctype html><title>b6</title><p>b6".utf8).write(to: page)
        frame.props["src"] = "app:/tmp/b6.html"
        session.webviews.update(frame)
        wait(15) { frame.web is WKWebView }
        let web = try XCTUnwrap(frame.web as? WKWebView, "\(String(describing: frame.web))")
        XCTAssertTrue(ElementHatches.platform(of: frame, session.presenter) === web, "the hatch's platform object is the web view")
        XCTAssertEqual(calls()["changed"], 1, "the one replacement, PDF to HTML: \(calls())")
    }
}
#endif
