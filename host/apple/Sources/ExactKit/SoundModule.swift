// @ref LLP 1096 D6, D8. The runner's voice table, played: a session's `sound`
// batch op names the plan's files at a boot and carries `play` and `end` ops
// in runner milliseconds, which become host seconds — runner time is
// `(CACurrentMediaTime() - t0) × 1000` — for the sound arm's mixer. The arm
// (soundarm/, AVFAudio) is a dylib loaded on demand, never ExactKit's link
// graph; nothing loads or plays under the agent, whose clock is virtual (D10).
import Foundation
#if os(iOS) || os(tvOS)
import AVFoundation
#endif

private typealias SoundLogCallback = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void

private final class SoundLibrary {
    typealias Open = @convention(c) (UnsafePointer<UnsafePointer<CChar>?>?, Int, UnsafeMutableRawPointer?, SoundLogCallback?) -> UnsafeMutableRawPointer?
    typealias Op = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt64, UInt32, Double, Float) -> Void
    typealias Counts = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutablePointer<UInt64>?) -> Void
    typealias Engine = @convention(c) (UnsafeMutableRawPointer?, UInt32) -> Void
    typealias Close = @convention(c) (UnsafeMutableRawPointer?) -> Void
    let open: Open, op: Op, counts: Counts, engine: Engine, close: Close
    private init(_ library: UnsafeMutableRawPointer) {
        func symbol<T>(_ name: String, _: T.Type) -> T { unsafeBitCast(dlsym(library, name)!, to: T.self) }
        open = symbol("exact_sound_open", Open.self)
        op = symbol("exact_sound_op", Op.self)
        counts = symbol("exact_sound_counts", Counts.self)
        engine = symbol("exact_sound_engine", Engine.self)
        close = symbol("exact_sound_close", Close.self)
    }
    static let shared: SoundLibrary? = {
        // As the video arm is loaded (VideoModule.swift): beside the Mac
        // executable; on iOS and tvOS, the app's Frameworks.
        #if os(macOS)
        let path = Bundle.main.executableURL!.deletingLastPathComponent().path + "/libexact_sound.dylib"
        #else
        let path = embeddedModule(framework: "ExactSound", dylib: "libexact_sound.dylib")
        #endif
        guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            FileHandle.standardError.write(Data("exact sound: \(String(cString: dlerror()))\n".utf8)); return nil
        }
        guard ["open", "op", "counts", "engine", "close"].allSatisfy({ dlsym(library, "exact_sound_" + $0) != nil }) else {
            dlclose(library); return nil
        }
        return SoundLibrary(library)
    }()
}

/// One session's output.
final class SoundOutput {
    private weak var session: ExactSession?
    private var handle: UnsafeMutableRawPointer?
    private var files: [String] = []
    private var interrupted = false

    init(_ session: ExactSession) { self.session = session }
    deinit { if let handle { SoundLibrary.shared?.close(handle) } }

    /// A batch's `sound` op: the files at a boot (the last boot's voices end
    /// now), then its ops in order.
    func apply(_ payload: [String: Any]) {
        guard !ExactEnv.agentMode, let session else { return }
        if let files = payload["files"] as? [String], files != self.files || handle == nil {
            self.files = files
            open(session)
        } else if payload["files"] != nil, let handle {
            SoundLibrary.shared?.op(handle, 3, 0, 0, 0, 0)
        }
        guard let handle, let library = SoundLibrary.shared else { return }
        for op in payload["ops"] as? [[String: Any]] ?? [] {
            let id = UInt64((op["id"] as? NSNumber)?.int64Value ?? 0), at = (op["at"] as? NSNumber)?.doubleValue ?? 0
            // Runner time to host seconds on CACurrentMediaTime's timebase (D6).
            let host = ExactEnv.t0 + at / 1000
            if op["op"] as? String == "play" {
                if interrupted { session.log("sound skipped: interrupted"); continue }
                library.op(handle, 1, id, UInt32((op["sound"] as? NSNumber)?.intValue ?? 0), host, (op["gain"] as? NSNumber)?.floatValue ?? 1)
            } else {
                library.op(handle, 2, id, 0, host, 0)
            }
        }
    }

    private func open(_ session: ExactSession) {
        if let handle { SoundLibrary.shared?.close(handle); self.handle = nil }
        guard let library = SoundLibrary.shared else { session.log("sound: unavailable: no sound module in this build"); return }
        #if os(iOS) || os(tvOS)
        do { try AudioSession.activate() } catch { session.log("sound: the audio session did not activate: \(error.localizedDescription)") }
        observeInterruptions()
        #endif
        let paths = files.map { NodeView.resolveSource($0, app: session.app)?.path ?? $0 }
        let c = paths.map { strdup($0) }
        defer { c.forEach { free($0) } }
        let context = Unmanaged.passUnretained(self).toOpaque()
        handle = c.map { UnsafePointer($0) }.withUnsafeBufferPointer { library.open($0.baseAddress, $0.count, context, { context, bytes, count in
            guard let context, let bytes else { return }
            let line = String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
            let output = Unmanaged<SoundOutput>.fromOpaque(context).takeUnretainedValue()
            // The device check's lines also go to stderr: a run outside the agent has no journal reader.
            if line.hasPrefix("sound check:") { fputs(line + "\n", stderr) }
            DispatchQueue.main.async { output.session?.log(line) }
        }) }
        if handle == nil { session.log("sound: unavailable: the audio engine did not start") }
        // The device check (LLP 1096 §5), when a development run asks for it.
        if let handle, ExactEnv.environment["EXACT_SOUND_CHECK"] == "1" { library.engine(handle, 2) }
    }

    /// `state.sounds.output` outside the agent.
    var state: [String: Any] {
        guard let handle, let library = SoundLibrary.shared else { return ["kind": "avaudioengine", "state": "unavailable"] }
        var c = [UInt64](repeating: 0, count: 4)
        library.counts(handle, &c)
        return ["kind": "avaudioengine", "state": c[3] == 1 ? "running" : "stopped", "live": c[0], "dropped": c[1], "late": c[2], "decoded": "\(files.count)/\(files.count)"]
    }

    #if os(iOS) || os(tvOS)
    private var observing = false
    /// An interruption stops the engine; it starts again when it ends. Voices
    /// due between are not played (D8).
    private func observeInterruptions() {
        guard !observing else { return }
        observing = true
        NotificationCenter.default.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] note in
            guard let self, let handle = self.handle, let library = SoundLibrary.shared,
                  let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                  let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
            self.interrupted = type == .began
            if type == .began { library.engine(handle, 0) } else { try? AudioSession.activate(); library.engine(handle, 1) }
        }
    }
    #endif
}
