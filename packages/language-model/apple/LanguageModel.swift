// @exact/language-model on Apple (LLP 1133.016): Foundation Models'
// `SystemLanguageModel` (LanguageModelApple.swift), and under the agent the
// agent model of §9 on the session clock. The app's composed module calls it
// through `ExactPackage`'s shape.
//
// - The transcript is this module's (D6): each session's instructions and
//   turns (an input, the tool calls it caused, the reply), from which
//   Foundation Models' session is built and rebuilt. web/index.js keeps the
//   same records, by the same rules, so the hosts agree on every count.
// - A response is a value the runner polls (D7): its text so far, which
//   Foundation Models' stream fills; only a change of `state` is announced.
// - A tool is the app's handler (D9), run through `context.invoke` for the
//   work it serves.
// - A download is the web's and Android's: Apple's system fetches its own
//   model, so the availability is never `downloadable` outside the agent.
// - Foundation Models is iOS and macOS 26's, weak-linked below (D15 item 6):
//   below 26 the availability reads `unavailable`, `os`.
import ExactPackage
import Foundation
#if canImport(UIKit)
import UIKit
#endif

/// One input of a session, the tool calls it caused and the reply (D6).
final class ModelTurn {
    var input: String?
    var calls: [ModelCall] = []
    var reply: String?
    init(input: String?, reply: String? = nil) { self.input = input; self.reply = reply }
}

/// A tool call as the model made it and as a response's state keeps it, each side cut to 4 KB (D9).
final class ModelCall {
    let id: String, name: String, arguments: String, fullArguments: String
    var state = "running", result: String?, fullResult: String?, truncated: Bool
    init(id: String, name: String, arguments full: String) {
        let cut = LanguageModelModule.cut(full)
        self.id = id; self.name = name; arguments = cut.text; fullArguments = full; truncated = cut.truncated
    }
    var json: [String: Any] {
        ["id": id, "name": name, "arguments": arguments, "state": state, "result": result ?? NSNull(), "truncated": truncated]
    }
}

final class ModelSession {
    let id: String
    var system: String?
    var turns: [ModelTurn]
    /// Each tool's name, description and input schema.
    let tools: [(name: String, description: String, schema: [String: Any])]
    let toolCost: Int
    var overflowed = false
    var window: Int
    var queue: [(response: ModelResponse?, go: (Double) -> Void)] = []
    var responding: ModelResponse?
    var ended: [String] = []
    var count = 0
    /// Foundation Models' session, a cache of the transcript; stale when the transcript changed under it.
    var platform: AnyObject?
    var stale = false
    let options: [String: Any]
    init(id: String, system: String?, turns: [ModelTurn], tools: [(name: String, description: String, schema: [String: Any])],
         window: Int, options: [String: Any]) {
        self.id = id; self.system = system; self.turns = turns; self.tools = tools; self.window = window; self.options = options
        toolCost = tools.isEmpty ? 0 : LanguageModelModule.schemaCost(tools.map { ["name": $0.name, "description": $0.description, "inputSchema": $0.schema] })
    }
}

final class ModelResponse {
    let id: String
    unowned let session: ModelSession
    var state = "generating", text = ""
    var calls: [ModelCall] = []
    var overflowed = false
    var error: String?, reason: String?, message: String?, refusalExplanation: String?
    let schema: [String: Any]?, typed: Bool, omit: Bool, whole: Bool
    let turn: ModelTurn
    /// What a tool call it causes serves: the session for `prompt`, which makes no handle, else the response.
    let handle: String
    let request: Int?
    let options: [String: Any]
    var planned: [String]?, hold = false, start = 0.0
    var contextUsage: Int?
    var settled: [(ModelResponse) -> Void] = []
    /// Foundation Models' generation, cancelled by an abort.
    var task: Task<Void, Never>?
    init(id: String, session: ModelSession, schema: [String: Any]?, typed: Bool, omit: Bool, whole: Bool, input: String,
         request: Int?, options: [String: Any]) {
        self.id = id; self.session = session; self.schema = schema; self.typed = typed; self.omit = omit; self.whole = whole
        turn = ModelTurn(input: input); handle = whole ? session.id : id; self.request = request; self.options = options
    }
    var live: Bool { state == "generating" || state == "calling-tool" }
}

public final class LanguageModelModule: ExactPackage {
    static let tokenMs = 25.0, defaultWindow = 4096, kept = 8, toolLimit = 16, cutBytes = 4096
    static let responseKey = "language-model.languageModelResponse", sessionKey = "language-model.languageModelSession",
               availabilityKey = "language-model.languageModelAvailability"
    private static let noResponse: [String: Any] = ["id": "", "session": "", "state": "complete", "text": "", "toolCalls": [Any](), "overflowed": false]
    private static let noSession: [String: Any] = ["id": "", "contextWindow": 0, "responding": false]

    let context: ExactPackageContext
    var agent: Bool { context.agent }
    var sessions: [String: ModelSession] = [:]
    var responses: [String: ModelResponse] = [:]
    private var byRequest: [Int: ModelResponse] = [:]
    private var serial = 0
    // The agent's words (§9) and its preferred availability, laid over the agent model's.
    private var overlay: [String: Any] = [:]
    private var script: [[String: Any]] = [], scriptSeen = ""
    private var rate = tokenMs
    var agentWindow = defaultWindow
    /// A download the agent model started, and the creates waiting on it (D5).
    private var downloading: Double?
    private var waiting: [(Bool) -> Void] = []
    /// Foundation Models, outside the agent on iOS and macOS 26 (LanguageModelApple.swift).
    var apple: AnyObject?

    public init(context: ExactPackageContext) {
        self.context = context
        if !context.agent { apple = Self.makeApple(self) }
        #if canImport(UIKit) && !os(watchOS)
        // A memory warning drops the platform sessions not responding (D13); each is rebuilt on its next use.
        NotificationCenter.default.addObserver(forName: UIApplication.didReceiveMemoryWarningNotification, object: nil, queue: .main) { [weak self] _ in
            self?.dropCaches()
        }
        #endif
    }

    func say(_ line: String) { context.log("language-model: \(line)") }
    func announce(_ key: String) { context.changed(key) }

    // MARK: counting (§9)

    static func bytes(_ s: String) -> Int { s.utf8.count }
    static func cost(_ text: String) -> Int { 4 + (bytes(text) + 3) / 4 }
    static func schemaCost(_ schema: Any) -> Int { (bytes(LanguageModelSchema.json(schema) ?? "") + 3) / 4 }
    /// The first 4 KB of `text`, on a character boundary, and whether it was cut (D9).
    static func cut(_ text: String) -> (text: String, truncated: Bool) {
        guard bytes(text) > cutBytes else { return (text, false) }
        var out = "", n = 0
        for c in text {
            let b = String(c).utf8.count
            if n + b > cutBytes { break }
            out.append(c); n += b
        }
        return (out, true)
    }
    /// Tokens as §9 cuts them: at the end of each run of whitespace.
    static func tokens(_ text: String) -> [String] {
        var out: [String] = [], current = "", lastSpace = false
        for c in text {
            if !c.isWhitespace && lastSpace && !current.isEmpty { out.append(current); current = "" }
            current.append(c)
            lastSpace = c.isWhitespace
        }
        if !current.isEmpty { out.append(current) }
        return out
    }

    func windowOf(_ s: ModelSession) -> Int { agent ? agentWindow : s.window }
    private func turnCost(_ t: ModelTurn) -> Int {
        (t.input.map(Self.cost) ?? 0) + (t.reply.map(Self.cost) ?? 0) + t.calls.reduce(0) { $0 + Self.cost($1.arguments + ($1.result ?? "")) }
    }
    func counted(_ s: ModelSession) -> Int {
        (s.system.map(Self.cost) ?? 0) + s.toolCost + s.turns.reduce(0) { $0 + turnCost($1) }
    }
    /// Evict the oldest whole turns until `extra` more fits (D6); false when even then it does not.
    func fit(_ s: ModelSession, _ extra: Int) -> Bool {
        let had = s.turns.count
        while counted(s) + extra > windowOf(s), !s.turns.isEmpty { s.turns.removeFirst(); s.overflowed = true; s.stale = true }
        if s.turns.count < had { say("\(s.id): evicted \(had - s.turns.count) turns, overflowed, \(counted(s)) of \(windowOf(s)) tokens") }
        return counted(s) + extra <= windowOf(s)
    }

    /// `initialPrompts` as the system message and turns: a user message and the assistant messages after it are one.
    static func turnsOf(_ messages: [[String: Any]]) throws -> (String?, [ModelTurn]) {
        var system: String?, turns: [ModelTurn] = []
        for (i, m) in messages.enumerated() {
            let role = m["role"] as? String ?? "user", content = m["content"] as? String ?? ""
            if role == "system" {
                guard i == 0 else { throw Refused(code: "TypeError", why: "a system message comes first") }
                system = content
            } else if role == "user" || turns.isEmpty {
                turns.append(ModelTurn(input: role == "user" ? content : nil, reply: role == "user" ? nil : content))
            } else if let t = turns.last {
                t.reply = t.reply.map { "\($0)\n\n\(content)" } ?? content
            }
        }
        return (system, turns)
    }

    struct Refused: Error { let code: String, why: String }

    func sessionInfo(_ s: ModelSession) -> [String: Any] {
        ["id": s.id, "contextUsage": counted(s), "contextWindow": windowOf(s), "responding": s.responding != nil || !s.queue.isEmpty]
    }
    func createdInfo(_ s: ModelSession) -> [String: Any] {
        ["id": s.id, "contextUsage": counted(s), "contextWindow": windowOf(s), "tools": s.tools.map(\.name), "overflowed": s.overflowed]
    }

    /// A session from a create's arguments, trimmed as a prompt evicts (D6): nil when the system message and the newest turn do not fit.
    func makeSession(_ args: [String: Any], window: Int) throws -> ModelSession? {
        let (system, turns) = try Self.turnsOf(args["initialPrompts"] as? [[String: Any]] ?? [])
        let tools = try (args["tools"] as? [[String: Any]] ?? []).map { t -> (name: String, description: String, schema: [String: Any]) in
            let schema = (t["inputSchema"] as? String).flatMap { LanguageModelSchema.parse($0) as? [String: Any] } ?? t["inputSchema"] as? [String: Any] ?? [:]
            if let why = LanguageModelSchema.refusal(schema) { throw Refused(code: "NotSupportedError", why: why) }
            return (t["name"] as? String ?? "", t["description"] as? String ?? "", schema)
        }
        serial += 1
        let s = ModelSession(id: "session-\(serial)", system: system, turns: turns, tools: tools, window: window,
                                     options: (args["apple"] as? [String: Any]) ?? [:])
        guard fit(s, 0) else { return nil }
        sessions[s.id] = s
        say("\(s.id): created, \(s.turns.count) turns, \(counted(s)) of \(windowOf(s)) tokens\(s.overflowed ? ", earlier turns evicted" : "")\(tools.isEmpty ? "" : ", tools \(tools.map(\.name).joined(separator: ", "))")")
        return s
    }

    // MARK: responses

    func responseInfo(_ r: ModelResponse) -> [String: Any] {
        if agent { advance(r) }
        var out: [String: Any] = ["id": r.id, "session": r.session.id, "state": r.state, "text": r.text, "toolCalls": r.calls.map(\.json),
                                  "overflowed": r.overflowed, "apple": r.refusalExplanation.map { ["refusalExplanation": $0] } ?? NSNull()]
        if let u = r.contextUsage { out["contextUsage"] = u }
        if let e = r.error { out["error"] = e; out["reason"] = r.reason ?? NSNull(); out["message"] = r.message ?? NSNull() }
        return out
    }

    /// Under the agent: the tokens the session clock has reached, and the end once every one has landed.
    func advance(_ r: ModelResponse) {
        guard r.state == "generating", let planned = r.planned else { return }
        let all = planned.count, whole = rate <= 0 || r.whole
        let reached = whole ? all : Int((context.now() - r.start) / rate)
        let shown = min(all - (r.hold ? 1 : 0), reached)
        r.text = planned.prefix(max(0, shown)).joined()
        if shown >= all { end(r, "complete", at: whole ? r.start : r.start + Double(all) * rate) }
    }
    private func advanceAll() { responses.values.forEach(advance) }

    /// The response ends: its turn committed with its text so far (D6), the session's next call begun at `at`.
    func end(_ r: ModelResponse, _ state: String, error: String? = nil, reason: String? = nil, message: String? = nil, at: Double? = nil) {
        guard r.live else { return }
        r.state = state
        if let error { r.error = error; r.reason = reason; r.message = message }
        let s = r.session, begun = s.responding === r
        // One still waiting its turn never reached the session: nothing of it is committed.
        if begun { r.turn.reply = r.text } else { s.queue.removeAll { $0.response === r } }
        r.contextUsage = counted(s)
        r.task?.cancel()
        if begun { s.responding = nil }
        s.ended.append(r.id)
        // An ended response is kept until its session has ended eight newer (D6).
        while s.ended.count > Self.kept { let old = s.ended.removeFirst(); responses[old] = nil; say("\(old): retired") }
        say("\(r.id): \(state)\(error.map { " (\($0)\(reason.map { ", \($0)" } ?? ""))" } ?? ""), \(r.text.count) characters, \(r.calls.count) tool calls")
        let settled = r.settled
        r.settled = []
        settled.forEach { $0(r) }
        announce(Self.responseKey)
        announce(Self.sessionKey)
        if begun { next(s, at: at ?? context.now()) }
    }
    /// The calls waiting on `s`, begun in order until one generates (D6: one generation a session).
    private func next(_ s: ModelSession, at: Double) {
        while s.responding == nil, !s.queue.isEmpty { s.queue.removeFirst().go(at) }
    }
    /// `work(at)` once the session is free; `r`, the response it begins, holds the session until it ends.
    func queued(_ s: ModelSession, _ r: ModelResponse?, _ work: @escaping (Double) -> Void) {
        s.queue.append((r, { at in
            if let r { guard r.state == "generating" else { return }; s.responding = r }
            work(at)
        }))
        next(s, at: context.now())
    }

    /// A prompt's response: made now, generated once its session is free.
    private func respond(_ s: ModelSession, _ args: [String: Any], streaming: Bool) throws -> ModelResponse {
        var schema: [String: Any]?, typed = false
        if let c = args["responseConstraint"], !(c is NSNull) {
            typed = c is String
            let parsed: Any? = typed ? LanguageModelSchema.parse(c as? String ?? "") : c
            guard let object = parsed as? [String: Any] else { throw Refused(code: "SyntaxError", why: "the constraint is not a JSON Schema object") }
            if let why = LanguageModelSchema.refusal(object) { say("a constraint outside D8's subset: \(why)"); throw Refused(code: "NotSupportedError", why: why) }
            schema = object
        }
        guard let input = args["input"] as? String else { throw Refused(code: "TypeError", why: "`input` is a string") }
        serial += 1
        let r = ModelResponse(id: "response-\(serial)", session: s, schema: schema, typed: typed, omit: args["omitResponseConstraintInput"] as? Bool == true,
                                      whole: !streaming, input: input, request: context.request, options: (args["apple"] as? [String: Any]) ?? [:])
        responses[r.id] = r
        if let request = r.request { byRequest[request] = r }
        s.count += 1
        queued(s, r) { [weak self] at in
            guard let self else { return }
            if self.agent { self.beginAgent(r, at: at) } else { self.beginApple(r) }
        }
        return r
    }

    /// The input fitted to the window, evicting oldest turns (D6): false, with the response failed, when it cannot.
    func admitInput(_ r: ModelResponse, extra measured: Int? = nil) -> Bool {
        let s = r.session, had = s.turns.count
        let extra = measured ?? (Self.cost(r.turn.input ?? "") + (r.schema.map { r.omit ? 0 : Self.schemaCost($0) } ?? 0))
        s.responding = r
        let fits = fit(s, extra)
        // A response reads `overflowed` when its own input evicted a turn.
        r.overflowed = s.turns.count < had
        s.turns.append(r.turn)
        if !fits { end(r, "failed", error: "QuotaExceededError", message: "the input needs \(extra) tokens of \(windowOf(s))"); return false }
        return true
    }

    // MARK: the agent model (§9)

    private func beginAgent(_ r: ModelResponse, at: Double) {
        guard r.state == "generating", admitInput(r) else { return }
        let entry = script.isEmpty ? nil : script.removeFirst()
        if let error = entry?["error"] as? String {
            r.text = ""
            end(r, "failed", error: error, reason: entry?["reason"] as? String, message: entry?["message"] as? String, at: at)
            return
        }
        let calls = entry?["toolCalls"] as? [[String: Any]] ?? []
        if !calls.isEmpty {
            // The handlers run once this answer has reached the runner, which registers the handle they serve.
            r.state = "calling-tool"
            DispatchQueue.main.async { [weak self] in
                self?.runCalls(r, calls) { ok in if ok { self?.plan(r, entry, at: self?.context.now() ?? at) } }
            }
            return
        }
        plan(r, entry, at: at)
    }

    /// The reply's text, checked as a constrained platform checks it, cut into tokens from `at`.
    private func plan(_ r: ModelResponse, _ entry: [String: Any]?, at: Double) {
        guard r.live else { return }
        r.state = "generating"
        var text = entry?["text"] as? String ?? entry?["value"].flatMap { LanguageModelSchema.json($0) }
        if text == nil { text = r.schema.map { LanguageModelSchema.firstInstance($0) } ?? "Agent model reply \(r.session.count)." }
        if let schema = r.schema {
            let why = LanguageModelSchema.parse(text!).map { LanguageModelSchema.violation(schema, $0) } ?? "the text is not JSON"
            // A constrained platform could not produce it (§9): the response fails, and the journal names why.
            if let why { say("\(r.id): the script's reply does not satisfy the constraint: \(why)"); end(r, "failed", error: "SyntaxError", message: why, at: at); return }
        }
        r.planned = Self.tokens(text!)
        r.hold = entry?["hold"] as? Bool == true
        r.start = at
        if !r.calls.isEmpty { announce(Self.responseKey) }
        advance(r)
    }

    /// Each scripted tool call through the runner, in order (D9): false when the response ended meanwhile.
    private func runCalls(_ r: ModelResponse, _ calls: [[String: Any]], _ done: @escaping (Bool) -> Void) {
        guard let call = calls.first else { return done(r.state == "calling-tool") }
        guard r.state == "calling-tool" else { return done(false) }
        let name = call["name"] as? String ?? "", args = call["arguments"] as? [String: Any] ?? [:]
        let why = r.session.tools.first { $0.name == name }.map { LanguageModelSchema.violation($0.schema, args) } ?? "the session lists no tool `\(name)`"
        if let why { say("\(r.id): the script's tool call is refused: \(why)"); end(r, "failed", error: "SyntaxError", message: why); return done(false) }
        toolCall(r, name: name, arguments: args) { [weak self] _ in self?.runCalls(r, Array(calls.dropFirst()), done) }
    }

    /// One handler's run through the runner (D9), its answer kept as the model saw it; `reply` gets the text the model reads.
    func toolCall(_ r: ModelResponse, name: String, arguments: [String: Any], id: String? = nil, reply: @escaping (String) -> Void) {
        guard r.calls.count < Self.toolLimit else {
            end(r, "failed", error: "OperationError", reason: "tool-limit")
            return reply("{\"error\":\"OperationError\",\"message\":\"tool-limit\"}")
        }
        let c = ModelCall(id: id ?? "\(r.id).call-\(r.calls.count + 1)", name: name, arguments: LanguageModelSchema.json(arguments) ?? "{}")
        r.calls.append(c)
        r.turn.calls.append(c)
        if r.state == "generating" { r.state = "calling-tool" }
        announce(Self.responseKey)
        context.invoke(r.handle, name, arguments) { [weak self] answer in
            guard let self else { return }
            let failed = answer["error"] is String
            let full = failed ? (LanguageModelSchema.json(["error": answer["error"] ?? "", "message": answer["message"] ?? ""]) ?? "{}") : (answer["text"] as? String ?? "null")
            let result = Self.cut(full)
            c.state = failed ? "failed" : "done"; c.result = result.text; c.fullResult = full
            if result.truncated { c.truncated = true }
            if r.live { announce(Self.responseKey) }
            reply(full)
        }
    }

    // MARK: the names

    private func abort(_ r: ModelResponse, _ why: String) {
        if agent { advance(r) }
        guard r.live else { return }
        say("\(r.id): \(why)")
        end(r, "aborted", error: "AbortError")
    }

    /// The agent model's availability: §9's, the drive's overlay laid over it.
    private func agentAvailability() -> [String: Any] {
        var a: [String: Any] = ["availability": "available", "contextWindow": agentWindow, "leavesDevice": false, "structuredOutput": "enforced", "tools": true]
        for (k, v) in overlay { a[k] = v }
        if a["availability"] as? String == "downloadable", let p = downloading { a["availability"] = "downloading"; a["progress"] = p }
        if a["availability"] as? String != "unavailable" { a["reason"] = nil }
        if a["availability"] as? String != "downloading" { a["progress"] = nil }
        a["apple"] = NSNull()
        return a
    }

    func availability(_ args: [String: Any]) -> [String: Any] {
        agent ? agentAvailability() : Self.appleAvailability(self, args)
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        let origin = context.origin
        let refused = { (e: Error) in
            let r = e as? Refused
            if let r { self.say("\(name): \(r.code): \(r.why)") }
            reply(["error": r?.code ?? "OperationError"])
        }
        let session = { () -> ModelSession? in (args["session"] as? String).flatMap { self.sessions[$0] } }
        switch name {
        case "languageModelAvailability": reply(availability(args))
        case "createLanguageModel":
            do { try create(args, origin: origin, reply: reply) } catch { refused(error) }
        case "prompt", "promptStreaming":
            guard let s = session() else { return reply(["error": "InvalidStateError"]) }
            // A handler of `s`'s own response prompting `s` would wait on itself (D9, reentry).
            if origin != "app", s.responding?.calls.contains(where: { $0.state == "running" }) == true { return reply(["error": "InvalidStateError"]) }
            do {
                let r = try respond(s, args, streaming: name == "promptStreaming")
                if name == "promptStreaming" { return reply(["id": r.id, "session": s.id]) }
                let settle = { [weak self] (r: ModelResponse) in
                    if let request = r.request { self?.byRequest[request] = nil }
                    if let e = r.error { return reply(["error": e]) }
                    guard r.typed else { return reply(["text": r.text, "contextUsage": r.contextUsage ?? 0, "overflowed": r.overflowed]) }
                    reply((LanguageModelSchema.parse(r.text) as? [String: Any]) ?? ["error": "SyntaxError"])
                }
                // A whole reply the agent model gives at once has ended already.
                if r.live { r.settled.append(settle) } else { settle(r) }
            } catch { refused(error) }
        case "languageModelResponse":
            guard let id = args["response"] as? String, !id.isEmpty else { return reply(Self.noResponse) }
            reply(responses[id].map(responseInfo) ?? ["error": "InvalidStateError"])
        case "abortLanguageModelResponse":
            guard let r = (args["response"] as? String).flatMap({ responses[$0] }) else { return reply(["error": "InvalidStateError"]) }
            abort(r, "aborted by the app")
            reply(["id": r.id])
        case "appendLanguageModel":
            guard let s = session() else { return reply(["error": "InvalidStateError"]) }
            let input = args["input"] as? String ?? ""
            queued(s, nil) { [weak self] _ in
                guard let self else { return }
                guard self.fit(s, Self.cost(input)) else { return reply(["error": "QuotaExceededError"]) }
                s.turns.append(ModelTurn(input: input))
                s.stale = true
                self.announce(Self.sessionKey)
                reply(self.sessionInfo(s))
            }
        case "measureContextUsage":
            guard let s = session() else { return reply(["error": "InvalidStateError"]) }
            if !agent, measureApple(s, args["input"] as? String ?? "", reply: reply) { return }
            let constraint = args["responseConstraint"]
            let schema: Any? = constraint is String ? LanguageModelSchema.parse(constraint as? String ?? "") : constraint
            let usage = Self.cost(args["input"] as? String ?? "") + ((schema as? [String: Any]).map(Self.schemaCost) ?? 0)
            reply(["usage": usage, "contextWindow": windowOf(s)])
        case "languageModelSession":
            guard let id = args["session"] as? String, !id.isEmpty else { return reply(Self.noSession) }
            if agent { advanceAll() }
            reply(sessions[id].map(sessionInfo) ?? ["error": "InvalidStateError"])
        case "cloneLanguageModel":
            guard let s = session() else { return reply(["error": "InvalidStateError"]) }
            queued(s, nil) { [weak self] _ in
                guard let self else { return }
                self.serial += 1
                let turns = s.turns.map { t -> ModelTurn in let c = ModelTurn(input: t.input, reply: t.reply); c.calls = t.calls; return c }
                let c = ModelSession(id: "session-\(self.serial)", system: s.system, turns: turns, tools: s.tools, window: s.window, options: s.options)
                c.overflowed = s.overflowed
                self.sessions[c.id] = c
                self.say("\(c.id): cloned from \(s.id)")
                reply(self.createdInfo(c))
            }
        case "destroyLanguageModel":
            guard let s = session() else { return reply(["error": "InvalidStateError"]) }
            drop(s, "destroyed by the app")
            reply(["id": s.id])
        case "prewarmLanguageModel":
            guard let s = session() else { return reply(["error": "InvalidStateError"]) }
            if !agent { prewarm(s) }
            reply(["id": s.id])
        default:
            reply(["error": "NotSupportedError"])
        }
    }

    private func create(_ args: [String: Any], origin: String, reply: @escaping ([String: Any]) -> Void) throws {
        if agent { advanceAll() }
        let a = availability(args)
        let state = a["availability"] as? String ?? "unavailable"
        if state == "unavailable" {
            return reply(["error": a["reason"] as? String == "model-not-ready" ? "OperationError" : "NotSupportedError"])
        }
        if !(args["tools"] as? [Any] ?? []).isEmpty, a["tools"] as? Bool != true { return reply(["error": "NotSupportedError"]) }
        let made = { [weak self] in
            guard let self else { return }
            do {
                guard let s = try self.makeSession(args, window: a["contextWindow"] as? Int ?? Self.defaultWindow) else { return reply(["error": "QuotaExceededError"]) }
                reply(self.createdInfo(s))
            } catch { reply(["error": (error as? Refused)?.code ?? "TypeError"]) }
        }
        guard state == "downloadable" || state == "downloading" else { return made() }
        if downloading == nil {
            // A download is a person's act (D5, D10): the app's own call, in the foreground.
            guard origin == "app" else { say("create: refused, person-required (\(origin) origin) to start a download"); return reply(["error": "NotAllowedError"]) }
            guard context.foreground else { say("create: refused, a download starts in the foreground"); return reply(["error": "NotAllowedError"]) }
            downloading = 0
            say("download started (agent)")
            announce(Self.availabilityKey)
        }
        waiting.append { ok in ok ? made() : reply(["error": "NotSupportedError"]) }
    }

    /// Under the agent: the preferred availability, laid over the agent model's, and the script, rate and window (§9).
    public func preferred(_ values: [String: Any]) {
        let a = values["languageModelAvailability"] as? [String: Any]
        overlay = a.flatMap { $0["error"] == nil ? $0 : nil } ?? [:]
        let seen = values["responses"].flatMap { LanguageModelSchema.json($0) } ?? "null"
        if seen != scriptSeen {
            scriptSeen = seen
            script = values["responses"] as? [[String: Any]] ?? []
            say("script: \(script.count) replies")
        }
        rate = LanguageModelSchema.number(values["rate"]) ?? Self.tokenMs
        agentWindow = (values["contextWindow"] as? Int).flatMap { $0 > 0 ? $0 : nil } ?? Self.defaultWindow
        advanceAll()
        let now = agentAvailability()["availability"] as? String
        if now == "available", downloading != nil || !waiting.isEmpty {
            downloading = nil
            say("download finished (agent)")
            let w = waiting; waiting = []; w.forEach { $0(true) }
        } else if now == "unavailable", !waiting.isEmpty {
            downloading = nil
            let w = waiting; waiting = []; w.forEach { $0(false) }
        }
    }

    /// A call let go (D15 item 2): its prompt's generation aborted.
    public func cancel(_ request: Int) {
        guard let r = byRequest.removeValue(forKey: request) else { return }
        abort(r, "cancelled: its call (request \(request)) was let go")
    }

    /// Sessions and responses whose reply was dropped: never the app's (D6).
    public func dispose(_ ids: [String]) {
        for id in ids {
            if let s = sessions[id] { drop(s, "its reply was dropped") }
            if let r = responses[id] { abort(r, "its reply was dropped") }
        }
    }

    /// A return to the foreground reads the availability again (D5): Apple Intelligence may have been turned on or off.
    public func foreground(_ shown: Bool) {
        if shown { announce(Self.availabilityKey) }
    }

    /// The end of the app's session: every response aborted, every session dropped (D6).
    public func destroy(_ done: @escaping () -> Void) {
        for s in Array(sessions.values) { drop(s, "the app's session ended") }
        done()
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "languageModelAvailabilitySync" ? availability(args) : ["error": "NotSupportedError"]
    }

    func drop(_ s: ModelSession, _ why: String) {
        for r in Array(responses.values) where r.session === s { abort(r, why); responses[r.id] = nil }
        s.queue = []
        s.platform = nil
        sessions[s.id] = nil
        say("\(s.id): \(why)")
        announce(Self.sessionKey)
    }

    private func dropCaches() {
        for s in sessions.values where s.responding == nil && s.platform != nil { s.platform = nil; s.stale = false }
        say("a memory warning: the platform sessions not responding dropped")
    }
}
