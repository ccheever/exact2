#if os(iOS)
import AVFoundation
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1096 D8: a `playback` app's session is held while something has sound
/// and goes back to ambient when nothing does, as Bluesky's player drops to
/// ambient on re-mute so other apps' audio resumes.
final class AudioSessionIOSTests: XCTestCase {
    private var saved = AudioSession.category
    override func setUp() { AudioSession.reset() }
    override func tearDown() { AudioSession.reset(); AudioSession.category = saved }

    func testAPlaybackAppHoldsPlaybackWhileSomethingHasSoundAndThenGoesAmbient() throws {
        AudioSession.category = .playback
        final class Holder {}
        let a = Holder(), b = Holder()
        try AudioSession.hold(ObjectIdentifier(a))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playback)
        try AudioSession.hold(ObjectIdentifier(b))
        AudioSession.release(ObjectIdentifier(a))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playback, "one still has sound")
        AudioSession.release(ObjectIdentifier(b))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient, "nothing has sound")
        // A release of something that never held is nothing.
        AudioSession.release(ObjectIdentifier(a))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
        try AudioSession.hold(ObjectIdentifier(a))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playback, "sound again takes it back")
        AudioSession.release(ObjectIdentifier(a))
    }

    /// A muted player keeps running, so with one alive the category goes
    /// ambient but the session is not given up under it.
    func testWithAPlayerAliveReleaseGoesAmbientAndThePlayerLeavingGivesItUp() throws {
        AudioSession.category = .playback
        final class Holder {}
        let player = Holder()
        AudioSession.playerCame(ObjectIdentifier(player))
        try AudioSession.hold(ObjectIdentifier(player))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playback)
        AudioSession.release(ObjectIdentifier(player))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
        AudioSession.playerWent(ObjectIdentifier(player))
        AudioSession.playerWent(ObjectIdentifier(player)) // a second leave is nothing
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
    }

    /// A hold whose activation fails (during a call) still wants the
    /// session: the next activation takes playback, and its release yields.
    func testAFailedActivationStillHoldsAndIsActivatedAgain() throws {
        AudioSession.category = .playback
        final class Holder {}
        let a = Holder(), b = Holder()
        struct Busy: Error {}
        AudioSession.nextHoldFailure = Busy()
        XCTAssertThrowsError(try AudioSession.hold(ObjectIdentifier(a)))
        try AudioSession.hold(ObjectIdentifier(b))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playback, "activated on the next try")
        AudioSession.release(ObjectIdentifier(b))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playback, "the failed one still wants it")
        AudioSession.release(ObjectIdentifier(a))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
    }

    /// LLP 1133.009 D7: a capture's claim takes `.playAndRecord` from the
    /// first claim to the last, whatever the app's sound holds meanwhile,
    /// and the session goes back to what the sound wants after.
    func testACaptureClaimsPlayAndRecordUntilItsLastClaimEnds() throws {
        AudioSession.category = .playback
        final class Holder {}
        let sound = Holder()
        let first = AudioSession.claimCapture(mode: .default, options: [.defaultToSpeaker], background: false) { _ in }
        XCTAssertNotEqual(first, 0)
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playAndRecord)
        try AudioSession.hold(ObjectIdentifier(sound))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playAndRecord, "the app's sound plays in the capture's category")
        let second = AudioSession.claimCapture(mode: .measurement, options: [], background: false) { _ in }
        AudioSession.releaseCapture(first)
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playAndRecord, "a claim is left")
        AudioSession.release(ObjectIdentifier(sound))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playAndRecord, "the sound's end leaves the capture's category")
        try AudioSession.hold(ObjectIdentifier(sound))
        AudioSession.releaseCapture(second)
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playback, "the last claim gone, the sound's category")
        AudioSession.release(ObjectIdentifier(sound))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
    }

    /// A claim whose activation fails (during a call) is refused, 0, and holds nothing.
    func testARefusedClaimHoldsNothing() {
        struct Busy: Error {}
        AudioSession.nextHoldFailure = Busy()
        XCTAssertEqual(AudioSession.claimCapture(mode: .default, options: [], background: true) { _ in }, 0)
        XCTAssertFalse(AudioSession.playsInBackground, "no claim is held")
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
    }

    func testAnAmbientAppStaysAmbient() throws {
        AudioSession.category = .ambient
        final class Holder {}
        let a = Holder()
        try AudioSession.hold(ObjectIdentifier(a))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
        AudioSession.release(ObjectIdentifier(a))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
    }

    /// LLP 1133.009 D7: the owner observes the session once for the process
    /// and tells each listening holder of sound before the claims.
    func testAListeningHolderHearsAResetAndTheAppBecomingActiveThroughTheOwner() {
        final class Holder {}
        let holder = Holder()
        var heard: [String] = []
        AudioSession.listen(ObjectIdentifier(holder)) { event in
            switch event {
            case .interruption(let began, _): heard.append(began ? "began" : "ended")
            case .appActive: heard.append("active")
            case .reset: heard.append("reset")
            }
        }
        var told: [UInt32] = []
        let claim = AudioSession.claimCapture(mode: .default, options: [.defaultToSpeaker], background: false) { told.append($0) }
        NotificationCenter.default.post(name: AVAudioSession.mediaServicesWereResetNotification, object: nil)
        NotificationCenter.default.post(name: UIApplication.didBecomeActiveNotification, object: nil)
        XCTAssertEqual(heard, ["reset", "active"])
        XCTAssertEqual(told, [3], "the capture hears the reset; the app's activity is the sound arm's")
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .playAndRecord, "a reset restores the capture's configuration")
        AudioSession.releaseCapture(claim)
    }
}
#endif
