#if os(macOS)
import AppKit
import WebKit

/// browser-surface part 3 (capture): a tab's artifacts, held pages, the separate preview window and downloads (X1
/// path B). MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975: apps/desktop/src/preview/Manager.ts (`artifactSiteSlug`,
/// `captureScreenshot`, `saveRecording`, `resolveArtifactPath`, `revealArtifact`, `copyArtifactToClipboard`,
/// `installDownloadHandler`, `openPictureInPicture`, `fitPictureInPictureContentSize`, the PICTURE_IN_PICTURE_* and
/// RECORDING_ARM_GRACE_MS constants), apps/desktop/src/app/DesktopEnvironment.ts (`browserArtifactsDir`) and
/// apps/web/src/browser/browserSurfaceStore.ts (`acquireBrowserSurfaceActivity`).
///
/// - Artifacts live where the reference keeps them, `<T3 home>/userdata/browser-artifacts`, the embedded server's own
///   state directory, so its Storage cleanup ("Browser artifacts", `browserArtifactsAfterDays`) prunes them. A build that
///   runs no local server keeps them in its data root.
/// - Frames come from `WKWebView.takeSnapshot` (the frame-source decision of 2026-10-09: no Screen Recording permission,
///   the page's own pixels). A page in no window is throttled to about one frame a second, so a page a recording or the
///   separate window needs while no view shows it is held in an offscreen host window (T3BrowserParking), as the
///   reference keeps a hidden surface painting while its activity is held.
/// - The separate preview window is a floating panel showing the page at about 12 frames a second; the page itself stays
///   where it is (one WKWebView sits in one place). The reference ships JPEG frames (quality 80) to a second renderer;
///   here the snapshots are drawn directly.
enum T3BrowserArtifacts {
    static let maxSlugLength = 80

    /// DesktopEnvironment `browserArtifactsDir`: the local server's `<T3 home>/userdata/browser-artifacts`, else the data root's.
    static func directory(dataRoot: URL?, environment: [String: String] = ProcessInfo.processInfo.environment, resources: URL? = Bundle.main.resourceURL) -> URL {
        let policy = T3LocalPolicy.resolve(env: environment, packaged: T3LocalPolicy.packaged(resources: resources), home: NSHomeDirectory(),
                                           accountHome: String(cString: getpwuid(getuid()).pointee.pw_dir))
        if case let .allowed(home, _, _, _) = policy { return home.appendingPathComponent("userdata", isDirectory: true).appendingPathComponent("browser-artifacts", isDirectory: true) }
        return (dataRoot ?? FileManager.default.temporaryDirectory).appendingPathComponent("browser-artifacts", isDirectory: true)
    }

    /// artifactSiteSlug: the page's host, lowercased, runs of other characters as one dash, at most 80 characters.
    static func slug(_ raw: String?) -> String {
        guard let raw, let host = URL(string: raw)?.host?.lowercased(), !host.isEmpty else { return "site" }
        var slug = host.replacingOccurrences(of: "[^a-z0-9]+", with: "-", options: .regularExpression).trimmingCharacters(in: CharacterSet(charactersIn: "-"))
        slug = String(slug.prefix(maxSlugLength))
        while slug.hasSuffix("-") { slug.removeLast() }
        return slug.isEmpty ? "site" : slug
    }
    static func millis36(_ date: Date) -> String { String(Int64(date.timeIntervalSince1970 * 1000), radix: 36) }
    static func iso(_ date: Date) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.string(from: date)
    }
    /// recordingFileExtension: the subtype without `x-` and punctuation, else "video".
    static func fileExtension(_ mimeType: String) -> String {
        let subtype = mimeType.split(separator: ";").first.map { $0.trimmingCharacters(in: .whitespaces).lowercased() } ?? ""
        let name = subtype.split(separator: "/").dropFirst().first.map(String.init) ?? ""
        let cleaned = (name.hasPrefix("x-") ? String(name.dropFirst(2)) : name).replacingOccurrences(of: "[^a-z0-9]", with: "", options: .regularExpression)
        return cleaned.isEmpty ? "video" : cleaned
    }
    /// resolveArtifactPath: a path inside the artifact directory (symbolic links resolved), else nil.
    static func resolve(_ path: String, in directory: URL) -> URL? {
        guard path.hasPrefix("/") else { return nil }
        let target = URL(fileURLWithPath: path).standardizedFileURL.resolvingSymlinksInPath()
        let root = directory.standardizedFileURL.resolvingSymlinksInPath().path
        return target.path.hasPrefix(root.hasSuffix("/") ? root : root + "/") ? target : nil
    }

    /// The image as PNG at its own pixel size: captureScreenshot saves capturePage's image as it is (MAX_SCREENSHOT_WIDTH
    /// applies to the automation snapshot only, T3BrowserAutomation).
    static func png(_ image: NSImage) -> (data: Data, width: Int, height: Int)? {
        guard let source = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return nil }
        let rep = NSBitmapImageRep(cgImage: source)
        guard let data = rep.representation(using: .png, properties: [:]) else { return nil }
        return (data, source.width, source.height)
    }

    /// The pasteboard the artifact actions write: an agent run never touches the person's clipboard.
    static func pasteboard(agent: Bool) -> NSPasteboard { agent ? NSPasteboard(name: NSPasteboard.Name("com.exact.t3code.browser-agent")) : .general }
}

/// Pages that must keep painting while no view shows them (acquireBrowserSurfaceActivity): a recording, the separate
/// window. A held page whose view lets it go moves into one offscreen, transparent, mouse-ignoring host window instead of
/// having none; a view that shows it again takes it back. Occlusion detection is off while a page is parked (the host
/// window is off screen), as agent runs keep it off for every page.
final class T3BrowserParking {
    private var window: NSWindow?
    private var holds: [ObjectIdentifier: Set<String>] = [:]
    private(set) var parked: Set<String> = []

    func held(_ session: T3BrowserSession) -> Bool { !(holds[ObjectIdentifier(session)] ?? []).isEmpty }
    func reasons(_ session: T3BrowserSession) -> [String] { Array(holds[ObjectIdentifier(session)] ?? []).sorted() }

    func hold(_ session: T3BrowserSession, _ reason: String) {
        holds[ObjectIdentifier(session), default: []].insert(reason)
        if session.web.window == nil { park(session) }
    }

    func release(_ session: T3BrowserSession, _ reason: String) {
        let key = ObjectIdentifier(session)
        holds[key]?.remove(reason)
        if holds[key]?.isEmpty == true { holds[key] = nil }
        if !held(session), session.web.superview === window?.contentView { unpark(session) }
    }

    /// A view let go of the page: a held one keeps painting here.
    func viewReleased(_ session: T3BrowserSession) { if held(session), session.web.window == nil { park(session) } }
    /// A view took the page (it left the host window by joining the view).
    func viewTook(_ session: T3BrowserSession) {
        guard parked.remove(session.id) != nil else { return }
        session.keepPainting(false)
    }

    func forget(_ session: T3BrowserSession) {
        holds[ObjectIdentifier(session)] = nil
        if session.web.superview === window?.contentView { unpark(session) }
    }

    private func park(_ session: T3BrowserSession) {
        let host = hostWindow()
        let size = session.web.frame.size.width > 1 && session.web.frame.size.height > 1 ? session.web.frame.size : NSSize(width: 1024, height: 768)
        session.web.removeFromSuperview()
        session.web.autoresizingMask = []
        session.web.frame = NSRect(origin: .zero, size: size)
        if host.frame.width < size.width || host.frame.height < size.height { host.setContentSize(NSSize(width: max(host.frame.width, size.width), height: max(host.frame.height, size.height))) }
        host.contentView?.addSubview(session.web)
        session.keepPainting(true)
        parked.insert(session.id)
        FileHandle.standardError.write(Data("t3.browser: park \(session.id.prefix(200))\n".utf8))
    }

    private func unpark(_ session: T3BrowserSession) {
        session.web.removeFromSuperview()
        session.keepPainting(false)
        parked.remove(session.id)
        FileHandle.standardError.write(Data("t3.browser: unpark \(session.id.prefix(200))\n".utf8))
    }

    private func hostWindow() -> NSWindow {
        if let window { return window }
        let host = NSWindow(contentRect: NSRect(x: -30_000, y: -30_000, width: 1024, height: 768), styleMask: [.borderless], backing: .buffered, defer: false)
        host.isReleasedWhenClosed = false
        host.alphaValue = 0
        host.ignoresMouseEvents = true
        host.isExcludedFromWindowsMenu = true
        host.collectionBehavior = [.transient, .ignoresCycle, .stationary]
        host.setAccessibilityElement(false)
        host.contentView = NSView(frame: NSRect(x: 0, y: 0, width: 1024, height: 768))
        host.orderFrontRegardless()
        window = host
        return host
    }

    func close() { window?.close(); window = nil; holds.removeAll(); parked.removeAll() }
}

/// The separate preview window (Manager.ts openPictureInPicture): a floating, non-activating panel on every space,
/// 480×320 at first, at least 240×160, never minimized or zoomed, its content following the page's aspect ratio, showing
/// the page at about 12 frames a second until it is closed.
final class T3BrowserPictureInPicture: NSObject, NSWindowDelegate {
    static let initialSize = NSSize(width: 480, height: 320)
    static let minimumSize = NSSize(width: 240, height: 160)
    static let frameIntervalMs = Int((1000.0 / 12).rounded(.up))
    static let aspectEpsilon = 0.002

    let window: NSPanel
    private let imageView = NSImageView()
    private weak var web: WKWebView?
    private var timer: Timer?
    private var inFlight = false
    private var aspect: Double?
    private(set) var frames = 0
    var closed: (() -> Void)?

    init(web: WKWebView, title: String, agent: Bool) {
        self.web = web
        window = NSPanel(contentRect: NSRect(origin: .zero, size: Self.initialSize), styleMask: [.titled, .closable, .resizable, .nonactivatingPanel], backing: .buffered, defer: false)
        super.init()
        window.title = title.isEmpty ? "Browser preview" : "Preview · \(title)"
        window.isReleasedWhenClosed = false
        window.isFloatingPanel = true
        window.level = .floating
        window.hidesOnDeactivate = false
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        window.contentMinSize = Self.minimumSize
        window.backgroundColor = NSColor(srgbRed: 0x11 / 255, green: 0x11 / 255, blue: 0x11 / 255, alpha: 1)
        window.standardWindowButton(.miniaturizeButton)?.isHidden = true
        window.standardWindowButton(.zoomButton)?.isHidden = true
        window.delegate = self
        imageView.imageScaling = .scaleProportionallyUpOrDown
        imageView.imageAlignment = .alignCenter
        imageView.setAccessibilityLabel("Live browser preview")
        imageView.frame = NSRect(origin: .zero, size: Self.initialSize)
        imageView.autoresizingMask = [.width, .height]
        window.contentView = imageView
        window.center()
        // BrowserWindow.showInactive: shown without taking the focus. An agent run keeps it off screen.
        if agent { window.setFrameOrigin(NSPoint(x: -30_000, y: -30_000)) }
        window.orderFrontRegardless()
        let tick = Timer(timeInterval: Double(Self.frameIntervalMs) / 1000, repeats: true) { [weak self] _ in self?.capture() }
        RunLoop.main.add(tick, forMode: .common)
        timer = tick
        capture()
    }

    var image: NSImage? { imageView.image }

    private func capture() {
        guard let web, !inFlight else { return }
        inFlight = true
        web.takeSnapshot(with: nil) { [weak self] image, _ in
            guard let self else { return }
            self.inFlight = false
            guard let image, image.size.width > 0, image.size.height > 0, self.timer != nil else { return }
            self.imageView.image = image
            self.frames += 1
            self.follow(aspect: Double(image.size.width / image.size.height))
        }
    }

    /// The window follows the page's aspect ratio, keeping its area (fitPictureInPictureContentSize).
    private func follow(aspect next: Double) {
        if let aspect, abs(aspect - next) <= Self.aspectEpsilon { return }
        aspect = next
        let current = window.contentRect(forFrameRect: window.frame).size
        let fitted = Self.fit(current: current, aspect: next)
        window.contentAspectRatio = NSSize(width: next, height: 1)
        window.setContentSize(NSSize(width: fitted.width, height: fitted.height))
    }

    /// fitPictureInPictureContentSize.
    static func fit(current: NSSize, aspect: Double) -> (width: Double, height: Double) {
        let width0 = max(1, Double(current.width)), height0 = max(1, Double(current.height))
        var width = (width0 * height0 * aspect).squareRoot()
        var height = width / aspect
        let scale = max(1, Double(minimumSize.width) / width, Double(minimumSize.height) / height)
        width *= scale; height *= scale
        return (width.rounded(), height.rounded())
    }

    func close() { window.close() }
    func windowWillClose(_ notification: Notification) {
        timer?.invalidate(); timer = nil
        let done = closed; closed = nil
        done?()
    }
}

/// Downloads a page starts (Manager.ts installDownloadHandler): the reference puts a download from a page the agent drove
/// into the artifact directory (`browser-download-<id>-<name>`) and lets a person's download ask where with the save
/// dialog. Pages become agent-driven with part 5 (automation); until then a person's download asks with a save panel and
/// an agent run, which shows nothing modal, saves into the artifact directory.
final class T3BrowserDownloads: NSObject, WKDownloadDelegate {
    weak var session: T3BrowserSession?
    var directory: () -> URL = { FileManager.default.temporaryDirectory }
    var agentDriven = false
    var changed: (() -> Void)?
    private var count = 0
    private(set) var entries: [[String: Any]] = []
    private var destinations: [ObjectIdentifier: URL] = [:]

    func adopt(_ download: WKDownload) { download.delegate = self }

    func download(_ download: WKDownload, decideDestinationUsing response: URLResponse, suggestedFilename: String, completionHandler: @escaping (URL?) -> Void) {
        let name = (suggestedFilename as NSString).lastPathComponent.isEmpty ? "download" : (suggestedFilename as NSString).lastPathComponent
        let toArtifacts = agentDriven || session?.dialogs == false
        if toArtifacts {
            let folder = directory()
            try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            // The start time keeps names unique across restarts; the count keeps two same-name downloads apart.
            let id = "\(T3BrowserArtifacts.millis36(Date()))-\(String(count, radix: 36))"
            count += 1
            let target = folder.appendingPathComponent("browser-download-\(id)-\(name)")
            return accept(download, target, name: name, completionHandler)
        }
        let panel = NSSavePanel()
        panel.nameFieldStringValue = name
        panel.directoryURL = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
        let finish: (NSApplication.ModalResponse) -> Void = { [weak self] result in
            guard let self, result == .OK, let url = panel.url else { self?.record(name: name, path: "", state: "cancelled"); return completionHandler(nil) }
            try? FileManager.default.removeItem(at: url) // the panel already confirmed a replacement
            self.accept(download, url, name: name, completionHandler)
        }
        if let window = session?.web.window { panel.beginSheetModal(for: window, completionHandler: finish) } else { finish(panel.runModal()) }
    }

    private func accept(_ download: WKDownload, _ target: URL, name: String, _ completionHandler: @escaping (URL?) -> Void) {
        destinations[ObjectIdentifier(download)] = target
        record(name: name, path: target.path, state: "downloading")
        FileHandle.standardError.write(Data("t3.browser: download \(target.lastPathComponent.prefix(200))\n".utf8))
        completionHandler(target)
    }

    func downloadDidFinish(_ download: WKDownload) { finish(download, state: "done") }
    func download(_ download: WKDownload, didFailWithError error: Error, resumeData: Data?) { finish(download, state: "failed") }

    private func finish(_ download: WKDownload, state: String) {
        guard let target = destinations.removeValue(forKey: ObjectIdentifier(download)) else { return }
        if let index = entries.lastIndex(where: { $0["path"] as? String == target.path }) { entries[index]["state"] = state }
        changed?()
    }

    private func record(name: String, path: String, state: String) {
        entries.append(["name": String(name.prefix(200)), "path": path, "state": state])
        if entries.count > 8 { entries.removeFirst(entries.count - 8) }
        changed?()
    }
}

// MARK: - The capture ops (T3Module+Browser.swift routes them)

extension T3BrowserSessions {
    /// Part 3's ops: annotation, recording, screenshots, artifact actions and the separate window. Replies through `reply`
    /// (some only once a snapshot or the encoder answers); false for any other op.
    func performCapture(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) -> Bool {
        let generation = request["generation"] as? Int ?? 0
        let answer = { (value: [String: Any]) in reply(["ok": true, "generation": generation, "value": value]) }
        let refuse = { (kind: String, message: String) in reply(["ok": false, "generation": generation, "error": ["kind": kind, "message": message, "uncertain": false]]) }
        let op = request["op"] as? String ?? ""
        guard ["browserAnnotate", "browserRecord", "browserScreenshot", "browserArtifact", "browserPip"].contains(op) else { return false }
        if op == "browserArtifact" { artifactAction(request, answer: answer, refuse: refuse); return true }
        guard let session = sessions[request["tab"] as? String ?? ""] else { refuse("NotFound", "That browser tab is not open."); return true }
        switch op {
        case "browserAnnotate": annotate(session, request, answer: answer)
        case "browserRecord": record(session, request, answer: answer, refuse: refuse)
        case "browserScreenshot": screenshot(session, answer: answer, refuse: refuse)
        default: pictureInPicture(session, open: request["action"] as? String != "close", answer: answer)
        }
        return true
    }

    private func annotate(_ session: T3BrowserSession, _ request: [String: Any], answer: ([String: Any]) -> Void) {
        let annotation = session.annotation
        switch request["action"] as? String {
        case "start":
            annotation.start(theme: (request["theme"] as? [String: Any] ?? [:]).compactMapValues { $0 as? String })
            note("annotate start \(session.id)")
            answer(annotation.report)
        case "cancel":
            annotation.cancel()
            answer(annotation.report)
        case "applied":
            note("annotate applied \(request["serial"] as? Int ?? 0) \(String((request["outcome"] as? String ?? "").prefix(60)))")
            answer([:])
        default:
            answer(["result": annotation.take(serial: request["serial"] as? Int ?? -1) ?? NSNull(), "serial": annotation.serial])
        }
    }

    private func record(_ session: T3BrowserSession, _ request: [String: Any], answer: @escaping ([String: Any]) -> Void, refuse: @escaping (String, String) -> Void) {
        let recording = session.recording
        let failed = { (error: Error) in refuse("Recording", error.localizedDescription) }
        switch request["action"] as? String {
        case "arm":
            parking.hold(session, "recording")
            note("record arm \(session.id)")
            let theme = (request["theme"] as? [String: Any] ?? [:]).compactMapValues { $0 as? String }
            recording.arm(showKeyPresses: request["showKeyPresses"] as? Bool ?? false, showMousePresses: request["showMousePresses"] as? Bool ?? false, theme: theme,
                          controller: request["controller"] as? String ?? "none") { [weak self, weak session] error in
                guard let self, let session else { return }
                if let error { self.parking.release(session, "recording"); return failed(error) }
                session.armGrace { [weak self, weak session] in
                    // RECORDING_ARM_GRACE_MS: an armed tab that never started recording lets go of its lease.
                    guard let self, let session, !session.recording.recording else { return }
                    session.recording.cancel()
                    self.parking.release(session, "recording")
                    self.note("record grace-expired \(session.id)")
                }
                answer(["armed": true])
            }
        case "disarm":
            session.armGrace(nil)
            recording.disarm()
            if !recording.capturing { parking.release(session, "recording") }
            note("record disarm \(session.id)")
            answer(["armed": false])
        case "capture":
            recording.capture(frameRate: request["frameRate"] as? Int ?? 30) { result in
                switch result { case .success(let value): answer(value); case .failure(let error): failed(error) }
            }
        case "release":
            recording.releaseCapture()
            if !recording.armed { parking.release(session, "recording") }
            answer(["capturing": false])
        case "decorate":
            let active = recording.useDecorations(showKeyPresses: request["showKeyPresses"] as? Bool ?? false, showMousePresses: request["showMousePresses"] as? Bool ?? false,
                                                  primaryColor: request["primaryColor"] as? String ?? "#2563eb")
            answer(["active": active])
        case "begin":
            do {
                session.armGrace(nil)
                let mimeType = try recording.begin(mimeType: request["mimeType"] as? String, bitsPerSecond: request["bitsPerSecond"] as? Int ?? 3_110_400)
                note("record begin \(session.id)")
                answer(["mimeType": mimeType])
            } catch { failed(error) }
        case "finish":
            recording.finish { [weak session] result in
                switch result {
                case .success(let value): session?.encodedPath = value["path"] as? String; answer(value)
                case .failure(let error): failed(error)
                }
            }
        case "save":
            saveRecording(session, request, answer: answer, refuse: refuse)
        default:
            session.armGrace(nil)
            recording.cancel()
            parking.release(session, "recording")
            note("record cancel \(session.id)")
            answer([:])
        }
    }

    /// Manager.ts saveRecording: the encoded file moved into the artifact directory as `browser-recording-<millis36>.<ext>`.
    private func saveRecording(_ session: T3BrowserSession, _ request: [String: Any], answer: ([String: Any]) -> Void, refuse: (String, String) -> Void) {
        let source = URL(fileURLWithPath: request["path"] as? String ?? "")
        let mimeType = request["mimeType"] as? String ?? "video/mp4"
        // Only the file this tab's encoder finished, once.
        guard let encoded = session.encodedPath, encoded == source.path, FileManager.default.fileExists(atPath: source.path) else { return refuse("Recording", "The encoded recording is missing.") }
        session.encodedPath = nil
        let now = Date(), id = "browser-recording-\(T3BrowserArtifacts.millis36(now))"
        let folder = artifactDirectory, target = folder.appendingPathComponent("\(id).\(T3BrowserArtifacts.fileExtension(mimeType))")
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try? FileManager.default.removeItem(at: target)
            try FileManager.default.moveItem(at: source, to: target)
        } catch { return refuse("Recording", "The recording could not be saved: \(error.localizedDescription)") }
        let size = (try? FileManager.default.attributesOfItem(atPath: target.path)[.size] as? Int) ?? 0
        note("record saved \(target.lastPathComponent)")
        answer(["id": id, "tabId": session.id, "path": target.path, "mimeType": mimeType, "sizeBytes": size, "createdAt": T3BrowserArtifacts.iso(now)])
    }

    /// Manager.ts captureScreenshot: `browser-screenshot-<site>-<millis36>.png`, the snapshot's own pixels (capturePage's size).
    private func screenshot(_ session: T3BrowserSession, answer: @escaping ([String: Any]) -> Void, refuse: @escaping (String, String) -> Void) {
        let url = session.web.url?.absoluteString
        var attempts = 0
        func attempt() {
            attempts += 1
            session.web.takeSnapshot(with: nil) { [weak self] image, error in
                guard let self else { return }
                guard let image, let png = T3BrowserArtifacts.png(image) else {
                    // CAPTURE_PAGE_RETRY_ATTEMPTS 3, 120 ms apart.
                    if attempts < 3 { return DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(120)) { attempt() } }
                    return refuse("Screenshot", error?.localizedDescription ?? "The page could not be captured.")
                }
                let now = Date(), id = "browser-screenshot-\(T3BrowserArtifacts.slug(url))-\(T3BrowserArtifacts.millis36(now))"
                let folder = self.artifactDirectory, target = folder.appendingPathComponent("\(id).png")
                do {
                    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
                    try png.data.write(to: target)
                } catch { return refuse("Screenshot", "The screenshot could not be saved: \(error.localizedDescription)") }
                self.note("screenshot \(target.lastPathComponent)")
                answer(["id": id, "tabId": session.id, "path": target.path, "mimeType": "image/png", "sizeBytes": png.data.count, "createdAt": T3BrowserArtifacts.iso(now),
                        "width": png.width, "height": png.height])
            }
        }
        attempt()
    }

    /// revealArtifact, copyArtifactToClipboard and the toasts' Copy path, for a file inside the artifact directory only.
    private func artifactAction(_ request: [String: Any], answer: ([String: Any]) -> Void, refuse: (String, String) -> Void) {
        guard let target = T3BrowserArtifacts.resolve(request["path"] as? String ?? "", in: artifactDirectory) else {
            return refuse("ArtifactPath", "Preview artifact path is outside \(artifactDirectory.path)")
        }
        let pasteboard = T3BrowserArtifacts.pasteboard(agent: agent)
        switch request["action"] as? String {
        case "reveal":
            note("artifact reveal \(target.lastPathComponent)")
            if !agent { NSWorkspace.shared.activateFileViewerSelecting([target]) } // an agent run records it (`browserLog`) and opens no Finder window
            answer(["revealed": target.path])
        case "copy-image":
            guard let image = NSImage(contentsOf: target), let tiff = image.tiffRepresentation, let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else {
                return refuse("ArtifactImage", "The screenshot could not be read.")
            }
            pasteboard.clearContents()
            pasteboard.setData(png, forType: .png)
            note("artifact copy-image \(target.lastPathComponent)")
            answer(["copied": true])
        default:
            pasteboard.clearContents()
            let copied = pasteboard.setString(target.path, forType: .string)
            note("artifact copy-path \(target.lastPathComponent)")
            answer(["copied": copied])
        }
    }

    private func pictureInPicture(_ session: T3BrowserSession, open: Bool, answer: ([String: Any]) -> Void) {
        if !open {
            session.pip?.close()
            return answer(["open": false])
        }
        if let pip = session.pip { pip.window.orderFrontRegardless(); return answer(["open": true]) }
        let pip = T3BrowserPictureInPicture(web: session.web, title: (session.web.title ?? "").trimmingCharacters(in: .whitespacesAndNewlines), agent: agent)
        pip.closed = { [weak self, weak session] in
            guard let self, let session else { return }
            session.pip = nil
            self.parking.release(session, "pip")
            self.note("pip close \(session.id)")
            self.publish()
        }
        session.pip = pip
        parking.hold(session, "pip")
        note("pip open \(session.id)")
        publish()
        answer(["open": true])
    }
}
#endif
