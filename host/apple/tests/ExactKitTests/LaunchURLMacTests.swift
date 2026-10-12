#if os(macOS)
import Foundation
import XCTest
@testable import ExactKit

/// A launch URL an app cannot hear is journaled, as a running one is (#104).
final class LaunchURLMacTests: XCTestCase {
    private let handlers = [["protocol": "exacttest", "url": "/open?u=%s"]]

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
              each e in stack(nav) key=e.id
                column navigationKey=`${e.id}`
                  text `${e.name} ${e.url}`

        """

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
                  each e in stack(nav) key=e.id
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
                  each e in stack(nav) key=e.id
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
}
#endif
