import XCTest
import WebKit
import CoreGraphics
@testable import ExactKit
#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

#if os(macOS) || os(iOS)
final class WebDocumentsTests: XCTestCase {
    // component App
    //   state loaded = 0
    //   state failed = ""
    //   action didLoad
    //     loaded = loaded + 1
    //   action didFail(message: string)
    //     failed = message
    //   view
    //     iframe src="app:/data/doc.pdf" width=300 height=200 load=didLoad error=didFail testId="pdf"
    private let plan = Data(base64Encoded: "RVhQTAUAAAD91h+9C+HuvDSjEgztc+xGGA1E2ggCJPIAAAAA//////////8KAAAABgAAAG51bWJlcgYAAABsb2FkZWQGAAAAc3RyaW5nBgAAAGZhaWxlZAcAAABkaWRMb2FkBwAAAG1lc3NhZ2UHAAAAZGlkRmFpbAAAAAARAAAAYXBwOi9kYXRhL2RvYy5wZGYDAAAAcGRmUAAAAAQoAAAAAAAAAAAAKAIHAAAAKAYAAAAAAAAAAAAAAPA/DyIAAAAAKAkAACIBAAAAKAIIAAAAKAAAAAAAAMByQCgAAAAAAAAAaUAoAgkAAAAoAAAAAAIAAAAA/////wAAAAAAAAAAAAAAAAL/////AAAAAAAAAAACAAAAAAAAAAAAAAAAAAAACAAAAAAAAAABAAAAAQAAAAEAAAACAAAAAQAAAAMAAAABAAAABAAAAAEAAAAFAAAAAQAAAAYAAAABAAAABwAAAAEAAAAIAAAAAf////8C/////wP/////BP////8F/////wb/////B/////8I/////wIAAAABAAAAAAAAAAIAAAAKAAAA/////wAAAwAAAAEAAAAMAAAABgAAAP////8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACAAAABAAAAAAAAAAAAAAAAAAAAAEAAAASAAAAFQAAAAYAAAAAAAAAAQAAAAEAAAABAAAAJwAAAAkAAAABAAAABQAAAAEAAAACAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEAAAAL//////////8AAAAAAAAAAAQAAAAAAAAAAgAAAP////8EAAAAACoAMAAAAAYAAAABAQAANgAAAAoAAAABAQEAQAAAAAoAAAABAAsASgAAAAYAAAABAgAAAAcAAAAAAAAAAAAAAAAgAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==")!

    func testAppTextFilePreservesMIMEAndLiteralMarkup() throws {
        guard let path = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw XCTSkip("name a built WebKit arm with EXACT_TEST_WEB_MODULE")
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
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 300, height: 200)).error)
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
            throw XCTSkip("name a built WebKit arm with EXACT_TEST_WEB_MODULE")
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
        XCTAssertNil(session.boot(plan: plan, size: bounds.size).error)
        // Boot learns the runtime's roots. This test owns one file root.
        AppFiles.learn(["roots": ["data": directory.path]])
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "pdf" })
        // The first read may already have started before this test supplied its root.
        node.props["src"] = "app:/data/doc.pdf#page=1"
        session.webviews.update(node)
        let web = try XCTUnwrap(node.web as? WKWebView)
        #if os(macOS)
        let window = NSWindow(contentRect: bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = session.presenter.viewport
        window.orderFrontRegardless()
        defer { window.close() }
        #else
        let window = UIWindow(frame: bounds)
        window.addSubview(session.presenter.viewport)
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        #endif
        let loaded = expectation(description: "the app file's PDF document")
        session.presenter.onLoad = { _ in loaded.fulfill() }
        wait(for: [loaded], timeout: 30)
        let snapshot = expectation(description: "PDF pixels")
        let config = WKSnapshotConfiguration()
        config.snapshotWidth = 300
        config.afterScreenUpdates = true
        web.takeSnapshot(with: config) { image, error in
            XCTAssertNil(error)
            #if os(macOS)
            let cg = image?.cgImage(forProposedRect: nil, context: nil, hints: nil)
            #else
            // Match AgentIOS.screenshot: flush WebKit, then capture its remote layer through UIKit.
            let rendered = UIGraphicsImageRenderer(bounds: web.bounds).image { _ in
                web.drawHierarchy(in: web.bounds, afterScreenUpdates: true)
            }
            let cg = rendered.cgImage
            let attachment = XCTAttachment(image: rendered)
            attachment.lifetime = .keepAlways
            self.add(attachment)
            #endif
            if let cg {
                var pixels = [UInt8](repeating: 0, count: 100 * 100 * 4)
                let red = pixels.withUnsafeMutableBytes { bytes -> Int in
                    let context = CGContext(data: bytes.baseAddress, width: 100, height: 100, bitsPerComponent: 8,
                        bytesPerRow: 400, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
                    context.draw(cg, in: CGRect(x: 0, y: 0, width: 100, height: 100))
                    return stride(from: 0, to: bytes.count, by: 4).filter { bytes[$0] > 200 && bytes[$0 + 1] < 60 && bytes[$0 + 2] < 60 }.count
                }
                XCTAssertGreaterThan(red, 100, "the PDF's red rectangle must be visible")
            } else { XCTFail("no PDF snapshot") }
            snapshot.fulfill()
        }
        wait(for: [snapshot], timeout: 30)
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
