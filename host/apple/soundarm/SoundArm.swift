// @ref LLP 1096 D6, D8. The sound arm: one AVAudioEngine holding one
// AVAudioSourceNode whose render block is the C mixer (sound_render.c),
// connected to `mainMixerNode` in the output's own format, so nothing converts
// downstream of it (a sample time downstream of a converter is not valid).
// Every declared sound is read once and converted to that format: a mono file
// is copied to every output channel, a stereo file going to a mono output is
// averaged. Loaded on demand (SoundModule.swift), never linked into ExactKit;
// the session is ExactKit's (AudioSession.swift), not the arm's.
//
// An output that cannot open is a reason, never a crash: every AVAudioEngine
// call that can raise is made inside sound_engine.m's @try, so no exception
// unwinds through Swift, and comes back from `exact_sound_open` and
// `exact_sound_engine` as text. A process abort inside CoreAudio
// (AURemoteIO's RPC timeout) cannot be caught; SoundModule.swift never opens
// the output at boot, and on iOS and tvOS not before the first voice.
import AVFoundation
import Foundation

public typealias SoundLog = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void

/// A failure's text from a C reason buffer.
private func text(_ reason: [CChar]) -> String { String(cString: reason) }

private final class SoundArm {
    let engine: AVAudioEngine
    let format: AVAudioFormat
    let mixer: OpaquePointer
    var source: AVAudioSourceNode?
    var planes: [UnsafeMutablePointer<Float>] = []
    let context: UnsafeMutableRawPointer?
    let log: SoundLog?
    var started: [UInt64: Double] = [:]
    var checking = false
    var silentFor = 1.0
    var changed: NSObjectProtocol?
    /// The media services were reset: the engine is invalid and is not messaged again.
    var orphaned = false

    private init(engine: AVAudioEngine, format: AVAudioFormat, mixer: OpaquePointer, context: UnsafeMutableRawPointer?, log: SoundLog?) {
        self.engine = engine
        self.format = format
        self.mixer = mixer
        self.context = context
        self.log = log
    }

    /// The arm with every sound read and its graph prepared, its engine
    /// running when `start`, or why not.
    static func open(_ paths: [String], start: Bool, context: UnsafeMutableRawPointer?, log: SoundLog?) -> (SoundArm?, String?) {
        let began = CACurrentMediaTime()
        var reason = [CChar](repeating: 0, count: 512), rate = 0.0, channels: UInt32 = 0
        guard let engine = exact_sound_engine_new(&rate, &channels, &reason, reason.count) else { return (nil, text(reason)) }
        guard rate > 0, channels > 0 else { return (nil, "the output has no channels (no audio device)") }
        guard let format = AVAudioFormat(standardFormatWithSampleRate: rate, channels: channels),
              let mixer = exact_sound_mixer(channels, rate) else { return (nil, "the output's format (\(rate) Hz, \(channels) channels) is not one the mixer takes") }
        let arm = SoundArm(engine: engine, format: format, mixer: mixer, context: context, log: log)
        if let failure = arm.build(paths, began: began) ?? (start ? arm.start() : nil) { return (nil, failure) }
        return (arm, nil)
    }

    /// Reads the sounds, connects the source and prepares the engine: nil, or why not.
    private func build(_ paths: [String], began: Double) -> String? {
        let output = CACurrentMediaTime()
        for (i, path) in paths.enumerated() {
            if let (plane, frames) = load(path) {
                planes.append(plane)
                exact_sound_set(mixer, UInt32(i), plane, frames)
            } else {
                say("sound: \((path as NSString).lastPathComponent) could not be read")
            }
        }
        let read = CACurrentMediaTime()
        var reason = [CChar](repeating: 0, count: 512)
        guard let source = exact_sound_engine_connect(engine, mixer, format.sampleRate, format.channelCount, &reason, reason.count) else { return text(reason) }
        self.source = source
        // A route or format change stops the engine (AVAudioEngineConfigurationChangeNotification);
        // the session's next voice finds it stopped and opens a new one in the new format.
        changed = NotificationCenter.default.addObserver(forName: .AVAudioEngineConfigurationChange, object: engine, queue: .main) { [weak self] _ in
            self?.say("sound: the output changed and its engine stopped; the next sound opens it again")
        }
        say(String(format: "sound: the output was made at %.0f Hz, %u channels, in %.0f ms (%d sounds read in %.0f ms)", format.sampleRate, format.channelCount, (CACurrentMediaTime() - began) * 1000, planes.count, (read - output) * 1000))
        return nil
    }

    deinit {
        if let changed { NotificationCenter.default.removeObserver(changed) }
        // An orphan is never messaged again, deallocation included (Apple's
        // reset guidance: its objects are invalid); it, and the mixer its
        // render block reads, are left as they are. An engine that raised even
        // on stop may still render from the mixer, which is left allocated too.
        // Both are rare enough that the kit left behind costs less than a crash.
        if orphaned { _ = Unmanaged.passRetained(engine); return }
        guard exact_sound_engine_stop(engine) else { return }
        exact_sound_mixer_free(mixer)
        planes.forEach { $0.deallocate() }
    }

    /// Starts the engine: nil, or why it did not.
    func start() -> String? {
        let began = CACurrentMediaTime()
        var reason = [CChar](repeating: 0, count: 512)
        guard exact_sound_engine_start(engine, &reason, reason.count) else { return text(reason) }
        // The source node's delay to the speaker: the mixer, the output's
        // processing and the device (AVAudioNode.outputPresentationLatency).
        exact_sound_latency(mixer, source?.outputPresentationLatency ?? 0)
        say(String(format: "sound: the output started in %.0f ms", (CACurrentMediaTime() - began) * 1000))
        return nil
    }

    func pause() -> String? {
        var reason = [CChar](repeating: 0, count: 512)
        return exact_sound_engine_pause(engine, &reason, reason.count) ? nil : text(reason)
    }

    func say(_ line: String) {
        let bytes = Array(line.utf8)
        bytes.withUnsafeBufferPointer { log?(context, $0.baseAddress, $0.count) }
    }

    /// The file's frames in the output format, as planes, or nil.
    func load(_ path: String) -> (UnsafeMutablePointer<Float>, UInt32)? {
        guard let file = try? AVAudioFile(forReading: URL(fileURLWithPath: path)),
              let read = AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: AVAudioFrameCount(file.length)),
              (try? file.read(into: read)) != nil else { return nil }
        // The rate converted once, the channels mapped here.
        var pcm = read
        if file.processingFormat.sampleRate != format.sampleRate {
            guard let target = AVAudioFormat(standardFormatWithSampleRate: format.sampleRate, channels: file.processingFormat.channelCount),
                  let converter = AVAudioConverter(from: file.processingFormat, to: target),
                  let out = AVAudioPCMBuffer(pcmFormat: target, frameCapacity: AVAudioFrameCount(ceil(Double(read.frameLength) * format.sampleRate / file.processingFormat.sampleRate)) + 64) else { return nil }
            var given = false
            var error: NSError?
            _ = converter.convert(to: out, error: &error) { _, status in
                if given { status.pointee = .endOfStream; return nil }
                given = true
                status.pointee = .haveData
                return read
            }
            guard error == nil else { return nil }
            pcm = out
        }
        guard let data = pcm.floatChannelData, pcm.frameLength > 0 else { return nil }
        let frames = Int(pcm.frameLength), channels = Int(format.channelCount), inChannels = Int(pcm.format.channelCount)
        let plane = UnsafeMutablePointer<Float>.allocate(capacity: frames * channels)
        for c in 0..<channels {
            for f in 0..<frames {
                plane[c * frames + f] = inChannels == 1 ? data[0][f]
                    : channels == 1 ? (data[0][f] + data[1][f]) / 2
                    : data[min(c, inChannels - 1)][f]
            }
        }
        return (plane, UInt32(frames))
    }

    func push(_ kind: UInt32, _ id: UInt64, _ sound: UInt32, _ at: Double, _ gain: Float) {
        if kind == 1 { started[id] = at }
        if !exact_sound_push(mixer, kind, id, sound, at, gain) { say("sound dropped: the mixer's queue is full") }
    }

    /// The device check (LLP 1096 §5): a tap on `mainMixerNode` finds each
    /// onset after a silence and journals its speaker time — the tap
    /// buffer's host time, plus its frame's offset, plus the latency still
    /// downstream of the tap — against the time its voice was aimed at.
    func check() -> String? {
        guard !checking else { return nil }
        checking = true
        let node = engine.mainMixerNode
        say(String(format: "sound check: presentation latency %.3f ms from the source, %.3f ms from the mixer, at %.0f Hz", (source?.outputPresentationLatency ?? 0) * 1000, node.outputPresentationLatency * 1000, format.sampleRate))
        let rate = node.outputFormat(forBus: 0).sampleRate
        var reason = [CChar](repeating: 0, count: 512)
        let tapped = exact_sound_engine_tap(engine, { [weak self] buffer, when in
            guard let self, let buffer, let when, let data = buffer.floatChannelData, when.isHostTimeValid else { return }
            let host = exact_sound_seconds(when.hostTime)
            let downstream = node.outputPresentationLatency
            for f in 0..<Int(buffer.frameLength) {
                if abs(data[0][f]) > 1e-4 {
                    if self.silentFor >= 0.05 {
                        let onset = host + Double(f) / rate + downstream
                        DispatchQueue.main.async { self.report(onset) }
                    }
                    self.silentFor = 0
                } else {
                    self.silentFor += 1 / rate
                }
            }
        }, &reason, reason.count)
        return tapped ? nil : text(reason)
    }

    func report(_ onset: Double) {
        guard let (id, aimed) = started.filter({ $0.value <= onset + 0.1 }).max(by: { $0.value < $1.value }) else { return }
        started[id] = nil
        say(String(format: "sound check: voice %llu reached the speaker %+.3f ms from its time", id, (onset - aimed) * 1000))
    }
}

/// The output, its engine started when `start` (else by `exact_sound_engine`
/// 1), or nil with why not in `reason` (`size` bytes, NUL-terminated).
/// `exact_sound_open`, `exact_sound_engine` and `exact_sound_close` are
/// called on one serial queue; `exact_sound_op` and `exact_sound_counts` on
/// the main thread, the mixer ring's one producer.
@_cdecl("exact_sound_open")
public func exact_sound_open(_ paths: UnsafePointer<UnsafePointer<CChar>?>?, _ count: Int, _ context: UnsafeMutableRawPointer?, _ log: SoundLog?, _ start: Bool, _ reason: UnsafeMutablePointer<CChar>?, _ size: Int) -> UnsafeMutableRawPointer? {
    let list = (0..<count).compactMap { paths?[$0].map { String(cString: $0) } }
    let (arm, failure) = SoundArm.open(list, start: start, context: context, log: log)
    guard let arm else {
        if let reason, size > 0 { _ = strlcpy(reason, failure ?? "the output did not open", size) }
        return nil
    }
    return Unmanaged.passRetained(arm).toOpaque()
}

/// `kind` 1 plays `sound` from host second `at` times `gain`; 2 ends voice
/// `id` at `at`; 3 ends every voice now.
@_cdecl("exact_sound_op")
public func exact_sound_op(_ handle: UnsafeMutableRawPointer?, _ kind: UInt32, _ id: UInt64, _ sound: UInt32, _ at: Double, _ gain: Float) {
    guard let handle else { return }
    Unmanaged<SoundArm>.fromOpaque(handle).takeUnretainedValue().push(kind, id, sound, at, gain)
}

/// What `state.sounds.output` reports: live voices, dropped ops and plays, late starts.
@_cdecl("exact_sound_counts")
public func exact_sound_counts(_ handle: UnsafeMutableRawPointer?, _ out: UnsafeMutablePointer<UInt64>?) {
    guard let handle, let out else { return }
    let arm = Unmanaged<SoundArm>.fromOpaque(handle).takeUnretainedValue()
    out[0] = UInt64(exact_sound_live(arm.mixer)); out[1] = exact_sound_dropped(arm.mixer); out[2] = exact_sound_late(arm.mixer)
    out[3] = arm.engine.isRunning ? 1 : 0
}

/// 0 pauses the engine (an interruption began), 1 starts it, 2 starts the
/// device check, 3 marks it orphaned (the media services were reset), so its
/// close never messages it: false, with why in `reason`, when it failed.
@_cdecl("exact_sound_engine")
public func exact_sound_engine(_ handle: UnsafeMutableRawPointer?, _ what: UInt32, _ reason: UnsafeMutablePointer<CChar>?, _ size: Int) -> Bool {
    guard let handle else { return false }
    let arm = Unmanaged<SoundArm>.fromOpaque(handle).takeUnretainedValue()
    if what == 3 { arm.orphaned = true; return true }
    guard let failure = what == 0 ? arm.pause() : what == 1 ? arm.start() : arm.check() else { return true }
    if let reason, size > 0 { _ = strlcpy(reason, failure, size) }
    return false
}

@_cdecl("exact_sound_close")
public func exact_sound_close(_ handle: UnsafeMutableRawPointer?) {
    guard let handle else { return }
    Unmanaged<SoundArm>.fromOpaque(handle).release()
}
