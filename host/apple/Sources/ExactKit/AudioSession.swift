// @ref LLP 1096 D8, LLP 1133.009 D7. The app's one AVAudioSession owner: the
// only `setCategory` caller, so the sound arm, a video with sound, a canvas's
// audio and a package's capture all run in one session the owner composes.
// iOS and tvOS only: AVAudioSession is unavailable on the Mac, where the arm
// starts its engine with no category and a capture's claim composes nothing.
//
// A capture (a microphone, LLP 1133.009) claims the session: while any claim
// is held the category is `.playAndRecord`, with the mode and options of the
// claim that started the running capture, so the app's own sound still plays
// (`.record` would silence it); the category changes when the first claim
// comes and when the last goes, and nowhere else while one is held. Each claim
// hears the owner's interruptions, their ends, media-services resets and
// route changes, observed once for the process on the main queue.
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
    /// when the interruption ends with `shouldResume`.
    nonisolated(unsafe) private static var holders: Set<ObjectIdentifier> = []
    /// Every live media player, muted or not: deactivating the session would
    /// stop a running one, so the session is only given up when none is left.
    nonisolated(unsafe) private static var players: Set<ObjectIdentifier> = []
    nonisolated(unsafe) private static var observing = false
    /// A canvas's hold, which it never gives back.
    private final class Forever {}
    private static let forever = Forever()

    /// A capture's claim: its mode, options, whether it may run in the
    /// background, and what it is told (1 interrupted, 2 the interruption
    /// ended, 3 media services were reset, 4 the route changed).
    struct Capture {
        let mode: AVAudioSession.Mode, options: AVAudioSession.CategoryOptions, background: Bool
        let events: (UInt32) -> Void
    }
    nonisolated(unsafe) private static var captures: [UInt64: Capture] = [:]
    nonisolated(unsafe) private static var nextCapture: UInt64 = 0
    /// The claim whose mode and options the session took: the first of the
    /// claims held now (LLP 1133.009 D3, D7).
    nonisolated(unsafe) private static var configuring: Capture?

    /// The process keeps running in the background with `audio` in
    /// `UIBackgroundModes` and either a `playback` session (LLP 1096 D8), so
    /// a sequence plays on with the screen locked (LLP 1125 D4), or a capture
    /// claim that may run in the background (LLP 1133.009 D7).
    static var playsInBackground: Bool {
        guard (Bundle.main.object(forInfoDictionaryKey: "UIBackgroundModes") as? [String] ?? []).contains("audio") else { return false }
        return category == .playback || captures.values.contains { $0.background }
    }
    /// For tests: the next activation fails with this, as it can during a call.
    nonisolated(unsafe) static var nextHoldFailure: Error?

    /// Set the app's category and activate the session, for good (a
    /// canvas's audio).
    package static func activate() throws {
        try hold(ObjectIdentifier(forever))
    }

    /// `holder` plays sound: the session takes the app's category and is
    /// active while anything wants it. A failure to activate throws, and the
    /// want stays for the interruption's end to activate.
    package static func hold(_ holder: ObjectIdentifier) throws {
        holders.insert(holder)
        observe()
        try reconcile()
    }

    /// `holder` no longer plays sound. With nothing left wanting it, a
    /// `playback` app goes back to `.ambient`, which mixes with other apps'
    /// audio, as Bluesky's player does on re-mute. A muted player keeps
    /// running, so the session itself is given up (telling other apps they
    /// may resume) only once no player is left. A capture's category stays.
    package static func release(_ holder: ObjectIdentifier) {
        guard holders.remove(holder) != nil, holders.isEmpty, category != .ambient, captures.isEmpty else { return }
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

    /// A capture claims the session (LLP 1133.009 D7): `.playAndRecord`
    /// from the first claim, activated. 0 when the session would not
    /// activate (a call), and then nothing is held.
    package static func claimCapture(mode: AVAudioSession.Mode, options: AVAudioSession.CategoryOptions, background: Bool,
                                     events: @escaping (UInt32) -> Void) -> UInt64 {
        observe()
        let claim = Capture(mode: mode, options: options, background: background, events: events)
        if captures.isEmpty {
            do {
                if let failure = nextHoldFailure { nextHoldFailure = nil; throw failure }
                let session = AVAudioSession.sharedInstance()
                try session.setCategory(.playAndRecord, mode: mode, options: options)
                try session.setActive(true)
            } catch {
                restoreAfterCaptures()
                return 0
            }
            configuring = claim
        }
        nextCapture += 1
        captures[nextCapture] = claim
        return nextCapture
    }

    /// A capture's claim ends; with none left the session goes back to what
    /// the app's sound wants.
    package static func releaseCapture(_ id: UInt64) {
        guard captures.removeValue(forKey: id) != nil, captures.isEmpty else { return }
        configuring = nil
        restoreAfterCaptures()
    }

    /// The category the app's own sound wants, or the session given up.
    private static func restoreAfterCaptures() {
        let session = AVAudioSession.sharedInstance()
        if holders.isEmpty {
            try? session.setCategory(.ambient)
            if players.isEmpty { try? session.setActive(false, options: .notifyOthersOnDeactivation) }
        } else {
            try? session.setCategory(category)
            try? session.setActive(true)
        }
    }

    /// For tests: nothing held, no player, no capture, the category back to ambient.
    static func reset() {
        holders.removeAll()
        players.removeAll()
        captures.removeAll()
        configuring = nil
        nextHoldFailure = nil
        try? AVAudioSession.sharedInstance().setCategory(.ambient)
    }

    private static func reconcile() throws {
        guard !holders.isEmpty || !captures.isEmpty else { return }
        if let failure = nextHoldFailure { nextHoldFailure = nil; throw failure }
        let session = AVAudioSession.sharedInstance()
        if captures.isEmpty, session.category != category { try session.setCategory(category) }
        try session.setActive(true)
    }

    private static func giveUpIfIdle() {
        guard holders.isEmpty, players.isEmpty, captures.isEmpty, category != .ambient else { return }
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    private static func tell(_ event: UInt32) {
        for claim in Array(captures.values) { claim.events(event) }
    }

    /// The process's observers, on the main queue. An interruption's end
    /// takes the session back only with `shouldResume`: without it the
    /// person started other audio, and the next want activates the session
    /// instead; a capture's tracks hear the end either way, and its recorders
    /// stay paused for the app (D8). A media-services reset restores a
    /// capture's configuration without activating it (D7).
    private static func observe() {
        guard !observing else { return }
        observing = true
        let center = NotificationCenter.default
        center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { note in
            guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                  let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
            if type == .began { tell(1); return }
            let resume = AVAudioSession.InterruptionOptions(rawValue: note.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0).contains(.shouldResume)
            if resume { try? reconcile() }
            tell(2)
        }
        center.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: .main) { _ in
            if let claim = configuring { try? AVAudioSession.sharedInstance().setCategory(.playAndRecord, mode: claim.mode, options: claim.options) }
            tell(3)
        }
        center.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { _ in tell(4) }
    }
}
#endif
