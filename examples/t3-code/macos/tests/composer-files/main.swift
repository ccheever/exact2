import AppKit
import XCTest

// Compile with T3ComposerAttach.swift alone: a plain file TypeScript staged for a draft's file chip
// (composer-attach-stage.ts writes composer-files/<id>) is read back once at send, and released.
final class ComposerFilesTests: XCTestCase {
    private var root: URL!

    override func setUp() {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-files-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: root.appendingPathComponent("composer-files"), withIntermediateDirectories: true)
    }
    override func tearDown() { try? FileManager.default.removeItem(at: root) }

    private func perform(_ attach: T3ComposerAttach, _ request: [String: Any]) -> [String: Any] {
        let done = expectation(description: "reply")
        var out: [String: Any] = [:]
        attach.perform(request) { reply in out = reply; done.fulfill() }
        wait(for: [done], timeout: 5)
        return out
    }

    func testAStagedFileIsReadBackOnceAndReleased() throws {
        let id = UUID().uuidString.lowercased()
        try "# Notes\n".write(to: root.appendingPathComponent("composer-files/\(id)"), atomically: true, encoding: .utf8)
        let attach = T3ComposerAttach(dataRoot: root)
        XCTAssertEqual(attach.stagedFile(id)?.lastPathComponent, id, "a chip's preview reads the staged copy")
        let read = perform(attach, ["op": "composerAttachRead", "id": id, "generation": 2])
        XCTAssertEqual(read["ok"] as? Bool, true)
        let value = try XCTUnwrap(read["value"] as? [String: Any])
        XCTAssertEqual(Data(base64Encoded: value["base64"] as? String ?? "").map { String(decoding: $0, as: UTF8.self) }, "# Notes\n")
        XCTAssertEqual(perform(attach, ["op": "composerAttachRemove", "id": id])["ok"] as? Bool, true)
        XCTAssertNil(attach.stagedFile(id))
        let gone = perform(attach, ["op": "composerAttachRead", "id": id, "generation": 2])
        XCTAssertEqual(gone["ok"] as? Bool, false)
        XCTAssertEqual((gone["error"] as? [String: Any])?["message"] as? String, "The attached file was not saved.")
    }

    func testOnlyStagedIdsAreRead() {
        let attach = T3ComposerAttach(dataRoot: root)
        XCTAssertEqual(perform(attach, ["op": "composerAttachRead", "id": "../attach/a.txt"])["ok"] as? Bool, false)
        XCTAssertEqual(perform(attach, ["op": "composerAttachPick"])["ok"] as? Bool, false, "picking is the Contract file input's")
        XCTAssertEqual(perform(attach, ["op": "composerAttachUnknown"])["ok"] as? Bool, false)
    }
}

_ = NSApplication.shared
let suite = XCTestSuite(forTestCaseClass: ComposerFilesTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
