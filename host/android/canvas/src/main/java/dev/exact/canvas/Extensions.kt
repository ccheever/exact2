package dev.exact.canvas

import android.app.Activity
import android.content.Context
import android.util.Log
import org.json.JSONObject

/**
 * A package's implementation of a module on Android (LLP 1133.001 D1, D5). The APK's build compiles each bound
 * implementation's Kotlin into the app and writes `exact.app.ExactExtensions`, an [ExtensionRoster] naming each
 * extension's class; the reader makes one per process at the extension's first call. Calls come from the runner as
 * requests (kind 11, [Requests]) and every one is on the main thread. An error is answered as `{"error": code}`.
 */
abstract class ExactExtension(protected val context: ExtensionContext) {
    /** A source's read or a send, by its declared name, with the keyword arguments the call wrote: answered once. */
    abstract fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit)

    /** The last reader of live source [name] is gone: stop what it started (LLP 1133.002 D5). */
    open fun release(name: String) {}

    /**
     * The agent's preferred values for this extension's names, by name, when it is bound under the agent and
     * whenever one changes (LLP 1133.008 D8 item 9): an implementation that is its own substitute answers by them.
     */
    open fun preferred(values: JSONObject) {}

    /** The activity is in front again: what changed in Settings meanwhile is read again (Android has no event). */
    open fun resumed() {}
}

/** The app's bound extensions by name, each with how to make it: what the build writes as `exact.app.ExactExtensions`. */
interface ExtensionRoster {
    val extensions: Map<String, (ExtensionContext) -> ExactExtension>
}

/** What an extension may use of the host. */
class ExtensionContext internal constructor(val application: Context) {
    /** The activity that shows the app now, if one does: a permission's rationale is read from it. */
    val activity: Activity? get() = Exact.reader?.get()

    /** The readers of [topic] are asked again (LLP 1016.002). */
    fun changed(topic: String) = Exact.changed(topic)

    fun log(line: String) { Log.i("exact", line) }

    /**
     * The platform's dialog for [permissions], in one request (`Activity.requestPermissions`), then [done], on the main
     * thread: when the result comes, or when the activity resumes without it (a result lost to recreation, LLP 1133.002
     * D9). Requests made while one is up share its answer. With no activity in front nothing is asked and [done] runs
     * at once, since the platform would present nothing.
     */
    fun requestPermissions(permissions: Array<String>, done: () -> Unit) = Extensions.ask(permissions, done)
}

/** The process's extensions: made at their first call, they outlive the activities that show the app. */
internal object Extensions {
    private const val CODE = 0x45584154 and 0xffff
    private val roster: Map<String, (ExtensionContext) -> ExactExtension> by lazy {
        val type = try { Class.forName("exact.app.ExactExtensions") } catch (e: ClassNotFoundException) { return@lazy emptyMap() }
        (type.getDeclaredConstructor().newInstance() as ExtensionRoster).extensions
    }
    private val made = HashMap<String, ExactExtension>()
    private var context: ExtensionContext? = null
    private var foreground = false
    private var asking = false
    private val waiting = ArrayList<() -> Unit>()

    /** The activity was made: the host binds the names of the extensions the APK carries (outside the agent). */
    fun bind(activity: Activity) {
        if (context == null) context = ExtensionContext(activity.applicationContext)
        if (roster.isNotEmpty()) Exact.bindExtensions(roster.keys.joinToString("\n"))
    }

    /**
     * Request [id]'s call, the runner's body: `{"extension", "source"|"send", "args"}`, `{"extension", "release"}` or
     * `{"extension", "preferred"}`.
     */
    fun call(id: Int, body: String) {
        var answered = false
        val reply = { answer: JSONObject -> if (!answered) { answered = true; Exact.answer(id, answer.toString()) } }
        try {
            val request = JSONObject(body)
            val name = request.getString("extension")
            val extension = made[name] ?: roster[name]?.let { make -> context?.let(make) }?.also { made[name] = it }
                ?: return reply(JSONObject().put("error", "NotSupportedError"))
            val released = request.optString("release")
            if (released.isNotEmpty()) {
                extension.release(released)
                return reply(JSONObject())
            }
            request.optJSONObject("preferred")?.let {
                extension.preferred(it)
                return reply(JSONObject())
            }
            val declared = request.optString("source").ifEmpty { request.optString("send") }
            extension.later(declared, request.optJSONObject("args") ?: JSONObject(), reply)
        } catch (e: Exception) {
            Log.e("exact", "extension call $id: $e")
            reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    fun ask(permissions: Array<String>, done: () -> Unit) {
        val activity = Exact.reader?.get()
        if (activity == null || !foreground) return done()
        waiting += done
        if (asking) return
        asking = true
        activity.requestPermissions(permissions, CODE)
    }

    private fun settle() {
        asking = false
        val ready = waiting.toList()
        waiting.clear()
        ready.forEach { it() }
    }

    /** [Activity.onRequestPermissionsResult]: the answer to the request up. */
    fun permissionsResult(code: Int) { if (code == CODE) settle() }

    /** [Activity.onResume]: a result that never came (the activity was made again) is read as the state now. */
    fun resumed() {
        foreground = true
        if (asking) settle()
        made.values.forEach { it.resumed() }
    }

    fun paused() { foreground = false }
}
