// @exact/language-model over Foundation Models (LLP 1133.016 D5, D6, D8,
// D9): the availability and its reasons, a `ModelSession` built from
// the module's transcript as a cache, constrained decoding from D8's subset,
// one `Tool` per declared tool whose call goes to the app's handler, and the
// errors in the interface's names. Outside the agent only, on iOS and macOS
// 26 and later; below, the framework is weak-linked and never touched.
import ExactPackage
import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

extension LanguageModelModule {
    private static var storedHatch: Any?

    static func makeApple(_ m: LanguageModelModule) -> AnyObject? {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, *) { return AppleModel(m) }
        #endif
        return nil
    }

    static func appleAvailability(_ m: LanguageModelModule, _ args: [String: Any]) -> [String: Any] {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, *), let a = m.apple as? AppleModel { return a.availability(args) }
        #endif
        return ["availability": "unavailable", "reason": "os", "leavesDevice": false, "structuredOutput": "none", "tools": false, "apple": NSNull()]
    }

    func beginApple(_ r: ModelResponse) {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, *), let a = apple as? AppleModel { return a.begin(r) }
        #endif
        r.session.turns.append(r.turn)
        end(r, "failed", error: "NotSupportedError")
    }

    func prewarm(_ s: ModelSession) {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, *), let a = apple as? AppleModel { a.prewarm(s) }
        #endif
    }

    /// `measureContextUsage` by the model's own count (iOS and macOS 26.4); false where it cannot count.
    func measureApple(_ s: ModelSession, _ input: String, reply: @escaping ([String: Any]) -> Void) -> Bool {
        #if canImport(FoundationModels)
        if #available(iOS 26.4, macOS 26.4, *), let a = apple as? AppleModel {
            Task { @MainActor in
                do { reply(["usage": try await a.model(for: s).tokenCount(for: input), "contextWindow": self.windowOf(s)]) }
                catch { reply(["error": AppleModel.code(error).0]) }
            }
            return true
        }
        #endif
        return false
    }
}

#if canImport(FoundationModels)
@available(iOS 26.0, macOS 26.0, *)
extension LanguageModelModule {
    /// The app's access hatch (D12): given the `SystemLanguageModel` a
    /// session will use, before each platform session is built, it may
    /// return another (a use case's, a guardrails setting's, an adapter's).
    /// Main thread. Never called under the agent.
    public static var systemLanguageModel: ((SystemLanguageModel) -> SystemLanguageModel)? {
        get { storedHatch as? (SystemLanguageModel) -> SystemLanguageModel }
        set { storedHatch = newValue }
    }
}

@available(iOS 26.0, macOS 26.0, *)
final class AppleModel {
    private weak var module: LanguageModelModule?
    /// The reason last read while `model-not-ready`, re-read each 10 s of the session clock (D5).
    private var watching = false
    private var last: String?

    init(_ m: LanguageModelModule) { module = m }

    // MARK: availability (D5)

    func availability(_ args: [String: Any]) -> [String: Any] {
        let model = SystemLanguageModel.default
        let languages = model.supportedLanguages.map { $0.minimalIdentifier }.sorted()
        var apple: [String: Any] = ["supportedLanguages": languages, "contextSize": model.contextSize, "modelVersion": NSNull(), "unavailableReason": NSNull()]
        var out: [String: Any] = ["leavesDevice": false, "structuredOutput": "enforced", "tools": true]
        switch model.availability {
        case .available:
            let expected = (args["expectedInputs"] as? [[String: Any]] ?? []) + (args["expectedOutputs"] as? [[String: Any]] ?? [])
            let types = expected.compactMap { $0["type"] as? String }.filter { $0 != "text" && $0 != "tool-call" }
            let asked = expected.flatMap { $0["languages"] as? [String] ?? [] }
            let unsupported = asked.filter { !model.supportsLocale(Locale(identifier: $0)) }
            if !types.isEmpty || !unsupported.isEmpty {
                out["availability"] = "unavailable"; out["reason"] = "unsupported-options"
            } else {
                out["availability"] = "available"; out["contextWindow"] = model.contextSize
            }
        case .unavailable(let why):
            out["availability"] = "unavailable"
            switch why {
            case .deviceNotEligible: out["reason"] = "device-not-eligible"; apple["unavailableReason"] = "deviceNotEligible"
            case .appleIntelligenceNotEnabled: out["reason"] = "not-enabled"; apple["unavailableReason"] = "appleIntelligenceNotEnabled"
            case .modelNotReady: out["reason"] = "model-not-ready"; apple["unavailableReason"] = "modelNotReady"
            @unknown default: out["reason"] = "device-not-eligible"
            }
        @unknown default:
            out["availability"] = "unavailable"; out["reason"] = "device-not-eligible"
        }
        out["apple"] = apple
        let word = "\(out["availability"] ?? "")/\(out["reason"] ?? "")"
        if word != last { module?.say("availability: \(word)"); last = word }
        if out["reason"] as? String == "model-not-ready" { watch() }
        return out
    }

    /// While `model-not-ready`, the availability is asked again each 10 s of the session clock (D5).
    private func watch() {
        guard !watching, let m = module else { return }
        watching = true
        let start = m.context.now()
        func tick() {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
                guard let self, let m = self.module else { return }
                if m.context.now() - start < 10_000 { return tick() }
                self.watching = false
                m.announce(LanguageModelModule.availabilityKey)
            }
        }
        tick()
    }

    // MARK: the platform session (D6)

    func model(for s: ModelSession) -> SystemLanguageModel {
        let base = SystemLanguageModel(useCase: s.options["useCase"] as? String == "contentTagging" ? .contentTagging : .general,
                                       guardrails: s.options["guardrails"] as? String == "permissiveContentTransformations" ? .permissiveContentTransformations : .default)
        return LanguageModelModule.systemLanguageModel?(base) ?? base
    }

    /// Foundation Models' session for `s`, built from the transcript when it has none or the transcript changed under it.
    private func platform(_ s: ModelSession, prompting turn: ModelTurn) throws -> FoundationModels.LanguageModelSession {
        if let p = s.platform as? FoundationModels.LanguageModelSession, !s.stale { return p }
        let tools = try s.tools.map { t in
            AppleTool(name: t.name, description: t.description, parameters: try LanguageModelSchema.generation(t.schema, name: t.name), session: s.id, owner: self)
        }
        let made = FoundationModels.LanguageModelSession(model: model(for: s), tools: tools, transcript: transcript(s, tools: tools, excluding: turn))
        s.platform = made
        s.stale = false
        module?.say("\(s.id): platform session built from \(s.turns.count - 1) turns")
        return made
    }

    /// The module's turns as a `Transcript`, tool exchanges included (D6), all but the input being prompted.
    private func transcript(_ s: ModelSession, tools: [AppleTool], excluding turn: ModelTurn) -> Transcript {
        var entries: [Transcript.Entry] = []
        let definitions = tools.map { Transcript.ToolDefinition(name: $0.name, description: $0.description, parameters: $0.parameters) }
        if s.system != nil || !definitions.isEmpty {
            entries.append(.instructions(.init(segments: s.system.map { [.text(.init(content: $0))] } ?? [], toolDefinitions: definitions)))
        }
        for t in s.turns where t !== turn {
            if let input = t.input { entries.append(.prompt(.init(segments: [.text(.init(content: input))]))) }
            for c in t.calls {
                let arguments = (try? GeneratedContent(json: c.fullArguments)) ?? GeneratedContent(c.fullArguments)
                entries.append(.toolCalls(.init([.init(id: c.id, toolName: c.name, arguments: arguments)])))
                entries.append(.toolOutput(.init(id: c.id, toolName: c.name, segments: [.text(.init(content: c.fullResult ?? c.result ?? ""))])))
            }
            if let reply = t.reply { entries.append(.response(.init(assetIDs: [], segments: [.text(.init(content: reply))]))) }
        }
        return Transcript(entries: entries)
    }

    func prewarm(_ s: ModelSession) {
        let turn = ModelTurn(input: nil)
        (try? platform(s, prompting: turn))?.prewarm()
    }

    // MARK: generation (D6, D7, D8)

    func begin(_ r: ModelResponse) {
        r.task = Task { @MainActor [weak self] in await self?.generate(r) }
    }

    @MainActor
    private func generate(_ r: ModelResponse) async {
        guard let m = module, r.state == "generating" else { return }
        let s = r.session, input = r.turn.input ?? ""
        // The model's own count of the input where it has one (26.4), so D6's eviction runs before the prompt.
        var measured: Int?
        if #available(iOS 26.4, macOS 26.4, *) {
            if let n = try? await model(for: s).tokenCount(for: input) {
                measured = n + 4
                if let schema = r.schema, !r.omit, let g = try? LanguageModelSchema.generation(schema, name: "Response"), let k = try? await model(for: s).tokenCount(for: g) { measured! += k }
            }
        }
        guard r.state == "generating", m.admitInput(r, extra: measured) else { return }
        if r.overflowed { s.stale = true }
        var output = false
        while r.live {
            do {
                let session = try platform(s, prompting: r.turn)
                let options = Self.options(r.options)
                if let schema = r.schema {
                    let g = try LanguageModelSchema.generation(schema, name: "Response")
                    let include = (r.options["includeSchemaInPrompt"] as? Bool) ?? !r.omit
                    for try await snapshot in session.streamResponse(to: input, schema: g, includeSchemaInPrompt: include, options: options) {
                        guard r.live else { return }
                        output = true
                        r.text = snapshot.rawContent.jsonString
                    }
                } else {
                    for try await snapshot in session.streamResponse(to: input, options: options) {
                        guard r.live else { return }
                        output = true
                        r.text = snapshot.content
                    }
                }
                guard r.live else { return }
                s.overflowed = s.overflowed || r.overflowed
                m.end(r, "complete")
                return
            } catch {
                guard r.live else { return }
                // 26.0 to 26.3 cannot count: an overflow before the first output evicts the oldest turn and asks again (D6).
                if case FoundationModels.LanguageModelSession.GenerationError.exceededContextWindowSize = error, !output, r.calls.isEmpty, s.turns.first !== r.turn {
                    s.turns.removeFirst()
                    s.overflowed = true; r.overflowed = true; s.stale = true
                    m.say("\(r.id): over the context window; the oldest turn evicted, asked again")
                    continue
                }
                // Foundation Models keeps no failed or cancelled reply: the next prompt rebuilds from the transcript.
                s.stale = true
                let (code, reason) = Self.code(error)
                if case let FoundationModels.LanguageModelSession.GenerationError.refusal(refusal, _) = error {
                    r.refusalExplanation = try? await refusal.explanation.content
                }
                if code == "QuotaExceededError" { s.overflowed = true; r.overflowed = true }
                m.say("\(r.id): \(error)")
                m.end(r, "failed", error: code, reason: reason)
                return
            }
        }
    }

    /// Foundation Models' error as D2's name and reason.
    static func code(_ error: Error) -> (String, String?) {
        if error is CancellationError { return ("AbortError", nil) }
        if let e = error as? LanguageModelSchema.Unsupported { return ("NotSupportedError", e.why) }
        guard let e = error as? FoundationModels.LanguageModelSession.GenerationError else { return ("OperationError", nil) }
        switch e {
        case .exceededContextWindowSize: return ("QuotaExceededError", nil)
        case .assetsUnavailable: return ("OperationError", "model-not-ready")
        case .guardrailViolation: return ("NotReadableError", "guardrail")
        case .refusal: return ("NotReadableError", "refusal")
        case .unsupportedGuide, .unsupportedLanguageOrLocale: return ("NotSupportedError", nil)
        case .decodingFailure: return ("SyntaxError", nil)
        case .rateLimited: return ("OperationError", "rate-limited")
        case .concurrentRequests: return ("OperationError", "rate-limited")
        @unknown default: return ("OperationError", nil)
        }
    }

    /// The prompt's Apple words (D11), each the platform's choice where unsaid (D4).
    private static func options(_ words: [String: Any]) -> GenerationOptions {
        var sampling: GenerationOptions.SamplingMode?
        switch words["sampling"] as? String {
        case "greedy": sampling = .greedy
        case "random":
            if let p = LanguageModelSchema.number(words["probabilityThreshold"]) { sampling = .random(probabilityThreshold: p) }
            else { sampling = .random(top: Int(LanguageModelSchema.number(words["top"]) ?? 50)) }
        default: break
        }
        return GenerationOptions(samplingMode: sampling, temperature: LanguageModelSchema.number(words["temperature"]),
                                 maximumResponseTokens: LanguageModelSchema.number(words["maximumResponseTokens"]).map { Int($0) })
    }

    // MARK: tools (D9)

    /// The model called `name` on session `id`'s responding response: the app's handler through the runner, its answer the text the model reads.
    func called(_ id: String, _ name: String, _ arguments: GeneratedContent, reply: @escaping (String) -> Void) {
        guard let m = module, let s = m.sessions[id], let r = s.responding, r.live else {
            return reply("{\"error\":\"AbortError\",\"message\":\"the response ended\"}")
        }
        let json = arguments.jsonString
        let args = LanguageModelSchema.parse(json) as? [String: Any] ?? [:]
        m.toolCall(r, name: name, arguments: args) { text in
            if r.state == "calling-tool", !r.calls.contains(where: { $0.state == "running" }) { r.state = "generating" }
            reply(text)
        }
    }
}

/// One declared tool as Foundation Models calls it: its `Arguments` are the
/// generated content of its own `parameters` (D9), and its call waits for
/// the app's handler, on the main thread.
@available(iOS 26.0, macOS 26.0, *)
final class AppleTool: Tool, @unchecked Sendable {
    typealias Arguments = GeneratedContent
    typealias Output = String
    let name: String, description: String, parameters: GenerationSchema
    private let session: String
    private weak var owner: AppleModel?

    init(name: String, description: String, parameters: GenerationSchema, session: String, owner: AppleModel) {
        self.name = name; self.description = description; self.parameters = parameters; self.session = session; self.owner = owner
    }

    func call(arguments: GeneratedContent) async throws -> String {
        await withCheckedContinuation { (done: CheckedContinuation<String, Never>) in
            DispatchQueue.main.async { [owner, session, name] in
                guard let owner else { return done.resume(returning: "{\"error\":\"AbortError\"}") }
                owner.called(session, name, arguments) { done.resume(returning: $0) }
            }
        }
    }
}
#endif
