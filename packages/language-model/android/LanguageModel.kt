// @exact/language-model on Android (LLP 1133.016): ML Kit's GenAI Prompt API over AICore's Gemini Nano
// (LanguageModelAndroid.kt), and under the agent the agent model of §9 on the session clock, with Android's limits.
//
// - The transcript is this module's (D6): each session's instructions and turns (an input and the reply). ML Kit has
//   no session, so each generation sends the turns again as one request. web/index.js and LanguageModel.swift keep
//   the same records, by the same rules, so the hosts agree on every count.
// - A response is a value the runner polls (D7): its text so far, which ML Kit's stream fills; only a change of
//   `state` is announced.
// - Structured output is validated, not enforced (D8), and there are no tools (D9): `structuredOutput` reads
//   `validated` and `tools` false, and a create with tools fails `NotSupportedError`.
// - ML Kit generates only for the app in the foreground (D10): a prompt in the background fails `NotAllowedError`,
//   `background`, and one running when the app leaves ends so, with its text so far. One generation runs per process.
package exact.languagemodel

import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject

/** One input of a session and the reply (D6). */
class ModelTurn(var input: String?, var reply: String? = null)

class ModelSession(val id: String, val system: String?, val turns: MutableList<ModelTurn>, val window: Int, val options: JSONObject) {
    var overflowed = false
    val queue = ArrayDeque<Pair<ModelResponse?, (Double) -> Unit>>()
    var responding: ModelResponse? = null
    val ended = ArrayDeque<String>()
    var count = 0
}

class ModelResponse(
    val id: String, val session: ModelSession, val schema: JSONObject?, val typed: Boolean, val omit: Boolean,
    val whole: Boolean, input: String, val request: Int?, val options: JSONObject,
) {
    var state = "generating"
    @Volatile var text = ""
    var overflowed = false
    var error: String? = null
    var reason: String? = null
    var message: String? = null
    var finishReason: Int? = null
    val turn = ModelTurn(input)
    var planned: List<String>? = null
    var hold = false
    var start = 0.0
    var contextUsage: Int? = null
    val settled = mutableListOf<(ModelResponse) -> Unit>()
    /** ML Kit's generation, cancelled by an abort. */
    var job: kotlinx.coroutines.Job? = null
    val live get() = state == "generating" || state == "calling-tool"
}

class LanguageModelModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        const val TOKEN_MS = 25.0
        const val WINDOW = 4096
        const val KEPT = 8
        const val CUT_BYTES = 4096
        const val RESPONSE = "language-model.languageModelResponse"
        const val SESSION = "language-model.languageModelSession"
        const val AVAILABILITY = "language-model.languageModelAvailability"

        /** The app's access hatches (D12), on the main thread and never under the agent: the model's configuration
         * before each client is made, and each request's builder after the words are set. */
        @JvmStatic var modelConfig: ((com.google.mlkit.genai.prompt.ModelConfig.Builder) -> Unit)? = null
        @JvmStatic var request: ((com.google.mlkit.genai.prompt.GenerateContentRequest.Builder) -> Unit)? = null

        fun bytes(s: String) = s.toByteArray(Charsets.UTF_8).size
        fun cost(text: String) = 4 + (bytes(text) + 3) / 4
        fun schemaCost(schema: Any) = (bytes(LanguageModelSchema.json(schema)) + 3) / 4

        /** Tokens as §9 cuts them: at the end of each run of whitespace. */
        fun tokens(text: String): List<String> {
            val out = mutableListOf<String>()
            val current = StringBuilder()
            var lastSpace = false
            for (c in text) {
                if (!c.isWhitespace() && lastSpace && current.isNotEmpty()) { out += current.toString(); current.clear() }
                current.append(c)
                lastSpace = c.isWhitespace()
            }
            if (current.isNotEmpty()) out += current.toString()
            return out
        }
    }

    class Refused(val code: String, val why: String) : Exception(why)

    val agent get() = context.agent
    val sessions = HashMap<String, ModelSession>()
    val responses = HashMap<String, ModelResponse>()
    private val byRequest = HashMap<Int, ModelResponse>()
    private var serial = 0
    // The agent's words (§9) and its preferred availability, laid over the agent model's.
    private var overlay = JSONObject()
    private var script = mutableListOf<JSONObject>()
    private var scriptSeen = ""
    private var rate = TOKEN_MS
    private var agentWindow = WINDOW
    /** A download the agent model started, and the creates waiting on it (D5). */
    private var downloading: Double? = null
    private val waiting = mutableListOf<(Boolean) -> Unit>()
    /** ML Kit, outside the agent (LanguageModelAndroid.kt). */
    private val mlkit: MlKit? = if (context.agent) null else MlKit(this)

    fun say(line: String) = context.log("language-model: $line")
    fun announce(key: String) = context.changed(key)

    // ---------------------------------------------------------------- counting (§9)

    fun windowOf(s: ModelSession) = if (agent) agentWindow else s.window
    private fun turnCost(t: ModelTurn) = (t.input?.let(::cost) ?: 0) + (t.reply?.let(::cost) ?: 0)
    fun counted(s: ModelSession) = (s.system?.let(::cost) ?: 0) + s.turns.sumOf(::turnCost)

    /** Evict the oldest whole turns until `extra` more fits (D6); false when even then it does not. */
    fun fit(s: ModelSession, extra: Int): Boolean {
        val had = s.turns.size
        while (counted(s) + extra > windowOf(s) && s.turns.isNotEmpty()) { s.turns.removeAt(0); s.overflowed = true }
        if (s.turns.size < had) say("${s.id}: evicted ${had - s.turns.size} turns, overflowed, ${counted(s)} of ${windowOf(s)} tokens")
        return counted(s) + extra <= windowOf(s)
    }

    /** `initialPrompts` as the system message and turns: a user message and the assistant messages after it are one. */
    private fun turnsOf(messages: JSONArray): Pair<String?, MutableList<ModelTurn>> {
        var system: String? = null
        val turns = mutableListOf<ModelTurn>()
        for (i in 0 until messages.length()) {
            val m = messages.optJSONObject(i) ?: JSONObject()
            val role = m.optString("role", "user")
            val content = m.optString("content")
            when {
                role == "system" -> { if (i != 0) throw Refused("TypeError", "a system message comes first"); system = content }
                role == "user" || turns.isEmpty() -> turns += ModelTurn(if (role == "user") content else null, if (role == "user") null else content)
                else -> turns.last().let { t -> t.reply = t.reply?.let { "$it\n\n$content" } ?: content }
            }
        }
        return system to turns
    }

    fun sessionInfo(s: ModelSession): JSONObject = JSONObject().put("id", s.id).put("contextUsage", counted(s))
        .put("contextWindow", windowOf(s)).put("responding", s.responding != null || s.queue.isNotEmpty())
    fun createdInfo(s: ModelSession): JSONObject = JSONObject().put("id", s.id).put("contextUsage", counted(s))
        .put("contextWindow", windowOf(s)).put("tools", JSONArray()).put("overflowed", s.overflowed)

    /** A session from a create's arguments, trimmed as a prompt evicts (D6): null when the system message and the newest
     * turn do not fit. */
    fun makeSession(args: JSONObject, window: Int): ModelSession? {
        val (system, turns) = turnsOf(args.optJSONArray("initialPrompts") ?: JSONArray())
        serial += 1
        val s = ModelSession("session-$serial", system, turns, window, args.optJSONObject("android") ?: JSONObject())
        if (!fit(s, 0)) return null
        sessions[s.id] = s
        say("${s.id}: created, ${s.turns.size} turns, ${counted(s)} of ${windowOf(s)} tokens${if (s.overflowed) ", earlier turns evicted" else ""}")
        return s
    }

    // ---------------------------------------------------------------- responses

    fun responseInfo(r: ModelResponse): JSONObject {
        if (agent) advance(r)
        val out = JSONObject().put("id", r.id).put("session", r.session.id).put("state", r.state).put("text", r.text)
            .put("toolCalls", JSONArray()).put("overflowed", r.overflowed)
            .put("android", r.finishReason?.let { JSONObject().put("finishReason", it) } ?: JSONObject.NULL)
        r.contextUsage?.let { out.put("contextUsage", it) }
        r.error?.let { out.put("error", it).put("reason", r.reason ?: JSONObject.NULL).put("message", r.message ?: JSONObject.NULL) }
        return out
    }

    /** Under the agent: the tokens the session clock has reached, and the end once every one has landed. */
    private fun advance(r: ModelResponse) {
        val planned = r.planned ?: return
        if (r.state != "generating") return
        val all = planned.size
        val whole = rate <= 0 || r.whole
        val reached = if (whole) all else ((context.now() - r.start) / rate).toInt()
        val shown = minOf(all - (if (r.hold) 1 else 0), reached)
        r.text = planned.take(maxOf(0, shown)).joinToString("")
        if (shown >= all) end(r, "complete", at = if (whole) r.start else r.start + all * rate)
    }
    private fun advanceAll() = responses.values.toList().forEach(::advance)

    /** The response ends: its turn committed with its text so far (D6), the session's next call begun at `at`. */
    fun end(r: ModelResponse, state: String, error: String? = null, reason: String? = null, message: String? = null, at: Double? = null) {
        if (!r.live) return
        r.state = state
        if (error != null) { r.error = error; r.reason = reason; r.message = message }
        val s = r.session
        val begun = s.responding === r
        // One still waiting its turn never reached the session: nothing of it is committed.
        if (begun) r.turn.reply = r.text else s.queue.removeAll { it.first === r }
        r.contextUsage = counted(s)
        r.job?.cancel()
        if (begun) s.responding = null
        s.ended.addLast(r.id)
        // An ended response is kept until its session has ended eight newer (D6).
        while (s.ended.size > KEPT) { val old = s.ended.removeFirst(); responses.remove(old); say("$old: retired") }
        say("${r.id}: $state${error?.let { " ($it${reason?.let { w -> ", $w" } ?: ""})" } ?: ""}, ${r.text.length} characters, 0 tool calls")
        val settled = r.settled.toList()
        r.settled.clear()
        settled.forEach { it(r) }
        announce(RESPONSE)
        announce(SESSION)
        if (begun) next(s, at ?: context.now())
        mlkit?.ended(r)
    }

    /** The calls waiting on `s`, begun in order until one generates (D6: one generation a session). */
    private fun next(s: ModelSession, at: Double) {
        while (s.responding == null && s.queue.isNotEmpty()) s.queue.removeFirst().second(at)
    }

    /** `work(at)` once the session is free; `r`, the response it begins, holds the session until it ends. */
    fun queued(s: ModelSession, r: ModelResponse?, work: (Double) -> Unit) {
        s.queue.addLast(r to { at: Double ->
            if (r == null || r.state == "generating") { if (r != null) s.responding = r; work(at) }
        })
        next(s, context.now())
    }

    /** A prompt's response: made now, generated once its session is free. */
    private fun respond(s: ModelSession, args: JSONObject, streaming: Boolean): ModelResponse {
        var schema: JSONObject? = null
        var typed = false
        val c = args.opt("responseConstraint")
        if (c != null && c != JSONObject.NULL) {
            typed = c is String
            val parsed = if (typed) LanguageModelSchema.parse(c as String) else c
            schema = parsed as? JSONObject ?: throw Refused("SyntaxError", "the constraint is not a JSON Schema object")
            LanguageModelSchema.refusal(schema)?.let { say("a constraint outside D8's subset: $it"); throw Refused("NotSupportedError", it) }
        }
        val input = args.opt("input") as? String ?: throw Refused("TypeError", "`input` is a string")
        serial += 1
        val r = ModelResponse("response-$serial", s, schema, typed, args.optBoolean("omitResponseConstraintInput"), !streaming,
            input, context.request, args.optJSONObject("android") ?: JSONObject())
        responses[r.id] = r
        r.request?.let { byRequest[it] = r }
        s.count += 1
        queued(s, r) { at -> if (agent) beginAgent(r, at) else mlkit?.begin(r) }
        return r
    }

    /** The input fitted to the window, evicting oldest turns (D6): false, with the response failed, when it cannot. */
    fun admitInput(r: ModelResponse, measured: Int? = null): Boolean {
        val s = r.session
        val had = s.turns.size
        val extra = measured ?: (cost(r.turn.input ?: "") + (r.schema?.let { if (r.omit) 0 else schemaCost(it) } ?: 0))
        s.responding = r
        val fits = fit(s, extra)
        // A response reads `overflowed` when its own input evicted a turn.
        r.overflowed = s.turns.size < had
        s.turns += r.turn
        if (!fits) { end(r, "failed", "QuotaExceededError", message = "the input needs $extra tokens of ${windowOf(s)}"); return false }
        return true
    }

    // ---------------------------------------------------------------- the agent model (§9)

    private fun beginAgent(r: ModelResponse, at: Double) {
        if (r.state != "generating" || !admitInput(r)) return
        val entry = script.removeFirstOrNull()
        entry?.optString("error")?.takeIf { it.isNotEmpty() }?.let {
            r.text = ""
            end(r, "failed", it, entry.optString("reason").ifEmpty { null }, entry.optString("message").ifEmpty { null }, at)
            return
        }
        // Android runs no tools (D9): a scripted call is one the session did not list.
        entry?.optJSONArray("toolCalls")?.takeIf { it.length() > 0 }?.let { calls ->
            val why = "the session lists no tool `${calls.optJSONObject(0)?.optString("name")}`"
            say("${r.id}: the script's tool call is refused: $why")
            end(r, "failed", "SyntaxError", message = why, at = at)
            return
        }
        var text = entry?.opt("text") as? String ?: entry?.opt("value")?.let(LanguageModelSchema::json)
        if (text == null) text = r.schema?.let { LanguageModelSchema.firstInstance(it) } ?: "Agent model reply ${r.session.count}."
        r.schema?.let { schema ->
            val why = LanguageModelSchema.parse(text).let { if (it == null) "the text is not JSON" else LanguageModelSchema.violation(schema, it) }
            // A validating platform would fail it (§9): the response fails, and the journal names why.
            if (why != null) { say("${r.id}: the script's reply does not satisfy the constraint: $why"); end(r, "failed", "SyntaxError", message = why, at = at); return }
        }
        r.planned = tokens(text)
        r.hold = entry?.optBoolean("hold") == true
        r.start = at
        advance(r)
    }

    // ---------------------------------------------------------------- the names

    private fun abort(r: ModelResponse, why: String) {
        if (agent) advance(r)
        if (!r.live) return
        say("${r.id}: $why")
        end(r, "aborted", "AbortError")
    }

    /** The agent model's availability: §9's with Android's limits, the drive's overlay laid over it. */
    private fun agentAvailability(args: JSONObject): JSONObject {
        val a = JSONObject().put("availability", "available").put("contextWindow", agentWindow).put("leavesDevice", false)
            .put("structuredOutput", "validated").put("tools", false)
        for (k in overlay.keys()) a.put(k, overlay.get(k))
        if (a.optString("availability") == "downloadable") downloading?.let { a.put("availability", "downloading").put("progress", it) }
        if (wantsTools(args)) a.put("availability", "unavailable").put("reason", "unsupported-options")
        if (a.optString("availability") != "unavailable") a.remove("reason")
        if (a.optString("availability") != "downloading") a.remove("progress")
        return a.put("android", JSONObject.NULL)
    }

    /** Tool use asked by `expectedOutputs`, which Android does not offer (D9). */
    fun wantsTools(args: JSONObject): Boolean {
        val outputs = args.optJSONArray("expectedOutputs") ?: return false
        return (0 until outputs.length()).any { outputs.optJSONObject(it)?.optString("type") == "tool-call" }
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) = later(name, args, null, reply)

    override fun later(name: String, args: JSONObject, preferred: Any?, reply: (JSONObject) -> Unit) {
        val origin = context.origin
        val session = { sessions[args.optString("session")] }
        try {
            when (name) {
                "languageModelAvailability" -> if (agent) reply(agentAvailability(args)) else mlkit!!.availability(args, reply)
                "createLanguageModel" -> create(args, origin, reply)
                "prompt", "promptStreaming" -> {
                    val s = session() ?: return reply(error("InvalidStateError"))
                    // ML Kit generates only for the app in the foreground (D10).
                    if (!context.foreground) return reply(JSONObject().put("error", "NotAllowedError").put("reason", "background"))
                    val r = respond(s, args, name == "promptStreaming")
                    if (name == "promptStreaming") return reply(JSONObject().put("id", r.id).put("session", s.id))
                    val settle = { done: ModelResponse ->
                        done.request?.let { byRequest.remove(it) }
                        when {
                            done.error != null -> reply(error(done.error!!))
                            !done.typed -> reply(JSONObject().put("text", done.text).put("contextUsage", done.contextUsage ?: 0).put("overflowed", done.overflowed))
                            else -> reply(LanguageModelSchema.parse(done.text) as? JSONObject ?: error("SyntaxError"))
                        }
                    }
                    // A whole reply the agent model gives at once has ended already.
                    if (r.live) r.settled += settle else settle(r)
                }
                "languageModelResponse" -> {
                    val id = args.optString("response")
                    if (id.isEmpty()) return reply(JSONObject().put("id", "").put("session", "").put("state", "complete").put("text", "").put("toolCalls", JSONArray()).put("overflowed", false))
                    reply(responses[id]?.let(::responseInfo) ?: error("InvalidStateError"))
                }
                "abortLanguageModelResponse" -> {
                    val r = responses[args.optString("response")] ?: return reply(error("InvalidStateError"))
                    abort(r, "aborted by the app")
                    reply(JSONObject().put("id", r.id))
                }
                "appendLanguageModel" -> {
                    val s = session() ?: return reply(error("InvalidStateError"))
                    val input = args.optString("input")
                    queued(s, null) {
                        if (!fit(s, cost(input))) return@queued reply(error("QuotaExceededError"))
                        s.turns += ModelTurn(input)
                        announce(SESSION)
                        reply(sessionInfo(s))
                    }
                }
                "measureContextUsage" -> {
                    val s = session() ?: return reply(error("InvalidStateError"))
                    val c = args.opt("responseConstraint")
                    val schema = if (c is String) LanguageModelSchema.parse(c) else c
                    val estimate = cost(args.optString("input")) + ((schema as? JSONObject)?.let(::schemaCost) ?: 0)
                    if (agent) reply(JSONObject().put("usage", estimate).put("contextWindow", windowOf(s)))
                    else mlkit!!.measure(s, args.optString("input")) { usage -> reply(JSONObject().put("usage", usage ?: estimate).put("contextWindow", windowOf(s))) }
                }
                "languageModelSession" -> {
                    val id = args.optString("session")
                    if (id.isEmpty()) return reply(JSONObject().put("id", "").put("contextWindow", 0).put("responding", false))
                    if (agent) advanceAll()
                    reply(sessions[id]?.let(::sessionInfo) ?: error("InvalidStateError"))
                }
                "cloneLanguageModel" -> {
                    val s = session() ?: return reply(error("InvalidStateError"))
                    queued(s, null) {
                        serial += 1
                        val c = ModelSession("session-$serial", s.system, s.turns.map { ModelTurn(it.input, it.reply) }.toMutableList(), s.window, s.options)
                        c.overflowed = s.overflowed
                        sessions[c.id] = c
                        say("${c.id}: cloned from ${s.id}")
                        reply(createdInfo(c))
                    }
                }
                "destroyLanguageModel" -> {
                    val s = session() ?: return reply(error("InvalidStateError"))
                    drop(s, "destroyed by the app")
                    reply(JSONObject().put("id", s.id))
                }
                "prewarmLanguageModel" -> {
                    val s = session() ?: return reply(error("InvalidStateError"))
                    mlkit?.prewarm(s)
                    reply(JSONObject().put("id", s.id))
                }
                else -> reply(error("NotSupportedError"))
            }
        } catch (e: Refused) {
            say("$name: ${e.code}: ${e.why}")
            reply(error(e.code))
        }
    }

    private fun error(code: String) = JSONObject().put("error", code)

    private fun create(args: JSONObject, origin: String, reply: (JSONObject) -> Unit) {
        if (agent) advanceAll()
        if ((args.optJSONArray("tools")?.length() ?: 0) > 0) return reply(error("NotSupportedError"))
        val made = { window: Int ->
            try { reply(makeSession(args, window)?.let(::createdInfo) ?: error("QuotaExceededError")) } catch (e: Refused) { reply(error(e.code)) }
        }
        if (!agent) return mlkit!!.create(args, origin, made, reply)
        val state = agentAvailability(args).optString("availability")
        if (state == "unavailable") return reply(error("NotSupportedError"))
        if (state == "available") return made(agentWindow)
        if (downloading == null) {
            // A download is a person's act (D5, D10): the app's own call, in the foreground.
            if (!startsDownload(origin)) return reply(error("NotAllowedError"))
            downloading = 0.0
            say("download started (agent)")
            announce(AVAILABILITY)
        }
        waiting += { ok -> if (ok) made(agentWindow) else reply(error("NotSupportedError")) }
    }

    /** Whether a create from `origin` may start a download (D5, D10), journaled when it may not. */
    fun startsDownload(origin: String): Boolean {
        if (origin != "app") { say("create: refused, person-required ($origin origin) to start a download"); return false }
        if (!context.foreground) { say("create: refused, a download starts in the foreground"); return false }
        return true
    }

    /** Under the agent: the preferred availability, laid over the agent model's, and the script, rate and window (§9). */
    override fun preferred(values: JSONObject) {
        overlay = values.optJSONObject("languageModelAvailability")?.takeIf { !it.has("error") } ?: JSONObject()
        val seen = values.opt("responses")?.let(LanguageModelSchema::json) ?: "null"
        if (seen != scriptSeen) {
            scriptSeen = seen
            val list = values.optJSONArray("responses") ?: JSONArray()
            script = (0 until list.length()).mapNotNull { list.optJSONObject(it) }.toMutableList()
            say("script: ${script.size} replies")
        }
        rate = LanguageModelSchema.number(values.opt("rate")) ?: TOKEN_MS
        agentWindow = (values.opt("contextWindow") as? Number)?.toInt()?.takeIf { it > 0 } ?: WINDOW
        advanceAll()
        val now = agentAvailability(JSONObject()).optString("availability")
        if (now == "available" && (downloading != null || waiting.isNotEmpty())) {
            downloading = null
            say("download finished (agent)")
            settleWaiting(true)
        } else if (now == "unavailable" && waiting.isNotEmpty()) {
            downloading = null
            settleWaiting(false)
        }
    }

    private fun settleWaiting(ok: Boolean) {
        val w = waiting.toList()
        waiting.clear()
        w.forEach { it(ok) }
    }

    /** A call let go (D15 item 2): its prompt's generation aborted. */
    override fun cancel(request: Int) {
        val r = byRequest.remove(request) ?: return
        abort(r, "cancelled: its call (request $request) was let go")
    }

    /** Sessions and responses whose reply was dropped: never the app's (D6). */
    override fun dispose(ids: List<String>) {
        for (id in ids) {
            sessions[id]?.let { drop(it, "its reply was dropped") }
            responses[id]?.let { abort(it, "its reply was dropped") }
        }
    }

    /** A return to the foreground reads the availability again (D5); leaving it ends what generates (D10). */
    override fun foreground(shown: Boolean) {
        if (shown) { announce(AVAILABILITY); return }
        for (r in responses.values.toList()) if (r.live && r.session.responding === r) {
            say("${r.id}: the app left the foreground")
            if (agent) advance(r)
            end(r, "failed", "NotAllowedError", "background")
        }
    }

    /** The end of the app's session: every response aborted, every session dropped (D6). */
    override fun destroy(done: () -> Unit) {
        for (s in sessions.values.toList()) drop(s, "the app's session ended")
        mlkit?.close()
        done()
    }

    fun drop(s: ModelSession, why: String) {
        for (r in responses.values.toList()) if (r.session === s) { abort(r, why); responses.remove(r.id) }
        s.queue.clear()
        sessions.remove(s.id)
        say("${s.id}: $why")
        announce(SESSION)
    }
}
