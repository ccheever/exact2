// @ref LLP 1096 D6, D8. The sound arm: one AVAudioEngine holding one
// AVAudioSourceNode whose render callback is the C mixer (sound_render.c),
// connected to `mainMixerNode` in the output's own format, so nothing converts
// downstream of it (a sample time downstream of a converter is not valid).
// Every declared sound is read once and converted to that format: a mono file
// is copied to every output channel, a stereo file going to a mono output is
// averaged. Loaded on demand (SoundModule.swift), never linked into ExactKit;
// the session is ExactKit's (AudioSession.swift), not the arm's.
import AVFoundation
import Foundation

public typealias SoundLog = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void

private final class SoundArm {
    let engine = AVAudioEngine()
    let format: AVAudioFormat
    let mixer: OpaquePointer
    var source: AVAudioSourceNode!
    var planes: [UnsafeMutablePointer<Float>] = []
    let context: UnsafeMutableRawPointer?
    let log: SoundLog?
    var started: [UInt64: Double] = [:]
    var checking = false
    var silentFor = 1.0

    init?(paths: [String], context: UnsafeMutableRawPointer?, log: SoundLog?) {
        self.context = context
        self.log = log
        let output = engine.outputNode.outputFormat(forBus: 0)
        guard output.sampleRate > 0, output.channelCount > 0,
              let format = AVAudioFormat(standardFormatWithSampleRate: output.sampleRate, channels: output.channelCount),
              let mixer = exact_sound_mixer(format.channelCount, format.sampleRate) else { return nil }
        self.format = format
        self.mixer = mixer
        for (i, path) in paths.enumerated() {
            if let (plane, frames) = load(path) {
                planes.append(plane)
                exact_sound_set(mixer, UInt32(i), plane, frames)
            } else {
                say("sound: \((path as NSString).lastPathComponent) could not be read")
            }
        }
        // The C callback does the work: no Swift runs per frame, nothing allocates.
        let m = mixer
        source = AVAudioSourceNode(format: format) { isSilence, timestamp, frames, buffers in
            isSilence.pointee = ObjCBool(!exact_sound_render(m, timestamp, frames, buffers))
            return noErr
        }
        engine.attach(source)
        engine.connect(source, to: engine.mainMixerNode, format: format)
        engine.prepare()
        guard start() else { return nil }
    }

    deinit {
        engine.stop()
        exact_sound_mixer_free(mixer)
        planes.forEach { $0.deallocate() }
    }

    @discardableResult func start() -> Bool {
        do { try engine.start() } catch { say("sound: the engine did not start: \(error.localizedDescription)"); return false }
        // The source node's delay to the speaker: the mixer, the output's
        // processing and the device (AVAudioNode.outputPresentationLatency).
        exact_sound_latency(mixer, source.outputPresentationLatency)
        return true
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
    func check() {
        guard !checking else { return }
        checking = true
        let node = engine.mainMixerNode
        say(String(format: "sound check: presentation latency %.3f ms from the source, %.3f ms from the mixer, at %.0f Hz", source.outputPresentationLatency * 1000, node.outputPresentationLatency * 1000, format.sampleRate))
        let rate = node.outputFormat(forBus: 0).sampleRate
        node.installTap(onBus: 0, bufferSize: 256, format: nil) { [weak self] buffer, when in
            guard let self, let data = buffer.floatChannelData, when.isHostTimeValid else { return }
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
        }
    }

    func report(_ onset: Double) {
        guard let (id, aimed) = started.filter({ $0.value <= onset + 0.1 }).max(by: { $0.value < $1.value }) else { return }
        started[id] = nil
        say(String(format: "sound check: voice %llu reached the speaker %+.3f ms from its time", id, (onset - aimed) * 1000))
    }
}

@_cdecl("exact_sound_open")
public func exact_sound_open(_ paths: UnsafePointer<UnsafePointer<CChar>?>?, _ count: Int, _ context: UnsafeMutableRawPointer?, _ log: SoundLog?) -> UnsafeMutableRawPointer? {
    let list = (0..<count).compactMap { paths?[$0].map { String(cString: $0) } }
    guard let arm = SoundArm(paths: list, context: context, log: log) else { return nil }
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

/// 0 stops the engine (an interruption began), 1 starts it again, 2 starts the device check.
@_cdecl("exact_sound_engine")
public func exact_sound_engine(_ handle: UnsafeMutableRawPointer?, _ what: UInt32) {
    guard let handle else { return }
    let arm = Unmanaged<SoundArm>.fromOpaque(handle).takeUnretainedValue()
    switch what {
    case 0: arm.engine.pause()
    case 1: arm.start()
    default: arm.check()
    }
}

@_cdecl("exact_sound_close")
public func exact_sound_close(_ handle: UnsafeMutableRawPointer?) {
    guard let handle else { return }
    Unmanaged<SoundArm>.fromOpaque(handle).release()
}
