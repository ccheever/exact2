#if os(macOS)
import AppKit
import WebKit

/// A Browser tab's artifacts and its separate preview window (T3BrowserPage.swift uses both). MIT reference, see
/// LICENSE-T3, T3 Code 2359643cff: apps/desktop/src/preview/Manager.ts (`artifactSiteSlug`, `resolveArtifactPath`,
/// `openPictureInPicture`, `fitPictureInPictureContentSize`, the PICTURE_IN_PICTURE_* constants) and
/// apps/desktop/src/app/DesktopEnvironment.ts (`browserArtifactsDir`).
///
/// - Artifacts live where the reference keeps them, `<T3 home>/userdata/browser-artifacts`, the embedded server's own
///   state directory, so its Storage cleanup ("Browser artifacts", `browserArtifactsAfterDays`) prunes them. A build that
///   runs no local server keeps them in its data root.
/// - The separate preview window is a floating panel showing the page at about 12 frames a second
///   (`WKWebView.takeSnapshot`); the page itself stays in its frame. The reference ships JPEG frames (quality 80) to a
///   second renderer; here the snapshots are drawn directly.
enum T3BrowserArtifacts {
    static let maxSlugLength = 80

    /// DesktopEnvironment `browserArtifactsDir`: the local server's `<T3 home>/userdata/browser-artifacts`, else the data root's.
    static func directory(dataRoot: URL?, environment: [String: String] = ProcessInfo.processInfo.environment, bundleInfo: [String: Any] = Bundle.main.infoDictionary ?? [:]) -> URL {
        let policy = T3LocalPolicy.resolve(env: environment, packaged: T3LocalPolicy.packaged(info: bundleInfo), home: NSHomeDirectory(),
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

    /// The pasteboard the artifact actions write: an agent run never touches the person's clipboard.
    static func pasteboard(agent: Bool) -> NSPasteboard { agent ? NSPasteboard(name: NSPasteboard.Name("com.exact.t3code.browser-agent")) : .general }
}

/// The separate preview window (Manager.ts openPictureInPicture): a floating, non-activating panel on every space,
/// 480×320 at first, at least 240×160, never minimized or zoomed, its content following the page's aspect ratio, showing
/// the page at about 12 frames a second until it is closed.
final class T3BrowserPictureInPicture: NSObject, NSWindowDelegate {
    static let initialSize = NSSize(width: 480, height: 320)
    static let minimumSize = NSSize(width: 240, height: 160)
    static let frameIntervalMs = Int((1000.0 / 12).rounded(.up))
    static let aspectEpsilon = 0.002
    /// CAPTURE_PAGE_ATTEMPT_TIMEOUT_MS: a snapshot that has not answered by then is given up, so the next tick captures.
    static let captureTimeoutMs = 1_000

    let window: NSPanel
    private let imageView = NSImageView()
    private weak var web: WKWebView?
    private var timer: Timer?
    private var inFlight = false
    private var attempt = 0
    private var aspect: Double?
    private(set) var frames = 0
    typealias Snapshot = (WKWebView, @escaping (NSImage?) -> Void) -> Void
    /// WKWebView.takeSnapshot, which a test replaces with one that never answers.
    private let snapshot: Snapshot
    var closed: (() -> Void)?

    init(web: WKWebView, title: String, agent: Bool, snapshot: @escaping Snapshot = { web, done in web.takeSnapshot(with: nil) { image, _ in done(image) } }) {
        self.web = web
        self.snapshot = snapshot
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
        attempt += 1
        let current = attempt
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(Self.captureTimeoutMs)) { [weak self] in
            guard let self, self.attempt == current, self.inFlight else { return }
            self.inFlight = false
        }
        snapshot(web) { [weak self] image in
            // An answer after its attempt timed out is dropped: a newer attempt owns the frame.
            guard let self, self.attempt == current, self.inFlight else { return }
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

#endif
