import AppKit
import XCTest

// Compile with T3ComposerAttach.swift and T3ImageFit.swift: what the module adds to a staged copy that TypeScript wrote
// (composer-attach-stage.ts): a video's or audio file's typed link and a video's first frame. TypeScript removes them
// (composer-staged-copies.ts).
final class ComposerAttachTests: XCTestCase {
    private var root: URL!

    override func setUp() {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-attach-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: root.appendingPathComponent("composer-files"), withIntermediateDirectories: true)
    }
    override func tearDown() { try? FileManager.default.removeItem(at: root) }

    private func perform(_ attach: T3ComposerAttach, _ request: [String: Any]) -> [String: Any] {
        let done = expectation(description: "reply")
        var out: [String: Any] = [:]
        attach.perform(request) { reply in out = reply; done.fulfill() }
        wait(for: [done], timeout: 10)
        return out
    }

    // A staged video's tile shows its first frame (a PNG beside the image drafts) and its preview plays a typed link
    // to the staged copy.
    func testVideoGetsItsFirstFrameAndTypedLink() throws {
        let app = ProcessInfo.processInfo.environment["T3_APP_DIR"] ?? FileManager.default.currentDirectoryPath + "/examples/t3-code"
        let movie = URL(fileURLWithPath: app).appendingPathComponent("../../../apps/video-player/assets/motion.mp4").standardizedFileURL
        try XCTSkipUnless(FileManager.default.fileExists(atPath: movie.path), "the video-player sample asset is not in this checkout")
        let id = UUID().uuidString.lowercased()
        try FileManager.default.copyItem(at: movie, to: root.appendingPathComponent("composer-files/\(id)"))
        let attach = T3ComposerAttach(dataRoot: root)
        let value = try XCTUnwrap(perform(attach, ["op": "composerAttachMedia", "id": id, "ext": "MP4", "video": true, "generation": 1])["value"] as? [String: Any])
        XCTAssertGreaterThan(value["videoWidth"] as? Int ?? 0, 0)
        XCTAssertGreaterThan(value["videoHeight"] as? Int ?? 0, 0)
        let poster = root.appendingPathComponent("snapshots/drafts/\(id).png")
        let rep = try XCTUnwrap(NSBitmapImageRep(data: try Data(contentsOf: poster)))
        XCTAssertLessThanOrEqual(max(rep.pixelsWide, rep.pixelsHigh), 512)
        let link = root.appendingPathComponent("composer-files/\(id).mp4")
        XCTAssertTrue(FileManager.default.fileExists(atPath: link.path))
        XCTAssertEqual(attach.stagedFile(id)?.lastPathComponent, id, "a chip's preview reads the staged copy")
    }

    func testAudioGetsItsTypedLinkAndNoFrame() throws {
        let id = UUID().uuidString.lowercased()
        try Data("ID3".utf8).write(to: root.appendingPathComponent("composer-files/\(id)"))
        let attach = T3ComposerAttach(dataRoot: root)
        let reply = perform(attach, ["op": "composerAttachMedia", "id": id, "ext": "m4a/../x", "video": false])
        XCTAssertEqual(reply["ok"] as? Bool, true)
        XCTAssertEqual((reply["value"] as? [String: Any])?.count, 0)
        XCTAssertTrue(FileManager.default.fileExists(atPath: root.appendingPathComponent("composer-files/\(id).m4ax").path), "the extension keeps letters and digits only")
        XCTAssertFalse(FileManager.default.fileExists(atPath: root.appendingPathComponent("snapshots/drafts/\(id).png").path))
        XCTAssertEqual(perform(attach, ["op": "composerAttachMedia", "id": UUID().uuidString, "ext": "mp4", "video": true])["ok"] as? Bool, false, "no staged copy")
    }
}

_ = NSApplication.shared
let suite = XCTestSuite(name: "attach")
suite.addTest(XCTestSuite(forTestCaseClass: ComposerAttachTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: ImageFitTests.self))
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
