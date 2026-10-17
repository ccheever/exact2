import AppKit
import WebKit
import XCTest

// The agent's recording tools on the previewAutomation host
// (T3BrowserAutomationRecording.swift). The host waits for the tab (recordingStart) or not (recordingStop), lists the
// request for the data module (`browserAutomation.recordings`), and answers `previewAutomation.respond` with the outcome
// the data module sends back, or with an execution error at the host deadline; the stopped recording's upload mints a
// pending attachment and sends the saved file's bytes on the request's connection (runAttachmentUploadCycle). The
// lifecycle itself is the data module's (browser-automation-recording.test.ts).
final class BrowserRecordingHandOffTests: XCTestCase {
    private var window: NSWindow!
    private var fixture: Fixture!
    private var sessions: T3BrowserSessions!
    private var automation: T3BrowserAutomation!
    private var artifacts: URL!
    private var responses: [String: [String: Any]] = [:]
    private var calls: [(String, [String: Any])] = []
    private var sent: [[String: Any]] = []
    private var uploadAnswer: [String: Any] = ["ok": true, "value": [:]]
    private var serial = 0

    override func setUpWithError() throws {
        fixture = try Fixture()
        fixture.page("/form", formPage)
        artifacts = FileManager.default.temporaryDirectory.appendingPathComponent("t3-recording-handoff-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: artifacts, withIntermediateDirectories: true)
        sessions = T3BrowserSessions(agent: true, changed: { _ in })
        sessions.artifactDirectoryOverride = artifacts
        automation = T3BrowserAutomation(sessions: sessions)
        sessions.created = { [weak automation] in automation?.prepare($0) }
        sessions.ended = { [weak automation] in automation?.forget($0) }
        automation.rpc = { [weak self] method, payload, generation, _, done in
            guard let self else { return }
            self.calls.append((method, payload.merging(["generation": generation]) { first, _ in first }))
            if method == "previewAutomation.respond", let id = payload["requestId"] as? String { self.responses[id] = payload; return done(["ok": true]) }
            if method == "attachments.createUploadUrl" {
                return done(["ok": true, "value": ["attachmentId": "pending-6f1c2a52-3c4e-4a8e-9a43-0d2c1b7f9e10-mp4", "relativeUrl": "/api/attachments/upload/pending-6f1c2a52?token=t", "expiresAt": 0]])
            }
            done(["ok": true, "value": [:]])
        }
        automation.send = { [weak self] request, generation, fleet, done in
            guard let self else { return }
            self.sent.append(request.merging(["generation": generation, "fleet": fleet ?? ""]) { first, _ in first })
            done(self.uploadAnswer)
        }
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 900, height: 700), styleMask: [.titled], backing: .buffered, defer: false)
        window.orderFrontRegardless()
    }
    override func tearDown() { sessions.sync([]); window.orderOut(nil); try? FileManager.default.removeItem(at: artifacts) }

    private func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
        let end = Date().addingTimeInterval(timeout)
        while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }
    private static let runtime = T3BrowserAutomation.runtimeId(environment: "env-1", thread: "thread-1", epoch: "epoch-1", tab: "tab-1")
    private func plan(_ operation: String, _ input: [String: Any] = [:], timeoutMs: Int = 15_000, runtimeId: String? = runtime, extra: [String: Any] = [:]) -> [String: Any] {
        serial += 1
        var plan: [String: Any] = ["requestId": "preview-\(serial)", "connectionId": "connection-1", "clientId": "preview-client", "operation": operation, "input": input,
                                   "environmentId": "env-1", "threadId": "thread-1", "tabId": "tab-1", "timeoutMs": timeoutMs, "generation": 4,
                                   "budgetMs": Double(timeoutMs) - min(1_500, ceil(Double(timeoutMs) * 0.2))]
        if let runtimeId { plan["runtimeId"] = runtimeId }
        for (key, value) in extra { plan[key] = value }
        return plan
    }
    private var handedOver: [[String: Any]] { (automation.status["browserAutomation"] as? [String: Any])?["recordings"] as? [[String: Any]] ?? [] }

    func testRecordingStartWaitsForTheTabThenAnswersWithTheDataModulesOutcome() throws {
        let request = plan("recordingStart")
        let id = request["requestId"] as! String
        XCTAssertEqual(automation.perform(request)["accepted"] as? Bool, true)
        spin(until: { false }, timeout: 0.4)
        XCTAssertTrue(handedOver.isEmpty, "requireReadyTab: nothing is handed over before the tab's page exists")
        sessions.ensure(id: Self.runtime, url: "\(fixture.base)/form", profile: "default", environment: "env-1")
        spin(until: { !self.handedOver.isEmpty })
        let entry = try XCTUnwrap(handedOver.first)
        XCTAssertEqual(entry["operation"] as? String, "recordingStart")
        XCTAssertEqual(entry["runtimeId"] as? String, Self.runtime)
        XCTAssertEqual(entry["tabId"] as? String, "tab-1")
        XCTAssertEqual(entry["environmentId"] as? String, "env-1")
        XCTAssertEqual(entry["threadId"] as? String, "thread-1")
        XCTAssertEqual(entry["generation"] as? Int, 4)
        let deadline = (entry["deadlineMs"] as? NSNumber)?.doubleValue ?? 0
        XCTAssertGreaterThan(deadline, Date().timeIntervalSince1970 * 1000, "the host deadline goes with the request (the data module has no clock)")
        XCTAssertNil(responses[id], "the host waits for the data module")
        let key = try XCTUnwrap(entry["key"] as? String)
        let status: [String: Any] = ["tabId": "tab-1", "recording": true, "startedAt": "2026-10-11T12:00:00.000Z"]
        XCTAssertEqual(automation.perform(["answer": key, "ok": true, "result": status])["accepted"] as? Bool, true)
        spin(until: { self.responses[id] != nil })
        XCTAssertEqual(responses[id]?["ok"] as? Bool, true)
        XCTAssertEqual(responses[id]?["result"] as? NSDictionary, status as NSDictionary, "PreviewAutomationRecordingStatus, as the data module gave it")
        XCTAssertTrue(handedOver.isEmpty, "an answered request is no longer listed")
        XCTAssertEqual(automation.perform(["answer": key, "ok": true, "result": status])["accepted"] as? Bool, false, "a second answer is refused")
        XCTAssertEqual(calls.filter { $0.0 == "previewAutomation.respond" }.count, 1)
    }

    func testRecordingStartWithNoPageTimesOutAsTheOverlayWaitDoes() {
        let request = plan("recordingStart", timeoutMs: 2_000)
        XCTAssertEqual(automation.perform(request)["accepted"] as? Bool, true)
        spin(until: { self.responses[request["requestId"] as! String] != nil }, timeout: 5)
        let error = responses[request["requestId"] as! String]?["error"] as? [String: Any] ?? [:]
        XCTAssertEqual(error["_tag"] as? String, "PreviewAutomationTimeoutError")
        XCTAssertTrue(handedOver.isEmpty)
    }

    func testRecordingStopIsHandedOverAtOnceAndAHostErrorPassesThrough() throws {
        let request = plan("recordingStop", ["transferToEnvironment": true], runtimeId: nil, extra: ["tabIdExplicit": true])
        let id = request["requestId"] as! String
        XCTAssertEqual(automation.perform(request)["accepted"] as? Bool, true)
        spin(until: { !self.handedOver.isEmpty }, timeout: 2)
        let entry = try XCTUnwrap(handedOver.first)
        XCTAssertEqual(entry["operation"] as? String, "recordingStop")
        XCTAssertEqual((entry["input"] as? [String: Any])?["transferToEnvironment"] as? Bool, true)
        XCTAssertEqual(entry["tabIdExplicit"] as? Bool, true)
        XCTAssertTrue(entry["runtimeId"] is NSNull, "the data module resolves the recording to stop")
        let transfer: [String: Any] = ["_tag": "PreviewAutomationRecordingTransferError",
                                       "message": "Preview recording could not be saved to the agent environment. The saved copy remains on the desktop.", "detail": ["threadId": "thread-1"]]
        XCTAssertEqual(automation.perform(["answer": entry["key"] as! String, "ok": false, "error": transfer])["accepted"] as? Bool, true)
        spin(until: { self.responses[id] != nil })
        XCTAssertEqual(responses[id]?["ok"] as? Bool, false)
        XCTAssertEqual(responses[id]?["error"] as? NSDictionary, transfer as NSDictionary, "serializePreviewAutomationHostError's shape, as the data module made it")
    }

    func testAHandedOverRequestWithNoAnswerFailsAtItsHostDeadline() {
        let request = plan("recordingStop", [:], timeoutMs: 2_000, runtimeId: nil)
        let id = request["requestId"] as! String
        XCTAssertEqual(automation.perform(request)["accepted"] as? Bool, true)
        spin(until: { !self.handedOver.isEmpty }, timeout: 2)
        let key = handedOver.first?["key"] as? String ?? ""
        spin(until: { self.responses[id] != nil }, timeout: 5)
        let error = responses[id]?["error"] as? [String: Any] ?? [:]
        XCTAssertEqual(error["_tag"] as? String, "PreviewAutomationExecutionError", "answered before the broker's own timeout (the budget keeps a margin)")
        XCTAssertTrue(handedOver.isEmpty)
        XCTAssertEqual(automation.perform(["answer": key, "ok": true, "result": [:]])["accepted"] as? Bool, false, "a late answer is dropped")
    }

    // MARK: The upload

    private func upload(_ request: [String: Any]) -> [String: Any] {
        var answer: [String: Any]?
        automation.recordingUpload(request) { answer = $0 }
        spin(until: { answer != nil })
        return answer ?? [:]
    }

    func testTheUploadMintsAPendingFileAttachmentAndSendsTheSavedBytesOnTheRequestsConnection() throws {
        let file = artifacts.appendingPathComponent("browser-recording-mg3k2a1b.mp4")
        let bytes = Data((0..<4096).map { UInt8($0 % 251) })
        try bytes.write(to: file)
        let deadline = (Date().timeIntervalSince1970 + 30) * 1000
        let answer = upload(["action": "upload", "path": file.path, "name": "browser-recording-mg3k2a1b.mp4", "mimeType": "video/mp4", "sizeBytes": bytes.count,
                             "deadlineMs": deadline, "connection": ["generation": 4, "fleet": "origin\nenv-2"]])
        XCTAssertEqual(answer["status"] as? String, "uploaded")
        XCTAssertEqual(answer["attachmentId"] as? String, "pending-6f1c2a52-3c4e-4a8e-9a43-0d2c1b7f9e10-mp4")
        let mint = try XCTUnwrap(calls.first { $0.0 == "attachments.createUploadUrl" }?.1)
        XCTAssertEqual(mint["type"] as? String, "file")
        XCTAssertEqual(mint["name"] as? String, "browser-recording-mg3k2a1b.mp4")
        XCTAssertEqual(mint["mimeType"] as? String, "video/mp4")
        XCTAssertEqual(mint["sizeBytes"] as? Int, bytes.count, "the file's own size")
        XCTAssertEqual(mint["generation"] as? Int, 4, "on the request's connection")
        let post = try XCTUnwrap(sent.first)
        XCTAssertEqual(post["op"] as? String, "uploadAttachment")
        XCTAssertEqual(post["path"] as? String, "/api/attachments/upload/pending-6f1c2a52?token=t")
        XCTAssertEqual(post["contentType"] as? String, "video/mp4")
        XCTAssertEqual(post["generation"] as? Int, 4)
        XCTAssertEqual(post["fleet"] as? String, "origin\nenv-2", "a background environment's request uploads on its fleet transport")
        XCTAssertEqual(Data(base64Encoded: post["base64"] as? String ?? ""), bytes, "the bytes of the saved file, once")
    }

    func testTheUploadRefusesAFileOutsideTheArtifactDirectoryAndAnExpiredBudget() throws {
        let outside = FileManager.default.temporaryDirectory.appendingPathComponent("t3-not-an-artifact-\(UUID().uuidString).mp4")
        try Data([1, 2, 3]).write(to: outside)
        defer { try? FileManager.default.removeItem(at: outside) }
        let future = (Date().timeIntervalSince1970 + 30) * 1000
        let refused = upload(["action": "upload", "path": outside.path, "mimeType": "video/mp4", "deadlineMs": future, "connection": ["generation": 4]])
        XCTAssertEqual(refused["status"] as? String, "failed")
        XCTAssertEqual(refused["expired"] as? Bool, false)
        let inside = artifacts.appendingPathComponent("browser-recording-late.mp4")
        try Data([1, 2, 3]).write(to: inside)
        let late = upload(["action": "upload", "path": inside.path, "mimeType": "video/mp4", "deadlineMs": (Date().timeIntervalSince1970 + 0.5) * 1000, "connection": ["generation": 4]])
        XCTAssertEqual(late["status"] as? String, "failed")
        XCTAssertEqual(late["expired"] as? Bool, true, "a second or less left: no transfer starts (\"Recording transfer deadline expired.\")")
        XCTAssertTrue(calls.filter { $0.0 == "attachments.createUploadUrl" }.isEmpty, "nothing is minted for either")
        XCTAssertTrue(sent.isEmpty)
    }

    func testARejectedTransferNamesItsPendingAttachmentAndRemoveDeletesIt() throws {
        let file = artifacts.appendingPathComponent("browser-recording-rejected.mp4")
        try Data(repeating: 7, count: 512).write(to: file)
        uploadAnswer = ["ok": false, "error": ["kind": "HTTP", "message": "Recording upload rejected (413)."]]
        let answer = upload(["action": "upload", "path": file.path, "mimeType": "video/mp4", "deadlineMs": (Date().timeIntervalSince1970 + 30) * 1000, "connection": ["generation": 4]])
        XCTAssertEqual(answer["status"] as? String, "failed")
        XCTAssertEqual(answer["attachmentId"] as? String, "pending-6f1c2a52-3c4e-4a8e-9a43-0d2c1b7f9e10-mp4", "the data module deletes the pending upload")
        XCTAssertEqual(answer["expired"] as? Bool, false, "a failure well before the deadline is a transfer error")
        _ = upload(["action": "remove", "attachmentId": "pending-6f1c2a52-3c4e-4a8e-9a43-0d2c1b7f9e10-mp4", "connection": ["generation": 4]])
        XCTAssertEqual(calls.last?.0, "attachments.delete")
        XCTAssertEqual(calls.last?.1["attachmentId"] as? String, "pending-6f1c2a52-3c4e-4a8e-9a43-0d2c1b7f9e10-mp4")
    }
}
