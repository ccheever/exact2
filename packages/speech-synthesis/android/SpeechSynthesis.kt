// @exact/speech-synthesis on Android (LLP 1133.012 D2–D6, §9): the process's `TextToSpeech`, answered in the
// interface's shapes.
//
// - The engine binds asynchronously (`OnInitListener`): it is made at the module's first name, and utterances spoken
//   before it is ready wait in this implementation's queue, in order. The engine is given one utterance at a time,
//   the next when the last ends, so `cancelSpeech`, `pauseSpeech` and a dropped handle act on this queue, not the
//   engine's. Android runs one session a process, so the queue is the process's.
// - `TextToSpeech` has no pause: `pauseSpeech` stops the engine and keeps the last range `onRangeStart` reported,
//   and resuming speaks the utterance again from that range's start (from its start if the engine reported none). A
//   word may repeat; none is skipped (D3).
// - Spoken audio claims audio focus through the reader's owner (D4, D12 item 2) with the queue's strongest type while
//   it speaks. A paused queue holds no claim, except one paused only by a transient loss, which keeps its request so
//   that focus comes back to it. A transient loss pauses the queue until focus returns; a lasting loss pauses it until
//   `resumeSpeech`. An unsaid type is `transient`: the app's `audio_session` is not carried to Android.
// - Leaving the foreground cancels a `transient` or `transient-solo` queue and pauses a `playback` one
//   (`background`) until the activity resumes and focus is granted again (a refused request pauses it `focus`).
// - The `UtteranceProgressListener` is called on a binder thread; each call posts to the main looper and names its
//   utterance by the engine id it was spoken with.
// - Tier 3 (D5): `SpeechSynthesisModule.hatch` is given the `TextToSpeech` once, after `onInit` succeeds.
// - Under the agent nothing is spoken (§9), as on the web and Apple: each utterance is spoken on the session clock at
//   170 words a minute times `rate`, a `word` boundary at each word's start, and `end` after the last; a paused
//   queue's clock stands still.
package exact.speechsynthesis

import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import android.speech.tts.Voice
import dev.exact.canvas.AudioClaim
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import java.util.Locale
import org.json.JSONArray
import org.json.JSONObject

class SpeechSynthesisModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's access hatch (D5): the engine, once, after it is ready. */
        @JvmStatic var hatch: ((TextToSpeech) -> Unit)? = null
        const val WPM = 170.0
        val strength = listOf("transient", "transient-solo", "playback")
    }

    private class Utterance(val id: String, val text: String, val type: String, val rate: Double, val args: JSONObject) {
        val words = words(text)
        var state = "queued"; var outcome: String? = null; var error: String? = null
        var began = 0.0; var paused: Double? = null; var failure: String? = null
        /** Where the engine's text begins in [text]: a resumed utterance is spoken again from its last range. */
        var offset = 0
        /** The id the engine was last given it under, so a callback from an earlier attempt is ignored. */
        var spokenAs: String? = null
        var attempts = 0
    }

    private val main = Handler(Looper.getMainLooper())
    private var tts: TextToSpeech? = null
    private var ready = false
    private var failedInit = false
    private val utterances = HashMap<String, Utterance>()
    private val queue = ArrayList<String>()
    private var current: String? = null
    private var boundary = noBoundary()
    /** Why the queue is paused, in the order the reasons came. */
    private val reasons = ArrayList<String>()
    private var claim: AudioClaim? = null
    private var claimed = "auto"
    private var fail: String? = null
    private var failSeen: String? = null
    private var next = 0
    private val voiceWaiters = ArrayList<(JSONArray) -> Unit>()

    private fun noBoundary() = JSONObject().put("charIndex", 0).put("charLength", 0).put("name", "").put("elapsedTime", 0)
    private fun say(line: String) = context.log("speech-synthesis: $line")
    private fun announce() {
        context.changed("speech-synthesis.speechSynthesisState")
        context.changed("speech-synthesis.speechUtterance")
    }
    private fun error(code: String) = JSONObject().put("error", code)

    // ------------------------------------------------------------ the names

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        if (!context.agent) engine(args)
        when (name) {
            "speak" -> reply(speak(args))
            "cancelSpeech" -> { cancelAll(); reply(state()) }
            "pauseSpeech" -> { pause("requested"); reply(state()) }
            "resumeSpeech" -> { resume(listOf("requested", "interrupted", "focus", "route")); reply(state()) }
            "speechSynthesisState" -> reply(state())
            "speechUtterance" -> {
                val id = args.optString("utterance")
                if (id.isEmpty()) return reply(JSONObject().put("id", "").put("state", "ended").put("outcome", JSONObject.NULL).put("error", JSONObject.NULL))
                advance()
                reply(utterances[id]?.let(::shape) ?: error("NotFoundError"))
            }
            "speechSynthesisVoices" -> when {
                context.agent -> reply(JSONObject().put("voices", agentVoices()))
                ready || failedInit -> reply(JSONObject().put("voices", voices()))
                else -> voiceWaiters += { reply(JSONObject().put("voices", it)) }
            }
            else -> reply(error("NotSupportedError"))
        }
    }

    /** The engine, made at the module's first name: `android.engine` written on that call picks it (D5). */
    private fun engine(args: JSONObject) {
        if (tts != null || failedInit) return
        val engine = args.optJSONObject("android")?.optString("engine")?.ifEmpty { null }
        tts = TextToSpeech(context.application, { status -> main.post { initialized(status) } }, engine)
    }

    private fun initialized(status: Int) {
        val t = tts ?: return
        if (status != TextToSpeech.SUCCESS) {
            failedInit = true
            say("the engine did not start ($status)")
            for (id in queue.toList()) utterances[id]?.let { end(it, "error", "synthesis-unavailable") }
            current?.let { utterances[it] }?.let { end(it, "error", "synthesis-unavailable") }
        } else {
            ready = true
            t.setOnUtteranceProgressListener(progress)
            hatch?.invoke(t)
        }
        val list = voices()
        voiceWaiters.toList().forEach { it(list) }
        voiceWaiters.clear()
        context.changed("speech-synthesis.speechSynthesisVoices")
        proceed()
        announce()
    }

    private fun speak(args: JSONObject): JSONObject {
        advance()
        next += 1
        val id = "utterance-${context.session}-$next"
        val written = args.optString("audioSessionType").takeIf { it in strength }
        val u = Utterance(id, args.optString("text"), written ?: "transient", if (args.has("rate")) args.optDouble("rate", 1.0) else 1.0, args)
        utterances[id] = u
        if (failedInit) { end(u, "error", "synthesis-unavailable"); announce(); return JSONObject().put("id", id) }
        if (!context.agent && u.text.length > TextToSpeech.getMaxSpeechInputLength()) {
            end(u, "error", "text-too-long")
            announce()
            return JSONObject().put("id", id)
        }
        queue += id
        say("$id: queued (${u.words.size} words, ${u.type})")
        proceed()
        announce()
        return JSONObject().put("id", id)
    }

    // ------------------------------------------------------------ the queue

    /** [u] begins at session time [at]: the engine is given it, or the agent's clock starts it. */
    private fun begin(u: Utterance, at: Double) {
        current = u.id; u.state = "speaking"; u.began = at; u.paused = null
        boundary = noBoundary()
        if (context.agent) { fail?.let { u.failure = it; fail = null } }
        // The claim before the engine speaks, as the guide asks (D4).
        reclaim()
        if (!context.agent && claim != null && reasons.isEmpty()) give(u)
    }

    /** The engine speaks [u] from [Utterance.offset], with its words (D3); a word it cannot honour ends it. */
    private fun give(u: Utterance) {
        val t = tts ?: return
        val voice = u.args.optString("voice")
        val lang = u.args.optString("lang")
        val refused = when {
            voice.isNotEmpty() -> {
                val found = t.voices?.firstOrNull { it.name == voice }
                if (found == null) "voice-unavailable" else { t.voice = found; null }
            }
            lang.isNotEmpty() -> {
                val locale = Locale.forLanguageTag(lang)
                if (t.isLanguageAvailable(locale) < TextToSpeech.LANG_AVAILABLE) "language-unavailable" else { t.language = locale; null }
            }
            else -> { t.defaultVoice?.let { t.voice = it }; null }
        }
        if (refused != null) return end(u, "error", refused)
        t.setSpeechRate(u.rate.toFloat())
        t.setPitch(if (u.args.has("pitch")) u.args.optDouble("pitch", 1.0).toFloat() else 1f)
        val params = Bundle()
        if (u.args.has("volume")) params.putFloat(TextToSpeech.Engine.KEY_PARAM_VOLUME, u.args.optDouble("volume", 1.0).toFloat().coerceIn(0f, 1f))
        u.attempts += 1
        val engineId = "${u.id}#${u.attempts}"
        u.spokenAs = engineId
        if (t.speak(u.text.substring(u.offset), TextToSpeech.QUEUE_FLUSH, params, engineId) != TextToSpeech.SUCCESS) end(u, "error", "synthesis-failed")
    }

    /** The next queued utterance begins, unless the queue is paused, one speaks, or the engine is not ready. */
    private fun proceed(at: Double? = null) {
        if (current == null && queue.isNotEmpty() && reasons.isEmpty() && (context.agent || ready)) {
            utterances[queue.removeAt(0)]?.let { begin(it, at ?: context.now()) }
        }
        if (current == null && queue.isEmpty()) boundary = noBoundary()
        reclaim()
    }

    /** [u] ends with [outcome] (and [error]) at session time [at]. */
    private fun end(u: Utterance, outcome: String, error: String?, at: Double? = null) {
        if (u.state == "ended") return
        u.state = "ended"; u.outcome = outcome; u.error = error; u.spokenAs = null
        if (outcome == "error" && error != null) say("${u.id}: $error")
        queue.remove(u.id)
        if (current == u.id) { current = null; proceed(at) }
        reclaim()
    }

    /** Under the agent, the queue moved to the session clock's now (§9). */
    private fun advance() {
        if (!context.agent) return
        val t = context.now()
        while (true) {
            val u = current?.let { utterances[it] } ?: return
            if (reasons.isNotEmpty()) { if (u.paused == null) u.paused = t; return }
            u.paused?.let { u.began += t - it; u.paused = null }
            u.failure?.let { end(u, "error", it, u.began); continue }
            val per = 60000 / (WPM * (if (u.rate > 0) u.rate else 1.0))
            val n = (maxOf(0.0, t - u.began) / per).toInt()
            if (n >= u.words.size) { end(u, "end", null, u.began + u.words.size * per); continue }
            val (start, length) = u.words[n]
            boundary = JSONObject().put("charIndex", start).put("charLength", length).put("name", "word").put("elapsedTime", n * per)
            return
        }
    }

    // ------------------------------------------------------------ the claim (D4)

    /** The strongest type among the utterances not yet ended, `auto` for none. */
    private fun strongest(): String {
        val ids = listOfNotNull(current) + queue
        return ids.mapNotNull { utterances[it]?.let { u -> strength.indexOf(u.type) } }.maxOrNull()?.let { strength[it] } ?: "auto"
    }

    /** Claim while the queue speaks and is not paused, or is paused only by a transient loss, whose regain it waits
     * for; the type chosen again whenever the queue changes. */
    private fun reclaim() {
        val holds = (current != null || queue.isNotEmpty()) && reasons.all { it == "focus-transient" }
        val want = if (holds) strongest() else "auto"
        if (want == claimed) return
        claim?.release(); claim = null; claimed = want
        if (want == "auto" || context.agent) return
        claim = context.audioFocus.claim(want) { heard(it) }
        if (claim == null) {
            say("audio focus was refused for $want")
            claimed = "auto"
            pause("focus")
        }
    }

    /** What the owner tells a claim (D4): a transient loss pauses until focus returns; a lasting one until
     * `resumeSpeech`. */
    private fun heard(event: String) {
        when (event) {
            "focus-transient" -> pause("focus-transient")
            "focus" -> pause("focus")
            "focus-gained" -> resume(listOf("focus-transient"))
        }
    }

    // ------------------------------------------------------------ values

    private fun state(): JSONObject {
        advance()
        val out = JSONObject()
            .put("available", context.agent || !failedInit)
            .put("speaking", current != null).put("pending", queue.isNotEmpty()).put("paused", reasons.isNotEmpty())
            .put("pauseReasons", JSONArray(reasons)).put("audioSessionType", claimed)
            .put("utterance", current ?: JSONObject.NULL).put("queue", JSONArray(queue)).put("apple", JSONObject.NULL)
        for (key in boundary.keys()) out.put(key, boundary.get(key))
        return out
    }

    private fun shape(u: Utterance) = JSONObject().put("id", u.id)
        .put("state", if (u.state == "speaking" && reasons.isNotEmpty()) "paused" else u.state)
        .put("outcome", u.outcome ?: JSONObject.NULL).put("error", u.error ?: JSONObject.NULL)

    private fun agentVoices(): JSONArray {
        val base = Locale.getDefault().toLanguageTag()
        return JSONArray()
            .put(JSONObject().put("voiceURI", "agent:en-US").put("name", "Agent voice").put("lang", "en-US").put("localService", true)
                .put("default", true).put("apple", JSONObject.NULL).put("android", JSONObject.NULL))
            .put(JSONObject().put("voiceURI", "agent:$base").put("name", "Agent voice ($base)").put("lang", base).put("localService", true)
                .put("default", false).put("apple", JSONObject.NULL).put("android", JSONObject.NULL))
    }

    /** The engine's voices (D5's `android` words); none before it is ready. */
    private fun voices(): JSONArray {
        val t = tts ?: return JSONArray()
        if (!ready) return JSONArray()
        val fallback = t.defaultVoice?.name
        val out = JSONArray()
        for (v: Voice in (t.voices ?: emptySet<Voice>()).sortedBy { it.name }) {
            out.put(JSONObject().put("voiceURI", v.name).put("name", v.name).put("lang", v.locale.toLanguageTag())
                .put("localService", !v.isNetworkConnectionRequired).put("default", v.name == fallback).put("apple", JSONObject.NULL)
                .put("android", JSONObject().put("quality", v.quality).put("latency", v.latency)
                    .put("requiresNetworkConnection", v.isNetworkConnectionRequired).put("features", JSONArray(v.features.sorted()))))
        }
        return out
    }

    // ------------------------------------------------------------ pause, resume, cancel

    /** Every utterance still to end ends: the queued `canceled`, the speaking `interrupted` (D6). */
    private fun cancelAll() {
        advance()
        for (id in queue) utterances[id]?.let { it.state = "ended"; it.outcome = "error"; it.error = "canceled"; it.spokenAs = null }
        queue.clear()
        current?.let { utterances[it] }?.let { it.state = "ended"; it.outcome = "error"; it.error = "interrupted"; it.spokenAs = null }
        current = null
        reasons.remove("requested")
        boundary = noBoundary()
        tts?.stop()
        reclaim()
        announce()
    }

    private fun pause(reason: String) {
        advance()
        val u = current?.let { utterances[it] }
        if (u != null && u.paused == null) {
            u.paused = context.now()
            // The engine stops; the utterance is spoken again from the start of the last range it reported (D3).
            if (!context.agent && u.spokenAs != null) {
                u.offset = boundary.optInt("charIndex", u.offset)
                u.spokenAs = null
                tts?.stop()
            }
        }
        if (reason !in reasons) reasons += reason
        reclaim()
        announce()
    }

    private fun resume(lifted: List<String>) {
        advance()
        reasons.removeAll(lifted)
        if (reasons.isEmpty()) {
            // The claim again before the engine goes on (D4).
            reclaim()
            val u = current?.let { utterances[it] }
            if (u != null && !context.agent && claim != null && reasons.isEmpty() && u.spokenAs == null && u.paused != null) { u.paused = null; give(u) }
            proceed()
        }
        reclaim()
        announce()
    }

    // ------------------------------------------------------------ the engine's callbacks, on a binder thread

    private fun on(engineId: String?, f: (Utterance) -> Unit) = main.post {
        val u = current?.let { utterances[it] } ?: return@post
        if (engineId != null && u.spokenAs == engineId) { f(u); announce() }
    }

    private val progress = object : UtteranceProgressListener() {
        override fun onStart(utteranceId: String?) {}
        override fun onDone(utteranceId: String?) {
            on(utteranceId) { end(it, "end", null) }
        }
        @Deprecated("Deprecated in Java")
        override fun onError(utteranceId: String?) {
            on(utteranceId) { end(it, "error", "synthesis-failed") }
        }
        override fun onError(utteranceId: String?, errorCode: Int) {
            on(utteranceId) { end(it, "error", errorWord(errorCode)) }
        }
        // A range reported as the engine stops for a pause is the one spoken at resume, so a paused mark holds.
        override fun onRangeStart(utteranceId: String?, start: Int, end: Int, frame: Int) {
            on(utteranceId) { u ->
                if (reasons.isEmpty()) boundary = JSONObject().put("charIndex", u.offset + start).put("charLength", end - start).put("name", "word")
                    .put("elapsedTime", maxOf(0.0, context.now() - u.began))
            }
        }
    }

    /** `TextToSpeech`'s error codes as the specification's words. */
    private fun errorWord(code: Int) = when (code) {
        TextToSpeech.ERROR_SERVICE, TextToSpeech.ERROR_NOT_INSTALLED_YET -> "synthesis-unavailable"
        TextToSpeech.ERROR_OUTPUT -> "audio-hardware"
        TextToSpeech.ERROR_NETWORK, TextToSpeech.ERROR_NETWORK_TIMEOUT -> "network"
        TextToSpeech.ERROR_INVALID_REQUEST -> "invalid-argument"
        else -> "synthesis-failed"
    }

    // ------------------------------------------------------------ the session

    /** Under the agent: `fail` makes the next utterance to begin end with that error (§9), armed once per value. */
    override fun preferred(values: JSONObject) {
        val word = values.optString("fail").ifEmpty { null }
        if (word == failSeen) return
        failSeen = word
        if (word == null) return
        advance()
        fail = word
        say("the next utterance fails $word")
    }

    /** Utterances whose reply was dropped: never the app's (D2). */
    override fun dispose(ids: List<String>) {
        advance()
        for (id in ids) {
            val u = utterances[id] ?: continue
            if (u.state == "ended") continue
            say("$id: its reply was dropped")
            val speaking = current == id
            if (speaking) tts?.stop()
            end(u, "error", if (speaking) "interrupted" else "canceled")
        }
        announce()
    }

    /** A prompt the person can no longer see the reason for is canceled at the background; a `playback` queue pauses
     * until the activity resumes (D4). */
    override fun foreground(shown: Boolean) {
        if (shown) {
            if ("background" in reasons) resume(listOf("background"))
            return
        }
        if (current == null && queue.isEmpty()) return
        if (strongest() != "playback") { say("left the foreground: the queue is canceled"); return cancelAll() }
        pause("background")
    }

    /** The session ends: the engine stops and shuts down, and the claim goes (D6). */
    override fun destroy(done: () -> Unit) {
        cancelAll()
        tts?.shutdown()
        tts = null
        claim?.release(); claim = null
        done()
    }
}

/** Each whitespace-separated word of [text]: its start and length in UTF-16 code units, as the web counts. */
private fun words(text: String): List<Pair<Int, Int>> {
    val out = ArrayList<Pair<Int, Int>>()
    var start = -1
    for (i in text.indices) {
        val space = text[i] == ' ' || text[i] in '\t'..'\r' || text[i] == ' '
        if (space && start >= 0) { out += start to (i - start); start = -1 } else if (!space && start < 0) start = i
    }
    if (start >= 0) out += start to (text.length - start)
    return out
}
