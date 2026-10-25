import PDFKit
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
        session.presenter.press(try XCTUnwrap(node("show")).id)
        wait(15) { node("doc")?.web is PDFView }
        let frame = try XCTUnwrap(node("doc"))
        let pdf = try XCTUnwrap(frame.web as? PDFView, "the node shows a PDF view: \(String(describing: frame.web))")
        XCTAssertEqual(pdf.document?.pageCount, 3)
        XCTAssertTrue(ElementHatches.platform(of: frame, session.presenter) === pdf, "the hatch's platform object is the PDF view")

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
    }
}
#endif
