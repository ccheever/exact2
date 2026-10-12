#if os(macOS)
import AppKit
import Foundation

/// Lane r7-device: one device's stream and input (MIT reference, see LICENSE-T3:
/// packages/client-runtime/src/device/stream.ts createDeviceStreamClient). iOS reads serve-sim's
/// `stream.avcc` (falling back to `stream.mjpeg` when the description cannot be decoded) and
/// talks to `helper/ws?device=` after priming the helper; Android opens serve-emu's one
/// `ws?device=<serial>&frame-meta=1` socket for H.264 and JSON input. Everything runs on main.
final class R7DeviceClient: NSObject, URLSessionDataDelegate, URLSessionWebSocketDelegate {
    typealias Access = (@escaping (URL?, String?) -> Void) -> Void
    struct Screen: Equatable { var width: Double; var height: Double; var orientation: String; var duo: R9DuoConfig? = nil }
    static let retryDelay: TimeInterval = 1, firstFrameTimeout: TimeInterval = 15, frameDuration: Int64 = 16_667

    let platform: String, deviceId: String, hostId: String
    private let access: Access
    var onStatus: (String, String) -> Void = { _, _ in }
    var onScreen: (Screen) -> Void = { _ in }
    var onInput: (Bool, String) -> Void = { _, _ in }
    var onFrame: (CGImage) -> Void = { _ in }
    var onMjpeg: () -> Void = {}
    // Lane r9-device (R9DeviceDuo.swift): the Duo's command queue, its fixed panel feeds and their frames.
    var onDuoControl: (R9DuoControlState) -> Void = { _ in }
    var onDuoUnavailable: (String) -> Void = { _ in }
    var onPanelFrame: (Int, CGImage) -> Void = { _, _ in }
    /// A fixed Duo panel's video-only feed (`/helper/<id>/panel/<n>/stream.avcc`), or the elected surface's (0).
    let panelId: Int?
    private(set) var panelClients: [R7DeviceClient] = []
    private(set) var duoPanels = false
    private var rotationCursor: String?
    private var pendingOrientation: Int?
    lazy var duoControl = R9DuoControl(send: { [weak self] id, command in self?.sendDuo(id, command) ?? false },
                                       onChange: { [weak self] state in if !state.pending { self?.pendingOrientation = nil }; self?.onDuoControl(state) })

    private var session: URLSession?
    private var video: URLSessionDataTask?
    private var prime: URLSessionDataTask?
    private var socket: URLSessionWebSocketTask?
    private(set) var origin: URL?
    private var ticket: String?
    private var decoder: R7H264Decoder?
    /// Lane r11-device: the soft queue measured over time on the device's own feed (R11DeviceBacklog.swift).
    private var backlog = R11DecodeBacklog()
    private let demuxer = R7H264.Demuxer()
    private var mjpegBuffer = Data()
    private(set) var stopped = true
    private(set) var mjpeg = false
    /// Why the decoder was last recovered ("queue", "error", "description"), for the report.
    private(set) var recovered = ""
    private(set) var screen: Screen?
    private(set) var sent: [Data] = []
    private var generation = 0
    private var firstFrame = false
    private var awaitingKeyframe = true
    private var configuring = false
    private var timestamp: Int64 = 0
    private var frameTimer: DispatchWorkItem?
    private var stallTimer: DispatchWorkItem?
    private var retries: [String: DispatchWorkItem] = [:]
    private var unauthorized = 0
    private var socketOpen = false

    init(platform: String, deviceId: String, hostId: String, access: @escaping Access, panelId: Int? = nil) {
        self.platform = platform; self.deviceId = deviceId; self.hostId = hostId; self.access = access; self.panelId = panelId
    }

    private var vendor: String { R6DeviceWire.vendor(platform) }
    private var encodedDevice: String { deviceId.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? deviceId }
    private func http(_ path: String, _ query: [URLQueryItem] = []) -> URL? {
        guard let origin, let ticket else { return nil }
        return R6DeviceWire.url(origin: origin, path: vendor + path, query: query, ticket: ticket, hostId: hostId)
    }
    private func ws(_ path: String, _ query: [URLQueryItem]) -> URL? {
        guard let origin, let ticket else { return nil }
        return R6DeviceWire.url(origin: origin, path: vendor + path, query: query, ticket: ticket, hostId: hostId, websocket: true)
    }

    // MARK: Lifecycle

    func start() {
        guard stopped else { return }
        stopped = false; generation += 1; configuring = false; mjpeg = false
        connecting()
        session = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: .main)
        let run = generation
        access { [weak self] origin, ticket in
            DispatchQueue.main.async {
                guard let self, !self.stopped, self.generation == run else { return }
                guard let origin, let ticket else { return self.fail("Reconnect to the environment and try again.") }
                self.origin = origin; self.ticket = ticket
                if self.panelId != nil { self.readIosVideo() } else if self.platform == "ios" { self.connectIosInput(); self.readIosVideo() } else { self.connectAndroid() }
            }
        }
    }

    func stop() {
        guard !stopped else { return }
        stopped = true; generation += 1
        if panelId == nil { duoControl.clear(); rotationCursor = nil; panelClients.forEach { $0.stop() }; panelClients = []; duoPanels = false }
        frameTimer?.cancel(); frameTimer = nil; stallTimer?.cancel(); stallTimer = nil
        retries.values.forEach { $0.cancel() }; retries = [:]
        prime?.cancel(); prime = nil; video?.cancel(); video = nil
        let discarded = socket; socket = nil; socketOpen = false
        discarded?.cancel(with: .goingAway, reason: nil)
        session?.invalidateAndCancel(); session = nil
        closeDecoder(); mjpeg = false; mjpegBuffer = Data(); demuxer.reset()
    }

    /// The Reconnect button: stop, then start with fresh access.
    func restart() { stop(); unauthorized = 0; start() }

    private func setStatus(_ status: String, _ detail: String = "") { if !stopped { onStatus(status, detail) } }

    private func fail(_ detail: String) {
        guard !stopped else { return }
        stop()
        onInput(false, detail)
        onStatus("error", detail)
    }

    private func connecting(_ detail: String = "") {
        firstFrame = false
        if frameTimer == nil {
            let item = DispatchWorkItem { [weak self] in self?.frameTimer = nil; self?.fail("No video received from the device. Reconnect to try again.") }
            frameTimer = item
            DispatchQueue.main.asyncAfter(deadline: .now() + Self.firstFrameTimeout, execute: item)
        }
        setStatus("connecting", detail)
    }

    private func frameReceived() {
        guard !firstFrame else { return }
        firstFrame = true; unauthorized = 0
        frameTimer?.cancel(); frameTimer = nil
        setStatus("streaming")
    }

    /// handleUnauthorized: the proxy refused the ticket; mint a fresh one and start again.
    private func handleUnauthorized() {
        stop()
        onInput(false, "")
        unauthorized += 1
        guard unauthorized <= 3 else { return onStatus("error", "Could not receive the device stream. Reconnect to try again.") }
        DispatchQueue.main.asyncAfter(deadline: .now() + (unauthorized > 1 ? Self.retryDelay : 0)) { [weak self] in
            guard let self, self.stopped else { return }
            self.start()
        }
    }

    private func scheduleRetry(_ channel: String, _ run: @escaping () -> Void) {
        guard !stopped, retries[channel] == nil else { return }
        let item = DispatchWorkItem { [weak self] in self?.retries[channel] = nil; run() }
        retries[channel] = item
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.retryDelay, execute: item)
    }

    // MARK: Decoding

    private func paint(_ image: CGImage) {
        guard !stopped else { return }
        let width = Double(image.width), height = Double(image.height)
        if platform == "android", screen?.width != width || screen?.height != height {
            screen = Screen(width: width, height: height, orientation: width > height ? "landscape_left" : "portrait")
            onScreen(screen!)
        }
        onFrame(image)
        frameReceived()
    }

    private func makeDecoder() -> R7H264Decoder {
        let run = generation
        var made: R7H264Decoder?
        made = R7H264Decoder(onFrame: { [weak self] image in
            guard let self, self.generation == run, self.decoder === made else { return }
            self.paint(image)
        }, onError: { [weak self] in
            guard let self, !self.stopped, self.generation == run, self.decoder === made else { return }
            self.recovered = "error"; self.recoverDecoder()
        })
        return made!
    }

    private func closeDecoder() { decoder?.close(); decoder = nil; awaitingKeyframe = true; backlog.reset() }

    private func recoverDecoder() {
        if panelId != nil { return reopenPanelVideo() }
        if platform == "ios" { fallBackToMjpeg() } else { closeDecoder(); connecting("Video decoder restarted."); requestKeyframe() }
    }

    /// Lane r10-device: a fixed or elected Duo feed has no MJPEG picture of its own. One that falls
    /// behind (a stalled main thread receives more samples at once than the soft queue) or whose
    /// decoder fails reopens its video, which starts again at a description and a keyframe, instead
    /// of going dark until its first-frame timer gives up (the round-9 black cover after Closed).
    /// A second recovery within the retry delay waits for it.
    private var lastReopen = -Double.infinity
    private(set) var reopened = 0
    private func reopenPanelVideo() {
        guard !stopped else { return }
        let previous = video; video = nil; previous?.cancel()
        stallTimer?.cancel(); stallTimer = nil
        closeDecoder(); demuxer.reset()
        connecting("Video decoder restarted.")
        reopened += 1
        let now = CACurrentMediaTime(), soon = now - lastReopen < Self.retryDelay
        lastReopen = now
        if soon { scheduleRetry("video") { [weak self] in self?.readIosVideo() } } else { readIosVideo() }
    }

    private func decode(isKey: Bool, sample: Data, pts: UInt64?) {
        guard let decoder, decoder.configured else { return }
        if awaitingKeyframe { guard isKey else { return }; awaitingKeyframe = false }
        // A Duo panel feed reopens at once (lane r10-device); the device's own feed waits out a burst.
        let pending = decoder.queueSize
        if panelId != nil ? pending > R7H264.softDecodeQueue : backlog.behind(pending: pending, now: CACurrentMediaTime()) { recovered = "queue"; return recoverDecoder() }
        decoder.decode(sample: sample, pts: pts.map { Int64(clamping: $0) } ?? timestamp)
        timestamp += Self.frameDuration
    }

    private func requestKeyframe() {
        if platform == "android", socketOpen { send(text: R7DeviceKeys.json(["type": "reset-video", "ack": false])) }
    }

    // MARK: iOS

    private func readIosVideo() {
        guard !stopped, !duoPanels, let url = http("/helper/\(encodedDevice)\(panelId.map { $0 > 0 ? "/panel/\($0)" : "" } ?? "")/stream.avcc") else { return }
        demuxer.reset(); closeDecoder()
        video = session?.dataTask(with: URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 60))
        video?.resume()
        armStall()
    }

    private func armStall() {
        stallTimer?.cancel()
        let run = generation
        let item = DispatchWorkItem { [weak self] in
            guard let self, self.generation == run, self.video != nil, !self.mjpeg else { return }
            self.fail("Device stream stopped receiving video. Reconnect to try again.")
        }
        stallTimer = item
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.firstFrameTimeout, execute: item)
    }

    private func fallBackToMjpeg() {
        guard !stopped, !mjpeg else { return }
        mjpeg = true
        let previous = video; video = nil; previous?.cancel()
        stallTimer?.cancel(); stallTimer = nil
        closeDecoder()
        connecting()
        // A fixed panel feed has no picture of its own to fall back to; its first-frame timer reports it.
        if panelId != nil { return }
        onMjpeg()
        guard let url = http("/helper/\(encodedDevice)/stream.mjpeg") else { return }
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 30)
        request.setValue("multipart/x-mixed-replace, image/jpeg", forHTTPHeaderField: "Accept")
        mjpegBuffer = Data()
        video = session?.dataTask(with: request); video?.resume()
    }

    /// primeIosHelper, then the helper's input socket.
    private func connectIosInput() {
        guard !stopped, let url = http("/helper/\(encodedDevice)/stream.mjpeg") else { return }
        let task = session?.dataTask(with: URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 2))
        prime = task
        task?.resume()
        let run = generation
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
            guard let self, self.generation == run, self.prime === task else { return }
            self.finishPrime()
        }
    }

    private func finishPrime() {
        guard let task = prime else { return }
        prime = nil; task.cancel()
        guard !stopped, let url = ws("/helper/ws", [URLQueryItem(name: "device", value: deviceId)]) else { return }
        openSocket(url)
    }

    // MARK: Android

    private func connectAndroid() {
        guard !stopped, let url = ws("/ws", [URLQueryItem(name: "device", value: deviceId), URLQueryItem(name: "frame-meta", value: "1")]) else { return }
        openSocket(url)
    }

    private func openSocket(_ url: URL) {
        guard let session else { return }
        let task = session.webSocketTask(with: url)
        task.maximumMessageSize = 32 * 1024 * 1024
        socket = task; socketOpen = false
        task.resume()
        receive(task, run: generation)
    }

    private func androidMessage(_ data: Data) {
        let packet = R7H264.parseSemu(data)
        let ready = decoder?.configured == true
        let needsScan = packet.isKey == nil || (packet.isKey == true && !ready)
        let scanned = needsScan ? R7H264.scan(packet.data) : nil
        let isKey = packet.isKey ?? scanned?.isKey ?? false
        if let sps = scanned?.sps, !ready {
            guard !configuring else { return }
            configuring = true
            guard let pps = scanned?.pps else { configuring = false; return requestKeyframe() }
            let next = makeDecoder()
            guard next.configure(sps: sps, pps: pps) else { configuring = false; return fail("This client cannot decode \(R7H264.codecString(sps)).") }
            decoder = next; configuring = false; awaitingKeyframe = true
            return requestKeyframe()
        }
        guard ready else { if !isKey { requestKeyframe() }; return }
        decode(isKey: isKey, sample: R7H264.avccSample(annexB: packet.data), pts: packet.timestamp)
    }

    // MARK: Sockets

    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol protocol: String?) {
        guard webSocketTask === socket, !stopped else { return }
        socketOpen = true
        if platform == "ios" { send(R6DeviceWire.tagged(R6DeviceWire.hardwareKeyboard, ["enabled": false])) } else { connecting() }
        onInput(true, "")
    }

    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?) {
        socketClosed(webSocketTask, code: closeCode.rawValue, reason: reason.flatMap { String(data: $0, encoding: .utf8) } ?? "")
    }

    private func socketClosed(_ task: URLSessionWebSocketTask, code: Int, reason: String) {
        guard task === socket else { return }
        socket = nil; socketOpen = false
        duoControl.clear(); rotationCursor = nil
        if platform == "android" { closeDecoder() }
        guard !stopped else { return }
        let detail = !reason.isEmpty ? reason : platform == "ios" && code == 1006 ? "input socket refused" : "closed \(code)"
        onInput(false, detail)
        // A rejected HTTP upgrade surfaces as 1006, including an expired stream ticket.
        if code == 1008 || code == 4401 || code == 1006 { return handleUnauthorized() }
        if platform == "ios" { scheduleRetry("input") { [weak self] in self?.connectIosInput() } }
        else { configuring = false; connecting(reason); scheduleRetry("input") { [weak self] in self?.connectAndroid() } }
    }

    private func receive(_ task: URLSessionWebSocketTask, run: Int) {
        task.receive { [weak self] result in
            DispatchQueue.main.async {
                guard let self, self.socket === task, self.generation == run, !self.stopped else { return }
                switch result {
                case .failure:
                    // URLSession reports a refused or dropped socket here, not through didClose.
                    let code = task.closeCode == .invalid ? 1006 : task.closeCode.rawValue
                    self.socketClosed(task, code: code, reason: task.closeReason.flatMap { String(data: $0, encoding: .utf8) } ?? "")
                case .success(let message):
                    switch message {
                    case .data(let data): self.platform == "ios" ? self.helperMessage(data) : self.androidMessage(data)
                    case .string(let text): if self.platform == "android" { self.androidText(text) }
                    @unknown default: break
                    }
                    self.receive(task, run: run)
                }
            }
        }
    }

    private func androidText(_ text: String) {
        // The encoder restarts at a new size when the device rotates; the next keyframe carries a fresh SPS.
        guard let object = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any], object["type"] as? String == "video-session" else { return }
        closeDecoder(); configuring = false
        connecting()
        requestKeyframe()
    }

    private func helperMessage(_ data: Data) {
        guard let tag = data.first, let payload = try? JSONSerialization.jsonObject(with: data.dropFirst()) as? [String: Any] else { return }
        // Lane r9-device: a hinge command's reply.
        if tag == 0x90 {
            guard let id = (payload["requestId"] as? NSNumber).flatMap({ Int(exactly: $0.doubleValue) }), let ok = payload["ok"] as? Bool else { return }
            return duoControl.receive(requestId: id, ok: ok, error: payload["error"] as? String)
        }
        guard tag == R6DeviceWire.screenConfig,
              let width = (payload["width"] as? NSNumber)?.doubleValue, let height = (payload["height"] as? NSNumber)?.doubleValue, width > 0, height > 0,
              let orientation = payload["orientation"] as? String, R6DeviceWire.orientations.contains(orientation),
              case .success(let duo) = R9DuoConfig.parse(payload) else { return }
        let previous = screen
        screen = Screen(width: width, height: height, orientation: orientation, duo: duo)
        if let pose = duo?.hingePose, pose != previous?.duo?.hingePose { rotationCursor = pose == "laptop" ? "landscape_left" : "portrait" }
        else if orientation != previous?.orientation { rotationCursor = orientation }
        onScreen(screen!)
        // A surface election can leave a decoder on the former encoder description: reopen only video.
        if duoPanels, duo?.supportsPhysicalOrientation == true, let previous, duo?.screenId != previous.duo?.screenId { startDuoVideo() }
        if let receipt = pendingOrientation {
            pendingOrientation = nil
            duoControl.receive(requestId: receipt, ok: true, error: nil)
        }
    }

    // MARK: Duo (lane r9-device)

    /// createDuoControl's send: hinge commands as `0x10`, orientation as the ordinary `0x07`, which
    /// the next screen config acknowledges.
    private func sendDuo(_ requestId: Int, _ command: R9DuoCommand) -> Bool {
        guard socketOpen, !stopped, let duo = screen?.duo, duo.supportsHingeAngle else { return false }
        if case .physical = command, !duo.supportsPhysicalOrientation { return false }
        pendingOrientation = nil
        if case .orientation(let value) = command {
            pendingOrientation = requestId
            rotationCursor = value
            send(R6DeviceWire.tagged(R6DeviceWire.orientation, ["orientation": value]))
        } else {
            send(R6DeviceWire.tagged(0x10, ["requestId": requestId, "command": command.json]))
        }
        return true
    }

    func controlDuo(_ command: R9DuoCommand) { duoControl.enqueue(command) }

    /// setDuoPanels: the 3D Duo takes its pictures from the fixed panel feeds (one elected feed when
    /// the hub hands the surface over physically); off returns to the device's own stream.
    func setDuoPanels(_ on: Bool) {
        guard platform == "ios", panelId == nil, !stopped, on != duoPanels else { return }
        if on, screen?.duo?.supportsHingeAngle != true { return }
        duoPanels = on
        video?.cancel(); video = nil; stallTimer?.cancel(); stallTimer = nil
        closeDecoder(); demuxer.reset()
        retries.removeValue(forKey: "video")?.cancel()
        panelClients.forEach { $0.stop() }; panelClients = []
        if !on { return readIosVideo() }
        startDuoVideo()
    }

    private func startDuoVideo() {
        panelClients.forEach { $0.stop() }
        let ids = screen?.duo?.supportsPhysicalOrientation == true ? [0] : [1, 3]
        panelClients = ids.map { id in
            let panel = R7DeviceClient(platform: platform, deviceId: deviceId, hostId: hostId, access: access, panelId: id)
            panel.onFrame = { [weak self] image in
                guard let self, !self.stopped else { return }
                // An inactive native LCD can emit its shutdown black frame: only the active panel paints.
                let active = self.screen?.duo?.screenId
                guard id == 0 || active == id, let panelId = id == 0 ? active : id else { return }
                self.onPanelFrame(panelId, image)
                self.onFrame(image)
            }
            panel.onStatus = { [weak self] status, detail in if status == "error" { self?.onDuoUnavailable(detail) } }
            return panel
        }
        panelClients.forEach { $0.start() }
    }

    // MARK: Video body

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse, completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        let code = (response as? HTTPURLResponse)?.statusCode ?? 0
        if dataTask === prime {
            completionHandler(code == 401 || code == 403 ? .cancel : .allow)
            if code == 401 || code == 403 { prime = nil; handleUnauthorized() }
            return
        }
        guard dataTask === video else { return completionHandler(.cancel) }
        if code == 401 || code == 403 { completionHandler(.cancel); return handleUnauthorized() }
        if let panelId, panelId > 0, [400, 404, 405, 410].contains(code) {
            completionHandler(.cancel); video = nil
            return setStatus("error", "This Device Hub does not provide fixed Duo display feeds.")
        }
        guard (200..<300).contains(code) else {
            completionHandler(.cancel)
            if mjpeg { return fail("Could not receive the device stream. Reconnect to try again.") }
            return videoEnded("stream \(code)")
        }
        completionHandler(.allow)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        if dataTask === prime { return finishPrime() }
        guard dataTask === video, !stopped else { return }
        if mjpeg { return mjpegData(data) }
        armStall()
        for chunk in demuxer.push(data) {
            switch chunk.kind {
            case .seed:
                if let image = NSImage(data: chunk.payload)?.cgImage(forProposedRect: nil, context: nil, hints: nil) { paint(image) }
            case .description:
                awaitingKeyframe = true
                let next = makeDecoder()
                guard next.configure(avcC: chunk.payload) else {
                    recovered = "description"
                    if panelId != nil { video?.cancel(); video = nil; return setStatus("error", "This client cannot decode the Duo panel's \(R7H264.codecString(chunk.payload)) stream.") }
                    return fallBackToMjpeg()
                }
                decoder = next
            case .keyframe, .delta:
                decode(isKey: chunk.kind == .keyframe, sample: chunk.payload, pts: nil)
            }
            if stopped || mjpeg || dataTask !== video { return }
        }
    }

    private func mjpegData(_ data: Data) {
        mjpegBuffer.append(data)
        let (images, rest) = R6DeviceWire.frames(mjpegBuffer)
        mjpegBuffer = rest.count > 8 * 1024 * 1024 ? Data() : rest
        if let last = images.last, let image = NSImage(data: last)?.cgImage(forProposedRect: nil, context: nil, hints: nil) { paint(image) }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let socket = task as? URLSessionWebSocketTask {
            if socket === self.socket, !stopped { socketClosed(socket, code: 1006, reason: "") }
            return
        }
        if task === prime { prime = nil; return finishPrimeAfterFailure() }
        guard task === video, !stopped, (error as? URLError)?.code != .cancelled else { return }
        if mjpeg { return fail("Could not receive the device stream. Reconnect to try again.") }
        videoEnded(error?.localizedDescription ?? "")
    }

    private func finishPrimeAfterFailure() {
        // A failed prime just means the socket may take a retry to come up.
        guard !stopped, socket == nil, let url = ws("/helper/ws", [URLQueryItem(name: "device", value: deviceId)]) else { return }
        openSocket(url)
    }

    private func videoEnded(_ detail: String) {
        video?.cancel(); video = nil
        stallTimer?.cancel(); stallTimer = nil
        closeDecoder()
        connecting(detail)
        scheduleRetry("video") { [weak self] in self?.readIosVideo() }
    }

    // MARK: Input

    private func send(_ data: Data) {
        guard !stopped, socketOpen, let socket else { return }
        log(data)
        socket.send(.data(data)) { _ in }
    }
    private func send(text: Data) {
        guard !stopped, socketOpen, let socket, let string = String(data: text, encoding: .utf8) else { return }
        log(text)
        socket.send(.string(string)) { _ in }
    }
    private func log(_ data: Data) { sent.append(data); if sent.count > 64 { sent.removeFirst(sent.count - 64) } }
    var inputOpen: Bool { socketOpen && !stopped }

    /// sendTouch: normalized 0…1 in the displayed frame.
    func sendTouch(_ phase: String, x: Double, y: Double) {
        if platform == "ios" {
            let raw = R6DeviceWire.rawPoint(x: x, y: y, screen: screen.map { ($0.width, $0.height, $0.orientation) })
            return send(R6DeviceWire.tagged(R6DeviceWire.touch, ["type": phase, "x": raw.0, "y": raw.1]))
        }
        send(text: R7DeviceKeys.json(["type": "touch", "action": phase == "begin" ? "down" : phase == "move" ? "move" : "up", "x": x, "y": y]))
    }

    /// sendRawTouch: the 3D view's UVs already map to the hardware framebuffer.
    func sendRawTouch(_ phase: String, x: Double, y: Double) {
        if platform == "ios" { send(R6DeviceWire.tagged(R6DeviceWire.touch, ["type": phase, "x": x, "y": y])) } else { sendTouch(phase, x: x, y: y) }
    }

    func sendKey(phase: String, keyCode: UInt16, characters: String?, flags: NSEvent.ModifierFlags) {
        guard let packet = R7DeviceKeys.packet(platform: platform, phase: phase, keyCode: keyCode, characters: characters, meta: flags.contains(.command), control: flags.contains(.control)) else { return }
        platform == "ios" ? send(packet) : send(text: packet)
    }

    func pressButton(_ button: String) {
        if platform == "ios" {
            let name = button == "home" ? "home" : button == "appSwitcher" ? "app_switcher" : button == "power" ? "lock" : nil
            if let name { send(R6DeviceWire.tagged(R6DeviceWire.button, ["button": name])) }
            return
        }
        let type = button == "appSwitcher" ? "recents" : button
        if ["home", "back", "recents", "power"].contains(type) { send(text: R7DeviceKeys.json(["type": type])) }
    }

    func rotate() {
        guard platform == "ios" else { return }
        if screen?.duo?.supportsHingeAngle == true {
            let next = R6DeviceWire.nextOrientation(rotationCursor ?? screen?.orientation)
            rotationCursor = next
            return duoControl.enqueue(.orientation(next))
        }
        send(R6DeviceWire.tagged(R6DeviceWire.orientation, ["orientation": R6DeviceWire.nextOrientation(screen?.orientation)]))
    }

    func setOrientation(_ orientation: String) {
        if platform == "ios" { send(R6DeviceWire.tagged(R6DeviceWire.orientation, ["orientation": orientation])) }
    }
}
#endif
