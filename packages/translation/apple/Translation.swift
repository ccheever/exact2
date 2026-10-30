// @exact/translation on Apple (LLP 1133.027 D7–D10): the Translation
// framework (iOS 18, macOS 15; not on tvOS) and Natural Language's
// `NLLanguageRecognizer` for detection.
//
// - A translator and a detector are handles (D9), each's calls one at a time
//   in admission order. On iOS and macOS 26 every translate runs on an
//   installed-only session, which cannot ask to download; before 26 on a
//   session a hosted view's `translationTask` gives (Hosted.swift), and only
//   from the app's own foreground code, after the pair reads installed (D8).
// - A create while the pair is downloadable calls `prepareTranslation()`,
//   whose system sheet asks the person; dismissed, the create fails
//   `NotAllowedError`, `declined`. Every create is the app's foreground act,
//   which the runner admits (D16).
// - Under the agent no framework is asked and no sheet shown (§9):
//   availability is the fixed list's, a translation the script's next entry
//   or the input behind the target's tag, detection the script's or a rule on
//   the input's scripts, and the download sheet the drive's `downloadSheet`.
import ExactPackage
import Foundation
import NaturalLanguage
#if canImport(Translation) && !os(tvOS)
import Translation
#endif

public final class TranslationModule: ExactPackage {
    static let pairTopic = "translation.translatorAvailability", detectorTopic = "translation.languageDetectorAvailability"

    let context: ExactPackageContext
    private var serial = 0
    private var translators: [String: Handle] = [:], detectors: [String: Handle] = [:]
    // The agent's words (§9).
    private var pairOverlay: [String: Any] = [:], detectorOverlay: [String: Any] = [:]
    private var translations: [[String: Any]] = [], detections: [[String: Any]] = [], sheet = "accept"
    private var seen = (translations: "", detections: "")
    private var removed: Set<String> = []
    /// Downloads in flight by pair key, and under the agent the creates waiting on each.
    private var downloads: [String: [(Bool) -> Void]] = [:]

    public init(context: ExactPackageContext) { self.context = context }

    func say(_ line: String) { context.log("translation: \(line)") }

    /// A translator or detector: its languages, its work one at a time, and the platform's objects behind it.
    final class Handle {
        let id: String, source: String, target: String, key: String, expected: [String]?
        var queue: [(@escaping () -> Void) -> Void] = [], running = false, dead = false
        /// Before 26: the hosted session view; on 26, the installed-only session.
        var hosted: AnyObject?, installed: AnyObject?
        init(id: String, source: String = "", target: String = "", key: String = "", expected: [String]? = nil) {
            self.id = id; self.source = source; self.target = target; self.key = key; self.expected = expected
        }
    }

    /// `work` after every call on `handle` admitted before it.
    private func serially(_ handle: Handle, _ work: @escaping (@escaping () -> Void) -> Void) {
        handle.queue.append(work)
        guard !handle.running else { return }
        func next() {
            guard !handle.queue.isEmpty else { handle.running = false; return }
            handle.running = true
            let w = handle.queue.removeFirst()
            w { DispatchQueue.main.async { next() } }
        }
        next()
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "translatorAvailability": pairAvailability(args, reply)
        case "createTranslator": createTranslator(args, reply)
        case "translate": translate(args, reply)
        case "destroyTranslator": destroy(&translators, args["translator"], reply)
        case "languageDetectorAvailability": reply(detectorAvailability(args))
        case "createLanguageDetector": createDetector(args, reply)
        case "detectLanguage": detect(args, reply)
        case "destroyLanguageDetector": destroy(&detectors, args["detector"], reply)
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "languageDetectorAvailability" || name == "languageDetectorAvailabilitySync" ? detectorAvailability(args) : ["error": "NotSupportedError"]
    }

    // MARK: languages (D7)

    struct Pair { let source: String, target: String, key: String }

    /// The pair as asked, canonicalised; nil for a tag that is none, or one language twice.
    static func pair(_ args: [String: Any]) -> Pair? {
        guard let s = (args["sourceLanguage"] as? String).flatMap(Languages.canonical),
              let t = (args["targetLanguage"] as? String).flatMap(Languages.canonical) else { return nil }
        let a = Languages.scripted(s), b = Languages.scripted(t)
        return a == b ? nil : Pair(source: s, target: t, key: "\(a)>\(b)")
    }

    // MARK: availability (D8)

    private func pairAvailability(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard let p = Self.pair(args) else { return reply(["availability": "unavailable", "reason": "unsupported-options"]) }
        if context.agent { return reply(during(agentPair(p), p.key)) }
        #if canImport(Translation) && !os(tvOS)
        guard #available(iOS 18.0, macOS 15.0, *) else { return reply(["availability": "unavailable", "reason": "os"]) }
        Task { @MainActor in
            let status = await Platform.status(p)
            reply(self.during(Platform.availability(status), p.key))
        }
        #else
        reply(["availability": "unavailable", "reason": "not-supported"])
        #endif
    }

    private func agentPair(_ p: Pair) -> [String: Any] {
        #if os(tvOS)
        return ["availability": "unavailable", "reason": "not-supported"]
        #else
        guard Languages.listed(p.source, in: Languages.agent) != nil, Languages.listed(p.target, in: Languages.agent) != nil else {
            return ["availability": "unavailable", "reason": "unsupported-options"]
        }
        var a: [String: Any] = ["availability": removed.contains(p.key) ? "downloadable" : "available"]
        for (k, v) in pairOverlay { a[k] = v }
        if a["availability"] as? String != "unavailable" { a["reason"] = nil }
        return a
        #endif
    }

    /// An availability with a download in flight laid over it.
    private func during(_ a: [String: Any], _ key: String) -> [String: Any] {
        let word = a["availability"] as? String
        guard downloads[key] != nil, word == "downloadable" || word == "downloading" else { return a }
        var d = a
        d["availability"] = "downloading"
        // Apple reports no progress (D8); the agent's download stands at 0 until the drive completes it (§9).
        if context.agent { d["progress"] = 0 } else { d["progress"] = nil }
        return d
    }

    private func detectorAvailability(_ args: [String: Any]) -> [String: Any] {
        if let expected = args["expectedInputLanguages"] as? [String], expected.contains(where: { Languages.canonical($0) == nil }) {
            return ["availability": "unavailable", "reason": "unsupported-options"]
        }
        var a: [String: Any] = ["availability": "available"]
        if context.agent { for (k, v) in detectorOverlay { a[k] = v } }
        return a
    }

    // MARK: translators (D9)

    private func createTranslator(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard let p = Self.pair(args) else { return reply(["error": "NotSupportedError", "message": "unsupported-options"]) }
        let made = { [self] (hosted: AnyObject?) in
            serial += 1
            let h = Handle(id: "translator-\(serial)", source: p.source, target: p.target, key: p.key)
            h.hosted = hosted
            translators[h.id] = h
            say("\(h.id): \(p.source) to \(p.target)")
            reply(["id": h.id, "sourceLanguage": p.source, "targetLanguage": p.target])
        }
        if context.agent {
            let a = during(agentPair(p), p.key)
            switch a["availability"] as? String {
            case "available": return made(nil)
            case "unavailable": return reply(["error": "NotSupportedError", "message": a["reason"] as? String ?? "unsupported-options"])
            default:
                guard sheet != "decline" else { say("\(p.key): the download sheet was declined (agent)"); return reply(["error": "NotAllowedError", "message": "declined"]) }
                let first = downloads[p.key] == nil
                downloads[p.key, default: []].append { ok in ok ? made(nil) : reply(["error": "NotSupportedError"]) }
                if first { say("\(p.key): download started (agent)"); context.changed(Self.pairTopic) }
                return
            }
        }
        #if canImport(Translation) && !os(tvOS)
        guard #available(iOS 18.0, macOS 15.0, *) else { return reply(["error": "NotSupportedError", "message": "os"]) }
        Task { @MainActor in
            let status = await Platform.status(p)
            switch status {
            case .none, .some(.unsupported): return reply(["error": "NotSupportedError", "message": "unsupported-options"])
            case .some(.installed):
                if #available(iOS 26.0, macOS 26.0, *) { return made(nil) }
                guard let hosted = Platform.host(p, self.context) else { return reply(["error": "OperationError", "message": "the session is in no window"]) }
                made(hosted)
            case .some(.supported):
                self.download(p) { error in
                    if let error { return reply(error) }
                    made(nil)
                }
            @unknown default: reply(["error": "NotSupportedError"])
            }
        }
        #else
        reply(["error": "NotSupportedError", "message": "not-supported"])
        #endif
    }

    #if canImport(Translation) && !os(tvOS)
    /// The pair downloaded through a hosted session's `prepareTranslation()`, whose sheet asks the person (D8).
    @available(iOS 18.0, macOS 15.0, *)
    @MainActor
    private func download(_ p: Pair, _ done: @escaping ([String: Any]?) -> Void) {
        guard let hosted = Platform.host(p, context) else { return done(["error": "OperationError", "message": "the session is in no window"]) }
        downloads[p.key] = []
        context.changed(Self.pairTopic)
        say("\(p.key): download asked")
        let finish = { [self] (error: [String: Any]?) in
            hosted.remove()
            downloads[p.key] = nil
            context.changed(Self.pairTopic)
            say("\(p.key): download \(error == nil ? "finished" : "ended: \(error?["message"] ?? "")")")
            done(error)
        }
        hosted.submit(.init(run: { session in
            do { try await session.prepareTranslation(); finish(nil) } catch {
                let status = await Platform.status(p)
                finish(status == .installed ? nil : ["error": "NotAllowedError", "message": "declined"])
            }
        }, failed: { finish(["error": "AbortError"]) }))
    }
    #endif

    private func translate(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard let id = args["translator"] as? String, let h = translators[id] else { return reply(["error": "InvalidStateError"]) }
        let origin = context.origin, shown = context.foreground
        serially(h) { [self] done in
            let answer = { (a: [String: Any]) in reply(a); done() }
            guard !h.dead else { return answer(["error": "AbortError"]) }
            guard let input = args["input"] as? String else { return answer(["error": "TypeError"]) }
            if input.isEmpty { return answer(["text": ""]) }
            if !Self.unattendedSafe {
                // Before 26 a translate could show the sheet: only the app's foreground code runs one (D8).
                if origin != "app" { return answer(["error": "NotAllowedError", "message": "person-required"]) }
                if !shown { return answer(["error": "NotAllowedError", "message": "background"]) }
            }
            if context.agent {
                let entry = translations.isEmpty ? nil : translations.removeFirst()
                if let error = entry?["error"] as? String {
                    if entry?["reason"] as? String == "not-installed" { removed.insert(h.key); context.changed(Self.pairTopic) }
                    return answer(["error": error, "message": entry?["reason"] as? String ?? ""])
                }
                return answer(["text": entry?["text"] as? String ?? "[\(h.target)] \(input)"])
            }
            #if canImport(Translation) && !os(tvOS)
            if #available(iOS 18.0, macOS 15.0, *) {
                Task { @MainActor in answer(await Platform.translate(h, input, module: self)) }
                return
            }
            #endif
            answer(["error": "NotSupportedError"])
        }
    }

    /// Whether a translate here can never ask to download: an installed-only session (iOS and macOS 26).
    static var unattendedSafe: Bool {
        if #available(iOS 26.0, macOS 26.0, tvOS 26.0, *) { return true }
        return false
    }

    /// The pair's pack is gone: its availability is read again (D8).
    func notInstalled(_ h: Handle) -> [String: Any] {
        context.changed(Self.pairTopic)
        say("\(h.id): \(h.key) is not installed")
        return ["error": "InvalidStateError", "message": "not-installed"]
    }

    private func destroy(_ map: inout [String: Handle], _ id: Any?, _ reply: @escaping ([String: Any]) -> Void) {
        guard let id = id as? String, let h = map.removeValue(forKey: id) else { return reply(["error": "InvalidStateError"]) }
        drop(h, "destroyed by the app")
        reply(["id": id])
    }

    private func drop(_ h: Handle, _ why: String) {
        h.dead = true
        #if canImport(Translation) && !os(tvOS)
        // A package's entries run on the main thread.
        if #available(iOS 18.0, macOS 15.0, *), let hosted = h.hosted as? Hosted { MainActor.assumeIsolated { hosted.remove() } }
        #endif
        h.hosted = nil; h.installed = nil
        say("\(h.id): \(why)")
    }

    // MARK: detection

    private func createDetector(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        let a = detectorAvailability(args)
        guard a["availability"] as? String == "available" else { return reply(["error": "NotSupportedError", "message": a["reason"] as? String ?? ""]) }
        serial += 1
        let expected = args["expectedInputLanguages"] as? [String]
        let h = Handle(id: "language-detector-\(serial)", expected: expected)
        detectors[h.id] = h
        say("\(h.id): made")
        var info: [String: Any] = ["id": h.id]
        if let expected { info["expectedInputLanguages"] = expected }
        reply(info)
    }

    private func detect(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        guard let id = args["detector"] as? String, let h = detectors[id] else { return reply(["error": "InvalidStateError"]) }
        serially(h) { [self] done in
            let answer = { (a: [String: Any]) in reply(a); done() }
            guard !h.dead else { return answer(["error": "AbortError"]) }
            guard let input = args["input"] as? String else { return answer(["error": "TypeError"]) }
            if input.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return answer(["results": [["detectedLanguage": "und", "confidence": 1.0]]]) }
            if context.agent {
                if !detections.isEmpty { return answer(detections.removeFirst()) }
                guard let language = Languages.ruled(input) else { return answer(["results": [["detectedLanguage": "und", "confidence": 1.0]]]) }
                return answer(["results": [["detectedLanguage": language, "confidence": 0.9], ["detectedLanguage": "und", "confidence": 0.1]]])
            }
            let recognizer = NLLanguageRecognizer()
            if let expected = h.expected, !expected.isEmpty {
                recognizer.languageHints = Dictionary(expected.map { (NLLanguage(rawValue: Languages.scripted($0)), 1.0 / Double(expected.count)) }, uniquingKeysWith: { a, _ in a })
            }
            recognizer.processString(input)
            let hypotheses = recognizer.languageHypotheses(withMaximum: 10)
            answer(["results": Languages.results(Dictionary(hypotheses.map { ($0.key.rawValue, $0.value) }, uniquingKeysWith: { a, _ in a }))])
        }
    }

    // MARK: the session

    public func preferred(_ values: [String: Any]) {
        pairOverlay = values["translatorAvailability"] as? [String: Any] ?? [:]
        detectorOverlay = values["languageDetectorAvailability"] as? [String: Any] ?? [:]
        sheet = values["downloadSheet"] as? String ?? "accept"
        let t = String(describing: values["translations"] ?? ""), d = String(describing: values["detections"] ?? "")
        if t != seen.translations { seen.translations = t; translations = values["translations"] as? [[String: Any]] ?? []; say("script: \(translations.count) translations") }
        if d != seen.detections { seen.detections = d; detections = values["detections"] as? [[String: Any]] ?? []; say("script: \(detections.count) detections") }
        // The drive completes or fails an agent's download by the availability it lays over (§9).
        switch pairOverlay["availability"] as? String {
        case "available":
            for (key, waiting) in downloads { removed.remove(key); say("\(key): download finished (agent)"); waiting.forEach { $0(true) } }
            if !downloads.isEmpty { downloads = [:]; context.changed(Self.pairTopic) }
        case "unavailable":
            for waiting in downloads.values { waiting.forEach { $0(false) } }
            if !downloads.isEmpty { downloads = [:]; context.changed(Self.pairTopic) }
        default: break
        }
    }

    public func dispose(_ ids: [String]) {
        for id in ids {
            if let h = translators.removeValue(forKey: id) ?? detectors.removeValue(forKey: id) { drop(h, "its reply was dropped") }
        }
    }

    /// Back in the foreground, a pack may have arrived or gone (D8).
    public func foreground(_ shown: Bool) {
        if shown, !context.agent { context.changed(Self.pairTopic) }
    }

    public func destroy(_ done: @escaping () -> Void) {
        for h in Array(translators.values) + Array(detectors.values) { drop(h, "the session ended") }
        translators = [:]; detectors = [:]
        done()
    }
}

#if canImport(Translation) && !os(tvOS)
/// The Translation framework's side, iOS 18 and macOS 15 on.
@available(iOS 18.0, macOS 15.0, *)
@MainActor
enum Platform {
    /// The framework's languages as D7's table lists them: each one's language, and its language and script.
    private static var listed: [String]?

    static func languages() async -> [String] {
        if let listed { return listed }
        var out: [String] = []
        for l in await LanguageAvailability().supportedLanguages {
            guard let code = l.languageCode?.identifier else { continue }
            if !out.contains(code) { out.append(code) }
            if let script = l.script?.identifier, !out.contains("\(code)-\(script)") { out.append("\(code)-\(script)") }
        }
        listed = out
        return out
    }

    /// The framework's languages for the pair, or nil where it lists one not.
    static func languages(_ p: TranslationModule.Pair) async -> (Locale.Language, Locale.Language)? {
        let listed = await languages()
        guard let s = Languages.listed(p.source, in: listed), let t = Languages.listed(p.target, in: listed), s != t else { return nil }
        return (Locale.Language(identifier: s), Locale.Language(identifier: t))
    }

    static func status(_ p: TranslationModule.Pair) async -> LanguageAvailability.Status? {
        guard let (s, t) = await languages(p) else { return nil }
        return await LanguageAvailability().status(from: s, to: t)
    }

    static func availability(_ status: LanguageAvailability.Status?) -> [String: Any] {
        switch status {
        case .some(.installed): return ["availability": "available", "apple": ["installed": "installed"]]
        case .some(.supported): return ["availability": "downloadable", "apple": ["installed": "supported"]]
        default: return ["availability": "unavailable", "reason": "unsupported-options", "apple": ["installed": "unsupported"]]
        }
    }

    /// A hosted session for the pair under the context's session view, or nil while it is in no window.
    static func host(_ p: TranslationModule.Pair, _ context: ExactPackageContext) -> Hosted? {
        // `Locale.Language(identifier:)` of D7's tags; the framework matches regions itself.
        let s = Locale.Language(identifier: Languages.scripted(p.source)), t = Locale.Language(identifier: Languages.scripted(p.target))
        return Hosted(source: s, target: t, context: context)
    }

    /// One translate on the handle's session: the installed-only one on 26, else its hosted one, after the pair reads
    /// installed (D8). A long input goes in paragraphs (D9).
    static func translate(_ h: TranslationModule.Handle, _ input: String, module: TranslationModule) async -> [String: Any] {
        let p = TranslationModule.Pair(source: h.source, target: h.target, key: h.key)
        guard let (s, t) = await languages(p) else { return ["error": "NotSupportedError", "message": "unsupported-options"] }
        let parts = input.count > 1000 ? input.components(separatedBy: "\n\n") : [input]
        if #available(iOS 26.0, macOS 26.0, *) {
            let session = h.installed as? TranslationSession ?? TranslationSession(installedSource: s, target: t)
            h.installed = session
            do {
                var out: [String] = []
                for part in parts { out.append(part.isEmpty ? "" : try await session.translate(part).targetText) }
                return ["text": out.joined(separator: "\n\n")]
            } catch TranslationError.notInstalled {
                h.installed = nil
                return module.notInstalled(h)
            } catch {
                return failure(error)
            }
        }
        guard await LanguageAvailability().status(from: s, to: t) == .installed else { return module.notInstalled(h) }
        if (h.hosted as? Hosted)?.dead != false {
            guard let hosted = host(p, module.context) else { return ["error": "OperationError", "message": "the session is in no window"] }
            h.hosted = hosted
        }
        guard let hosted = h.hosted as? Hosted else { return ["error": "OperationError"] }
        return await withCheckedContinuation { done in
            hosted.submit(.init(run: { session in
                do {
                    var out: [String] = []
                    for part in parts { out.append(part.isEmpty ? "" : try await session.translate(part).targetText) }
                    done.resume(returning: ["text": out.joined(separator: "\n\n")])
                } catch {
                    done.resume(returning: failure(error))
                }
            }, failed: { done.resume(returning: ["error": "AbortError"]) }))
        }
    }

    static func failure(_ error: Error) -> [String: Any] {
        switch error {
        case TranslationError.unsupportedSourceLanguage, TranslationError.unsupportedTargetLanguage, TranslationError.unsupportedLanguagePairing:
            return ["error": "NotSupportedError", "message": "unsupported-options"]
        case TranslationError.internalError: return ["error": "OperationError", "message": error.localizedDescription]
        default:
            if #available(iOS 26.0, macOS 26.0, *), case TranslationError.alreadyCancelled = error { return ["error": "AbortError"] }
            if error is CancellationError { return ["error": "AbortError"] }
            return ["error": "OperationError", "message": error.localizedDescription]
        }
    }
}
#endif
