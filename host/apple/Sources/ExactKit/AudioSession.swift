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
    package static func hold(_ holder: ObjectIdentifier) throws {
        let session = AVAudioSession.sharedInstance()
        let first = holders.isEmpty
        holders.insert(holder)
        if first || session.category != category { try session.setCategory(category) }
        try session.setActive(true)
    }

    /// `holder` no longer plays sound. With nothing left holding it, a
    /// `playback` app goes back to `.ambient` and lets other apps' audio
    /// resume, as Bluesky's player does on re-mute: a muted autoplaying video
    /// never takes the session from them. Deactivating fails while muted
    /// output still runs; the ambient category alone mixes with others then.
    package static func release(_ holder: ObjectIdentifier) {
        guard holders.remove(holder) != nil, holders.isEmpty, category != .ambient else { return }
        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.ambient)
        try? session.setActive(false, options: .notifyOthersOnDeactivation)
    }
}
#endif
