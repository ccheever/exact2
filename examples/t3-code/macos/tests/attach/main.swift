import AppKit
import XCTest

// Compile with T3ComposerAttach.swift alone: real image files through the
// agent's attach/ directory, re-encoded into PNG drafts beside SnapShot's.
final class ComposerAttachTests: XCTestCase {
    private var root: URL!

    override func setUp() {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-attach-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: root.appendingPathComponent("attach"), withIntermediateDirectories: true)
    }
    override func tearDown() { try? FileManager.default.removeItem(at: root) }

    private func image(_ name: String, type: NSBitmapImageRep.FileType) throws {
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 4, pixelsHigh: 3, bitsPerSample: 8, samplesPerPixel: 4,
            hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        rep.setColor(.systemBlue, atX: 1, y: 1)
        try rep.representation(using: type, properties: [:])!.write(to: root.appendingPathComponent("attach/\(name)"))
    }

    func testKinds() {
        XCTAssertEqual(T3ComposerAttach.kind(of: URL(fileURLWithPath: "/x/a.PNG")), "image")
        XCTAssertEqual(T3ComposerAttach.kind(of: URL(fileURLWithPath: "/x/a.jpeg")), "image")
        XCTAssertEqual(T3ComposerAttach.kind(of: URL(fileURLWithPath: "/x/a.webp")), "image")
        XCTAssertEqual(T3ComposerAttach.kind(of: URL(fileURLWithPath: "/x/a.tiff")), "unsupported-image")
        XCTAssertEqual(T3ComposerAttach.kind(of: URL(fileURLWithPath: "/x/notes.txt")), "file")
    }

    func testAgentPickStagesPngDraftsAndConsumesTheFiles() throws {
        try image("photo.jpg", type: .jpeg)
        try image("scan.tiff", type: .tiff)
        try "hello".write(to: root.appendingPathComponent("attach/notes.txt"), atomically: true, encoding: .utf8)
        let attach = T3ComposerAttach(dataRoot: root, agent: true)
        let done = expectation(description: "reply")
        var files: [[String: Any]] = []
        attach.pick(["generation": 3]) { reply in
            XCTAssertEqual(reply["generation"] as? Int, 3)
            files = (reply["value"] as? [String: Any])?["files"] as? [[String: Any]] ?? []
            done.fulfill()
        }
        wait(for: [done], timeout: 5)
        XCTAssertEqual(files.map { $0["kind"] as? String }, ["file", "image", "unsupported-image"])
        let staged = files[1]
        XCTAssertEqual(staged["name"] as? String, "photo.png")
        let id = try XCTUnwrap(staged["id"] as? String)
        let png = try Data(contentsOf: root.appendingPathComponent("snapshots/drafts/\(id).png"))
        XCTAssertTrue(png.starts(with: [0x89, 0x50, 0x4e, 0x47]))
        XCTAssertEqual(staged["sizeBytes"] as? Int, png.count)
        XCTAssertEqual(NSBitmapImageRep(data: png)?.pixelsWide, 4)
        XCTAssertTrue(((try? FileManager.default.contentsOfDirectory(atPath: root.appendingPathComponent("attach").path)) ?? []).isEmpty)
    }

    // r4-composer: a picked video is a staged file whose tile shows its first
    // frame (a PNG beside the image drafts) and whose preview plays a typed
    // link to the staged copy; removing the file takes both.
    func testVideoStagesFirstFrameAndTypedLink() throws {
        let app = ProcessInfo.processInfo.environment["T3_APP_DIR"] ?? FileManager.default.currentDirectoryPath + "/examples/t3-code"
        let movie = URL(fileURLWithPath: app).appendingPathComponent("../../../apps/video-player/assets/motion.mp4").standardizedFileURL
        try XCTSkipUnless(FileManager.default.fileExists(atPath: movie.path), "the video-player sample asset is not in this checkout")
        try FileManager.default.copyItem(at: movie, to: root.appendingPathComponent("attach/clip.mp4"))
        XCTAssertTrue(T3ComposerAttach.isVideo(URL(fileURLWithPath: "/x/a.MOV")))
        XCTAssertFalse(T3ComposerAttach.isVideo(URL(fileURLWithPath: "/x/a.md")))
        let attach = T3ComposerAttach(dataRoot: root, agent: true)
        let done = expectation(description: "reply")
        var files: [[String: Any]] = []
        attach.pick(["generation": 1, "fileLimit": 50 * 1024 * 1024]) { reply in
            files = (reply["value"] as? [String: Any])?["files"] as? [[String: Any]] ?? []
            done.fulfill()
        }
        wait(for: [done], timeout: 10)
        let file = try XCTUnwrap(files.first)
        XCTAssertEqual(file["kind"] as? String, "file")
        XCTAssertEqual(file["mimeType"] as? String, "video/mp4")
        let id = try XCTUnwrap(file["id"] as? String)
        XCTAssertGreaterThan(file["videoWidth"] as? Int ?? 0, 0)
        XCTAssertGreaterThan(file["videoHeight"] as? Int ?? 0, 0)
        let poster = root.appendingPathComponent("snapshots/drafts/\(id).png")
        let rep = try XCTUnwrap(NSBitmapImageRep(data: try Data(contentsOf: poster)))
        XCTAssertLessThanOrEqual(max(rep.pixelsWide, rep.pixelsHigh), 512)
        let link = root.appendingPathComponent("composer-files/\(id).mp4")
        XCTAssertTrue(FileManager.default.fileExists(atPath: link.path))
        XCTAssertEqual(T3ComposerVideo(dataRoot: root, muted: true).source(id)?.lastPathComponent, "\(id).mp4")
        let removed = expectation(description: "removed")
        attach.perform(["op": "composerAttachRemove", "id": id, "generation": 2]) { _ in removed.fulfill() }
        wait(for: [removed], timeout: 5)
        XCTAssertFalse(FileManager.default.fileExists(atPath: link.path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: poster.path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: root.appendingPathComponent("composer-files/\(id)").path))
    }
}

_ = NSApplication.shared
let suite = XCTestSuite(forTestCaseClass: ComposerAttachTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
