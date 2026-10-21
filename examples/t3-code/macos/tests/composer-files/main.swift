import AppKit
import XCTest

// Compile with T3ComposerAttach.swift alone: a plain file TypeScript staged for a draft's file chip
// (composer-attach-stage.ts writes composer-files/<id>) is named for its preview; TypeScript reads and removes it
// (composer-staged-copies.ts), so the module answers no read or removal.
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

    func testAStagedFileIsNamedForItsPreviewWhileItIsThere() throws {
        let id = UUID().uuidString.lowercased()
        try "# Notes\n".write(to: root.appendingPathComponent("composer-files/\(id)"), atomically: true, encoding: .utf8)
        let attach = T3ComposerAttach(dataRoot: root)
        XCTAssertEqual(attach.stagedFile(id)?.lastPathComponent, id)
        XCTAssertNil(attach.stagedFile("../attach/a.txt"))
        try FileManager.default.removeItem(at: root.appendingPathComponent("composer-files/\(id)"))
        XCTAssertNil(attach.stagedFile(id))
    }

    func testOnlyTheMediaOpIsAnswered() {
        let attach = T3ComposerAttach(dataRoot: root)
        for op in ["composerAttachPick", "composerAttachRead", "composerAttachRemove"] {
            XCTAssertEqual(perform(attach, ["op": op, "id": UUID().uuidString])["ok"] as? Bool, false, "\(op) is TypeScript's")
        }
    }
}

_ = NSApplication.shared
let suite = XCTestSuite(forTestCaseClass: ComposerFilesTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
