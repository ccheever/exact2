#if os(iOS)
import AVFoundation
import XCTest
@testable import ExactKit

/// LLP 1096 D8: a `playback` app's session is held while something has sound
/// and goes back to ambient when nothing does, as Bluesky's player drops to
/// ambient on re-mute so other apps' audio resumes.
final class AudioSessionIOSTests: XCTestCase {
    private var saved = AudioSession.category
    override func tearDown() { AudioSession.category = saved }

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

    func testAnAmbientAppStaysAmbient() throws {
        AudioSession.category = .ambient
        final class Holder {}
        let a = Holder()
        try AudioSession.hold(ObjectIdentifier(a))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
        AudioSession.release(ObjectIdentifier(a))
        XCTAssertEqual(AVAudioSession.sharedInstance().category, .ambient)
    }
}
#endif
