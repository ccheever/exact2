#if os(iOS)
import AVFoundation
import XCTest
@testable import ExactKit

/// LLP 1133.012 D4: speech claims on the audio-session owner. The session
/// takes the strongest spoken type held, changes only while no other I/O
/// runs, and is given up when the last speech claim goes.
final class AudioSessionSpeechIOSTests: XCTestCase {
    private var saved = AudioSession.category
    override func setUp() { AudioSession.reset() }
    override func tearDown() { AudioSession.reset(); AudioSession.category = saved }

    private var session: AVAudioSession { AVAudioSession.sharedInstance() }

    func testTheStrongestSpokenTypeHeldIsTheSessionsAndTheLastReleaseGivesItUp() {
        let prompt = AudioSession.claimSpeech(type: "transient") { _ in }
        XCTAssertNotEqual(prompt, 0)
        XCTAssertEqual(session.category, .playback, "spoken audio plays with the Silent switch set")
        XCTAssertEqual(session.mode, .voicePrompt)
        XCTAssertTrue(session.categoryOptions.contains(.duckOthers))
        let reading = AudioSession.claimSpeech(type: "playback") { _ in }
        XCTAssertEqual(session.mode, .spokenAudio, "playback is stronger than transient")
        XCTAssertEqual(session.categoryOptions, [], "and not mixable")
        AudioSession.releaseSpeech(reading)
        XCTAssertEqual(session.mode, .voicePrompt, "the prompt still queued takes the session back")
        AudioSession.releaseSpeech(prompt)
        XCTAssertEqual(session.category, .ambient)
        XCTAssertNil(AudioSession.spoken)
        AudioSession.releaseSpeech(prompt) // a second release is nothing
        XCTAssertEqual(session.category, .ambient)
    }

    func testTransientSoloIsNotMixable() {
        let id = AudioSession.claimSpeech(type: "transient-solo") { _ in }
        XCTAssertEqual(session.category, .playback)
        XCTAssertEqual(session.mode, .default)
        XCTAssertFalse(session.categoryOptions.contains(.mixWithOthers))
        AudioSession.releaseSpeech(id)
    }

    /// A capture's configuration holds for its whole length: speech plays
    /// under `.playAndRecord`, and takes its own when the capture goes.
    func testACaptureHoldsItsConfigurationAndSpeechTakesTheSessionAfter() {
        let capture = AudioSession.claimCapture(mode: .default, options: [.defaultToSpeaker], background: false) { _ in }
        let speech = AudioSession.claimSpeech(type: "playback") { _ in }
        XCTAssertNotEqual(speech, 0)
        XCTAssertEqual(session.category, .playAndRecord, "the take's configuration holds")
        AudioSession.releaseCapture(capture)
        XCTAssertEqual(session.category, .playback)
        XCTAssertEqual(session.mode, .spokenAudio)
        AudioSession.releaseSpeech(speech)
        XCTAssertEqual(session.category, .ambient)
    }

    /// A holder that will not stop (a voice pending) keeps the session as it
    /// stands; the speech configuration applies at the boundary its release is.
    func testABusyHolderDelaysTheChangeToItsRelease() throws {
        final class Holder {}
        let sound = Holder()
        try AudioSession.hold(ObjectIdentifier(sound))
        AudioSession.stopsWhenIdle(ObjectIdentifier(sound)) { false }
        let speech = AudioSession.claimSpeech(type: "transient") { _ in }
        XCTAssertNotEqual(speech, 0)
        XCTAssertEqual(session.category, .ambient, "speech plays in the session as it stands")
        XCTAssertNil(AudioSession.spoken)
        AudioSession.release(ObjectIdentifier(sound))
        XCTAssertEqual(session.mode, .voicePrompt, "nothing runs: the speech configuration applies")
        AudioSession.releaseSpeech(speech)
        XCTAssertEqual(session.category, .ambient)
    }

    /// An idle sound output is asked to stop first, and gives its hold back.
    func testAnIdleHolderIsAskedToStop() throws {
        final class Holder {}
        let sound = Holder()
        try AudioSession.hold(ObjectIdentifier(sound))
        var asked = 0
        AudioSession.stopsWhenIdle(ObjectIdentifier(sound)) {
            asked += 1
            AudioSession.release(ObjectIdentifier(sound))
            return true
        }
        let speech = AudioSession.claimSpeech(type: "playback") { _ in }
        XCTAssertEqual(asked, 1)
        XCTAssertEqual(session.mode, .spokenAudio)
        AudioSession.releaseSpeech(speech)
        XCTAssertEqual(session.category, .ambient)
    }

    /// A sound started during speech plays in the configuration in force.
    func testASoundDuringSpeechChangesNothing() throws {
        AudioSession.category = .playback
        let speech = AudioSession.claimSpeech(type: "transient") { _ in }
        final class Holder {}
        let sound = Holder()
        try AudioSession.hold(ObjectIdentifier(sound))
        XCTAssertEqual(session.mode, .voicePrompt)
        AudioSession.release(ObjectIdentifier(sound))
        XCTAssertEqual(session.mode, .voicePrompt)
        AudioSession.releaseSpeech(speech)
        XCTAssertEqual(session.category, .ambient)
    }

    /// A live player, muted or not, keeps the speech configuration until it goes.
    func testALivePlayerDelaysGivingUp() {
        let speech = AudioSession.claimSpeech(type: "playback") { _ in }
        final class Holder {}
        let player = Holder()
        AudioSession.playerCame(ObjectIdentifier(player))
        AudioSession.releaseSpeech(speech)
        XCTAssertEqual(session.mode, .spokenAudio, "a player runs: the session stays")
        AudioSession.playerWent(ObjectIdentifier(player))
        XCTAssertEqual(session.category, .ambient)
    }

    /// A claim the session would not activate for (a call) is refused, 0, and holds nothing.
    func testARefusedSpeechClaimHoldsNothing() {
        struct Busy: Error {}
        AudioSession.nextHoldFailure = Busy()
        XCTAssertEqual(AudioSession.claimSpeech(type: "playback") { _ in }, 0)
        XCTAssertNil(AudioSession.spoken)
        XCTAssertEqual(session.category, .ambient)
    }
}
#endif
