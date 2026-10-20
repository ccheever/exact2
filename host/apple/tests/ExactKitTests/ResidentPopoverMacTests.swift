import XCTest
import WebKit
@testable import ExactKit
#if os(macOS)
import AppKit

/// `popover=(full ? some("manual") : none)` moves one mounted box into the
/// manual top layer and back (LLP 1021 D2, "Removing the attribute").
final class ResidentPopoverMacTests: XCTestCase {
    func testOptionalManualPopoverPreservesTheMountedGuestAndRestoresItsFrame() throws {
        guard let module = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        _ = NSApplication.shared
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let html = "<!doctype html><body>Resident<script>window.residentValue='kept';window.residentDocumentToken=Array.from(crypto.getRandomValues(new Uint32Array(4))).join('-');parent.postMessage('resident-ready','*');</script>"
        try Data(html.utf8).write(to: directory.appendingPathComponent("guest.html"))
        let session = ExactApp.shared.makeSession(label: "resident-iframe")
        session.webviews.moduleLocation = { module }
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try fixturePlan("IFRAME_RESIDENT_POPOVER"), size: CGSize(width: 640, height: 500)).error)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 500), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        window.orderFrontRegardless()
        defer { window.close() }
        // First-pixel activation learns the runtime's roots; the fixture's root goes in after it.
        let activationDeadline = Date(timeIntervalSinceNow: 30)
        while !session.dataActivated && Date() < activationDeadline {
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05))
        }
        XCTAssertTrue(session.dataActivated, "storage activation must finish before installing the fixture root")
        guard session.dataActivated else { return }
        AppFiles.learn(["roots": ["data": directory.path]])
        let resident = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "resident" })
        let guest = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "resident-guest" })
        let button = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "resident-toggle" })
        let loaded = expectation(description: "resident document loaded")
        let original = session.presenter.onMessage
        session.presenter.onMessage = { id, text in
            original?(id, text)
            if id == guest.id && text == "resident-ready" { loaded.fulfill() }
        }
        guest.props["src"] = "app:/data/guest.html"
        session.webviews.update(guest)
        wait(for: [loaded], timeout: 30)
        let web = try XCTUnwrap(guest.web as? WKWebView)
        var documentToken: String?
        func read(_ expected: String, prefix: String = "", done: XCTestExpectation) {
            web.evaluateJavaScript(prefix + "JSON.stringify({value:window.residentValue,token:window.residentDocumentToken})", in: nil, in: .page) { result in
                switch result {
                case .success(let value):
                    guard let text = value as? String, let data = text.data(using: .utf8),
                          let fields = try? JSONSerialization.jsonObject(with: data) as? [String: String],
                          let token = fields["token"], !token.isEmpty else { XCTFail("missing resident document identity"); done.fulfill(); return }
                    XCTAssertEqual(fields["value"], expected)
                    if let documentToken { XCTAssertEqual(token, documentToken) } else { documentToken = token }
                case .failure(let error): XCTFail("\(error)")
                }
                done.fulfill()
            }
        }
        let ready = expectation(description: "resident document value")
        read("kept", done: ready)
        wait(for: [ready], timeout: 30)
        let changed = expectation(description: "edit the mounted document's local value")
        read("edited", prefix: "window.residentValue='edited';", done: changed)
        wait(for: [changed], timeout: 30)
        let parent = try XCTUnwrap(resident.superview), inline = resident.frame
        session.presenter.press(button.id)
        XCTAssertTrue(session.presenter.menus.isOpen(resident))
        XCTAssertTrue(guest.web === web)
        let full = resident.convert(resident.bounds, to: session.presenter.viewport)
        XCTAssertEqual(full.origin.x, 0, accuracy: 1)
        XCTAssertEqual(full.origin.y, 0, accuracy: 1)
        XCTAssertEqual(full.width, 640, accuracy: 1)
        XCTAssertEqual(full.height, 500, accuracy: 1)
        let expanded = expectation(description: "same document in the top layer")
        read("edited", done: expanded)
        wait(for: [expanded], timeout: 30)
        session.presenter.press(button.id)
        XCTAssertFalse(session.presenter.menus.isOpen(resident))
        XCTAssertNil(resident.props["popover"])
        XCTAssertTrue(resident.superview === parent)
        XCTAssertEqual(resident.frame, inline)
        XCTAssertTrue(guest.web === web)
        let restored = expectation(description: "same document restored inline")
        read("edited", done: restored)
        wait(for: [restored], timeout: 30)
    }
}
#endif
