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

/// One session's output, never opened at boot. On iOS and tvOS it opens at
/// the session's first voice: an app that plays nothing never touches the
/// audio session or AURemoteIO, whose RPC timeout aborts the process. On the
/// Mac, whose cold output costs 100 to 200 ms, it is made after first pixel
/// (`prepare`) and started at the first voice.
///
/// The main thread never waits on audio. Activating the session, making,
/// starting and restarting the arm run on `audio`, a serial queue, and finish
/// back on the main thread; plays and ends that come meanwhile are held, and
/// pushed once the output runs, but for a voice more than `overdue` past
/// its time, which is skipped. The main thread stays the mixer ring's one
/// producer. A failure is journaled and reported in `state`, its voices are
/// skipped, and the first voice `retryAfter` or more later tries again.
final class SoundOutput {
    /// A play (kind 1) or an end (2) held while the output opens or starts.
    private struct Op { let kind: UInt32; let id: UInt64; let sound: UInt32; let at: Double; let gain: Float }
    /// The arm and its log context, a +1 on `journal` released after the arm.
    private struct Arm { let handle: UnsafeMutableRawPointer; let context: UnsafeMutableRawPointer }
    /// What the arm's log lines reach: it outlives every arm made with it.
    private final class Journal { weak var output: SoundOutput? }

    private let log: (String) -> Void
    private let resolve: (String) -> String
    private let library: () -> SoundLibrary?
    let audio: DispatchQueue
    private let journal = Journal()
    private var arm: Arm?
    /// Whether `arm`'s engine has been started; a prepared one has not.
    private var started = false
    /// A make, start or restart is under way on `audio`; ops wait in `pending`.
    private var busy = false
    private var pending: [Op] = []
    /// Moved on when the output is let go: a job's result from before is released.
    private var epoch = 0
    private var files: [String] = []
    private var interrupted = false
    /// The session must be activated again before the engine starts: an
    /// interruption ended without `shouldResume`, or its end never came.
    private var reactivate = false
    /// When the output was last opened again after the system stopped it.
    private var reopened: Double?
    /// Why the last open failed, and when (host seconds), until one succeeds.
    private var failure: (reason: String, at: Double)?
    /// Voices skipped because the output was not open, or opened too late for them.
    private var skipped: UInt64 = 0
    /// Seconds after a failed open before a voice tries again; settable for tests.
    var retryAfter = 1.0
    /// Seconds a held voice may be past its time when the output runs and
    /// still play: enough for a cold open; a sequencer's older hits are
    /// skipped rather than sounded at once.
    var overdue = 0.5
    /// The host clock, on CACurrentMediaTime's timebase; settable for tests.
    var now: () -> Double = CACurrentMediaTime
    static let queue = DispatchQueue(label: "exact.sound", qos: .userInitiated)

    convenience init(_ session: ExactSession) {
        self.init(log: { [weak session] in session?.log($0) },
                  resolve: { [weak session] in NodeView.resolveSource($0, app: session?.app)?.path ?? $0 })
    }

    init(log: @escaping (String) -> Void, resolve: @escaping (String) -> String,
         library: @escaping () -> SoundLibrary? = { SoundLibrary.shared }, audio: DispatchQueue = SoundOutput.queue) {
        self.log = log
        self.resolve = resolve
        self.library = library
        self.audio = audio
        journal.output = self
    }

    deinit { if let arm { release(arm) } }

    /// A batch's `sound` op: the files at a boot (the last boot's voices end
    /// now), then its ops in order.
    func apply(_ payload: [String: Any]) {
        guard !ExactEnv.agentMode else { return }
        if let files = payload["files"] as? [String] {
            if files != self.files { close(); self.files = files; failure = nil }
            else { pending.removeAll(); if let arm { push(arm, Op(kind: 3, id: 0, sound: 0, at: 0, gain: 0)) } }
        }
        for op in payload["ops"] as? [[String: Any]] ?? [] {
            let id = UInt64((op["id"] as? NSNumber)?.int64Value ?? 0), at = (op["at"] as? NSNumber)?.doubleValue ?? 0
            // Runner time to host seconds on CACurrentMediaTime's timebase (D6).
            let host = ExactEnv.t0 + at / 1000
            if op["op"] as? String == "play" {
                if interrupted { say("sound skipped: interrupted"); continue }
                play(Op(kind: 1, id: id, sound: UInt32((op["sound"] as? NSNumber)?.intValue ?? 0), at: host, gain: (op["gain"] as? NSNumber)?.floatValue ?? 1))
            } else if busy {
                hold(Op(kind: 2, id: id, sound: 0, at: host, gain: 0))
            } else if let arm {
                push(arm, Op(kind: 2, id: id, sound: 0, at: host, gain: 0))
            }
        }
    }

    /// After first pixel, the Mac makes the output, so the first voice only
    /// starts it. Elsewhere nothing happens here. `mac` is settable for tests.
    func prepare(mac: Bool = SoundOutput.isMac) {
        guard mac, !ExactEnv.agentMode, arm == nil, !busy, failure == nil, !files.isEmpty else { return }
        open(start: false)
    }

    #if os(macOS)
    static let isMac = true
    #else
    static let isMac = false
    #endif

    /// A voice: pushed to a running output; otherwise held while one opens
    /// (at the first voice, after the system stopped the engine, or
    /// `retryAfter` after a failed open), or skipped.
    private func play(_ op: Op) {
        if busy { hold(op); return }
        if let arm {
            if !started { hold(op); start(arm, activating: reactivate); return }
            if counts(arm)[3] == 1 { push(arm, op); return }
            close()
            // Stopped again within `retryAfter` of the last reopening: a failure, so a route
            // change that keeps stopping the engine costs one rebuild a second, not one a voice.
            if let reopened, now() - reopened < retryAfter {
                fail("the output stopped again within \(String(format: "%g", retryAfter)) s of opening")
            } else {
                say("sound: the output had stopped; opening it again")
                reopened = now()
            }
        }
        if let failure, now() - failure.at < retryAfter { skipped += 1; return }
        hold(op)
        open(start: true)
    }

    /// Held for the output: as many as the mixer's ring takes.
    private func hold(_ op: Op) {
        if pending.count < 256 { pending.append(op) } else if op.kind == 1 { skipped += 1 }
    }

    /// Makes the arm on `audio`, its session first on iOS and tvOS.
    private func open(start: Bool) {
        busy = true
        let epoch = epoch, paths = files.map(resolve), library = library, journal = journal
        #if os(iOS) || os(tvOS)
        let (category, injected) = AudioSession.want(ObjectIdentifier(self))
        observe()
        #endif
        audio.async { [self] in
            var made: Arm?, reason = "no sound module in this build"
            #if os(iOS) || os(tvOS)
            // The session before the engine: an output made with no active session is AURemoteIO's to fail.
            do { if let injected { throw injected }; try AudioSession.activate(category) } catch {
                let why = "the audio session did not activate: \(error.localizedDescription)"
                DispatchQueue.main.async { self.made(nil, why, epoch: epoch, started: start) }
                return
            }
            #endif
            if let library = library() { (made, reason) = Self.make(library, paths, journal, start: start) }
            DispatchQueue.main.async { [made, reason] in self.made(made, reason, epoch: epoch, started: start) }
        }
    }

    /// The arm over `paths`, its engine started when `start`, or why not. On `audio`.
    private static func make(_ library: SoundLibrary, _ paths: [String], _ journal: Journal, start: Bool) -> (Arm?, String) {
        let c = paths.map { strdup($0) }
        defer { c.forEach { free($0) } }
        let context = Unmanaged.passRetained(journal).toOpaque()
        var reason = [CChar](repeating: 0, count: 512)
        let handle = c.map { UnsafePointer($0) }.withUnsafeBufferPointer { library.open($0.baseAddress, $0.count, context, { context, bytes, count in
            guard let context, let bytes else { return }
            let line = String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
            let journal = Unmanaged<Journal>.fromOpaque(context).takeUnretainedValue()
            DispatchQueue.main.async { journal.output?.say(line) }
        }, start, &reason, reason.count) }
        guard let handle else { Unmanaged<Journal>.fromOpaque(context).release(); return (nil, String(cString: reason)) }
        return (Arm(handle: handle, context: context), "")
    }

    private func made(_ made: Arm?, _ reason: String, epoch: Int, started: Bool) {
        guard epoch == self.epoch else { if let made { release(made) }; return }
        busy = false
        guard let made else { fail(reason); return }
        arm = made
        self.started = started
        failure = nil
        if started { run(made) } else if !pending.isEmpty { start(made) }
    }

    /// Starts `arm`'s engine on `audio`: a prepared one at its first voice,
    /// or one an interruption paused, with the session activated again
    /// when `activating`.
    private func start(_ arm: Arm, activating: Bool = false) {
        busy = true
        let epoch = epoch, library = library
        #if os(iOS) || os(tvOS)
        let category = AudioSession.category
        #endif
        audio.async { [self] in
            var reason = [CChar](repeating: 0, count: 512), why: String?
            #if os(iOS) || os(tvOS)
            if activating { do { try AudioSession.activate(category) } catch { why = "the audio session did not activate again: \(error.localizedDescription)" } }
            #endif
            if why == nil, library()?.engine(arm.handle, 1, &reason, reason.count) != true { why = String(cString: reason) }
            DispatchQueue.main.async { [why] in self.ran(why, epoch: epoch) }
        }
    }

    private func ran(_ failed: String?, epoch: Int) {
        guard epoch == self.epoch else { return }
        busy = false
        guard failed == nil, let arm else { fail(failed ?? "the output was let go"); close(); return }
        started = true
        reactivate = false
        failure = nil
        run(arm)
    }

    /// The output runs: the held ops are pushed, but for voices too far past
    /// their time. The rest are moved later together by the lateness of the
    /// earliest, so a held window keeps its offsets and lengths: a choked
    /// hit whose end had already passed would otherwise never sound.
    private func run(_ arm: Arm) {
        if ExactEnv.environment["EXACT_SOUND_CHECK"] == "1" { let library = library; audio.async { _ = library()?.engine(arm.handle, 2, nil, 0) } } // LLP 1096 §5
        let ready = now()
        let late = pending.filter { $0.kind == 1 && $0.at < ready - overdue }.count
        let kept = pending.filter { !($0.kind == 1 && $0.at < ready - overdue) }
        let shift = max(0, ready - (kept.filter { $0.kind == 1 }.map(\.at).min() ?? ready))
        for op in kept { push(arm, Op(kind: op.kind, id: op.id, sound: op.sound, at: op.at + shift, gain: op.gain)) }
        pending.removeAll()
        guard late > 0 else { return }
        skipped += UInt64(late)
        say(String(format: "sound skipped: %d held, more than %.0f ms past their time when the output started", late, overdue * 1000))
    }

    private func fail(_ reason: String) {
        failure = (reason, now())
        #if os(iOS) || os(tvOS)
        AudioSession.release(ObjectIdentifier(self)) // a playback app's others resume
        #endif
        skipped += UInt64(pending.filter { $0.kind == 1 }.count)
        pending.removeAll()
        say("sound: the output did not open: \(reason); the app plays on silent, and a sound \(String(format: "%g", retryAfter)) s later tries again")
    }

    /// Lets the output go: a job under way finds itself stale, and what it held is dropped.
    private func close() {
        epoch += 1
        busy = false
        pending.removeAll()
        guard let arm else { return }
        self.arm = nil
        started = false
        release(arm)
    }

    private func release(_ arm: Arm) {
        let library = library
        audio.async { library()?.close(arm.handle); Unmanaged<Journal>.fromOpaque(arm.context).release() }
    }

    private func push(_ arm: Arm, _ op: Op) { library()?.op(arm.handle, op.kind, op.id, op.sound, op.at, op.gain) }

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

    /// `state.sounds.output` outside the agent: `closed` until the output is
    /// made, `opening` (with `held` ops) while a job is under way, `ready`
    /// until the first voice starts it, then `running` or `stopped`;
    /// `failed`, with its `error`, after an open that failed.
    var state: [String: Any] {
        var state: [String: Any] = ["kind": "avaudioengine", "skipped": skipped]
        if busy {
            state.merge(["state": "opening", "held": pending.count]) { $1 }
        } else if let arm {
            let c = counts(arm)
            state.merge(["state": !started ? "ready" : c[3] == 1 ? "running" : "stopped", "live": c[0], "dropped": c[1], "late": c[2], "decoded": "\(files.count)/\(files.count)"]) { $1 }
        } else if let failure {
            state.merge(["state": "failed", "error": failure.reason]) { $1 }
        } else {
            state["state"] = "closed"
        }
        return state
    }

    #if os(iOS) || os(tvOS)
    private var observing = false
    /// An interruption pauses the engine; when it ends with `shouldResume`
    /// the session is activated and the engine started again, on `audio`,
    /// and a failure fails the output as a first open's does. Without it (the
    /// person started other audio), or when the end never comes (the app was
    /// suspended; cleared when it is active again), the next voice does that.
    /// Voices due between are not played (D8). A media services reset leaves
    /// the engine an orphan: it is let go without being messaged, and the
    /// next voice opens a new one.
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
            guard let self, self.arm != nil || self.busy else { return }
            self.say("sound: the media services were reset; the next sound opens the output again")
            if let arm = self.arm { let library = self.library; self.audio.async { _ = library()?.engine(arm.handle, 3, nil, 0) } }
            self.close()
        }
    }

    /// Internal for tests.
    func interruption(began: Bool, resume: Bool) {
        interrupted = began
        guard let arm, started else { return }
        if began { let library = library; audio.async { _ = library()?.engine(arm.handle, 0, nil, 0) }; return }
        if resume, !busy { start(arm, activating: true) } else { started = false; reactivate = true }
    }

    /// Internal for tests: an interruption whose end never came is over.
    func becameActive() {
        guard interrupted else { return }
        interrupted = false
        if arm != nil, started { started = false; reactivate = true }
    }
    #endif
}
