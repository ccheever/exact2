// App-owned desktop capture boundary. No permission grants occur during state reads.
import AppKit
import ApplicationServices
import Foundation
import ImageIO

/// The macOS grants, prompts and System Settings as snapshot setup and
/// requestPermissions reach them (reference getMediaAccessStatus,
/// isTrustedAccessibilityClient, desktopCapturer.getSources, shell.openExternal).
/// The defaults are the real system; the snapshot AppKit test replaces every one,
/// so it never reaches TCC.
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

final class T3SnapShot {
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
    /// The attachment's image area: an aspect-fill (object-cover) thumbnail,
    /// hidden while its capture's flight is still in the air.
    private final class Tile {
        weak var view: NSView?
        let image: NSView
        var loaded = false
        init(view: NSView) {
            self.view = view
            image = NSView(frame: view.bounds); image.autoresizingMask = [.width, .height]; image.wantsLayer = true
            image.layer?.contentsGravity = .resizeAspectFill; image.layer?.masksToBounds = true; image.layer?.cornerRadius = 9
            view.addSubview(image, positioned: .below, relativeTo: nil)
        }
    }
    private var tiles: [String: Tile] = [:]
    private var deferredAcks = Set<String>()
    /// The painted composer: its draft identity (data-snapshot-owner) and Exact focus.
    private struct Composer { let key: ObjectIdentifier; let owner: String; weak var view: NSView?; let focus: () -> Void }
    private var composer: Composer?
    func setComposer(key: ObjectIdentifier, owner: String, view: NSView?, focus: @escaping () -> Void) { composer = Composer(key: key, owner: owner, view: view, focus: focus) }
    func removeComposer(key: ObjectIdentifier) { if composer?.key == key { composer = nil } }
    func removeTile(view: NSView) {
        for (id, tile) in tiles where tile.view === view { tile.image.removeFromSuperview(); tiles.removeValue(forKey: id) }
    }
    func installTile(id: String, view: NSView) {
        guard UUID(uuidString: id) != nil else { return }
        if tiles[id]?.view === view { return }
        removeTile(view: view)
        tiles[id]?.image.removeFromSuperview()
        let tile = Tile(view: view); tiles[id] = tile
        tile.image.isHidden = feedback.flights[id] != nil
        let path = directory.appendingPathComponent("drafts/\(id).png")
        DispatchQueue.global(qos: .userInitiated).async { [weak self, weak tile] in
            guard let bytes = try? Data(contentsOf: path), bytes.count <= 10 * 1024 * 1024,
                  let source = CGImageSourceCreateWithData(bytes as CFData, nil),
                  let decoded = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true, kCGImageSourceThumbnailMaxPixelSize: 2048] as CFDictionary),
                  let image = Self.composerThumbnail(decoded) else { return }
            DispatchQueue.main.async {
                guard let self, let tile, self.tiles[id] === tile, tile.view?.window != nil else { return }
                tile.image.layer?.contents = image; tile.loaded = true
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
        let tile = tiles[id], painted = tile?.loaded == true && tile?.view?.window?.isVisible == true && (tile?.view?.bounds.width ?? 0) > 0
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
        feedback.ended = { [weak self] id in if self?.feedback.flights[id] == nil { self?.tiles[id]?.image.isHidden = false } }
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
        if enabled, let permissionError = Self.permissionMessage(screenRecording: CGPreflightScreenCaptureAccess(), accessibility: AXIsProcessTrusted(), includeAccessibility: includeAccessibility) {
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
    private func captureFrontWindow() {
        guard enabled, !shortcut.recording else { return }
        verified = true
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
            complete(["enabled": enabled, "wanted": wanted, "verified": verified, "flights": feedback.flights.keys.sorted(), "error": lastError, "recording": shortcut.recording, "candidate": shortcut.candidate, "shortcut": shortcutText, "mode": "direct", "screenRecording": CGPreflightScreenCaptureAccess(), "accessibility": AXIsProcessTrusted(), "captures": pending.map { ["id": $0.key, "owner": $0.value.owner] }, "pending": pending.filter { $0.value.owner == (request["owner"] as? String ?? "") }.keys.sorted(), "capturing": children.filter { $0.value.lifetime.owner == (request["owner"] as? String ?? "") }.keys.sorted()])
        case "snapshotDraftSave":
            guard let id = request["id"] as? String, let capture = pending[id], capture.owner == (request["owner"] as? String ?? "") else { fail("This snapshot is no longer available in that draft."); return }
            do {
                let drafts = directory.appendingPathComponent("drafts", isDirectory: true)
                try FileManager.default.createDirectory(at: drafts, withIntermediateDirectories: true)
                try Data(contentsOf: capture.path).write(to: drafts.appendingPathComponent("\(id).png"), options: .atomic)
                complete(["id": id])
            } catch { fail("Could not save the captured image to its draft.") }
        case "snapshotDraftRead", "snapshotDraftRemove":
            guard let id = request["id"] as? String, UUID(uuidString: id) != nil else { fail("That draft image is unavailable."); return }
            let path = directory.appendingPathComponent("drafts", isDirectory: true).appendingPathComponent("\(id).png")
            do {
                if request["op"] as? String == "snapshotDraftRemove" { if FileManager.default.fileExists(atPath: path.path) { try FileManager.default.removeItem(at: path) }; complete(["removed": id]); return }
                let data = try Data(contentsOf: path)
                guard data.count <= 10 * 1024 * 1024, data.starts(with: [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) else { fail("The captured image is too large or invalid."); return }
                complete(["id": id, "base64": data.base64EncodedString()])
            } catch { fail("Could not read the saved draft image.") }
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
            guard let window = NSApp.windows.first(where: { $0.isVisible && $0.windowNumber > 0 }) else { fail("No window is available to test capture."); return }
            let number = window.windowNumber, test = FileManager.default.temporaryDirectory.appendingPathComponent("t3-snapshot-test-\(UUID().uuidString).png")
            DispatchQueue.global(qos: .userInitiated).async {
                let process = Process(); process.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
                process.arguments = ["-l", String(number), "-o", "-x", "-t", "png", test.path]
                process.standardOutput = FileHandle.nullDevice; process.standardError = FileHandle.nullDevice
                let ok = (try? process.run()) != nil && { process.waitUntilExit(); return process.terminationStatus == 0 }() && ((try? Data(contentsOf: test))?.starts(with: [0x89, 0x50, 0x4e, 0x47]) == true)
                try? FileManager.default.removeItem(at: test)
                DispatchQueue.main.async { ok ? complete(["action": action]) : fail("macOS did not return a test snapshot. Check Screen Recording in System Settings.") }
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
            if !deferredAcks.contains(id) { feedback.dismiss(id: id); tiles[id]?.image.isHidden = false }
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
