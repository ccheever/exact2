#if os(macOS)
import AppKit
import CryptoKit
import WebKit

/// The `t3-browser-page` hatch: each Browser tab's partitioned frame (browser-frames.contract `BrowserFrame`) hands
/// its WKWebView here (LLP 1020.002 §9, tier 3) for what the framework does not do yet. MIT reference, see LICENSE-T3,
/// T3 Code 1e2ecbd975: apps/desktop/src/preview/Manager.ts (`startRecording`, `saveRecording`, `captureScreenshot`,
/// `revealArtifact`, `openPictureInPicture`, RECORDING_ARM_GRACE_MS),
/// apps/web/src/components/preview/AgentBrowserCursor.tsx.
///
/// - The recording (T3BrowserRecorder.swift): the frame as a capture source is LLP 1020.002 D8, waiting on B10.
/// - The agent's cursor (T3BrowserAgentCursor.swift): a layer of the page's view, so it travels with the page.
/// - The separate preview window (T3BrowserCapture.swift `T3BrowserPictureInPicture`): the page's snapshots, B10's too.
/// - The artifact directory (`T3BrowserArtifacts`), the local server's state directory outside the app's files: a capture
///   the frame wrote under `app:/tmp/browser-shots` moves in, and a screenshot or recording there is revealed in the Finder
///   or copied, which have no framework call; a saved recording's bytes go from there to the upload URL the data module
///   minted on its connection.
/// - The profiles' cookies from the stores the module kept before the frames (`migrateCookies`), moved once into the
///   frame's partition, which the framework names by its own scheme.
///
/// The arm owns the view (its delegates, zoom, appearance and user scripts): this adds one script message handler in the
/// client world and evaluates there, and never moves the view. A page is keyed by its frame's element id.
final class T3BrowserPages {
    let agent: Bool
    final class Page {
        weak var web: WKWebView?
        /// Cookies moved into the page's partition from the profile's old store; the data module reloads the page once for them.
        var migrated = 0
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

    init(agent: Bool) { self.agent = agent }

    // MARK: The hatch

    func install(_ element: ExactElement) {
        guard element.hatch == .t3BrowserPage, let id = element.data[.browserFrame], !id.isEmpty, let web = element.platform as? WKWebView else { return }
        attach(id, web)
        let environment = element.data[.browserEnvironment] ?? "", profile = element.data[.browserProfile] ?? ""
        if !agent, !environment.isEmpty, !profile.isEmpty, profile != "incognito" {
            migrateCookies(environment: environment, profile: profile, into: web.configuration.websiteDataStore) { [weak self] moved in
                guard moved > 0, let page = self?.pages[id] else { return }
                page.migrated = moved
                self?.changed()
            }
        }
    }

    // MARK: The profiles' old stores
    /// `remove(forIdentifier:)` refuses a store the network process still holds, which it lets go of shortly after the
    /// last reference: asked again every half second.
    @available(macOS 14.0, *)
    static func removeStore(_ identifier: UUID, attempts: Int, done: @escaping (Error?) -> Void) {
        WKWebsiteDataStore.remove(forIdentifier: identifier) { error in
            guard error != nil, attempts > 1 else { return done(error) }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { removeStore(identifier, attempts: attempts - 1, done: done) }
        }
    }

    /// The store the module kept an environment's profile in before the frames (SHA-256 of the scope as an RFC 9562 v8 UUID).
    static func oldStoreIdentifier(environment: String, profile: String) -> UUID {
        var bytes = Array(SHA256.hash(data: Data("t3code-preview\u{0}\(environment)\u{0}\(profile)".utf8)).prefix(16))
        bytes[6] = (bytes[6] & 0x0f) | 0x80
        bytes[8] = (bytes[8] & 0x3f) | 0x80
        return UUID(uuid: (bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]))
    }
    /// Scopes asked this run: a store is looked for once.
    private var migrations: Set<String> = []
    /// Moves the cookies of the profile's old store, if one is left, into `target` (the frame's partition), then deletes the
    /// old store (`WKWebsiteDataStore.remove(forIdentifier:)`), so this happens once. Local storage, IndexedDB and the cache
    /// stay behind: WebKit gives no call that reads them out of a store. `done` gets the number of cookies moved.
    func migrateCookies(environment: String, profile: String, into target: WKWebsiteDataStore, done: @escaping (Int) -> Void) {
        guard #available(macOS 14.0, *), migrations.insert("\(environment)\u{0}\(profile)").inserted else { return done(0) }
        let identifier = Self.oldStoreIdentifier(environment: environment, profile: profile)
        WKWebsiteDataStore.fetchAllDataStoreIdentifiers { [weak self] identifiers in
            guard identifiers.contains(identifier) else { return done(0) }
            // The store and its cookie store are released in this pool, so the old store can be deleted after.
            var old: WKWebsiteDataStore? = WKWebsiteDataStore(forIdentifier: identifier)
            autoreleasepool {
                old?.httpCookieStore.getAllCookies { cookies in
                    let group = DispatchGroup()
                    for cookie in cookies { group.enter(); target.httpCookieStore.setCookie(cookie) { group.leave() } }
                    group.notify(queue: .main) {
                        old = nil
                        Self.removeStore(identifier, attempts: 10) { error in
                            self?.note("profile store moved: \(cookies.count) cookies\(error.map { ", the old store kept: \($0.localizedDescription)" } ?? "")")
                            done(cookies.count)
                        }
                    }
                }
            }
        }
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
    /// separate window, the recording, and the cookies moved in from the profile's old store.
    var status: [String: Any] {
        ["browserPages": pages.mapValues { page -> [String: Any] in
            ["size": [Double(page.web?.frame.width ?? 0), Double(page.web?.frame.height ?? 0)], "pip": page.pip != nil, "recording": page.recording.report, "migrated": page.migrated]
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

    // MARK: Artifacts (Manager.ts captureScreenshot's file, revealArtifact)

    /// `adopt` moves a capture the frame wrote under `app:/tmp/browser-shots` into the artifact directory as
    /// `browser-screenshot-<site>-<millis36>-<8 hex>.png` (the first eight characters of a v4 UUID, so two captures in one
    /// millisecond never share a file), and an agent's finished download under `app:/tmp/browser-downloads` as
    /// `browser-download-<millis36>-<8 hex>-<its name>`. `reveal`, `stage` and `copy-path` act on a file inside the
    /// directory only. `stage` copies one under `app:/tmp/copy-image` for Copy image (copyArtifactToClipboard,
    /// browser-capture.ts): data code reads only the app's own files, and the directory is the local server's.
    private func artifact(_ request: [String: Any], answer: ([String: Any]) -> Void, refuse: (String, String) -> Void) {
        if request["action"] as? String == "adopt" {
            let raw = request["path"] as? String ?? "", root = temporaryRoot?.standardizedFileURL.path ?? "/nonexistent"
            let relative = raw.hasPrefix("app:/tmp/") ? String(raw.dropFirst(9)) : raw.hasPrefix(root + "/") ? String(raw.dropFirst(root.count + 1)) : ""
            let download = relative.hasPrefix("browser-downloads/")
            guard let temporary = temporaryRoot, relative.hasPrefix("browser-shots/") || download, !relative.contains("..") else { return refuse("ArtifactPath", "The capture is not one of the frame's.") }
            let source = temporary.appendingPathComponent(relative)
            let now = Date(), stamp = "\(T3BrowserArtifacts.millis36(now))-\(UUID().uuidString.lowercased().prefix(8))"
            let name = source.lastPathComponent.replacingOccurrences(of: "[/:]", with: "-", options: .regularExpression)
            let id = download ? "browser-download-\(stamp)-\(name)" : "browser-screenshot-\(T3BrowserArtifacts.slug(request["url"] as? String))-\(stamp)"
            let target = artifactDirectory.appendingPathComponent(download ? id : "\(id).png")
            do {
                try FileManager.default.createDirectory(at: artifactDirectory, withIntermediateDirectories: true)
                try FileManager.default.moveItem(at: source, to: target)
            } catch { return refuse(download ? "Download" : "Screenshot", "The \(download ? "download" : "screenshot") could not be saved: \(error.localizedDescription)") }
            let size = (try? FileManager.default.attributesOfItem(atPath: target.path)[.size] as? Int) ?? 0
            note("\(download ? "download" : "screenshot") \(target.lastPathComponent)")
            return answer(["id": id, "path": target.path, "mimeType": download ? "" : "image/png", "sizeBytes": size, "createdAt": T3BrowserArtifacts.iso(now)])
        }
        guard let target = T3BrowserArtifacts.resolve(request["path"] as? String ?? "", in: artifactDirectory) else {
            return refuse("ArtifactPath", "Preview artifact path is outside \(artifactDirectory.path)")
        }
        let pasteboard = T3BrowserArtifacts.pasteboard(agent: agent)
        switch request["action"] as? String {
        case "stage":
            guard let temporary = temporaryRoot else { return refuse("ArtifactPath", "The app's files are not ready.") }
            let directory = temporary.appendingPathComponent("copy-image", isDirectory: true)
            let name = "artifact-\(UUID().uuidString.lowercased())" + (target.pathExtension.isEmpty ? "" : ".\(target.pathExtension)")
            do {
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                try FileManager.default.copyItem(at: target, to: directory.appendingPathComponent(name))
            } catch { return refuse("ArtifactPath", "The artifact could not be read: \(error.localizedDescription)") }
            answer(["path": "app:/tmp/copy-image/\(name)"])
        case "reveal":
            note("artifact reveal \(target.lastPathComponent)")
            if !agent { NSWorkspace.shared.activateFileViewerSelecting([target]) } // an agent run records it and opens no Finder window
            answer(["revealed": target.path])
        default:
            pasteboard.clearContents()
            answer(["copied": pasteboard.setString(target.path, forType: .string)])
        }
    }

    // MARK: A saved recording's upload (uploadBrowserRecording's transfer: the bytes, after the data module's createUploadUrl)

    /// `browserRecordingUpload` POSTs a saved recording (a file in the artifact directory, which the data module cannot
    /// read) to `relativeUrl` on `origin`, the upload URL the data module minted on its connection, unauthenticated as the
    /// reference's upload is. It answers `{status: "uploaded"}` or `{status: "failed", error, expired}`. The transfer does not
    /// start with a second or less left before `deadlineMs` and gives up then (AbortSignal.timeout(remainingMs)); `expired`
    /// is the reference's `Date.now() >= deadlineMs - 1_000` after a failure: the module keeps the time.
    private func recordingUpload(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let deadline = Date(timeIntervalSince1970: ((request["deadlineMs"] as? NSNumber)?.doubleValue ?? 0) / 1000)
        var settled = false
        let finish = { (error: String?) in
            guard !settled else { return }
            settled = true
            reply(error.map { ["status": "failed", "error": $0, "expired": Date() >= deadline.addingTimeInterval(-1)] } ?? ["status": "uploaded"])
        }
        guard let file = T3BrowserArtifacts.resolve(request["path"] as? String ?? "", in: artifactDirectory), let data = try? Data(contentsOf: file), !data.isEmpty else {
            return finish("The saved recording is missing.")
        }
        let relative = request["relativeUrl"] as? String ?? ""
        guard relative.hasPrefix("/api/attachments/upload/"), let origin = URL(string: request["origin"] as? String ?? ""), ["http", "https"].contains(origin.scheme ?? ""),
              let url = URL(string: relative, relativeTo: origin)?.absoluteURL, url.host == origin.host, url.port == origin.port else {
            return finish("The upload could not be prepared.")
        }
        let remaining = deadline.timeIntervalSinceNow - 1
        guard remaining > 0 else { return finish("Recording transfer deadline expired.") }
        var post = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: remaining)
        post.httpMethod = "POST"; post.httpBody = data
        post.setValue(request["mimeType"] as? String ?? "video/mp4", forHTTPHeaderField: "Content-Type")
        post.setValue("application/json", forHTTPHeaderField: "Accept")
        post.setValue("2", forHTTPHeaderField: "x-t3-orchestration-protocol")
        let session = URLSession(configuration: .ephemeral, delegate: T3UploadRedirects(), delegateQueue: nil)
        let task = session.dataTask(with: post) { body, response, error in
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            let message = body.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }?["message"] as? String
            DispatchQueue.main.async {
                if let error { return finish(error.localizedDescription) }
                finish((200..<300).contains(status) ? nil : message ?? "The server returned HTTP \(status).")
            }
        }
        task.resume()
        session.finishTasksAndInvalidate()
        DispatchQueue.main.asyncAfter(deadline: .now() + remaining) { task.cancel(); finish("Recording transfer deadline expired.") }
    }
}

/// A redirect would carry the recording to another host: the upload stays on the server that minted it.
private final class T3UploadRedirects: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(nil)
    }
}
#endif
