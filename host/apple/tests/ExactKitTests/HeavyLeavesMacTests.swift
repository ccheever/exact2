#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1068 §5.2 on macOS (`Mac/HeavyLeavesMac.swift`): a video in a
/// collection's row is made when its row comes within a quarter viewport
/// of what shows, not when the row is built ahead; one outside any list is
/// made with its batch; the agent's settle makes every one.
final class HeavyLeavesMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil }

    func testARowsVideoWaitsUntilItsRowIsNearAndSettleMakesEveryOne() throws {
        _ = NSApplication.shared
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = """
        shape Location
          lat: number
          lon: number
        shape Station
          id: string
          name: string
          zone: number
          distance: number
        component App
          resource location = defaultLocation() as shape Location
          resource allStations = stations(location) as shape list<Station>
          view
            column width="100%" height="100%"
              video "clip.mp4" width="100%" height=40 muted=true testId="lead"
              list virtualized=true estimated-item-height=120 flex=1 min-height=0 width="100%" testId="feed"
                each s in allStations key=s.id
                  column width="100%" height=120 padding=8
                    text s.name
                    video "clip.mp4" width="100%" height=80 muted=true

        """
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let bytes = try Data(contentsOf: dir.appendingPathComponent("app.plan"))

        let session = ExactApp.shared.makeSession(label: "heavy-leaves")
        defer { session.destroy() }
        let view = ExactView(session: session)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 320, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = view
        self.window = window
        view.layoutSubtreeIfNeeded()
        XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 320, height: 240)).error)
        func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
        let deadline = Date().addingTimeInterval(5)
        while session.presenter.views.values.filter({ $0.kind == "video" }).count < 4, Date() < deadline { spin(0.02) }
        let presenter = session.presenter
        let videos = presenter.views.values.filter { $0.kind == "video" }
        XCTAssertGreaterThan(videos.count, 3, "the list built rows ahead of the port")
        // Outside any list: made with its batch.
        let lead = try XCTUnwrap(videos.first { $0.props["testId"] == "lead" })
        XCTAssertNotNil(lead.video, "a video in no collection's row is made with its batch")
        XCTAssertFalse(presenter.leaves.isPending(lead))
        // In rows: those near the port are made, those built ahead wait.
        let content = try XCTUnwrap(window.contentView)
        let shown = content.convert(content.bounds, to: nil).insetBy(dx: -80, dy: -60)
        let rows = videos.filter { $0.props["testId"] == nil }
        let near = rows.filter { $0.convert($0.bounds, to: nil).intersects(shown) }
        let far = rows.filter { !$0.convert($0.bounds, to: nil).intersects(shown) }
        XCTAssertFalse(near.isEmpty, "some rows show")
        XCTAssertFalse(far.isEmpty, "some rows were built beyond a quarter viewport")
        for v in near { XCTAssertNotNil(v.video, "a row within a quarter viewport has its video") }
        for v in far {
            XCTAssertNil(v.video, "a row built ahead waits for its video")
            XCTAssertTrue(presenter.leaves.isPending(v))
        }
        // The agent's settle: every one is made.
        presenter.leaves.settle()
        for v in rows { XCTAssertNotNil(v.video); XCTAssertFalse(presenter.leaves.isPending(v)) }
        XCTAssertEqual((presenter.leaves.observation["pendingLeaves"] as? [Int])?.count ?? -1, 0)
    }
}
#endif
