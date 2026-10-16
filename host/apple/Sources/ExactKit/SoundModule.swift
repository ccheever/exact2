// @ref LLP 1096 D6, D8. The runner's voice table, played: a session's `sound`
// batch op names the plan's files at a boot and carries `play` and `end` ops
// in runner milliseconds, which become host seconds — runner time is
// `(CACurrentMediaTime() - t0) × 1000` — for the sound arm's mixer. The arm
// (soundarm/, AVFAudio) is a dylib loaded on demand, never ExactKit's link
// graph; nothing loads or plays under the agent, whose clock is virtual (D10).
import Foundation
import QuartzCore
#if os(iOS) || os(tvOS)
import AVFoundation
#endif

typealias SoundLogCallback = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void

/// The arm's five entry points: the dylib's, or a test's stand-in.
final class SoundLibrary {
    typealias Open = @convention(c) (UnsafePointer<UnsafePointer<CChar>?>?, Int, UnsafeMutableRawPointer?, SoundLogCallback?, Bool, UnsafeMutablePointer<CChar>?, Int) -> UnsafeMutableRawPointer?
    typealias Op = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt64, UInt32, Double, Float) -> Void
    typealias Counts = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutablePointer<UInt64>?) -> Void
    typealias Engine = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafeMutablePointer<CChar>?, Int) -> Bool
    typealias Close = @convention(c) (UnsafeMutableRawPointer?) -> Void
    let open: Open, op: Op, counts: Counts, engine: Engine, close: Close
    init(open: Open, op: Op, counts: Counts, engine: Engine, close: Close) {
        self.open = open; self.op = op; self.counts = counts; self.engine = engine; self.close = close
    }
    private convenience init(_ library: UnsafeMutableRawPointer) {
        func symbol<T>(_ name: String, _: T.Type) -> T { unsafeBitCast(dlsym(library, name)!, to: T.self) }
        self.init(open: symbol("exact_sound_open", Open.self), op: symbol("exact_sound_op", Op.self),
                  counts: symbol("exact_sound_counts", Counts.self), engine: symbol("exact_sound_engine", Engine.self),
                  close: symbol("exact_sound_close", Close.self))
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

/// One session's output, never opened at boot. On iOS and tvOS it opens at
/// the session's first voice: an app that plays nothing never touches the
/// audio session or AURemoteIO, whose RPC timeout aborts the process. On the
/// Mac, whose cold output costs about 100 ms, it is made off the main thread
/// after first pixel (`prepare`) and started at the first voice.
///
/// A failed open is journaled and reported in `state`, its voices are
/// skipped, and the first voice `retryAfter` or more later tries again.
final class SoundOutput {
    private let log: (String) -> Void
    private let resolve: (String) -> String
    private let library: () -> SoundLibrary?
    private var handle: UnsafeMutableRawPointer?
    /// Whether `handle`'s engine has been started; a prepared one has not.
    private var started = false
    private var files: [String] = []
    private var interrupted = false
    /// Why the last open failed, and when (host seconds), until one succeeds.
    private var failure: (reason: String, at: Double)?
    /// Voices skipped because the output was not open.
    private var skipped: UInt64 = 0
    /// Seconds after a failed open before a voice tries again; settable for tests.
    var retryAfter = 1.0
    /// The clock `retryAfter` is read on; settable for tests.
    var now: () -> Double = CACurrentMediaTime
    /// Where `prepare` makes the output; `made` is touched only on it.
    private static let maker = DispatchQueue(label: "exact.sound.prepare")
    private var made: (handle: UnsafeMutableRawPointer?, reason: String)?
    private var making = false

    convenience init(_ session: ExactSession) {
        self.init(log: { [weak session] in session?.log($0) },
                  resolve: { [weak session] in NodeView.resolveSource($0, app: session?.app)?.path ?? $0 })
    }

    init(log: @escaping (String) -> Void, resolve: @escaping (String) -> String, library: @escaping () -> SoundLibrary? = { SoundLibrary.shared }) {
        self.log = log
        self.resolve = resolve
        self.library = library
    }

    deinit { close() }

    /// A batch's `sound` op: the files at a boot (the last boot's voices end
    /// now), then its ops in order.
    func apply(_ payload: [String: Any]) {
        guard !ExactEnv.agentMode else { return }
        if let files = payload["files"] as? [String] {
            if files != self.files { close(); self.files = files; failure = nil }
            else if let handle { library()?.op(handle, 3, 0, 0, 0, 0) }
        }
        for op in payload["ops"] as? [[String: Any]] ?? [] {
            let id = UInt64((op["id"] as? NSNumber)?.int64Value ?? 0), at = (op["at"] as? NSNumber)?.doubleValue ?? 0
            // Runner time to host seconds on CACurrentMediaTime's timebase (D6).
            let host = ExactEnv.t0 + at / 1000
            if op["op"] as? String == "play" {
                if interrupted { say("sound skipped: interrupted"); continue }
                guard let handle = ready(), let library = library() else { skipped += 1; continue }
                library.op(handle, 1, id, UInt32((op["sound"] as? NSNumber)?.intValue ?? 0), host, (op["gain"] as? NSNumber)?.floatValue ?? 1)
            } else if let handle, let library = library() {
                library.op(handle, 2, id, 0, host, 0)
            }
        }
    }

    /// After first pixel, the Mac makes the output off the main thread, so
    /// the first voice only starts it. Elsewhere nothing happens here.
    /// `mac` is settable for tests.
    func prepare(mac: Bool = SoundOutput.isMac) {
        guard mac, !ExactEnv.agentMode, handle == nil, !making, failure == nil, !files.isEmpty, let library = library() else { return }
        making = true
        let paths = files.map(resolve)
        Self.maker.async { [self] in
            made = make(library, paths, start: false)
            DispatchQueue.main.async { self.takeMade() }
        }
    }

    #if os(macOS)
    static let isMac = true
    #else
    static let isMac = false
    #endif

    /// The running output, opened when there is none: at the first voice,
    /// after the system stopped the engine (a route or format change, a
    /// media services reset), and `retryAfter` after a failed open.
    private func ready() -> UnsafeMutableRawPointer? {
        takeMade()
        if let handle {
            if !started {
                started = true
                var reason = [CChar](repeating: 0, count: 512)
                if library()?.engine(handle, 1, &reason, reason.count) == true { check(handle); return handle }
                close()
                fail(String(cString: reason))
                return nil
            }
            if counts(handle)[3] == 1 { return handle }
            say("sound: the output had stopped; opening it again")
            close()
        }
        if let failure, now() - failure.at < retryAfter { return nil }
        open()
        return handle
    }

    private func open() {
        guard let library = library() else { fail("no sound module in this build"); return }
        #if os(iOS) || os(tvOS)
        // The session before the engine: an output made with no active session is AURemoteIO's to fail.
        do { try AudioSession.activate() } catch { fail("the audio session did not activate: \(error.localizedDescription)"); return }
        observe()
        #endif
        let (handle, reason) = make(library, files.map(resolve), start: true)
        guard let handle else { fail(reason); return }
        self.handle = handle
        started = true
        check(handle)
    }

    /// The arm over `paths`, its engine started when `start`, or why not. Any thread.
    private func make(_ library: SoundLibrary, _ paths: [String], start: Bool) -> (UnsafeMutableRawPointer?, String) {
        let c = paths.map { strdup($0) }
        defer { c.forEach { free($0) } }
        let context = Unmanaged.passUnretained(self).toOpaque()
        var reason = [CChar](repeating: 0, count: 512)
        let handle = c.map { UnsafePointer($0) }.withUnsafeBufferPointer { library.open($0.baseAddress, $0.count, context, { context, bytes, count in
            guard let context, let bytes else { return }
            let line = String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
            let output = Unmanaged<SoundOutput>.fromOpaque(context).takeUnretainedValue()
            DispatchQueue.main.async { output.say(line) }
        }, start, &reason, reason.count) }
        return (handle, String(cString: reason))
    }

    /// The output `prepare` made, waited for if it is still being made.
    private func takeMade() {
        guard making else { return }
        making = false
        guard let made = Self.maker.sync(execute: { defer { self.made = nil }; return self.made }) else { return }
        if let handle = made.handle { self.handle = handle; started = false } else { fail(made.reason) }
    }

    /// The device check (LLP 1096 §5), when a development run asks for it.
    private func check(_ handle: UnsafeMutableRawPointer) {
        failure = nil
        if ExactEnv.environment["EXACT_SOUND_CHECK"] == "1" { _ = library()?.engine(handle, 2, nil, 0) }
    }

    private func fail(_ reason: String) {
        failure = (reason, now())
        say("sound: the output did not open: \(reason); the app plays on silent, and a sound \(String(format: "%g", retryAfter)) s later tries again")
    }

    private func close() {
        takeMade()
        guard let handle else { return }
        self.handle = nil
        library()?.close(handle)
    }

    /// A line for the journal, and for stderr: a run outside the agent, the
    /// only one that plays, has no journal reader.
    private func say(_ line: String) {
        fputs(line + "\n", stderr)
        log(line)
    }

    /// The arm's live voices, dropped ops and plays, late starts, and whether its engine runs.
    private func counts(_ handle: UnsafeMutableRawPointer) -> [UInt64] {
        var c = [UInt64](repeating: 0, count: 4)
        library()?.counts(handle, &c)
        return c
    }

    /// `state.sounds.output` outside the agent: `closed` until the output is
    /// made, `preparing` while the Mac makes it, `ready` until the first
    /// voice starts it, then `running` or `stopped`; `failed`, with its
    /// `error`, after an open that failed.
    var state: [String: Any] {
        var state: [String: Any] = ["kind": "avaudioengine", "skipped": skipped]
        if let handle {
            let c = counts(handle)
            state.merge(["state": !started ? "ready" : c[3] == 1 ? "running" : "stopped", "live": c[0], "dropped": c[1], "late": c[2], "decoded": "\(files.count)/\(files.count)"]) { $1 }
        } else if making {
            state["state"] = "preparing"
        } else if let failure {
            state.merge(["state": "failed", "error": failure.reason]) { $1 }
        } else {
            state["state"] = "closed"
        }
        return state
    }

    #if os(iOS) || os(tvOS)
    private var observing = false
    /// An interruption stops the engine; it starts again when it ends. Voices
    /// due between are not played (D8). A media services reset leaves the
    /// engine an orphan: it is let go, and the next voice opens a new one.
    private func observe() {
        guard !observing else { return }
        observing = true
        let center = NotificationCenter.default
        center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] note in
            guard let self, let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                  let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
            self.interrupted = type == .began
            guard let handle = self.handle, let library = self.library() else { return }
            if type == .began { _ = library.engine(handle, 0, nil, 0); return }
            try? AudioSession.activate()
            var reason = [CChar](repeating: 0, count: 512)
            // One that does not start is found stopped, and opened again, by the next voice.
            if !library.engine(handle, 1, &reason, reason.count) { self.say("sound: the output did not start again after an interruption: \(String(cString: reason))") }
        }
        center.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: .main) { [weak self] _ in
            guard let self, self.handle != nil else { return }
            self.say("sound: the media services were reset; the next sound opens the output again")
            self.close()
        }
    }
    #endif
}
