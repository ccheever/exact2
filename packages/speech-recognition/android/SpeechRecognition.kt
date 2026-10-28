// @exact/speech-recognition on Android (LLP 1133.012 D7–D11, §9): `SpeechRecognizer`, answered in the interface's
// shapes.
//
// - The recognizer (D9): `createSpeechRecognizer`, the default service, which may stream audio to a server; with
//   `processLocally=true`, `createOnDeviceSpeechRecognizer` (API 31) where `isOnDeviceRecognitionAvailable`, else the
//   recognition ends `service-not-allowed` before anything opens. `android.recognitionService` names another service.
// - Inputs (D8): `microphone=true`, which the service captures itself, under an exclusive hold of the session's
//   microphone (the track table's `hold`), refused `NotReadableError` while a capture runs. A track or a file goes to a service through a
//   pipe (`EXTRA_AUDIO_SOURCE`) only where the service is on the qualified list, which a device run builds; the list
//   is empty, so the status lists `microphone` alone and a `track=` or `path=` start is `NotSupportedError`.
// - Permission (D11): `RECORD_AUDIO`, for every input; a start asks for it when the state is `prompt`.
// - `continuous=true` starts a new session at each end, the seam journaled, since the class "is not intended to be
//   used for continuous recognition".
// - A recognition is an id (D7), its state a value: final results numbered from 0, the last `history` of them (20 by
//   default, 200 at most, 256 KB) with the interim one. Every final goes, as it arrives, to the `transcript=` directory
//   as JSON Lines files of at most 1 MiB, the directory emptied when the recognition starts.
// - A reply the runner drops aborts its recognition; leaving the foreground aborts every one (D10), its finals kept.
// - Tier 3 (D5): `SpeechRecognitionModule.hatch` is given each `RecognizerIntent` after the module's extras are set.
// - Under the agent no recognizer is made (§9), as on the web and Apple: a recognition hears what the drive says
//   (`heard`), one word as an interim result every 300 ms of the session clock and the final 500 ms after the last
//   word; a non-continuous one then ends, and with no phrase ends `no-speech` after 8 s; a `path=` one answers the
//   phrase preferred at its start, at once.
package exact.speechrecognition

import android.Manifest
import android.annotation.TargetApi
import android.content.ComponentName
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.speech.RecognitionListener
import android.speech.RecognitionSupport
import android.speech.RecognitionSupportCallback
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import dev.exact.canvas.TrackConsumer
import dev.exact.canvas.TrackError
import java.io.File
import java.util.concurrent.Executors
import org.json.JSONArray
import org.json.JSONObject

class SpeechRecognitionModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's access hatch (D5): each intent, after the module's extras are set, before `startListening`. */
        @JvmStatic var hatch: ((Intent) -> Unit)? = null
        const val HISTORY = 20
        const val MAX_HISTORY = 200
        const val WINDOW_BYTES = 256 * 1024
        const val FILE_BYTES = 1 shl 20
        const val SPLIT = 16 * 1024
        const val WORD_MS = 300.0
        const val FINAL_MS = 500.0
        const val NO_SPEECH_MS = 8000.0
    }

    private class Phrase(val text: String, val words: List<String>, val at: Double)

    private val main = Handler(Looper.getMainLooper())
    private val recognitions = HashMap<String, Recognition>()
    private val phrases = ArrayList<Phrase>()
    private var overlay = JSONObject()
    private var heard: String? = null
    private var next = 0

    internal fun say(line: String) = context.log("speech-recognition: $line")
    internal fun announce() = context.changed("speech-recognition.speechRecognitionState")
    private fun error(code: String) = JSONObject().put("error", code)

    private fun noInfo() = JSONObject().put("id", "").put("leavesDevice", false).put("finalCount", 0).put("transcript", JSONObject.NULL)
        .put("files", 0).put("error", JSONObject.NULL)
    private fun noRecognition() = JSONObject().put("id", "").put("state", "ended").put("audio", false).put("sound", false).put("speech", false)
        .put("results", JSONArray()).put("finalCount", 0).put("interim", JSONObject.NULL).put("leavesDevice", false)
        .put("error", JSONObject.NULL).put("message", JSONObject.NULL)

    // ------------------------------------------------------------ the names

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        val id = args.optString("recognition")
        val r = recognitions[id]
        when (name) {
            "startSpeechRecognition" -> start(args, reply)
            "stopSpeechRecognition" -> when {
                id.isEmpty() -> reply(noInfo())
                r == null -> reply(error("NotFoundError"))
                else -> { advance(r); r.stop(reply) }
            }
            // An abort of no recognition answers none: an app supersedes a start still in flight with it.
            "abortSpeechRecognition" -> when {
                id.isEmpty() -> reply(noInfo())
                r == null -> reply(error("NotFoundError"))
                else -> { advance(r); r.abort("aborted", reply) }
            }
            "speechRecognitionState" -> when {
                id.isEmpty() -> reply(noRecognition())
                r == null -> reply(error("NotFoundError"))
                else -> { advance(r); reply(r.shape()) }
            }
            "speechRecognitionStatus" -> reply(status(args))
            "speechRecognitionAvailability" -> availability(args, reply)
            "installSpeechRecognition" -> install(args, reply)
            else -> reply(error("NotSupportedError"))
        }
    }

    // ------------------------------------------------------------ the status (D7, D8, D11)

    private fun local(args: JSONObject) = args.optBoolean("processLocally", false)
    private fun onDevice() = Build.VERSION.SDK_INT >= 31 && SpeechRecognizer.isOnDeviceRecognitionAvailable(context.application)
    private fun service(args: JSONObject) = args.optJSONObject("android")?.optString("recognitionService")?.ifEmpty { null }?.let(ComponentName::unflattenFromString)

    private fun status(args: JSONObject): JSONObject {
        if (context.agent) {
            val out = JSONObject().put("available", true).put("state", "granted").put("inputs", JSONArray(listOf("track", "microphone", "file")))
                .put("local", true).put("apple", JSONObject.NULL).put("android", JSONObject.NULL)
            for (key in overlay.keys()) out.put(key, overlay.get(key))
            return out
        }
        val device = onDevice()
        val available = if (local(args)) device else SpeechRecognizer.isRecognitionAvailable(context.application)
        return JSONObject().put("available", available).put("state", context.permission(Manifest.permission.RECORD_AUDIO).state)
            .put("inputs", JSONArray(if (available) listOf("microphone") else emptyList()))
            .put("local", local(args) && device).put("apple", JSONObject.NULL)
            .put("android", JSONObject().put("service", service(args)?.flattenToString() ?: JSONObject.NULL).put("onDeviceAvailable", device))
    }

    /** `checkRecognitionSupport` (API 33) for [args]' recognizer, answered on main: null where it cannot say. */
    @TargetApi(33)
    private fun support(args: JSONObject, answer: (RecognitionSupport?) -> Unit) {
        if (Build.VERSION.SDK_INT < 33) return answer(null)
        val recognizer = recognizer(args) ?: return answer(null)
        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
        val done = { s: RecognitionSupport? -> main.post { recognizer.destroy(); answer(s) }; Unit }
        recognizer.checkRecognitionSupport(intent, Executors.newSingleThreadExecutor(), object : RecognitionSupportCallback {
            override fun onSupportResult(recognitionSupport: RecognitionSupport) = done(recognitionSupport)
            override fun onError(error: Int) = done(null)
        })
    }

    private fun langs(args: JSONObject): List<String> = args.optJSONArray("langs")?.let { a -> (0 until a.length()).map { a.getString(it) } }.orEmpty()
    private fun has(list: List<String>, lang: String) = list.any { it.replace('_', '-').equals(lang, ignoreCase = true) }

    @TargetApi(33)
    private fun availability(args: JSONObject, reply: (JSONObject) -> Unit) {
        val s = status(args)
        val available = s.optBoolean("available") && (s.optBoolean("local") || !local(args))
        if (context.agent || Build.VERSION.SDK_INT < 33) return reply(JSONObject().put("availability", if (available) "available" else "unavailable"))
        if (!s.optBoolean("available")) return reply(JSONObject().put("availability", "unavailable"))
        support(args) { support ->
            if (support == null) return@support reply(JSONObject().put("availability", if (available) "available" else "unavailable"))
            var word = "available"
            for (lang in langs(args)) {
                val each = when {
                    has(support.installedOnDeviceLanguages, lang) -> "available"
                    !local(args) && has(support.onlineLanguages, lang) -> "available"
                    has(support.pendingOnDeviceLanguages, lang) -> "downloading"
                    has(support.supportedOnDeviceLanguages, lang) -> "downloadable"
                    else -> "unavailable"
                }
                word = listOf("available", "downloading", "downloadable", "unavailable").let { order -> if (order.indexOf(each) > order.indexOf(word)) each else word }
            }
            reply(JSONObject().put("availability", word))
        }
    }

    /** The on-device model for each language, downloaded by the system (`triggerModelDownload`, API 33): installed
     * answers whether every one is installed now; a download the system began completes later. */
    @TargetApi(33)
    private fun install(args: JSONObject, reply: (JSONObject) -> Unit) {
        if (context.agent) return reply(JSONObject().put("installed", status(args).optBoolean("local")))
        if (Build.VERSION.SDK_INT < 33 || !onDevice()) return reply(JSONObject().put("installed", false))
        val local = JSONObject(args.toString()).put("processLocally", true)
        support(local) { support ->
            val missing = langs(args).filter { support == null || !has(support.installedOnDeviceLanguages, it) }
            if (missing.isNotEmpty()) recognizer(local)?.let { recognizer ->
                for (lang in missing) recognizer.triggerModelDownload(Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).putExtra(RecognizerIntent.EXTRA_LANGUAGE, lang))
                recognizer.destroy()
            }
            context.changed("speech-recognition.speechRecognitionAvailability")
            reply(JSONObject().put("installed", support != null && missing.isEmpty()))
        }
    }

    /** The recognizer [args] select (D9): the on-device one for `processLocally`, else the named or default service. */
    internal fun recognizer(args: JSONObject): SpeechRecognizer? = when {
        local(args) -> if (Build.VERSION.SDK_INT >= 31 && SpeechRecognizer.isOnDeviceRecognitionAvailable(context.application))
            SpeechRecognizer.createOnDeviceSpeechRecognizer(context.application) else null
        else -> service(args)?.let { SpeechRecognizer.createSpeechRecognizer(context.application, it) } ?: SpeechRecognizer.createSpeechRecognizer(context.application)
    }

    // ------------------------------------------------------------ starting

    private fun start(args: JSONObject, reply: (JSONObject) -> Unit) {
        val given = listOfNotNull(
            "track".takeIf { args.optString("track").isNotEmpty() },
            "microphone".takeIf { args.optBoolean("microphone", false) },
            "file".takeIf { args.optString("path").isNotEmpty() },
        )
        if (given.size != 1) return reply(error("TypeError"))
        val input = given[0]
        val s = status(args)
        val local = local(args)
        // `processLocally` where the recognizer cannot keep audio on the device (D9) opens nothing.
        val refused = local && !s.optBoolean("local")
        if (!refused) {
            val inputs = s.getJSONArray("inputs").let { a -> (0 until a.length()).map { a.getString(it) } }
            if (!s.optBoolean("available") || input !in inputs) return reply(error("NotSupportedError"))
            if (input == "track" && context.tracks.lookup(args.getString("track"))?.kind != "audio") return reply(error("InvalidStateError"))
            if (!context.agent) {
                when (s.getString("state")) {
                    "denied" -> return reply(error("NotAllowedError"))
                    "prompt" -> return context.askPermission(Manifest.permission.RECORD_AUDIO) {
                        if (context.permission(Manifest.permission.RECORD_AUDIO).state == "granted") start(args, reply) else reply(error("NotAllowedError"))
                    }
                }
            }
        }
        next += 1
        val leaves = if (context.agent) !local && !s.optBoolean("local") else !local
        val r = Recognition(this, "recognition-${context.session}-$next", input, args, leaves)
        // The service opens the microphone itself, so the session's captures stand aside while it listens (D8).
        if (input == "microphone" && !refused) {
            try {
                r.hold = context.tracks.hold("audio")
            } catch (e: TrackError) {
                return reply(error(e.code))
            }
        }
        args.optString("transcript").takeIf { it.isNotEmpty() }?.let { path -> context.file(path)?.let { r.transcript = path to it } }
        r.prepareTranscript()
        recognitions[r.id] = r
        say("${r.id}: ${if (input == "track") "on ${args.getString("track")}" else if (input == "file") "of ${args.getString("path")}" else "on the microphone"}" +
            "${r.transcript?.let { ", transcript ${it.first}" } ?: ""}, ${if (leaves) "may leave the device" else "on the device"}")
        if (refused) { r.finish("service-not-allowed"); return reply(r.info()) }
        if (input == "track") {
            try {
                r.subscription = context.tracks.subscribe(args.getString("track"), r, true, "app")
            } catch (e: TrackError) {
                recognitions.remove(r.id)
                return reply(error(e.code))
            }
            if (r.state == "ended") return reply(error("InvalidStateError"))
        }
        if (context.agent) {
            r.state = "listening"; r.audio = input != "file"; r.sound = r.audio
            if (input == "file") {
                heard?.let { r.final(listOf(it to 0.9), 0.0, WORD_MS * it.split(Regex("\\s+")).size) }
                r.finish(null)
            }
            announce()
            return reply(r.info())
        }
        r.state = "listening"
        r.listen()
        announce()
        reply(r.info())
    }

    /** The intent for one session of [r] (D9's extras), given to the hatch. */
    internal fun intent(r: Recognition): Intent {
        val args = r.args
        val android = args.optJSONObject("android") ?: JSONObject()
        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
            .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, if (android.optString("languageModel") == "webSearch") RecognizerIntent.LANGUAGE_MODEL_WEB_SEARCH else RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            .putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, r.interimResults)
            .putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, r.maxAlternatives)
            .putExtra(RecognizerIntent.EXTRA_CALLING_PACKAGE, context.application.packageName)
        args.optString("lang").takeIf { it.isNotEmpty() }?.let { intent.putExtra(RecognizerIntent.EXTRA_LANGUAGE, it) }
        if (android.has("preferOffline")) intent.putExtra(RecognizerIntent.EXTRA_PREFER_OFFLINE, android.optBoolean("preferOffline"))
        if (android.has("completeSilenceLengthMillis")) intent.putExtra(RecognizerIntent.EXTRA_SPEECH_INPUT_COMPLETE_SILENCE_LENGTH_MILLIS, android.optLong("completeSilenceLengthMillis"))
        if (android.has("minimumLengthMillis")) intent.putExtra(RecognizerIntent.EXTRA_SPEECH_INPUT_MINIMUM_LENGTH_MILLIS, android.optLong("minimumLengthMillis"))
        if (Build.VERSION.SDK_INT >= 33) {
            args.optJSONArray("phrases")?.let { list ->
                val words = ArrayList((0 until list.length()).mapNotNull { list.optJSONObject(it)?.optString("phrase")?.ifEmpty { null } })
                if (words.isNotEmpty()) intent.putStringArrayListExtra(RecognizerIntent.EXTRA_BIASING_STRINGS, words)
            }
            if (args.has("unspokenPunctuation")) intent.putExtra(RecognizerIntent.EXTRA_ENABLE_FORMATTING,
                if (args.optBoolean("unspokenPunctuation")) RecognizerIntent.FORMATTING_OPTIMIZE_QUALITY else RecognizerIntent.FORMATTING_OPTIMIZE_LATENCY)
            if (android.has("maskOffensiveWords")) intent.putExtra(RecognizerIntent.EXTRA_MASK_OFFENSIVE_WORDS, android.optBoolean("maskOffensiveWords"))
        }
        hatch?.invoke(intent)
        return intent
    }

    // ------------------------------------------------------------ the agent's hearing (§9)

    /** [r] moved to the session clock's now: the words of each phrase heard since it began. */
    internal fun advance(r: Recognition) {
        if (!context.agent || r.state != "listening" || r.input == "file") return
        val t = context.now()
        while (r.heard < phrases.size) {
            val p = phrases[r.heard]
            if (p.at < r.began) { r.heard += 1; continue }
            val from = maxOf(p.at, r.cursor)
            val last = from + WORD_MS * p.words.size
            val done = last + FINAL_MS
            if (done <= t) {
                r.final(listOf(p.text to 0.9), from, last)
                r.cursor = done
                r.heard += 1
                if (!r.continuous) { r.finish(null); return }
                continue
            }
            val count = minOf(p.words.size, ((t - from) / WORD_MS).toInt())
            r.speech = count > 0
            r.interim = if (count > 0 && r.interimResults) r.result(r.finalCount, listOf(p.words.take(count).joinToString(" ") to 0.9), from, from + WORD_MS * count) else null
            return
        }
        r.speech = false
        if (!r.continuous && r.finalCount == 0 && t - r.began >= NO_SPEECH_MS) r.finish("no-speech")
    }

    // ------------------------------------------------------------ the session

    /** Under the agent: the preferred status (laid over the default, as the runner answers it) and the phrase heard. */
    override fun preferred(values: JSONObject) {
        overlay = values.optJSONObject("speechRecognitionStatus")?.takeIf { !it.has("error") } ?: JSONObject()
        val phrase = values.optString("heard").ifEmpty { null }.takeIf { values.has("heard") && !values.isNull("heard") }
        if (phrase == heard) return
        heard = phrase
        recognitions.values.forEach(::advance)
        if (!phrase.isNullOrEmpty()) {
            phrases += Phrase(phrase, phrase.split(Regex("\\s+")).filter { it.isNotEmpty() }, context.now())
            say("heard: $phrase")
        }
        announce()
    }

    /** Recognitions whose reply was dropped: aborted, never the app's (D7). */
    override fun dispose(ids: List<String>) {
        for (id in ids) {
            val r = recognitions[id] ?: continue
            if (r.state == "ended") continue
            say("$id: its reply was dropped; aborted")
            r.abort("aborted") {}
        }
    }

    /** Recognition is a foreground activity: leaving the foreground aborts every one (D10). */
    override fun foreground(shown: Boolean) {
        if (shown) return
        for (r in recognitions.values.toList()) {
            advance(r)
            if (r.state == "ended") continue
            say("${r.id}: left the foreground; aborted")
            r.abort("aborted") {}
        }
    }

    override fun destroy(done: () -> Unit) {
        val live = recognitions.values.filter { it.state != "ended" }
        if (live.isEmpty()) return done()
        var left = live.size
        live.forEach { r -> r.abort("aborted") { if (--left == 0) done() } }
    }
}

/** One recognition: a consumer of its track, its results, its transcript directory and, off the agent, its
 * recognizer. Main thread. */
internal class Recognition(private val module: SpeechRecognitionModule, val id: String, val input: String, val args: JSONObject, val leavesDevice: Boolean) :
    TrackConsumer, RecognitionListener {
    val continuous = args.optBoolean("continuous", false)
    val interimResults = args.optBoolean("interimResults", false)
    val history = args.optInt("history", SpeechRecognitionModule.HISTORY).coerceIn(1, SpeechRecognitionModule.MAX_HISTORY)
    val maxAlternatives = args.optInt("maxAlternatives", 1).coerceIn(1, 10)
    var state = "starting"
    var audio = false
    var sound = false
    var speech = false
    private val results = ArrayList<JSONObject>()
    var finalCount = 0
    var interim: JSONObject? = null
    private var error: String? = null
    private var message: String? = null
    var transcript: Pair<String, File>? = null
    private var files = 0
    private var fileSize = 0
    var subscription: (() -> Unit)? = null
    /** The exclusive hold of a `microphone=true` recognition, released at its end. */
    var hold: (() -> Unit)? = null
    // The agent's clock (§9).
    val began = module.context.now()
    var cursor = began
    var heard = 0
    private val writer = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())
    private val waiting = ArrayList<(JSONObject) -> Unit>()
    private var stopping = false
    private var recognizer: SpeechRecognizer? = null

    fun info(): JSONObject = JSONObject().put("id", id).put("leavesDevice", leavesDevice).put("finalCount", finalCount)
        .put("transcript", transcript?.first ?: JSONObject.NULL).put("files", files)
        .put("error", if (state == "ended") error ?: JSONObject.NULL else JSONObject.NULL)

    fun shape(): JSONObject {
        val window = ArrayList(results.takeLast(history))
        while (window.size > 1 && JSONArray(window).toString().length > SpeechRecognitionModule.WINDOW_BYTES) window.removeAt(0)
        return JSONObject().put("id", id).put("state", state).put("audio", audio).put("sound", sound).put("speech", speech)
            .put("results", JSONArray(window)).put("finalCount", finalCount).put("interim", interim ?: JSONObject.NULL)
            .put("leavesDevice", leavesDevice).put("error", error ?: JSONObject.NULL).put("message", message ?: JSONObject.NULL)
    }

    fun result(index: Int, alternatives: List<Pair<String, Double>>, start: Double?, end: Double?): JSONObject = JSONObject().put("index", index)
        .put("alternatives", JSONArray(alternatives.take(maxAlternatives).map { JSONObject().put("transcript", it.first).put("confidence", it.second) }))
        .put("start", start ?: JSONObject.NULL).put("end", end ?: JSONObject.NULL).put("apple", JSONObject.NULL)

    /** A final result (best first): numbered, kept in the window, appended to the transcript; a text over 16 KB in
     * pieces split at a word boundary (D7). */
    fun final(alternatives: List<Pair<String, Double>>, start: Double?, end: Double?) {
        val pieces = pieces(alternatives.firstOrNull()?.first ?: "")
        for (piece in pieces) {
            val r = result(finalCount, if (pieces.size == 1) alternatives else listOf(piece to (alternatives.firstOrNull()?.second ?: 0.0)), start, end)
            finalCount += 1
            results += r
            if (results.size > SpeechRecognitionModule.MAX_HISTORY) results.removeAt(0)
            write(r)
        }
        interim = null
    }

    private fun pieces(text: String): List<String> {
        val out = ArrayList<String>()
        var rest = text
        while (rest.toByteArray().size > SpeechRecognitionModule.SPLIT) {
            var limit = minOf(rest.length, SpeechRecognitionModule.SPLIT)
            while (rest.substring(0, limit).toByteArray().size > SpeechRecognitionModule.SPLIT) limit -= 1
            val cut = rest.lastIndexOf(' ', limit).takeIf { it > 0 } ?: limit
            out += rest.substring(0, cut)
            rest = rest.substring(cut).trimStart(' ')
        }
        out += rest
        return out
    }

    // ------------------------------------------------------------ the transcript (D7, D12 item 7)

    /** The directory, emptied as the recognition starts, so a replacement leaves no stale file. */
    fun prepareTranscript() {
        val dir = transcript?.second ?: return
        writer.execute { dir.deleteRecursively(); dir.mkdirs() }
    }

    private fun write(result: JSONObject) {
        val dir = transcript?.second ?: return
        val line = (result.toString() + "\n").toByteArray()
        if (files == 0 || fileSize + line.size > SpeechRecognitionModule.FILE_BYTES) { files += 1; fileSize = 0 }
        fileSize += line.size
        val file = File(dir, String.format("%05d.jsonl", files - 1))
        writer.execute {
            try { file.appendBytes(line) } catch (e: Exception) { main.post { module.say("$id: ${file.name} not written: ${e.message}") } }
        }
    }

    // ------------------------------------------------------------ ending

    /** The recognition ends (with [why]): its input let go, its files finished; sticky (D7). Every stop that waited
     * is answered. */
    fun finish(why: String?, text: String? = null) {
        if (state == "ended") return
        state = "ended"; error = error ?: why; message = message ?: text
        audio = false; sound = false; speech = false
        subscription?.invoke(); subscription = null
        hold?.invoke(); hold = null
        recognizer?.destroy(); recognizer = null
        module.say("$id: ended${error?.let { " ($it)" } ?: ""}${message?.let { ": $it" } ?: ""}, $finalCount final results")
        module.announce()
        writer.execute { main.post { val i = info(); waiting.toList().forEach { it(i) }; waiting.clear() } }
        writer.shutdown()
    }

    /** Answered once the recognition has ended and its files are finished. */
    private fun whenDone(f: (JSONObject) -> Unit) {
        if (state == "ended") main.post { f(info()) } else waiting += f
    }

    /** What was heard so far is final; the recognizer finishes what it has, then the recognition ends. */
    fun stop(reply: (JSONObject) -> Unit) {
        whenDone(reply)
        if (state == "ended" || stopping) return
        stopping = true
        if (module.context.agent) {
            interim?.let { i ->
                val alts = i.getJSONArray("alternatives")
                final((0 until alts.length()).map { alts.getJSONObject(it).let { a -> a.optString("transcript") to a.optDouble("confidence", 0.0) } },
                    i.optDouble("start").takeIf { !it.isNaN() }, i.optDouble("end").takeIf { !it.isNaN() })
            }
            return finish(null)
        }
        state = "stopping"
        module.announce()
        recognizer?.stopListening() ?: finish(null)
    }

    fun abort(why: String, reply: (JSONObject) -> Unit) {
        whenDone(reply)
        if (state == "ended") return
        interim = null
        recognizer?.cancel()
        finish(why)
    }

    // ------------------------------------------------------------ the recognizer (D9)

    /** A session of the recognizer on the microphone: the first, or the next after a continuous one's end. */
    fun listen() {
        val made = recognizer ?: module.recognizer(args)?.also { it.setRecognitionListener(this); recognizer = it }
            ?: return finish("service-not-allowed", "no on-device recognizer")
        made.startListening(module.intent(this))
    }

    private fun alternatives(bundle: Bundle?): List<Pair<String, Double>> {
        val texts = bundle?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION).orEmpty()
        val scores = bundle?.getFloatArray(SpeechRecognizer.CONFIDENCE_SCORES)
        return texts.mapIndexed { i, t -> t to (scores?.getOrNull(i)?.toDouble()?.takeIf { it >= 0 } ?: 0.0) }
    }

    /** A continuous recognition's session ended: the next begins, the seam journaled (D9). */
    private fun again(why: String): Boolean {
        if (!continuous || stopping || state != "listening") return false
        module.say("$id: a new session ($why)")
        listen()
        return true
    }

    override fun onReadyForSpeech(params: Bundle?) { audio = true; module.announce() }
    override fun onBeginningOfSpeech() { sound = true; speech = true; module.announce() }
    override fun onRmsChanged(rmsdB: Float) {}
    override fun onBufferReceived(buffer: ByteArray?) {}
    override fun onEndOfSpeech() { speech = false; module.announce() }
    override fun onEvent(eventType: Int, params: Bundle?) {}

    override fun onPartialResults(partialResults: Bundle?) {
        if (state == "ended") return
        val alts = alternatives(partialResults).filter { it.first.isNotEmpty() }
        interim = if (interimResults && alts.isNotEmpty()) result(finalCount, alts, null, null) else null
        module.announce()
    }

    override fun onResults(results: Bundle?) {
        if (state == "ended") return
        val alts = alternatives(results).filter { it.first.isNotEmpty() }
        if (alts.isNotEmpty()) final(alts, null, null)
        speech = false
        module.announce()
        if (!again("the service ended its session")) finish(null)
    }

    override fun onError(error: Int) {
        if (state == "ended") return
        val quiet = error == SpeechRecognizer.ERROR_NO_MATCH || error == SpeechRecognizer.ERROR_SPEECH_TIMEOUT
        if (quiet && again("no speech in the last session")) return
        if (stopping && (quiet || error == SpeechRecognizer.ERROR_CLIENT)) return finish(null)
        val word = when (error) {
            SpeechRecognizer.ERROR_NO_MATCH, SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> if (finalCount > 0) null else "no-speech"
            SpeechRecognizer.ERROR_AUDIO -> "audio-capture"
            SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> "not-allowed"
            SpeechRecognizer.ERROR_CLIENT -> "aborted"
            SpeechRecognizer.ERROR_LANGUAGE_NOT_SUPPORTED, SpeechRecognizer.ERROR_LANGUAGE_UNAVAILABLE -> "language-not-supported"
            SpeechRecognizer.ERROR_NETWORK, SpeechRecognizer.ERROR_NETWORK_TIMEOUT, SpeechRecognizer.ERROR_SERVER,
            SpeechRecognizer.ERROR_SERVER_DISCONNECTED, SpeechRecognizer.ERROR_TOO_MANY_REQUESTS -> "network"
            else -> "audio-capture"
        }
        if (word != null && word != "aborted") module.say("$id: $word (SpeechRecognizer error $error)")
        finish(word, word?.let { "SpeechRecognizer error $error" })
    }

    // ------------------------------------------------------------ the track (D8, D10)

    override fun live(source: dev.exact.canvas.AudioSource?) { audio = true; sound = true; module.announce() }
    override fun muted(reason: String) {}
    override fun unmuted() {}
    override fun ended() {
        if (state == "ended") return
        interim = null
        recognizer?.cancel()
        finish(null)
    }
}
