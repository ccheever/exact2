#if os(macOS)
import Foundation
import AppKit
import ObjectiveC
import XCTest
import UserNotifications
@testable import ExactKit

private final class StubNotification: UNNotification {
    static var requestValue: UNNotificationRequest!
    static var dateValue = Date()
    override var request: UNNotificationRequest { Self.requestValue }
    override var date: Date { Self.dateValue }
}

private final class StubNotificationResponse: UNNotificationResponse {
    static var notificationValue: UNNotification!
    static var actionValue = UNNotificationDefaultActionIdentifier
    override var notification: UNNotification { Self.notificationValue }
    override var actionIdentifier: String { Self.actionValue }
}

/// A launch URL an app cannot hear is journaled, as a running one is (#104).
final class LaunchURLMacTests: XCTestCase {
    private let handlers = [["protocol": "exacttest", "url": "/open?u=%s"]]

    private func respond(action: String, id: String, date: Date, info: [AnyHashable: Any],
                         completion: @escaping () -> Void) {
        let content = UNMutableNotificationContent()
        content.userInfo = info
        StubNotification.requestValue = UNNotificationRequest(identifier: id, content: content, trigger: nil)
        StubNotification.dateValue = date
        StubNotificationResponse.notificationValue = class_createInstance(StubNotification.self, 0) as? StubNotification
        StubNotificationResponse.actionValue = action
        let response = class_createInstance(StubNotificationResponse.self, 0) as! StubNotificationResponse
        let center = class_createInstance(UNUserNotificationCenter.self, 0) as! UNUserNotificationCenter
        NotificationPresenter.shared.userNotificationCenter(center, didReceive: response, withCompletionHandler: completion)
    }

    func testNotificationInstallerDoesNotOpenTheCenterInAnUnbundledHost() {
        guard Bundle.main.bundleURL.pathExtension != "app" else { return }
        let presenter = NotificationPresenter.shared
        defer { presenter.app = nil; presenter.activate = nil }
        presenter.install(on: ExactApp.shared, activate: nil)
        XCTAssertTrue(presenter.app === ExactApp.shared)
    }

    private func plan(_ source: String) throws -> Data {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    private func logs(launching url: String?, _ source: String) throws -> [String] {
        let bytes = try plan(source)
        let session = ExactApp.shared.makeSession()
        defer { session.destroy() }
        if let url { XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: url)))) }
        XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 390, height: 844)).error)
        let reply = try JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"logs\"}").utf8)) as? [String: Any]
        return try XCTUnwrap(reply?["lines"] as? [String])
    }

    private let rootless = "component App\n  view\n    main\n      text \"hi\"\n"
    private let routed = """
        routes nav
          home "/"
          notfound

        component App
          state got = "none"
          action back
            nav = back(nav)
          action follow(location: string)
            got = location
          view
            main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow
              for e in stack(nav) key=e.id
                column navigationKey=`${e.id}`
                  text `${e.name} ${e.url}`
                  text got

        """

    /// A launch no link started opens at the manifest's `start_url`; one a
    /// link started opens where the link goes.
    func testALaunchWithNoLinkOpensAtTheStartLocation() throws {
        let bytes = try plan("""
            routes nav
              home "/"
              today "/today"
              opened "/open"
              notfound

            component App
              action back
                nav = back(nav)
              view
                main navigationKey=`${top(nav).id}` navigationBack="back"
                  text `at ${top(nav).url}`
                  for e in stack(nav) key=e.id
                    column navigationKey=`${e.id}`
                      text `${e.name}`

            """)
        let was = ExactSession.startLocation
        ExactSession.startLocation = "/today"
        defer { ExactSession.startLocation = was }
        for (link, at) in [(nil, "at /today"), ("exacttest://auth/x", "at /open?u=exacttest%3A%2F%2Fauth%2Fx")] {
            let session = ExactApp.shared.makeSession()
            session.protocolHandlers = handlers
            defer { session.destroy() }
            if let link { XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: link)))) }
            XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 390, height: 844)).error)
            XCTAssertTrue(session.agent(#"{"op":"tree"}"#).contains("\"\(at)\""), "\(link ?? "no link"): \(session.agent(#"{"op":"tree"}"#).prefix(600))")
        }
    }

    func testALaunchURLWithNoNavigationRootIsJournaled() throws {
        let lines = try logs(launching: "exacttest://auth/x?request=1", rootless)
        XCTAssertTrue(lines.contains { $0.hasSuffix("launch URL refused: no navigation root handler (/auth/x?request=1)") }, "\(lines)")
    }

    func testNoLaunchURLOrANavigationRootJournalsNothing() throws {
        for lines in [try logs(launching: nil, rootless), try logs(launching: "exacttest://auth/x?request=1", routed)] {
            XCTAssertFalse(lines.contains { $0.contains("launch URL refused") }, "\(lines)")
        }
    }

    func testDeclaredProtocolArrivesWholeAtLaunchAndInWarmOrder() throws {
        let source = """
            routes nav
              home "/"
              opened "/open"
              notfound

            component App
              state seen = ""
              action back
                nav = back(nav)
              action follow(location: string)
                seen = `${seen}|${location}`
              view
                main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow
                  text `${top(nav).url}` testId="where"
                  text `${seen}` testId="seen"
                  for e in stack(nav) key=e.id
                    column navigationKey=`${e.id}`
                      text `${e.name}`

            """
        let bytes = try plan(source)
        let session = ExactApp.shared.makeSession()
        session.protocolHandlers = handlers
        defer { session.destroy() }
        let cold = "exacttest://auth/x?request=1#frag"
        let encodedCold = "/open?u=exacttest%3A%2F%2Fauth%2Fx%3Frequest%3D1%23frag"
        XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: cold))))
        XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 390, height: 844)).error)
        XCTAssertTrue(session.agent(#"{"op":"tree"}"#).contains(encodedCold))

        let first = "exacttest://auth/first#one"
        let second = "exacttest://auth/second#two"
        XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: first))))
        XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: second))))
        let state = session.agent(#"{"op":"state"}"#)
        let a = "/open?u=exacttest%3A%2F%2Fauth%2Ffirst%23one"
        let b = "/open?u=exacttest%3A%2F%2Fauth%2Fsecond%23two"
        XCTAssertTrue(state.contains("\(a)|\(b)"), state)

        let long = "exacttest://auth/x?request=\(String(repeating: "x", count: 16_384))#frag"
        XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: long))))
        XCTAssertTrue(session.agent(#"{"op":"state"}"#).contains("request%3D\(String(repeating: "x", count: 16_384))%23frag"))
    }

    func testTwoColdProtocolLinksKeepTheFirstLaunchAndDeliverTheSecond() throws {
        let source = """
            routes nav
              home "/"
              opened "/open"
              notfound

            component App
              state seen = ""
              action back
                nav = back(nav)
              action follow(location: string)
                seen = `${seen}|${location}`
              view
                main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow
                  text `${top(nav).url}` testId="where"
                  text `${seen}` testId="seen"
                  for e in stack(nav) key=e.id
                    column navigationKey=`${e.id}`

            """
        let session = ExactApp.shared.makeSession()
        session.protocolHandlers = handlers
        defer { session.destroy() }
        XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: "exacttest://auth/first#one"))))
        XCTAssertTrue(session.openURL(try XCTUnwrap(URL(string: "exacttest://auth/second#two"))))
        XCTAssertNil(session.boot(plan: try plan(source), size: CGSize(width: 390, height: 844)).error)
        let tree = session.agent(#"{"op":"tree"}"#)
        let state = session.agent(#"{"op":"state"}"#)
        XCTAssertTrue(tree.contains("/open?u=exacttest%3A%2F%2Fauth%2Ffirst%23one"), tree)
        XCTAssertTrue(state.contains("/open?u=exacttest%3A%2F%2Fauth%2Fsecond%23two"), state)
    }

    /// The agent's `type <root> <url>` (euxy FRICTION 13): a URL with a
    /// scheme arrives as the system's `openURL` would deliver it, a path as written.
    func testATypedURLArrivesAsTheSystemWouldDeliverIt() throws {
        let session = ExactApp.shared.makeSession()
        session.protocolHandlers = handlers
        defer { session.destroy() }
        XCTAssertEqual(session.typedLocation("exacttest://auth/x#frag"), "/open?u=exacttest%3A%2F%2Fauth%2Fx%23frag")
        XCTAssertEqual(session.typedLocation("/saved?q=1"), "/saved?q=1")
    }

    func testAdapterBatchDeliversEveryProtocolLinkInOrder() throws {
        let session = ExactApp.shared.makeSession()
        session.protocolHandlers = handlers
        defer { session.destroy() }
        let urls = ["exacttest://auth/first#one", "exacttest://auth/second#two"].compactMap(URL.init(string:))
        var delivered: [String] = []
        IncomingURLBatch.deliver(urls, development: { _ in false }, document: { _ in }, location: { url in
            XCTAssertTrue(session.openURL(url))
            delivered.append(url.absoluteString)
        })
        XCTAssertEqual(delivered, urls.map(\.absoluteString))
        XCTAssertEqual(session.launchLocation, "/open?u=exacttest%3A%2F%2Fauth%2Ffirst%23one")
    }

    /// A Mac browser (LLP 1030 D2): an `https` link cold and an `http` one
    /// warm reach the template whole, and so does an HTML file Finder
    /// brings, as its `file:` URL; any other file stays a document, and an
    /// HTML file is one too for an app that claims no web links.
    func testABrowserTakesWebLinksAndHTMLFilesAtItsTemplate() throws {
        let source = """
            routes nav
              home "/"
              opened "/open"

            component App
              state seen = ""
              action back
                nav = back(nav)
              action follow(location: string)
                seen = `${seen}|${location}`
              view
                main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow
                  text `${top(nav).url}` testId="where"
                  text `${seen}` testId="seen"
                  for e in stack(nav) key=e.id
                    column navigationKey=`${e.id}`
                      text `${e.name}`

            """
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("weblinks-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let page = dir.appendingPathComponent("page.html"), xhtml = dir.appendingPathComponent("page.xhtml"), notes = dir.appendingPathComponent("notes.txt")
        for file in [page, xhtml, notes] { try Data("<p>hi</p>".utf8).write(to: file) }

        let session = ExactApp.shared.makeSession()
        session.protocolHandlers = [["protocol": "https", "url": "/open?u=%s"], ["protocol": "http", "url": "/open?u=%s"]]
        defer { session.destroy() }
        var documents: [URL] = []
        func deliver(_ urls: [URL]) {
            IncomingURLBatch.deliver(urls, development: { _ in false }, webPage: session.isWebPageFile,
                                     document: { documents.append($0) }, location: { XCTAssertTrue(session.openURL($0)) })
        }
        deliver([try XCTUnwrap(URL(string: "https://example.com/a?b=1#c"))])
        XCTAssertEqual(session.launchLocation, "/open?u=https%3A%2F%2Fexample.com%2Fa%3Fb%3D1%23c")
        XCTAssertNil(session.boot(plan: try plan(source), size: CGSize(width: 390, height: 844)).error)
        XCTAssertTrue(session.agent(#"{"op":"tree"}"#).contains("/open?u=https%3A%2F%2Fexample.com%2Fa%3Fb%3D1%23c"))

        deliver([try XCTUnwrap(URL(string: "http://example.com/")), page, notes, xhtml])
        XCTAssertEqual(documents, [notes])
        let seen = session.agent(#"{"op":"state"}"#)
        let encoded = { (url: URL) in "/open?u=" + (url.absoluteString.addingPercentEncoding(withAllowedCharacters: .alphanumerics.union(CharacterSet(charactersIn: "-_.!~*'()"))) ?? "") }
        XCTAssertTrue(seen.contains("|/open?u=http%3A%2F%2Fexample.com%2F|\(encoded(page))|\(encoded(xhtml))"), seen)
        XCTAssertEqual(session.typedLocation("https://example.com/"), "/open?u=https%3A%2F%2Fexample.com%2F")

        let plain = ExactApp.shared.makeSession()
        plain.protocolHandlers = handlers
        defer { plain.destroy() }
        XCTAssertFalse(plain.isWebPageFile(page))
        XCTAssertEqual(plain.typedLocation("https://example.com/a"), "/a")
    }

    /// The web-page type a Mac browser declares (`ExactWebPage`) is Launch
    /// Services' alone: no picker offers it and no drop takes an HTML file
    /// for it, beside the app's own types or without them; an app whose own
    /// `file_handlers` name HTML still takes one.
    func testTheWebPageTypeIsNeitherPickedNorDropped() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("webpage-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let page = dir.appendingPathComponent("page.html"), notes = dir.appendingPathComponent("notes.md")
        for file in [page, notes] { try Data("<p>hi</p>".utf8).write(to: file) }
        let web: [String: Any] = ["CFBundleTypeName": "Web page", "CFBundleTypeRole": "Viewer", "LSHandlerRank": "Alternate",
                                  "LSItemContentTypes": ["public.html", "public.xhtml"], "ExactWebPage": true]
        let markdown: [String: Any] = ["CFBundleTypeRole": "Viewer", "LSItemContentTypes": ["net.daringfireball.markdown"], "CFBundleTypeExtensions": ["md"]]
        let browser: [String: Any] = ["CFBundleDocumentTypes": [web]]
        XCTAssertTrue(Picker.declarations(in: browser).isEmpty)
        XCTAssertEqual(Picker.types(in: browser), [])
        XCTAssertFalse(ExactDocuments.accepts(page, metadata: browser))
        let both: [String: Any] = ["CFBundleDocumentTypes": [markdown, web]]
        XCTAssertEqual(Picker.declarations(in: both).count, 1)
        XCTAssertFalse(Picker.types(in: both).contains(.html))
        XCTAssertFalse(ExactDocuments.accepts(page, metadata: both))
        XCTAssertTrue(ExactDocuments.accepts(notes, metadata: both))
        let viewer: [String: Any] = ["CFBundleDocumentTypes": [["CFBundleTypeRole": "Viewer", "LSItemContentTypes": ["public.html"]], web]]
        XCTAssertTrue(ExactDocuments.accepts(page, metadata: viewer))
    }

    func testNotificationClickUsesLaunchLocationThenWarmNavigateOnce() throws {
        let bytes = try plan(routed)
        let session = ExactApp.shared.makeSession(label: "notification-\(UUID().uuidString)")
        defer { session.destroy(); NotificationPresenter.shared.app = nil; NotificationPresenter.shared.activate = nil }
        NotificationPresenter.shared.app = ExactApp.shared
        var activations = 0
        NotificationPresenter.shared.activate = { if $0 === session { activations += 1 } }
        let info: [AnyHashable: Any] = ["navigate": "/thread/42", "session": session.label,
                                        "owner": session.notificationOwnerID, "process": ExactApp.shared.notificationProcessID]
        let date = Date()
        func texts() throws -> [String] {
            let data = try XCTUnwrap(session.agent("{\"op\":\"tree\"}").data(using: .utf8))
            let tree = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            let nodes = try XCTUnwrap(tree["nodes"] as? [[String: Any]])
            return nodes.compactMap { ($0["props"] as? [String: Any])?["text"] as? String }
        }
        NotificationPresenter.shared.deliver(id: "thread-42", date: date, info: info)
        XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 390, height: 844)).error)
        XCTAssertEqual(activations, 1)
        let cold = try texts()
        XCTAssertTrue(cold.contains { $0.contains("/thread/42") }, "\(cold)")
        XCTAssertTrue(cold.contains("none"), "\(cold)")
        NotificationPresenter.shared.deliver(id: "thread-42", date: date, info: info)
        XCTAssertEqual(try texts(), cold)
        XCTAssertEqual(activations, 1)
        NotificationPresenter.shared.deliver(id: "thread-43", date: date, info: ["navigate": "/thread/43", "session": session.label,
                                                                           "owner": session.notificationOwnerID])
        XCTAssertTrue(try texts().contains("/thread/43"))
        XCTAssertEqual(activations, 2)
    }

    func testNotificationDelegateClickArrivesBeforeMacBoot() throws {
        _ = NSApplication.shared
        let bytes = try plan(routed)
        let session = ExactApp.shared.makeSession(label: "delegate-\(UUID().uuidString)")
        let view = ExactView(session: session)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 390, height: 844),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close(); session.destroy(); NotificationPresenter.shared.app = nil; NotificationPresenter.shared.activate = nil }
        let presenter = NotificationPresenter.shared
        presenter.app = ExactApp.shared
        let received = expectation(description: "notification delegate completion")
        respond(action: UNNotificationDefaultActionIdentifier, id: UUID().uuidString,
                date: Date(), info: ["navigate": "/thread/42", "session": session.label,
                                     "owner": session.notificationOwnerID]) {
            received.fulfill()
        }
        XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 390, height: 844)).error)
        view.frame = window.contentView!.bounds
        window.contentView = view
        view.layoutSubtreeIfNeeded()
        XCTAssertTrue(view.window === window)
        XCTAssertEqual(session.generation, 1, "attaching an embedded view must preserve its launch boot")
        wait(for: [received], timeout: 2)
        let data = try XCTUnwrap(session.agent("{\"op\":\"tree\"}").data(using: .utf8))
        let tree = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let nodes = try XCTUnwrap(tree["nodes"] as? [[String: Any]])
        let texts = nodes.compactMap { ($0["props"] as? [String: Any])?["text"] as? String }
        XCTAssertTrue(texts.contains { $0.contains("/thread/42") }, "\(texts)")
        XCTAssertTrue(texts.contains("none"), "cold launch should not run the warm navigate action: \(texts)")
    }

    func testNotificationDismissalDoesNotOpenOrActivate() {
        let session = ExactApp.shared.makeSession(label: "dismiss-\(UUID().uuidString)")
        defer { session.destroy(); NotificationPresenter.shared.app = nil; NotificationPresenter.shared.activate = nil }
        let presenter = NotificationPresenter.shared
        presenter.app = ExactApp.shared
        var activations = 0
        presenter.activate = { _ in activations += 1 }
        let received = expectation(description: "notification dismissal completion")
        respond(action: UNNotificationDismissActionIdentifier, id: UUID().uuidString,
                date: Date(), info: ["navigate": "/thread/42", "session": session.label,
                                     "owner": session.notificationOwnerID]) {
            received.fulfill()
        }
        wait(for: [received], timeout: 2)
        XCTAssertNil(session.launchLocation)
        XCTAssertEqual(activations, 0)
    }

    func testNotificationClickSelectsItsOwnerWhenDefaultLabelsRepeat() {
        let app = ExactApp.shared
        let a = app.makeSession()
        let b = app.makeSession()
        a.destroy()
        let c = app.makeSession()
        defer { b.destroy(); c.destroy(); NotificationPresenter.shared.app = nil; NotificationPresenter.shared.activate = nil }
        XCTAssertEqual(b.label, c.label, "the label collision this click must survive")
        let presenter = NotificationPresenter.shared
        presenter.app = app
        let info: [AnyHashable: Any] = ["navigate": "/thread/42", "session": c.label,
                                        "owner": c.notificationOwnerID, "process": app.notificationProcessID]
        presenter.deliver(id: UUID().uuidString, date: Date(), info: info)
        XCTAssertNil(b.launchLocation)
        XCTAssertEqual(c.launchLocation, "/thread/42")
        c.destroy()
        b.destroy()
        let replacement = app.makeSession(label: c.label)
        defer { replacement.destroy() }
        presenter.deliver(id: UUID().uuidString, date: Date(), info: info)
        XCTAssertNil(replacement.launchLocation, "a dead owner's click must not transfer to a reused label")
    }

    func testNotificationFromPreviousProcessUsesUniqueColdSessionLabel() {
        let app = ExactApp.shared
        let session = app.makeSession(label: "main-\(UUID().uuidString)")
        defer { session.destroy(); NotificationPresenter.shared.app = nil; NotificationPresenter.shared.activate = nil }
        let presenter = NotificationPresenter.shared
        presenter.app = app
        presenter.deliver(id: UUID().uuidString, date: Date(), info: ["navigate": "/thread/42", "session": session.label,
                                                                 "owner": UUID().uuidString, "process": UUID().uuidString])
        XCTAssertEqual(session.launchLocation, "/thread/42")
    }

    func testPreviousProcessClicksReachBootedSessionThroughDelegate() throws {
        let session = ExactApp.shared.makeSession(label: "main-\(UUID().uuidString)")
        defer { session.destroy(); NotificationPresenter.shared.app = nil; NotificationPresenter.shared.activate = nil }
        NotificationPresenter.shared.app = ExactApp.shared
        XCTAssertNil(session.boot(plan: try plan(routed), size: CGSize(width: 390, height: 844)).error)
        let oldProcess = UUID().uuidString
        let oldOwner = UUID().uuidString
        for (id, location) in [("first", " \tthread/./42#reply "), ("second", "app://thread/../thread/43?view=full#reply")] {
            let received = expectation(description: "notification delegate completion \(id)")
            respond(action: UNNotificationDefaultActionIdentifier, id: UUID().uuidString, date: Date(),
                    info: ["navigate": location, "session": session.label,
                           "owner": oldOwner, "process": oldProcess]) { received.fulfill() }
            wait(for: [received], timeout: 2)
            let data = try XCTUnwrap(session.agent("{\"op\":\"tree\"}").data(using: .utf8))
            let tree = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            let nodes = try XCTUnwrap(tree["nodes"] as? [[String: Any]])
            let texts = nodes.compactMap { ($0["props"] as? [String: Any])?["text"] as? String }
            XCTAssertTrue(texts.contains(id == "first" ? "/thread/42" : "/thread/43?view=full"), "\(texts)")
        }
    }

    func testPreviousProcessClickWithoutMatchingLabelUsesFirstSession() {
        let app = ExactApp.shared
        let first = app.makeSession(label: "main-\(UUID().uuidString)")
        let second = app.makeSession(label: "other-\(UUID().uuidString)")
        defer { first.destroy(); second.destroy(); NotificationPresenter.shared.app = nil }
        NotificationPresenter.shared.app = app
        NotificationPresenter.shared.deliver(id: UUID().uuidString, date: Date(),
                                             info: ["navigate": "/thread/42", "session": "old-main",
                                                    "owner": UUID().uuidString, "process": UUID().uuidString])
        XCTAssertEqual(first.launchLocation, "/thread/42")
        XCTAssertNil(second.launchLocation)
    }
}
#endif
