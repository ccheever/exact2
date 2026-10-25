import AppKit
import XCTest

/// A saved recording's bytes to the upload URL the data module minted (`browserRecordingUpload`, T3BrowserPage.swift),
/// against CaptureFixture peers standing in for the environment's server.
final class BrowserRecordingUploadTests: XCTestCase {
    private var pages: T3BrowserPages!
    private var server: CaptureFixture!
    private var elsewhere: CaptureFixture!
    private var saved: URL!
    private let path = "/api/attachments/upload/pending-1-mp4"

    override func setUpWithError() throws {
        server = try CaptureFixture()
        elsewhere = try CaptureFixture()
        let artifacts = CaptureSpin.outputDirectory("upload-artifacts")
        pages = T3BrowserPages(agent: true)
        pages.artifactDirectoryOverride = artifacts
        saved = artifacts.appendingPathComponent("recording.mp4")
        try Data("mp4".utf8).write(to: saved)
    }
    override func tearDown() { pages.destroy() }

    private func upload(origin: String? = nil, relativeUrl: String? = nil, deadline: TimeInterval = 10, timeout: TimeInterval = 10) -> [String: Any] {
        var answer: [String: Any]?
        let request: [String: Any] = ["op": "browserRecordingUpload", "origin": origin ?? server.base, "relativeUrl": relativeUrl ?? path, "path": saved.path,
                                      "mimeType": "video/mp4", "deadlineMs": (Date().timeIntervalSince1970 + deadline) * 1000]
        XCTAssertTrue(pages.perform(request) { answer = $0 })
        CaptureSpin.until({ answer != nil }, timeout: timeout)
        return answer?["value"] as? [String: Any] ?? [:]
    }

    func testTheBytesGoToTheMintedUrlOnTheConnectedServer() {
        server.file(path, "application/json", Data("{}".utf8))
        XCTAssertEqual(upload()["status"] as? String, "uploaded")
        XCTAssertEqual(server.seen, [path])
    }

    func testARedirectToAnotherServerIsRefused() {
        elsewhere.file("/stolen", "application/json", Data("{}".utf8))
        server.routes[path] = (302, "text/plain", Data(), 0, "Location: \(elsewhere.base)/stolen\r\n")
        let answer = upload()
        XCTAssertEqual(answer["status"] as? String, "failed")
        XCTAssertEqual(server.seen, [path])
        XCTAssertEqual(elsewhere.seen, [])
    }

    func testAnUploadUrlOffTheConnectedServerOrOutsideTheUploadPathIsRefused() {
        for (origin, relative) in [(server.base, "\(elsewhere.base)\(path)"), (server.base, "//127.0.0.1:\(elsewhere.port)\(path)"),
                                   (server.base, "/api/orchestration/shell"), ("file:///tmp", path)] {
            let answer = upload(origin: origin, relativeUrl: relative)
            XCTAssertEqual(answer["status"] as? String, "failed", relative)
            XCTAssertEqual(answer["error"] as? String, "The upload could not be prepared.", relative)
        }
        XCTAssertEqual(server.seen + elsewhere.seen, [])
    }

    func testTheDeadlineEndsATransferTheServerHoldsAndSaysItExpired() {
        server.routes[path] = (200, "application/json", Data("{}".utf8), 5, "")
        let started = Date()
        let answer = upload(deadline: 2, timeout: 4)
        XCTAssertEqual(answer["status"] as? String, "failed")
        XCTAssertEqual(answer["error"] as? String, "Recording transfer deadline expired.")
        XCTAssertEqual(answer["expired"] as? Bool, true)
        XCTAssertLessThan(Date().timeIntervalSince(started), 2)
        XCTAssertEqual(upload(deadline: 0.5)["error"] as? String, "Recording transfer deadline expired.") // a second or less left: nothing is sent
        XCTAssertEqual(server.seen, [path])
    }
}
