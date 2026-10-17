// @ref LLP 1125 D6–D8. The runner's MIDI queue, sent: a session's `midi`
// batch op carries `send`, `retract`, `open` and `request` in runner
// milliseconds, which become host seconds — runner time is
// `(CACurrentMediaTime() - t0) × 1000`, as for sound (SoundModule.swift) —
// for the MIDI arm (midiarm/, CoreMIDI), a dylib loaded on demand, never
// ExactKit's link graph. Until a request the state is `prompt` and no
// CoreMIDI client exists; the first request opens the arm, whose first
// enumeration reports `granted` and the ports. One arm serves the process:
// every session that asked hears its ports, an input goes to each session
// that opened it, and each session's message ids are its own. Timing Clock
// goes only to a session that follows its input (the open op's `clock`,
// LLP 1125 D9): ticks alone wait up to 50 ms from the oldest, and anything
// else is delivered at once with the ticks ahead of it. Nothing loads under
// the agent, whose clock is virtual: the runner answers a request itself
// there.
import Foundation
import QuartzCore
#if os(iOS)
import UIKit
#endif

typealias MIDITextCallback = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void
typealias MIDIInputCallback = @convention(c) (UnsafeMutableRawPointer?, Int32, Double, UnsafePointer<UInt8>?, Int) -> Void

/// The arm's five entry points: the dylib's, or a test's stand-in.
final class MIDILibrary {
    typealias Open = @convention(c) (UnsafeMutableRawPointer?, MIDITextCallback?, MIDITextCallback?, MIDIInputCallback?) -> UnsafeMutableRawPointer?
    typealias Op = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt64, Int32, Double, UnsafePointer<UInt8>?, Int) -> Void
    typealias Inputs = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<Int32>?, Int) -> Void
    typealias Handle = @convention(c) (UnsafeMutableRawPointer?) -> Void
    let open: Open, op: Op, inputs: Inputs, refresh: Handle, close: Handle
    init(open: Open, op: Op, inputs: Inputs, refresh: Handle, close: Handle) {
        self.open = open; self.op = op; self.inputs = inputs; self.refresh = refresh; self.close = close
    }
    /// The arm at `path`, or nil (its error on stderr).
    static func load(_ path: String) -> MIDILibrary? {
        guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            FileHandle.standardError.write(Data("exact midi: \(String(cString: dlerror()))\n".utf8)); return nil
        }
        func symbol<T>(_ name: String, _: T.Type) -> T? { dlsym(library, "exact_midi_" + name).map { unsafeBitCast($0, to: T.self) } }
        guard let open = symbol("open", Open.self), let op = symbol("op", Op.self), let inputs = symbol("inputs", Inputs.self),
              let refresh = symbol("refresh", Handle.self), let close = symbol("close", Handle.self) else { dlclose(library); return nil }
        return MIDILibrary(open: open, op: op, inputs: inputs, refresh: refresh, close: close)
    }
    static let shared: MIDILibrary? = {
        #if os(tvOS)
        return nil // no CoreMIDI on tvOS (LLP 1125 D6: `unavailable`)
        #elseif os(macOS)
        return load(Bundle.main.executableURL!.deletingLastPathComponent().path + "/libexact_midi.dylib")
        #else
        return load(embeddedModule(framework: "ExactMIDI", dylib: "libexact_midi.dylib"))
        #endif
    }()
}

/// The process's one arm, and the sessions it serves.
final class MIDIHub {
    private struct Heard { let uid: Int32; let at: Double; let bytes: [UInt8] }
    private let library: MIDILibrary
    private var handle: UnsafeMutableRawPointer?
    private var outputs: [Weak] = []
    private final class Weak { weak var output: MIDIOutput?; init(_ o: MIDIOutput) { output = o } }
    /// The last ports the arm reported, as `exactMIDI`'s JSON.
    private(set) var ports: String?
    /// Input messages from the arm's thread, drained on the main queue: at
    /// once for anything but Timing Clock (`draining`), else at most 50 ms
    /// after the oldest tick (`ticking`).
    private let lock = NSLock()
    private var heard: [Heard] = []
    private var draining = false
    private var ticking = false
    /// Drains so far: a tick's delayed drain armed before the last drain is
    /// stale, and leaves the ticks since to their own 50 ms.
    private var drains = 0
    /// The inputs some session follows the clock of (under `lock`).
    private var clockUIDs: Set<Int32> = []
    private var nextID: UInt64 = 0

    private static var hubs: [ObjectIdentifier: MIDIHub] = [:]
    /// The hub over `library`, made at the first request.
    static func shared(_ library: MIDILibrary) -> MIDIHub {
        if let hub = hubs[ObjectIdentifier(library)] { return hub }
        let hub = MIDIHub(library)
        hubs[ObjectIdentifier(library)] = hub
        return hub
    }

    private init(_ library: MIDILibrary) {
        self.library = library
        // The hub lives as long as the process: its context is never released.
        let context = Unmanaged.passRetained(self).toOpaque()
        handle = library.open(context, { context, bytes, count in
            guard let context, let bytes else { return }
            let line = String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
            let hub = Unmanaged<MIDIHub>.fromOpaque(context).takeUnretainedValue()
            DispatchQueue.main.async { hub.say(line) }
        }, { context, bytes, count in
            guard let context, let bytes else { return }
            let json = String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
            let hub = Unmanaged<MIDIHub>.fromOpaque(context).takeUnretainedValue()
            DispatchQueue.main.async { hub.reported(json) }
        }, { context, uid, at, bytes, count in
            guard let context, let bytes else { return }
            let hub = Unmanaged<MIDIHub>.fromOpaque(context).takeUnretainedValue()
            hub.heard(Heard(uid: uid, at: at, bytes: Array(UnsafeBufferPointer(start: bytes, count: count))))
        })
        #if os(iOS)
        // Ports change while the app is suspended; enumerate again on return.
        for name in [UIApplication.willEnterForegroundNotification, UIApplication.didBecomeActiveNotification] {
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in self?.refresh() }
        }
        #endif
    }

    func refresh() { if let handle { library.refresh(handle) } }

    /// A session that asked: it hears the ports, and the inputs it opened
    /// before it asked are listened to now.
    func join(_ output: MIDIOutput) {
        outputs.removeAll { $0.output == nil || $0.output === output }
        outputs.append(Weak(output))
        if let ports { output.reported(ports) }
        listen()
    }

    func leave(_ output: MIDIOutput) {
        outputs.removeAll { $0.output == nil || $0.output === output }
        listen()
    }

    private func say(_ line: String) { outputs.first?.output?.say(line) }

    private func reported(_ json: String) {
        ports = json
        for o in outputs { o.output?.reported(json) }
    }

    /// A process-wide id for one session's message.
    func id() -> UInt64 { nextID += 1; return nextID }

    func op(_ kind: UInt32, _ id: UInt64, _ uid: Int32, _ host: Double, _ bytes: [UInt8]) {
        guard let handle else { return }
        bytes.withUnsafeBufferPointer { library.op(handle, kind, id, uid, host, $0.baseAddress, $0.count) }
    }

    /// Every session's open inputs, together, and the inputs whose clock
    /// some session follows.
    func listen() {
        let sessions = outputs.compactMap(\.output)
        lock.withLock { clockUIDs = Set(sessions.flatMap(\.clock)) }
        guard let handle else { return }
        let uids = Array(Set(sessions.flatMap(\.open)))
        uids.withUnsafeBufferPointer { library.inputs(handle, $0.baseAddress, $0.count) }
    }

    /// One message from the arm's thread. A tick no session follows goes no
    /// further.
    private func heard(_ message: Heard) {
        let tick = message.bytes.first == 0xF8
        lock.lock()
        if tick && !clockUIDs.contains(message.uid) {
            lock.unlock()
            return
        }
        heard.append(message)
        let now = !tick && !draining, later = tick && !draining && !ticking
        draining = draining || now
        ticking = ticking || later
        let armed = drains
        lock.unlock()
        if now {
            DispatchQueue.main.async { self.drain() }
        } else if later {
            let wait = max(0, 0.05 - (CACurrentMediaTime() - message.at))
            DispatchQueue.main.asyncAfter(deadline: .now() + wait) { self.drain(armed: armed) }
        }
    }

    /// What arrived since the last drain: to each session that opened its
    /// input, one delivery, its ticks only to a session that follows them.
    /// A tick's drain (`armed`, the drains when it was armed) that another
    /// drain came before does nothing.
    private func drain(armed: Int? = nil) {
        lock.lock()
        if let armed, armed != drains {
            lock.unlock()
            return
        }
        let batch = heard
        heard = []
        (draining, ticking) = (false, false)
        drains += 1
        lock.unlock()
        for output in outputs.compactMap(\.output) {
            let mine = batch.filter { output.open.contains($0.uid) && ($0.bytes.first != 0xF8 || output.clock.contains($0.uid)) }
            if !mine.isEmpty { output.received(mine.map { ($0.uid, $0.at, $0.bytes) }) }
        }
    }
}

/// One session's MIDI: the runner's ops to the hub, the hub's reports to
/// the runner.
final class MIDIOutput {
    private let log: (String) -> Void
    private let deliver: (String) -> Void
    private let library: () -> MIDILibrary?
    private var hub: MIDIHub?
    /// This session's ids of messages still held, to the hub's.
    private var ids: [UInt64: UInt64] = [:]
    /// The inputs this session opened, by unique ID, and those of them a
    /// sequence follows the clock of.
    fileprivate(set) var open: Set<Int32> = []
    fileprivate(set) var clock: Set<Int32> = []
    private var toldAbsent = false

    convenience init(_ session: ExactSession) {
        self.init(log: { [weak session] in session?.log($0) },
                  deliver: { [weak session] json in
                      guard let session, session.booted, session.state != .destroyed else { return }
                      session.apply(session.runtime.midi(json))
                  })
    }

    init(log: @escaping (String) -> Void, deliver: @escaping (String) -> Void, library: @escaping () -> MIDILibrary? = { MIDILibrary.shared }) {
        self.log = log
        self.deliver = deliver
        self.library = library
    }

    deinit { hub?.leave(self) }

    /// The session is destroyed: its held messages are settled (a note-off
    /// or Stop goes now, the rest is let go) and its inputs closed.
    func shutdown() {
        for id in ids.values { hub?.op(3, id, 0, 0, []) }
        ids = [:]
        (open, clock) = ([], [])
        hub?.leave(self)
        hub = nil
    }

    /// A batch's `midi` op: a boot (what an earlier runner held is settled:
    /// its note-offs and Stop go now, the rest is let go), then its ops in
    /// order.
    func apply(_ payload: [String: Any]) {
        guard !ExactEnv.agentMode else { return }
        if payload["boot"] as? Bool == true {
            for id in ids.values { hub?.op(3, id, 0, 0, []) }
            ids = [:]
        }
        for op in payload["ops"] as? [[String: Any]] ?? [] {
            let id = UInt64((op["id"] as? NSNumber)?.int64Value ?? 0)
            switch op["op"] as? String {
            case "request":
                guard let library = library() else {
                    #if !os(tvOS)
                    if !toldAbsent { toldAbsent = true; say("midi: no MIDI module in this build") }
                    #endif
                    deliver("{\"ports\":{\"state\":\"unavailable\"}}")
                    continue
                }
                if hub == nil { hub = MIDIHub.shared(library); hub?.join(self) }
            case "send":
                guard let hub, let output = op["output"] as? String, let uid = Int32(output) else { continue }
                let at = (op["at"] as? NSNumber)?.doubleValue ?? 0
                let bytes = (op["data"] as? [NSNumber] ?? []).map { UInt8(truncatingIfNeeded: $0.intValue) }
                let global = hub.id()
                ids[id] = global
                // Runner time to host seconds on CACurrentMediaTime's timebase (D7).
                hub.op(1, global, uid, ExactEnv.t0 + at / 1000, bytes)
                // A handed message is never taken back: keep the newest half's ids.
                if ids.count > 4096 { let cut = ids.keys.sorted()[2048]; ids = ids.filter { $0.key >= cut } }
            case "retract":
                if let global = ids.removeValue(forKey: id) { hub?.op(2, global, 0, 0, []) }
            case "open":
                open = Set((op["inputs"] as? [String] ?? []).compactMap { Int32($0) })
                clock = Set((op["clock"] as? [String] ?? []).compactMap { Int32($0) })
                hub?.listen()
            default:
                continue
            }
        }
    }

    fileprivate func reported(_ ports: String) { deliver("{\"ports\":\(ports)}") }

    /// One delivery of input messages: host seconds to runner milliseconds.
    fileprivate func received(_ messages: [(uid: Int32, at: Double, bytes: [UInt8])]) {
        let items = messages.map { m in
            "[\"\(m.uid)\",\((m.at - ExactEnv.t0) * 1000),[\(m.bytes.map(String.init).joined(separator: ","))]]"
        }
        deliver("{\"messages\":[\(items.joined(separator: ","))]}")
    }

    /// A line for the journal, and for stderr: a run outside the agent has
    /// no journal reader.
    fileprivate func say(_ line: String) {
        fputs(line + "\n", stderr)
        log(line)
    }
}
