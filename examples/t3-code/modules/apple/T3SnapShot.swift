// App-owned desktop capture boundary. No permission grants occur during state reads.
import AppKit
import ApplicationServices
import Foundation
import ImageIO

/// The macOS grants, prompts and System Settings as snapshot setup and
/// requestPermissions reach them (reference getMediaAccessStatus,
/// isTrustedAccessibilityClient, desktopCapturer.getSources, shell.openExternal).
/// The defaults are the real system; the snapshot AppKit test replaces every one,
/// so it never reaches TCC. The setup rows (snapshotState) and the helper's grant
/// poll read the same two checks, as the reference's currentMacPermissions and
/// permissionGranted read the same Electron calls.
struct T3SnapshotPermissionSystem {
    var screenRecording: () -> Bool = { CGPreflightScreenCaptureAccess() }
    var accessibility: () -> Bool = { AXIsProcessTrusted() }
    /// isTrustedAccessibilityClient(true): the system prompt while the grant is missing.
    var promptAccessibility: () -> Bool = { AXIsProcessTrustedWithOptions([kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary) }
    /// desktopCapturer.getSources: the system prompt the first time.
    var requestScreenRecording: () -> Bool = { CGRequestScreenCaptureAccess() }
    var openSettings: (URL) -> Void = { _ = NSWorkspace.shared.open($0) }
    var focusedWindow: () -> NSWindow? = { NSApp.keyWindow ?? NSApp.mainWindow }
    func granted(_ permission: T3MacPermission) -> Bool { permission == .screenRecording ? screenRecording() : accessibility() }
}

/// How the macOS test capture failed (DesktopSnapShotSetupError's `status` for the window capture step): no window
/// to capture, the command timed out, exited with a code, or failed otherwise (it did not start, or wrote no PNG).
enum T3TestCaptureFailure: Equatable { case missing, timedOut, exited(Int32), failed }

final class T3SnapShot {
    /// DesktopSnapShotSetupError's test-mac-capture message. The window is this app's own, so finding it cannot fail
    /// as the reference's osascript can; only its capture names a step. `appName` is the app as Privacy & Security
    /// lists it, since macOS can report both grants as allowed while they belong to another install.
    static func testCaptureMessage(_ failure: T3TestCaptureFailure, appName: String? = nil) -> String {
        let summary = "Could not capture a test snapshot."
        let failed: String
        switch failure {
        case .missing: return "\(summary) No window is available to capture."
        case .timedOut: failed = "Capturing the window timed out."
        case .exited(let code): failed = "Capturing the window failed: screencapture exited with code \(code)."
        case .failed: failed = "Capturing the window failed."
        }
        guard let appName else { return "\(summary) \(failed)" }
        return "\(summary) \(failed) macOS grants Screen Recording and Accessibility to each app separately. In System Settings, open Privacy & Security and check that \"\(appName)\" is allowed for both."
    }
    /// The running app's name (Electron's app name, `environment.displayName`).
    static func appName(_ bundle: Bundle = .main) -> String {
        bundle.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ?? bundle.object(forInfoDictionaryKey: "CFBundleName") as? String ?? ProcessInfo.processInfo.processName
    }
    private let directory: URL
    private let agent: Bool
    private struct Capture { let path: URL; let owner: String; let metadata: [String: Any] }
    private var pending: [String: Capture] = [:]
    private var children: [String: (process: Process, pid: Int32, path: URL, lifetime: T3SnapshotLifetime)] = [:]
    private var scope = ""
    private let changed: (String) -> Void
    // `wanted` is the saved choice; `enabled` means grants allow an installed shortcut.
    private var wanted = false
    private var enabled = false
    private var verified = false
    private var includeAccessibility = true
    private var monitors: [Any] = []
    private var pairHeld = false
    private var shortcutText = "shift+shift"
    private lazy var shortcut = T3SnapshotShortcut(changed: { [weak self] in self?.changed("t3.status") })
    private var lastError = ""
    private var closing = false
    private let feedback = T3SnapshotFeedback()
    private lazy var permissionHelper: T3PermissionHelper = {
        let helper = T3PermissionHelper(); helper.finished = { [weak self] in self?.changed("t3.status") }; return helper
    }()
    // Seams for the snapshot AppKit test; the defaults are the real system and helper.
    var permissions = T3SnapshotPermissionSystem()
    lazy var showHelper: (T3MacPermission, NSWindow?, @escaping () -> Bool) -> Void = { [weak self] permission, owner, isGranted in
        self?.permissionHelper.show(permission, owner: owner, isGranted: isGranted)
    }
    private var playSound = true
    private var soundChoice = "soft-pop"
    private var flashEnabled = true
    private var animationsEnabled = true
    private enum TileMode: String { case shelf, thumb, contain, preview, fit }
    /// Each view owns its image; only a shelf hides beneath its capture flight.
    private final class Tile {
        weak var view: NSView?
        let image: NSView
        let file: String
        let mode: TileMode
        var loaded = false
        init(view: NSView, file: String, mode: TileMode) {
            self.view = view; self.file = file; self.mode = mode
            let contain = mode != .shelf && mode != .thumb
            image = PassiveImage(frame: view.bounds); image.autoresizingMask = [.width, .height]; image.wantsLayer = true
            image.layer?.contentsGravity = contain ? .resizeAspect : .resizeAspectFill; image.layer?.masksToBounds = true; image.layer?.cornerRadius = mode == .shelf ? 9 : 0
            view.addSubview(image, positioned: .below, relativeTo: nil)
        }
    }
    /// The tile's picture takes no clicks: they go to the node it paints, which a plain `NSView` (no first mouse)
    /// would keep from the first click into a window that is not key.
    private final class PassiveImage: NSView {
        override func hitTest(_ point: NSPoint) -> NSView? { nil }
    }
    private var tiles: [ObjectIdentifier: Tile] = [:]
    private(set) var tileLoadsInFlight = 0
    private var deferredAcks = Set<String>()
    /// The painted composer: its draft identity (data-snapshot-owner) and Exact focus.
    private struct Composer { let key: ObjectIdentifier; let owner: String; weak var view: NSView?; let focus: () -> Void }
    private var composer: Composer?
    func setComposer(key: ObjectIdentifier, owner: String, view: NSView?, focus: @escaping () -> Void) { composer = Composer(key: key, owner: owner, view: view, focus: focus) }
    func removeComposer(key: ObjectIdentifier) { if composer?.key == key { composer = nil } }
    func removeTile(view: NSView) {
        tiles.removeValue(forKey: ObjectIdentifier(view))?.image.removeFromSuperview()
    }
    /// Only a bare-id shelf can receive a capture flight; inline thumbs and full previews never do.
    func captureTileView(id: String) -> NSView? {
        let shelves = tiles.values.filter { $0.mode == .shelf && $0.file == id && $0.view != nil }
        return (shelves.first { $0.loaded && $0.view?.window?.isVisible == true && ($0.view?.bounds.width ?? 0) > 0 } ?? shelves.first)?.view
    }
    private func revealShelfTiles(id: String) {
        for tile in tiles.values where tile.mode == .shelf && tile.file == id { tile.image.isHidden = false }
    }
    /// `thumb:` is the inline square crop, `contain:` and `preview:` show the whole image,
    /// and `fit:` also avoids upscaling. Each view owns its picture, even for repeated source atoms.
    func installTile(id: String, view: NSView) {
        let mode: TileMode, file: String
        if let colon = id.firstIndex(of: ":") {
            guard let requested = TileMode(rawValue: String(id[..<colon])), requested != .shelf else { return }
            mode = requested; file = String(id[id.index(after: colon)...])
        } else { mode = .shelf; file = id }
        guard UUID(uuidString: file) != nil else { return }
        let key = ObjectIdentifier(view)
        if let tile = tiles[key], tile.view === view, tile.file == file, tile.mode == mode { return }
        removeTile(view: view)
        let tile = Tile(view: view, file: file, mode: mode); tiles[key] = tile
        tile.image.isHidden = mode == .shelf && feedback.flights[file] != nil
        let path = directory.appendingPathComponent("drafts/\(file).png")
        tileLoadsInFlight += 1
        DispatchQueue.global(qos: .userInitiated).async { [weak self, weak tile] in
            let image: CGImage? = {
                guard let bytes = try? Data(contentsOf: path), bytes.count <= 10 * 1024 * 1024,
                      let source = CGImageSourceCreateWithData(bytes as CFData, nil),
                      let decoded = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true, kCGImageSourceThumbnailMaxPixelSize: 2048] as CFDictionary) else { return nil }
                return mode == .shelf || mode == .thumb ? Self.composerThumbnail(decoded) : decoded
            }()
            DispatchQueue.main.async {
                guard let self else { return }
                self.tileLoadsInFlight -= 1
                guard let image, let tile, self.tiles[key] === tile, tile.view?.window != nil else { return }
                tile.image.layer?.contents = image; tile.loaded = true
                if mode == .fit, let bounds = tile.view?.bounds, CGFloat(image.width) <= bounds.width, CGFloat(image.height) <= bounds.height {
                    tile.image.layer?.contentsGravity = .center; tile.image.layer?.contentsScale = 1
                }
            }
        }
    }
    /// Reference createComposerImageThumbnail: the centred square of the
    /// shorter side, at most 256 px, then object-cover in the frame.
    static func composerThumbnail(_ image: CGImage) -> CGImage? {
        let side = min(image.width, image.height)
        guard side > 0, let square = image.cropping(to: CGRect(x: (image.width - side) / 2, y: (image.height - side) / 2, width: side, height: side)) else { return nil }
        let dimension = min(256, side)
        guard let context = CGContext(data: nil, width: dimension, height: dimension, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        context.interpolationQuality = .high
        context.draw(square, in: CGRect(x: 0, y: 0, width: dimension, height: dimension))
        return context.makeImage()
    }
    /// The painted frame a flight lands on: the bordered attachment frame
    /// around the image area, styled as the reference SNAP_SHOT frame.
    private func destination(of tile: Tile) -> T3SnapshotFeedback.Destination? {
        guard let view = tile.view, let window = view.window, window.isVisible, view.bounds.width > 0 else { return nil }
        let dark = view.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        let frame = window.convertToScreen(view.convert(view.bounds, to: nil)).insetBy(dx: -1, dy: -1)
        return .init(frame: frame, radius: 10, border: 1, borderColor: dark ? NSColor(white: 1, alpha: 0.06) : NSColor(red: 0.894, green: 0.894, blue: 0.906, alpha: 0.8), background: dark ? NSColor(white: 0.04, alpha: 1) : NSColor(white: 0.99, alpha: 1))
    }
    /// Focus only the composer painted for the capture's original draft, when
    /// nothing covers it (a settings sheet, a dialog) and no other field is typing.
    func focusComposer(owner: String) {
        guard let composer, composer.owner == owner, let view = composer.view, let window = view.window, window.isVisible,
              let content = window.contentView else { return }
        if let editing = window.firstResponder as? NSTextView, editing.isEditable, !editing.isDescendant(of: view) { return }
        let center = content.convert(CGPoint(x: view.bounds.midX, y: view.bounds.midY), from: view)
        guard let hit = content.hitTest(center), hit === view || hit.isDescendant(of: view) || view.isDescendant(of: hit) && hit !== content else { return }
        composer.focus()
    }
    private func acknowledge(_ id: String, capture: Capture, focusOwner: String?, deadline: TimeInterval) {
        guard !closing, pending[id]?.owner == capture.owner else { deferredAcks.remove(id); return }
        let tile = captureTileView(id: id).flatMap { tiles[ObjectIdentifier($0)] }, painted = tile?.loaded == true && tile?.view?.window?.isVisible == true && (tile?.view?.bounds.width ?? 0) > 0
        if !painted && ProcessInfo.processInfo.systemUptime < deadline {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.acknowledge(id, capture: capture, focusOwner: focusOwner, deadline: deadline) }; return
        }
        // The saved tile has painted underneath before the overlay lands on it.
        tile?.view?.displayIfNeeded()
        feedback.land(id: id, target: painted ? tile.flatMap(destination) : nil, reveal: { [weak tile] in tile?.image.isHidden = false; tile?.view?.displayIfNeeded() }) { [weak self] landed in
            guard let self, !self.closing, self.pending[id]?.owner == capture.owner else { return }
            self.deferredAcks.remove(id)
            try? FileManager.default.removeItem(at: capture.path)
            if FileManager.default.fileExists(atPath: capture.path.path) { self.lastError = "Could not release the captured window."; self.changed("t3.status"); return }
            self.pending.removeValue(forKey: id)
            // Cancellation (disable, dismissal) releases the delivered capture without moving focus.
            if landed, let focusOwner { self.focusComposer(owner: focusOwner) }
            self.changed("t3.status")
        }
    }

    init(directory: URL, agent: Bool, changed: @escaping (String) -> Void) {
        self.directory = directory.appendingPathComponent("snapshots", isDirectory: true)
        self.agent = agent
        self.changed = changed
        // A tile hidden under its flight is shown however that flight ends.
        feedback.ended = { [weak self] id in if self?.feedback.flights[id] == nil { self?.revealShelfTiles(id: id) } }
    }
    private func persistedDraftIds() -> Set<String>? {
        let preference = directory.deletingLastPathComponent().appendingPathComponent("t3-code.json")
        if !FileManager.default.fileExists(atPath: preference.path) { return [] }
        guard let bytes = try? Data(contentsOf: preference), let saved = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any], let drafts = saved["snapshotDrafts"] as? [String: [[String: Any]]] else { return nil }
        return Set(drafts.values.flatMap { $0 }.compactMap { $0["id"] as? String })
    }
    private func stopActive() {
        let needsChange = children.values.contains { !$0.lifetime.cancelled }
        for child in children.values {
            child.lifetime.cancel(owner: child.lifetime.owner)
            if child.process.isRunning && child.process.processIdentifier == child.pid { child.process.terminate() }
            try? FileManager.default.removeItem(at: child.path)
        }
        if needsChange { feedback.destroy(); changed("t3.status") }
    }
    private func removeMonitors() {
        for monitor in monitors { NSEvent.removeMonitor(monitor) }
        monitors.removeAll(); pairHeld = false; shortcut.stopRegistration()
    }
    func destroy() {
        closing = true; permissionHelper.close(); tiles.values.forEach { $0.image.removeFromSuperview() }; tiles.removeAll(); deferredAcks.removeAll(); composer = nil; shortcut.destroy(); feedback.destroy(); enabled = false; wanted = false; removeMonitors(); stopActive()
        for (id, capture) in pending {
            try? FileManager.default.removeItem(at: capture.path)
            // A saved but unacknowledged copy may be referenced by preferences;
            // retain it if the durable association exists, otherwise release it.
            if let saved = persistedDraftIds(), !saved.contains(id) { try? FileManager.default.removeItem(at: directory.appendingPathComponent("drafts/\(id).png")) }
        }
        pending.removeAll()
    }
    /// Reference MacPermissions.showHelper; the helper polls the same grant it docks for.
    private func dockHelper(_ permission: T3MacPermission, owner: NSWindow?) {
        let system = permissions
        showHelper(permission, owner) { system.granted(permission) }
    }
    /// Reference requestMacScreenCapturePermission: the system prompt once, then
    /// Privacy › Screen Recording while the grant is still missing.
    private func requestScreenRecording() {
        if !permissions.screenRecording() && !permissions.requestScreenRecording() { permissions.openSettings(T3MacPermission.screenRecording.settingsURL) }
    }
    /// Reference macPermissionMessage: the saved choice stays On while a grant
    /// is missing; the shortcut is installed again once grants return.
    static func permissionMessage(screenRecording: Bool, accessibility: Bool, includeAccessibility: Bool) -> String? {
        let text = !includeAccessibility || accessibility
        if !text && !screenRecording { return "Allow Accessibility and Screen Recording in System Settings, then restart T3 Code." }
        if !text { return "Allow Accessibility in System Settings, then restart T3 Code." }
        return screenRecording ? nil : "Allow Screen Recording in System Settings, then restart T3 Code."
    }
    private func configure(enabled: Bool, includeAccessibility: Bool) throws {
        guard !closing else { throw T3Failure(kind: "SnapShot", message: "Window capture is closed.") }
        wanted = enabled
        if !enabled { verified = false }
        if enabled, let permissionError = Self.permissionMessage(screenRecording: permissions.screenRecording(), accessibility: permissions.accessibility(), includeAccessibility: includeAccessibility) {
            if self.enabled { feedback.destroy() }
            self.enabled = false; self.includeAccessibility = includeAccessibility; verified = false; removeMonitors(); stopActive(); lastError = permissionError
            return
        }
        guard self.enabled != enabled || self.includeAccessibility != includeAccessibility else { lastError = ""; return }
        if self.includeAccessibility != includeAccessibility { stopActive() }
        removeMonitors(); self.enabled = enabled; self.includeAccessibility = includeAccessibility; lastError = ""
        if !enabled { feedback.destroy(); stopActive(); return }
        try shortcut.install(shortcutText) { [weak self] in self?.captureFrontWindow() }
        if let monitor = NSEvent.addGlobalMonitorForEvents(matching: .flagsChanged, handler: { [weak self] _ in self?.modifiersChanged() }) { monitors.append(monitor) }
        if let monitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged, handler: { [weak self] event in self?.modifiersChanged(); return event }) { monitors.append(monitor) }
    }
    private func modifiersChanged() {
        guard !shortcut.recording, let keys = T3SnapshotShortcut.pairs[shortcutText] else { return }
        let both = keys.allSatisfy { CGEventSource.keyState(.combinedSessionState, key: $0) }
        defer { pairHeld = both }
        if enabled && both && !pairHeld { captureFrontWindow() }
    }
    /// DesktopSnapShot SHORTCUT_COOLDOWN_NS.
    static let shortcutCooldown: TimeInterval = 0.2
    private var lastShortcutAt: TimeInterval?
    /// captureFromShortcut: a press while a capture flies to its attachment is dropped, so it cannot tear that
    /// flight down and start another; one within the cooldown of the last accepted press is dropped too.
    static func acceptsShortcut(animating: Bool, now: TimeInterval, last: TimeInterval?) -> (verified: Bool, capture: Bool) {
        if animating { return (false, false) }
        return (true, last.map { now - $0 >= shortcutCooldown } ?? true)
    }
    private func captureFrontWindow() {
        guard enabled, !shortcut.recording else { return }
        let now = ProcessInfo.processInfo.systemUptime
        let accepted = Self.acceptsShortcut(animating: feedback.isAnimating, now: now, last: lastShortcutAt)
        guard accepted.verified else { return }
        verified = true
        guard accepted.capture else { return }
        lastShortcutAt = now
        guard let front = NSWorkspace.shared.frontmostApplication,
              let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]],
              let window = windows.first(where: { ($0[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == front.processIdentifier && ($0[kCGWindowLayer as String] as? NSNumber)?.intValue == 0 }),
              let id = window[kCGWindowNumber as String] as? NSNumber else { return }
        perform(["op": "snapshotCapture", "windowId": id.uint32Value, "includeAccessibility": includeAccessibility]) { [weak self] response in
            self?.lastError = (response["error"] as? [String: Any])?["message"] as? String ?? ""
            self?.changed("t3.status")
        }
    }
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let complete: (Any) -> Void = { value in reply(["ok": true, "generation": generation, "value": value]) }
        let fail: (String) -> Void = { message in reply(["ok": false, "generation": generation, "error": ["kind": "SnapShot", "message": message, "uncertain": false]]) }
        switch request["op"] as? String {
        case "snapshotRecordShortcut":
            if request["record"] as? Bool == true { shortcut.startRecording() } else { shortcut.cancel() }
            complete(["recording": shortcut.recording])
        case "snapshotCheckShortcut":
            let text = request["shortcut"] as? String ?? ""
            let bindings = request["bindings"] as? [[String: String]] ?? []
            let physical = T3SnapshotShortcut.chord(text)
            let conflict = bindings.first { binding in
                guard let physical, let chord = T3SnapshotShortcut.chord(binding["key"] ?? "") else { return false }
                return physical.0 == chord.0 && physical.1 == chord.1
            }
            if let conflict { complete(["available": false, "message": "T3 Code already uses this for \(conflict["command"] ?? "another command")."]) }
            else if let error = shortcut.check(text, saved: shortcutText) { complete(["available": false, "message": error]) }
            else { complete(["available": true]) }
        case "snapshotPlaySound":
            if feedback.play(request["sound"] as? String ?? "") { complete(["played": true]) }
            else { fail("The snapshot sound could not be played.") }
        case "snapshotConfigure":
            let nextShortcut = request["shortcut"] as? String ?? "shift+shift"
            guard let canonical = T3SnapshotShortcut.canonical(nextShortcut) else { fail("Choose a valid snapshot shortcut."); return }
            if canonical != shortcutText {
                if let error = shortcut.check(canonical, saved: shortcutText) { fail(error); return }
                if enabled {
                    do { try shortcut.install(canonical) { [weak self] in self?.captureFrontWindow() } }
                    catch { fail((error as? T3Failure)?.message ?? "Could not replace snapshot shortcut."); return }
                }
                shortcut.cancel(); stopActive(); shortcutText = canonical; verified = false
            }
            playSound = request["playSound"] as? Bool ?? true
            soundChoice = request["sound"] as? String ?? "soft-pop"
            flashEnabled = request["flash"] as? Bool ?? true
            animationsEnabled = request["animations"] as? Bool ?? true
            if !animationsEnabled { feedback.cancelFlights() }
            let nextScope = request["owner"] as? String ?? ""
            if nextScope != scope { shortcut.cancel(); scope = nextScope }
            do { try configure(enabled: request["enabled"] as? Bool ?? false, includeAccessibility: request["includeAccessibility"] as? Bool ?? true); complete(["enabled": enabled]) } catch { fail((error as? T3Failure)?.message ?? "Could not configure window capture.") }
        case "snapshotState":
            complete(["enabled": enabled, "wanted": wanted, "verified": verified, "flights": feedback.flights.keys.sorted(), "error": lastError, "recording": shortcut.recording, "candidate": shortcut.candidate, "shortcut": shortcutText, "mode": "direct", "screenRecording": permissions.screenRecording(), "accessibility": permissions.accessibility(), "captures": pending.map { ["id": $0.key, "owner": $0.value.owner] }, "pending": pending.filter { $0.value.owner == (request["owner"] as? String ?? "") }.keys.sorted(), "capturing": children.filter { $0.value.lifetime.owner == (request["owner"] as? String ?? "") }.keys.sorted()])
        case "snapshotDraftSave":
            guard let id = request["id"] as? String, let capture = pending[id], capture.owner == (request["owner"] as? String ?? "") else { fail("This snapshot is no longer available in that draft."); return }
            do {
                let drafts = directory.appendingPathComponent("drafts", isDirectory: true)
                try FileManager.default.createDirectory(at: drafts, withIntermediateDirectories: true)
                // The draft holds what the send uploads: a capture over `imageMaxEdge` is scaled to it (T3ImageFit).
                let data = try Data(contentsOf: capture.path)
                let fitted = T3ImageFit.fit(data, maxEdge: request["imageMaxEdge"] as? Int ?? 0)
                try (fitted?.data ?? data).write(to: drafts.appendingPathComponent("\(id).png"), options: .atomic)
                complete(fitted.map { ["id": id, "sizeBytes": $0.data.count, "width": $0.width, "height": $0.height] } ?? ["id": id])
            } catch { fail("Could not save the captured image to its draft.") }
        case "snapshotDraftRead", "snapshotDraftRemove":
            guard let id = request["id"] as? String, UUID(uuidString: id) != nil else { fail("That draft image is unavailable."); return }
            let path = directory.appendingPathComponent("drafts", isDirectory: true).appendingPathComponent("\(id).png")
            do {
                if request["op"] as? String == "snapshotDraftRemove" { if FileManager.default.fileExists(atPath: path.path) { try FileManager.default.removeItem(at: path) }; complete(["removed": id]); return }
                var data = try Data(contentsOf: path)
                if let fitted = T3ImageFit.fit(data, maxEdge: request["imageMaxEdge"] as? Int ?? 0) {
                    data = fitted.data
                    try data.write(to: path, options: .atomic)
                }
                // A capture is a PNG; an attached image keeps its own PNG, JPEG, GIF or WebP bytes (composer-attach-stage.ts).
                let signatures: [[UInt8]] = [[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a], [0xff, 0xd8, 0xff], Array("GIF8".utf8)]
                let webP = data.count >= 12 && data.starts(with: Array("RIFF".utf8)) && data.subdata(in: 8..<12) == Data("WEBP".utf8)
                guard data.count <= 10 * 1024 * 1024, webP || signatures.contains(where: { data.starts(with: $0) }) else { fail("The captured image is too large or invalid."); return }
                complete(["id": id, "base64": data.base64EncodedString(), "sizeBytes": data.count])
            } catch { fail("Could not read the saved draft image.") }
        case "snapshotDraftSize":
            // The expanded preview's natural size: the pixels the tile shows (`installTile` decodes at most 2048 on the
            // longer side), which it draws one to a point.
            guard let id = request["id"] as? String, UUID(uuidString: id) != nil else { fail("That draft image is unavailable."); return }
            let path = directory.appendingPathComponent("drafts", isDirectory: true).appendingPathComponent("\(id).png")
            guard let source = CGImageSourceCreateWithURL(path as CFURL, nil),
                  let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
                  let width = properties[kCGImagePropertyPixelWidth] as? Int, let height = properties[kCGImagePropertyPixelHeight] as? Int,
                  width > 0, height > 0 else { fail("Could not read the saved draft image."); return }
            let scale = min(1, 2048 / Double(max(width, height)))
            complete(["width": (Double(width) * scale).rounded(), "height": (Double(height) * scale).rounded()])
        case "snapshotRead":
            guard let id = request["id"] as? String, let capture = pending[id], capture.owner == (request["owner"] as? String ?? "") else { fail("This snapshot is no longer available."); return }
            do { complete(capture.metadata.merging(["id": id, "owner": capture.owner, "mimeType": "image/png", "base64": try Data(contentsOf: capture.path).base64EncodedString()]) { _, new in new }) }
            catch { fail("Could not read the captured window.") }
        case "snapshotAcknowledge":
            guard let id = request["id"] as? String, let capture = pending[id], capture.owner == (request["owner"] as? String ?? "") else { fail("This snapshot is no longer available."); return }
            if deferredAcks.insert(id).inserted {
                let focusOwner = request["focus"] as? Bool == true ? request["focusOwner"] as? String : nil
                // Return before the new projection paints; the original file and
                // overlay survive until the own tile is ready (or background fallback).
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.acknowledge(id, capture: capture, focusOwner: focusOwner, deadline: ProcessInfo.processInfo.systemUptime + 1) }
            }
            complete(["acknowledgmentPending": id])
        case "snapshotSetup":
            // Reference setupSnapShot for macOS. Isolated testing never prompts for,
            // or changes, OS grants and never captures for a test.
            let action = request["action"] as? String ?? ""
            guard ["allow-screen-recording", "allow-accessibility", "test-mac-capture"].contains(action) else { fail("Unsupported capture setup action."); return }
            guard !agent else { fail("Permission prompts and test captures are disabled in isolated testing."); return }
            // Both Allow actions dock the helper beside System Settings (dockHelper).
            if action == "allow-accessibility" {
                let owner = permissions.focusedWindow()
                if !permissions.promptAccessibility() { permissions.openSettings(T3MacPermission.accessibility.settingsURL) }
                dockHelper(.accessibility, owner: owner)
                changed("t3.status"); complete(["action": action]); return
            }
            if action == "allow-screen-recording" {
                let owner = permissions.focusedWindow()
                requestScreenRecording()
                dockHelper(.screenRecording, owner: owner)
                changed("t3.status"); complete(["action": action]); return
            }
            // Exercise the real capture path on this app's own window; the image is discarded.
            guard CGPreflightScreenCaptureAccess() else { fail("Allow Screen Recording in System Settings, then restart T3 Code."); return }
            guard let window = NSApp.windows.first(where: { $0.isVisible && $0.windowNumber > 0 }) else { fail(Self.testCaptureMessage(.missing)); return }
            let number = window.windowNumber, test = FileManager.default.temporaryDirectory.appendingPathComponent("t3-snapshot-test-\(UUID().uuidString).png")
            let appName = Self.appName()
            DispatchQueue.global(qos: .userInitiated).async {
                let process = Process(); process.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
                process.arguments = ["-l", String(number), "-o", "-x", "-t", "png", test.path]
                process.standardOutput = FileHandle.nullDevice; process.standardError = FileHandle.nullDevice
                // Set before `run`, so a command that exits at once still signals.
                let exited = DispatchSemaphore(value: 0)
                process.terminationHandler = { _ in exited.signal() }
                let failure: T3TestCaptureFailure? = {
                    guard (try? process.run()) != nil else { return .failed }
                    // MacSnapShot.ts MAC_SCREEN_CAPTURE_TIMEOUT_MS: execFile's timeout kills the command.
                    if exited.wait(timeout: .now() + 15) == .timedOut { process.terminate(); process.waitUntilExit(); return .timedOut }
                    if process.terminationStatus != 0 { return .exited(process.terminationStatus) }
                    return (try? Data(contentsOf: test))?.starts(with: [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) == true ? nil : .failed
                }()
                try? FileManager.default.removeItem(at: test)
                DispatchQueue.main.async { failure.map { fail(Self.testCaptureMessage($0, appName: appName)) } ?? complete(["action": action]) }
            }
        case "snapshotRequestPermissions":
            // Reference requestPermissions (setup's Continue, Include app text): prompt
            // for what is missing, then dock the helper for the first missing grant.
            guard !agent else { complete(["requested": false]); return }
            let include = request["includeAccessibility"] as? Bool ?? true, owner = permissions.focusedWindow()
            if include { _ = permissions.promptAccessibility() }
            requestScreenRecording()
            if !permissions.screenRecording() { dockHelper(.screenRecording, owner: owner) }
            else if include && !permissions.accessibility() { permissions.openSettings(T3MacPermission.accessibility.settingsURL); dockHelper(.accessibility, owner: owner) }
            changed("t3.status"); complete(["requested": true])
        case "snapshotDismiss":
            // Reference dismissSnapShotAnimation: end this capture's flight now; the
            // capture stays pending. An acknowledged landing is never cut short.
            guard let id = request["id"] as? String, let capture = pending[id], capture.owner == (request["owner"] as? String ?? "") else { fail("This snapshot is no longer available."); return }
            if !deferredAcks.contains(id) { feedback.dismiss(id: id); revealShelfTiles(id: id) }
            complete(["dismissed": id])
        case "snapshotCancel":
            guard let id = request["id"] as? String, let child = children[id], child.lifetime.cancel(owner: request["owner"] as? String ?? "") else { fail("This capture is no longer running in this draft."); return }
            if child.process.isRunning && child.process.processIdentifier == child.pid { child.process.terminate() }
            try? FileManager.default.removeItem(at: child.path)
            changed("t3.status"); complete(["cancelled": id])
        case "snapshotCapture":
            let captureOwner = request["owner"] as? String ?? scope
            guard !captureOwner.isEmpty, captureOwner == scope, !closing, enabled else { fail("Choose a draft before capturing a window."); return }
            guard CGPreflightScreenCaptureAccess() else { fail("Screen Recording access is required to capture a window. Grant access in System Settings, then retry."); return }
            if request["includeAccessibility"] as? Bool == true, !AXIsProcessTrusted() { fail("Accessibility access is required to include app text. Grant access in System Settings, then retry."); return }
            guard let windowId = request["windowId"] as? UInt32,
                  let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]],
                  let window = windows.first(where: { ($0[kCGWindowNumber as String] as? NSNumber)?.uint32Value == windowId }),
                  let owner = window[kCGWindowOwnerPID as String] as? NSNumber else { fail("The selected window is no longer available."); return }
            // Agent capture can only touch this workflow's isolated native app.
            guard !agent || owner.int32Value == ProcessInfo.processInfo.processIdentifier else { fail("Isolated testing may only capture this app's window."); return }
            let id = UUID().uuidString, path = directory.appendingPathComponent("\(id).png")
            do { try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true) }
            catch { fail("Could not prepare snapshot storage."); return }
            let lifetime = T3SnapshotLifetime(owner: captureOwner)
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
            process.arguments = ["-l", String(windowId), "-o", "-x", "-t", "png", path.path]
            process.standardOutput = FileHandle.nullDevice
            process.standardError = FileHandle.nullDevice
            process.terminationHandler = { [weak self] process in
                DispatchQueue.main.async {
                    guard let self else { try? FileManager.default.removeItem(at: path); fail("Capture was cancelled."); return }
                    if !lifetime.canPublish(owner: captureOwner, enabled: self.enabled, closing: self.closing) { self.children.removeValue(forKey: id); try? FileManager.default.removeItem(at: path); self.changed("t3.status"); fail("Capture was cancelled or timed out."); return }
                    guard process.terminationStatus == 0, let data = try? Data(contentsOf: path),
                          data.starts(with: [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) else { self.children.removeValue(forKey: id); try? FileManager.default.removeItem(at: path); self.changed("t3.status"); fail("macOS did not return a valid window snapshot."); return }
                    let bounds = CGRect(dictionaryRepresentation: (window[kCGWindowBounds as String] as? [String: Any] ?? [:]) as CFDictionary) ?? .zero
                    DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                        let resized = T3SnapshotImage.boundedPNG(data, cancelled: { lifetime.cancelled })
                        let saved: Bool = {
                            guard let resized, !lifetime.cancelled else { return false }
                            do { try resized.data.write(to: path, options: .atomic); return true } catch { return false }
                        }()
                        let thumbnail: CGImage? = resized.flatMap { resized in
                            guard let source = CGImageSourceCreateWithData(resized.data as CFData, nil) else { return nil }
                            return CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceThumbnailMaxPixelSize: 1024] as CFDictionary)
                        }
                        let metadata = request["includeAccessibility"] as? Bool == true ? T3SnapshotAccessibility.read(owner: owner.int32Value, windowId: windowId, bounds: bounds, cancelled: { lifetime.cancelled }) : [:]
                        DispatchQueue.main.async {
                            guard let self else { try? FileManager.default.removeItem(at: path); fail("Capture was cancelled."); return }
                            self.children.removeValue(forKey: id)
                            guard lifetime.canPublish(owner: captureOwner, enabled: self.enabled, closing: self.closing) else { try? FileManager.default.removeItem(at: path); self.changed("t3.status"); fail("The capture draft changed. Capture again in the selected draft."); return }
                            guard let resized, saved else { try? FileManager.default.removeItem(at: path); self.changed("t3.status"); fail("The captured window is too large or invalid."); return }
                            let width = resized.width, height = resized.height
                            let source: [String: Any] = ["kind": "snap-shot", "capturedAt": ISO8601DateFormatter().string(from: Date()), "appName": String((window[kCGWindowOwnerName as String] as? String ?? "App").prefix(255)), "windowTitle": String((window[kCGWindowName as String] as? String ?? "").prefix(1000))]
                            let details: [String: Any] = ["source": source.merging(metadata) { _, new in new }, "name": "SnapShot-\(id).png", "sizeBytes": resized.data.count, "width": width, "height": height]
                            if self.playSound { _ = self.feedback.play(self.soundChoice) }
                            // Reference showCaptureFeedback: the flight carries the flash;
                            // without motion a separate flash is static (60 ms).
                            let motion = self.animationsEnabled && !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
                            let flying = motion && thumbnail.map { self.feedback.begin(id: id, image: $0, bounds: bounds, flash: self.flashEnabled) } == true
                            if !flying && self.flashEnabled { self.feedback.show(bounds: bounds, animated: motion) }
                            self.pending[id] = Capture(path: path, owner: captureOwner, metadata: details)
                            self.changed("t3.status")
                            complete(["id": id, "owner": captureOwner, "mimeType": "image/png", "source": window[kCGWindowName as String] as? String ?? "Window", "width": width, "height": height])
                        }
                    }
                }
            }
            do {
                try process.run()
                children[id] = (process, process.processIdentifier, path, lifetime)
                let record: [String: Any] = ["id": id, "pid": process.processIdentifier, "at": ISO8601DateFormatter().string(from: Date()), "path": path.path]
                let journal = directory.appendingPathComponent("capture-processes.ndjson")
                if !FileManager.default.fileExists(atPath: journal.path) { _ = FileManager.default.createFile(atPath: journal.path, contents: nil) }
                if let file = try? FileHandle(forWritingTo: journal), let data = try? JSONSerialization.data(withJSONObject: record) { _ = try? file.seekToEnd(); try? file.write(contentsOf: data + Data([10])); try? file.close() }
                changed("t3.status")
                DispatchQueue.main.asyncAfter(deadline: .now() + 15) { [weak self] in
                    guard let self, let child = self.children[id] else { return }
                    child.lifetime.cancel(owner: captureOwner)
                    if child.process.isRunning && child.process.processIdentifier == child.pid { child.process.terminate() }
                    try? FileManager.default.removeItem(at: path)
                    self.changed("t3.status")
                }
            } catch { try? FileManager.default.removeItem(at: path); fail("Could not start macOS window capture.") }
        default: fail("Unsupported snapshot operation.")
        }
    }
}
