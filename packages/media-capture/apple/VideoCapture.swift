// @exact/media-capture's video half on Apple (LLP 1133.008 D1-D3, D6):
// cameras as Media Capture's video tracks, each in the session's track table
// with its state and, while leased, its capture (`ExactVideoSource`), which a
// consumer such as @exact/camera's viewfinder attaches outputs to.
// MediaCapture.swift routes its names here for a video track and `video=`.
//
// - Captures are the process's (D3): one `AVCaptureSession` on iOS, which
//   runs one camera at a time, and one per open camera on macOS. Each has a
//   serial queue it is configured, started and stopped on; the main thread
//   never waits on it. It is made at its first lease, with the `.photo`
//   preset (D2), and let go at its last.
// - A track holds a lease while it has a consumer (D3): a consumer that shows
//   the capture takes one whenever its origin is the person's or the app's own
//   code and the app is in the foreground (§14, "a visible consumer"); a
//   tool's or a wake's takes none. The last consumer gone, the track reads
//   `muted`, `idle`. On iOS a lease on another camera while one is live waits,
//   `in-use`, and is taken when that one is let go.
// - Interruptions become `muteReason` (D3); after one other than the
//   background, the session is started again while a lease is live.
// - Under the agent no session is made and nothing is asked: two cameras,
//   "Agent back camera" and "Agent front camera", whose consumers are told
//   `live(video: nil)` (§9).
// - Tier 3 (D5): `MediaCaptureModule.sessionHatch` is called inside each
//   configuration on the session's queue, after the module's input;
//   `deviceHatch` after the track's constraints, under the device's lock.
import AVFoundation
import ExactPackage
import Foundation

/// A video track: its state and its consumers (main thread).
final class VideoTrack {
    let id: String, stream: String
    var label: String, enabled = true, muted = true, muteReason: String? = "idle", readyState = "live"
    var device: AVCaptureDevice?
    /// Under the agent: the camera it shows.
    var facing = "environment"
    var settings: [String: Any] = [:], capabilities: [String: Any] = [:]
    var consumers: [ObjectIdentifier: (consumer: ExactTrackConsumer, origin: String)] = [:]
    /// Outputs that are not consumers (LLP 1133.027 D17 item 1): told what consumers are told, holding no lease.
    var observers: [ObjectIdentifier: ExactTrackConsumer] = [:]
    /// The capture whose lease it holds, if it holds one.
    var capture: VideoCapture?
    /// Leased under the agent, where no capture exists.
    var agentLease = false
    /// Waiting for another camera's lease to end (iOS), with the origin it was admitted with.
    var waiting: String?
    /// Why the platform mutes it now, if it does.
    var platformMute: String?
    weak var tracks: VideoTracks?
    init(id: String, stream: String, label: String) { self.id = id; self.stream = stream; self.label = label }
    var leased: Bool { capture != nil || agentLease }
}

/// One session's video tracks: the owner the track table names for them.
final class VideoTracks: ExactTrackOwner {
    private let context: ExactPackageContext
    private let nextId: () -> Int
    private var tracks: [String: VideoTrack] = [:]
    private var overlay: String?
    private(set) var opened = false

    init(context: ExactPackageContext, nextId: @escaping () -> Int) {
        self.context = context; self.nextId = nextId
    }

    func owns(_ id: String?) -> Bool { id.map { tracks[$0] != nil } ?? false }
    private func say(_ line: String) { context.log("media-capture: \(line)") }
    private func announce() { context.changed("media-capture.mediaStreamTrack"); disclose() }
    /// Under the agent, each track for `state.extensions` (§9, Disclosure): its device, settings, consumers' origins,
    /// lease and mute.
    private func disclose() {
        context.state(["videoTracks": tracks.values.sorted { $0.id < $1.id }.map { t -> [String: Any] in
            var out = shape(t)
            out["consumers"] = t.consumers.values.map(\.origin).sorted()
            out["observers"] = t.observers.count
            out["lease"] = t.leased
            if let w = t.waiting { out["waiting"] = w }
            return out
        }])
    }

    // MARK: the names

    /// A video track for `getUserMedia(video=…)` in `stream`: its shape, or `{error}`.
    func open(_ args: [String: Any], stream: String, reply: @escaping ([String: Any]) -> Void) {
        let constraints = args["videoConstraints"] as? [String: Any] ?? [:]
        let apple = args["apple"] as? [String: Any] ?? [:]
        let make = { [self] (device: AVCaptureDevice?) in
            let n = nextId()
            let t = VideoTrack(id: "track-\(context.session)-\(n)", stream: stream, label: device?.localizedName ?? "")
            t.tracks = self
            if let device {
                t.device = device
                read(t)
            } else {
                t.facing = constraints["facingMode"] as? String == "user" ? "user" : "environment"
                agentCamera(t)
            }
            if overlay != nil { t.platformMute = overlay }
            tracks[t.id] = t
            opened = true
            let entry = ExactTrackEntry(id: t.id, kind: "video", owner: self)
            entry.state = { [weak self, weak t] in t.flatMap { self?.shape($0) } ?? [:] }
            entry.video = { [weak t] in t?.capture }
            context.tracks.add(entry)
            say("\(stream): opened with \(t.id) (\(t.label))")
            announce()
            context.changed("media-capture.enumerateDevices")
            reply(shape(t))
        }
        if context.agent { return make(nil) }
        guard let device = Cameras.choose(constraints, apple: apple) else { return reply(["error": "NotFoundError"]) }
        CameraPermission.ask { granted in
            DispatchQueue.main.async {
                guard granted else { return reply(["error": "NotAllowedError"]) }
                make(device)
            }
        }
    }

    func applyConstraints(_ args: [String: Any], origin: String, reply: @escaping ([String: Any]) -> Void) {
        guard let id = args["track"] as? String, let t = tracks[id] else { return reply(["error": "NotFoundError"]) }
        guard t.readyState == "live" else { return reply(shape(t)) }
        let apple = args["apple"] as? [String: Any] ?? [:]
        let deviceId = args["deviceId"] as? String, facing = args["facingMode"] as? String
        let moves = (deviceId != nil && deviceId != t.settings["deviceId"] as? String)
            || (facing != nil && facing != t.settings["facingMode"] as? String)
            || (apple["deviceType"] as? String).map { word in t.device.map { Cameras.word($0.deviceType) != word } ?? true } ?? false
        if moves {
            // Another camera under the same id (D1): a capture start (D6).
            guard origin == "app", context.foreground else {
                say("\(t.id): not moved, a capture start needs the app's own origin in the foreground (\(origin))")
                return reply(["error": "NotAllowedError"])
            }
            if context.agent {
                t.facing = facing ?? (deviceId == Self.agentFront ? "user" : "environment")
                agentCamera(t)
            } else {
                var c: [String: Any] = [:]
                if let deviceId { c["deviceId"] = deviceId }
                if let facing { c["facingMode"] = facing }
                guard let next = Cameras.choose(c, apple: apple) else { return reply(["error": "NotFoundError"]) }
                // On iOS the capture is the process's: another live lease keeps its camera (D3).
                if let capture = t.capture, capture.holders.count > 1, VideoCapture.oneForProcess {
                    say("\(t.id): not moved, another session's lease holds the camera")
                    return reply(["error": "NotReadableError"])
                }
                t.device = next
                t.label = next.localizedName
                if let capture = t.capture {
                    if VideoCapture.oneForProcess { capture.switchTo(next) } else { release(t); lease(t, origin: origin) }
                }
                read(t)
            }
            say("\(t.id): moved to \(t.label)")
            if t.leased { each(t) { $0.live(video: t.capture) } }
            announce()
            return reply(shape(t))
        }
        var kept: [String: Any] = [:]
        for k in ["zoom", "torch"] { if let v = args[k], !(v is NSNull) { kept[k] = v } }
        if context.agent {
            if let torch = kept["torch"] as? Bool, t.capabilities["torch"] as? Bool == true { t.settings["torch"] = torch }
        } else if let capture = t.capture {
            capture.apply(kept)
        } else if let device = t.device {
            Cameras.apply(kept, to: device, queue: nil)
            read(t)
        }
        announce()
        reply(shape(t))
    }

    func stop(_ id: String) -> [String: Any] {
        guard let t = tracks[id] else { return ["error": "NotFoundError"] }
        end(t, "stopTrack")
        return shape(t)
    }

    func setEnabled(_ id: String, _ enabled: Bool, origin: String) -> [String: Any] {
        guard let t = tracks[id] else { return ["error": "NotFoundError"] }
        t.enabled = enabled
        if !enabled, t.leased { release(t); mute(t, "idle") }
        // Turning a track back on is a start: it reopens an idle one that has consumers (D6).
        if enabled, !t.leased, !t.consumers.isEmpty, t.readyState == "live" { lease(t, origin: origin) }
        announce()
        return shape(t)
    }

    func state(_ id: String) -> [String: Any] { tracks[id].map(shape) ?? ["error": "NotFoundError"] }

    /// The cameras, as `MediaDeviceInfo`s: none named before a grant, as a browser lists them.
    var devices: [[String: Any]] {
        if context.agent {
            guard opened else { return [["deviceId": "", "kind": "videoinput", "label": "", "groupId": ""]] }
            return [["deviceId": Self.agentBack, "kind": "videoinput", "label": "Agent back camera", "groupId": "agent", "facingMode": "environment"],
                    ["deviceId": Self.agentFront, "kind": "videoinput", "label": "Agent front camera", "groupId": "agent", "facingMode": "user"]]
        }
        guard CameraPermission.state == "granted" else { return Cameras.all.isEmpty ? [] : [["deviceId": "", "kind": "videoinput", "label": "", "groupId": ""]] }
        return Cameras.all.map { d in
            var info: [String: Any] = ["deviceId": d.uniqueID, "kind": "videoinput", "label": d.localizedName, "groupId": d.modelID]
            if let facing = Cameras.facing(d) { info["facingMode"] = facing }
            return info
        }
    }

    var status: [String: Any] {
        ["available": !Cameras.all.isEmpty, "state": CameraPermission.state, "apple": ["authorizationStatus": CameraPermission.word]]
    }

    /// Under the agent: a preferred `mediaStreamTrack` is the platform's change to every live track (LLP 1133.009 D10 item 11).
    func preferred(_ value: [String: Any]?) {
        overlay = value?["muted"] as? Bool == true ? value?["muteReason"] as? String ?? "interrupted" : nil
        for t in tracks.values where t.readyState == "live" {
            if value?["readyState"] as? String == "ended" { end(t, "the agent ended it"); continue }
            t.platformMute = overlay
            if t.leased { settle(t) }
        }
    }

    /// The app went to the background or came back (D6): on iOS the system
    /// interrupts the session and resumes it itself; on macOS nothing changes.
    func foreground(_ shown: Bool) {
        guard context.agent else {
            for t in tracks.values where t.readyState == "live" {
                guard let capture = t.capture else { continue }
                say("\(t.id): foreground=\(shown), running=\(capture.session.isRunning), interrupted=\(capture.session.interrupted), muted=\(t.muteReason ?? "none")")
            }
            return
        }
        for t in tracks.values where t.readyState == "live" && t.leased {
            t.platformMute = shown ? overlay : "background"
            settle(t)
        }
    }

    func dispose(_ id: String) { if let t = tracks[id] { end(t, "disposed") } }
    func endAll(_ why: String) { for t in tracks.values { end(t, why) } }

    // MARK: leases (D3)

    func subscribe(_ id: String, _ consumer: ExactTrackConsumer, start: Bool, origin: String) throws -> ExactSubscription {
        guard let t = tracks[id] else { throw ExactTrackError("NotFoundError") }
        if t.readyState == "ended" { consumer.ended(); return ExactSubscription {} }
        t.consumers[ObjectIdentifier(consumer)] = (consumer, origin)
        if t.leased {
            consumer.live(video: t.capture)
            if t.muted { consumer.muted(t.muteReason ?? "idle") }
        } else if origin == "app", t.enabled, context.foreground {
            say("\(t.id): a consumer takes the lease (\(origin) origin)")
            lease(t, origin: origin)
            if !t.leased { consumer.muted(t.muteReason ?? "idle") }
        } else {
            say("\(t.id): a consumer takes no lease (\(origin) origin\(context.foreground ? "" : ", in the background"))")
            consumer.muted(t.muteReason ?? "idle")
        }
        disclose()
        return ExactSubscription { [weak self, weak t, weak consumer] in
            guard let self, let t, let consumer, t.consumers.removeValue(forKey: ObjectIdentifier(consumer)) != nil else { return }
            if t.consumers.isEmpty {
                t.waiting = nil
                if t.leased { self.release(t); self.mute(t, "idle") }
            }
            self.disclose()
        }
    }

    /// `observer` watches `id` (LLP 1133.027 D17 item 1): it hears the capture while a consumer's lease runs it, and never
    /// starts, holds or reopens one.
    func observe(_ id: String, _ observer: ExactTrackConsumer) throws -> ExactSubscription {
        guard let t = tracks[id] else { throw ExactTrackError("NotFoundError") }
        if t.readyState == "ended" { observer.ended(); return ExactSubscription {} }
        t.observers[ObjectIdentifier(observer)] = observer
        say("\(t.id): an observer joins, taking no lease")
        if t.leased {
            observer.live(video: t.capture)
            if t.muted { observer.muted(t.muteReason ?? "idle") }
        } else {
            observer.muted(t.muteReason ?? "idle")
        }
        return ExactSubscription { [weak self, weak t, weak observer] in
            guard let t, let observer, t.observers.removeValue(forKey: ObjectIdentifier(observer)) != nil else { return }
            self?.say("\(t.id): an observer leaves")
        }
    }

    private func each(_ t: VideoTrack, _ f: (ExactTrackConsumer) -> Void) {
        for c in Array(t.consumers.values) { f(c.consumer) }
        for o in Array(t.observers.values) { f(o) }
    }

    private func mute(_ t: VideoTrack, _ reason: String) {
        guard !(t.muted && t.muteReason == reason) else { return }
        t.muted = true; t.muteReason = reason
        each(t) { $0.muted(reason) }
        announce()
    }

    private func unmute(_ t: VideoTrack) {
        guard t.muted else { return }
        t.muted = false; t.muteReason = nil
        each(t) { $0.unmuted() }
        announce()
    }

    /// The platform's mute over a held lease, or none.
    func settle(_ t: VideoTrack) {
        if let reason = t.platformMute { mute(t, reason) } else { unmute(t) }
    }

    /// Take the lease, admitted with `origin` (D3, D6): the capture runs, or the track waits `in-use`.
    func lease(_ t: VideoTrack, origin: String) {
        guard !t.leased, t.readyState == "live" else { return }
        if context.agent {
            t.agentLease = true
        } else {
            guard let device = t.device else { return }
            guard let capture = VideoCapture.lease(t, device: device, log: { [weak self] in self?.say($0) }) else {
                t.waiting = origin
                VideoCapture.wait(t)
                say("\(t.id): waits for the camera, in use by another lease")
                mute(t, "in-use")
                return
            }
            t.capture = capture
            t.waiting = nil
            read(t)
        }
        each(t) { $0.live(video: t.capture) }
        settle(t)
    }

    /// Give the lease back; a track waiting for the camera may take it.
    func release(_ t: VideoTrack) {
        t.agentLease = false
        guard let capture = t.capture else { return }
        t.capture = nil
        capture.unlease(t)
    }

    /// A lease another track gave back: a waiting track of this session takes it, with its kept origin.
    func retry(_ t: VideoTrack) {
        guard let origin = t.waiting, !t.consumers.isEmpty, t.readyState == "live", t.enabled else { t.waiting = nil; return }
        t.waiting = nil
        lease(t, origin: origin)
    }

    /// The capture `t` holds was interrupted (`reason`) or resumed (nil).
    func interrupted(_ t: VideoTrack, _ reason: String?) {
        say("\(t.id): interruption=\(reason ?? "ended")")
        t.platformMute = reason ?? overlay
        settle(t)
    }

    /// A device change the capture made (a pinch's zoom, the torch).
    func changed(_ t: VideoTrack) { read(t); announce() }

    private func end(_ t: VideoTrack, _ why: String) {
        guard t.readyState == "live" else { return }
        t.readyState = "ended"
        t.waiting = nil
        release(t)
        say("\(t.id): ended (\(why))")
        each(t) { $0.ended() }
        t.consumers.removeAll()
        t.observers.removeAll()
        announce()
    }

    // MARK: answers

    static let agentBack = "agent-camera-back", agentFront = "agent-camera-front"

    private func agentCamera(_ t: VideoTrack) {
        let back = t.facing != "user"
        t.label = back ? "Agent back camera" : "Agent front camera"
        t.settings = ["deviceId": back ? Self.agentBack : Self.agentFront, "groupId": "agent", "facingMode": t.facing,
                      "width": 1280, "height": 960, "aspectRatio": 4.0 / 3.0, "frameRate": 30, "resizeMode": "none"]
        if back { t.settings["torch"] = false }
        t.capabilities = ["facingMode": [t.facing], "width": ["min": 640, "max": 1280, "step": 1], "height": ["min": 480, "max": 960, "step": 1],
                          "frameRate": ["min": 1, "max": 30, "step": 1], "torch": back]
    }

    /// The device's settings and capabilities, as the track reports them.
    private func read(_ t: VideoTrack) {
        guard let d = t.device else { return }
        t.settings = Cameras.settings(d)
        t.capabilities = Cameras.capabilities(d)
    }

    private func shape(_ t: VideoTrack) -> [String: Any] {
        var out: [String: Any] = ["id": t.id, "kind": "video", "label": t.label, "enabled": t.enabled, "muted": t.muted,
                                  "readyState": t.readyState, "settings": t.settings, "capabilities": t.capabilities]
        if t.muted, let reason = t.muteReason { out["muteReason"] = reason }
        if let d = t.device {
            var apple: [String: Any] = ["deviceType": Cameras.word(d.deviceType)]
            if let position = Cameras.position(d) { apple["position"] = position }
            out["apple"] = apple
        }
        return out
    }
}

/// The process's captures (D3).
final class VideoCapture: NSObject, ExactVideoSource {
    /// iOS runs one camera at a time in an app; macOS one capture per camera.
    #if os(iOS)
    static let oneForProcess = true
    #else
    static let oneForProcess = false
    #endif
    private static var captures: [String: VideoCapture] = [:]

    let session = AVCaptureSession()
    private(set) var device: AVCaptureDevice
    private let queue = DispatchQueue(label: "dev.exact.media-capture.camera")
    private var input: AVCaptureDeviceInput?
    private(set) var holders: [ObjectIdentifier: () -> VideoTrack?] = [:]
    private var objects: [String: AnyObject] = [:]
    private var observers: [NSObjectProtocol] = []
    private var log: (String) -> Void = { _ in }

    private init(device: AVCaptureDevice) {
        self.device = device
        super.init()
        #if os(iOS)
        // The audio session is the host's (LLP 1133.009 D7): a capture session never configures it.
        session.automaticallyConfiguresApplicationAudioSession = false
        #endif
    }

    private static func key(_ device: AVCaptureDevice) -> String { oneForProcess ? "camera" : device.uniqueID }

    /// A lease on `device`'s capture for `t`, or nil while another camera's lease holds the process's (iOS).
    static func lease(_ t: VideoTrack, device: AVCaptureDevice, log: @escaping (String) -> Void) -> VideoCapture? {
        let key = key(device)
        let capture = captures[key] ?? VideoCapture(device: device)
        if !capture.holders.isEmpty, capture.device.uniqueID != device.uniqueID { return nil }
        captures[key] = capture
        capture.log = log
        let first = capture.holders.isEmpty
        capture.holders[ObjectIdentifier(t)] = { [weak t] in t }
        if first { capture.start(device) }
        return capture
    }

    /// `t` gives its lease back: the last one stops the capture, and a track waiting for the camera takes it.
    func unlease(_ t: VideoTrack) {
        guard holders.removeValue(forKey: ObjectIdentifier(t)) != nil, holders.isEmpty else { return }
        stop()
        VideoCapture.captures[VideoCapture.key(device)] = nil
        for waiting in VideoCapture.waiters() { waiting.tracks?.retry(waiting) }
    }

    /// Tracks of every session waiting for the camera, oldest first.
    private static var waiting: [() -> VideoTrack?] = []
    static func wait(_ t: VideoTrack) { waiting.append { [weak t] in t } }
    private static func waiters() -> [VideoTrack] {
        let tracks = waiting.compactMap { $0() }.filter { $0.waiting != nil }
        waiting = tracks.map { t in { [weak t] in t } }
        return tracks
    }

    private func start(_ device: AVCaptureDevice) {
        self.device = device
        let session = session, hatch = MediaCaptureModule.sessionHatch
        listen()
        queue.async { [weak self] in
            session.beginConfiguration()
            // The preset a photo app sets (D2), not the API's `.high`.
            if session.canSetSessionPreset(.photo) { session.sessionPreset = .photo }
            if let input = try? AVCaptureDeviceInput(device: device), session.canAddInput(input) {
                session.addInput(input)
                self?.input = input
            }
            hatch?(session)
            session.commitConfiguration()
            session.startRunning()
        }
        log("camera \(device.localizedName): capture started")
    }

    /// Another camera for the process's capture (iOS): the session's input replaced.
    func switchTo(_ next: AVCaptureDevice) {
        let session = session, old = input, hatch = MediaCaptureModule.sessionHatch
        device = next
        queue.async { [weak self] in
            session.beginConfiguration()
            if let old { session.removeInput(old) }
            if let input = try? AVCaptureDeviceInput(device: next), session.canAddInput(input) {
                session.addInput(input)
                self?.input = input
            }
            hatch?(session)
            session.commitConfiguration()
        }
        log("camera: the capture moved to \(next.localizedName)")
    }

    private func stop() {
        for o in observers { NotificationCenter.default.removeObserver(o) }
        observers = []
        let session = session, input = input
        self.input = nil
        objects = [:]
        queue.async {
            session.stopRunning()
            session.beginConfiguration()
            if let input { session.removeInput(input) }
            for output in session.outputs { session.removeOutput(output) }
            session.commitConfiguration()
        }
        log("camera \(device.localizedName): capture stopped")
    }

    /// Interruptions as `muteReason` (D3); after any but the background, the session runs again while leased.
    private func listen() {
        let center = NotificationCenter.default
        for name in [AVCaptureSession.didStartRunningNotification, AVCaptureSession.didStopRunningNotification] {
            observers.append(center.addObserver(forName: name, object: session, queue: .main) { [weak self] _ in
                guard let self else { return }
                self.log("camera: running=\(self.session.isRunning), interrupted=\(self.session.interrupted)")
            })
        }
        let tell = { [weak self] (reason: String?) in
            guard let self else { return }
            for get in self.holders.values { if let t = get() { t.tracks?.interrupted(t, reason) } }
        }
        observers.append(center.addObserver(forName: AVCaptureSession.wasInterruptedNotification, object: session, queue: .main) { note in
            var reason = "interrupted"
            #if os(iOS)
            if let raw = note.userInfo?[AVCaptureSessionInterruptionReasonKey] as? Int, let r = AVCaptureSession.InterruptionReason(rawValue: raw) {
                switch r {
                case .videoDeviceNotAvailableInBackground: reason = "background"
                case .videoDeviceNotAvailableWithMultipleForegroundApps: reason = "multitasking"
                case .videoDeviceInUseByAnotherClient: reason = "in-use"
                case .videoDeviceNotAvailableDueToSystemPressure: reason = "system-pressure"
                default: reason = "interrupted"
                }
            }
            #endif
            tell(reason)
        })
        observers.append(center.addObserver(forName: AVCaptureSession.interruptionEndedNotification, object: session, queue: .main) { [weak self] _ in
            tell(nil)
            guard let self, !self.holders.isEmpty else { return }
            let session = self.session
            self.queue.async { if !session.isRunning { session.startRunning() } }
        })
        observers.append(center.addObserver(forName: AVCaptureSession.runtimeErrorNotification, object: session, queue: .main) { [weak self] note in
            self?.log("camera: runtime error \((note.userInfo?[AVCaptureSessionErrorKey] as? Error).map { "\($0)" } ?? "")")
            tell("interrupted")
            guard let self, !self.holders.isEmpty else { return }
            let session = self.session
            self.queue.async { session.startRunning() }
        })
    }

    // MARK: ExactVideoSource

    func configure(_ change: @escaping (AVCaptureSession) -> Void) {
        let session = session
        queue.async {
            session.beginConfiguration()
            change(session)
            session.commitConfiguration()
        }
    }

    func run(_ work: @escaping (AVCaptureSession) -> Void) {
        let session = session
        queue.async { work(session) }
    }

    func shared<T: AnyObject>(_ key: String, _ make: () -> T) -> T {
        if let made = objects[key] as? T { return made }
        let made = make()
        objects[key] = made
        return made
    }

    func apply(_ constraints: [String: Any]) {
        Cameras.apply(constraints, to: device, queue: queue) { [weak self] in
            guard let self else { return }
            for get in self.holders.values { if let t = get() { t.tracks?.changed(t) } }
        }
    }

    func focus(at point: CGPoint) {
        let device = device
        queue.async {
            guard (try? device.lockForConfiguration()) != nil else { return }
            if device.isFocusPointOfInterestSupported, device.isFocusModeSupported(.autoFocus) {
                device.focusPointOfInterest = point
                device.focusMode = .autoFocus
            }
            if device.isExposurePointOfInterestSupported, device.isExposureModeSupported(.autoExpose) {
                device.exposurePointOfInterest = point
                device.exposureMode = .autoExpose
            }
            MediaCaptureModule.deviceHatch?(device)
            device.unlockForConfiguration()
        }
    }
}

/// The cameras, as the platform names them (D2, D4).
enum Cameras {
    static var all: [AVCaptureDevice] {
        AVCaptureDevice.DiscoverySession(deviceTypes: types, mediaType: .video, position: .unspecified).devices
    }

    private static var types: [AVCaptureDevice.DeviceType] {
        #if os(iOS)
        var out: [AVCaptureDevice.DeviceType] = [.builtInWideAngleCamera, .builtInUltraWideCamera, .builtInTelephotoCamera,
                                                 .builtInDualCamera, .builtInDualWideCamera, .builtInTripleCamera, .builtInTrueDepthCamera]
        if #available(iOS 17.0, *) { out += [.continuityCamera, .external] }
        #else
        var out: [AVCaptureDevice.DeviceType] = [.builtInWideAngleCamera]
        if #available(macOS 13.0, *) { out.append(.deskViewCamera) }
        if #available(macOS 14.0, *) { out += [.continuityCamera, .external] }
        #endif
        return out
    }

    /// AVFoundation's name for a device type, as the `deviceType` word writes it (`builtInWideAngleCamera`).
    static func word(_ type: AVCaptureDevice.DeviceType) -> String {
        let name = type.rawValue.hasPrefix("AVCaptureDeviceType") ? String(type.rawValue.dropFirst("AVCaptureDeviceType".count)) : type.rawValue
        return name.prefix(1).lowercased() + name.dropFirst()
    }

    /// The camera a call asks for: by `deviceId`, by `apple.deviceType`, by `facingMode`, else the platform's own choice (D2).
    static func choose(_ constraints: [String: Any], apple: [String: Any]) -> AVCaptureDevice? {
        if let id = constraints["deviceId"] as? String, let d = AVCaptureDevice(uniqueID: id) { return d }
        let facing = constraints["facingMode"] as? String
        let position: AVCaptureDevice.Position = facing == "user" ? .front : facing == "environment" ? .back : .unspecified
        if let word = apple["deviceType"] as? String {
            let type = AVCaptureDevice.DeviceType(rawValue: "AVCaptureDeviceType" + word.prefix(1).uppercased() + word.dropFirst())
            if let d = AVCaptureDevice.DiscoverySession(deviceTypes: [type], mediaType: .video, position: position).devices.first { return d }
        }
        #if os(iOS)
        if position == .front { return AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .front) }
        // The back virtual device with the most constituents, so a zoom below 1 reaches the ultra-wide (D2).
        let back: [AVCaptureDevice.DeviceType] = [.builtInTripleCamera, .builtInDualWideCamera, .builtInDualCamera, .builtInWideAngleCamera]
        let found = AVCaptureDevice.DiscoverySession(deviceTypes: back, mediaType: .video, position: .back).devices
        return back.lazy.compactMap { type in found.first { $0.deviceType == type } }.first
        #else
        if position != .unspecified, let d = all.first(where: { $0.position == position }) { return d }
        if #available(macOS 14.0, *), let preferred = AVCaptureDevice.systemPreferredCamera { return preferred }
        return AVCaptureDevice.default(for: .video)
        #endif
    }

    static func position(_ d: AVCaptureDevice) -> String? {
        switch d.position {
        case .back: "back"
        case .front: "front"
        default: nil
        }
    }

    /// Media Capture's `facingMode`; none where the platform reports no position (most Mac cameras, D2).
    static func facing(_ d: AVCaptureDevice) -> String? {
        switch d.position {
        case .back: "environment"
        case .front: "user"
        default: nil
        }
    }

    static func settings(_ d: AVCaptureDevice) -> [String: Any] {
        let size = CMVideoFormatDescriptionGetDimensions(d.activeFormat.formatDescription)
        var out: [String: Any] = ["deviceId": d.uniqueID, "groupId": d.modelID, "width": Int(size.width), "height": Int(size.height),
                                  "aspectRatio": size.height > 0 ? Double(size.width) / Double(size.height) : 0, "resizeMode": "none"]
        let duration = d.activeVideoMinFrameDuration
        if duration.seconds > 0 { out["frameRate"] = (1 / duration.seconds).rounded() }
        if let facing = facing(d) { out["facingMode"] = facing }
        #if os(iOS)
        out["zoom"] = Double(d.videoZoomFactor)
        #endif
        if d.hasTorch { out["torch"] = d.torchMode == .on }
        return out
    }

    static func capabilities(_ d: AVCaptureDevice) -> [String: Any] {
        let sizes = d.formats.map { CMVideoFormatDescriptionGetDimensions($0.formatDescription) }
        let range = { (values: [Int32]) -> [String: Any] in ["min": Int(values.min() ?? 0), "max": Int(values.max() ?? 0), "step": 1] }
        var out: [String: Any] = ["width": range(sizes.map(\.width)), "height": range(sizes.map(\.height)), "torch": d.hasTorch]
        if let facing = facing(d) { out["facingMode"] = [facing] }
        let rates = d.activeFormat.videoSupportedFrameRateRanges
        if !rates.isEmpty { out["frameRate"] = ["min": rates.map(\.minFrameRate).min() ?? 0, "max": rates.map(\.maxFrameRate).max() ?? 0] }
        #if os(iOS)
        out["zoom"] = ["min": Double(d.minAvailableVideoZoomFactor), "max": Double(d.maxAvailableVideoZoomFactor), "step": 0.1]
        #endif
        return out
    }

    /// `zoom` and `torch` on the device, under its lock, on `queue` (or now); `done` on main after.
    static func apply(_ c: [String: Any], to d: AVCaptureDevice, queue: DispatchQueue?, done: @escaping () -> Void = {}) {
        let work = {
            guard (try? d.lockForConfiguration()) != nil else { return }
            #if os(iOS)
            if let zoom = (c["zoom"] as? NSNumber)?.doubleValue {
                d.videoZoomFactor = CGFloat(min(max(zoom, Double(d.minAvailableVideoZoomFactor)), Double(d.maxAvailableVideoZoomFactor)))
            }
            #endif
            if let torch = c["torch"] as? Bool, d.hasTorch, d.isTorchModeSupported(torch ? .on : .off) { d.torchMode = torch ? .on : .off }
            MediaCaptureModule.deviceHatch?(d)
            d.unlockForConfiguration()
            DispatchQueue.main.async(execute: done)
        }
        if let queue { queue.async(execute: work) } else { work() }
    }
}

/// The camera's permission, as the platform says it (D6).
enum CameraPermission {
    static var state: String {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized: "granted"
        case .denied, .restricted: "denied"
        default: "prompt"
        }
    }
    static var word: String {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized: "authorized"
        case .denied: "denied"
        case .restricted: "restricted"
        default: "notDetermined"
        }
    }
    static func ask(_ done: @escaping (Bool) -> Void) {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized: done(true)
        case .notDetermined: AVCaptureDevice.requestAccess(for: .video, completionHandler: done)
        default: done(false)
        }
    }
}

extension AVCaptureSession {
    /// `isInterrupted` for the journal; macOS has no such property and no interruption to report.
    var interrupted: Bool {
        #if os(macOS)
        false
        #else
        isInterrupted
        #endif
    }
}
