// @ref LLP 1096 D8: the sound output is never opened at boot. On iOS and tvOS
// the first voice opens it; the Mac makes it after first pixel and the first
// voice starts it. An open that fails is journaled, reported in
// `state.sounds.output`, skips its voices and is tried again by a voice a
// second later; an engine the system stopped is opened again by the next
// voice; and the arm's catch turns what AVAudioEngine raises into a reason.
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
    nonisolated(unsafe) static var closes = 0
    static func reset() { opens = []; refuse = nil; refuseStart = nil; running = 0; plays = []; closes = 0 }
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

    override func setUp() { Arm.reset(); journal = [] }
    override func tearDown() {
        Arm.reset()
        #if os(iOS) || os(tvOS)
        AudioSession.reset()
        #endif
    }

    private func output() -> SoundOutput {
        let output = SoundOutput(log: { [weak self] in self?.journal.append($0) }, resolve: { $0 }, library: { Arm.library })
        output.now = { [unowned self] in clock }
        return output
    }

    private func play(_ output: SoundOutput, _ id: Int) {
        output.apply(["ops": [["op": "play", "id": id, "sound": 0, "at": 0, "gain": 1]]])
    }

    private func state(_ output: SoundOutput) -> String? { output.state["state"] as? String }

    func testABootOpensNothingAndTheFirstVoiceOpensIt() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"], "ops": []])
        output.prepare(mac: false)
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
        output.apply(["files": ["assets/kick.wav"]])
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

    func testAnEngineTheSystemStoppedIsOpenedAgainByTheNextVoice() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        play(output, 1)
        Arm.running = 0 // a route change, say
        XCTAssertEqual(state(output), "stopped")
        play(output, 2)
        XCTAssertEqual(Arm.closes, 1)
        XCTAssertEqual(Arm.opens, [true, true])
        XCTAssertEqual(Arm.plays, [1, 2])
        XCTAssertTrue(journal.contains("sound: the output had stopped; opening it again"), "\(journal)")
    }

    func testNewFilesLetTheOutputGoAndTheNextVoiceOpensThem() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        play(output, 1)
        output.apply(["files": ["assets/kick.wav", "assets/hat.wav"]])
        XCTAssertEqual(Arm.closes, 1)
        XCTAssertEqual(state(output), "closed")
        play(output, 2)
        XCTAssertEqual(Arm.opens, [true, true])
    }

    /// The Mac's path, run on every platform by asking for it.
    func testAPreparedOutputIsMadeOffTheMainThreadAndStartedByTheFirstVoice() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        output.prepare(mac: true)
        XCTAssertEqual(state(output), "preparing")
        while state(output) == "preparing" { RunLoop.main.run(until: Date().addingTimeInterval(0.01)) }
        XCTAssertEqual(Arm.opens, [false], "made, not started")
        XCTAssertEqual(state(output), "ready")
        play(output, 1)
        XCTAssertEqual(Arm.opens.count, 1)
        XCTAssertEqual(Arm.plays, [1])
        XCTAssertEqual(state(output), "running")
    }

    func testAPreparedOutputThatDoesNotStartFailsAndIsOpenedAgainLater() {
        let output = output()
        output.apply(["files": ["assets/kick.wav"]])
        output.prepare(mac: true)
        Arm.refuseStart = "the engine did not start: busy"
        play(output, 1) // waits for the making
        XCTAssertEqual(Arm.plays, [])
        XCTAssertEqual(Arm.closes, 1)
        XCTAssertEqual(output.state["error"] as? String, "the engine did not start: busy")
        clock += 1
        Arm.refuseStart = nil
        play(output, 2)
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
        XCTAssertEqual(Arm.opens, [], "no output made with no active session")
        XCTAssertEqual(state(output), "failed")
        XCTAssertTrue((output.state["error"] as? String)?.hasPrefix("the audio session did not activate") == true)
    }
    #endif

    func testTheArmsCatchTurnsAnAVAudioEngineRaiseIntoAReason() {
        var reason = [CChar](repeating: 0, count: 512)
        let engine = AVAudioEngine()
        // Connecting nodes the engine does not hold is a raise, not a throw.
        XCTAssertFalse(exact_sound_try({ engine.connect(AVAudioPlayerNode(), to: AVAudioMixerNode(), format: nil) }, &reason, reason.count))
        XCTAssertFalse(String(cString: reason).isEmpty)
        XCTAssertTrue(exact_sound_try({}, &reason, reason.count))
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
        session.sound = SoundOutput(log: { [weak session] in session?.log($0) }, resolve: { $0 }, library: { Arm.library })
        // Refused from the start, so a Mac's making after first pixel, if a draw comes, fails too.
        Arm.refuse = "the output has no channels (no audio device)"
        XCTAssertNil(session.boot(plan: try Data(contentsOf: dir.appendingPathComponent("app.plan")), size: CGSize(width: 300, height: 200)).error)
        XCTAssertFalse(Arm.opens.contains(true), "the boot started no output")
        XCTAssertNotEqual(try output(session)["state"] as? String, "running")

        let go = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "go" })
        session.apply(session.runtime.press(go.id, now: session.now()))
        XCTAssertEqual(try output(session)["state"] as? String, "failed")
        XCTAssertEqual(try output(session)["error"] as? String, "the output has no channels (no audio device)")
        XCTAssertTrue(session.agent(#"{"op":"logs","since":0}"#).contains("sound: the output did not open: the output has no channels (no audio device)"))

        Arm.refuse = nil
        session.sound.retryAfter = 0
        session.apply(session.runtime.press(go.id, now: session.now()))
        XCTAssertEqual(try output(session)["state"] as? String, "running")
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
