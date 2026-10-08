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
    /// What wants the app's category now: the sound arm and a canvas's audio
    /// for as long as they run, and each video while it has sound. A want
    /// whose activation failed (during a call) stays, and is activated again
    /// when the interruption ends.
    nonisolated(unsafe) private static var holders: Set<ObjectIdentifier> = []
    /// Every live media player, muted or not: deactivating the session would
    /// stop a running one, so the session is only given up when none is left.
    nonisolated(unsafe) private static var players: Set<ObjectIdentifier> = []
    nonisolated(unsafe) private static var observing = false
    /// The sound arm's and a canvas's hold, which they never give back.
    private final class Forever {}
    private static let forever = Forever()
    /// For tests: the next activation fails with this, as it can during a call.
    nonisolated(unsafe) static var nextHoldFailure: Error?

    /// Set the app's category and activate the session, for good (the sound
    /// arm, a canvas's audio).
    package static func activate() throws {
        try hold(ObjectIdentifier(forever))
    }

    /// `holder` plays sound: the session takes the app's category and is
    /// active while anything wants it. A failure to activate throws, and the
    /// want stays for the interruption's end to activate.
    package static func hold(_ holder: ObjectIdentifier) throws {
        holders.insert(holder)
        observeInterruptions()
        try reconcile()
    }

    /// `holder` no longer plays sound. With nothing left wanting it, a
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

    /// For tests: nothing held, no player, the category back to ambient.
    static func reset() {
        holders.removeAll()
        players.removeAll()
        nextHoldFailure = nil
        try? AVAudioSession.sharedInstance().setCategory(.ambient)
    }

    private static func reconcile() throws {
        guard !holders.isEmpty else { return }
        if let failure = nextHoldFailure { nextHoldFailure = nil; throw failure }
        let session = AVAudioSession.sharedInstance()
        if session.category != category { try session.setCategory(category) }
        try session.setActive(true)
    }

    private static func giveUpIfIdle() {
        guard holders.isEmpty, players.isEmpty, category != .ambient else { return }
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    private static func observeInterruptions() {
        guard !observing else { return }
        observing = true
        NotificationCenter.default.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { note in
            guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                  AVAudioSession.InterruptionType(rawValue: raw) == .ended else { return }
            try? reconcile()
        }
    }
}
#endif
