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

    /**
     * Make view [tag], one of the package's `views`, for node [id] of [session] (LLP 1133.008 D8 item 1): the one
     * extension of the process makes every session's views, so it keeps their state by session, and [id] is what its
     * reports name. [emit] sends an event (`message`, `load`, …) with its text. A report can come before the view is
     * made, since the reader attaches views after first pixel.
     */
    open fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): android.view.View =
        throw IllegalArgumentException("${javaClass.name} serves no view $tag")

    /** Every prop of a view [view] made, replaced. */
    open fun update(view: android.view.View, props: JSONObject) {}

    /** The view's node is gone. */
    open fun destroyView(view: android.view.View) {}

    /**
     * View [id] of [session] was displayed or hidden, by the origin of the commit that did it (`app`, `tool` or
     * `wake`; LLP 1133.008 D8 item 2).
     */
    open fun displayed(session: String, id: String, shown: Boolean, origin: String) {}

    /** Prop [prop] of view [id], which binds a device, changed, by the origin of the commit that changed it. */
    open fun bound(session: String, id: String, prop: String, origin: String) {}

    /**
     * A text field whose `autocomplete` ends in `webauthn` was made ([present]) or went (LLP 1133.011 D10 item 4):
     * the real view, on which a conditional credential request is set. Every live one is told to an extension when it
     * is made.
     */
    open fun webauthnField(view: android.widget.EditText, present: Boolean) {}
}

/**
 * The authenticator entry (LLP 1133.010 D13): what an implementation whose interface declares `authenticator` serves,
 * called by the reader's presence backend ([Presence]) in this process on the main thread. The prompt has one owner,
 * the implementation; the key and its bytes stay the reader's.
 */
interface ExactAuthenticator {
    /**
     * Show the platform's prompt over the resumed activity for [authenticators] (`BiometricManager.Authenticators`),
     * with [reason] already translated, [crypto] its `CryptoObject` (null for a check alone); [origin] is who made the
     * call. [done] once, on the main thread: the authenticated object, or a failure `{"error": code, "message"}`.
     */
    fun authenticate(crypto: android.hardware.biometrics.BiometricPrompt.CryptoObject?, authenticators: Int, reason: String, origin: String,
                     done: (android.hardware.biometrics.BiometricPrompt.CryptoObject?, JSONObject?) -> Unit)

    /** A presence name's key builder, before generation: the key hatch (D11). What the backend owns it sets again after. */
    fun configure(builder: android.security.keystore.KeyGenParameterSpec.Builder) {}
}

/** The app's bound extensions by name, each with how to make it: what the build writes as `exact.app.ExactExtensions`. */
interface ExtensionRoster {
    val extensions: Map<String, (ExtensionContext) -> ExactExtension>
    /** Each view tag a package serves, with the extension that makes it (LLP 1133.001 D1). */
    val views: Map<String, String> get() = emptyMap()
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

    /** Who made the call being answered now, `app`, `tool` or `wake`, read at [ExactExtension.later]'s entry: a
     * capture's first lease is refused to a tool's read (LLP 1133.009 D6). */
    var origin = "app"
        internal set

    /** The session's track table, which its implementations share (LLP 1133.009 D10 item 6). */
    val tracks: Tracks get() = Extensions.tracks

    /** The file an admitted `app:/` path names (LLP 1133.009 D10 item 4): the app's own directories, a drive's
     * scratch tree under the agent. Admission checked the path against the calling source's grants. */
    fun file(path: String): java.io.File? = try { Native.appFile(path)?.let { java.io.File(it) } } catch (e: UnsatisfiedLinkError) { null }

    /** Whether the activity is resumed: a prompt shows only over a resumed activity (LLP 1133.010 D8). */
    val resumed: Boolean get() = Extensions.resumedNow

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

    /**
     * Runtime permission [name]'s state, from the one history every module reads (LLP 1133.008 D8 item 7): `granted`;
     * `denied` once a request was refused and the platform owes no rationale after it, so it will not ask again; else
     * `prompt`. A grant in Settings clears the history.
     */
    fun permission(name: String): PermissionState {
        val granted = application.checkSelfPermission(name) == android.content.pm.PackageManager.PERMISSION_GRANTED
        val history = application.getSharedPreferences("exact.permissions", Context.MODE_PRIVATE)
        if (granted && history.getBoolean(name, false)) history.edit().remove(name).apply()
        val rationale = !granted && activity?.shouldShowRequestPermissionRationale(name) == true
        val permanent = !granted && history.getBoolean(name, false)
        return PermissionState(if (granted) "granted" else if (permanent) "denied" else "prompt", rationale, permanent)
    }

    /** [requestPermissions] for [name], keeping the history [permission] reads. */
    fun askPermission(name: String, done: () -> Unit) = requestPermissions(arrayOf(name)) {
        val a = activity
        if (permission(name).state != "granted" && a != null && !a.shouldShowRequestPermissionRationale(name)) {
            application.getSharedPreferences("exact.permissions", Context.MODE_PRIVATE).edit().putBoolean(name, true).apply()
        }
        done()
    }
}

/** A runtime permission's state and the platform's words for it ([ExtensionContext.permission]). */
class PermissionState(val state: String, val shouldShowRationale: Boolean, val permanentlyDenied: Boolean)

/** The process's extensions: made at their first call, they outlive the activities that show the app. */
internal object Extensions {
    private const val CODE = 0x45584154 and 0xffff
    private val written: ExtensionRoster? by lazy {
        val type = try { Class.forName("exact.app.ExactExtensions") } catch (e: ClassNotFoundException) { return@lazy null }
        type.getDeclaredConstructor().newInstance() as ExtensionRoster
    }
    private val roster: Map<String, (ExtensionContext) -> ExactExtension> get() = written?.extensions ?: emptyMap()
    private val made = HashMap<String, ExactExtension>()
    /** The one session's track table. */
    val tracks = Tracks()
    private var application: Context? = null
    private var foreground = false
    private var asking = false
    private val waiting = ArrayList<() -> Unit>()

    /** The activity was made: the host binds the names of the extensions the APK carries. */
    fun bind(activity: Activity) {
        if (application == null) application = activity.applicationContext
        Presence.application = activity.applicationContext
        if (roster.isNotEmpty()) Exact.bindExtensions(roster.keys.joinToString("\n"))
    }

    /**
     * Request [id]'s call, the runner's body: `{"extension", "source"|"send", "args", "preferred"?}`, `{"extension",
     * "release", "args"?}`, `{"extension", "preferred"}`, `{"extension", "dispose"}`, `{"extension", "foreground"}`, or
     * `{"extension", "view", "displayed"|"bound", "origin"}` (a package view's report, LLP 1133.008 D8 item 2).
     */
    fun call(id: Int, body: String) {
        var answered = false
        val reply = { answer: JSONObject -> if (!answered) { answered = true; Exact.answer(id, answer.toString()) } }
        try {
            val request = JSONObject(body)
            val name = request.getString("extension")
            val extension = extension(name) ?: return reply(JSONObject().put("error", "NotSupportedError"))
            val declared = request.optString("source").ifEmpty { request.optString("send") }
            val released = request.optString("release")
            when {
                request.has("view") -> {
                    val (view, origin) = request.getString("view") to request.optString("origin", "app")
                    if (request.has("displayed")) extension.displayed(extension.context.session, view, request.getBoolean("displayed"), origin)
                    if (request.has("bound")) extension.bound(extension.context.session, view, request.getString("bound"), origin)
                    reply(JSONObject())
                }
                released.isNotEmpty() -> {
                    val args = request.optJSONObject("args")
                    if (args != null) extension.release(released, args) else extension.release(released)
                    reply(JSONObject())
                }
                declared.isNotEmpty() -> {
                    extension.context.origin = request.optString("origin").ifEmpty { "app" }
                    extension.later(declared, request.optJSONObject("args") ?: JSONObject(), request.opt("preferred"), reply)
                }
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

    /** Whether the activity is resumed ([resumed], [paused]). */
    val resumedNow: Boolean get() = foreground

    /** The extension that serves the authenticator entry (LLP 1133.010 D13), made if need be. */
    fun authenticator(): ExactAuthenticator? = roster.keys.firstNotNullOfOrNull { extension(it) as? ExactAuthenticator }

    /** Extension [name], made at its first call or view. */
    private fun extension(name: String): ExactExtension? = made[name]
        ?: roster[name]?.let { make -> application?.let { make(ExtensionContext(it, name)) } }?.also { made[name] = it; webauthn.forEach { f -> it.webauthnField(f, true) } }

    /** The live fields whose `autocomplete` ends in `webauthn` ([Fields]). */
    private val webauthn = LinkedHashSet<android.widget.EditText>()

    fun webauthnField(view: android.widget.EditText, present: Boolean) {
        if (if (present) !webauthn.add(view) else !webauthn.remove(view)) return
        made.values.forEach { it.webauthnField(view, present) }
    }

    /** The app's module views with each package's beside them, each made by its extension (LLP 1133.001 D1). */
    fun withViews(modules: Map<String, NativeModule>): Map<String, NativeModule> {
        val views = written?.views ?: emptyMap()
        return if (views.isEmpty()) modules else modules + views.mapValues { (tag, name) -> PackageView(tag, name) }
    }

    /** A package's view, as [Natives] makes a module's: the process's one extension makes it for its session. */
    private class PackageView(val tag: String, val name: String) : NativeModule {
        private val emits = HashMap<android.view.View, (String, String) -> Unit>()
        override fun create(context: Context, props: JSONObject) = create(context, props, 0)
        override fun create(context: Context, props: JSONObject, id: Int): android.view.View {
            val extension = extension(name) ?: throw IllegalStateException("no extension $name for $tag")
            lateinit var made: android.view.View
            made = extension.view(tag, extension.context.session, id.toString(), context, props) { kind, text -> emits[made]?.invoke(kind, text) }
            return made
        }
        override fun update(view: android.view.View, props: JSONObject) { extension(name)?.update(view, props) }
        override fun events(view: android.view.View, emit: (String, String) -> Unit) { emits[view] = emit }
        override fun destroy(view: android.view.View) { emits.remove(view); extension(name)?.destroyView(view) }
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
