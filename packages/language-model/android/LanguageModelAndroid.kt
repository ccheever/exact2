// ML Kit's GenAI Prompt API for @exact/language-model (LLP 1133.016 D5–D8, D10, D11): `checkStatus()` and
// `download()`, a request per generation built from the module's transcript, `generateContentStream` appended into the
// response, the answer to a constraint validated (D8), and GenAiException's codes as D2's errors.
//
// ML Kit has no session and no roles on a request's contents, so the turns before the input go into its text as a
// transcript, the system message as its `SystemInstruction` where the model takes one. Its suspend functions run on
// ML Kit's own executors; this class resumes on the main thread, where every entry into the module is (D6).
package exact.languagemodel

import android.os.Handler
import android.os.Looper
import com.google.mlkit.genai.common.DownloadStatus
import com.google.mlkit.genai.common.FeatureStatus
import com.google.mlkit.genai.common.GenAiException
import com.google.mlkit.genai.prompt.GenerateContentRequest
import com.google.mlkit.genai.prompt.Generation
import com.google.mlkit.genai.prompt.GenerationConfig
import com.google.mlkit.genai.prompt.GenerativeModel
import com.google.mlkit.genai.prompt.ModelConfig
import com.google.mlkit.genai.prompt.ModelPreference
import com.google.mlkit.genai.prompt.ModelReleaseStage
import com.google.mlkit.genai.prompt.SystemInstruction
import com.google.mlkit.genai.prompt.TextPart
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.util.concurrent.Executor

class MlKit(private val m: LanguageModelModule) {
    private val main = Handler(Looper.getMainLooper())
    private val scope = CoroutineScope(SupervisorJob() + Executor { main.post(it) }.asCoroutineDispatcher())
    /** A client per model configuration a create names. */
    private val clients = HashMap<String, GenerativeModel>()
    private var systemPrompts: Boolean? = null
    // The status last read, and a download this module started (D5).
    private var last: String? = null
    private var download: Job? = null
    private var bytesTotal: Long? = null
    private var bytesDownloaded: Long? = null
    private val waiting = mutableListOf<(Boolean) -> Unit>()
    /** One generation a process (D6): ML Kit answers `BUSY` to an app that asks for more. */
    private var generating: ModelResponse? = null
    private val pending = ArrayDeque<ModelResponse>()

    private fun client(options: JSONObject = JSONObject()): GenerativeModel {
        val preference = when (options.optString("modelPreference")) { "fast" -> ModelPreference.FAST; "full" -> ModelPreference.FULL; else -> null }
        val stage = when (options.optString("releaseStage")) { "stable" -> ModelReleaseStage.STABLE; "preview" -> ModelReleaseStage.PREVIEW; else -> null }
        return clients.getOrPut("$preference/$stage") {
            val model = ModelConfig.builder()
            preference?.let { model.preference = it }
            stage?.let { model.releaseStage = it }
            LanguageModelModule.modelConfig?.invoke(model)
            val config = GenerationConfig.Builder()
            config.modelConfig = model.build()
            Generation.getClient(config.build())
        }
    }

    // ---------------------------------------------------------------- availability (D5)

    fun availability(args: JSONObject, reply: (JSONObject) -> Unit) {
        if (m.wantsTools(args)) return reply(base().put("availability", "unavailable").put("reason", "unsupported-options").put("android", JSONObject.NULL))
        scope.launch { reply(read(client())) }
    }

    private fun base() = JSONObject().put("leavesDevice", false).put("structuredOutput", "validated").put("tools", false)

    /** `checkStatus()` as the interface's availability, with the window and model name once it is available. */
    private suspend fun read(model: GenerativeModel): JSONObject {
        val out = base()
        val status = try { model.checkStatus() } catch (e: GenAiException) {
            m.say("checkStatus: ${e.errorCode} ${e.message}")
            return out.put("availability", "unavailable").put("reason", "device-not-eligible").put("android", JSONObject.NULL)
        }
        val android = JSONObject().put("featureStatus", status)
        when (status) {
            FeatureStatus.AVAILABLE -> {
                out.put("availability", "available")
                try { out.put("contextWindow", model.getTokenLimit()) } catch (e: GenAiException) { m.say("getTokenLimit: ${e.errorCode}") }
                try { android.put("baseModelName", model.getBaseModelName()) } catch (e: GenAiException) { m.say("getBaseModelName: ${e.errorCode}") }
            }
            FeatureStatus.DOWNLOADABLE -> out.put("availability", if (download != null) "downloading" else "downloadable")
            FeatureStatus.DOWNLOADING -> out.put("availability", "downloading")
            else -> out.put("availability", "unavailable").put("reason", "device-not-eligible")
        }
        if (out.optString("availability") == "downloading") {
            val total = bytesTotal
            val done = bytesDownloaded
            if (total != null && total > 0 && done != null) out.put("progress", done.toDouble() / total)
            done?.let { android.put("bytesDownloaded", it) }
        }
        val word = "${out.optString("availability")}/${out.optString("reason")}"
        if (word != last) { m.say("availability: $word"); last = word }
        return out.put("android", android)
    }

    /** A create: at once while available, after a download this call may start while downloadable (D5, D10). */
    fun create(args: JSONObject, origin: String, made: (Int) -> Unit, reply: (JSONObject) -> Unit) {
        val model = client(args.optJSONObject("android") ?: JSONObject())
        scope.launch {
            val a = read(model)
            when (a.optString("availability")) {
                "available" -> made(a.optInt("contextWindow", LanguageModelModule.WINDOW))
                "unavailable" -> reply(JSONObject().put("error", "NotSupportedError"))
                else -> {
                    if (download == null) {
                        if (!m.startsDownload(origin)) return@launch reply(JSONObject().put("error", "NotAllowedError"))
                        startDownload(model)
                    }
                    waiting += { ok ->
                        if (!ok) reply(JSONObject().put("error", "NotSupportedError"))
                        else scope.launch { made(read(model).optInt("contextWindow", LanguageModelModule.WINDOW)) }
                    }
                }
            }
        }
    }

    private fun startDownload(model: GenerativeModel) {
        m.say("download started")
        download = scope.launch {
            var ok = false
            try {
                model.download().collect { s ->
                    when (s) {
                        is DownloadStatus.DownloadStarted -> { bytesTotal = s.bytesToDownload; bytesDownloaded = 0 }
                        is DownloadStatus.DownloadProgress -> bytesDownloaded = s.totalBytesDownloaded
                        is DownloadStatus.DownloadCompleted -> ok = true
                        is DownloadStatus.DownloadFailed -> m.say("download failed: ${s.e.errorCode} ${s.e.message}")
                    }
                    m.announce(LanguageModelModule.AVAILABILITY)
                }
            } catch (e: GenAiException) {
                m.say("download failed: ${e.errorCode} ${e.message}")
            }
            download = null
            bytesTotal = null
            bytesDownloaded = null
            if (ok) m.say("download finished")
            m.announce(LanguageModelModule.AVAILABILITY)
            val w = waiting.toList()
            waiting.clear()
            w.forEach { it(ok) }
        }
    }

    // ---------------------------------------------------------------- generation

    /** `r`'s session is free: it generates once no other response of the process does. */
    fun begin(r: ModelResponse) {
        if (generating != null) { pending.addLast(r); return }
        generating = r
        r.job = scope.launch { generate(r) }
    }

    /** `r` ended: the process's next waiting response begins. */
    fun ended(r: ModelResponse) {
        pending.remove(r)
        if (generating !== r) return
        generating = null
        while (generating == null && pending.isNotEmpty()) { val next = pending.removeFirst(); if (next.live) begin(next) }
    }

    private suspend fun generate(r: ModelResponse) {
        val model = client(r.session.options)
        try {
            val measured = try { model.countTokens(GenerateContentRequest.Builder(TextPart(asked(r))).build()).totalTokens } catch (e: GenAiException) { null }
            if (!m.admitInput(r, measured)) return
            var complaint: String? = null
            for (attempt in 0..1) {
                r.text = ""
                model.generateContentStream(request(model, r, complaint)).collect { chunk ->
                    val c = chunk.candidates.firstOrNull() ?: return@collect
                    r.text += c.text
                    c.finishReason?.let { r.finishReason = it }
                }
                val schema = r.schema ?: break
                // D8: the answer is checked, and asked for once more with the complaint on a mismatch.
                val json = unfenced(r.text)
                val why = LanguageModelSchema.parse(json).let { if (it == null) "the answer is not JSON" else LanguageModelSchema.violation(schema, it) }
                if (why == null) { r.text = json; break }
                m.say("${r.id}: the answer does not satisfy the constraint: $why")
                if (attempt == 1) return m.end(r, "failed", "SyntaxError", message = why)
                complaint = why
            }
            m.end(r, "complete")
        } catch (e: CancellationException) {
            throw e
        } catch (e: GenAiException) {
            val (code, reason) = errorOf(e.errorCode)
            m.end(r, "failed", code, reason, e.message)
        } catch (e: Exception) {
            m.end(r, "failed", "OperationError", message = e.message)
        }
    }

    /** The prompt's text: the input, and the schema it must answer unless the app omits it (D8). */
    private fun asked(r: ModelResponse): String {
        val schema = r.schema?.takeIf { !r.omit } ?: return r.turn.input ?: ""
        return "${r.turn.input ?: ""}\n\nAnswer with JSON alone that satisfies this JSON Schema:\n${LanguageModelSchema.json(schema)}"
    }

    private suspend fun request(model: GenerativeModel, r: ModelResponse, complaint: String?): GenerateContentRequest {
        val s = r.session
        val text = StringBuilder()
        val system = s.system
        val instructs = system != null && (systemPrompts ?: try { model.isSystemPromptAvailable() } catch (e: GenAiException) { false }.also { systemPrompts = it })
        if (system != null && !instructs) text.append(system).append("\n\n")
        val before = s.turns.filter { it !== r.turn }
        if (before.isNotEmpty()) {
            text.append("The conversation so far:\n")
            for (t in before) {
                t.input?.let { text.append("User: ").append(it).append('\n') }
                t.reply?.let { text.append("Model: ").append(it).append('\n') }
            }
            text.append("\nUser: ")
        }
        text.append(asked(r))
        complaint?.let { text.append("\n\nThe last answer was not valid: ").append(it).append(". Answer again with JSON alone.") }
        val builder = if (instructs) GenerateContentRequest.Builder(SystemInstruction(system), TextPart(text.toString()))
            else GenerateContentRequest.Builder(TextPart(text.toString()))
        // D11's words, then the app's hatch (D12), which sets what the words do not.
        LanguageModelSchema.number(r.options.opt("temperature"))?.let { builder.temperature = it.toFloat() }
        LanguageModelSchema.number(r.options.opt("topK"))?.let { builder.topK = it.toInt() }
        LanguageModelSchema.number(r.options.opt("seed"))?.let { builder.seed = it.toInt() }
        LanguageModelSchema.number(r.options.opt("maxOutputTokens"))?.let { builder.maxOutputTokens = it.toInt() }
        LanguageModelModule.request?.invoke(builder)
        return builder.build()
    }

    /** The JSON in a reply the model fenced as a Markdown code block. */
    private fun unfenced(text: String): String =
        Regex("^\\s*```(?:json)?\\s*([\\s\\S]*?)\\s*```\\s*$").find(text)?.groupValues?.get(1) ?: text.trim()

    /** D2's name and reason for a GenAiException's code. */
    private fun errorOf(code: Int): Pair<String, String?> = when (code) {
        GenAiException.ErrorCode.BACKGROUND_USE_BLOCKED -> "NotAllowedError" to "background"
        GenAiException.ErrorCode.BUSY -> "OperationError" to "rate-limited"
        GenAiException.ErrorCode.PER_APP_BATTERY_USE_QUOTA_EXCEEDED -> "OperationError" to "quota"
        GenAiException.ErrorCode.REQUEST_TOO_LARGE -> "QuotaExceededError" to null
        GenAiException.ErrorCode.NOT_AVAILABLE, GenAiException.ErrorCode.NEEDS_SYSTEM_UPDATE,
        GenAiException.ErrorCode.AICORE_INCOMPATIBLE -> "OperationError" to "model-not-ready"
        GenAiException.ErrorCode.NOT_ENOUGH_DISK_SPACE -> "OperationError" to "disk"
        GenAiException.ErrorCode.NOT_SUPPORTED -> "NotSupportedError" to null
        GenAiException.ErrorCode.CANCELLED -> "AbortError" to null
        else -> "OperationError" to null
    }

    /** `countTokens` of `input` alone, or null where ML Kit cannot count. */
    fun measure(s: ModelSession, input: String, reply: (Int?) -> Unit) {
        scope.launch {
            reply(try { client(s.options).countTokens(GenerateContentRequest.Builder(TextPart(input)).build()).totalTokens } catch (e: GenAiException) { null })
        }
    }

    fun prewarm(s: ModelSession) {
        scope.launch { try { client(s.options).warmup() } catch (e: GenAiException) { m.say("warmup: ${e.errorCode}") } }
    }

    fun close() {
        scope.cancel()
        clients.values.forEach { it.close() }
        clients.clear()
    }
}
