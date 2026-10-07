#if os(macOS)
import Foundation
import XCTest
@testable import ExactKit

/// A launch URL an app cannot hear is journaled, as a running one is (#104).
final class LaunchURLMacTests: XCTestCase {
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
}
#endif
