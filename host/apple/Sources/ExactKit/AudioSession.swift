// @ref LLP 1096 D8. The app's one AVAudioSession owner: the only `setCategory`
// caller, so the sound arm, a video with sound and a canvas's audio all play
// in the session the app chose. iOS and tvOS only: AVAudioSession is
// unavailable on the Mac, where the arm starts its engine with no category.
#if os(iOS) || os(tvOS)
import AVFoundation
import Foundation

package enum AudioSession {
    /// `app.json`'s `audio_session`, baked into the Info.plist as
    /// `ExactAudioSession`: `.ambient` unless the app asks for `playback`
    /// (the ring/silent switch then no longer mutes it). Settable for tests.
    nonisolated(unsafe) static var category: AVAudioSession.Category =
        Bundle.main.object(forInfoDictionaryKey: "ExactAudioSession") as? String == "playback" ? .playback : .ambient
    /// What holds the app's category now: the sound arm and a canvas's audio
    /// for as long as they run, and each video while it has sound.
    nonisolated(unsafe) private static var holders: Set<ObjectIdentifier> = []
    /// Every live media player, muted or not: deactivating the session would
    /// stop a running one, so the session is only given up when none is left.
    nonisolated(unsafe) private static var players: Set<ObjectIdentifier> = []
    /// The sound arm's and a canvas's hold, which they never give back.
    private final class Forever {}
    private static let forever = Forever()

    /// Set the app's category and activate the session, for good (the sound
    /// arm, a canvas's audio).
    package static func activate() throws {
        try hold(ObjectIdentifier(forever))
    }

    /// `holder` plays sound: the session takes the app's category and is
    /// active while anything holds it.
    /// A failure leaves `holder` not holding, so a later attempt can retry.
    package static func hold(_ holder: ObjectIdentifier) throws {
        let session = AVAudioSession.sharedInstance()
        do {
            if session.category != category { try session.setCategory(category) }
            try session.setActive(true)
        } catch {
            if holders.isEmpty, category != .ambient { try? session.setCategory(.ambient) }
            throw error
        }
        holders.insert(holder)
    }

    /// `holder` no longer plays sound. With nothing left holding it, a
    /// `playback` app goes back to `.ambient`, which mixes with other apps'
    /// audio, as Bluesky's player does on re-mute. A muted player keeps
    /// running, so the session itself is given up (telling other apps they
    /// may resume) only once no player is left.
    package static func release(_ holder: ObjectIdentifier) {
        guard holders.remove(holder) != nil, holders.isEmpty, category != .ambient else { return }
        try? AVAudioSession.sharedInstance().setCategory(.ambient)
        giveUpIfIdle()
    }

    /// A media player exists, muted or not.
    package static func playerCame(_ player: ObjectIdentifier) { players.insert(player) }
    /// A media player is gone; with no player and no holder left, a
    /// `playback` app's session is given up.
    package static func playerWent(_ player: ObjectIdentifier) {
        guard players.remove(player) != nil else { return }
        giveUpIfIdle()
    }

    private static func giveUpIfIdle() {
        guard holders.isEmpty, players.isEmpty, category != .ambient else { return }
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }
}
#endif
