// @exact/media-capture on Apple (LLP 1133.009 D1-D3, D5-D9): the
// microphone as Media Capture's tracks, one capture for the process, each
// session's tracks in its track table (`context.tracks`) so a package that
// consumes a track (`@exact/recording`) reaches it by id.
//
// - One capture per process (D3): one `AVAudioEngine`, whose input node's
//   tap hands each buffer to every consumer of every session. It runs while
//   any track in the process holds a lease, configured by the lease that
//   opened it; it is made at the first and stopped at the last.
// - A track holds a lease while it has a consumer (D6): its first consumer
//   takes it, and after use only a start does (a recorder's start,
//   `setTrackEnabled(enabled=true)`); a reader joining an idle track hears
//   silence and opens nothing. A tool's or a wake's consumer never leases.
// - The audio session is the host's (D7): the capture claims it
//   (`.playAndRecord`), and hears its interruptions, their ends, resets and
//   route changes. A change of route or hardware format stops the engine,
//   which posts `AVAudioEngineConfigurationChange` on its own queue; the
//   handler only schedules the reopening on main (D7).
// - The permission is a fact (D9): `AVAudioApplication.recordPermission` on
//   iOS, `AVCaptureDevice.authorizationStatus(for: .audio)` on macOS, read
//   before anything starts, since iOS records silence when it is not granted.
// - Unsaid constraints are the platform's (D2): the input node unprocessed.
//   `echoCancellation`, `noiseSuppression` or `autoGainControl` written true
//   turn on voice processing, one switch on Apple.
// - Under the agent no engine is made and nothing is asked: a stream with
//   one track, "Agent microphone", whose mute follows the agent's preferred
//   `mediaStreamTrack` (D10 item 11).
// - Tier 3 (D5): `MediaCaptureModule.hatch` is given the engine once per
//   capture, before it starts. The input node's bus 0 is the fan-out's.
import AVFoundation
import ExactPackage
import Foundation
#if os(iOS)
import UIKit
#endif

public final class MediaCaptureModule: ExactPackage, ExactTrackOwner {
    /// The app's access hatch (D5): the capture's engine, once per capture,
    /// before `start()`.
    public static var hatch: ((AVAudioEngine) -> Void)?

    private let context: ExactPackageContext
    private var next = 0
    private var tracks: [String: Track] = [:]
    /// The agent's preferred `mediaStreamTrack`: an interruption begun while it says muted.
    private var overlay: String?
    private var opened = false

    final class Track {
        let id: String, stream: String
        var label: String, enabled = true, muted = true, muteReason: String? = "idle", readyState = "live"
        var settings: [String: Any] = [:], options: CaptureOptions
        var consumers: [ObjectIdentifier: ExactTrackConsumer] = [:]
        var active = false, used = false, leased = false
        /// Why the platform mutes it now, if it does (an interruption, the background).
        var platformMute: String?
        init(id: String, stream: String, label: String, options: CaptureOptions) {
            self.id = id; self.stream = stream; self.label = label; self.options = options
        }
    }

    public init(context: ExactPackageContext) {
        self.context = context
        if !context.agent { ProcessCapture.shared.listen(self) }
    }

    // MARK: the names

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "getUserMedia": getUserMedia(args, reply)
        case "stopTrack":
            guard let t = track(args) else { return reply(["error": "NotFoundError"]) }
            end(t, "stopTrack")
            reply(shape(t))
        case "setTrackEnabled":
            guard let t = track(args) else { return reply(["error": "NotFoundError"]) }
            t.enabled = args["enabled"] as? Bool ?? true
            // Turning a track back on is a start: it reopens an idle one that has consumers (D6).
            if t.enabled, !t.active, !t.consumers.isEmpty, t.readyState == "live" { activate(t) }
            announce()
            reply(shape(t))
        case "mediaStreamTrack": reply(track(args).map(shape) ?? ["error": "NotFoundError"])
        case "enumerateDevices": reply(devices())
        case "microphoneStatus": reply(status())
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "microphoneStatusSync" ? status() : ["error": "NotSupportedError"]
    }

    private func track(_ args: [String: Any]) -> Track? { (args["track"] as? String).flatMap { tracks[$0] } }
    private func announce() { context.changed("media-capture.mediaStreamTrack") }

    private func getUserMedia(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let constraints = args["audioConstraints"] as? [String: Any] ?? [:]
        // No kind asked for is Media Capture's TypeError, before anything opens.
        guard args["audio"] as? Bool == true || args["audioConstraints"] != nil else { return reply(["error": "TypeError"]) }
        let options = CaptureOptions(constraints: constraints, apple: args["apple"] as? [String: Any] ?? [:])
        let make = { [self] in
            next += 1
            let t = Track(id: "track-\(context.session)-\(next)", stream: "stream-\(context.session)-\(next)",
                          label: context.agent ? "Agent microphone" : Permission.inputName, options: options)
            t.settings = context.agent ? Self.agentSettings : options.settings(ProcessCapture.shared.format)
            if overlay != nil { t.platformMute = overlay }
            tracks[t.id] = t
            opened = true
            context.tracks.add(ExactTrackEntry(id: t.id, kind: "audio", owner: self))
            context.tracks.add(ExactTrackEntry(id: t.stream, kind: "stream", tracks: [t.id]))
            context.log("media-capture: \(t.stream) opened with \(t.id)")
            announce()
            context.changed("media-capture.enumerateDevices")
            reply(["id": t.stream, "active": true, "tracks": [shape(t)]])
        }
        if context.agent { return make() }
        guard Permission.inputAvailable else { return reply(["error": "NotFoundError"]) }
        Permission.ask { granted in
            DispatchQueue.main.async {
                guard granted else { return reply(["error": "NotAllowedError"]) }
                make()
            }
        }
    }

    // MARK: tracks and leases (D6)

    public func subscribe(_ id: String, _ consumer: ExactTrackConsumer, start: Bool, origin: String) throws -> ExactSubscription {
        guard let t = tracks[id] else { throw ExactTrackError("NotFoundError") }
        if t.readyState == "ended" { consumer.ended(); return ExactSubscription {} }
        t.consumers[ObjectIdentifier(consumer)] = consumer
        if t.active {
            consumer.live(context.agent ? nil : ProcessCapture.shared)
            if t.muted { consumer.muted(t.muteReason ?? "idle") }
        } else if origin == "app", t.enabled, !t.used || start {
            activate(t)
        } else {
            consumer.muted(t.muteReason ?? "idle")
        }
        return ExactSubscription { [weak self, weak t, weak consumer] in
            guard let self, let t, let consumer, t.consumers.removeValue(forKey: ObjectIdentifier(consumer)) != nil else { return }
            if t.consumers.isEmpty, t.active { self.deactivate(t) }
        }
    }

    private func each(_ t: Track, _ f: (ExactTrackConsumer) -> Void) { for c in Array(t.consumers.values) { f(c) } }

    private func mute(_ t: Track, _ reason: String, consumersHear heard: String? = nil) {
        guard !(t.muted && t.muteReason == reason) else { return }
        t.muted = true; t.muteReason = reason
        each(t) { $0.muted(heard ?? reason) }
        announce()
    }

    private func unmute(_ t: Track) {
        guard t.muted else { return }
        t.muted = false; t.muteReason = nil
        each(t) { $0.unmuted() }
        announce()
    }

    /// The platform's mute over a running capture, or none.
    private func settle(_ t: Track) {
        if let reason = t.platformMute { mute(t, reason) } else { unmute(t) }
    }

    /// Take the lease: the capture runs (D6). A capture that will not start
    /// leaves the track muted, `interrupted`, and says why.
    private func activate(_ t: Track) {
        t.active = true; t.used = true
        if !context.agent, !t.leased {
            do {
                try ProcessCapture.shared.lease(t.options, session: context.audioSession, log: context.log)
                t.leased = true
                t.settings = t.options.settings(ProcessCapture.shared.format)
            } catch {
                context.log("media-capture: \(t.id) did not open: \(error)")
                t.platformMute = "interrupted"
            }
        }
        each(t) { $0.live(context.agent ? nil : ProcessCapture.shared) }
        settle(t)
    }

    /// Give the lease back: the track is idle.
    private func deactivate(_ t: Track) {
        t.active = false
        release(t)
        mute(t, "idle")
    }

    private func release(_ t: Track) {
        guard t.leased else { return }
        t.leased = false
        ProcessCapture.shared.unlease()
    }

    private func end(_ t: Track, _ why: String) {
        guard t.readyState == "live" else { return }
        t.readyState = "ended"; t.active = false
        release(t)
        context.log("media-capture: \(t.id) ended (\(why))")
        each(t) { $0.ended() }
        t.consumers.removeAll()
        announce()
    }

    // MARK: what the process's capture and the session tell (D7, D8)

    /// An event of the audio session or the engine, for every track this session runs.
    func heard(_ event: String) {
        for t in tracks.values where t.readyState == "live" {
            switch event {
            case "interrupted": t.platformMute = "interrupted"
            case "resumed", "reopened": if t.platformMute == "interrupted" { t.platformMute = nil }
            case "reset":
                // A media-services reset: the capture is let go, and nothing records until a start (D7).
                t.platformMute = "interrupted"
                if t.active { mute(t, "interrupted", consumersHear: "media-reset") }
                continue
            default: continue
            }
            if t.active { settle(t) }
        }
    }

    /// The app left the foreground or came back (D8): without the background
    /// mode on iOS a capture's tracks mute, `background`, and its lease goes.
    public func foreground(_ shown: Bool) {
        if shown { context.changed("media-capture.microphoneStatus") }
        #if os(iOS)
        guard !context.agent, !Permission.recordsInBackground else { return }
        for t in tracks.values where t.readyState == "live" && t.active {
            if shown {
                t.platformMute = nil
                activate(t)
            } else {
                t.platformMute = "background"
                release(t)
                settle(t)
            }
        }
        #endif
    }

    /// Under the agent: a preferred `mediaStreamTrack` is the platform's change to every live track (D10 item 11).
    public func preferred(_ values: [String: Any]) {
        let value = values["mediaStreamTrack"] as? [String: Any]
        overlay = value?["muted"] as? Bool == true ? value?["muteReason"] as? String ?? "interrupted" : nil
        for t in tracks.values where t.readyState == "live" {
            t.platformMute = overlay
            if t.active { settle(t) }
        }
    }

    /// Handles a dropped reply carried: the stream's tracks end (D6).
    public func dispose(_ ids: [String]) {
        for id in ids {
            if let t = tracks[id] { end(t, "disposed") }
            for t in tracks.values where t.stream == id { end(t, "disposed") }
        }
    }

    public func destroy(_ done: @escaping () -> Void) {
        for t in tracks.values { end(t, "the session ended") }
        ProcessCapture.shared.forget(self)
        done()
    }

    // MARK: answers

    private static let agentSettings: [String: Any] = ["deviceId": "agent-microphone", "groupId": "agent", "sampleRate": 48000, "sampleSize": 16,
                                                       "channelCount": 1, "echoCancellation": false, "autoGainControl": false, "noiseSuppression": false, "latency": 0.01]

    private func shape(_ t: Track) -> [String: Any] {
        var out: [String: Any] = ["id": t.id, "kind": "audio", "label": t.label, "enabled": t.enabled, "muted": t.muted,
                                  "readyState": t.readyState, "settings": t.settings]
        if t.muted, let reason = t.muteReason { out["muteReason"] = reason }
        if !context.agent, let port = Permission.inputPort { out["apple"] = port }
        return out
    }

    private func devices() -> [String: Any] {
        let granted = context.agent ? opened : Permission.state == "granted"
        guard granted else { return ["devices": [["deviceId": "", "kind": "audioinput", "label": "", "groupId": ""]]] }
        if context.agent { return ["devices": [["deviceId": "agent-microphone", "kind": "audioinput", "label": "Agent microphone", "groupId": "agent"]]] }
        return ["devices": Permission.inputs]
    }

    private func status() -> [String: Any] {
        ["available": Permission.inputAvailable, "state": Permission.state, "apple": ["recordPermission": Permission.word]]
    }
}

/// What a lease configures (D2, D4): voice processing, the session's mode and options.
struct CaptureOptions {
    let voiceProcessing: Bool, mode: String?, categoryOptions: [String], background: Bool
    let preferredIOBufferDuration: Double?, deviceId: String?
    init(constraints: [String: Any], apple: [String: Any]) {
        voiceProcessing = ["echoCancellation", "noiseSuppression", "autoGainControl"].contains { constraints[$0] as? Bool == true }
        mode = apple["mode"] as? String
        categoryOptions = apple["categoryOptions"] as? [String] ?? []
        background = (Bundle.main.object(forInfoDictionaryKey: "UIBackgroundModes") as? [String] ?? []).contains("audio")
        preferredIOBufferDuration = (apple["preferredIOBufferDuration"] as? Double) ?? (constraints["latency"] as? Double)
        deviceId = constraints["deviceId"] as? String
    }
    /// What the track applied.
    func settings(_ format: AVAudioFormat) -> [String: Any] {
        ["deviceId": deviceId ?? "default", "groupId": "default", "sampleRate": format.sampleRate, "sampleSize": 32,
         "channelCount": Int(format.channelCount), "echoCancellation": voiceProcessing, "autoGainControl": voiceProcessing,
         "noiseSuppression": voiceProcessing]
    }
}

/// The microphone's permission and inputs, as the platform says them (D9).
enum Permission {
    static var state: String {
        #if os(iOS)
        switch AVAudioApplication.shared.recordPermission {
        case .granted: "granted"
        case .denied: "denied"
        default: "prompt"
        }
        #else
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: "granted"
        case .denied, .restricted: "denied"
        default: "prompt"
        }
        #endif
    }
    /// The platform's own word.
    static var word: String {
        #if os(iOS)
        switch AVAudioApplication.shared.recordPermission {
        case .granted: "granted"
        case .denied: "denied"
        default: "undetermined"
        }
        #else
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: "authorized"
        case .denied: "denied"
        case .restricted: "restricted"
        default: "notDetermined"
        }
        #endif
    }
    static func ask(_ done: @escaping (Bool) -> Void) {
        #if os(iOS)
        AVAudioApplication.requestRecordPermission(completionHandler: done)
        #else
        AVCaptureDevice.requestAccess(for: .audio, completionHandler: done)
        #endif
    }
    static var inputAvailable: Bool {
        #if os(iOS)
        AVAudioSession.sharedInstance().isInputAvailable
        #else
        AVCaptureDevice.default(for: .audio) != nil
        #endif
    }
    static var inputName: String {
        #if os(iOS)
        AVAudioSession.sharedInstance().currentRoute.inputs.first?.portName ?? "iPhone Microphone"
        #else
        AVCaptureDevice.default(for: .audio)?.localizedName ?? "Microphone"
        #endif
    }
    static var inputPort: [String: Any]? {
        #if os(iOS)
        AVAudioSession.sharedInstance().currentRoute.inputs.first.map { ["portType": $0.portType.rawValue, "portName": $0.portName] }
        #else
        AVCaptureDevice.default(for: .audio).map { ["portName": $0.localizedName] }
        #endif
    }
    static var inputs: [[String: Any]] {
        #if os(iOS)
        (AVAudioSession.sharedInstance().availableInputs ?? []).map { ["deviceId": $0.uid, "kind": "audioinput", "label": $0.portName, "groupId": $0.uid] }
        #else
        AVCaptureDevice.DiscoverySession(deviceTypes: [.microphone], mediaType: .audio, position: .unspecified).devices
            .map { ["deviceId": $0.uniqueID, "kind": "audioinput", "label": $0.localizedName, "groupId": $0.modelID] }
        #endif
    }
    /// `audio` in `UIBackgroundModes`: the capture records on in the background (D8).
    static var recordsInBackground: Bool {
        (Bundle.main.object(forInfoDictionaryKey: "UIBackgroundModes") as? [String] ?? []).contains("audio")
    }
}

/// The process's one capture (D3): an engine whose input tap fans every
/// buffer out to each tap, running while any lease is held.
final class ProcessCapture: ExactAudioSource {
    static let shared = ProcessCapture()
    private var engine: AVAudioEngine?
    private var leases = 0
    private var claim: ExactSubscription?
    private var options: CaptureOptions?
    private var log: (String) -> Void = { _ in }
    private var observer: NSObjectProtocol?
    private var listeners: [ObjectIdentifier: () -> MediaCaptureModule?] = [:]
    /// The taps, read on the audio thread under the lock.
    private let lock = NSLock()
    private var taps: [Int: (AVAudioPCMBuffer, AVAudioTime) -> Void] = [:]
    private var nextTap = 0
    private(set) var format = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 1)!

    func listen(_ m: MediaCaptureModule) { listeners[ObjectIdentifier(m)] = { [weak m] in m } }
    func forget(_ m: MediaCaptureModule) { listeners[ObjectIdentifier(m)] = nil }
    private func tell(_ event: String) { for get in Array(listeners.values) { get()?.heard(event) } }

    func tap(_ block: @escaping (AVAudioPCMBuffer, AVAudioTime) -> Void) -> ExactSubscription {
        lock.lock(); nextTap += 1; let id = nextTap; taps[id] = block; lock.unlock()
        return ExactSubscription { [weak self] in
            guard let self else { return }
            self.lock.lock(); self.taps[id] = nil; self.lock.unlock()
        }
    }

    /// A lease: the first claims the session and starts the engine with its options.
    func lease(_ options: CaptureOptions, session: ExactAudioSession, log: @escaping (String) -> Void) throws {
        if leases == 0 {
            self.log = log
            guard let claim = session.claim(mode: options.mode, options: options.categoryOptions, background: options.background,
                                            events: { [weak self] in self?.tell($0) }) else { throw CaptureError("NotReadableError: the audio session would not activate") }
            self.claim = claim
            self.options = options
            do { try start(options) } catch { claim.cancel(); self.claim = nil; throw error }
        }
        leases += 1
    }

    func unlease() {
        leases = max(0, leases - 1)
        guard leases == 0 else { return }
        stop()
        claim?.cancel()
        claim = nil
    }

    private func start(_ options: CaptureOptions) throws {
        let engine = AVAudioEngine()
        let input = engine.inputNode
        if options.voiceProcessing { try input.setVoiceProcessingEnabled(true) }
        #if os(iOS)
        let s = AVAudioSession.sharedInstance()
        if let d = options.preferredIOBufferDuration { try? s.setPreferredIOBufferDuration(d) }
        if let id = options.deviceId, let port = s.availableInputs?.first(where: { $0.uid == id }) { try? s.setPreferredInput(port) }
        #endif
        let format = input.outputFormat(forBus: 0)
        // No input: a tap on a 0 Hz format raises an exception that ends the app, so refuse first.
        guard format.sampleRate > 0, format.channelCount > 0 else { throw CaptureError("NotFoundError: no microphone is available") }
        self.format = format
        input.installTap(onBus: 0, bufferSize: 4096, format: format) { [weak self] buffer, time in
            guard let self else { return }
            self.lock.lock(); let taps = Array(self.taps.values); self.lock.unlock()
            for tap in taps { tap(buffer, time) }
        }
        MediaCaptureModule.hatch?(engine)
        engine.prepare()
        try engine.start()
        self.engine = engine
        // A route or hardware change stops the engine; the handler only schedules the reopening (D7).
        observer = NotificationCenter.default.addObserver(forName: .AVAudioEngineConfigurationChange, object: engine, queue: nil) { [weak self] _ in
            DispatchQueue.main.async { self?.reopen() }
        }
    }

    private func stop() {
        if let observer { NotificationCenter.default.removeObserver(observer) }
        observer = nil
        engine?.inputNode.removeTap(onBus: 0)
        engine?.stop()
        engine = nil
    }

    /// The engine stopped under a configuration change: another one, the gap journaled.
    private func reopen() {
        guard leases > 0, let options else { return }
        let began = Date()
        stop()
        do {
            try start(options)
            log("media-capture: capture reopened, \(Int(Date().timeIntervalSince(began) * 1000)) ms")
            tell("reopened")
        } catch {
            log("media-capture: the capture did not reopen: \(error)")
            tell("interrupted")
        }
    }
}

struct CaptureError: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}
