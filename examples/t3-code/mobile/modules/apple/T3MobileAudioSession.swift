#if os(iOS)
// Shared AVAudioSession activation belongs to every live app playback/recording owner.
// @ref llp/1106.005-composer-and-transcript.decision.md#media-presentation
import AVFoundation

final class T3MobileAudioSession {
    private var owners = Set<UUID>()
    private var activated = false
    func acquire(_ owner: UUID) throws {
        try AVAudioSession.sharedInstance().setActive(true)
        activated = true; owners.insert(owner)
    }
    /// Quick Look, AVKit and WebKit may activate themselves; never deactivate under a live view.
    func hold(_ owner: UUID) { owners.insert(owner) }
    @discardableResult func release(_ owner: UUID) -> Bool {
        owners.remove(owner)
        guard owners.isEmpty, activated else { return true }
        do {
            try AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
            activated = false; return true
        } catch { return false }
    }
}
#endif
