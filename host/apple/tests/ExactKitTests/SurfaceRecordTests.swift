#if os(macOS)
import AppKit
import XCTest
import CryptoKit
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

    func testWorldCarrierRefusesSizeBeforeReadingAndCaptureBeforeEncoding() throws {
        let path = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        FileManager.default.createFile(atPath: path.path, contents: Data())
        defer { try? FileManager.default.removeItem(at: path) }
        let file = try FileHandle(forWritingTo: path)
        try file.truncate(atOffset: UInt64(WorldCarrier.limit + 1)); try file.close()
        let result = WorldCarrier.read(path.path)
        XCTAssertNil(result.bytes)
        XCTAssertEqual(result.error, WorldCarrier.refusal)
        XCTAssertThrowsError(try WorldCarrier.check(WorldCarrier.limit + 1))
        XCTAssertNoThrow(try WorldCarrier.check(WorldCarrier.limit))
    }

    func testSurfaceCheckpointStoreScopesOpaqueBytesByAppAndSurface() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let first = Data([0, 1, 2, 255])
        let second = Data("another world".utf8)
        try SurfaceCheckpointStore.write(first, app: "app.one", surface: "world/a", under: root)
        try SurfaceCheckpointStore.write(second, app: "app.two", surface: "world/a", under: root)
        XCTAssertEqual(try SurfaceCheckpointStore.read(app: "app.one", surface: "world/a", under: root), first)
        XCTAssertEqual(try SurfaceCheckpointStore.read(app: "app.two", surface: "world/a", under: root), second)
        XCTAssertNotEqual(
            try SurfaceCheckpointStore.url(app: "app.one", surface: "world/a", under: root),
            try SurfaceCheckpointStore.url(app: "app.one", surface: "world/b", under: root))
    }

    func testTerminalRestoreRefusalIsOneReplyAndCanvasStateRemainsAvailable() throws {
        let session = try fixture()
        defer { session.destroy() }
        let c = session.canvases
        let view = NodeView(id: 100, kind: "canvas", presenter: session.presenter)
        c.surface(view: view, name: "world", values: [])
        let entry = try XCTUnwrap(c.entries[100])
        entry.id = 1; entry.restoreAttempted = true; entry.restoreError = "surface world: restore refused: fixture"
        c.worldInput.bytes = Data([1])
        XCTAssertEqual(c.restoreReply(["ok": true])["error"] as? String, entry.restoreError)
        XCTAssertNil(c.restoreReply(["ok": true])["error"])
        XCTAssertEqual(c.worldInput.bytes, Data([1]), "refusal keeps bytes for a later capable surface")
        entry.id = 0
    }
    func testModuleIdentityAndDevelopmentOnlyPathOverride() throws {
        let path = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".dylib")
        let bytes = Data("a GPU product".utf8)
        try bytes.write(to: path)
        defer { try? FileManager.default.removeItem(at: path) }
        let card: [String: Any] = ["app":"test.app", "cohort":"cohort", "trust":"production",
            "sha256":SHA256.hash(data: bytes).map { String(format:"%02x", $0) }.joined()]
        func compat(_ value: [String: Any]) -> [String: Any] {
            ["id":"cohort", "inputs":["app":"test.app"], "embedded":["gpu":value]]
        }
        XCTAssertNil(GpuModule.verify(path:path.path, compat:compat(card)))
        for key in ["sha256", "app", "cohort"] {
            var wrong = card; wrong[key] = "other"
            let refusal = try XCTUnwrap(GpuModule.verify(path:path.path, compat:compat(wrong)))
            XCTAssertTrue(refusal.message.contains(key == "sha256" ? "digest" : key))
            XCTAssertTrue(refusal.message.contains(path.lastPathComponent))
        }
        XCTAssertNotNil(GpuModule.verify(path:path.path, compat:[:]))
        XCTAssertEqual(GpuModule.modulePath(defaultPath:"baked", compat:compat(card), environment:["EXACT_GPU_DYLIB":"override"]), "baked")
        var dev = card; dev["trust"] = "development"
        XCTAssertEqual(GpuModule.modulePath(defaultPath:"baked", compat:compat(dev), environment:["EXACT_GPU_DYLIB":"override"]), "override")
    }
    func testLostSurfaceLeavesFrameSchedulingUntilRecreated() throws {
        let session = try fixture()
        defer { session.destroy() }
        let view = NodeView(id:100, kind:"canvas", presenter:session.presenter)
        let entry = Canvases.Entry(view:view, name:"world", values:[])
        entry.id = 1
        entry.rendered(1)
        XCTAssertTrue(entry.needsFrame(dirty:false))
        entry.rendered(3)
        XCTAssertFalse(entry.needsFrame(dirty:true, editing:true))
        entry.rendered(1) // Stale frame results do not recreate a target.
        XCTAssertFalse(entry.needsFrame(dirty:true))
        XCTAssertEqual(entry.id, 1, "simulation identity survives")
        entry.presentable = true // Successful target creation restores scheduling.
        XCTAssertTrue(entry.needsFrame(dirty:true))
    }

}
#endif
