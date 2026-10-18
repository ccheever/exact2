import XCTest
import WebKit
@testable import ExactKit
#if os(macOS)
import AppKit
#endif

#if os(macOS) || os(iOS)
final class IframePostMessageTests: XCTestCase {
    func testOrderedRepliesStayWithTheAddressedDocument() throws {
        guard let module = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let html = """
        <!doctype html><meta name="viewport" content="width=device-width,initial-scale=1">
        <title>FRAME</title><body>Guest<script>
        window.received=[];
        addEventListener('message', e => { received.push(e.data); parent.postMessage(JSON.stringify({frame:document.title,text:e.data}),'*'); });
        parent.postMessage(JSON.stringify({frame:document.title,text:'ready'}),'*');
        </script>
        """
        for name in ["guest", "sibling", "replacement"] {
            try Data(html.replacingOccurrences(of: "FRAME", with: name).utf8).write(to: directory.appendingPathComponent("\(name).html"))
        }
        let session = ExactApp.shared.makeSession(label: "iframe-replies")
        session.webviews.moduleLocation = { module }
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try fixturePlan("IFRAME_POST_MESSAGES"), size: CGSize(width: 640, height: 500)).error)
        AppFiles.learn(["roots": ["data": directory.path]])
        let guest = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "guest" })
        let sibling = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "sibling" })
        #if os(macOS)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 500), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        window.orderFrontRegardless()
        defer { window.close() }
        #endif
        let ready = expectation(description: "both committed guest documents are ready")
        ready.expectedFulfillmentCount = 2
        let replies = expectation(description: "two ordered replies from the addressed guest")
        replies.expectedFulfillmentCount = 2
        var observed: [[String: String]] = []
        let original = session.presenter.onMessage
        session.presenter.onMessage = { id, text in
            original?(id, text)
            guard let bytes = text.data(using: .utf8), let value = try? JSONSerialization.jsonObject(with: bytes) as? [String: String] else { return }
            if value["text"] == "ready" { ready.fulfill() }
            else { observed.append(value); replies.fulfill() }
        }
        for (node, name) in [(guest, "guest"), (sibling, "sibling")] {
            node.props["sandbox"] = "allow-scripts"
            node.props["src"] = "app:/data/\(name).html"
            session.webviews.update(node)
        }
        wait(for: [ready], timeout: 30)
        let button = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "reply" })
        session.presenter.press(button.id)
        wait(for: [replies], timeout: 30)
        XCTAssertEqual(observed, [["frame": "guest", "text": "one"], ["frame": "guest", "text": "two"]])
        let reset = expectation(description: "replacement document ready")
        session.presenter.onMessage = { id, text in
            original?(id, text)
            if text.contains("ready") { reset.fulfill() }
        }
        guest.props["src"] = "app:/data/replacement.html"
        session.webviews.update(guest)
        session.webviews.postMessage("guest", "must-drop-during-replacement")
        wait(for: [reset], timeout: 30)
        let checked = expectation(description: "new document has no inherited posts")
        let web = try XCTUnwrap(guest.web as? WKWebView)
        web.evaluateJavaScript("JSON.stringify(window.received)", in: nil, in: .page) { result in
            switch result {
            case .success(let value): XCTAssertEqual(value as? String, "[]")
            case .failure(let error): XCTFail("\(error)")
            }
            checked.fulfill()
        }
        wait(for: [checked], timeout: 30)
    }

    #if os(macOS)
    func testOptionalManualPopoverPreservesTheMountedGuestAndRestoresItsFrame() throws {
        guard let module = ProcessInfo.processInfo.environment["EXACT_TEST_WEB_MODULE"] else {
            throw missing("module", "no WebKit arm: name a built one with EXACT_TEST_WEB_MODULE")
        }
        _ = NSApplication.shared
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let html = "<!doctype html><body>Resident<script>window.residentValue='kept';parent.postMessage('resident-ready','*');</script>"
        try Data(html.utf8).write(to: directory.appendingPathComponent("guest.html"))
        let session = ExactApp.shared.makeSession(label: "resident-iframe")
        session.webviews.moduleLocation = { module }
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try fixturePlan("IFRAME_RESIDENT_POPOVER"), size: CGSize(width: 640, height: 500)).error)
        AppFiles.learn(["roots": ["data": directory.path]])
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 500), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        window.orderFrontRegardless()
        defer { window.close() }
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
        let ready = expectation(description: "resident document value")
        let web = try XCTUnwrap(guest.web as? WKWebView)
        func read(_ expected: String, done: XCTestExpectation) {
            web.evaluateJavaScript("window.residentValue", in: nil, in: .page) { result in
                switch result {
                case .success(let value): XCTAssertEqual(value as? String, expected)
                case .failure(let error): XCTFail("\(error)")
                }
                done.fulfill()
            }
        }
        read("kept", done: ready)
        wait(for: [ready], timeout: 30)
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
        read("kept", done: expanded)
        wait(for: [expanded], timeout: 30)
        session.presenter.press(button.id)
        XCTAssertFalse(session.presenter.menus.isOpen(resident))
        XCTAssertNil(resident.props["popover"])
        XCTAssertTrue(resident.superview === parent)
        XCTAssertEqual(resident.frame, inline)
        XCTAssertTrue(guest.web === web)
        let restored = expectation(description: "same document restored inline")
        read("kept", done: restored)
        wait(for: [restored], timeout: 30)
    }
    #endif

}
#endif
