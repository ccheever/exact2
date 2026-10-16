// @ref LLP 1125 D6–D8. The MIDI arm: CoreMIDI on a thread of its own with
// its own run loop. The client is made there (`MIDIClientCreateWithBlock`),
// so its setup notifications arrive on that run loop and a cold MIDIServer's
// first call (up to about 600 ms) never blocks the main thread. Loaded on
// demand (MIDIModule.swift), never linked into ExactKit.
//
// - Ports are reported as JSON, `{"state":"granted","inputs":[…],
//   "outputs":[…]}`, each port's `id` its `kMIDIPropertyUniqueID`, stable
//   across replug where an endpoint ref is not. They are enumerated again on
//   each setup notification, on `exact_midi_refresh`, and on a 3 s watchdog
//   while an input is open or a message went out in the last 10 s (euxy saw
//   notifications missed on hardware).
// - A send is held on this thread and handed to CoreMIDI 50 ms before it is
//   due (`HOLD`), stamped with its host time, so a retraction can still take
//   it back; `MIDIFlushOutput` is never called, since it empties a whole
//   destination. A message handed out plays.
// - An input's MIDI 1.0 messages (UMP types 1 and 2) are passed on with the
//   packet's host time, stamped in CoreMIDI's read callback; Clock and Active
//   Sensing are dropped (they feed clock in, stage 4).
import CoreMIDI
import Foundation
import QuartzCore

/// A line of text for the host: a log line or the ports' JSON.
public typealias MIDIText = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void
/// One input message: the input's unique ID, its host time in seconds
/// (CACurrentMediaTime's timebase), its bytes. Called on CoreMIDI's thread.
public typealias MIDIInput = @convention(c) (UnsafeMutableRawPointer?, Int32, Double, UnsafePointer<UInt8>?, Int) -> Void

/// How long before its time a held message is handed to CoreMIDI, seconds.
private let HOLD = 0.050

private let timebase: mach_timebase_info_data_t = {
    var info = mach_timebase_info_data_t()
    mach_timebase_info(&info)
    return info
}()

/// Host seconds (CACurrentMediaTime's) as a `MIDITimeStamp`, mach ticks.
private func ticks(_ seconds: Double) -> MIDITimeStamp {
    seconds <= 0 ? 0 : MIDITimeStamp(seconds * 1e9 * Double(timebase.denom) / Double(timebase.numer))
}

/// A `MIDITimeStamp` as host seconds.
private func seconds(_ stamp: MIDITimeStamp) -> Double {
    Double(stamp) * Double(timebase.numer) / Double(timebase.denom) / 1e9
}

/// A MIDI 1.0 message's length from its status byte; 0 for none.
private func length(_ status: UInt8) -> Int {
    switch status {
    case 0x80...0xBF, 0xE0...0xEF, 0xF2: return 3
    case 0xC0...0xDF, 0xF1, 0xF3: return 2
    case 0xF6, 0xF8, 0xFA...0xFC, 0xFE, 0xFF: return 1
    default: return 0
    }
}

private final class MIDIArm {
    let context: UnsafeMutableRawPointer?
    let log: MIDIText?, ports: MIDIText?, input: MIDIInput?
    private var thread: Thread!
    private var loop: CFRunLoop!
    private let started = DispatchSemaphore(value: 0)
    private var client = MIDIClientRef()
    private var inPort = MIDIPortRef()
    private var outPort = MIDIPortRef()
    /// Messages held until 50 ms before they are due, by (at, id).
    private var held: [(at: Double, id: UInt64, uid: Int32, bytes: [UInt8])] = []
    private var timer: CFRunLoopTimer?
    private var watchdog: CFRunLoopTimer?
    /// The inputs to listen to, by unique ID, and the sources connected now.
    private var wanted: Set<Int32> = []
    private var connected: [Int32: MIDIEndpointRef] = [:]
    private var lastReport: String?
    private var lastSend = -Double.infinity
    private var closed = false

    init(context: UnsafeMutableRawPointer?, log: MIDIText?, ports: MIDIText?, input: MIDIInput?) {
        self.context = context; self.log = log; self.ports = ports; self.input = input
        thread = Thread { self.run() } // held until the run loop ends, after `close`
        thread.name = "exact.midi"
        thread.qualityOfService = .userInteractive
        thread.start()
        // Only the run loop's existence is waited for, never CoreMIDI.
        started.wait()
    }

    private func run() {
        loop = CFRunLoopGetCurrent()
        // A source keeps the run loop running with nothing else to do.
        var ctx = CFRunLoopSourceContext()
        let keep = CFRunLoopSourceCreate(nil, 0, &ctx)
        CFRunLoopAddSource(loop, keep, .defaultMode)
        started.signal()
        perform { self.setUp() }
        while !closed { CFRunLoopRunInMode(.defaultMode, 1e9, false) }
    }

    /// Runs `block` on the arm's thread, after everything asked before it.
    func perform(_ block: @escaping () -> Void) {
        CFRunLoopPerformBlock(loop, CFRunLoopMode.defaultMode.rawValue, block)
        CFRunLoopWakeUp(loop)
    }

    private func say(_ line: String) {
        var line = line
        line.withUTF8 { log?(context, $0.baseAddress, $0.count) }
    }

    private func setUp() {
        var status = MIDIClientCreateWithBlock("exact" as CFString, &client) { [weak self] note in
            switch note.pointee.messageID {
            case .msgSetupChanged, .msgObjectAdded, .msgObjectRemoved, .msgPropertyChanged: self?.enumerate()
            default: break
            }
        }
        guard status == noErr else { return fail("the MIDI client was not made (\(status))") }
        status = MIDIInputPortCreateWithProtocol(client, "exact in" as CFString, ._1_0, &inPort) { [weak self] list, source in
            self?.received(list, uid: Int32(truncatingIfNeeded: Int(bitPattern: source)))
        }
        guard status == noErr else { return fail("the MIDI input port was not made (\(status))") }
        status = MIDIOutputPortCreate(client, "exact out" as CFString, &outPort)
        guard status == noErr else { return fail("the MIDI output port was not made (\(status))") }
        watchdog = CFRunLoopTimerCreateWithHandler(nil, CFAbsoluteTimeGetCurrent() + 3, 3, 0, 0) { [weak self] _ in
            guard let self else { return }
            if !self.wanted.isEmpty || CACurrentMediaTime() - self.lastSend < 10 { self.enumerate() }
        }
        CFRunLoopAddTimer(loop, watchdog, .defaultMode)
        enumerate()
    }

    private func fail(_ why: String) {
        say("midi: \(why)")
        report("{\"state\":\"unavailable\",\"inputs\":[],\"outputs\":[]}")
    }

    private func report(_ json: String) {
        guard json != lastReport else { return }
        lastReport = json
        var json = json
        json.withUTF8 { ports?(context, $0.baseAddress, $0.count) }
    }

    private static func quoted(_ s: String) -> String {
        let data = try? JSONSerialization.data(withJSONObject: [s])
        return data.map { String(decoding: $0, as: UTF8.self).dropFirst().dropLast() }.map(String.init) ?? "\"\""
    }

    private static func text(_ object: MIDIObjectRef, _ key: CFString) -> String? {
        var value: Unmanaged<CFString>?
        guard MIDIObjectGetStringProperty(object, key, &value) == noErr, let value else { return nil }
        return value.takeRetainedValue() as String
    }

    private static func uniqueID(_ object: MIDIObjectRef) -> Int32? {
        var id: Int32 = 0
        return MIDIObjectGetIntegerProperty(object, kMIDIPropertyUniqueID, &id) == noErr ? id : nil
    }

    /// An endpoint's port record; its manufacturer is its own, else its device's.
    private static func port(_ endpoint: MIDIEndpointRef) -> String? {
        guard let id = uniqueID(endpoint) else { return nil }
        let name = text(endpoint, kMIDIPropertyDisplayName) ?? text(endpoint, kMIDIPropertyName) ?? ""
        var maker = text(endpoint, kMIDIPropertyManufacturer)
        if maker == nil {
            var entity = MIDIEntityRef(), device = MIDIDeviceRef()
            if MIDIEndpointGetEntity(endpoint, &entity) == noErr, MIDIEntityGetDevice(entity, &device) == noErr {
                maker = text(device, kMIDIPropertyManufacturer)
            }
        }
        return "{\"id\":\(quoted(String(id))),\"name\":\(quoted(name)),\"manufacturer\":\(quoted(maker ?? "")),\"state\":\"connected\"}"
    }

    /// The ports present now, reported when they changed; the wanted inputs
    /// connected again by unique ID (a replugged device has a new ref).
    func enumerate() {
        let inputs = (0..<MIDIGetNumberOfSources()).compactMap { Self.port(MIDIGetSource($0)) }
        let outputs = (0..<MIDIGetNumberOfDestinations()).compactMap { Self.port(MIDIGetDestination($0)) }
        connect()
        report("{\"state\":\"granted\",\"inputs\":[\(inputs.joined(separator: ","))],\"outputs\":[\(outputs.joined(separator: ","))]}")
    }

    /// The endpoint of unique ID `uid`, if present.
    private static func endpoint(_ uid: Int32, _ type: MIDIObjectType) -> MIDIEndpointRef? {
        var object = MIDIObjectRef(), found = MIDIObjectType.other
        guard MIDIObjectFindByUniqueID(uid, &object, &found) == noErr, found == type else { return nil }
        return object
    }

    func listen(_ uids: Set<Int32>) {
        wanted = uids
        connect()
    }

    /// Each wanted input connected (its unique ID the refCon), each other
    /// one let go.
    private func connect() {
        guard inPort != 0 else { return }
        for (uid, source) in connected where !wanted.contains(uid) || Self.endpoint(uid, .source) != source {
            MIDIPortDisconnectSource(inPort, source)
            connected[uid] = nil
        }
        for uid in wanted where connected[uid] == nil {
            guard let source = Self.endpoint(uid, .source) else { continue }
            if MIDIPortConnectSource(inPort, source, UnsafeMutableRawPointer(bitPattern: Int(uid))) == noErr { connected[uid] = source }
        }
    }

    /// An event list's MIDI 1.0 messages, passed on stamped (CoreMIDI's thread).
    private func received(_ list: UnsafePointer<MIDIEventList>, uid: Int32) {
        for packet in list.unsafeSequence() {
            let stamp = packet.pointee.timeStamp
            let at = seconds(stamp == 0 ? mach_absolute_time() : stamp)
            let words = Array(packet.words())
            var i = 0
            while i < words.count {
                let w = words[i], type = w >> 28
                // A UMP message is 1, 2 or 4 words by its type.
                i += [1, 1, 1, 2, 2, 4, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4][Int(type)]
                guard type == 1 || type == 2 else { continue }
                let status = UInt8((w >> 16) & 0xFF), n = length(status)
                guard n > 0, status != 0xF8, status != 0xFE else { continue }
                let bytes = [status, UInt8((w >> 8) & 0x7F), UInt8(w & 0x7F)].prefix(n)
                Array(bytes).withUnsafeBufferPointer { input?(context, uid, at, $0.baseAddress, $0.count) }
            }
        }
    }

    /// Hold a message until 50 ms before `at` (host seconds).
    func send(id: UInt64, uid: Int32, at: Double, bytes: [UInt8]) {
        let i = held.firstIndex { ($0.at, $0.id) > (at, id) } ?? held.count
        held.insert((at, id, uid, bytes), at: i)
        schedule()
    }

    func retract(id: UInt64) {
        held.removeAll { $0.id == id }
        schedule()
    }

    /// A new boot: what an earlier runner left held is forgotten.
    func forget() {
        held = []
        schedule()
    }

    /// Hand out what is due within the hold; the timer at the next one's.
    private func schedule() {
        if let timer { CFRunLoopTimerInvalidate(timer); self.timer = nil }
        let now = CACurrentMediaTime()
        while let first = held.first, first.at - HOLD <= now {
            held.removeFirst()
            hand(first.uid, first.at, first.bytes)
        }
        guard let next = held.first else { return }
        let fire = CFAbsoluteTimeGetCurrent() + (next.at - HOLD - now)
        let timer = CFRunLoopTimerCreateWithHandler(nil, fire, 0, 0, 0) { [weak self] _ in self?.schedule() }
        self.timer = timer
        CFRunLoopAddTimer(loop, timer, .defaultMode)
    }

    /// One message to CoreMIDI, stamped with its host time.
    private func hand(_ uid: Int32, _ at: Double, _ bytes: [UInt8]) {
        guard let destination = Self.endpoint(uid, .destination) else {
            return say("midi: output \(uid) is not connected; a message to it is dropped")
        }
        let n = length(bytes.first ?? 0)
        guard n > 0, n == bytes.count else { return }
        // MIDI 1.0 as UMP: a channel voice message is type 2, a system one type 1.
        let type: UInt32 = bytes[0] >= 0xF0 ? 0x1000_0000 : 0x2000_0000
        var word = type | UInt32(bytes[0]) << 16
        if n > 1 { word |= UInt32(bytes[1]) << 8 }
        if n > 2 { word |= UInt32(bytes[2]) }
        lastSend = CACurrentMediaTime()
        withUnsafeTemporaryAllocation(byteCount: 256, alignment: 8) { raw in
            let list = raw.baseAddress!.assumingMemoryBound(to: MIDIEventList.self)
            var packet = MIDIEventListInit(list, ._1_0)
            var w = word
            packet = MIDIEventListAdd(list, 256, packet, ticks(at), 1, &w)
            let status = MIDISendEventList(outPort, destination, list)
            if status != noErr { say("midi: a message to \(uid) was not sent (\(status))") }
        }
    }

    func close() {
        perform {
            if let timer = self.timer { CFRunLoopTimerInvalidate(timer) }
            if let watchdog = self.watchdog { CFRunLoopTimerInvalidate(watchdog) }
            self.held = []
            if self.client != 0 { MIDIClientDispose(self.client) }
            self.closed = true
            CFRunLoopStop(self.loop)
        }
    }
}

/// The arm, its client made on its own thread; the ports come to `ports`
/// as JSON once enumerated, input messages to `input`, log lines to `log`.
@_cdecl("exact_midi_open")
public func exact_midi_open(_ context: UnsafeMutableRawPointer?, _ log: MIDIText?, _ ports: MIDIText?, _ input: MIDIInput?) -> UnsafeMutableRawPointer? {
    Unmanaged.passRetained(MIDIArm(context: context, log: log, ports: ports, input: input)).toOpaque()
}

/// `kind` 1 holds `bytes` (one message) for output `uid` until 50 ms before
/// host second `at`; 2 takes held message `id` back; 3 forgets every held
/// message (a new boot).
@_cdecl("exact_midi_op")
public func exact_midi_op(_ handle: UnsafeMutableRawPointer?, _ kind: UInt32, _ id: UInt64, _ uid: Int32, _ at: Double, _ bytes: UnsafePointer<UInt8>?, _ count: Int) {
    guard let handle else { return }
    let arm = Unmanaged<MIDIArm>.fromOpaque(handle).takeUnretainedValue()
    let data = bytes.map { Array(UnsafeBufferPointer(start: $0, count: count)) } ?? []
    arm.perform {
        switch kind {
        case 1: arm.send(id: id, uid: uid, at: at, bytes: data)
        case 2: arm.retract(id: id)
        default: arm.forget()
        }
    }
}

/// Listen to exactly these inputs, by unique ID.
@_cdecl("exact_midi_inputs")
public func exact_midi_inputs(_ handle: UnsafeMutableRawPointer?, _ uids: UnsafePointer<Int32>?, _ count: Int) {
    guard let handle else { return }
    let arm = Unmanaged<MIDIArm>.fromOpaque(handle).takeUnretainedValue()
    let set = Set(uids.map { Array(UnsafeBufferPointer(start: $0, count: count)) } ?? [])
    arm.perform { arm.listen(set) }
}

/// Enumerate the ports again (the app came to the foreground).
@_cdecl("exact_midi_refresh")
public func exact_midi_refresh(_ handle: UnsafeMutableRawPointer?) {
    guard let handle else { return }
    let arm = Unmanaged<MIDIArm>.fromOpaque(handle).takeUnretainedValue()
    arm.perform { arm.enumerate() }
}

@_cdecl("exact_midi_close")
public func exact_midi_close(_ handle: UnsafeMutableRawPointer?) {
    guard let handle else { return }
    let arm = Unmanaged<MIDIArm>.fromOpaque(handle).takeRetainedValue()
    arm.close()
}
