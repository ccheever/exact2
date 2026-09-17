#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class SurfaceRecordTests: XCTestCase {
    private func fixture() throws -> ExactSession {
        _ = NSApplication.shared
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = "shape Hud\n  beacons: number\ncomponent App\n  resource hud = exactSurface(\"world\") as shape Hud\n  view\n    text `Count ${hud.beacons}`\n"
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let compiler = Process()
        compiler.executableURL = root.appendingPathComponent("target/debug/contract")
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let session = ExactApp.shared.makeSession(label: "surface-record")
        session.canvases.loadRequested = true // No GPU is needed to exercise publication ownership.
        let batch = session.boot(plan: try Data(contentsOf: dir.appendingPathComponent("app.plan")), size: CGSize(width: 300, height: 100))
        XCTAssertNil(batch.error)
        return session
    }

    func testPublicationDuringApplyWaitsForTheLastStaleProp() throws {
        let session = try fixture()
        defer { session.destroy() }
        let text = try XCTUnwrap(session.presenter.views.values.first { $0.kind == "text" })
        session.presenter.onCommand = { [unowned session] _, _ in
            session.canvases.surfaceRecord("world", "{\"beacons\":7}")
            XCTAssertEqual(text.props["text"], "Count 0")
        }
        session.apply(Batch(ops: [
            ["op": "command", "name": "publish", "args": []],
            ["op": "props", "id": Int(text.id), "set": ["text": "stale"], "clear": []],
        ], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertEqual(text.props["text"], "Count 7")
    }

    func testOnlyTheFirstLivePublisherCanClearTheRecord() throws {
        let session = try fixture()
        defer { session.destroy() }
        let canvases = session.canvases
        let owner = NodeView(id: 100, kind: "canvas", presenter: session.presenter)
        let duplicate = NodeView(id: 101, kind: "canvas", presenter: session.presenter)
        canvases.surface(view: owner, name: "world", values: [])
        canvases.surface(view: duplicate, name: "world", values: [])
        canvases.surfaceRecord("world", "{\"beacons\":7}")
        let text = try XCTUnwrap(session.presenter.views.values.first { $0.kind == "text" })
        canvases.destroy(view: 101)
        XCTAssertEqual(text.props["text"], "Count 7")
        canvases.destroy(view: 100)
        XCTAssertEqual(text.props["text"], "Count 0")
    }
}
#endif
