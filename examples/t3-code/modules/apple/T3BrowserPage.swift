#if os(macOS)
import AppKit
import WebKit

/// The `t3-browser-page` hatch: each Browser tab's partitioned frame (browser-frames.contract `BrowserFrame`) hands
/// its WKWebView here (LLP 1020.002 §9, tier 3) for what the framework does not do yet. MIT reference, see LICENSE-T3,
/// T3 Code 1e2ecbd975: apps/desktop/src/preview/Manager.ts (`startRecording`, `saveRecording`, `captureScreenshot`,
/// `revealArtifact`, `copyArtifactToClipboard`, `openPictureInPicture`, RECORDING_ARM_GRACE_MS),
/// apps/web/src/components/preview/AgentBrowserCursor.tsx.
///
/// - The recording (T3BrowserRecorder.swift): the frame as a capture source is LLP 1020.002 D8, waiting on B10.
/// - The agent's cursor (T3BrowserAgentCursor.swift): a layer of the page's view, so it travels with the page.
/// - The separate preview window (T3BrowserCapture.swift `T3BrowserPictureInPicture`): the page's snapshots, B10's too.
/// - The artifact directory (`T3BrowserArtifacts`), the local server's state directory outside the app's files: a capture
///   the frame wrote under `app:/tmp/browser-shots` moves in, and a screenshot or recording there is revealed in the Finder
///   or copied, which have no framework call; a saved recording is uploaded to an environment from there.
///
/// The arm owns the view (its delegates, zoom, appearance and user scripts): this adds one script message handler in the
/// client world and evaluates there, and never moves the view. A page is keyed by its frame's element id.
final class T3BrowserPages {
    let agent: Bool
    final class Page {
        weak var web: WKWebView?
        let recording = T3BrowserRecording()
        var pip: T3BrowserPictureInPicture?
        /// The file the encoder finished last, which `save` may move into the artifact directory (once).
        var encodedPath: String?
        var grace: DispatchWorkItem?
        func close() { grace?.cancel(); grace = nil; recording.close(); pip?.close(); pip = nil }
    }
    private(set) var pages: [String: Page] = [:]
    /// The module's data root (T3Module sets it), where the artifact directory falls back to, and its `app:/tmp`.
    var dataRoot: URL?
    var temporaryRoot: URL?
    var artifactDirectoryOverride: URL?
    lazy var artifactDirectory: URL = artifactDirectoryOverride ?? T3BrowserArtifacts.directory(dataRoot: dataRoot)
    /// Publishes a `t3.status` (a page's report changed).
    var changed: () -> Void = {}
    /// One RPC on an environment's connection (method, payload, generation, fleet key; nil the focused one), and any other
    /// transport op there: T3Module+Browser.swift wires both to the module's links.
    var rpc: (String, [String: Any], Int, String?, @escaping ([String: Any]) -> Void) -> Void = { _, _, _, _, done in done(["ok": false]) }
    var send: ([String: Any], Int, String?, @escaping ([String: Any]) -> Void) -> Void = { _, _, _, done in done(["ok": false]) }

    init(agent: Bool) { self.agent = agent }

    // MARK: The hatch

    func install(_ element: ExactElement) {
        guard element.hatch == .t3BrowserPage, let id = element.data[.browserFrame], !id.isEmpty, let web = element.platform as? WKWebView else { return }
        attach(id, web)
    }

    /// The frame `id`'s web view, new or a replacement (a changed partition makes a new one).
    func attach(_ id: String, _ web: WKWebView) {
        let page = pages[id] ?? Page()
        pages[id] = page
        page.web = web
        page.recording.attach(web)
        page.recording.changed = { [weak self] in self?.changed() }
        note("page \(id)")
        changed()
    }

    func remove(_ element: ExactElement) {
        guard element.hatch == .t3BrowserPage, let id = element.data[.browserFrame], let page = pages[id], page.web == nil || page.web === element.platform as? WKWebView else { return }
        pages[id] = nil
        page.close()
        note("page ended \(id)")
        changed()
    }

    func destroy() { for page in pages.values { page.close() }; pages.removeAll() }

    /// `presentation.browserPages[frameId]`: the page's laid-out size (the floating player sizes itself from it), the
    /// separate window, and the recording.
    var status: [String: Any] {
        ["browserPages": pages.mapValues { page -> [String: Any] in
            ["size": [Double(page.web?.frame.width ?? 0), Double(page.web?.frame.height ?? 0)], "pip": page.pip != nil, "recording": page.recording.report]
        }]
    }

    private func note(_ line: String) { FileHandle.standardError.write(Data("t3.browser: \(line.prefix(300))\n".utf8)) }

    // MARK: Ops (T3Module+Browser.swift routes them)

    /// `browserPage` (the cursor), `browserRecord`, `browserPip`, `browserArtifact` and `browserRecordingUpload`; false for any other op.
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) -> Bool {
        let generation = request["generation"] as? Int ?? 0
        let answer = { (value: [String: Any]) in reply(["ok": true, "generation": generation, "value": value]) }
        let refuse = { (kind: String, message: String) in reply(["ok": false, "generation": generation, "error": ["kind": kind, "message": message, "uncertain": false]]) }
        let op = request["op"] as? String ?? ""
        guard ["browserPage", "browserRecord", "browserPip", "browserArtifact", "browserRecordingUpload"].contains(op) else { return false }
        DispatchQueue.main.async { [self] in
            if op == "browserArtifact" { return artifact(request, answer: answer, refuse: refuse) }
            if op == "browserRecordingUpload" { return recordingUpload(request, reply: answer) }
            guard let page = pages[request["frame"] as? String ?? ""], let web = page.web else { return refuse("NotFound", "That browser tab is not open.") }
            switch op {
            case "browserPage":
                T3BrowserAgentCursor.show(on: web, phase: request["phase"] as? String ?? "move", x: (request["x"] as? NSNumber)?.doubleValue ?? 0,
                                          y: (request["y"] as? NSNumber)?.doubleValue ?? 0, sequence: request["sequence"] as? Int ?? 0, controller: request["controller"] as? String ?? "agent")
                answer([:])
            case "browserRecord": record(page, request, answer: answer, refuse: refuse)
            default: pictureInPicture(page, web, open: request["action"] as? String != "close", answer: answer)
            }
        }
        return true
    }

    private func record(_ page: Page, _ request: [String: Any], answer: @escaping ([String: Any]) -> Void, refuse: @escaping (String, String) -> Void) {
        let recording = page.recording
        let failed = { (error: Error) in refuse("Recording", error.localizedDescription) }
        switch request["action"] as? String {
        case "arm":
            let theme = (request["theme"] as? [String: Any] ?? [:]).compactMapValues { $0 as? String }
            recording.arm(showKeyPresses: request["showKeyPresses"] as? Bool ?? false, showMousePresses: request["showMousePresses"] as? Bool ?? false, theme: theme,
                          controller: request["controller"] as? String ?? "none") { [weak page] error in
                guard let page else { return }
                if let error { return failed(error) }
                // RECORDING_ARM_GRACE_MS: an armed tab that never starts recording lets go of its lease after 10 s.
                page.grace?.cancel()
                let work = DispatchWorkItem { [weak page] in if let page, !page.recording.recording { page.recording.cancel() } }
                page.grace = work
                DispatchQueue.main.asyncAfter(deadline: .now() + 10, execute: work)
                answer(["armed": true])
            }
        case "disarm":
            page.grace?.cancel(); page.grace = nil
            recording.disarm()
            answer(["armed": false])
        case "capture":
            recording.capture(frameRate: request["frameRate"] as? Int ?? 30) { result in
                switch result { case .success(let value): answer(value); case .failure(let error): failed(error) }
            }
        case "release":
            recording.releaseCapture()
            answer(["capturing": false])
        case "decorate":
            let active = recording.useDecorations(showKeyPresses: request["showKeyPresses"] as? Bool ?? false, showMousePresses: request["showMousePresses"] as? Bool ?? false,
                                                  primaryColor: request["primaryColor"] as? String ?? "#2563eb")
            answer(["active": active])
        case "begin":
            do {
                page.grace?.cancel(); page.grace = nil
                answer(["mimeType": try recording.begin(mimeType: request["mimeType"] as? String, bitsPerSecond: request["bitsPerSecond"] as? Int ?? 3_110_400)])
            } catch { failed(error) }
        case "finish":
            recording.finish { [weak page] result in
                switch result {
                case .success(let value): page?.encodedPath = value["path"] as? String; answer(value)
                case .failure(let error): failed(error)
                }
            }
        case "save": saveRecording(page, request, answer: answer, refuse: refuse)
        default:
            page.grace?.cancel(); page.grace = nil
            recording.cancel()
            answer([:])
        }
    }

    /// Manager.ts saveRecording: the encoded file moved into the artifact directory as `browser-recording-<millis36>.<ext>`.
    private func saveRecording(_ page: Page, _ request: [String: Any], answer: ([String: Any]) -> Void, refuse: (String, String) -> Void) {
        let source = URL(fileURLWithPath: request["path"] as? String ?? "")
        let mimeType = request["mimeType"] as? String ?? "video/mp4"
        // Only the file this page's encoder finished, once.
        guard let encoded = page.encodedPath, encoded == source.path, FileManager.default.fileExists(atPath: source.path) else { return refuse("Recording", "The encoded recording is missing.") }
        page.encodedPath = nil
        let now = Date(), id = "browser-recording-\(T3BrowserArtifacts.millis36(now))"
        let target = artifactDirectory.appendingPathComponent("\(id).\(T3BrowserArtifacts.fileExtension(mimeType))")
        do {
            try FileManager.default.createDirectory(at: artifactDirectory, withIntermediateDirectories: true)
            try? FileManager.default.removeItem(at: target)
            try FileManager.default.moveItem(at: source, to: target)
        } catch { return refuse("Recording", "The recording could not be saved: \(error.localizedDescription)") }
        let size = (try? FileManager.default.attributesOfItem(atPath: target.path)[.size] as? Int) ?? 0
        note("record saved \(target.lastPathComponent)")
        answer(["id": id, "path": target.path, "mimeType": mimeType, "sizeBytes": size, "createdAt": T3BrowserArtifacts.iso(now)])
    }

    private func pictureInPicture(_ page: Page, _ web: WKWebView, open: Bool, answer: ([String: Any]) -> Void) {
        if !open { page.pip?.close(); return answer(["open": false]) }
        if let pip = page.pip { pip.window.orderFrontRegardless(); return answer(["open": true]) }
        let pip = T3BrowserPictureInPicture(web: web, title: (web.title ?? "").trimmingCharacters(in: .whitespacesAndNewlines), agent: agent)
        pip.closed = { [weak self, weak page] in page?.pip = nil; self?.changed() }
        page.pip = pip
        changed()
        answer(["open": true])
    }

    // MARK: Artifacts (Manager.ts captureScreenshot's file, revealArtifact, copyArtifactToClipboard)

    /// `adopt` moves a capture the frame wrote under `app:/tmp/browser-shots` into the artifact directory as
    /// `browser-screenshot-<site>-<millis36>-<8 hex>.png` (the first eight characters of a v4 UUID, so two captures in one
    /// millisecond never share a file). `reveal`, `copy-image` and `copy-path` act on a file inside the directory only.
    private func artifact(_ request: [String: Any], answer: ([String: Any]) -> Void, refuse: (String, String) -> Void) {
        if request["action"] as? String == "adopt" {
            let relative = (request["path"] as? String ?? "").replacingOccurrences(of: "app:/tmp/", with: "")
            guard let temporary = temporaryRoot, relative.hasPrefix("browser-shots/"), !relative.contains("..") else { return refuse("ArtifactPath", "The capture is not one of the frame's.") }
            let source = temporary.appendingPathComponent(relative)
            let now = Date(), id = "browser-screenshot-\(T3BrowserArtifacts.slug(request["url"] as? String))-\(T3BrowserArtifacts.millis36(now))-\(UUID().uuidString.lowercased().prefix(8))"
            let target = artifactDirectory.appendingPathComponent("\(id).png")
            do {
                try FileManager.default.createDirectory(at: artifactDirectory, withIntermediateDirectories: true)
                try FileManager.default.moveItem(at: source, to: target)
            } catch { return refuse("Screenshot", "The screenshot could not be saved: \(error.localizedDescription)") }
            let size = (try? FileManager.default.attributesOfItem(atPath: target.path)[.size] as? Int) ?? 0
            note("screenshot \(target.lastPathComponent)")
            return answer(["id": id, "path": target.path, "mimeType": "image/png", "sizeBytes": size, "createdAt": T3BrowserArtifacts.iso(now)])
        }
        guard let target = T3BrowserArtifacts.resolve(request["path"] as? String ?? "", in: artifactDirectory) else {
            return refuse("ArtifactPath", "Preview artifact path is outside \(artifactDirectory.path)")
        }
        let pasteboard = T3BrowserArtifacts.pasteboard(agent: agent)
        switch request["action"] as? String {
        case "reveal":
            note("artifact reveal \(target.lastPathComponent)")
            if !agent { NSWorkspace.shared.activateFileViewerSelecting([target]) } // an agent run records it and opens no Finder window
            answer(["revealed": target.path])
        case "copy-image":
            guard let image = NSImage(contentsOf: target), let tiff = image.tiffRepresentation, let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else {
                return refuse("ArtifactImage", "The screenshot could not be read.")
            }
            pasteboard.clearContents()
            pasteboard.setData(png, forType: .png)
            answer(["copied": true])
        default:
            pasteboard.clearContents()
            answer(["copied": pasteboard.setString(target.path, forType: .string)])
        }
    }

    // MARK: A saved recording's upload (uploadBrowserRecording's transfer: createUploadUrl, then the bytes)

    /// `upload` sends a saved recording (a file in the artifact directory) to the environment as a pending attachment and
    /// answers `{status: "uploaded", attachmentId}` or `{status: "failed", attachmentId?, error, expired}`; `remove` deletes
    /// a pending attachment (deletePendingAttachmentUpload). `expired` is the reference's `Date.now() >= deadlineMs - 1_000`
    /// after a failed transfer: the module keeps the time, the data module has no clock. The request's connection is
    /// `connection: {generation, fleet}`, never a top-level `fleet` (T3Module `later` sends any request naming one to the fleet).
    private func recordingUpload(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let connection = request["connection"] as? [String: Any] ?? [:]
        let generation = (connection["generation"] as? NSNumber)?.intValue ?? 0
        let fleet = (connection["fleet"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        if request["action"] as? String == "remove" {
            if let id = request["attachmentId"] as? String, !id.isEmpty { rpc("attachments.delete", ["attachmentId": id], generation, fleet) { _ in } }
            return reply([:])
        }
        let deadline = Date(timeIntervalSince1970: ((request["deadlineMs"] as? NSNumber)?.doubleValue ?? 0) / 1000)
        let expired = { Date() >= deadline.addingTimeInterval(-1) }
        let failed = { (attachmentId: String?, error: String) in reply(["status": "failed", "attachmentId": attachmentId ?? NSNull(), "error": error, "expired": expired()]) }
        guard let file = T3BrowserArtifacts.resolve(request["path"] as? String ?? "", in: artifactDirectory), let data = try? Data(contentsOf: file), !data.isEmpty else {
            return failed(nil, "The saved recording is missing.")
        }
        // Encoding, saving and minting consume the same request budget. Leave time to reply.
        guard deadline.timeIntervalSinceNow - 1 > 0 else { return failed(nil, "Recording transfer deadline expired.") }
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
