// @ref LLP 1125 D6–D8. The MIDI arm (midiarm/MIDIArm.swift, the dylib
// build.mjs names in EXACT_MIDI_DYLIB) looped back through virtual
// endpoints this test makes in its own CoreMIDI client: a send arrives at
// its destination on its time, never more than the hold early; a retraction
// before the hold sends nothing; an input is stamped with its packet's time
// and its bytes come through MIDIOutput as the runner's JSON; a replugged
// endpoint keeps its unique-ID id; inputs opened before the request are heard
// once it is granted; a new runner's boot hands a held note-off at once, never
// ahead of its handed note-on, and lets the rest go; a controller flood loses
// no note-off; a followed input's clock is batched and its Start goes at
// once (LLP 1125 D9). macOS only: a simulator has no MIDIServer.
#if os(macOS)
import CoreMIDI
import QuartzCore
import XCTest
@testable import ExactKit

final class MIDIMacTests: XCTestCase {
    private var client = MIDIClientRef()
    private var destination = MIDIEndpointRef()
    private var source = MIDIEndpointRef()
    private let lock = NSLock()
    private var arrived: [(at: Double, status: UInt8)] = []

    private func library() throws -> MIDILibrary {
        guard let path = ProcessInfo.processInfo.environment["EXACT_MIDI_DYLIB"], let library = MIDILibrary.load(path) else {
            throw missing("module", "the MIDI arm is not named to this test runner (EXACT_MIDI_DYLIB)")
        }
        return library
    }

    override func setUpWithError() throws {
        let status = MIDIClientCreateWithBlock("exact test" as CFString, &client, nil)
        // No MIDI server here (a simulator), or one too loaded to answer (-304 at load 100+): skipped, saying which.
        guard status == noErr else { throw XCTSkip("CoreMIDI did not make a client here (\(status)): no MIDI server in this environment, or it did not answer (-304 seen at load 100+)") }
        XCTAssertEqual(MIDIDestinationCreateWithProtocol(client, "exact test in" as CFString, ._1_0, &destination) { [weak self] list, _ in
            let now = CACurrentMediaTime()
            for packet in list.unsafeSequence() {
                for word in packet.words() { self?.lock.withLock { self?.arrived.append((now, UInt8((word >> 16) & 0xFF))) } }
            }
        }, noErr)
        XCTAssertEqual(MIDISourceCreateWithProtocol(client, "exact test out" as CFString, ._1_0, &source), noErr)
    }

    override func tearDown() {
        MIDIClientDispose(client)
    }

    private func uid(_ endpoint: MIDIEndpointRef) -> Int32 {
        var id: Int32 = 0
        MIDIObjectGetIntegerProperty(endpoint, kMIDIPropertyUniqueID, &id)
        return id
    }

    /// `seconds` on this thread, on a strict timer: `Thread.sleep` is coalesced
    /// where the system coalesces timers (a session without a console: 20 ms
    /// slept 100 to 150 ms), and 48 ticks took past the test's wait.
    private func pause(_ seconds: Double) {
        let done = DispatchSemaphore(value: 0)
        let timer = DispatchSource.makeTimerSource(flags: .strict, queue: .global(qos: .userInteractive))
        timer.schedule(deadline: .now() + seconds, leeway: .nanoseconds(0))
        timer.setEventHandler { done.signal() }
        timer.resume()
        done.wait()
        timer.cancel()
    }

    /// How late the main queue runs a strict timer's handler: each 10 ms
    /// beat's lateness, by its deadline. A loaded machine delays every handler
    /// on main, a batch's drain among them; a bound adds only the lag seen
    /// while that batch waited. Its own wakeups can run a coalesced drain on
    /// time, so the lone-tick test measures with it only around its wait.
    private final class MainLag {
        private let timer: DispatchSourceTimer
        private var due: Double
        private let lock = NSLock()
        /// Each beat's deadline and how late it ran.
        private var beats: [(due: Double, late: Double)] = []
        /// On `queue`: main's by default; a user-interactive one's stands for
        /// the threads a send passes (the arm's, CoreMIDI's) on a busy machine.
        init(queue: DispatchQueue = .main) {
            timer = DispatchSource.makeTimerSource(flags: .strict, queue: queue)
            due = CACurrentMediaTime() + 0.010
            timer.schedule(deadline: .now() + 0.010, repeating: 0.010, leeway: .nanoseconds(0))
            timer.setEventHandler { [weak self] in
                guard let self else { return }
                let now = CACurrentMediaTime()
                self.lock.withLock { self.beats.append((self.due, now - self.due)) }
                // The expirations this handler stands for keep the timer's phase.
                self.due += Double(max(1, self.timer.data)) * 0.010
            }
            timer.resume()
        }
        func stop() { timer.cancel() }
        /// The most a beat due in `from...to` came late: a beat due while a
        /// batch waited runs late, after the batch, when main is as busy.
        func most(_ from: Double, _ to: Double) -> Double {
            lock.withLock { beats.filter { (from...to).contains($0.due) }.map(\.late).max() ?? 0 }
        }
        var most: Double { lock.withLock { beats.map(\.late).max() ?? 0 } }
    }

    /// The 1-minute load against twice the cores: past it (load 100 on 18 cores) no
    /// probe sees every delay a send or a batch meets, and timing says nothing.
    private func machineBusy() -> (busy: Bool, load: Double) {
        var load = [0.0, 0, 0]
        guard getloadavg(&load, 3) > 0 else { return (true, .nan) } // no evidence of a quiet machine
        return (load[0] > Double(2 * ProcessInfo.processInfo.activeProcessorCount), load[0])
    }

    private func wait(_ seconds: Double, until done: () -> Bool = { false }) {
        let end = Date(timeIntervalSinceNow: seconds)
        while Date() < end, !done() { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.005)) }
    }

    func testASendArrivesOnItsTimeAndARetractionSendsNothing() throws {
        let library = try library()
        var delivered: [String] = []
        let output = MIDIOutput(log: { _ in }, deliver: { delivered.append($0) }, library: { library })
        // Before a request nothing is open (ruling 4): no ports are reported.
        output.apply(["ops": [["op": "open", "inputs": []]]])
        wait(0.1)
        XCTAssertTrue(delivered.isEmpty)
        output.apply(["ops": [["op": "request"]]])
        wait(2) { delivered.contains { $0.contains("\(self.uid(self.destination))") } }
        let ports = try XCTUnwrap(delivered.last)
        XCTAssertTrue(ports.hasPrefix("{\"ports\":{\"state\":\"granted\""), ports)
        XCTAssertTrue(ports.contains("\"name\":\"exact test in\""), ports)
        // Runner ms: 200 ms from now, and a note-off taken back before its hold.
        var now = (CACurrentMediaTime() - ExactEnv.t0) * 1000
        let out = String(uid(destination))
        output.apply(["ops": [["op": "send", "id": 1, "output": out, "data": [0x90, 60, 100], "at": now + 200],
                              ["op": "send", "id": 2, "output": out, "data": [0x80, 60, 0], "at": now + 400],
                              ["op": "retract", "id": 2]]])
        // Timed only while a strict beat on a user-interactive queue (the arm's
        // thread and CoreMIDI's stand-in) stays within 20 ms of its time until
        // the note came: a busy machine delays the hand-off itself (75 ms seen
        // at load 117) and says nothing either way. The 20 ms ceiling is never
        // widened: a coalesced hold timer is 90-100 ms late.
        var timed = false, worst = 0.0
        for attempt in 0..<5 where !timed {
            let busy = MainLag(queue: DispatchQueue(label: "send lag", qos: .userInteractive))
            if attempt > 0 {
                lock.withLock { arrived.removeAll() }
                now = (CACurrentMediaTime() - ExactEnv.t0) * 1000
                output.apply(["ops": [["op": "send", "id": UInt64(10 + attempt), "output": out, "data": [0x90, 60, 100], "at": now + 200]]])
            }
            wait(0.6)
            busy.stop()
            let got = lock.withLock { arrived }
            if attempt == 0 { XCTAssertEqual(got.map(\.status), [0x90], "the retracted note-off never went out") }
            guard let first = got.first else { XCTFail("no note came"); break }
            let due = ExactEnv.t0 + (now + 200) / 1000, lag = busy.most(due - 0.250, first.at)
            worst = max(worst, lag)
            guard lag <= 0.020, !machineBusy().busy else { continue }
            timed = true
            let late = (first.at - due) * 1000
            XCTAssertGreaterThan(late, -1, "not early")
            // Within the jitter a qualified attempt can still have (delivery on
            // CoreMIDI's threads, a 10 ms probe's blind spots); a hand-off after
            // the 50 ms hold is 90-100 ms late.
            XCTAssertLessThan(late, 20, "on time")
        }
        output.shutdown()
        print("MIDIMacTests: send timed \(timed) (user-interactive lag up to \(worst) s)")
        if !timed { throw XCTSkip("a user-interactive thread lagged past 20 ms or the load passed twice the cores in every attempt (lag \(worst) s, load \(machineBusy().load)): too loaded to time a send") }
    }

    func testAnInputIsStampedAndAReplugKeepsItsID() throws {
        let library = try library()
        var delivered: [String] = []
        let output = MIDIOutput(log: { _ in }, deliver: { delivered.append($0) }, library: { library })
        output.apply(["ops": [["op": "request"]]])
        let input = String(uid(source))
        wait(2) { delivered.contains { $0.contains(input) } }
        output.apply(["ops": [["op": "open", "inputs": [input]]]])
        wait(0.2)
        let sent = CACurrentMediaTime() + 0.05
        var buffer = [UInt8](repeating: 0, count: 256)
        buffer.withUnsafeMutableBytes { raw in
            let list = raw.baseAddress!.assumingMemoryBound(to: MIDIEventList.self)
            var word: UInt32 = 0x2090_3C64
            var packet = MIDIEventListInit(list, ._1_0)
            packet = MIDIEventListAdd(list, 256, packet, 0, 1, &word)
            // Stamped with the packet's own time: 50 ms ahead.
            list.pointee.packet.timeStamp = MIDITimeStamp(sent * 1e9 * Double(Self.timebase.denom) / Double(Self.timebase.numer))
            XCTAssertEqual(MIDIReceivedEventList(source, list), noErr)
        }
        wait(1) { delivered.last?.hasPrefix("{\"messages\"") == true }
        let message = try XCTUnwrap(delivered.last)
        XCTAssertTrue(message.hasPrefix("{\"messages\":[[\"\(input)\","), message)
        XCTAssertTrue(message.hasSuffix(",[144,60,100]]]}"), message)
        let stamp = try XCTUnwrap(Double(message.split(separator: ",")[1]))
        XCTAssertEqual(stamp, (sent - ExactEnv.t0) * 1000, accuracy: 0.01)
        // Replug: the endpoint goes and comes back with the same unique ID.
        let id = uid(source)
        MIDIEndpointDispose(source)
        wait(0.5) { delivered.last?.contains(input) == false }
        XCTAssertFalse(try XCTUnwrap(delivered.last).contains(input), "a pulled port is no longer reported")
        XCTAssertEqual(MIDISourceCreateWithProtocol(client, "exact test out" as CFString, ._1_0, &source), noErr)
        MIDIObjectSetIntegerProperty(source, kMIDIPropertyUniqueID, id)
        wait(1) { delivered.last?.contains(input) == true }
        XCTAssertTrue(try XCTUnwrap(delivered.last).contains("\"id\":\"\(input)\""))
        output.shutdown()
    }

    /// One note-on from the test's virtual source, stamped now.
    private func play() {
        var buffer = [UInt8](repeating: 0, count: 256)
        buffer.withUnsafeMutableBytes { raw in
            let list = raw.baseAddress!.assumingMemoryBound(to: MIDIEventList.self)
            var word: UInt32 = 0x2090_3C64
            var packet = MIDIEventListInit(list, ._1_0)
            packet = MIDIEventListAdd(list, 256, packet, 0, 1, &word)
            XCTAssertEqual(MIDIReceivedEventList(source, list), noErr)
        }
    }

    /// Review round 1, A5: inputs a mounted listener opened before the
    /// request are listened to once the session joins.
    func testInputsOpenedBeforeTheRequestAreHeard() throws {
        let library = try library()
        var delivered: [String] = []
        let output = MIDIOutput(log: { _ in }, deliver: { delivered.append($0) }, library: { library })
        let input = String(uid(source))
        output.apply(["ops": [["op": "open", "inputs": [input]], ["op": "request"]]])
        wait(2) { delivered.contains { $0.contains(input) } }
        wait(0.2)
        play()
        wait(1) { delivered.last?.hasPrefix("{\"messages\"") == true }
        XCTAssertTrue(try XCTUnwrap(delivered.last).hasPrefix("{\"messages\":[[\"\(input)\","))
        output.shutdown()
    }

    /// Review round 1, A1: a new runner's boot settles what the old one left
    /// held: its note-off goes now, its note-on is let go.
    func testABootHandsAHeldNoteOffNowAndLetsTheRestGo() throws {
        let library = try library()
        var delivered: [String] = []
        let output = MIDIOutput(log: { _ in }, deliver: { delivered.append($0) }, library: { library })
        output.apply(["ops": [["op": "request"]]])
        wait(2) { delivered.contains { $0.contains("\(self.uid(self.destination))") } }
        let now = (CACurrentMediaTime() - ExactEnv.t0) * 1000, out = String(uid(destination))
        output.apply(["ops": [["op": "send", "id": 1, "output": out, "data": [0x90, 61, 100], "at": now + 2000],
                              ["op": "send", "id": 2, "output": out, "data": [0x80, 60, 0], "at": now + 2000]]])
        output.apply(["boot": true, "ops": []])
        // Until the note-off came (a loaded machine delays it), then long
        // enough that a note-on let go would have; 1 s at most, well before
        // the held messages' own 2 s, which a boot that kept them would send.
        wait(1) { self.lock.withLock { !self.arrived.isEmpty } }
        wait(0.3)
        XCTAssertEqual(lock.withLock { arrived }.map(\.status), [0x80], "the note-off at once, the note-on never")
        output.shutdown()
    }

    /// Review round 2: a note-off a boot sends is stamped no earlier than its
    /// note-on, which CoreMIDI may still hold: it never reaches the synth first.
    func testABootNeverSendsANoteOffAheadOfItsNoteOn() throws {
        let library = try library()
        var delivered: [String] = []
        let output = MIDIOutput(log: { _ in }, deliver: { delivered.append($0) }, library: { library })
        output.apply(["ops": [["op": "request"]]])
        wait(2) { delivered.contains { $0.contains("\(self.uid(self.destination))") } }
        let now = (CACurrentMediaTime() - ExactEnv.t0) * 1000, out = String(uid(destination))
        // Within the hold: handed at once, stamped 40 ms ahead; its note-off held.
        output.apply(["ops": [["op": "send", "id": 1, "output": out, "data": [0x90, 62, 100], "at": now + 40],
                              ["op": "send", "id": 2, "output": out, "data": [0x80, 62, 0], "at": now + 2000]]])
        wait(0.01)
        output.apply(["boot": true, "ops": []])
        wait(1) { self.lock.withLock { self.arrived.count >= 2 } }
        wait(0.1)
        let got = lock.withLock { arrived }
        XCTAssertEqual(got.map(\.status), [0x90, 0x80], "the note-on, then its note-off")
        if got.count == 2 { XCTAssertGreaterThanOrEqual(got[1].at, got[0].at) }
        output.shutdown()
    }

    /// Review round 3: a boot stamps a note-off no earlier than the latest
    /// note-on of its note, whichever was handed last.
    func testABootStampsANoteOffAfterTheLatestNoteOnOfItsNote() throws {
        let library = try library()
        var delivered: [String] = []
        let output = MIDIOutput(log: { _ in }, deliver: { delivered.append($0) }, library: { library })
        output.apply(["ops": [["op": "request"]]])
        wait(2) { delivered.contains { $0.contains("\(self.uid(self.destination))") } }
        let now = (CACurrentMediaTime() - ExactEnv.t0) * 1000, out = String(uid(destination))
        output.apply(["ops": [["op": "send", "id": 1, "output": out, "data": [0x90, 63, 100], "at": now + 40],
                              ["op": "send", "id": 2, "output": out, "data": [0x80, 63, 0], "at": now + 2000]]])
        output.apply(["ops": [["op": "send", "id": 3, "output": out, "data": [0x90, 63, 100], "at": now],
                              ["op": "send", "id": 4, "output": out, "data": [0x80, 63, 0], "at": now + 2000]]])
        wait(0.01)
        output.apply(["boot": true, "ops": []])
        // Until all four came: a loaded machine delays them (two of four by
        // 300 ms at load 80); their order and stamps are what is checked.
        wait(1) { self.lock.withLock { self.arrived.count >= 4 } }
        wait(0.1)
        let got = lock.withLock { arrived }
        XCTAssertEqual(got.map(\.status), [0x90, 0x90, 0x80, 0x80])
        if got.count == 4 { XCTAssertGreaterThanOrEqual(got[2].at, got[1].at, "both offs after the later note-on") }
        output.shutdown()
    }

    /// Review round 3: a note-on the runner takes back after the arm handed
    /// it (the runner's clock behind the arm's) is ended after it.
    func testANoteOnRetractedAfterItWasHandedIsEndedAfterIt() throws {
        let library = try library()
        var delivered: [String] = []
        let output = MIDIOutput(log: { _ in }, deliver: { delivered.append($0) }, library: { library })
        output.apply(["ops": [["op": "request"]]])
        wait(2) { delivered.contains { $0.contains("\(self.uid(self.destination))") } }
        let now = (CACurrentMediaTime() - ExactEnv.t0) * 1000, out = String(uid(destination))
        output.apply(["ops": [["op": "send", "id": 1, "output": out, "data": [0x92, 64, 100], "at": now + 40],
                              ["op": "send", "id": 2, "output": out, "data": [0x82, 64, 0], "at": now + 500]]])
        wait(0.01)
        output.apply(["ops": [["op": "retract", "id": 1], ["op": "retract", "id": 2]]])
        wait(0.8)
        let got = lock.withLock { arrived }
        XCTAssertEqual(got.map(\.status), [0x92, 0x82], "the note-on, then an off for it")
        if got.count == 2 { XCTAssertGreaterThanOrEqual(got[1].at, got[0].at) }
        output.shutdown()
    }

    /// Review round 2: a controller flood the arm cannot pass on at once
    /// loses its own messages, never the note-off behind it (D8).
    func testAControllerFloodLosesNoNoteOff() throws {
        let library = try library()
        var delivered: [String] = []
        let output = MIDIOutput(log: { _ in }, deliver: { delivered.append($0) }, library: { library })
        let input = String(uid(source))
        output.apply(["ops": [["op": "open", "inputs": [input]], ["op": "request"]]])
        wait(2) { delivered.contains { $0.contains(input) } }
        wait(0.2)
        // 5,000 Control Changes and a note-off in one event list, one callback's worth.
        var buffer = [UInt8](repeating: 0, count: 65536)
        buffer.withUnsafeMutableBytes { raw in
            let list = raw.baseAddress!.assumingMemoryBound(to: MIDIEventList.self)
            var packet = MIDIEventListInit(list, ._1_0)
            var words = (0..<5000).map { UInt32(0x20B0_0700) | UInt32($0 % 128) } + [0x2080_3C00]
            var i = 0
            while i < words.count {
                let n = min(60, words.count - i)
                packet = words.withUnsafeMutableBufferPointer { MIDIEventListAdd(list, 65536, packet, 0, n, $0.baseAddress! + i) }
                i += n
            }
            XCTAssertEqual(MIDIReceivedEventList(source, list), noErr)
        }
        wait(2) { delivered.contains { $0.contains("[128,60,0]") } }
        XCTAssertTrue(delivered.contains { $0.contains("[128,60,0]") }, "the note-off came through")
        output.shutdown()
    }

    /// One system message (UMP type 1) from the test's virtual source,
    /// stamped at host second `at`.
    private func system(_ status: UInt8, at: Double) {
        var buffer = [UInt8](repeating: 0, count: 256)
        buffer.withUnsafeMutableBytes { raw in
            let list = raw.baseAddress!.assumingMemoryBound(to: MIDIEventList.self)
            var word: UInt32 = 0x1000_0000 | UInt32(status) << 16
            let stamp = MIDITimeStamp(at * 1e9 * Double(Self.timebase.denom) / Double(Self.timebase.numer))
            var packet = MIDIEventListInit(list, ._1_0)
            packet = MIDIEventListAdd(list, 256, packet, stamp, 1, &word)
            XCTAssertEqual(MIDIReceivedEventList(source, list), noErr)
        }
    }

    /// Each delivery's messages, `(status, stamp in runner ms)`, with when it
    /// came.
    private func messages(_ deliveries: [(at: Double, json: String)]) -> [(at: Double, list: [(status: UInt8, ms: Double)])] {
        deliveries.compactMap { d in
            guard let object = try? JSONSerialization.jsonObject(with: Data(d.json.utf8)) as? [String: Any],
                  let list = object["messages"] as? [[Any]] else { return nil }
            return (d.at, list.map { m in (UInt8((m[2] as? [NSNumber])?.first?.intValue ?? 0), (m[1] as? NSNumber)?.doubleValue ?? .nan) })
        }
    }

    /// LLP 1125 D9: a followed input's ticks come stamped from their packets
    /// and batched at most 50 ms from the oldest; Start goes at once with the
    /// ticks ahead of it; Active Sensing never comes, nor ticks from an input
    /// no sequence follows.
    func testAFollowedClockIsBatchedAndStartGoesAtOnce() throws {
        let library = try library()
        var deliveries: [(at: Double, json: String)] = []
        let output = MIDIOutput(log: { _ in }, deliver: { deliveries.append((CACurrentMediaTime(), $0)) }, library: { library })
        let input = String(uid(source))
        output.apply(["ops": [["op": "open", "inputs": [input], "clock": [input]], ["op": "request"]]])
        wait(2) { deliveries.contains { $0.json.contains(input) } }
        wait(0.2)
        deliveries.removeAll()
        // When the hub heard each message, by its stamp: a tick it heard more
        // than 50 ms late is drained at once, that late.
        var handed: [Double: Double] = [:]
        MIDIHub.observeHeard = { at, heard in self.lock.withLock { handed[at] = heard } }
        defer { MIDIHub.observeHeard = nil }
        let lag = MainLag()
        // 48 ticks at 120 bpm, then Active Sensing, Start and Stop, each
        // stamped as it is sent, from another thread while this one runs.
        var sent: [(status: UInt8, at: Double)] = []
        var finished = false
        DispatchQueue.global().async {
            for status in Array(repeating: UInt8(0xF8), count: 48) + [0xFE, 0xFA, 0xFC] {
                let at = CACurrentMediaTime()
                self.system(status, at: at)
                self.lock.withLock { sent.append((status, at)) }
                // After the last tick, past its batch's 50 ms before Start: a
                // Start wrongly batched cannot ride that drain and look on time.
                self.pause(status == 0xF8 ? 0.5 / 24 : status == 0xFE ? 0.08 : 0.03)
            }
            self.lock.withLock { finished = true }
        }
        wait(5) { self.lock.withLock { finished } }
        wait(0.2)
        lag.stop()
        let got = messages(deliveries), all = got.flatMap(\.list), want = lock.withLock { sent.filter { $0.status != 0xFE } }
        XCTAssertEqual(all.map(\.status), want.map(\.status), "every tick, Start and Stop, in order; no Active Sensing")
        for (m, s) in zip(all, want) { XCTAssertEqual(m.ms, (s.at - ExactEnv.t0) * 1000, accuracy: 0.5, "stamped from its packet") }
        let ticks = got.filter { $0.list.allSatisfy { $0.status == 0xF8 } }
        XCTAssertGreaterThan(ticks.count, 10)
        XCTAssertLessThan(ticks.count, 30, "batched")
        // A batch is timed only if the hub heard its oldest tick within the
        // 50 ms and main lagged at most 20 ms while it waited: then it goes by
        // 100 ms after that tick, and a second hold (150) does not. Under load
        // a batch says nothing either way (main lagged 470 ms at load 85); the
        // ceiling is never widened by a measured lag.
        let machine = machineBusy()
        var timedBatches = 0, worst = 0.0, timedAt: [Bool] = []
        for d in ticks {
            let oldest = ExactEnv.t0 + d.list[0].ms / 1000, late = lag.most(oldest, d.at)
            let handoff = lock.withLock { handed.filter { abs($0.key - oldest) < 0.001 }.min { abs($0.key - oldest) < abs($1.key - oldest) }.map { $0.value - $0.key } } ?? .infinity
            worst = max(worst, late)
            let timed = handoff <= 0.050 && late <= 0.020 && !machine.busy
            timedAt.append(timed)
            guard timed else { continue }
            timedBatches += 1
            XCTAssertLessThan(d.at - oldest, 0.100, "nor held past a second batch")
        }
        // Batch spacing between two timed batches: one delayed then one on time
        // under load comes closer, a tick drained alone 21 ms after the last.
        for i in ticks.indices.dropFirst() where timedAt[i - 1] && timedAt[i] {
            XCTAssertGreaterThan(ticks[i].at - ticks[i - 1].at, 0.040, "ticks alone no more often than 50 ms")
        }
        let start = try XCTUnwrap(got.first { $0.list.contains { $0.status == 0xFA } })
        let fa = try XCTUnwrap(want.first { $0.status == 0xFA })
        // Start goes at once, not with the ticks' batch (50 ms): within 10 ms
        // plus main's lag while it came, timed only if that lag is at most
        // 20 ms, so a Start held with the ticks still fails.
        let startLag = lag.most(fa.at - 0.010, start.at), startTimed = startLag <= 0.020 && !machine.busy
        if startTimed { XCTAssertLessThan(start.at - fa.at, 0.010 + startLag, "Start at once") }
        // Not followed: a tick goes nowhere, a note still comes.
        output.apply(["ops": [["op": "open", "inputs": [input], "clock": []]]])
        wait(0.1)
        deliveries.removeAll()
        for _ in 0..<5 { system(0xF8, at: CACurrentMediaTime()) }
        play()
        wait(0.3)
        XCTAssertEqual(messages(deliveries).flatMap(\.list).map(\.status), [0x90])
        output.shutdown()
        print("MIDIMacTests: \(timedBatches) of \(ticks.count) batches timed (main lagged up to \(worst) s)")
        print("MIDIMacTests: Start timed \(startTimed) (main lagged \(startLag) s)")
        if !startTimed { throw XCTSkip("main lagged \(startLag) s while Start came, load \(machine.load): too loaded to time it") }
        if timedBatches < 3 { throw XCTSkip("\(timedBatches) batches came on a quiet enough machine (main lagged up to \(worst) s, load \(machine.load)): too loaded to time them (their order, count and spacing were checked)") }
    }

    /// Review round 1 (Astra): a drain at once (a note) leaves the ticks
    /// after it their own 50 ms; the delayed drain armed before it does not
    /// deliver them early.
    func testTicksAfterADrainAtOnceWaitTheirOwnFiftyMilliseconds() throws {
        let library = try library()
        var deliveries: [(at: Double, json: String)] = []
        let output = MIDIOutput(log: { _ in }, deliver: { deliveries.append((CACurrentMediaTime(), $0)) }, library: { library })
        let input = String(uid(source))
        output.apply(["ops": [["op": "open", "inputs": [input], "clock": [input]], ["op": "request"]]])
        wait(2) { deliveries.contains { $0.json.contains(input) } }
        wait(0.2)
        deliveries.removeAll()
        var finished = false
        DispatchQueue.global().async {
            self.system(0xF8, at: CACurrentMediaTime())
            self.pause(0.010)
            self.play()
            for _ in 0..<15 {
                self.pause(0.020)
                self.system(0xF8, at: CACurrentMediaTime())
            }
            self.lock.withLock { finished = true }
        }
        wait(3) { self.lock.withLock { finished } }
        wait(0.2)
        let ticks = messages(deliveries).filter { $0.list.allSatisfy { $0.status == 0xF8 } }
        XCTAssertGreaterThan(ticks.count, 2)
        for d in ticks {
            XCTAssertGreaterThan(d.at - (ExactEnv.t0 + d.list[0].ms / 1000), 0.040, "ticks alone wait their 50 ms")
        }
        output.shutdown()
    }

    /// A lone tick goes out its 50 ms after it came, with nothing else waking
    /// the main thread meanwhile: the batch's deadline is not coalesced
    /// (a main-queue `asyncAfter` came 100 to 150 ms late in a session that
    /// coalesces timers, past a follower's lookahead).
    func testALoneTickIsDeliveredFiftyMillisecondsAfterItCame() throws {
        let library = try library()
        var deliveries: [(at: Double, json: String)] = []
        let output = MIDIOutput(log: { _ in }, deliver: { deliveries.append((CACurrentMediaTime(), $0)) }, library: { library })
        let input = String(uid(source))
        output.apply(["ops": [["op": "open", "inputs": [input], "clock": [input]], ["op": "request"]]])
        wait(2) { deliveries.contains { $0.json.contains(input) } }
        wait(0.2)
        // Timed only on a quiet main queue: an attempt whose main lagged more
        // than 20 ms just before or after its wait proves nothing either way,
        // and its ceiling is never widened toward the coalesced 150 ms.
        var timed = 0, worst = 0.0, loadSeen = 0.0
        for _ in 0..<12 where timed < 3 {
            // Measured around the wait, never during it: a wakeup of main
            // then would run a coalesced drain on time.
            let before = MainLag()
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.2))
            before.stop()
            deliveries.removeAll()
            let at = CACurrentMediaTime()
            system(0xF8, at: at)
            // One idle sleep of the main run loop, to its next timer.
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.4))
            let after = MainLag()
            RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.2))
            after.stop()
            let late = max(before.most, after.most)
            worst = max(worst, late)
            loadSeen = max(loadSeen, machineBusy().load)
            guard late <= 0.020, !machineBusy().busy else { continue }
            timed += 1
            let tick = try XCTUnwrap(messages(deliveries).first { $0.list.contains { $0.status == 0xF8 } })
            XCTAssertGreaterThan(tick.at - at, 0.040, "a lone tick waits for its batch")
            XCTAssertLessThan(tick.at - at, 0.100, "and goes when its 50 ms are up, not a coalesced 150 later")
        }
        print("MIDIMacTests: \(timed) lone ticks timed (main lagged up to \(worst) s, load up to \(loadSeen))")
        if timed == 0 { throw XCTSkip("the main queue lagged past 20 ms or the load passed twice the cores around every attempt (lag \(worst) s, load \(loadSeen)): too loaded to time a batch") }
        output.shutdown()
    }

    private static let timebase: mach_timebase_info_data_t = {
        var info = mach_timebase_info_data_t()
        mach_timebase_info(&info)
        return info
    }()
}
#endif
