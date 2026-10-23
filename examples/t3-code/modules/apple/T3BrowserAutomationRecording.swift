#if os(macOS)
import AppKit

/// The agent's recording tools on the previewAutomation host (MIT reference, see
/// LICENSE-T3, T3 Code 1e2ecbd975: apps/web/src/components/preview/PreviewAutomationHosts.tsx, the `recordingStart` and
/// `recordingStop` cases; apps/web/src/browser/browserRecordingUpload.ts; packages/client-runtime/src/state/attachments.ts
/// `runAttachmentUploadCycle`).
///
/// A recording's lifecycle is in the data module (browser-recording.ts: one recording per tab, the one the
/// Browser row's Capture button starts and stops), so the agent's and the person's recordings are the same. This host
/// keeps the request: it waits for the tab as for every other operation (`requireReadyTab`), lists the request in its
/// status (`presentation.browserAutomation.recordings`), and answers `previewAutomation.respond` with the outcome the
/// data module sends back (`browserAutomation` with `answer`, browser-automation-recording.ts), or with an execution
/// error once the request's host deadline passes with none. The stopped recording's upload to the request's
/// environment is native (`browserRecordingUpload`): the encoded file never crosses the data module.
extension T3BrowserAutomation {
    /// The request, handed to the data module until it answers or the host deadline passes.
    @MainActor func handOver(_ context: Context, runtimeId: String?) async -> Result<Any?, HostError> {
        let entry: [String: Any] = [
            "key": context.key, "requestId": context.requestId, "connectionId": context.connectionId, "operation": context.operation,
            "environmentId": context.environmentId, "threadId": context.threadId, "tabId": context.tabId ?? NSNull(),
            "tabIdExplicit": context.plan["tabIdExplicit"] as? Bool == true, "runtimeId": runtimeId ?? NSNull(), "input": context.input,
            "generation": context.generation, "fleet": context.fleet ?? "", "deadlineMs": (context.deadline.timeIntervalSince1970 * 1000).rounded(),
        ]
        let result = await withCheckedContinuation { (continuation: CheckedContinuation<Result<Any?, HostError>, Never>) in
            waiting[context.key] = continuation
            handedOver.append(entry)
            if handedOver.count > 16 { handedOver.removeFirst(handedOver.count - 16) }
            changed()
            let wait = max(0, context.deadline.timeIntervalSinceNow)
            DispatchQueue.main.asyncAfter(deadline: .now() + wait) { [weak self] in
                guard let self, let continuation = self.waiting.removeValue(forKey: context.key) else { return }
                self.handedOver.removeAll { $0["key"] as? String == context.key }
                continuation.resume(returning: .failure(.operation("the recording did not settle within the host deadline")))
            }
        }
        return result
    }

    /// A handed-over request's outcome: `ok` with its `result`, or the host error the data module serialized. False when
    /// the request is no longer waiting (answered, or past its deadline).
    func settle(_ key: String, _ plan: [String: Any]) -> Bool {
        guard let continuation = waiting.removeValue(forKey: key) else { return false }
        handedOver.removeAll { $0["key"] as? String == key }
        if plan["ok"] as? Bool == true { continuation.resume(returning: .success(plan["result"])) }
        else { continuation.resume(returning: .failure(.planned(plan["error"] as? [String: Any] ?? ["_tag": "PreviewAutomationExecutionError", "message": "The recording failed."]))) }
        changed()
        return true
    }

    // MARK: The upload (uploadBrowserRecording's transfer: createUploadUrl, then the bytes)

    /// `browserRecordingUpload`: `upload` sends a saved recording (a file in the artifact directory) to the environment as a
    /// pending attachment and answers `{status: "uploaded", attachmentId}` or `{status: "failed", attachmentId?, error,
    /// expired}`; `remove` deletes a pending attachment (deletePendingAttachmentUpload). `expired` is the reference's
    /// `Date.now() >= deadlineMs - 1_000` after a failed transfer: the module keeps the time, the data module has no clock.
    /// The request's connection is `connection: {generation, fleet}`, never a top-level `fleet` (T3Module `later` sends any
    /// request that names one to the fleet).
    func recordingUpload(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let connection = request["connection"] as? [String: Any] ?? [:]
        let generation = (connection["generation"] as? NSNumber)?.intValue ?? 0
        let fleet = (connection["fleet"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        if request["action"] as? String == "remove" {
            if let id = request["attachmentId"] as? String, !id.isEmpty { rpc("attachments.delete", ["attachmentId": id], generation, fleet) { _ in } }
            return reply([:])
        }
        let deadline = Date(timeIntervalSince1970: ((request["deadlineMs"] as? NSNumber)?.doubleValue ?? 0) / 1000)
        let expired = { Date() >= deadline.addingTimeInterval(-1) }
        let failed = { (attachmentId: String?, error: String) in
            reply(["status": "failed", "attachmentId": attachmentId ?? NSNull(), "error": error, "expired": expired()])
        }
        guard let directory = sessions?.artifactDirectory, let file = T3BrowserArtifacts.resolve(request["path"] as? String ?? "", in: directory),
              let data = try? Data(contentsOf: file), !data.isEmpty else { return failed(nil, "The saved recording is missing.") }
        // Encoding, saving and minting consume the same request budget. Leave time to reply.
        let remaining = deadline.timeIntervalSinceNow - 1
        guard remaining > 0 else { return failed(nil, "Recording transfer deadline expired.") }
        let mimeType = request["mimeType"] as? String ?? "video/mp4"
        let name = (request["name"] as? String).flatMap { $0.isEmpty ? nil : String($0.prefix(255)) } ?? file.lastPathComponent
        rpc("attachments.createUploadUrl", ["type": "file", "name": name, "mimeType": mimeType, "sizeBytes": data.count], generation, fleet) { [weak self] minted in
            DispatchQueue.main.async {
                guard let self else { return }
                let value = minted["value"] as? [String: Any] ?? [:]
                guard minted["ok"] as? Bool == true, let attachmentId = value["attachmentId"] as? String, let relativeUrl = value["relativeUrl"] as? String else {
                    return failed(nil, ((minted["error"] as? [String: Any])?["message"] as? String) ?? "The upload could not be prepared.")
                }
                var settled = false
                let finish = { (outcome: [String: Any]?) in
                    guard !settled else { return }
                    settled = true
                    if let outcome, outcome["ok"] as? Bool == true { return reply(["status": "uploaded", "attachmentId": attachmentId]) }
                    failed(attachmentId, outcome.map { (($0["error"] as? [String: Any])?["message"] as? String) ?? "Recording upload rejected." } ?? "Recording transfer deadline expired.")
                }
                // AbortSignal.timeout(remainingMs): the answer does not wait past the budget.
                DispatchQueue.main.asyncAfter(deadline: .now() + max(0, deadline.timeIntervalSinceNow - 1)) { finish(nil) }
                self.send(["op": "uploadAttachment", "path": relativeUrl, "base64": data.base64EncodedString(), "contentType": mimeType], generation, fleet) { outcome in
                    DispatchQueue.main.async { finish(outcome) }
                }
            }
        }
    }
}
#endif
