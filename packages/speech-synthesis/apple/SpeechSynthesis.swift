// @exact/speech-synthesis on Apple (LLP 1133.012 D2–D6, §9): the session's
// `AVSpeechSynthesizer`, answered in the interface's shapes.
//
// - An utterance is an id (D2), kept with its state and outcome until the
//   session ends. The queue is this implementation's: the synthesizer is
//   given one utterance at a time, the next when the last ends, so a
//   canceled or dropped utterance never reaches it, and the state's
//   `queue`, `utterance` and last boundary are values.
// - Spoken audio claims the host's audio session (D4) with the queue's
//   strongest type while it speaks and is not paused, as the mode word
//   `speech-<type>`; the owner decides when the session can take it. An
//   unsaid type is `playback` in an app whose `audio_session` is
//   `playback`, else `transient` (D3). The synthesizer plays in the
//   application's session (`usesApplicationAudioSession`, iOS).
// - Unsaid options are the platform's (D3): no voice set is the person's
//   default; `prefersAssistiveTechnologySettings` is true unless written.
// - An interruption pauses the queue (`interrupted`), and only
//   `resumeSpeech` resumes it. Headphones removed pause a `playback` queue
//   (`route`). Leaving the foreground cancels a `transient` or
//   `transient-solo` queue; a `playback` queue speaks on where the app has
//   `audio` in `UIBackgroundModes`, and pauses (`background`) elsewhere on
//   iOS until the app returns.
// - The delegate is called on a queue the synthesizer chooses; each call
//   hops to main and names its utterance by identity.
// - Tier 3 (D5): `SpeechSynthesisModule.synthesizerHatch` is given the
//   synthesizer once, before its first utterance; `utteranceHatch` each
//   utterance, after the module set its words.
// - Under the agent nothing is spoken (§9), as on the web: each utterance
//   is spoken on the session clock at 170 words a minute times `rate`, a
//   `word` boundary at each word's start, and `end` after the last; a
//   paused queue's clock stands still.
import AVFoundation
import ExactPackage
import Foundation

public final class SpeechSynthesisModule: NSObject, ExactPackage, AVSpeechSynthesizerDelegate {
    /// The app's access hatch (D5): the session's synthesizer, once, before its first `speak`.
    public static var synthesizerHatch: ((AVSpeechSynthesizer) -> Void)?
    /// Each utterance after the module set its words, before it is spoken.
    public static var utteranceHatch: ((AVSpeechUtterance) -> Void)?

    static let wpm = 170.0
    static let strength = ["transient", "transient-solo", "playback"]

    final class Utterance {
        let id: String, text: String, type: String, rate: Double, words: [(at: Int, length: Int)]
        var state = "queued", outcome: String?, error: String?
        var began = 0.0, paused: Double?, failure: String?
        var spoken: AVSpeechUtterance?
        /// Its voice is still being looked up, off the main thread; the queue does not begin it until then.
        var resolving = false
        init(id: String, text: String, type: String, rate: Double) {
            self.id = id; self.text = text; self.type = type; self.rate = rate; words = Self.words(text)
        }
        /// Each whitespace-separated word: its start and length in UTF-16 code units, as the web counts.
        static func words(_ text: String) -> [(at: Int, length: Int)] {
            var out: [(Int, Int)] = [], start: Int?, i = 0
            for unit in text.utf16 {
                let space = unit == 0x20 || (0x09...0x0D).contains(unit) || unit == 0xA0
                if space, let s = start { out.append((s, i - s)); start = nil } else if !space, start == nil { start = i }
                i += 1
            }
            if let s = start { out.append((s, i - s)) }
            return out
        }
    }

    private let context: ExactPackageContext
    private var synthesizer: AVSpeechSynthesizer?
    private var utterances: [String: Utterance] = [:]
    private var queue: [String] = [], current: String?
    private var boundary: [String: Any] = SpeechSynthesisModule.noBoundary, heldBoundary: [String: Any]?
    /// Why the queue is paused, in the order the reasons came.
    private var reasons: [String] = []
    private var claim: ExactSubscription?, claimed = "auto"
    private var fail: String?, failSeen: String?
    private var next = 0
    private var observers: [NSObjectProtocol] = []
    /// Where the voice catalog is read. On the iOS 27 simulator `AVSpeechSynthesisVoice.speechVoices()` called on
    /// the main thread waits on the main thread (TextToSpeech's `axUnsafeForcedSync`) and never returns, so no
    /// voice lookup is made there.
    private let voiceQueue = DispatchQueue(label: "exact.speech-synthesis.voices", qos: .userInitiated)
    /// The voices last read, nil until the first read lands.
    private var catalog: [[String: Any]]?
    private var catalogWaiters: [([[String: Any]]) -> Void] = [], reading = false

    static let noBoundary: [String: Any] = ["charIndex": 0, "charLength": 0, "name": "", "elapsedTime": 0]
    static let noUtterance: [String: Any] = ["id": "", "state": "ended", "outcome": NSNull(), "error": NSNull()]

    public init(context: ExactPackageContext) {
        self.context = context
        super.init()
        guard !context.agent else { return }
        readVoices()
        observers.append(NotificationCenter.default.addObserver(forName: AVSpeechSynthesizer.availableVoicesDidChangeNotification, object: nil, queue: .main) { [weak self] _ in
            self?.readVoices()
        })
        #if os(iOS) || os(tvOS)
        observers.append(NotificationCenter.default.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] note in
            self?.routeChanged(note)
        })
        #endif
    }

    deinit { observers.forEach(NotificationCenter.default.removeObserver) }

    private func say(_ line: String) { context.log("speech-synthesis: \(line)") }
    private func announce() {
        context.changed("speech-synthesis.speechSynthesisState")
        context.changed("speech-synthesis.speechUtterance")
    }

    // MARK: the names

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "speak": reply(speak(args))
        case "cancelSpeech": cancelAll(); reply(state())
        case "pauseSpeech": pause("requested"); reply(state())
        case "resumeSpeech": resume(["requested", "interrupted", "focus", "route"]); reply(state())
        case "speechSynthesisState": reply(state())
        case "speechUtterance":
            let id = args["utterance"] as? String ?? ""
            if id.isEmpty { return reply(Self.noUtterance) }
            advance()
            reply(utterances[id].map(shape) ?? ["error": "NotFoundError"])
        case "speechSynthesisVoices":
            if context.agent { return reply(["voices": Self.agentVoices()]) }
            if let catalog { return reply(["voices": catalog]) }
            catalogWaiters.append { reply(["voices": $0]) }
            readVoices()
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        switch name {
        case "speechSynthesisStateSync": state()
        case "speechSynthesisVoicesSync": ["voices": context.agent ? Self.agentVoices() : catalog ?? []]
        default: ["error": "NotSupportedError"]
        }
    }

    private func speak(_ args: [String: Any]) -> [String: Any] {
        advance()
        next += 1
        let id = "utterance-\(context.session)-\(next)", text = args["text"] as? String ?? ""
        let u = Utterance(id: id, text: text, type: Self.resolved(args["audioSessionType"] as? String), rate: args["rate"] as? Double ?? 1)
        utterances[id] = u
        queue.append(id)
        say("\(id): queued (\(u.words.count) words, \(u.type))")
        if !context.agent {
            u.resolving = true
            voiceQueue.async { [weak self] in
                let made = Self.spoken(u, args)
                DispatchQueue.main.async { self?.resolved(u, made, args) }
            }
        }
        proceed()
        announce()
        return ["id": id]
    }

    /// `u`'s platform utterance, made off the main thread, or why it cannot be spoken: it ends with that error, or
    /// is given to the app's hatch and may begin.
    private func resolved(_ u: Utterance, _ made: Result<AVSpeechUtterance, Refusal>, _ args: [String: Any]) {
        u.resolving = false
        guard u.state != "ended" else { return }
        switch made {
        case .failure(let refused): end(u, "error", refused.word)
        case .success(let s):
            #if os(iOS)
            if let uplink = (args["apple"] as? [String: Any])?["mixToTelephonyUplink"] as? Bool { self.made().mixToTelephonyUplink = uplink }
            #endif
            Self.utteranceHatch?(s)
            u.spoken = s
            proceed()
        }
        announce()
    }

    struct Refusal: Error { let word: String }

    /// The type an utterance claims: the written one, or the app's (D3).
    static func resolved(_ written: String?) -> String {
        if let written, strength.contains(written) { return written }
        return Bundle.main.object(forInfoDictionaryKey: "ExactAudioSession") as? String == "playback" ? "playback" : "transient"
    }

    /// The platform's utterance for `u` (D3), or why it cannot be spoken. Made on `voiceQueue`.
    private static func spoken(_ u: Utterance, _ args: [String: Any]) -> Result<AVSpeechUtterance, Refusal> {
        let apple = args["apple"] as? [String: Any] ?? [:]
        let s: AVSpeechUtterance
        if apple["ssml"] as? Bool == true {
            guard let made = AVSpeechUtterance(ssmlRepresentation: u.text) else { return .failure(Refusal(word: "invalid-argument")) }
            s = made
        } else {
            s = AVSpeechUtterance(string: u.text)
        }
        if let voice = args["voice"] as? String {
            guard let v = AVSpeechSynthesisVoice(identifier: voice) else { return .failure(Refusal(word: "voice-unavailable")) }
            s.voice = v
        } else if let lang = args["lang"] as? String {
            guard let v = AVSpeechSynthesisVoice(language: lang) else { return .failure(Refusal(word: "language-unavailable")) }
            s.voice = v
        }
        if let rate = args["rate"] as? Double {
            s.rate = min(AVSpeechUtteranceMaximumSpeechRate, max(AVSpeechUtteranceMinimumSpeechRate, AVSpeechUtteranceDefaultSpeechRate * Float(rate)))
        }
        if let pitch = args["pitch"] as? Double { s.pitchMultiplier = min(2, max(0.5, Float(pitch))) }
        if let volume = args["volume"] as? Double { s.volume = min(1, max(0, Float(volume))) }
        if let delay = apple["preUtteranceDelay"] as? Double { s.preUtteranceDelay = delay }
        if let delay = apple["postUtteranceDelay"] as? Double { s.postUtteranceDelay = delay }
        s.prefersAssistiveTechnologySettings = apple["prefersAssistiveTechnologySettings"] as? Bool ?? true
        return .success(s)
    }

    /// The session's synthesizer, made at its first utterance.
    private func made() -> AVSpeechSynthesizer {
        if let synthesizer { return synthesizer }
        let s = AVSpeechSynthesizer()
        s.delegate = self
        #if os(iOS) || os(tvOS)
        s.usesApplicationAudioSession = true
        #endif
        Self.synthesizerHatch?(s)
        synthesizer = s
        return s
    }

    // MARK: the queue

    /// `u` begins at session time `at`: the synthesizer is given it, or the agent's clock starts it.
    private func begin(_ u: Utterance, _ at: Double) {
        current = u.id; u.state = "speaking"; u.began = at; u.paused = nil
        boundary = Self.noBoundary; heldBoundary = nil
        if context.agent, let fail { u.failure = fail; self.fail = nil }
        // The claim before the synthesizer, so the owner has activated the session when it speaks (D4).
        reclaim()
        if let s = u.spoken { made().speak(s) }
    }

    /// The next queued utterance begins, unless the queue is paused or one speaks.
    private func proceed(_ at: Double? = nil) {
        if current == nil, let head = queue.first, reasons.isEmpty, let u = utterances[head], !u.resolving {
            queue.removeFirst()
            begin(u, at ?? context.now())
        }
        if current == nil, queue.isEmpty { boundary = Self.noBoundary }
        reclaim()
    }

    /// `u` ends with `outcome` (and `error`) at session time `at`.
    private func end(_ u: Utterance, _ outcome: String, _ error: String?, at: Double? = nil) {
        guard u.state != "ended" else { return }
        u.state = "ended"; u.outcome = outcome; u.error = error; u.spoken = nil
        if outcome == "error", let error { say("\(u.id): \(error)") }
        queue.removeAll { $0 == u.id }
        if current == u.id { current = nil; proceed(at) }
        reclaim()
    }

    /// Under the agent, the queue moved to the session clock's now (§9).
    private func advance() {
        guard context.agent else { return }
        let t = context.now()
        while let id = current, let u = utterances[id] {
            if !reasons.isEmpty { if u.paused == nil { u.paused = t }; return }
            if let p = u.paused { u.began += t - p; u.paused = nil }
            if let failure = u.failure { end(u, "error", failure, at: u.began); continue }
            let per = 60000 / (Self.wpm * (u.rate > 0 ? u.rate : 1)), n = Int(max(0, t - u.began) / per)
            if n >= u.words.count { end(u, "end", nil, at: u.began + Double(u.words.count) * per); continue }
            boundary = ["charIndex": u.words[n].at, "charLength": u.words[n].length, "name": "word", "elapsedTime": Double(n) * per]
            return
        }
    }

    // MARK: the claim (D4)

    /// The strongest type among the utterances not yet ended, `auto` for none.
    private func strongest() -> String {
        let ids = (current.map { [$0] } ?? []) + queue
        let best = ids.compactMap { utterances[$0].flatMap { Self.strength.firstIndex(of: $0.type) } }.max()
        return best.map { Self.strength[$0] } ?? "auto"
    }

    /// Claim while the queue speaks and is not paused; the type chosen again whenever the queue changes. A paused
    /// queue holds no claim (D4).
    private func reclaim() {
        let want = (current != nil || !queue.isEmpty) && reasons.isEmpty ? strongest() : "auto"
        guard want != claimed else { return }
        claim?.cancel(); claim = nil; claimed = want
        guard want != "auto", !context.agent else { return }
        claim = context.audioSession.claim(mode: "speech-\(want)", options: [], background: want == "playback") { [weak self] event in
            self?.heard(event)
        }
        if claim == nil { say("the audio session refused the \(want) claim; speaking in the session as it stands") }
    }

    /// What the owner tells a claim: an interruption pauses the queue until `resumeSpeech` (D4).
    private func heard(_ event: String) {
        if event == "interrupted" || event == "reset" { pause("interrupted") }
    }

    #if os(iOS) || os(tvOS)
    /// Headphones removed pause a `playback` queue (D4; Apple's "Responding to audio route changes").
    private func routeChanged(_ note: Notification) {
        guard let raw = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
              AVAudioSession.RouteChangeReason(rawValue: raw) == .oldDeviceUnavailable,
              let previous = note.userInfo?[AVAudioSessionRouteChangePreviousRouteKey] as? AVAudioSessionRouteDescription,
              previous.outputs.contains(where: { [.headphones, .bluetoothA2DP, .bluetoothLE, .bluetoothHFP].contains($0.portType) }),
              current != nil, strongest() == "playback" else { return }
        pause("route")
    }
    #endif

    // MARK: values

    private func state() -> [String: Any] {
        advance()
        var out: [String: Any] = [
            "available": context.agent || catalog.map { !$0.isEmpty } ?? true,
            "speaking": current != nil, "pending": !queue.isEmpty, "paused": !reasons.isEmpty, "pauseReasons": reasons,
            "audioSessionType": claimed, "utterance": current ?? NSNull(), "queue": queue,
        ]
        out.merge(boundary) { $1 }
        if !context.agent, #available(iOS 17, macOS 14, tvOS 17, *) {
            let words: [AVSpeechSynthesizer.PersonalVoiceAuthorizationStatus: String] = [.notDetermined: "notDetermined", .denied: "denied", .unsupported: "unsupported", .authorized: "authorized"]
            out["apple"] = ["personalVoiceAuthorizationStatus": words[AVSpeechSynthesizer.personalVoiceAuthorizationStatus] ?? "notDetermined"]
        } else {
            out["apple"] = NSNull()
        }
        return out
    }

    private func shape(_ u: Utterance) -> [String: Any] {
        ["id": u.id, "state": u.state == "speaking" && !reasons.isEmpty ? "paused" : u.state, "outcome": u.outcome ?? NSNull(), "error": u.error ?? NSNull()]
    }

    private static func agentVoices() -> [[String: Any]] {
        let base = Locale.preferredLanguages.first ?? "en-US"
        return [
            ["voiceURI": "agent:en-US", "name": "Agent voice", "lang": "en-US", "localService": true, "default": true, "apple": NSNull(), "android": NSNull()],
            ["voiceURI": "agent:\(base)", "name": "Agent voice (\(base))", "lang": base, "localService": true, "default": false, "apple": NSNull(), "android": NSNull()],
        ]
    }

    /// The catalog read again on `voiceQueue`; the requests waiting on it are answered when it lands.
    private func readVoices() {
        guard !reading else { return }
        reading = true
        voiceQueue.async { [weak self] in
            let list = Self.voices()
            DispatchQueue.main.async {
                guard let self else { return }
                self.reading = false
                let changed = self.catalog != nil
                self.catalog = list
                let waiting = self.catalogWaiters
                self.catalogWaiters = []
                waiting.forEach { $0(list) }
                if changed { self.context.changed("speech-synthesis.speechSynthesisVoices") }
                self.announce()
            }
        }
    }

    private static func voices() -> [[String: Any]] {
        let fallback = AVSpeechSynthesisVoice(language: nil)?.identifier
        let qualities: [AVSpeechSynthesisVoiceQuality: String] = [.default: "default", .enhanced: "enhanced", .premium: "premium"]
        let genders: [AVSpeechSynthesisVoiceGender: String] = [.unspecified: "unspecified", .male: "male", .female: "female"]
        return AVSpeechSynthesisVoice.speechVoices().map { v in
            var apple: [String: Any] = ["quality": qualities[v.quality] ?? "default", "gender": genders[v.gender] ?? "unspecified",
                                        "isPersonalVoice": false, "isNoveltyVoice": false]
            if #available(iOS 17, macOS 14, tvOS 17, *) {
                apple["isPersonalVoice"] = v.voiceTraits.contains(.isPersonalVoice)
                apple["isNoveltyVoice"] = v.voiceTraits.contains(.isNoveltyVoice)
            }
            return ["voiceURI": v.identifier, "name": v.name, "lang": v.language, "localService": true, "default": v.identifier == fallback,
                    "apple": apple, "android": NSNull()]
        }
    }

    // MARK: pause, resume, cancel

    /// Every utterance still to end ends: the queued `canceled`, the speaking `interrupted` (D6).
    private func cancelAll() {
        advance()
        for id in queue { if let u = utterances[id] { u.state = "ended"; u.outcome = "error"; u.error = "canceled"; u.spoken = nil } }
        queue = []
        if let id = current, let u = utterances[id] { u.state = "ended"; u.outcome = "error"; u.error = "interrupted"; u.spoken = nil }
        current = nil
        reasons.removeAll { $0 == "requested" }
        boundary = Self.noBoundary
        synthesizer?.stopSpeaking(at: .immediate)
        reclaim()
        announce()
    }

    private func pause(_ reason: String) {
        advance()
        if let id = current, let u = utterances[id], u.paused == nil { u.paused = context.now() }
        if !reasons.contains(reason) { reasons.append(reason) }
        synthesizer?.pauseSpeaking(at: .word)
        reclaim()
        announce()
    }

    private func resume(_ lifted: [String]) {
        advance()
        reasons.removeAll { lifted.contains($0) }
        if reasons.isEmpty {
            // The claim again before the synthesizer goes on (D4).
            reclaim()
            if let held = heldBoundary { boundary = held; heldBoundary = nil }
            if synthesizer?.isPaused == true { synthesizer?.continueSpeaking() }
            proceed()
        }
        reclaim()
        announce()
    }

    // MARK: the delegate, on the synthesizer's queue

    private func on(_ spoken: AVSpeechUtterance, _ f: @escaping (Utterance) -> Void) {
        DispatchQueue.main.async { [weak self] in
            guard let self, let id = self.current, let u = self.utterances[id], u.spoken === spoken else { return }
            f(u)
            self.announce()
        }
    }

    public func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, willSpeakRangeOfSpeechString range: NSRange, utterance: AVSpeechUtterance) {
        on(utterance) { [weak self] u in
            guard let self else { return }
            self.reached(["charIndex": range.location, "charLength": range.length, "name": "word", "elapsedTime": max(0, self.context.now() - u.began)])
        }
    }

    @available(iOS 17, macOS 14, tvOS 17, *)
    public func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, willSpeak marker: AVSpeechSynthesisMarker, utterance: AVSpeechUtterance) {
        guard marker.mark == .bookmark else { return }
        let name = marker.bookmarkName, at = marker.textRange.location
        on(utterance) { [weak self] u in
            guard let self else { return }
            self.reached(["charIndex": at, "charLength": 0, "name": name, "elapsedTime": max(0, self.context.now() - u.began)])
        }
    }

    /// A boundary the synthesizer reports after a pause (iOS delivers the next word's as it stops) is the word
    /// spoken at resume: it is shown then, so a paused queue's mark holds (D6).
    private func reached(_ b: [String: Any]) {
        if reasons.isEmpty { boundary = b } else { heldBoundary = b }
    }

    public func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didFinish utterance: AVSpeechUtterance) {
        on(utterance) { [weak self] u in self?.end(u, "end", nil) }
    }

    // MARK: the session

    /// Under the agent: `fail` makes the next utterance to begin end with that error (§9). Each value preferred
    /// arms it once; the queue is first brought to now, so the one speaking is not the one that fails.
    public func preferred(_ values: [String: Any]) {
        let word = values["fail"] as? String
        guard word != failSeen else { return }
        failSeen = word
        guard let word else { return }
        advance()
        fail = word
        say("the next utterance fails \(word)")
    }

    /// Utterances whose reply was dropped: never the app's (D2).
    public func dispose(_ ids: [String]) {
        advance()
        for id in ids {
            guard let u = utterances[id], u.state != "ended" else { continue }
            say("\(id): its reply was dropped")
            let speaking = current == id
            if speaking { synthesizer?.stopSpeaking(at: .immediate) }
            end(u, "error", speaking ? "interrupted" : "canceled")
        }
        announce()
    }

    /// A prompt the person can no longer see the reason for is canceled at the background (D4); a `playback`
    /// queue speaks on only where the app may play in the background.
    public func foreground(_ shown: Bool) {
        if shown {
            if reasons.contains("background") { resume(["background"]) }
            return
        }
        guard current != nil || !queue.isEmpty else { return }
        if strongest() != "playback" { say("left the foreground: the queue is canceled"); return cancelAll() }
        #if os(iOS)
        let background = (Bundle.main.object(forInfoDictionaryKey: "UIBackgroundModes") as? [String] ?? []).contains("audio")
        if !background { pause("background") }
        #endif
    }

    /// The session ends: the synthesizer stops and the claim goes (D6).
    public func destroy(_ done: @escaping () -> Void) {
        cancelAll()
        synthesizer?.delegate = nil
        synthesizer = nil
        claim?.cancel(); claim = nil
        done()
    }
}
