// @ref LLP 1096 D8: the sound output is never opened at boot, and never on
// the main thread. On iOS and tvOS the first voice opens it; the Mac makes it
// after first pixel and the first voice starts it. Voices that come while it
// opens are held and played once it runs, unless too far past their time. An
// open that fails is journaled, reported in `state.sounds.output`, skips its
// voices and is tried again by a voice a second later; an interruption whose
// restart fails fails the same way; an engine the system stopped is opened
// again by the next voice; and AVAudioEngine's raises come back as reasons.
import AVFAudio
import ExactSoundRender
import XCTest
@testable import ExactKit

/// A stand-in for the arm: what it was asked, and what it answers.
private enum Arm {
    /// Each open's `start`.
    nonisolated(unsafe) static var opens: [Bool] = []
    /// An open's failure, while set.
    nonisolated(unsafe) static var refuse: String?
    /// A start's failure, while set.
    nonisolated(unsafe) static var refuseStart: String?
    nonisolated(unsafe) static var running: UInt64 = 0
    nonisolated(unsafe) static var plays: [UInt64] = []
    /// Every op pushed: kind, id, host seconds.
    nonisolated(unsafe) static var ops: [(UInt32, UInt64, Double)] = []
    /// Every engine call's `what`.
    nonisolated(unsafe) static var engine: [UInt32] = []
    nonisolated(unsafe) static var pauses = 0
    nonisolated(unsafe) static var closes = 0
    static func reset() { opens = []; refuse = nil; refuseStart = nil; running = 0; plays = []; ops = []; engine = []; pauses = 0; closes = 0 }
    static let library = SoundLibrary(
        open: { _, _, _, _, start, reason, size in
            Arm.opens.append(start)
            if let refuse = Arm.refuse { _ = strlcpy(reason, refuse, size); return nil }
            Arm.running = start ? 1 : 0
            return UnsafeMutableRawPointer(bitPattern: 0x5011)
        },
        op: { _, kind, id, _, at, _ in Arm.ops.append((kind, id, at)); if kind == 1 { Arm.plays.append(id) } },
        counts: { _, out in out?[3] = Arm.running },
        engine: { _, what, reason, size in
            Arm.engine.append(what)
            if what == 0 { Arm.pauses += 1; Arm.running = 0 }
            guard what == 1 else { return true }
            if let refuse = Arm.refuseStart { _ = strlcpy(reason, refuse, size); return false }
            Arm.running = 1
            return true
        },
        close: { _ in Arm.closes += 1; Arm.running = 0 })
}

final class SoundOutputTests: XCTestCase {
    private var journal: [String] = []
    private var clock = ExactEnv.t0
    private let audio = DispatchQueue(label: "sound-output-tests")

    #if os(iOS) || os(tvOS)
    private let category = AudioSession.category
    #endif
    override func setUp() {
        Arm.reset(); journal = []; clock = ExactEnv.t0
        #if os(iOS) || os(tvOS)
        AudioSession.reset()
        #endif
    }
    override func tearDown() {
        Arm.reset()
        #if os(iOS) || os(tvOS)
        AudioSession.reset()
        AudioSession.category = category
        #endif
    }

    private func output() -> SoundOutput {
        let output = SoundOutput(log: { [weak self] in self?.journal.append($0) }, resolve: { $0 }, library: { Arm.library }, audio: audio)
        output.now = { [unowned self] in clock }
        return output
    }

    /// A voice due `late` seconds before the clock.
    private func play(_ output: SoundOutput, _ id: Int, late: Double = 0) {
        output.apply(["ops": [["op": "play", "id": id, "sound": 0, "at": (clock - late - ExactEnv.t0) * 1000, "gain": 1]]])
    }

    /// Voice `id` ended `late` seconds before the clock.
    private func end(_ output: SoundOutput, _ id: Int, late: Double = 0) {
        output.apply(["ops": [["op": "end", "id": id, "at": (clock - late - ExactEnv.t0) * 1000]]])
    }

    private func state(_ output: SoundOutput) -> String? { output.state["state"] as? String }

    /// Lets the audio queue's jobs finish and land on the main thread, and
    /// the releases those send back run.
    private func settle(_ output: SoundOutput) {
        for round in 0..<500 {
            audio.sync {}
            RunLoop.main.run(until: Date().addingTimeInterval(0.002))
            if round >= 3, state(output) != "opening" { audio.sync {}; return }
        }
        XCTFail("the output is still opening")
    }

    func testABootOpensNothingAndTheFirstVoiceOpensItOffTheMainThread() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"], "ops": []])
        output.prepare(mac: false)
        settle(output)
        XCTAssertEqual(Arm.opens, [], "a boot touches no audio")
        XCTAssertEqual(state(output), "closed")
        audio.suspend()
        play(output, 1)
        XCTAssertEqual(Arm.opens, [], "the main thread does not wait for the open")
        XCTAssertEqual(state(output), "opening")
        XCTAssertEqual(output.state["held"] as? Int, 1)
        play(output, 2)
        audio.resume()
        settle(output)
        XCTAssertEqual(Arm.opens, [true], "one open, started")
        XCTAssertEqual(Arm.plays, [1, 2], "the held voices, in order")
        XCTAssertEqual(state(output), "running")
        play(output, 3)
        XCTAssertEqual(Arm.plays, [1, 2, 3], "a running output takes a voice at once")
    }

    func testAHeldVoiceTooFarPastItsTimeIsSkipped() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        audio.suspend()
        play(output, 1, late: 0.6)
        play(output, 2)
        clock += 0.2 // the open took 200 ms
        audio.resume()
        settle(output)
        XCTAssertEqual(Arm.plays, [2], "the first was 800 ms past its time when the output started")
        XCTAssertEqual(output.state["skipped"] as? UInt64, 1)
        XCTAssertTrue(journal.contains { $0.hasPrefix("sound skipped: 1 held") }, "\(journal)")
    }

    /// A choked window held through a 200 ms open: its first hit's end has
    /// passed too, so the window is moved later whole and still sounds.
    func testAHeldWindowIsMovedLaterWholeSoItsHitsKeepTheirLengths() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        audio.suspend()
        play(output, 1)
        end(output, 1, late: -0.025)
        play(output, 2, late: -0.025)
        let t = clock
        clock += 0.2
        audio.resume()
        settle(output)
        XCTAssertEqual(Arm.ops.map(\.0), [1, 2, 1])
        XCTAssertEqual(Arm.ops.map(\.1), [1, 1, 2])
        for (op, at) in zip(Arm.ops, [t + 0.2, t + 0.225, t + 0.225]) { XCTAssertEqual(op.2, at, accuracy: 1e-6) }
        XCTAssertEqual(output.state["skipped"] as? UInt64, 0)
    }

    func testAnEngineThatKeepsStoppingIsOpenedAgainAtMostOnceASecond() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        play(output, 1)
        settle(output)
        Arm.running = 0
        play(output, 2)
        settle(output)
        XCTAssertEqual(Arm.opens.count, 2, "the first stop reopens at once")
        Arm.running = 0
        clock += 0.5
        play(output, 3)
        settle(output)
        XCTAssertEqual(Arm.opens.count, 2, "stopped again within the second: a failure")
        XCTAssertEqual(state(output), "failed")
        XCTAssertEqual(Arm.plays, [1, 2])
        clock += 1
        play(output, 4)
        settle(output)
        XCTAssertEqual(Arm.opens.count, 3)
        XCTAssertEqual(Arm.plays, [1, 2, 4])
    }

    func testAFailedOpenIsJournaledReportedSkippedAndTriedAgainASecondLater() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        Arm.refuse = "the output has no channels (no audio device)"
        play(output, 1)
        settle(output)
        XCTAssertEqual(Arm.plays, [])
        XCTAssertEqual(state(output), "failed")
        XCTAssertEqual(output.state["error"] as? String, "the output has no channels (no audio device)")
        XCTAssertEqual(output.state["skipped"] as? UInt64, 1)
        XCTAssertTrue(journal.contains { $0.hasPrefix("sound: the output did not open: the output has no channels (no audio device);") }, "\(journal)")
        clock += 0.5
        play(output, 2)
        settle(output)
        XCTAssertEqual(Arm.opens.count, 1, "not tried again within the second")
        XCTAssertEqual(output.state["skipped"] as? UInt64, 2)
        clock += 0.5
        Arm.refuse = nil
        play(output, 3)
        settle(output)
        XCTAssertEqual(Arm.opens.count, 2, "tried again a second later")
        XCTAssertEqual(Arm.plays, [3])
        XCTAssertEqual(state(output), "running")
        XCTAssertNil(output.state["error"])
    }

    func testAnEngineTheSystemStoppedIsOpenedAgainByTheNextVoice() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        play(output, 1)
        settle(output)
        Arm.running = 0 // a route change, say
        XCTAssertEqual(state(output), "stopped")
        play(output, 2)
        settle(output)
        XCTAssertEqual(Arm.closes, 1)
        XCTAssertEqual(Arm.opens, [true, true])
        XCTAssertEqual(Arm.plays, [1, 2])
        XCTAssertTrue(journal.contains("sound: the output had stopped; opening it again"), "\(journal)")
    }

    func testNewFilesLetTheOutputGoAndAnOpenUnderWayIsReleased() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        play(output, 1)
        settle(output)
        output.apply(["files": ["assets/kick.wav", "assets/hat.wav"]])
        settle(output)
        XCTAssertEqual(Arm.closes, 1)
        XCTAssertEqual(state(output), "closed")
        audio.suspend()
        play(output, 2)
        output.apply(["files": ["assets/hat.wav"]]) // a reload while it opens
        audio.resume()
        settle(output)
        XCTAssertEqual(Arm.opens, [true, true])
        XCTAssertEqual(Arm.closes, 2, "the stale open is released")
        XCTAssertEqual(Arm.plays, [1], "and the last boot's held voice dropped")
        XCTAssertEqual(state(output), "closed")
    }

    /// The Mac's path, run on every platform by asking for it.
    func testAPreparedOutputIsStartedOffTheMainThreadByTheFirstVoice() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        output.prepare(mac: true)
        settle(output)
        XCTAssertEqual(Arm.opens, [false], "made, not started")
        XCTAssertEqual(state(output), "ready")
        audio.suspend()
        play(output, 1)
        XCTAssertEqual(state(output), "opening")
        XCTAssertEqual(Arm.plays, [])
        audio.resume()
        settle(output)
        XCTAssertEqual(Arm.opens.count, 1)
        XCTAssertEqual(Arm.plays, [1])
        XCTAssertEqual(state(output), "running")
    }

    func testAPreparedOutputThatDoesNotStartFailsAndIsOpenedAgainLater() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        output.prepare(mac: true)
        Arm.refuseStart = "the engine did not start: busy"
        play(output, 1) // held while the making finishes, then the start fails
        settle(output)
        XCTAssertEqual(Arm.plays, [])
        XCTAssertEqual(Arm.closes, 1)
        XCTAssertEqual(output.state["error"] as? String, "the engine did not start: busy")
        XCTAssertEqual(output.state["skipped"] as? UInt64, 1)
        clock += 1
        Arm.refuseStart = nil
        play(output, 2)
        settle(output)
        XCTAssertEqual(Arm.opens, [false, true])
        XCTAssertEqual(Arm.plays, [2])
    }

    #if os(iOS) || os(tvOS)
    func testASessionThatDoesNotActivateLeavesTheEngineUntouched() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        struct Busy: Error {}
        AudioSession.nextHoldFailure = Busy()
        play(output, 1)
        settle(output)
        XCTAssertEqual(Arm.opens, [], "no output made with no active session")
        XCTAssertEqual(state(output), "failed")
        XCTAssertTrue((output.state["error"] as? String)?.hasPrefix("the audio session did not activate") == true)
    }

    func testARestartThatFailsAfterAnInterruptionFailsTheOutputUntilASecondLater() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        play(output, 1)
        settle(output)
        output.interruption(began: true, resume: false)
        settle(output)
        XCTAssertEqual(Arm.pauses, 1)
        play(output, 2)
        XCTAssertTrue(journal.contains("sound skipped: interrupted"))
        Arm.refuseStart = "the engine did not start: still in a call"
        output.interruption(began: false, resume: true)
        settle(output)
        XCTAssertEqual(state(output), "failed")
        XCTAssertEqual(output.state["error"] as? String, "the engine did not start: still in a call")
        XCTAssertEqual(Arm.closes, 1)
        Arm.refuseStart = nil
        play(output, 3)
        settle(output)
        XCTAssertEqual(Arm.opens.count, 1, "not reopened within the second")
        clock += 1
        play(output, 4)
        settle(output)
        XCTAssertEqual(Arm.opens.count, 2)
        XCTAssertEqual(Arm.plays, [1, 4])
    }

    /// Without `shouldResume` (the person started other audio) nothing
    /// restarts; the next voice activates the session and starts the engine.
    func testAnInterruptionEndedWithoutResumeWaitsForTheNextVoice() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        play(output, 1)
        settle(output)
        output.interruption(began: true, resume: false)
        output.interruption(began: false, resume: false)
        settle(output)
        XCTAssertEqual(Arm.engine.filter { $0 == 1 }, [], "not restarted")
        XCTAssertEqual(state(output), "ready")
        play(output, 2)
        settle(output)
        XCTAssertEqual(Arm.engine.filter { $0 == 1 }.count, 1, "the voice starts it")
        XCTAssertEqual(Arm.plays, [1, 2])
        XCTAssertEqual(state(output), "running")
    }

    /// An interruption's end that never came (the app was suspended) is
    /// over when the app is active again.
    func testBecomingActiveEndsAnInterruptionWhoseEndNeverCame() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        play(output, 1)
        settle(output)
        output.interruption(began: true, resume: false)
        settle(output)
        play(output, 2)
        output.becameActive()
        play(output, 3)
        settle(output)
        XCTAssertEqual(Arm.plays, [1, 3])
    }

    /// A playback app's failed output gives the session back, so other
    /// apps' audio is not left paused.
    func testAFailedOpenGivesTheSessionBack() {
        AudioSession.category = .playback
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        Arm.refuse = "the output has no channels (no audio device)"
        play(output, 1)
        settle(output)
        XCTAssertEqual(state(output), "failed")
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
    }

    func testAMediaServicesResetLetsTheEngineGoUnmessaged() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        play(output, 1)
        settle(output)
        NotificationCenter.default.post(name: AVAudioSession.mediaServicesWereResetNotification, object: nil)
        settle(output)
        XCTAssertEqual(Arm.engine, [3], "orphaned before its close")
        XCTAssertEqual(Arm.closes, 1)
        XCTAssertEqual(state(output), "closed")
        play(output, 2)
        settle(output)
        XCTAssertEqual(Arm.opens.count, 2)
    }
    #endif

    func testAnAVAudioEngineRaiseComesBackAsAReason() {
        var reason = [CChar](repeating: 0, count: 512)
        // Starting an engine with neither an input nor an output node raises,
        // and touches no audio device.
        XCTAssertFalse(exact_sound_engine_start(AVAudioEngine(), &reason, reason.count))
        XCTAssertTrue(String(cString: reason).hasPrefix("AVAudioEngine raised "), String(cString: reason))
    }

    #if os(macOS)
    /// The agent's `state` fills `sounds.output` from the session's output,
    /// and its journal has the failure: a boot opens nothing, a press opens.
    func testTheAgentsStateReportsTheOutputAndTheJournalItsFailure() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: dir.appendingPathComponent("assets"), withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try wav().write(to: dir.appendingPathComponent("assets/tick.wav"))
        try """
        sound "assets/tick.wav"
        component App
          action go
            playSound("assets/tick.wav")
          view
            column
              button "Go" press=go testId="go"
        """.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process(), said = Pipe()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        compiler.standardError = said
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0, String(decoding: said.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self))

        let session = ExactApp.shared.makeSession(label: "sound-" + UUID().uuidString)
        defer { session.destroy() }
        let output = SoundOutput(log: { [weak session] in session?.log($0) }, resolve: { $0 }, library: { Arm.library }, audio: audio)
        session.sound = output
        // Refused from the start, so a Mac's making after first pixel, if a draw comes, fails too.
        Arm.refuse = "the output has no channels (no audio device)"
        XCTAssertNil(session.boot(plan: try Data(contentsOf: dir.appendingPathComponent("app.plan")), size: CGSize(width: 300, height: 200)).error)
        settle(output)
        XCTAssertFalse(Arm.opens.contains(true), "the boot started no output")
        XCTAssertNotEqual(try self.output(session)["state"] as? String, "running")

        let go = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "go" })
        session.apply(session.runtime.press(go.id, now: session.now()))
        settle(output)
        XCTAssertEqual(try self.output(session)["state"] as? String, "failed")
        XCTAssertEqual(try self.output(session)["error"] as? String, "the output has no channels (no audio device)")
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("sound: the output did not open: the output has no channels (no audio device)"))

        Arm.refuse = nil
        output.retryAfter = 0
        session.apply(session.runtime.press(go.id, now: session.now()))
        settle(output)
        XCTAssertEqual(try self.output(session)["state"] as? String, "running")
        XCTAssertEqual(Arm.plays.count, 1)
    }

    /// `state.sounds.output` as the agent replies it.
    private func output(_ session: ExactSession) throws -> [String: Any] {
        let pipe = Pipe(), savedOut = Agent.out, savedRoutes = Agent.routes
        Agent.out = pipe.fileHandleForWriting
        Agent.routes = [("sound", session)]
        defer { Agent.out = savedOut; Agent.routes = savedRoutes }
        Agent.handle(#"{"op":"state"}"#)
        try pipe.fileHandleForWriting.close()
        let reply = try XCTUnwrap(try JSONSerialization.jsonObject(with: pipe.fileHandleForReading.readDataToEndOfFile()) as? [String: Any])
        return try XCTUnwrap((reply["sounds"] as? [String: Any])?["output"] as? [String: Any], "\(reply["sounds"] ?? "no sounds")")
    }

    /// A tenth of a second of 16-bit mono silence at 48 kHz.
    private func wav() -> Data {
        let frames = 4_800, bytes = frames * 2
        var d = Data()
        func u32(_ v: Int) { withUnsafeBytes(of: UInt32(v).littleEndian) { d.append(contentsOf: $0) } }
        func u16(_ v: Int) { withUnsafeBytes(of: UInt16(v).littleEndian) { d.append(contentsOf: $0) } }
        d.append(contentsOf: Array("RIFF".utf8)); u32(36 + bytes); d.append(contentsOf: Array("WAVE".utf8))
        d.append(contentsOf: Array("fmt ".utf8)); u32(16); u16(1); u16(1); u32(48_000); u32(96_000); u16(2); u16(16)
        d.append(contentsOf: Array("data".utf8)); u32(bytes); d.append(Data(count: bytes))
        return d
    }
    #endif
}
