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
    /// (the ring/silent switch then no longer mutes it).
    static var category: AVAudioSession.Category {
        Bundle.main.object(forInfoDictionaryKey: "ExactAudioSession") as? String == "playback" ? .playback : .ambient
    }
    nonisolated(unsafe) private static var configured = false

    /// Set the app's category once, then activate the session.
    package static func activate() throws {
        let session = AVAudioSession.sharedInstance()
        if !configured { try session.setCategory(category); configured = true }
        try session.setActive(true)
    }
}
#endif
