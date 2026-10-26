// @exact/speech-recognition on Apple (LLP 1133.012 D7–D11, §9): Speech's
// recognizers, answered in the interface's shapes.
//
// - The engine (D9): on iOS and macOS 26 and later `SpeechAnalyzer` with
//   `SpeechTranscriber` where it is available, else `DictationTranscriber`,
//   both on the device; before 26, or with `apple.engine=
//   "sfSpeechRecognizer"`, `SFSpeechRecognizer`, which may send audio to
//   Apple unless `processLocally` asks for the device.
// - Inputs (D8): `track=` is a consumer of a track `@exact/media-capture`
//   opened, holding its lease like a recorder, and its start is an app's
//   explicit start, which reopens an idle track; its buffers are converted
//   to the analyzer's format on a queue of their own. `path=` transcribes an
//   `app:/` file to its end. `microphone=true` is not built here: the status
//   lists `track` and `file`, and a microphone start is `NotSupportedError`.
// - Permission (D11): the analyzer engines need no authorization and read
//   `granted`; `SFSpeechRecognizer` reads its authorization status and asks
//   at a start that needs it.
// - A recognition is an id (D7), its state a value: final results numbered
//   from 0, the last `history` of them (20 by default, 200 at most, 256 KB)
//   with the interim one. Every final goes, as it arrives, to the
//   `transcript=` directory as JSON Lines files of at most 1 MiB, the
//   directory emptied when the recognition starts.
// - A reply the runner drops aborts its recognition; leaving the foreground
//   aborts every one (D10), its finals so far kept.
// - Under the agent no recognizer is made (§9), as on the web: a
//   recognition hears what the drive says (`heard`), one word as an interim
//   result every 300 ms of the session clock and the final 500 ms after the
//   last word; a non-continuous one then ends, and with no phrase ends
//   `no-speech` after 8 s; a `path=` one answers the phrase preferred at its
//   start, at once.
import AVFoundation
import ExactPackage
import Foundation
import Speech

public final class SpeechRecognitionModule: ExactPackage {
    static let history = 20, maxHistory = 200, windowBytes = 256 * 1024, fileBytes = 1 << 20, split = 16 * 1024
    static let wordMs = 300.0, finalMs = 500.0, noSpeechMs = 8000.0
    static let agentStatus: [String: Any] = ["available": true, "state": "granted", "inputs": ["track", "microphone", "file"], "local": true,
                                             "apple": NSNull(), "android": NSNull()]
    static let noInfo: [String: Any] = ["id": "", "leavesDevice": false, "finalCount": 0, "transcript": NSNull(), "files": 0, "error": NSNull()]
    static let noRecognition: [String: Any] = ["id": "", "state": "ended", "audio": false, "sound": false, "speech": false, "results": [Any](),
                                               "finalCount": 0, "interim": NSNull(), "leavesDevice": false, "error": NSNull(), "message": NSNull()]

    let context: ExactPackageContext
    private var recognitions: [String: Recognition] = [:]
    private var phrases: [(text: String, words: [String], at: Double)] = []
    private var overlay: [String: Any] = [:], heard: String?
    private var next = 0

    public init(context: ExactPackageContext) { self.context = context }

    func say(_ line: String) { context.log("speech-recognition: \(line)") }
    func announce() { context.changed("speech-recognition.speechRecognitionState") }

    // MARK: the names

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        let r = (args["recognition"] as? String).flatMap { recognitions[$0] }
        let none = (args["recognition"] as? String ?? "").isEmpty
        switch name {
        case "startSpeechRecognition": start(args, reply)
        case "stopSpeechRecognition":
            if none { return reply(Self.noInfo) }
            guard let r else { return reply(["error": "NotFoundError"]) }
            advance(r)
            r.stop(reply)
        case "abortSpeechRecognition":
            // An abort of no recognition answers none: an app supersedes a start still in flight with it.
            if none { return reply(Self.noInfo) }
            guard let r else { return reply(["error": "NotFoundError"]) }
            advance(r)
            r.abort("aborted") { reply($0) }
        case "speechRecognitionState":
            if none { return reply(Self.noRecognition) }
            guard let r else { return reply(["error": "NotFoundError"]) }
            advance(r)
            reply(r.shape())
        case "speechRecognitionStatus": reply(status(args))
        case "speechRecognitionAvailability": availability(args, reply)
        case "installSpeechRecognition": install(args, reply)
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "speechRecognitionStatusSync" ? status(args) : ["error": "NotSupportedError"]
    }

    // MARK: the status (D7, D8, D11)

    /// The engine `args` select (D9).
    static func engine(_ args: [String: Any]) -> String {
        if let written = (args["apple"] as? [String: Any])?["engine"] as? String { return written }
        if #available(iOS 26, macOS 26, *) { return SpeechTranscriber.isAvailable ? "speechTranscriber" : "dictationTranscriber" }
        return "sfSpeechRecognizer"
    }

    static func locale(_ args: [String: Any]) -> Locale {
        (args["lang"] as? String).map { Locale(identifier: $0) } ?? Locale.current
    }

    static func authorization() -> String {
        switch SFSpeechRecognizer.authorizationStatus() {
        case .authorized: "authorized"
        case .denied: "denied"
        case .restricted: "restricted"
        default: "notDetermined"
        }
    }

    func status(_ args: [String: Any]) -> [String: Any] {
        if context.agent { return Self.agentStatus.merging(overlay) { $1 } }
        let engine = Self.engine(args), local = args["processLocally"] as? Bool == true
        let sf = SFSpeechRecognizer(locale: Self.locale(args))
        var out: [String: Any] = ["inputs": ["track", "file"], "android": NSNull()]
        let auth = Self.authorization()
        if engine == "sfSpeechRecognizer" {
            let state = ["authorized": "granted", "denied": "denied", "restricted": "denied"][auth] ?? "prompt"
            out.merge(["available": sf?.isAvailable ?? false, "state": state, "local": local && sf?.supportsOnDeviceRecognition == true]) { $1 }
        } else {
            // The SpeechAnalyzer engines send no audio to Apple and need no authorization (D11).
            out.merge(["available": true, "state": "granted", "local": true]) { $1 }
        }
        out["apple"] = ["authorizationStatus": auth, "supportsOnDeviceRecognition": sf?.supportsOnDeviceRecognition ?? false, "engine": engine]
        return out
    }

    private func availability(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let s = status(args), local = args["processLocally"] as? Bool == true
        if context.agent { return reply(["availability": s["available"] as? Bool == true && (s["local"] as? Bool == true || !local) ? "available" : "unavailable"]) }
        let requested = (args["langs"] as? [String] ?? []).map(Locale.init(identifier:))
        let langs = requested.isEmpty ? [Locale.current] : requested
        guard #available(iOS 26, macOS 26, *), Self.engine(args) != "sfSpeechRecognizer" else {
            let sf = langs.map { SFSpeechRecognizer(locale: $0) }
            let ok = !sf.isEmpty && sf.allSatisfy { $0 != nil && (!local || $0!.supportsOnDeviceRecognition) }
            return reply(["availability": ok ? "available" : "unavailable"])
        }
        Task { [weak self] in
            var word = "available"
            for lang in langs {
                guard let module = await AnalyzerEngine.module(Self.engine(args), lang, interim: false, alternatives: false) else { word = "unavailable"; break }
                switch await AssetInventory.status(forModules: [module]) {
                case .installed:
                    // An installed model no analyzer takes audio for is unavailable: the iOS simulator's
                    // DictationTranscriber reports one (SpeechTranscriber is unavailable there).
                    if await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [module]) != nil { continue }
                    word = "unavailable"
                case .downloading: word = word == "available" ? "downloading" : word
                case .supported: word = "downloadable"
                default: word = "unavailable"
                }
                if word == "unavailable" { break }
            }
            let answer = word
            await MainActor.run { _ = self; reply(["availability": answer]) }
        }
    }

    private func install(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        if context.agent { return reply(["installed": status(args)["local"] as? Bool == true]) }
        guard #available(iOS 26, macOS 26, *), Self.engine(args) != "sfSpeechRecognizer" else { return reply(["installed": false]) }
        let langs = (args["langs"] as? [String] ?? []).map(Locale.init(identifier:))
        Task { [weak self] in
            var installed = true
            for lang in langs.isEmpty ? [Locale.current] : langs {
                guard let module = await AnalyzerEngine.module(Self.engine(args), lang, interim: false, alternatives: false) else { installed = false; continue }
                do {
                    if let request = try await AssetInventory.assetInstallationRequest(supporting: [module]) { try await request.downloadAndInstall() }
                } catch {
                    installed = false
                }
            }
            let answer = installed
            await MainActor.run {
                self?.context.changed("speech-recognition.speechRecognitionAvailability")
                reply(["installed": answer])
            }
        }
    }

    // MARK: starting

    private func start(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let given = [args["track"] as? String != nil && !(args["track"] as? String ?? "").isEmpty ? "track" : nil,
                     args["microphone"] as? Bool == true ? "microphone" : nil,
                     args["path"] as? String != nil && !(args["path"] as? String ?? "").isEmpty ? "file" : nil].compactMap { $0 }
        guard given.count == 1, let input = given.first else { return reply(["error": "TypeError"]) }
        let s = status(args)
        guard s["available"] as? Bool == true, (s["inputs"] as? [String] ?? []).contains(input) else { return reply(["error": "NotSupportedError"]) }
        let engine = Self.engine(args), local = args["processLocally"] as? Bool == true
        if input == "track", context.tracks.lookup(args["track"] as! String)?.kind != "audio" { return reply(["error": "InvalidStateError"]) }
        var file: URL?
        if input == "file", !context.agent {
            guard let url = context.file(args["path"] as! String), FileManager.default.fileExists(atPath: url.path) else { return reply(["error": "NotFoundError"]) }
            file = url
        }
        // The SF recognizer asks its authorization at the start that needs it (D11).
        if !context.agent, engine == "sfSpeechRecognizer", SFSpeechRecognizer.authorizationStatus() == .notDetermined {
            return SFSpeechRecognizer.requestAuthorization { _ in DispatchQueue.main.async { [weak self] in self?.start(args, reply) } }
        }
        if !context.agent, s["state"] as? String == "denied" { return reply(["error": "NotAllowedError"]) }
        next += 1
        let leaves = context.agent ? !local && s["local"] as? Bool != true : engine == "sfSpeechRecognizer" && !local
        let r = Recognition(id: "recognition-\(context.session)-\(next)", input: input, args: args, leavesDevice: leaves, module: self)
        r.engineWord = engine
        if let path = args["transcript"] as? String, let url = context.file(path) { r.transcript = (path, url) }
        // `processLocally` where the recognizer cannot keep audio on the device (D9) opens nothing.
        let refused = local && s["local"] as? Bool != true
        r.prepareTranscript()
        recognitions[r.id] = r
        say("\(r.id): \(input == "track" ? "on \(args["track"]!)" : "of \(args["path"]!)")\(r.transcript.map { ", transcript \($0.path)" } ?? ""), \(leaves ? "may leave the device" : "on the device"), \(engine)")
        if refused { r.finish("service-not-allowed"); return reply(r.info()) }
        if input == "track" {
            do {
                r.subscription = try context.tracks.subscribe(args["track"] as! String, r, start: true, origin: "app")
            } catch {
                recognitions[r.id] = nil
                return reply(["error": (error as? ExactTrackError)?.code ?? "InvalidStateError"])
            }
            if r.state == "ended" { return reply(["error": "InvalidStateError"]) }
        }
        if context.agent {
            r.state = "listening"; r.audio = input != "file"; r.sound = r.audio
            if input == "file" {
                if let heard { r.final([(heard, 0.9)], 0, Self.wordMs * Double(heard.split(separator: " ").count)) }
                r.finish(nil)
            }
            announce()
            return reply(r.info())
        }
        r.state = "listening"
        r.engine = makeEngine(engine, r, args, file)
        if r.engine == nil { r.finish("language-not-supported", "no recognizer for \(Self.locale(args).identifier)") }
        announce()
        reply(r.info())
    }

    private func makeEngine(_ word: String, _ r: Recognition, _ args: [String: Any], _ file: URL?) -> RecognitionEngine? {
        if word != "sfSpeechRecognizer", #available(iOS 26, macOS 26, *) { return AnalyzerEngine(word, r, args, file: file) }
        return RecognizerEngine(r, args, file: file)
    }

    // MARK: the agent's hearing (§9)

    /// `r` moved to the session clock's now: the words of each phrase heard since it began.
    func advance(_ r: Recognition) {
        guard context.agent, r.state == "listening", r.input != "file" else { return }
        let t = context.now()
        while r.heard < phrases.count {
            let p = phrases[r.heard]
            if p.at < r.began { r.heard += 1; continue }
            let from = max(p.at, r.cursor), n = Double(p.words.count), last = from + Self.wordMs * n, done = last + Self.finalMs
            if done <= t {
                r.final([(p.text, 0.9)], from, last)
                r.cursor = done
                r.heard += 1
                if !r.continuous { r.finish(nil); return }
                continue
            }
            let count = min(p.words.count, Int((t - from) / Self.wordMs))
            r.speech = count > 0
            r.interim = count > 0 && r.interimResults
                ? r.result(r.finalCount, [(p.words.prefix(count).joined(separator: " "), 0.9)], from, from + Self.wordMs * Double(count)) : nil
            return
        }
        r.speech = false
        if !r.continuous, r.finalCount == 0, t - r.began >= Self.noSpeechMs { r.finish("no-speech") }
    }

    // MARK: the session

    /// Under the agent: the preferred status (laid over the default, as the runner answers it) and the phrase heard.
    public func preferred(_ values: [String: Any]) {
        overlay = (values["speechRecognitionStatus"] as? [String: Any]).flatMap { $0["error"] == nil ? $0 : nil } ?? [:]
        let phrase = values["heard"] as? String
        guard phrase != heard else { return }
        heard = phrase
        for r in recognitions.values { advance(r) }
        if let phrase, !phrase.isEmpty {
            phrases.append((phrase, phrase.split(whereSeparator: \.isWhitespace).map(String.init), context.now()))
            say("heard: \(phrase)")
        }
        announce()
    }

    /// Recognitions whose reply was dropped: aborted, never the app's (D7).
    public func dispose(_ ids: [String]) {
        for id in ids {
            guard let r = recognitions[id], r.state != "ended" else { continue }
            say("\(id): its reply was dropped; aborted")
            r.abort("aborted") { _ in }
        }
    }

    /// Recognition is a foreground activity: leaving the foreground aborts every one (D10).
    public func foreground(_ shown: Bool) {
        guard !shown else { return }
        for r in recognitions.values {
            advance(r)
            guard r.state != "ended" else { continue }
            say("\(r.id): left the foreground; aborted")
            r.abort("aborted") { _ in }
        }
    }

    public func destroy(_ done: @escaping () -> Void) {
        let group = DispatchGroup()
        for r in recognitions.values where r.state != "ended" {
            group.enter()
            r.abort("aborted") { _ in group.leave() }
        }
        group.notify(queue: .main, execute: done)
    }
}

/// What a recognizer does for a recognition: hear a track's capture, stop at the end of what it has, or abort.
protocol RecognitionEngine: AnyObject {
    func live(_ source: ExactAudioSource)
    func stop()
    func abort()
}

/// One recognition: a consumer of its track, its results, and its transcript directory. Main thread.
final class Recognition: ExactTrackConsumer {
    let id: String, input: String, continuous: Bool, interimResults: Bool, history: Int, maxAlternatives: Int, leavesDevice: Bool
    var engineWord = ""
    var state = "starting", audio = false, sound = false, speech = false
    var results: [[String: Any]] = [], finalCount = 0, interim: [String: Any]?
    var error: String?, message: String?
    var transcript: (path: String, url: URL)?
    var files = 0, fileSize = 0
    var subscription: ExactSubscription?
    var engine: RecognitionEngine? {
        didSet { if let engine, let source { engine.live(source) } }
    }
    /// The track's running capture, which an engine made after the subscription is given.
    private var source: ExactAudioSource?
    // The agent's clock (§9).
    let began: Double
    var cursor: Double, heard = 0
    private unowned let module: SpeechRecognitionModule
    private let writer: DispatchQueue
    private var waiting: [([String: Any]) -> Void] = []
    private var stopping = false

    init(id: String, input: String, args: [String: Any], leavesDevice: Bool, module: SpeechRecognitionModule) {
        self.id = id; self.input = input; self.leavesDevice = leavesDevice; self.module = module
        continuous = args["continuous"] as? Bool == true
        interimResults = args["interimResults"] as? Bool == true
        history = max(1, min(SpeechRecognitionModule.maxHistory, (args["history"] as? Double).map { Int($0) } ?? SpeechRecognitionModule.history))
        maxAlternatives = max(1, min(10, (args["maxAlternatives"] as? Double).map { Int($0) } ?? 1))
        began = module.context.now(); cursor = began
        writer = DispatchQueue(label: "exact.speech-recognition.\(id)")
    }

    // MARK: values

    func info() -> [String: Any] {
        ["id": id, "leavesDevice": leavesDevice, "finalCount": finalCount, "transcript": transcript?.path ?? NSNull(), "files": files,
         "error": state == "ended" ? error ?? NSNull() : NSNull()]
    }

    func shape() -> [String: Any] {
        var window = Array(results.suffix(history))
        while window.count > 1, ((try? JSONSerialization.data(withJSONObject: window))?.count ?? 0) > SpeechRecognitionModule.windowBytes { window.removeFirst() }
        return ["id": id, "state": state, "audio": audio, "sound": sound, "speech": speech, "results": window, "finalCount": finalCount,
                "interim": interim ?? NSNull(), "leavesDevice": leavesDevice, "error": error ?? NSNull(), "message": message ?? NSNull()]
    }

    func result(_ index: Int, _ alternatives: [(String, Double)], _ start: Double?, _ end: Double?) -> [String: Any] {
        ["index": index, "alternatives": alternatives.prefix(maxAlternatives).map { ["transcript": $0.0, "confidence": $0.1] },
         "start": start ?? NSNull(), "end": end ?? NSNull(),
         "apple": module.context.agent ? NSNull() : ["engine": engineWord]]
    }

    /// A final result (best first): numbered, kept in the window, appended to the transcript; a text over 16 KB
    /// in pieces split at a word boundary (D7).
    func final(_ alternatives: [(String, Double)], _ start: Double?, _ end: Double?) {
        let pieces = Self.pieces(alternatives.first?.0 ?? "")
        for piece in pieces {
            let r = result(finalCount, pieces.count == 1 ? alternatives : [(piece, alternatives.first?.1 ?? 0)], start, end)
            finalCount += 1
            results.append(r)
            if results.count > SpeechRecognitionModule.maxHistory { results.removeFirst() }
            write(r)
        }
        interim = nil
    }

    static func pieces(_ text: String) -> [String] {
        var out: [String] = [], rest = Substring(text)
        while rest.utf8.count > SpeechRecognitionModule.split {
            let limit = rest.utf8.index(rest.startIndex, offsetBy: SpeechRecognitionModule.split)
            let cut = rest[..<limit].lastIndex(of: " ") ?? limit
            out.append(String(rest[..<cut]))
            rest = rest[cut...].drop { $0 == " " }
        }
        out.append(String(rest))
        return out
    }

    // MARK: the transcript (D7, D12 item 7)

    /// The directory, emptied as the recognition starts, so a replacement leaves no stale file.
    func prepareTranscript() {
        guard let url = transcript?.url else { return }
        writer.async {
            try? FileManager.default.removeItem(at: url)
            try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        }
    }

    private func write(_ result: [String: Any]) {
        guard let url = transcript?.url, var line = try? JSONSerialization.data(withJSONObject: result, options: [.sortedKeys]) else { return }
        line.append(0x0A)
        if files == 0 || fileSize + line.count > SpeechRecognitionModule.fileBytes { files += 1; fileSize = 0 }
        fileSize += line.count
        let file = url.appendingPathComponent(String(format: "%05d.jsonl", files - 1)), id = id, say = module.say
        writer.async {
            if let handle = try? FileHandle(forWritingTo: file) {
                handle.seekToEndOfFile(); handle.write(line); try? handle.close()
            } else if !FileManager.default.createFile(atPath: file.path, contents: line) {
                DispatchQueue.main.async { say("\(id): \(file.lastPathComponent) not written") }
            }
        }
    }

    // MARK: ending

    /// `r` ends (with `error`): its input let go, its files finished; sticky (D7). Every stop that waited is answered.
    func finish(_ why: String?, _ text: String? = nil) {
        guard state != "ended" else { return }
        state = "ended"; error = error ?? why; message = message ?? text
        audio = false; sound = false; speech = false
        subscription?.cancel(); subscription = nil
        engine = nil; source = nil
        module.say("\(id): ended\(error.map { " (\($0))" } ?? "")\(message.map { ": \($0)" } ?? ""), \(finalCount) final results")
        module.announce()
        writer.async { [self] in DispatchQueue.main.async { [self] in let i = info(); for f in waiting { f(i) }; waiting = [] } }
    }

    /// Answered once the recognition has ended and its files are finished.
    func whenDone(_ f: @escaping ([String: Any]) -> Void) {
        if state == "ended" { writer.async { DispatchQueue.main.async { f(self.info()) } } } else { waiting.append(f) }
    }

    /// What was heard so far is final; the recognizer finishes what it has, then the recognition ends.
    func stop(_ reply: @escaping ([String: Any]) -> Void) {
        whenDone(reply)
        guard state != "ended", !stopping else { return }
        stopping = true
        if module.context.agent {
            if let interim, let alts = interim["alternatives"] as? [[String: Any]] {
                final(alts.map { ($0["transcript"] as? String ?? "", $0["confidence"] as? Double ?? 0) }, interim["start"] as? Double, interim["end"] as? Double)
            }
            return finish(nil)
        }
        state = "stopping"
        module.announce()
        if let engine { engine.stop() } else { finish(nil) }
    }

    func abort(_ why: String, _ reply: @escaping ([String: Any]) -> Void) {
        whenDone(reply)
        guard state != "ended" else { return }
        interim = nil
        engine?.abort()
        finish(why)
    }

    /// A seam where the recognizer started a new request (D9), journaled.
    func seam(_ why: String) { module.say("\(id): a new request (\(why))") }

    /// An engine's result, on main.
    func recognized(_ alternatives: [(String, Double)], final isFinal: Bool, _ start: Double?, _ end: Double?) {
        guard state != "ended" else { return }
        if isFinal {
            final(alternatives, start, end)
            if !continuous, input != "file" { stop { _ in } }
        } else {
            interim = interimResults ? result(finalCount, alternatives, start, end) : nil
        }
        speech = !isFinal
        module.announce()
    }

    /// An engine ended, on main: `error` nil when it finished what it had.
    func engineEnded(_ error: String?, _ text: String? = nil) {
        if let error, error != "aborted" { module.say("\(id): \(error)\(text.map { ": \($0)" } ?? "")") }
        finish(error, text)
    }

    // MARK: the track (D8, D10)

    func live(_ source: ExactAudioSource?) {
        audio = true; sound = true
        self.source = source
        if let source { engine?.live(source) }
        module.announce()
    }
    func muted(_ reason: String) {}
    func unmuted() {}
    func ended() {
        guard state != "ended" else { return }
        interim = nil
        engine?.abort()
        finish(nil)
    }
}

// MARK: SpeechAnalyzer (iOS, macOS 26)

@available(iOS 26, macOS 26, *)
final class AnalyzerEngine: RecognitionEngine {
    private weak var r: Recognition?
    private var analyzer: SpeechAnalyzer?
    private var input: AsyncStream<AnalyzerInput>.Continuation?
    private var tap: ExactSubscription?, source: ExactAudioSource?
    private var ready = false, aborted = false
    private let queue = DispatchQueue(label: "exact.speech-recognition.convert")

    /// The transcriber `word` names for `locale`, or nil when it supports no equivalent locale.
    static func module(_ word: String, _ locale: Locale, interim: Bool, alternatives: Bool) async -> (any SpeechModule)? {
        if word == "dictationTranscriber" {
            guard let l = await DictationTranscriber.supportedLocale(equivalentTo: locale) else { return nil }
            var reporting: Set<DictationTranscriber.ReportingOption> = []
            if interim { reporting.insert(.volatileResults) }
            if alternatives { reporting.insert(.alternativeTranscriptions) }
            return DictationTranscriber(locale: l, contentHints: [], transcriptionOptions: [.punctuation], reportingOptions: reporting,
                                        attributeOptions: [.audioTimeRange, .transcriptionConfidence])
        }
        guard let l = await SpeechTranscriber.supportedLocale(equivalentTo: locale) else { return nil }
        var reporting: Set<SpeechTranscriber.ReportingOption> = []
        if interim { reporting.insert(.volatileResults) }
        if alternatives { reporting.insert(.alternativeTranscriptions) }
        return SpeechTranscriber(locale: l, transcriptionOptions: [], reportingOptions: reporting, attributeOptions: [.audioTimeRange, .transcriptionConfidence])
    }

    init(_ word: String, _ r: Recognition, _ args: [String: Any], file: URL?) {
        self.r = r
        let locale = SpeechRecognitionModule.locale(args), interim = r.interimResults, alternatives = r.maxAlternatives > 1
        let phrases = (args["phrases"] as? [[String: Any]] ?? []).compactMap { $0["phrase"] as? String }
        Task { [weak self] in
            guard let module = await Self.module(word, locale, interim: interim, alternatives: alternatives) else {
                return await self?.end("language-not-supported", "no \(word) for \(locale.identifier)")
            }
            guard await AssetInventory.status(forModules: [module]) == .installed else {
                return await self?.end("language-not-supported", "the \(locale.identifier) model is not installed (installSpeechRecognition)")
            }
            guard let format = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [module]) else {
                return await self?.end("language-not-supported", "no analyzer takes audio for the \(locale.identifier) \(word) model here")
            }
            let context = AnalysisContext()
            if !phrases.isEmpty { context.contextualStrings[.general] = phrases }
            do {
                let analyzer: SpeechAnalyzer
                if let file {
                    analyzer = try await SpeechAnalyzer(inputAudioFile: AVAudioFile(forReading: file), modules: [module], analysisContext: context, finishAfterFile: true)
                } else {
                    let (stream, input) = AsyncStream.makeStream(of: AnalyzerInput.self)
                    analyzer = SpeechAnalyzer(modules: [module])
                    try await analyzer.setContext(context)
                    try await analyzer.start(inputSequence: stream)
                    await self?.began(analyzer, input, format)
                }
                await self?.hold(analyzer)
                try await self?.results(module)
                await self?.end(nil)
            } catch {
                await self?.end(self?.aborted == true ? "aborted" : "audio-capture", "\(error)")
            }
        }
    }

    @MainActor private func hold(_ analyzer: SpeechAnalyzer) { self.analyzer = analyzer }

    @MainActor private func began(_ analyzer: SpeechAnalyzer, _ input: AsyncStream<AnalyzerInput>.Continuation, _ format: AVAudioFormat) {
        self.analyzer = analyzer; self.input = input
        ready = true
        if aborted { input.finish(); return }
        if let source { attach(source, format) } else { self.format = format }
    }
    private var format: AVAudioFormat?

    /// Each buffer of the capture, copied off the audio thread and converted on `queue` to the analyzer's format.
    private func attach(_ source: ExactAudioSource, _ format: AVAudioFormat) {
        guard tap == nil, let input, let converter = AVAudioConverter(from: source.format, to: format) else { return }
        let queue = queue
        tap = source.tap { buffer, _ in
            guard let copy = Self.copy(buffer) else { return }
            queue.async {
                let ratio = format.sampleRate / copy.format.sampleRate
                guard let out = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(Double(copy.frameLength) * ratio) + 16) else { return }
                var fed = false
                _ = converter.convert(to: out, error: nil) { _, status in
                    if fed { status.pointee = .noDataNow; return nil }
                    fed = true; status.pointee = .haveData; return copy
                }
                if out.frameLength > 0 { input.yield(AnalyzerInput(buffer: out)) }
            }
        }
    }

    static func copy(_ buffer: AVAudioPCMBuffer) -> AVAudioPCMBuffer? {
        guard let out = AVAudioPCMBuffer(pcmFormat: buffer.format, frameCapacity: buffer.frameLength) else { return nil }
        out.frameLength = buffer.frameLength
        let from = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: buffer.audioBufferList))
        let to = UnsafeMutableAudioBufferListPointer(out.mutableAudioBufferList)
        for (a, b) in zip(from, to) where a.mData != nil && b.mData != nil { memcpy(b.mData, a.mData, Int(min(a.mDataByteSize, b.mDataByteSize))) }
        return out
    }

    private func results(_ module: any SpeechModule) async throws {
        if let m = module as? SpeechTranscriber {
            for try await result in m.results { await deliver(result.text, result.alternatives, result.isFinal, result.range) }
        } else if let m = module as? DictationTranscriber {
            for try await result in m.results { await deliver(result.text, result.alternatives, result.isFinal, result.range) }
        }
    }

    @MainActor private func deliver(_ text: AttributedString, _ alternatives: [AttributedString], _ isFinal: Bool, _ range: CMTimeRange) {
        let confidence = Self.confidence(text), best = String(text.characters)
        let rest = alternatives.map { String($0.characters) }.filter { $0 != best }.map { ($0, 0.0) }
        let start = range.start.isNumeric ? range.start.seconds * 1000 : nil, end = range.end.isNumeric ? range.end.seconds * 1000 : nil
        r?.recognized([(best, confidence)] + rest, final: isFinal, start, end)
    }

    /// The mean of the text's runs' confidence, 0 where none is given.
    static func confidence(_ text: AttributedString) -> Double {
        let values = text.runs.compactMap { $0[AttributeScopes.SpeechAttributes.ConfidenceAttribute.self] }
        return values.isEmpty ? 0 : values.reduce(0, +) / Double(values.count)
    }

    @MainActor private func end(_ error: String?, _ text: String? = nil) {
        tap?.cancel(); tap = nil
        r?.engineEnded(error, text)
    }

    func live(_ source: ExactAudioSource) {
        self.source = source
        if ready, let format { attach(source, format) }
    }

    func stop() {
        tap?.cancel(); tap = nil
        input?.finish()
        let analyzer = analyzer
        Task { try? await analyzer?.finalizeAndFinishThroughEndOfInput() }
    }

    func abort() {
        aborted = true
        tap?.cancel(); tap = nil
        input?.finish()
        let analyzer = analyzer
        Task { await analyzer?.cancelAndFinishNow() }
    }
}

// MARK: SFSpeechRecognizer (before 26, or `apple.engine`)

final class RecognizerEngine: RecognitionEngine {
    /// A request's audio before a new request begins (D9): Apple ends a task at about a minute.
    static let segment = 50.0
    private weak var r: Recognition?
    private let recognizer: SFSpeechRecognizer?
    private let args: [String: Any], file: URL?
    private var request: SFSpeechAudioBufferRecognitionRequest?, task: SFSpeechRecognitionTask?
    private var tap: ExactSubscription?
    private var stopping = false, aborted = false
    private let lock = NSLock()
    private var segmentStart = 0.0

    init(_ r: Recognition, _ args: [String: Any], file: URL?) {
        self.r = r; self.args = args; self.file = file
        recognizer = SFSpeechRecognizer(locale: SpeechRecognitionModule.locale(args))
        guard recognizer?.isAvailable == true else {
            DispatchQueue.main.async { r.engineEnded("language-not-supported", "SFSpeechRecognizer is not available for this language") }
            return
        }
        begin()
        if let file { DispatchQueue.global(qos: .userInitiated).async { [weak self] in self?.read(file) } }
    }

    private func begin() {
        let request = SFSpeechAudioBufferRecognitionRequest()
        let apple = args["apple"] as? [String: Any] ?? [:]
        request.shouldReportPartialResults = r?.interimResults ?? false
        request.requiresOnDeviceRecognition = args["processLocally"] as? Bool == true
        request.contextualStrings = (args["phrases"] as? [[String: Any]] ?? []).compactMap { $0["phrase"] as? String }
        if let punctuation = apple["addsPunctuation"] as? Bool { request.addsPunctuation = punctuation }
        let hints: [String: SFSpeechRecognitionTaskHint] = ["unspecified": .unspecified, "dictation": .dictation, "search": .search, "confirmation": .confirmation]
        if let hint = (apple["taskHint"] as? String).flatMap({ hints[$0] }) { request.taskHint = hint }
        lock.lock(); self.request = request; lock.unlock()
        let offset = segmentStart
        task = recognizer?.recognitionTask(with: request) { [weak self] result, error in
            DispatchQueue.main.async { self?.answered(result, error, offset) }
        }
    }

    private func answered(_ result: SFSpeechRecognitionResult?, _ error: Error?, _ offset: Double) {
        guard let r, !aborted else { return }
        if let result {
            let alternatives = result.transcriptions.map { t in
                (t.formattedString, t.segments.isEmpty ? 0 : t.segments.map { Double($0.confidence) }.reduce(0, +) / Double(t.segments.count))
            }
            let segments = result.bestTranscription.segments
            let start = segments.first.map { offset + $0.timestamp * 1000 }, end = segments.last.map { offset + ($0.timestamp + $0.duration) * 1000 }
            r.recognized(alternatives, final: result.isFinal, start, end)
            if result.isFinal { segmentEnded() }
        } else if let error {
            let code = (error as NSError).code
            // 1110: no speech detected; 301: the request was canceled.
            if code == 1110, r.continuous, !stopping { return segmentEnded() }
            r.engineEnded(code == 1110 ? "no-speech" : code == 301 ? "aborted" : r.leavesDevice ? "network" : "audio-capture", error.localizedDescription)
        }
    }

    /// A request ended: the recognition goes on in a new one, or ends.
    private func segmentEnded() {
        guard let r, r.state != "ended" else { return }
        if stopping || (!r.continuous && file == nil) { return r.engineEnded(nil) }
        if file != nil, readDone { return r.engineEnded(nil) }
        r.seam("the request ended")
        begin()
    }

    // The file, read in segments of under a minute each (D9).
    private var readDone = false
    private func read(_ url: URL) {
        guard let f = try? AVAudioFile(forReading: url) else {
            return DispatchQueue.main.async { [weak self] in self?.r?.engineEnded("audio-capture", "the file did not open") }
        }
        let format = f.processingFormat, chunk = AVAudioFrameCount(format.sampleRate)
        var inSegment = 0.0
        while !aborted, let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: chunk), (try? f.read(into: buffer)) != nil, buffer.frameLength > 0 {
            lock.lock(); let request = request; lock.unlock()
            request?.append(buffer)
            inSegment += Double(buffer.frameLength) / format.sampleRate
            if inSegment >= Self.segment {
                // The segment's end: its request finishes, and the next begins once its final arrives.
                segmentStart += inSegment * 1000
                inSegment = 0
                request?.endAudio()
                while !aborted { lock.lock(); let same = self.request === request; lock.unlock(); if !same { break }; usleep(20000) }
            }
        }
        readDone = true
        lock.lock(); let request = request; lock.unlock()
        request?.endAudio()
    }

    func live(_ source: ExactAudioSource) {
        guard tap == nil else { return }
        tap = source.tap { [weak self] buffer, _ in
            guard let self else { return }
            self.lock.lock(); let request = self.request; self.lock.unlock()
            request?.append(buffer)
        }
    }

    func stop() {
        stopping = true
        tap?.cancel(); tap = nil
        lock.lock(); let request = request; lock.unlock()
        request?.endAudio()
    }

    func abort() {
        aborted = true
        tap?.cancel(); tap = nil
        task?.cancel()
    }
}
