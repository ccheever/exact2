#if os(macOS)
import AppKit
import ExactCore
import XCTest
@testable import ExactKit

/// LLP 1027.007 P1: a system notice of the page's facts does not hold the
/// main thread while the data source activates on the owner, and its batches
/// land before any batch the owner makes after them.
final class PageFactsSoonMacTests: XCTestCase {
    private static let plan: Result<Data, Error> = Result {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = "shape Page\n  visibilityState: string\ncomponent App\n  resource page = exactPage() as shape Page\n  state count = 0\n  action add\n    count = count + 1\n  view\n    column\n      text `${count}`\n      text `${page.visibilityState}` testId=\"visibility\"\n      button \"Add\" testId=\"add\" press=add\n"
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    func testANoticeWhileTheOwnerIsBusyReturnsAtOnceAndLandsBeforeALaterBatch() throws {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let session = ExactApp.shared.makeSession(label: "page-soon")
        defer { session.destroy() }
        window.contentView = ExactView(session: session)
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 300, height: 200)).error)
        window.makeKeyAndOrderFront(nil)
        session.tellPage()
        XCTAssertNotNil(session.readsOnLineKnown, "a synchronous tell learns whether the plan reads onLine")
        // Wait for the activation the first draw starts to finish, so a
        // system notice the click raises below (the window made key) takes
        // the synchronous path and can't add to `noticesInFlight`.
        let calm = Date(timeIntervalSinceNow: 10)
        while session.dataGeneration != session.generation || session.activationInFlight, Date() < calm {
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
        }
        XCTAssertEqual(session.dataGeneration, session.generation, "the first draw's activation settles")
        XCTAssertFalse(session.activationInFlight)

        // The owner busy as it is while the data source activates.
        let held = DispatchSemaphore(value: 0)
        Owner.shared.post { held.wait() }
        session.dataPreparing = true
        let started = Date()
        session.tellPageSoon()
        XCTAssertLessThan(Date().timeIntervalSince(started), 0.05, "the main thread does not wait for the busy owner")
        XCTAssertEqual(session.noticesInFlight, 3, "the facts, the root font size and the display scale are on the owner")
        held.signal()
        session.dataPreparing = false

        let add = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "add" })
        XCTAssertNil(session.agentInstance.tap(["op": "tap", "id": Int(add.id)])["error"])
        XCTAssertEqual(session.noticesInFlight, 0, "the notice's batches land before the press's, which the owner made after them")
        XCTAssertNotNil(session.presenter.views.values.first { $0.props["text"] == "1" }, "and the press applies")
    }
}
#endif
