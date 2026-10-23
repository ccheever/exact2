package dev.exact.canvas

import android.app.Activity
import android.content.Context
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject

/**
 * A package's implementation of a module on Android (LLP 1133.001 D1, D5). The APK's build compiles each bound
 * implementation's Kotlin into the app and writes `exact.app.ExactExtensions`, an [ExtensionRoster] naming each
 * extension's class; the reader makes one per session at the extension's first call. Calls come from the runner as
 * requests (kind 11, [Requests]) and every one is on the main thread. An error is answered as `{"error": code}`.
 */
abstract class ExactExtension(val context: ExtensionContext) {
    /** A source's read or a send, by its declared name, with the keyword arguments the call wrote: answered once. */
    abstract fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit)

    /**
     * [later] with the agent's preferred value for [name], under the agent (LLP 1133.009 D10 item 11): the runner also
     * lays it over the answer. An implementation that is its own substitute overrides this one.
     */
    open fun later(name: String, args: JSONObject, preferred: Any?, reply: (JSONObject) -> Unit) = later(name, args, reply)

    /** The last reader of live source [name] is gone: stop what it started (LLP 1133.002 D5). */
    open fun release(name: String) {}

    /**
     * No reader of live source [name] reads it with [args] any longer (LLP 1133.009 D10 item 12); a reader asked again
     * has its new arguments first.
     */
    open fun release(name: String, args: JSONObject) {}

    /**
     * The agent's preferred values for this extension's names, by name, when it is bound under the agent and
     * whenever one changes (LLP 1133.008 D8 item 9): an implementation that is its own substitute answers by them.
     */
    open fun preferred(values: JSONObject) {}

    /** Handles a reply carried that the runner dropped, never the app's (LLP 1133.009 D10 item 10). */
    open fun dispose(ids: List<String>) {}

    /** The app went to the foreground or left it ([ExtensionContext.foreground], LLP 1133.009 D10 item 3). */
    open fun foreground(shown: Boolean) {}

    /** The activity is in front again: what changed in Settings meanwhile is read again (Android has no event). */
    open fun resumed() {}

    /**
     * The session ends: finish what must be finished, then [done] (LLP 1133.009 D10 item 1). Android's session lives
     * as long as its process, which ends without calling this.
     */
    open fun destroy(done: () -> Unit) = done()
}

/** The app's bound extensions by name, each with how to make it: what the build writes as `exact.app.ExactExtensions`. */
interface ExtensionRoster {
    val extensions: Map<String, (ExtensionContext) -> ExactExtension>
}

/** What an extension may use of the host. */
class ExtensionContext internal constructor(val application: Context, name: String) {
    /** The activity that shows the app now, if one does: a permission's rationale is read from it. */
    val activity: Activity? get() = Exact.reader?.get()

    /**
     * Under the agent, unless the drive bound this extension as in production (`--bind`): an implementation that is its
     * own substitute then opens no device and asks the OS for nothing (LLP 1133.006 D8).
     */
    val agent: Boolean = try { Native.underAgent(name) } catch (e: UnsatisfiedLinkError) { false }

    /** The session this instance belongs to. Android runs one session a process. */
    val session: String = "1"

    /** Whether the app is in the foreground, as the runner's admission reads it ([ExactExtension.foreground]). */
    var foreground = true
        internal set

    /** The session's clock, ms: the agent's under the agent (LLP 1133.009 D10 item 2). */
    fun now(): Double = Native.now()

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
    private var application: Context? = null
    private var foreground = false
    private var asking = false
    private val waiting = ArrayList<() -> Unit>()

    /** The activity was made: the host binds the names of the extensions the APK carries. */
    fun bind(activity: Activity) {
        if (application == null) application = activity.applicationContext
        if (roster.isNotEmpty()) Exact.bindExtensions(roster.keys.joinToString("\n"))
    }

    /**
     * Request [id]'s call, the runner's body: `{"extension", "source"|"send", "args", "preferred"?}`, `{"extension",
     * "release", "args"?}`, `{"extension", "preferred"}`, `{"extension", "dispose"}` or `{"extension", "foreground"}`.
     */
    fun call(id: Int, body: String) {
        var answered = false
        val reply = { answer: JSONObject -> if (!answered) { answered = true; Exact.answer(id, answer.toString()) } }
        try {
            val request = JSONObject(body)
            val name = request.getString("extension")
            val extension = made[name]
                ?: roster[name]?.let { make -> application?.let { make(ExtensionContext(it, name)) } }?.also { made[name] = it }
                ?: return reply(JSONObject().put("error", "NotSupportedError"))
            val declared = request.optString("source").ifEmpty { request.optString("send") }
            val released = request.optString("release")
            when {
                released.isNotEmpty() -> {
                    val args = request.optJSONObject("args")
                    if (args != null) extension.release(released, args) else extension.release(released)
                    reply(JSONObject())
                }
                declared.isNotEmpty() ->
                    extension.later(declared, request.optJSONObject("args") ?: JSONObject(), request.opt("preferred"), reply)
                request.has("preferred") -> {
                    extension.preferred(request.optJSONObject("preferred") ?: JSONObject())
                    reply(JSONObject())
                }
                request.has("dispose") -> {
                    val ids = request.optJSONArray("dispose") ?: JSONArray()
                    extension.dispose((0 until ids.length()).map { ids.getString(it) })
                    reply(JSONObject())
                }
                request.has("foreground") -> {
                    val shown = request.getBoolean("foreground")
                    extension.context.foreground = shown
                    extension.foreground(shown)
                    reply(JSONObject())
                }
                else -> reply(JSONObject().put("error", "NotSupportedError"))
            }
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
