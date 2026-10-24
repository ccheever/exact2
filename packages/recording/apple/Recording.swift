// @exact/recording on Apple (LLP 1133.009 D1, D2, D6, D8, §10): a track
// `@exact/media-capture` opened, reached through the session's track table,
// recorded to a file and metered.
//
// - A recorder is a consumer of its stream's audio track, so it holds the
//   track's lease while it is not `inactive`; its start is a start (D6). A
//   level reader is a consumer too, and never a start.
// - `audio/mp4` (the unsaid default, D2) is AAC in a fragmented MP4
//   (`AVAssetWriter`, one-second fragments), which plays up to its last
//   fragment if the process ends mid-take (D6); `audio/wav` is 16-bit PCM
//   (`AVAudioFile`). The file is at the admitted `app:/` path, its
//   extension added when the app wrote none.
// - Buffers arrive on the audio thread; each recorder copies what it takes
//   and writes on its own serial queue. A writer two seconds behind stops
//   its recorder with `error` "the writer fell behind" and a finished file
//   of what it wrote (D6).
// - An interruption, a media-services reset or the background pauses a
//   recording recorder (D8); a track that ends stops it (`track-ended`).
// - `audioLevel` (D1): the RMS of each `interval`, computed on the audio
//   thread; an answer covers every interval since the last one.
// - `<audio-waveform>` (D13) draws a track's level on the display link from
//   a meter of its own, so no level crosses into Contract.
// - Under the agent (§10) no engine runs: the level is the synthetic signal
//   on the session clock, and a stopped recorder's file is WAV of a tone that
//   follows it, answered as `audio/wav`.
// - At the session's end every recorder stops and its file is finished
//   (`destroy`, D10 item 1).
import AVFoundation
import ExactPackage
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

public final class RecordingModule: ExactPackage {
    private let context: ExactPackageContext
    private var recorders: [String: Recorder] = [:]
    private var meters: [String: Meter] = [:]
    private var next = 0

    public init(context: ExactPackageContext) { self.context = context }

    /// The synthetic signal at step `i` (one step a 50 ms of the session clock): the recorder's formula (§10).
    static func synthetic(_ i: Int) -> Double {
        let x = Double(i)
        return min(1, 0.12 + 0.75 * abs(sin(x * 0.37)) * (0.55 + 0.45 * sin(x * 0.05)))
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        let recorder = (args["recorder"] as? String).flatMap { recorders[$0] }
        switch name {
        case "startMediaRecorder": reply(start(args))
        case "pauseMediaRecorder", "resumeMediaRecorder":
            guard let r = recorder else { return reply(["error": "NotFoundError"]) }
            guard r.state != "inactive" else { return reply(["error": "InvalidStateError"]) }
            if name == "pauseMediaRecorder" { r.pause("requested") } else if r.state == "paused" { r.resume() }
            reply(r.info())
        case "stopMediaRecorder":
            guard let r = recorder else { return reply(["error": "NotFoundError"]) }
            r.finish("requested") { reply($0) }
        case "mediaRecorderState":
            // A read of no recorder (`recorder=""`): an app reads before it holds one.
            if args["recorder"] as? String ?? "" == "" { return reply(["id": "", "state": "inactive", "mimeType": "", "path": "", "duration": 0]) }
            reply(recorder?.info() ?? ["error": "NotFoundError"])
        case "isTypeSupported":
            let type = args["type"] as? String ?? ""
            reply(["type": type, "supported": context.agent ? type == "audio/wav" : ["audio/mp4", "audio/wav"].contains(type)])
        case "audioLevel": reply(level(args))
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "audioLevelSync" ? level(args) : ["error": "NotSupportedError"]
    }

    /// `<audio-waveform>` (D13).
    public func view(_ tag: String, id: String, props: [String: String], events: ExactPackageEvents) throws -> ExactPackageView {
        guard tag == "audio-waveform" else { throw ExactPackageRefusal("no view \(tag)") }
        let waveform = Waveform(module: self)
        try waveform.setProps(props)
        return waveform
    }

    var now: Double { context.now() }

    /// A waveform's own meter of `track`, a reader that never starts it.
    func waveformMeter(_ track: String) -> Meter? {
        let m = Meter(interval: 50, history: 1000, agent: context.agent, now: context.now())
        m.subscription = try? context.tracks.subscribe(track, m, start: false, origin: context.origin)
        return m.subscription == nil ? nil : m
    }

    private func announce() { context.changed("recording.mediaRecorderState") }

    // MARK: recorders

    private func start(_ args: [String: Any]) -> [String: Any] {
        guard let stream = args["stream"] as? String, let entry = context.tracks.lookup(stream), entry.kind == "stream" else { return ["error": "NotFoundError"] }
        guard let track = entry.tracks.first(where: { context.tracks.lookup($0)?.kind == "audio" }) else { return ["error": "NotSupportedError"] }
        let asked = args["mimeType"] as? String
        let type = context.agent ? "audio/wav" : asked ?? "audio/mp4"
        guard context.agent || ["audio/mp4", "audio/wav"].contains(type) else { return ["error": "NotSupportedError"] }
        guard let path = Recorder.path(args["path"] as? String ?? "", type), let url = context.file(path) else { return ["error": "NotSupportedError"] }
        next += 1
        let r = Recorder(id: "recorder-\(context.session)-\(next)", type: type, path: path, url: url, context: context,
                         bitRate: args["audioBitsPerSecond"] as? Double, apple: args["apple"] as? [String: Any] ?? [:], changed: { [weak self] in self?.announce() })
        do {
            r.subscription = try context.tracks.subscribe(track, r, start: true, origin: context.origin)
        } catch {
            return ["error": (error as? ExactTrackError)?.code ?? "NotFoundError"]
        }
        recorders[r.id] = r
        context.log("recording: \(r.id) recording \(stream) to \(path)")
        announce()
        return r.info()
    }

    // MARK: the level

    private func level(_ args: [String: Any]) -> [String: Any] {
        // A read of no track (`track=""`) is silence: an app reads before it holds one.
        guard let track = args["track"] as? String, !track.isEmpty else { return ["audioLevel": 0, "totalAudioEnergy": 0, "totalSamplesDuration": 0, "levels": [Double]()] }
        let interval = (args["interval"] as? Double).flatMap { $0 > 0 ? $0 : nil } ?? 50
        let history = (args["history"] as? Double).flatMap { $0 > 0 ? Int($0) : nil } ?? 100
        let key = Meter.key(track, args)
        let m: Meter
        if let known = meters[key] {
            m = known
        } else {
            m = Meter(interval: interval, history: history, agent: context.agent, now: context.now())
            do { m.subscription = try context.tracks.subscribe(track, m, start: false, origin: context.origin) } catch {
                return ["error": (error as? ExactTrackError)?.code ?? "NotFoundError"]
            }
            meters[key] = m
        }
        return m.read(context.now())
    }

    /// A level's argument group has no reader left: its meter goes (D10 item 12).
    public func release(_ name: String, args: [String: Any]) {
        guard name == "audioLevel", let track = args["track"] as? String else { return }
        meters.removeValue(forKey: Meter.key(track, args))?.stop()
    }

    public func release(_ name: String) {
        guard name == "audioLevel" else { return }
        for m in meters.values { m.stop() }
        meters.removeAll()
    }

    /// A recorder whose reply was dropped: stopped, its file finished, never the app's (D6).
    public func dispose(_ ids: [String]) {
        for id in ids { recorders[id]?.finish("requested") { _ in } }
    }

    /// The session ends: every recorder stops and its file is finished (D6).
    public func destroy(_ done: @escaping () -> Void) {
        for m in meters.values { m.stop() }
        let group = DispatchGroup()
        for r in recorders.values where r.state != "inactive" {
            group.enter()
            r.finish("session-ended") { _ in group.leave() }
        }
        group.notify(queue: .main, execute: done)
    }
}

/// One recorder: a consumer of its track, writing on its own queue.
final class Recorder: ExactTrackConsumer {
    let id: String, type: String, path: String, url: URL
    private(set) var state = "recording", reason: String?, error: String?
    private var result: [String: Any]?
    private var recorded = 0.0, since: Double?
    private let context: ExactPackageContext, changed: () -> Void
    private let bitRate: Double?, apple: [String: Any]
    var subscription: ExactSubscription?
    private var tap: ExactSubscription?
    /// The agent's spans of recording, ms of the session clock.
    private var spans: [(Double, Double?)] = []
    // The writer's own, on `queue`.
    private let queue: DispatchQueue
    private var writer: Writer?
    private var finishing: [([String: Any]) -> Void] = []
    /// Frames handed to the queue and not yet written: the bound (D6).
    private let pending = Pending()

    init(id: String, type: String, path: String, url: URL, context: ExactPackageContext, bitRate: Double?, apple: [String: Any], changed: @escaping () -> Void) {
        self.id = id; self.type = type; self.path = path; self.url = url; self.context = context
        self.bitRate = bitRate; self.apple = apple; self.changed = changed
        queue = DispatchQueue(label: "exact.recording.\(id)")
        since = context.now()
        spans = [(context.now(), nil)]
    }

    /// The path with its container's extension: added when the app wrote none, nil when it disagrees.
    static func path(_ path: String, _ type: String) -> String? {
        let ext = ["audio/mp4": ".m4a", "audio/wav": ".wav"][type] ?? ""
        let name = path.split(separator: "/").last.map(String.init) ?? ""
        guard let dot = name.lastIndex(of: "."), dot != name.startIndex else { return path + ext }
        return String(name[dot...]) == ext ? path : nil
    }

    var duration: Double { recorded + (state == "recording" ? since.map { max(0, context.now() - $0) } ?? 0 : 0) }

    func info() -> [String: Any] {
        var out: [String: Any] = ["id": id, "state": state, "mimeType": type, "path": path, "duration": duration]
        if let bitRate { out["audioBitsPerSecond"] = bitRate }
        if let reason { out["reason"] = reason }
        if let error { out["error"] = error }
        if let result { out["result"] = result }
        return out
    }

    func pause(_ why: String) {
        guard state == "recording" else { return }
        recorded = duration; since = nil
        if context.agent, let last = spans.indices.last { spans[last].1 = context.now() }
        state = "paused"; reason = why; recordingFlag.value = false
        context.log("recording: \(id) paused (\(why))")
        changed()
    }

    func resume() {
        state = "recording"; reason = nil; since = context.now(); recordingFlag.value = true
        if context.agent { spans.append((context.now(), nil)) }
        changed()
    }

    // MARK: the track

    func live(_ source: ExactAudioSource?) {
        guard let source, tap == nil, state != "inactive" else { return }
        let format = source.format, pending = pending, limit = AVAudioFrameCount(format.sampleRate * 2)
        do {
            let writer = try Writer(url: url, type: type, format: format, bitRate: bitRate, apple: apple)
            queue.sync { self.writer = writer }
        } catch {
            return finish("error", error: "the file did not open: \(error)") { _ in }
        }
        tap = source.tap { [weak self] buffer, _ in
            guard let self, self.isRecording else { return }
            // Nothing waits here: a writer two seconds behind stops the recorder (D6).
            guard pending.add(buffer.frameLength) <= limit, let copy = Recorder.copy(buffer) else {
                DispatchQueue.main.async { self.finish("error", error: "the writer fell behind") { _ in } }
                return
            }
            self.queue.async {
                self.writer?.append(copy)
                pending.remove(copy.frameLength)
            }
        }
    }

    func muted(_ why: String) {
        switch why {
        case "idle": return
        case "media-reset": pause("media-reset")
        case "background": pause("background")
        default: pause("interrupted")
        }
    }

    func unmuted() {}

    func ended() { finish("track-ended") { _ in } }

    /// Written from the audio thread: whether buffers are taken now.
    private let recordingFlag = Flag()
    private var isRecording: Bool { recordingFlag.value }

    /// Stop for `why`, finish the file, answer the result; sticky once inactive (D1).
    func finish(_ why: String, error: String? = nil, _ done: @escaping ([String: Any]) -> Void) {
        if let result { return done(result) }
        finishing.append(done)
        guard state != "inactive" else { return }
        recorded = duration; since = nil
        if context.agent, let last = spans.indices.last, spans[last].1 == nil { spans[last].1 = context.now() }
        state = "inactive"; reason = why; self.error = error
        recordingFlag.value = false
        tap?.cancel(); tap = nil
        changed()
        let spans = spans, agent = context.agent, url = url, duration = recorded, writer = writer
        queue.async { [self] in
            if agent {
                try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
                try? Writer.tone(spans).write(to: url)
            }
            let complete = { DispatchQueue.main.async { self.completed(why, duration) } }
            if let writer { writer.finish(complete) } else { complete() }
        }
    }

    /// The file is finished: the result, answered to every stop that waited.
    private func completed(_ why: String, _ duration: Double) {
        let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.intValue ?? 0
        subscription?.cancel(); subscription = nil
        let result: [String: Any] = ["path": path, "mimeType": type, "duration": duration, "size": size]
        self.result = result
        context.log("recording: \(id) stopped (\(why)), \(path), \(size) bytes")
        changed()
        for f in finishing { f(result) }
        finishing = []
    }

    /// A buffer's frames, copied off the audio thread's reuse.
    static func copy(_ buffer: AVAudioPCMBuffer) -> AVAudioPCMBuffer? {
        guard let out = AVAudioPCMBuffer(pcmFormat: buffer.format, frameCapacity: buffer.frameLength) else { return nil }
        out.frameLength = buffer.frameLength
        let from = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: buffer.audioBufferList))
        let to = UnsafeMutableAudioBufferListPointer(out.mutableAudioBufferList)
        for (a, b) in zip(from, to) where a.mData != nil && b.mData != nil { memcpy(b.mData, a.mData, Int(min(a.mDataByteSize, b.mDataByteSize))) }
        return out
    }
}

/// A flag the main thread sets and the audio thread reads.
final class Flag: @unchecked Sendable {
    private let lock = NSLock()
    private var _value = true
    var value: Bool {
        get { lock.lock(); defer { lock.unlock() }; return _value }
        set { lock.lock(); _value = newValue; lock.unlock() }
    }
}

/// Frames handed to a writer and not yet written.
final class Pending: @unchecked Sendable {
    private let lock = NSLock()
    private var frames: AVAudioFrameCount = 0
    func add(_ n: AVAudioFrameCount) -> AVAudioFrameCount { lock.lock(); defer { lock.unlock() }; frames += n; return frames }
    func remove(_ n: AVAudioFrameCount) { lock.lock(); frames -= min(frames, n); lock.unlock() }
}

/// A file being written: AAC in fragmented MP4, or 16-bit PCM WAV.
final class Writer {
    private var asset: AVAssetWriter?, input: AVAssetWriterInput?, file: AVAudioFile?
    private var frames: Int64 = 0
    private let rate: Double

    init(url: URL, type: String, format: AVAudioFormat, bitRate: Double?, apple: [String: Any]) throws {
        rate = format.sampleRate
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? FileManager.default.removeItem(at: url)
        if type == "audio/wav" {
            let settings: [String: Any] = [AVFormatIDKey: kAudioFormatLinearPCM, AVSampleRateKey: format.sampleRate, AVNumberOfChannelsKey: format.channelCount,
                                           AVLinearPCMBitDepthKey: 16, AVLinearPCMIsFloatKey: false, AVLinearPCMIsBigEndianKey: false]
            file = try AVAudioFile(forWriting: url, settings: settings, commonFormat: format.commonFormat, interleaved: format.isInterleaved)
            return
        }
        let writer = try AVAssetWriter(outputURL: url, fileType: .m4a)
        // One-second fragments: a take whose process ended plays up to its last fragment (D6).
        writer.movieFragmentInterval = CMTime(seconds: 1, preferredTimescale: 600)
        var settings: [String: Any] = [AVFormatIDKey: kAudioFormatMPEG4AAC, AVSampleRateKey: format.sampleRate, AVNumberOfChannelsKey: min(format.channelCount, 2)]
        if let bitRate { settings[AVEncoderBitRateKey] = bitRate }
        let qualities: [String: AVAudioQuality] = ["min": .min, "low": .low, "medium": .medium, "high": .high, "max": .max]
        if let q = (apple["audioQuality"] as? String).flatMap({ qualities[$0] }) { settings[AVEncoderAudioQualityKey] = q.rawValue }
        let input = AVAssetWriterInput(mediaType: .audio, outputSettings: settings)
        input.expectsMediaDataInRealTime = true
        guard writer.canAdd(input) else { throw CaptureFailure("the writer takes no audio input") }
        writer.add(input)
        guard writer.startWriting() else { throw writer.error ?? CaptureFailure("the writer did not start") }
        writer.startSession(atSourceTime: .zero)
        asset = writer; self.input = input
    }

    /// One buffer, on the recorder's queue. Its time is the frames written,
    /// so a pause leaves no gap in the file.
    func append(_ buffer: AVAudioPCMBuffer) {
        if let file { try? file.write(from: buffer); return }
        guard let input, input.isReadyForMoreMediaData, let sample = sample(buffer) else { return }
        if input.append(sample) { frames += Int64(buffer.frameLength) }
    }

    private func sample(_ buffer: AVAudioPCMBuffer) -> CMSampleBuffer? {
        var description: CMAudioFormatDescription?
        guard CMAudioFormatDescriptionCreate(allocator: nil, asbd: buffer.format.streamDescription, layoutSize: 0, layout: nil, magicCookieSize: 0,
                                             magicCookie: nil, extensions: nil, formatDescriptionOut: &description) == noErr else { return nil }
        var timing = CMSampleTimingInfo(duration: CMTime(value: 1, timescale: CMTimeScale(rate)),
                                        presentationTimeStamp: CMTime(value: frames, timescale: CMTimeScale(rate)), decodeTimeStamp: .invalid)
        var sample: CMSampleBuffer?
        guard CMSampleBufferCreate(allocator: nil, dataBuffer: nil, dataReady: false, makeDataReadyCallback: nil, refcon: nil, formatDescription: description,
                                   sampleCount: CMItemCount(buffer.frameLength), sampleTimingEntryCount: 1, sampleTimingArray: &timing,
                                   sampleSizeEntryCount: 0, sampleSizeArray: nil, sampleBufferOut: &sample) == noErr, let sample,
              CMSampleBufferSetDataBufferFromAudioBufferList(sample, blockBufferAllocator: nil, blockBufferMemoryAllocator: nil, flags: 0,
                                                             bufferList: buffer.audioBufferList) == noErr else { return nil }
        return sample
    }

    /// Finish the file, then `done`.
    func finish(_ done: @escaping () -> Void) {
        guard let asset, let input else { file = nil; return done() }
        input.markAsFinished()
        asset.finishWriting(completionHandler: done)
    }

    /// The agent's take: WAV, 16-bit mono at 8 kHz, of a 440 Hz tone whose amplitude follows the signal over its spans (§10).
    static func tone(_ spans: [(Double, Double?)], rate: Double = 8000) -> Data {
        var samples: [Int16] = []
        for (from, to) in spans {
            var t = from
            while t < (to ?? from) {
                let level = RecordingModule.synthetic(Int(floor(t / 50)))
                samples.append(Int16(max(-1, min(1, 0.8 * level * sin(2 * .pi * 440 * t / 1000))) * 32767))
                t += 1000 / rate
            }
        }
        var data = Data()
        func word<T: FixedWidthInteger>(_ v: T) { withUnsafeBytes(of: v.littleEndian) { data.append(contentsOf: $0) } }
        data.append(contentsOf: Array("RIFF".utf8)); word(UInt32(36 + samples.count * 2)); data.append(contentsOf: Array("WAVE".utf8))
        data.append(contentsOf: Array("fmt ".utf8)); word(UInt32(16)); word(UInt16(1)); word(UInt16(1)); word(UInt32(rate)); word(UInt32(rate * 2)); word(UInt16(2)); word(UInt16(16))
        data.append(contentsOf: Array("data".utf8)); word(UInt32(samples.count * 2))
        for s in samples { word(s) }
        return data
    }
}

struct CaptureFailure: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}

/// A level on one track with one argument group (D1).
final class Meter: ExactTrackConsumer {
    let interval: Double, history: Int, agent: Bool
    var subscription: ExactSubscription?
    private var tap: ExactSubscription?
    private(set) var levels: [Double] = []
    private var energy = 0.0, duration = 0.0, last = 0.0
    private var at: Double, running = false, quiet = false
    /// Each interval's RMS, from the audio thread.
    private let measured = Measured()

    init(interval: Double, history: Int, agent: Bool, now: Double) {
        self.interval = interval; self.history = history; self.agent = agent; at = now
    }

    static func key(_ track: String, _ args: [String: Any]) -> String {
        "\(track) \(args["interval"] as? Double ?? 0) \(args["history"] as? Double ?? 0)"
    }

    func live(_ source: ExactAudioSource?) {
        running = true
        guard let source, tap == nil else { return }
        let window = AVAudioFrameCount(max(1, source.format.sampleRate * interval / 1000)), measured = measured
        tap = source.tap { buffer, _ in measured.add(buffer, window: window) }
    }
    func muted(_ why: String) { quiet = true }
    func unmuted() { quiet = false }
    func ended() { running = false; tap?.cancel(); tap = nil }
    func stop() { tap?.cancel(); tap = nil; subscription?.cancel(); subscription = nil }

    /// Every interval since the last answer (D1).
    func read(_ now: Double) -> [String: Any] {
        let steps = Int(floor((now - at) / interval))
        var fresh: [Double] = []
        if steps > 0 {
            if !running || quiet { fresh = Array(repeating: 0, count: steps) }
            else if agent { fresh = (1...steps).map { RecordingModule.synthetic(Int(floor((at + Double($0) * interval) / 50))) } }
            else { fresh = Array(measured.take().suffix(steps)) }
            at += Double(steps) * interval
        }
        for level in fresh { energy += level * level * interval / 1000; duration += interval / 1000; last = level }
        levels.append(contentsOf: fresh)
        if levels.count > history { levels.removeFirst(levels.count - history) }
        return ["audioLevel": last, "totalAudioEnergy": energy, "totalSamplesDuration": duration, "levels": levels]
    }
}

/// RMS levels computed on the audio thread, taken on main.
final class Measured: @unchecked Sendable {
    private let lock = NSLock()
    private var done: [Double] = [], sum = 0.0, count: AVAudioFrameCount = 0
    func add(_ buffer: AVAudioPCMBuffer, window: AVAudioFrameCount) {
        guard let samples = buffer.floatChannelData?[0] else { return }
        lock.lock(); defer { lock.unlock() }
        for i in 0..<Int(buffer.frameLength) {
            sum += Double(samples[i] * samples[i]); count += 1
            if count >= window { done.append(min(1, (sum / Double(count)).squareRoot())); sum = 0; count = 0 }
        }
        if done.count > 4096 { done.removeFirst(done.count - 4096) }
    }
    func take() -> [Double] { lock.lock(); defer { lock.unlock() }; let out = done; done = []; return out }
}

// MARK: the waveform (D13)

#if os(macOS)
typealias WaveformColor = NSColor
#else
typealias WaveformColor = UIColor
#endif

/// `<audio-waveform track=…>`: the track's levels as bars, newest at the
/// right, drawn each display frame from a meter of its own, so no level
/// crosses into Contract. A reader of the track, never a start: an idle
/// track draws silence.
final class Waveform: NSObject, ExactPackageView {
    private let canvas = WaveformCanvas()
    private let module: RecordingModule
    private var track = ""

    init(module: RecordingModule) {
        self.module = module
        super.init()
        canvas.levels = { [weak self] fit in self?.levels(fit) ?? [] }
    }

    var view: ExactPackagePlatformView { canvas }

    func setProps(_ props: [String: String]) throws {
        canvas.color = props["tint"].flatMap(Waveform.color) ?? canvas.color
        let next = props["track"] ?? ""
        guard next != track else { return }
        canvas.meter?.stop()
        track = next
        canvas.meter = track.isEmpty ? nil : module.waveformMeter(track)
        canvas.bars = []
    }

    private func levels(_ fit: Int) -> [Double] {
        guard let meter = canvas.meter else { return [] }
        _ = meter.read(module.now)
        return Array(meter.levels.suffix(fit))
    }

    func destroy() { canvas.stop(); canvas.meter?.stop() }

    /// `#rgb` or `#rrggbb`.
    static func color(_ hex: String) -> WaveformColor? {
        var digits = hex.hasPrefix("#") ? String(hex.dropFirst()) : hex
        if digits.count == 3 { digits = digits.map { "\($0)\($0)" }.joined() }
        guard digits.count == 6, let v = UInt32(digits, radix: 16) else { return nil }
        let c = { (shift: UInt32) in CGFloat((v >> shift) & 0xff) / 255 }
        return WaveformColor(red: c(16), green: c(8), blue: c(0), alpha: 1)
    }
}

final class WaveformCanvas: ExactPackagePlatformView {
    var color: WaveformColor = .systemRed
    var meter: Meter?
    var bars: [Double] = []
    var levels: (Int) -> [Double] = { _ in [] }
    private var link: AnyObject?
    static let bar: CGFloat = 3, gap: CGFloat = 2

    init() {
        super.init(frame: .zero)
        #if os(macOS)
        wantsLayer = true
        let link = displayLink(target: self, selector: #selector(tick))
        link.add(to: .main, forMode: .common)
        self.link = link
        #else
        isUserInteractionEnabled = false
        isOpaque = false
        contentMode = .redraw
        let link = CADisplayLink(target: self, selector: #selector(tick))
        link.add(to: .main, forMode: .common)
        self.link = link
        #endif
    }

    required init?(coder: NSCoder) { fatalError("not from a coder") }

    #if os(macOS)
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    #endif

    @objc private func tick() {
        let fit = max(0, Int((bounds.width + Self.gap) / (Self.bar + Self.gap)))
        bars = levels(fit)
        #if os(macOS)
        needsDisplay = true
        #else
        setNeedsDisplay()
        #endif
    }

    func stop() { (link as? CADisplayLink)?.invalidate(); link = nil }

    override func draw(_ rect: CGRect) {
        #if os(macOS)
        guard let g = NSGraphicsContext.current?.cgContext else { return }
        #else
        guard let g = UIGraphicsGetCurrentContext() else { return }
        #endif
        let w = bounds.width, h = bounds.height
        g.setFillColor(color.cgColor)
        for (i, level) in bars.enumerated() {
            let x = w - CGFloat(bars.count - i) * (Self.bar + Self.gap) + Self.gap
            let half = max(1, CGFloat(level) * (h / 2 - 2))
            g.fill(CGRect(x: x, y: h / 2 - half, width: Self.bar, height: half * 2))
        }
    }
}
