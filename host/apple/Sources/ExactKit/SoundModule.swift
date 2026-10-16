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
import UIKit
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


/// One session's output, opened at its first voice, never at boot: an app
/// that plays nothing never loads the arm or touches the audio session or
/// AURemoteIO, whose RPC timeout aborts the process. As a hand-built app
/// calls `engine.start()` on first use, the open runs on the main thread in
/// that voice's commit, so the voice starts as late as the open took (from
/// its first frame, D6). A failure is journaled and reported in `state`, its
/// voices are skipped, and the first voice `retryAfter` or more later tries
/// again.
final class SoundOutput {
    /// The arm and its log context, a +1 on `journal` released after the arm.
    private struct Arm { let handle: UnsafeMutableRawPointer; let context: UnsafeMutableRawPointer }
    /// What the arm's log lines reach: it outlives every arm made with it.
    private final class Journal { weak var output: SoundOutput? }

    private let log: (String) -> Void
    private let resolve: (String) -> String
    private let library: () -> SoundLibrary?
    private let journal = Journal()
    private var arm: Arm?
    /// Whether `arm`'s engine runs as far as this output knows: false after
    /// an interruption that did not resume it.
    private var started = false
    private var files: [String] = []
    private var interrupted = false
    /// The session must be activated again before the engine starts: an
    /// interruption ended without `shouldResume`, or its end never came.
    private var reactivate = false
    /// Why the last open failed, and when (host seconds), until one succeeds.
    private var failure: (reason: String, at: Double)?
    /// When the output last opened: one that stops again within `retryAfter`
    /// of it is a failure.
    private var opened: Double?
    /// Voices skipped because the output was not open.
    private var skipped: UInt64 = 0
    /// Seconds after a failure before a voice tries again; settable for tests.
    var retryAfter = 1.0
    /// The host clock, on CACurrentMediaTime's timebase; settable for tests.
    var now: () -> Double = CACurrentMediaTime

    convenience init(_ session: ExactSession) {
        self.init(log: { [weak session] in session?.log($0) },
                  resolve: { [weak session] in NodeView.resolveSource($0, app: session?.app)?.path ?? $0 })
    }

    init(log: @escaping (String) -> Void, resolve: @escaping (String) -> String, library: @escaping () -> SoundLibrary? = { SoundLibrary.shared }) {
        self.log = log
        self.resolve = resolve
        self.library = library
        journal.output = self
    }

    deinit { shutdown() }

    /// The session is destroyed: the arm is stopped and let go, and the
    /// session claim given back.
    func shutdown() {
        close()
        #if os(iOS) || os(tvOS)
        AudioSession.release(ObjectIdentifier(self))
        #endif
    }

    /// A batch's `sound` op: the files at a boot (the last boot's voices end
    /// now), then its ops in order.
    func apply(_ payload: [String: Any]) {
        guard !ExactEnv.agentMode else { return }
        if let files = payload["files"] as? [String] {
            if files != self.files { close(); self.files = files; failure = nil }
            else if let arm { library()?.op(arm.handle, 3, 0, 0, 0, 0) }
        }
        for op in payload["ops"] as? [[String: Any]] ?? [] {
            let id = UInt64((op["id"] as? NSNumber)?.int64Value ?? 0), at = (op["at"] as? NSNumber)?.doubleValue ?? 0
            // Runner time to host seconds on CACurrentMediaTime's timebase (D6).
            let host = ExactEnv.t0 + at / 1000
            if op["op"] as? String == "play" {
                if interrupted { say("sound skipped: interrupted"); continue }
                guard let arm = ready(), let library = library() else { skipped += 1; continue }
                library.op(arm.handle, 1, id, UInt32((op["sound"] as? NSNumber)?.intValue ?? 0), host, (op["gain"] as? NSNumber)?.floatValue ?? 1)
            } else if let arm, let library = library() {
                library.op(arm.handle, 2, id, 0, host, 0)
            }
        }
    }

    /// The running output, opened when there is none: at the first voice,
    /// after the system stopped the engine, or `retryAfter` after a failure.
    private func ready() -> Arm? {
        if let arm {
            if !started {
                // An interruption that did not resume: this voice resumes it.
                if let failed = start(arm) { fail(failed); close() } else { return arm }
            } else if counts(arm)[3] == 1 {
                return arm
            } else {
                close()
                // Stopped again within `retryAfter` of opening: a failure, so a route
                // that keeps stopping the engine costs one rebuild a second, not one a voice.
                if let opened, now() - opened < retryAfter {
                    fail("the output stopped again within \(String(format: "%g", retryAfter)) s of opening")
                } else {
                    say("sound: the output had stopped; opening it again")
                }
            }
        }
        if let failure, now() - failure.at < retryAfter { return nil }
        return open()
    }

    private func open() -> Arm? {
        guard let library = library() else { fail("no sound module in this build"); return nil }
        #if os(iOS) || os(tvOS)
        // The session before the engine: an output made with no active session is AURemoteIO's to fail.
        do { try AudioSession.hold(ObjectIdentifier(self)) } catch { fail("the audio session did not activate: \(error.localizedDescription)"); return nil }
        observe()
        #endif
        let c = files.map { strdup(resolve($0)) }
        defer { c.forEach { free($0) } }
        let context = Unmanaged.passRetained(journal).toOpaque()
        var reason = [CChar](repeating: 0, count: 512)
        let handle = c.map { UnsafePointer($0) }.withUnsafeBufferPointer { library.open($0.baseAddress, $0.count, context, { context, bytes, count in
            guard let context, let bytes else { return }
            let line = String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
            let journal = Unmanaged<Journal>.fromOpaque(context).takeUnretainedValue()
            DispatchQueue.main.async { journal.output?.say(line) }
        }, true, &reason, reason.count) }
        guard let handle else {
            Unmanaged<Journal>.fromOpaque(context).release()
            fail(String(cString: reason))
            return nil
        }
        let arm = Arm(handle: handle, context: context)
        self.arm = arm
        started = true
        reactivate = false
        failure = nil
        opened = now()
        // The device check (LLP 1096 §5), when a development run asks for it.
        if ExactEnv.environment["EXACT_SOUND_CHECK"] == "1" { _ = library.engine(handle, 2, nil, 0) }
        return arm
    }

    /// Starts `arm`'s engine again after an interruption, the session first
    /// when it must be activated again: nil, or why not.
    private func start(_ arm: Arm) -> String? {
        #if os(iOS) || os(tvOS)
        if reactivate { do { try AudioSession.hold(ObjectIdentifier(self)) } catch { return "the audio session did not activate again: \(error.localizedDescription)" } }
        #endif
        var reason = [CChar](repeating: 0, count: 512)
        guard library()?.engine(arm.handle, 1, &reason, reason.count) == true else { return String(cString: reason) }
        started = true
        reactivate = false
        return nil
    }

    /// Journaled and reported; the session claim is given back, so a
    /// `playback` app's failed output leaves other apps' audio playing.
    private func fail(_ reason: String) {
        failure = (reason, now())
        #if os(iOS) || os(tvOS)
        AudioSession.release(ObjectIdentifier(self))
        #endif
        say("sound: the output did not open: \(reason); the app plays on silent, and a sound \(String(format: "%g", retryAfter)) s later tries again")
    }

    private func close() {
        guard let arm else { return }
        self.arm = nil
        started = false
        library()?.close(arm.handle)
        Unmanaged<Journal>.fromOpaque(arm.context).release()
    }

    /// A line for the journal, and for stderr: a run outside the agent, the
    /// only one that plays, has no journal reader.
    private func say(_ line: String) {
        fputs(line + "\n", stderr)
        log(line)
    }

    /// The arm's live voices, dropped ops and plays, late starts, and whether its engine runs.
    private func counts(_ arm: Arm) -> [UInt64] {
        var c = [UInt64](repeating: 0, count: 4)
        library()?.counts(arm.handle, &c)
        return c
    }

    /// `state.sounds.output` outside the agent: `closed` until the first
    /// voice, `running` or `stopped` after it (`stopped` too while an
    /// interruption holds it); `failed`, with its `error`, after a failure.
    var state: [String: Any] {
        var state: [String: Any] = ["kind": "avaudioengine", "skipped": skipped]
        if let arm {
            let c = counts(arm)
            state.merge(["state": started && c[3] == 1 ? "running" : "stopped", "live": c[0], "dropped": c[1], "late": c[2], "decoded": "\(files.count)/\(files.count)"]) { $1 }
        } else if let failure {
            state.merge(["state": "failed", "error": failure.reason]) { $1 }
        } else {
            state["state"] = "closed"
        }
        return state
    }

    #if os(iOS) || os(tvOS)
    private var observing = false
    /// An interruption pauses the engine. When it ends with `shouldResume`
    /// the session is activated and the engine started again, and a failure
    /// fails the output as a first open's does. Without it (the person
    /// started other audio), or when the end never comes (the app was
    /// suspended; over when it is active again), the next voice does that.
    /// Voices due between are not played (D8). A media services reset
    /// invalidates the engine: it is disposed of, and the next voice opens a
    /// new one (Apple QA1749).
    private func observe() {
        guard !observing else { return }
        observing = true
        let center = NotificationCenter.default
        center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] note in
            guard let self, let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                  let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
            let options = AVAudioSession.InterruptionOptions(rawValue: note.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0)
            self.interruption(began: type == .began, resume: options.contains(.shouldResume))
        }
        center.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
            self?.becameActive()
        }
        center.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: .main) { [weak self] _ in
            self?.mediaServicesWereReset()
        }
    }

    /// Internal for tests.
    func interruption(began: Bool, resume: Bool) {
        interrupted = began
        guard let arm else { return }
        if began {
            if started { _ = library()?.engine(arm.handle, 0, nil, 0) }
            started = false
            return
        }
        reactivate = true
        if resume, let failed = start(arm) { fail(failed); close() }
    }

    /// Internal for tests: an interruption whose end never came is over.
    func becameActive() {
        guard interrupted else { return }
        interrupted = false
        reactivate = true
    }

    /// Internal for tests.
    func mediaServicesWereReset() {
        guard let arm else { return }
        say("sound: the media services were reset; the next sound opens the output again")
        _ = library()?.engine(arm.handle, 3, nil, 0)
        close()
    }
    #endif
}
