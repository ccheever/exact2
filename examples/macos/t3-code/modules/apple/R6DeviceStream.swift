#if os(macOS)
import AppKit
import Foundation
import UniformTypeIdentifiers

/// Lane r6-media / r7-device: a device's live screen (MIT reference, see LICENSE-T3:
/// packages/client-runtime/src/device/stream.ts, hubAccess.ts, screenshot.ts;
/// apps/web/src/components/device/DeviceStreamView.tsx). Hook `t3-media` with
/// `data-media-kind` "device-ios" / "device-android", `data-device-id`, `data-device-host`,
/// `data-media-name` (the device's name, for its 3D model), `data-device-view` ("phone" / "flat"
/// in the panel, empty in the floating player) and `data-device-ax` ("1" draws accessibility
/// frames); `data-media-kind="device-events"` is the Tools drawer's open event log.
///
/// The hub is reached through the server's `/api/device-hub` proxy with a short-lived
/// `wsTicket` (and `hostId`) in the query, as the reference's bearer connections do. The stream
/// and input protocols are R7DeviceClient.swift's; what each stream is doing is reported as
/// `presentation.deviceStreams[hostId \0 deviceId]`, the Tools drawer's feeds as
/// `presentation.deviceTools[hostId \0 deviceId]` (R7DeviceTools.swift).
enum R6DeviceWire {
    static let touch: UInt8 = 0x03, button: UInt8 = 0x04, orientation: UInt8 = 0x07, hardwareKeyboard: UInt8 = 0x0d, screenConfig: UInt8 = 0x82
    static let orientations = ["portrait", "landscape_left", "portrait_upside_down", "landscape_right"]
    static let firstFrameTimeout: TimeInterval = 15
    static let hubBase = "/api/device-hub"

    /// `[tag][json]`, as taggedJson.
    static func tagged(_ tag: UInt8, _ payload: [String: Any]) -> Data {
        var data = Data([tag])
        data.append((try? JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys])) ?? Data("{}".utf8))
        return data
    }

    /// A hub URL with the access query (withDeviceHubQuery): wsTicket, then hostId.
    static func url(origin: URL, path: String, query: [URLQueryItem], ticket: String, hostId: String, websocket: Bool = false) -> URL? {
        guard var components = URLComponents(url: origin, resolvingAgainstBaseURL: false) else { return nil }
        if websocket { components.scheme = components.scheme == "https" ? "wss" : "ws" }
        components.path = hubBase + path
        components.queryItems = query + [URLQueryItem(name: "wsTicket", value: ticket), URLQueryItem(name: "hostId", value: hostId)]
        return components.url
    }
    static func vendor(_ platform: String) -> String { platform == "ios" ? "/vendor/serve-sim" : "/vendor/serve-emu" }

    /// Complete JPEG images in an MJPEG byte stream (SOI … EOI), and the bytes left over.
    static func frames(_ buffer: Data) -> (images: [Data], rest: Data) {
        var images: [Data] = [], cursor = buffer.startIndex
        let bytes = [UInt8](buffer)
        var index = 0, start = -1
        while index + 1 < bytes.count {
            if start < 0, bytes[index] == 0xFF, bytes[index + 1] == 0xD8 { start = index; index += 2; continue }
            if start >= 0, bytes[index] == 0xFF, bytes[index + 1] == 0xD9 {
                images.append(Data(bytes[start...(index + 1)]))
                start = -1; index += 2
                cursor = buffer.startIndex + index
                continue
            }
            index += 1
        }
        let restStart = start >= 0 ? buffer.startIndex + start : max(cursor, buffer.endIndex - 1)
        return (images, Data(buffer[restStart...]))
    }

    /// rawPoint: serve-sim takes touches in the raw framebuffer, so a rotated portrait device remaps them.
    static func rawPoint(x: Double, y: Double, screen: (width: Double, height: Double, orientation: String)?) -> (Double, Double) {
        guard let screen, screen.width <= screen.height else { return (x, y) }
        switch screen.orientation {
        case "landscape_left": return (y, 1 - x)
        case "landscape_right": return (1 - y, x)
        case "portrait_upside_down": return (1 - x, 1 - y)
        default: return (x, y)
        }
    }

    /// The picture's rectangle inside the view (object-fit: contain).
    static func fit(_ image: CGSize, in bounds: CGRect) -> CGRect {
        guard image.width > 0, image.height > 0, bounds.width > 0, bounds.height > 0 else { return .zero }
        let scale = min(bounds.width / image.width, bounds.height / image.height)
        let size = CGSize(width: image.width * scale, height: image.height * scale)
        return CGRect(x: bounds.midX - size.width / 2, y: bounds.midY - size.height / 2, width: size.width, height: size.height)
    }

    /// The next orientation Rotate asks for (IOS_ORIENTATIONS, one step).
    static func nextOrientation(_ current: String?) -> String {
        let index = orientations.firstIndex(of: current ?? "portrait") ?? 0
        return orientations[(index + 1) % orientations.count]
    }
}

/// DeviceStreamView: the live screen of one device, flat (the picture in the largest box at the
/// device's aspect, rotated for a portrait framebuffer shown sideways) or, in the panel, the 3D
/// phone (R7DevicePhone.swift). Pointer touches and the focused view's keys go to the device
/// through R7DeviceClient; accessibility frames draw over the flat picture (R7DeviceTools.swift).
final class R6DeviceScreenView: NSView {
    typealias Access = R7DeviceClient.Access
    let key: String, platform: String, deviceId: String, hostId: String
    private let report: () -> Void
    let client: R7DeviceClient
    private let picture = CALayer()
    let overlay = R7DeviceAxOverlay()
    private(set) var image: CGImage?
    private var imageSize: CGSize = .zero
    private var touching = false
    private var restartNotice = false
    private var restartTimer: DispatchWorkItem?
    private(set) var status = "connecting", detail = "", inputConnected = false, inputDetail = ""
    /// The panel asks for "phone", "phone-keyboard" (an iPad with its Magic Keyboard) or "flat"; the
    /// floating player leaves it empty (always flat).
    var presentation = "" { didSet { if presentation != oldValue { refresh() } } }
    var deviceName = "" { didSet { if deviceName != oldValue { phone?.setName(deviceName) } } }
    var axOverlay = false { didSet { if axOverlay != oldValue { refresh() } } }
    private(set) var phone: R7DevicePhoneView?
    /// Lane r9-device: the iPhone Duo's hinged viewer (R9DeviceDuoView.swift) and a foldable's fold reads (R9DeviceFold.swift).
    private(set) var duoView: R9DuoView?
    private(set) var fold: R9DeviceFold?
    private(set) var duoState = R9DuoControlState()
    private var phoneFailed = false
    var screen: (width: Double, height: Double, orientation: String)? { client.screen.map { ($0.width, $0.height, $0.orientation) } }
    var sent: [Data] { client.sent }

    init(key: String, platform: String, deviceId: String, hostId: String, access: @escaping Access, report: @escaping () -> Void) {
        self.key = key; self.platform = platform; self.deviceId = deviceId; self.hostId = hostId; self.report = report
        client = R7DeviceClient(platform: platform, deviceId: deviceId, hostId: hostId, access: access)
        super.init(frame: .zero)
        wantsLayer = true
        layer?.masksToBounds = true
        layer?.addSublayer(picture)
        layer?.addSublayer(overlay)
        picture.contentsGravity = .resizeAspect
        setAccessibilityRole(.image)
        setAccessibilityLabel(platform == "ios" ? "iOS Simulator screen" : "Android Emulator screen")
        client.onStatus = { [weak self] status, detail in
            guard let self else { return }
            self.status = status; self.detail = detail
            if status != "connecting" { self.restartNotice = false }
            self.refresh(); self.report()
        }
        client.onInput = { [weak self] connected, detail in
            guard let self else { return }
            self.inputConnected = connected; self.inputDetail = detail
            if !connected { self.restartNotice = false }
            self.refresh(); self.report()
        }
        client.onScreen = { [weak self] _ in
            guard let self else { return }
            self.phone?.setScreen(self.client.screen); self.duoView?.setScreen(self.client.screen)
            self.refresh(); self.layoutPicture(); self.report()
        }
        client.onDuoControl = { [weak self] state in
            guard let self else { return }
            self.duoState = state
            if case .angle(let value) = state.requested { self.duoView?.setControlPreview(value) } else { self.duoView?.setControlPreview(nil) }
            if state.error != nil { self.duoView?.rejectOrientation() }
            self.report()
        }
        client.onDuoUnavailable = { [weak self] _ in self?.phoneFailed = true; self?.refresh(); self?.report() }
        client.onPanelFrame = { [weak self] panel, image in self?.duoView?.frameUpdated(panel, image) }
        if platform == "android" {
            fold = R9DeviceFold(deviceId: deviceId, hostId: hostId, access: access) { [weak self] in
                guard let self else { return }
                self.phone?.setFoldAngle(self.fold?.angle)
                self.report()
            }
        }
        client.onMjpeg = { [weak self] in self?.refresh(); self?.report() }
        client.onFrame = { [weak self] image in self?.show(image) }
    }
    required init?(coder: NSCoder) { nil }

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func layout() { super.layout(); layoutPicture(); phone?.frame = bounds; duoView?.frame = bounds }

    var reportValue: [String: Any] {
        var value: [String: Any] = ["status": status, "detail": detail, "inputConnected": inputConnected, "inputDetail": inputDetail,
                                    "mjpeg": client.mjpeg, "recovered": client.recovered, "phone": showPhone, "phoneUnavailable": phoneUnavailableReason ?? "",
                                    "restartNotice": restartNotice && retainingAndroidFrame && showPhone, "retaining": retainingAndroidFrame]
        let shown = displayedSize
        if shown.width > 0 { value["width"] = Double(shown.width); value["height"] = Double(shown.height) }
        if let screen = client.screen { value["orientation"] = screen.orientation }
        value["ax"] = overlay.count
        if let duo = client.screen?.duo { value["duo"] = duo.report.merging(duoState.report) { _, new in new } }
        // Lane r10-device: the Duo's panel feeds (fixed 1 / 3, or the elected 0), for drive evidence.
        if !client.panelClients.isEmpty { value["duoFeeds"] = client.panelClients.map { ["panel": $0.panelId ?? -1, "reopened": $0.reopened, "recovered": $0.recovered, "mjpeg": $0.mjpeg] } }
        if let duoView { value["duoReady"] = duoView.readyDisplay }
        if let fold { value["fold"] = fold.report }
        return value
    }

    // MARK: Presentation

    /// iOS streams the raw framebuffer; a device reporting landscape over portrait frames is rotated.
    var rotation: Double {
        guard platform == "ios", let screen = client.screen, screen.width <= screen.height else { return 0 }
        switch screen.orientation {
        case "landscape_left": return 90
        case "landscape_right": return -90
        case "portrait_upside_down": return 180
        default: return 0
        }
    }

    /// The displayed aspect (width / height) as the user sees the device.
    var aspect: Double {
        guard let screen = client.screen else { return platform == "ios" ? 9 / 19.5 : 9 / 20 }
        let landscape = screen.orientation == "landscape_left" || screen.orientation == "landscape_right"
        let long = max(screen.width, screen.height), short = min(screen.width, screen.height)
        return landscape ? long / short : short / long
    }

    var displayedSize: CGSize {
        guard imageSize.width > 0 else { return .zero }
        return abs(rotation) == 90 ? CGSize(width: imageSize.height, height: imageSize.width) : imageSize
    }

    /// fitDeviceFrame: the largest box at `aspect` inside the view, centred.
    var frameRect: CGRect {
        let fitted = R7DeviceGeometry.fit(aspect: aspect, width: Double(bounds.width), height: Double(bounds.height))
        return CGRect(x: (bounds.width - fitted.width) / 2, y: (bounds.height - fitted.height) / 2, width: fitted.width, height: fitted.height)
    }

    var retainingAndroidFrame: Bool { platform == "android" && status == "connecting" && inputConnected && client.screen != nil }

    var phoneUnavailableReason: String? {
        if isDuo, client.screen?.duo?.supportsHingeAngle != true { return "iPhone Duo 3D requires Device Hub 0.11.0 or newer" }
        if phoneFailed { return "3D is unavailable on this browser" }
        if client.mjpeg { return "3D requires the H.264 stream" }
        if axOverlay { return "Turn off accessibility frames to use 3D" }
        return nil
    }

    var showPhone: Bool {
        presentation.hasPrefix("phone") && (status == "streaming" || retainingAndroidFrame) && phoneUnavailableReason == nil && window != nil
    }

    var isDuo: Bool { R7DeviceModels.id(platform: platform, name: deviceName) == "iphone-duo" }

    private func refresh() {
        updateFold()
        if showPhone && isDuo {
            if duoView == nil {
                let view = R9DuoView(touch: { [weak self] phase, x, y in self?.client.sendRawTouch(phase, x: x, y: y) },
                                     unavailable: { [weak self] in self?.phoneFailed = true; self?.refresh(); self?.report() },
                                     panelRequested: { [weak self] panel in self?.client.controlDuo(.physical(panel == 1 ? "facedown" : "faceup")) },
                                     orientationRequested: { [weak self] value in self?.client.controlDuo(.orientation(value)) },
                                     hinge: { [weak self] value in self?.client.controlDuo(.angle(value)) })
                view.frame = bounds; view.autoresizingMask = [.width, .height]
                addSubview(view)
                duoView = view
                view.setScreen(client.screen)
                view.origin = client.origin
                if let image, let panel = client.screen?.duo?.screenId { view.frameUpdated(panel, image) }
                client.setDuoPanels(true)
            }
        } else if let view = duoView { view.dispose(); view.removeFromSuperview(); duoView = nil; client.setDuoPanels(false) }
        if showPhone && !isDuo {
            if phone == nil {
                let view = R7DevicePhoneView(platform: platform, name: deviceName, touch: { [weak self] phase, x, y in self?.client.sendTouch(phase, x: x, y: y) },
                                             unavailable: { [weak self] in self?.phoneFailed = true; self?.refresh(); self?.report() })
                view.frame = bounds; view.autoresizingMask = [.width, .height]
                addSubview(view)
                phone = view
                view.setScreen(client.screen)
                view.origin = client.origin
                view.setFoldAngle(fold?.angle)
                if let image { view.setFrame(image) }
            }
            phone?.setKeyboard(presentation == "phone-keyboard")
        }
        if !(showPhone && !isDuo), let view = phone { view.dispose(); view.removeFromSuperview(); phone = nil }
        picture.isHidden = showPhone
        overlay.isHidden = showPhone || !axOverlay
        if retainingAndroidFrame && showPhone {
            if restartTimer == nil {
                let item = DispatchWorkItem { [weak self] in self?.restartTimer = nil; self?.restartNotice = true; self?.report() }
                restartTimer = item
                DispatchQueue.main.asyncAfter(deadline: .now() + 2, execute: item)
            }
        } else { restartTimer?.cancel(); restartTimer = nil }
    }

    private func show(_ image: CGImage) {
        self.image = image
        let size = CGSize(width: image.width, height: image.height)
        let resized = size != imageSize
        imageSize = size
        CATransaction.begin(); CATransaction.setDisableActions(true)
        picture.contents = image
        CATransaction.commit()
        phone?.setFrame(image)
        if resized { layoutPicture(); report() }
    }

    private func layoutPicture() {
        CATransaction.begin(); CATransaction.setDisableActions(true)
        let frame = frameRect, turn = rotation
        picture.setAffineTransform(.identity)
        if abs(turn) == 90 {
            picture.bounds = CGRect(x: 0, y: 0, width: frame.height, height: frame.width)
        } else { picture.bounds = CGRect(origin: .zero, size: frame.size) }
        picture.position = CGPoint(x: frame.midX, y: frame.midY)
        // The layer tree is flipped (isFlipped), so a CSS clockwise turn is a positive angle here.
        picture.setAffineTransform(CGAffineTransform(rotationAngle: CGFloat(turn * .pi / 180)))
        overlay.frame = frame
        overlay.layoutElements()
        CATransaction.commit()
    }

    // MARK: Accessibility frames

    /// Reads the hub's accessibility tree; set while the overlay is on (DeviceStreamView polls it
    /// every 2 s and keeps the last good tree when a read fails).
    var axSource: ((@escaping ([R7AxElement]?) -> Void) -> Void)? {
        didSet { axGeneration += 1; overlay.set([]); if axSource != nil { pollAx(axGeneration) } }
    }
    private var axGeneration = 0
    private func pollAx(_ run: Int) {
        guard let axSource, run == axGeneration, client.stopped == false else { return }
        axSource { [weak self] elements in
            DispatchQueue.main.async {
                guard let self, run == self.axGeneration else { return }
                if let elements { self.overlay.set(elements); self.report() }
                DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in self?.pollAx(run) }
            }
        }
    }

    /// resetView: an iPad with its keyboard turns to landscape, anything else to portrait, then the
    /// 3D pose returns to rest.
    func resetView(keyboard: Bool) {
        if R7DeviceModels.id(platform: platform, name: deviceName) != "iphone-duo" {
            let orientation = keyboard ? "landscape_right" : "portrait"
            if client.screen?.orientation != orientation { client.setOrientation(orientation) }
        }
        phone?.resetPose()
        duoView?.resetPose()
    }

    /// DeviceAndroidFoldControls' read effect: visible, streaming and sized.
    private func updateFold() {
        fold?.update(visible: window != nil && !presentation.isEmpty, enabled: status == "streaming", width: client.screen?.width, height: client.screen?.height)
    }

    /// A Duo stand (`pose:<id>`) or a fold posture from the rail.
    func folding(_ action: String, _ value: String) {
        if action == "fold" { return fold?.set(value, enabled: status == "streaming") ?? () }
        guard let command = R9DuoCommand.parse(value) else { return }
        duoView?.end(); duoView?.cancelInput()
        client.controlDuo(command)
    }

    // MARK: Input

    func start() {
        client.start()
        if axSource != nil { axGeneration += 1; pollAx(axGeneration) }
    }
    override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); refresh() }
    func stop() { fold?.stop(); duoView?.dispose(); duoView?.removeFromSuperview(); duoView = nil; client.stop(); inputConnected = false; restartTimer?.cancel(); restartTimer = nil; axGeneration += 1; phone?.dispose(); phone?.removeFromSuperview(); phone = nil }
    func reconnect() { client.restart() }

    func press(_ action: String) {
        if action == "rotate" { client.rotate() } else { client.pressButton(action) }
    }

    /// A touch at a point in this view, normalized to the displayed frame.
    func touch(_ phase: String, at point: CGPoint) {
        let rect = frameRect
        guard rect.width > 0, rect.height > 0 else { return }
        let x = min(1, max(0, (point.x - rect.minX) / rect.width)), y = min(1, max(0, (point.y - rect.minY) / rect.height))
        client.sendTouch(phase, x: Double(x), y: Double(y))
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        let point = convert(event.locationInWindow, from: nil)
        guard !showPhone, frameRect.contains(point) else { return }
        touching = true
        touch("begin", at: point)
    }
    override func mouseDragged(with event: NSEvent) { if touching { touch("move", at: convert(event.locationInWindow, from: nil)) } }
    override func mouseUp(with event: NSEvent) { if touching { touching = false; touch("end", at: convert(event.locationInWindow, from: nil)) } }

    /// onKeyDown: ⌘ chords stay with the app, except ⌘R (the device's reload).
    override func keyDown(with event: NSEvent) {
        if event.modifierFlags.contains(.command), event.charactersIgnoringModifiers?.lowercased() != "r" { return super.keyDown(with: event) }
        client.sendKey(phase: "down", keyCode: event.keyCode, characters: event.characters, flags: event.modifierFlags)
    }
    override func keyUp(with event: NSEvent) {
        client.sendKey(phase: "up", keyCode: event.keyCode, characters: event.characters, flags: event.modifierFlags)
    }
    override func flagsChanged(with event: NSEvent) {
        guard let phase = R7DeviceKeys.modifierPhase(keyCode: event.keyCode, flags: event.modifierFlags) else { return }
        if phase == "down", event.modifierFlags.contains(.command) { return }
        client.sendKey(phase: phase, keyCode: event.keyCode, characters: nil, flags: event.modifierFlags)
    }
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        // ⌘R reaches the device while the screen has focus (DeviceStreamView keeps it).
        guard window?.firstResponder === self, event.type == .keyDown, event.modifierFlags.contains(.command), event.charactersIgnoringModifiers?.lowercased() == "r" else { return super.performKeyEquivalent(with: event) }
        client.sendKey(phase: "down", keyCode: event.keyCode, characters: event.characters, flags: event.modifierFlags)
        return true
    }
}
/// The hooked screens, their reports, input and screenshots.
final class R6DeviceStreams {
    private var views: [ObjectIdentifier: R6DeviceScreenView] = [:]
    private let access: R6DeviceScreenView.Access
    private let changed: (String) -> Void

    private let tools: R7DeviceToolFeeds

    init(access: @escaping R6DeviceScreenView.Access, changed: @escaping (String) -> Void) {
        self.access = access; self.changed = changed
        tools = R7DeviceToolFeeds(access: access, changed: changed)
    }

    /// key → the panel's report when the panel shows it, else the most advanced among the views
    /// showing it (the floating player); plus the Tools drawer's feeds.
    var status: [String: Any] {
        var reports: [String: [String: Any]] = [:], panels = Set<String>()
        let rank = ["error": 0, "connecting": 1, "streaming": 2]
        for view in views.values.sorted(by: { !$0.presentation.isEmpty && $1.presentation.isEmpty }) {
            let value = view.reportValue
            if panels.contains(view.key) { continue }
            if !view.presentation.isEmpty { panels.insert(view.key); reports[view.key] = value; continue }
            if let current = reports[view.key], rank[current["status"] as? String ?? ""] ?? 0 >= rank[value["status"] as? String ?? ""] ?? 0 { continue }
            reports[view.key] = value
        }
        return ["deviceStreams": reports, "deviceTools": tools.status]
    }

    func install(_ element: ExactElement) {
        guard element.hook == .t3Media, let host = element.view, let kind = element.data[.mediaKind], kind.hasPrefix("device-") else { return }
        let deviceId = element.data[.deviceId] ?? "", hostId = element.data[.deviceHost] ?? ""
        let key = ObjectIdentifier(host), platform = String(kind.dropFirst("device-".count)), deviceKey = "\(hostId)\u{0}\(deviceId)"
        if platform == "events" { return tools.watchEvents(host: key, key: deviceKey, deviceId: deviceId, hostId: hostId) }
        if let current = views[key], current.key == deviceKey, current.platform == platform { return configure(current, element) }
        detach(key)
        guard !deviceId.isEmpty else { return }
        let view = R6DeviceScreenView(key: deviceKey, platform: platform, deviceId: deviceId, hostId: hostId, access: access) { [weak self] in self?.changed("t3.status") }
        view.frame = host.bounds
        view.autoresizingMask = [.width, .height]
        host.addSubview(view)
        views[key] = view
        configure(view, element)
        view.start()
        changed("t3.status")
    }

    /// The panel's presentation choices, the device's name and the foreground-app feed.
    private func configure(_ view: R6DeviceScreenView, _ element: ExactElement) {
        view.deviceName = element.data[.mediaName] ?? ""
        view.presentation = element.data[.deviceView] ?? ""
        view.axOverlay = element.data[.deviceAx] == "1"
        view.axSource = view.axOverlay ? { [tools] done in tools.readAx(platform: view.platform, deviceId: view.deviceId, hostId: view.hostId, done) } : nil
        tools.watchForeground(panels: views.values.filter { !$0.presentation.isEmpty }.map { ($0.key, $0.platform, $0.deviceId, $0.hostId) })
        changed("t3.status")
    }

    func remove(_ element: ExactElement) {
        if let host = element.view { tools.unwatchEvents(host: ObjectIdentifier(host)); detach(ObjectIdentifier(host)) }
        for (key, view) in views where view.superview == nil { view.stop(); views[key] = nil }
    }

    func destroy() { for key in Array(views.keys) { detach(key) }; tools.destroy() }

    private func detach(_ key: ObjectIdentifier) {
        guard let view = views.removeValue(forKey: key) else { return }
        view.stop(); view.removeFromSuperview()
        tools.watchForeground(panels: views.values.filter { !$0.presentation.isEmpty }.map { ($0.key, $0.platform, $0.deviceId, $0.hostId) })
        changed("t3.status")
    }

    /// `r6DeviceInput`: Home, Back, Recents, Rotate, Reconnect, "orientation" (value) and "reset"
    /// (Restore 3D view) on every view showing that device.
    func input(_ request: [String: Any]) -> [String: Any] {
        let action = request["action"] as? String ?? "", key = request["key"] as? String ?? "", value = request["value"] as? String ?? ""
        let targets = views.values.filter { $0.key == key }
        for view in targets {
            switch action {
            case "reconnect": view.reconnect()
            case "orientation": view.client.setOrientation(value)
            case "reset": view.resetView(keyboard: value == "keyboard")
            case "duo", "fold": view.folding(action, value)
            default: view.press(action)
            }
        }
        return ["ok": true, "generation": request["generation"] as? Int ?? 0, "value": ["delivered": targets.count]]
    }

    /// `r6DeviceScreenshot` (captureDeviceScreenshot): POST the hub's screenshot route, then save
    /// the PNG where the person chooses (under the agent, the data root's exports/).
    static func screenshot(_ request: [String: Any], access: @escaping R6DeviceScreenView.Access, exportsRoot: URL?, reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let done = { (value: [String: Any]) in reply(["ok": true, "generation": generation, "value": value]) }
        let platform = request["platform"] as? String ?? "ios", deviceId = request["deviceId"] as? String ?? "", hostId = request["hostId"] as? String ?? ""
        let name = (request["name"] as? String ?? "device").replacingOccurrences(of: "[^A-Za-z0-9-]", with: "-", options: .regularExpression)
        access { origin, ticket in
            guard let origin, let ticket, let url = R6DeviceWire.url(origin: origin, path: "\(R6DeviceWire.vendor(platform))/api/screenshot", query: [URLQueryItem(name: "device", value: deviceId)], ticket: ticket, hostId: hostId) else {
                return done(["ok": false, "message": "Reconnect to the environment and try again."])
            }
            var post = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 20)
            post.httpMethod = "POST"
            URLSession(configuration: .ephemeral).dataTask(with: post) { data, response, error in
                let code = (response as? HTTPURLResponse)?.statusCode ?? 0
                guard error == nil, (200..<300).contains(code) else { return done(["ok": false, "message": "Screenshot capture failed (\(code)). Try again."]) }
                guard let data, !data.isEmpty else { return done(["ok": false, "message": "The device returned an empty screenshot. Try again."]) }
                let file = "\(name)-\(Int(Date().timeIntervalSince1970 * 1000)).png"
                let write = { (target: URL) in
                    do {
                        try FileManager.default.createDirectory(at: target.deletingLastPathComponent(), withIntermediateDirectories: true)
                        try data.write(to: target, options: .atomic)
                        done(["ok": true, "saved": true, "name": target.lastPathComponent])
                    } catch { done(["ok": false, "message": "Could not save \(file)."]) }
                }
                if let exportsRoot { return write(exportsRoot.appendingPathComponent(file)) }
                DispatchQueue.main.async {
                    let panel = NSSavePanel()
                    panel.nameFieldStringValue = file
                    panel.allowedContentTypes = [.png]
                    guard panel.runModal() == .OK, let target = panel.url else { return done(["ok": true, "saved": false]) }
                    DispatchQueue.global(qos: .userInitiated).async { write(target) }
                }
            }.resume()
        }
    }
}
#endif
