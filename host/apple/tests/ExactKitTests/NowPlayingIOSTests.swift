// @ref LLP 1098 D5, D7, D10, §5: the video arm's media session coordinator
// with publication on, over stand-in players. `MPNowPlayingInfoCenter`
// reads back the owner's title, duration, elapsed time, rate and media type;
// play, pause and the toggle are enabled for any owner, the six only where
// handled, the skips with the element's offsets; the owner moves on play,
// stays after a pause and moves on removal; the last removal clears the
// info, disables every command and gives AVKit its publication back; a
// failed artwork says so; under the driver nothing is assigned. On the Mac
// and, by its name, the iOS simulator (`build.mjs --test --ios`).
import MediaPlayer
import XCTest
@testable import ExactNowPlaying

private final class Player: NowPlayingPlayer {
    var sessionProps: [String: String]
    var sessionPaused = true
    var sessionDuration = 3180.0
    var sessionElapsed = 600.0
    var sessionRate = 1.0
    var avkit = true
    var sent: [String] = []
    init(_ props: [String: String]) { sessionProps = props }
    func sessionPlay() { sessionPaused = false; sent.append("play") }
    func sessionPause() { sessionPaused = true; sent.append("pause") }
    func sessionAction(_ name: String, payload: String) { sent.append("\(name) \(payload)") }
    func sessionAVKitPublishes(_ on: Bool) { avkit = on }
}

final class NowPlayingIOSTests: XCTestCase {
    private let np = NowPlaying.shared
    private var center: [String: Any] { MPNowPlayingInfoCenter.default().nowPlayingInfo ?? [:] }
    private var commands: MPRemoteCommandCenter { MPRemoteCommandCenter.shared() }
    private func claimant(_ title: String, _ tag: String, listeners: String = "", extra: [String: String] = [:]) -> Player {
        Player(["mediaTitle": title, "mediaArtist": "Show", "mediaAlbum": "", "mediaArtwork": "", "semanticTag": tag, "exactListeners": listeners].merging(extra) { $1 })
    }

    func testTheOwnerIsPublishedAndClearedWithItsCommands() {
        let episode = claimant("Episode 1", "audio", listeners: "pause seekforward seekto nexttrack", extra: ["seekbackwardOffset": "15", "seekforwardOffset": "30"])
        let trailer = claimant("Trailer", "video"), plain = Player(["semanticTag": "video"])
        for p in [episode, trailer, plain] { np.joined(p) }
        np.updated(episode); np.updated(trailer); np.updated(plain)
        // Registered last, the trailer owns; no handler, so play, pause and the toggle alone.
        XCTAssertTrue(np.owner === trailer)
        XCTAssertEqual(center[MPMediaItemPropertyTitle] as? String, "Trailer")
        XCTAssertEqual((center[MPNowPlayingInfoPropertyMediaType] as? NSNumber)?.uintValue, MPNowPlayingInfoMediaType.video.rawValue)
        XCTAssertTrue(commands.playCommand.isEnabled && commands.pauseCommand.isEnabled && commands.togglePlayPauseCommand.isEnabled)
        XCTAssertFalse(commands.skipForwardCommand.isEnabled || commands.nextTrackCommand.isEnabled || commands.changePlaybackPositionCommand.isEnabled)
        XCTAssertEqual([episode.avkit, trailer.avkit, plain.avkit], [false, false, false])
        // The episode plays: it owns, and keeps the session paused.
        episode.sessionPaused = false; np.played(episode)
        XCTAssertTrue(np.owner === episode)
        XCTAssertEqual(center[MPMediaItemPropertyTitle] as? String, "Episode 1")
        XCTAssertEqual(center[MPMediaItemPropertyPlaybackDuration] as? Double, 3180)
        XCTAssertEqual(center[MPNowPlayingInfoPropertyElapsedPlaybackTime] as? Double, 600)
        XCTAssertEqual(center[MPNowPlayingInfoPropertyPlaybackRate] as? Double, 1)
        XCTAssertEqual((center[MPNowPlayingInfoPropertyMediaType] as? NSNumber)?.uintValue, MPNowPlayingInfoMediaType.audio.rawValue)
        XCTAssertTrue(commands.skipForwardCommand.isEnabled && commands.changePlaybackPositionCommand.isEnabled && commands.nextTrackCommand.isEnabled)
        XCTAssertFalse(commands.skipBackwardCommand.isEnabled || commands.previousTrackCommand.isEnabled || commands.stopCommand.isEnabled)
        XCTAssertEqual(commands.skipForwardCommand.preferredIntervals, [30])
        XCTAssertEqual(commands.skipBackwardCommand.preferredIntervals, [15])
        episode.sessionPaused = true; np.moved(episode)
        XCTAssertTrue(np.owner === episode)
        XCTAssertEqual(center[MPNowPlayingInfoPropertyPlaybackRate] as? Double, 0)
        #if os(macOS)
        XCTAssertEqual(MPNowPlayingInfoCenter.default().playbackState, .paused)
        #endif
        // The targets' path: the offset the platform gives, else the element's.
        XCTAssertEqual(np.perform("seekforward"), .success)
        XCTAssertEqual(np.perform("seekforward", seconds: 5), .success)
        XCTAssertEqual(np.perform("seekto", seconds: 120.5), .success)
        XCTAssertEqual(np.perform("seekto"), .commandFailed)
        XCTAssertEqual(np.perform("stop"), .commandFailed)
        XCTAssertEqual(np.perform("togglePlayPause"), .success)
        XCTAssertEqual(episode.sent, ["seekforward 30 0 0", "seekforward 5 0 0", "seekto 0 120.5 0", "play"])
        let report = np.report(episode)
        XCTAssertEqual(report["published"] as? String, "MPNowPlayingInfoCenter")
        XCTAssertEqual(report["actions"] as? [String], ["nexttrack", "pause", "play", "seekforward", "seekto"])
        XCTAssertEqual((report["readback"] as? [String: Any])?["title"] as? String, "Episode 1")
        XCTAssertEqual(np.report(trailer)["owner"] as? Bool, false)
        XCTAssertEqual(np.report(plain)["claimant"] as? Bool, false)
        // Removed, the owner hands the session on; the last removal clears it.
        np.left(episode)
        XCTAssertTrue(np.owner === trailer)
        np.left(trailer)
        XCTAssertNil(MPNowPlayingInfoCenter.default().nowPlayingInfo)
        XCTAssertFalse([commands.playCommand, commands.pauseCommand, commands.togglePlayPauseCommand, commands.skipForwardCommand, commands.nextTrackCommand].contains { $0.isEnabled })
        #if os(macOS)
        XCTAssertEqual(MPNowPlayingInfoCenter.default().playbackState, .stopped)
        #endif
        XCTAssertTrue(plain.avkit)
        XCTAssertEqual(np.perform("play"), .noActionableNowPlayingItem)
        np.left(plain)
    }

    func testAFailedArtworkSaysSoAndTheDriverPublishesNothing() {
        let p = claimant("Episode 1", "audio", extra: ["mediaArtwork": "assets/gone.png", "mediaArtworkURL": "file:///nonexistent/gone.png", "exactPublish": "false"])
        np.joined(p); np.updated(p)
        let loaded = expectation(description: "the artwork's load ends")
        DispatchQueue.global().asyncAfter(deadline: .now() + 0.3) { DispatchQueue.main.async { loaded.fulfill() } }
        wait(for: [loaded], timeout: 5)
        let report = np.report(p)
        XCTAssertEqual(report["artworkError"] as? String, "the artwork did not load")
        XCTAssertEqual(report["published"] as? String, "agent")
        XCTAssertEqual((report["readback"] as? [String: Any])?["title"] as? String, "Episode 1")
        XCTAssertNil(MPNowPlayingInfoCenter.default().nowPlayingInfo)
        p.sessionProps["mediaArtwork"] = "app:/tmp/art.png"; p.sessionProps["mediaArtworkURL"] = ""
        np.updated(p)
        XCTAssertEqual((np.report(p)["artworkError"] as? String)?.hasPrefix("an app:/ artwork"), true)
        np.left(p)
    }
}
