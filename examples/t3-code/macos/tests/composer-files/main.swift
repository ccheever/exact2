import AppKit
import XCTest

// Compile with T3ComposerAttach.swift alone: plain files picked with Attach
// files (the agent's attach/ directory stands in for the panel) are staged
// for the draft's file chips, read back once at send, and released.
final class ComposerFilesTests: XCTestCase {
    private var root: URL!

    override func setUp() {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-files-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: root.appendingPathComponent("attach"), withIntermediateDirectories: true)
    }
    override func tearDown() { try? FileManager.default.removeItem(at: root) }

    private func perform(_ attach: T3ComposerAttach, _ request: [String: Any]) -> [String: Any] {
        let done = expectation(description: "reply")
        var out: [String: Any] = [:]
        attach.perform(request) { reply in out = reply; done.fulfill() }
        wait(for: [done], timeout: 5)
        return out
    }

    func testPlainFilesStageWithinTheServerLimitAndReadBackOnce() throws {
        try "# Notes\n".write(to: root.appendingPathComponent("attach/notes.md"), atomically: true, encoding: .utf8)
        try Data(repeating: 7, count: 4096).write(to: root.appendingPathComponent("attach/big.bin"))
        try Data().write(to: root.appendingPathComponent("attach/empty.txt"))
        let attach = T3ComposerAttach(dataRoot: root, agent: true)
        let picked = perform(attach, ["op": "composerAttachPick", "generation": 2, "fileLimit": 1024])
        let files = (picked["value"] as? [String: Any])?["files"] as? [[String: Any]] ?? []
        XCTAssertEqual(files.map { $0["name"] as? String }, ["big.bin", "empty.txt", "notes.md"])
        XCTAssertEqual(files.map { $0["kind"] as? String }, ["file", "file", "file"])
        // Over the limit or empty: reported with its size, nothing staged.
        XCTAssertNil(files[0]["id"]); XCTAssertEqual(files[0]["sizeBytes"] as? Int, 4096)
        XCTAssertNil(files[1]["id"]); XCTAssertEqual(files[1]["sizeBytes"] as? Int, 0)
        let notes = files[2]
        XCTAssertEqual(notes["mimeType"] as? String, "text/markdown")
        XCTAssertEqual(notes["sizeBytes"] as? Int, 8)
        let id = try XCTUnwrap(notes["id"] as? String)
        XCTAssertEqual((try? FileManager.default.contentsOfDirectory(atPath: root.appendingPathComponent("composer-files").path)) ?? [], [id])
        let read = perform(attach, ["op": "composerAttachRead", "id": id, "generation": 2])
        XCTAssertEqual(read["ok"] as? Bool, true)
        let value = try XCTUnwrap(read["value"] as? [String: Any])
        XCTAssertEqual(Data(base64Encoded: value["base64"] as? String ?? "").map { String(decoding: $0, as: UTF8.self) }, "# Notes\n")
        XCTAssertEqual(perform(attach, ["op": "composerAttachRemove", "id": id])["ok"] as? Bool, true)
        let gone = perform(attach, ["op": "composerAttachRead", "id": id, "generation": 2])
        XCTAssertEqual(gone["ok"] as? Bool, false)
        XCTAssertEqual((gone["error"] as? [String: Any])?["message"] as? String, "The attached file was not saved.")
    }

    func testAServerThatTakesNoFilesStagesNothing() throws {
        try "x".write(to: root.appendingPathComponent("attach/a.txt"), atomically: true, encoding: .utf8)
        let attach = T3ComposerAttach(dataRoot: root, agent: true)
        let files = ((perform(attach, ["op": "composerAttachPick"])["value"] as? [String: Any])?["files"] as? [[String: Any]]) ?? []
        XCTAssertEqual(files.count, 1)
        XCTAssertNil(files[0]["id"])
        XCTAssertFalse(FileManager.default.fileExists(atPath: root.appendingPathComponent("composer-files").path))
    }

    func testAPickedPngKeepsItsBytes() throws {
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 5, pixelsHigh: 5, bitsPerSample: 8, samplesPerPixel: 4,
            hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        let png = try XCTUnwrap(rep.representation(using: .png, properties: [:]))
        try png.write(to: root.appendingPathComponent("attach/shot.png"))
        let attach = T3ComposerAttach(dataRoot: root, agent: true)
        let files = ((perform(attach, ["op": "composerAttachPick"])["value"] as? [String: Any])?["files"] as? [[String: Any]]) ?? []
        let id = try XCTUnwrap(files.first?["id"] as? String)
        XCTAssertEqual(files.first?["sizeBytes"] as? Int, png.count, "the shelf and chip report the file's own size")
        XCTAssertEqual(try Data(contentsOf: root.appendingPathComponent("snapshots/drafts/\(id).png")), png)
    }

    func testOnlyStagedIdsAreRead() {
        let attach = T3ComposerAttach(dataRoot: root, agent: true)
        XCTAssertEqual(perform(attach, ["op": "composerAttachRead", "id": "../attach/a.txt"])["ok"] as? Bool, false)
        XCTAssertEqual(perform(attach, ["op": "composerAttachUnknown"])["ok"] as? Bool, false)
        XCTAssertEqual(T3ComposerAttach.mimeType(of: URL(fileURLWithPath: "/x/data.unknownext")), "application/octet-stream")
        XCTAssertEqual(T3ComposerAttach.mimeType(of: URL(fileURLWithPath: "/x/report.pdf")), "application/pdf")
    }
}

_ = NSApplication.shared
let suite = XCTestSuite(forTestCaseClass: ComposerFilesTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
