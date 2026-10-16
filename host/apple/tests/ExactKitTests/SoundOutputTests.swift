// @ref LLP 1096 D8: the sound output is never opened at boot; the first
// voice opens it, synchronously. An open that fails is journaled, reported
// in `state.sounds.output`, skips its voices, gives back its session claim
// and is tried again by a voice a second later; an engine the system stopped
// is opened again by the next voice, and one that stops again within a
// second is a failure; an interruption resumes only with `shouldResume`;
// a media services reset disposes of the engine; a destroyed session lets
// its output go; and AVAudioEngine's raises come back as reasons.
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
    /// Every engine call's `what`.
    nonisolated(unsafe) static var engine: [UInt32] = []
    nonisolated(unsafe) static var closes = 0
    static func reset() { opens = []; refuse = nil; refuseStart = nil; running = 0; plays = []; engine = []; closes = 0 }
    static let library = SoundLibrary(
        open: { _, _, _, _, start, reason, size in
            Arm.opens.append(start)
            if let refuse = Arm.refuse { _ = strlcpy(reason, refuse, size); return nil }
            Arm.running = start ? 1 : 0
            return UnsafeMutableRawPointer(bitPattern: 0x5011)
        },
        op: { _, kind, id, _, _, _ in if kind == 1 { Arm.plays.append(id) } },
        counts: { _, out in out?[3] = Arm.running },
        engine: { _, what, reason, size in
            Arm.engine.append(what)
            if what == 0 { Arm.running = 0 }
            guard what == 1 else { return true }
            if let refuse = Arm.refuseStart { _ = strlcpy(reason, refuse, size); return false }
            Arm.running = 1
            return true
        },
        close: { _ in Arm.closes += 1; Arm.running = 0 })
}

final class SoundOutputTests: XCTestCase {
    private var journal: [String] = []
    private var clock = 100.0
    #if os(iOS) || os(tvOS)
    private let category = AudioSession.category
    #endif

    override func setUp() {
        Arm.reset(); journal = []; clock = 100
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
        let output = SoundOutput(log: { [weak self] in self?.journal.append($0) }, resolve: { $0 }, library: { Arm.library })
        output.now = { [unowned self] in clock }
        output.apply(["files": ["assets/kick.wav"], "ops": []])
        return output
    }

    private func play(_ output: SoundOutput, _ id: Int) {
        output.apply(["ops": [["op": "play", "id": id, "sound": 0, "at": 0, "gain": 1]]])
    }

    private func state(_ output: SoundOutput) -> String? { output.state["state"] as? String }

    func testABootOpensNothingAndTheFirstVoiceOpensIt() {
        let output = output()
        XCTAssertEqual(Arm.opens, [], "a boot touches no audio")
        XCTAssertEqual(state(output), "closed")
        play(output, 1)
        XCTAssertEqual(Arm.opens, [true], "the first voice opens and starts it")
        XCTAssertEqual(Arm.plays, [1])
        XCTAssertEqual(state(output), "running")
        play(output, 2)
        XCTAssertEqual(Arm.opens.count, 1, "once")
        XCTAssertEqual(Arm.plays, [1, 2])
    }

    func testAFailedOpenIsJournaledReportedSkippedAndTriedAgainASecondLater() {
        let output = output()
        Arm.refuse = "the output has no channels (no audio device)"
        play(output, 1)
        XCTAssertEqual(Arm.plays, [])
        XCTAssertEqual(state(output), "failed")
        XCTAssertEqual(output.state["error"] as? String, "the output has no channels (no audio device)")
        XCTAssertEqual(output.state["skipped"] as? UInt64, 1)
        XCTAssertTrue(journal.contains { $0.hasPrefix("sound: the output did not open: the output has no channels (no audio device);") }, "\(journal)")
        clock += 0.5
        play(output, 2)
        XCTAssertEqual(Arm.opens.count, 1, "not tried again within the second")
        XCTAssertEqual(output.state["skipped"] as? UInt64, 2)
        clock += 0.5
        Arm.refuse = nil
        play(output, 3)
        XCTAssertEqual(Arm.opens.count, 2, "tried again a second later")
        XCTAssertEqual(Arm.plays, [3])
        XCTAssertEqual(state(output), "running")
        XCTAssertNil(output.state["error"])
    }

    func testAnEngineTheSystemStoppedIsOpenedAgainAndOneThatStopsAgainWithinASecondFails() {
        let output = output()
        play(output, 1)
        clock += 2
        Arm.running = 0 // a route change, say
        XCTAssertEqual(state(output), "stopped")
        play(output, 2)
        XCTAssertEqual(Arm.closes, 1)
        XCTAssertEqual(Arm.opens, [true, true], "opened again at once")
        XCTAssertTrue(journal.contains("sound: the output had stopped; opening it again"), "\(journal)")
        Arm.running = 0
        clock += 0.5
        play(output, 3)
        XCTAssertEqual(Arm.opens.count, 2, "stopped again within the second of opening: a failure")
        XCTAssertEqual(state(output), "failed")
        clock += 1
        play(output, 4)
        XCTAssertEqual(Arm.opens.count, 3, "the cooldown's retry opens it")
        Arm.running = 0
        clock += 0.5
        play(output, 5)
        XCTAssertEqual(Arm.opens.count, 3, "the retry counts as an opening: stopped within its second, a failure")
        XCTAssertEqual(Arm.plays, [1, 2, 4])
    }

    func testNewFilesLetTheOutputGoAndTheNextVoiceOpensThem() {
        let output = output()
        play(output, 1)
        output.apply(["files": ["assets/kick.wav", "assets/hat.wav"]])
        XCTAssertEqual(Arm.closes, 1)
        XCTAssertEqual(state(output), "closed")
        play(output, 2)
        XCTAssertEqual(Arm.opens, [true, true])
    }

    func testShutdownLetsTheOutputGo() {
        let output = output()
        play(output, 1)
        output.shutdown()
        XCTAssertEqual(Arm.closes, 1)
        XCTAssertEqual(state(output), "closed")
    }

    #if os(iOS) || os(tvOS)
    func testASessionThatDoesNotActivateLeavesTheEngineUntouched() {
        let output = output()
        struct Busy: Error {}
        AudioSession.nextHoldFailure = Busy()
        play(output, 1)
        XCTAssertEqual(Arm.opens, [], "no output made with no active session")
        XCTAssertEqual(state(output), "failed")
        XCTAssertTrue((output.state["error"] as? String)?.hasPrefix("the audio session did not activate") == true)
    }

    /// A playback app's failed output gives the session back, so other
    /// apps' audio is not left paused.
    func testAFailedOpenGivesTheSessionBack() {
        AudioSession.category = .playback
        let output = output()
        Arm.refuse = "the output has no channels (no audio device)"
        play(output, 1)
        XCTAssertEqual(state(output), "failed")
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
    }

    func testARestartThatFailsAfterAnInterruptionFailsTheOutputUntilASecondLater() {
        let output = output()
        play(output, 1)
        output.interruption(began: true, resume: false)
        XCTAssertEqual(Arm.engine, [0])
        play(output, 2)
        XCTAssertTrue(journal.contains("sound skipped: interrupted"))
        Arm.refuseStart = "the engine did not start: still in a call"
        output.interruption(began: false, resume: true)
        XCTAssertEqual(state(output), "failed")
        XCTAssertEqual(output.state["error"] as? String, "the engine did not start: still in a call")
        XCTAssertEqual(Arm.closes, 1)
        Arm.refuseStart = nil
        play(output, 3)
        XCTAssertEqual(Arm.opens.count, 1, "not opened again within the second")
        clock += 1
        play(output, 4)
        XCTAssertEqual(Arm.opens.count, 2)
        XCTAssertEqual(Arm.plays, [1, 4])
    }

    /// Without `shouldResume` (the person started other audio) nothing
    /// restarts; the next voice activates the session and starts the engine.
    func testAnInterruptionEndedWithoutResumeWaitsForTheNextVoice() {
        let output = output()
        play(output, 1)
        output.interruption(began: true, resume: false)
        output.interruption(began: false, resume: false)
        XCTAssertEqual(Arm.engine, [0], "not restarted")
        XCTAssertEqual(state(output), "stopped")
        play(output, 2)
        XCTAssertEqual(Arm.engine, [0, 1], "the voice starts it")
        XCTAssertEqual(Arm.plays, [1, 2])
        XCTAssertEqual(state(output), "running")
    }

    /// An interruption's end that never came (the app was suspended) is
    /// over when the app is active again.
    func testBecomingActiveEndsAnInterruptionWhoseEndNeverCame() {
        let output = output()
        play(output, 1)
        output.interruption(began: true, resume: false)
        play(output, 2)
        output.becameActive()
        play(output, 3)
        XCTAssertEqual(Arm.plays, [1, 3])
    }

    func testAMediaServicesResetDisposesOfTheEngine() {
        let output = output()
        play(output, 1)
        output.mediaServicesWereReset()
        XCTAssertEqual(Arm.engine, [3], "orphaned before its close")
        XCTAssertEqual(Arm.closes, 1)
        XCTAssertEqual(state(output), "closed")
        play(output, 2)
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
        let output = SoundOutput(log: { [weak session] in session?.log($0) }, resolve: { $0 }, library: { Arm.library })
        session.sound = output
        Arm.refuse = "the output has no channels (no audio device)"
        XCTAssertNil(session.boot(plan: try Data(contentsOf: dir.appendingPathComponent("app.plan")), size: CGSize(width: 300, height: 200)).error)
        XCTAssertEqual(Arm.opens, [], "the boot opened no output")
        XCTAssertEqual(try self.output(session)["state"] as? String, "closed")

        let go = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "go" })
        session.apply(session.runtime.press(go.id, now: session.now()))
        XCTAssertEqual(try self.output(session)["state"] as? String, "failed")
        XCTAssertEqual(try self.output(session)["error"] as? String, "the output has no channels (no audio device)")
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("sound: the output did not open: the output has no channels (no audio device)"))

        Arm.refuse = nil
        output.retryAfter = 0
        session.apply(session.runtime.press(go.id, now: session.now()))
        XCTAssertEqual(try self.output(session)["state"] as? String, "running")
        XCTAssertEqual(Arm.plays.count, 1)
        session.destroy()
        XCTAssertEqual(Arm.closes, 1, "a destroyed session lets its output go")
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
