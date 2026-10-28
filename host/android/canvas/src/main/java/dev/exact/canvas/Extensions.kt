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

    /** The send numbered [request] ([ExtensionContext.request]) was let go: what it started stops (LLP 1133.016 D15
     * item 2). */
    open fun cancel(request: Int) {}

    /** The app went to the foreground or left it ([ExtensionContext.foreground], LLP 1133.009 D10 item 3). */
    open fun foreground(shown: Boolean) {}

    /** The wake [ExtensionContext.wake] raised for [id] ended with [outcome] (LLP 1133.017 D8), after its `ended`. */
    open fun woke(id: String, outcome: String) {}

    /** The activity is in front again: what changed in Settings meanwhile is read again (Android has no event). */
    open fun resumed() {}

    /** [Activity.onPause]: what only the activity in front may hold is let go, such as NFC reader mode, which
     * [android.nfc.NfcAdapter.enableReaderMode] admits only while the activity is resumed (LLP 1133.035 D16 item 5). */
    open fun paused() {}

    /**
     * Picture [source] of the interface's `img` scheme (`photos:<id>`, LLP 1133.019 D2, D11 item 1) for a box of
     * [width] by [height] device pixels at `object-fit` [fit], once the runner admitted it: [reply] once, from any
     * thread, with the picture upright and no larger than the box needs, or null and an error code (`NotFoundError`),
     * which empties the box. [owned]: admitted as the app's own item alone (D6), drawn only where the platform says it
     * is.
     */
    open fun picture(source: String, width: Int, height: Int, fit: String, owned: Boolean, reply: (android.graphics.Bitmap?, String?) -> Unit) =
        reply(null, "NotSupportedError")

    /** The state the runner holds of each permission this extension's interfaces declare, by name, told with
     * [preferred] under the agent (LLP 1133.019 §10). */
    open fun permissions(states: JSONObject) {}

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

    /**
     * The canvases of [surface], which this extension's input feeds ([ExtensionSurfaces]), were displayed or hidden,
     * by the origin of the commit that did it (LLP 1133.013 D6, D12 item 3). The lease: hidden, the input pauses;
     * shown by an `app` commit, it resumes; shown by a `tool`'s or a `wake`'s, it stays paused. A start on a hidden
     * canvas is told this before the start.
     */
    open fun displayed(surface: String, shown: Boolean, origin: String) {}

    /** The display's angle changed ([ExtensionContext.orientation]). */
    open fun orientation(angle: Int) {}
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
    /** The extensions whose interface says `"start": "load"`: made after the reader's first frame rather than at their
     * first call (LLP 1133.015 D16 item 1). */
    val startAtLoad: List<String> get() = emptyList()
}

/** What an extension may use of the host. */
class ExtensionContext internal constructor(val application: Context, private val name: String) {
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

    /** The number the runner gave the send being answered now, which a later [ExactExtension.cancel] names; null for
     * a source (LLP 1133.016 D15 item 2). */
    var request: Int? = null
        internal set

    /** The session's track table, which its implementations share (LLP 1133.009 D10 item 6). */
    val tracks: Tracks get() = Extensions.tracks

    /** The process's audio focus owner, through which a module that plays or captures claims focus (LLP 1133.012
     * D12 item 2). */
    val audioFocus: AudioFocus get() = AudioFocus

    /** The file an admitted `app:/` path names (LLP 1133.009 D10 item 4): the app's own directories, a drive's
     * scratch tree under the agent. Admission checked the path against the calling source's grants. */
    fun file(path: String): java.io.File? = try { Native.appFile(path)?.let { java.io.File(it) } } catch (e: UnsatisfiedLinkError) { null }

    /** A content URI for the file an admitted `app:/` path names, which an intent may carry with a read grant
     * (`FLAG_GRANT_READ_URI_PERMISSION`), or null when it names none ([Files], LLP 1133.024 D22 item 2). */
    fun contentUri(path: String): android.net.Uri? = file(path)?.let { Files.uri(application, it) }

    /** Whether the activity is resumed: a prompt shows only over a resumed activity (LLP 1133.010 D8). */
    val resumed: Boolean get() = Extensions.resumedNow

    /** Whether the app is in the foreground, as the runner's admission reads it ([ExactExtension.foreground]). */
    var foreground = true
        internal set

    /** The session's clock, ms: the agent's under the agent (LLP 1133.009 D10 item 2). */
    fun now(): Double = Native.now()

    /** The page's canvases, whose input an extension may feed (LLP 1133.013 D6). */
    val surfaces = ExtensionSurfaces()

    /** Claims on the host's foreground services, which keep a capability running while the app is hidden (LLP
     * 1133.033 D2). */
    val foregroundService = ForegroundServices(application, name) { agent }

    /**
     * The display's angle as the web's `screen.orientation.angle` (0, 90, 180, 270; LLP 1133.013 D12 item 4), from
     * `Display.getRotation`: [ExactExtension.orientation] is told each change.
     */
    val orientation: Int get() = Extensions.angle(activity)

    /** The readers of [topic] are asked again (LLP 1016.002). */
    fun changed(topic: String) = Exact.changed(topic)

    /**
     * Raise the interface's event [event] with [args] (LLP 1133.004 D6): delivered in order, once, with the device's
     * origin. Its one channel to the runner is [changed], as on Apple (`exact-event:`,
     * `runner/src/runner/modules/events.rs`); the sequence number keeps two alike from coalescing. Main thread.
     */
    fun emit(event: String, args: JSONArray) {
        Extensions.emitted += 1
        changed("exact-event:" + JSONObject().put("extension", name).put("event", event).put("args", args).put("seq", Extensions.emitted))
    }

    /**
     * Under the agent, what the implementation discloses of its own objects for `state.extensions` (LLP 1133.008 D8
     * item 9): listed as its `implementation`, each field replacing the one of its name said before. Through
     * [changed], as on Apple (`exact-state:`, `runner/src/runner/modules/disclosed.rs`). Nothing outside the agent.
     */
    fun state(fields: JSONObject) {
        if (agent) changed("exact-state:" + JSONObject().put("extension", name).put("state", fields))
    }

    /** The process's pads (LLP 1133.022 D10 item 1), which a world canvas reads too. */
    val pads = ExtensionPads(application)

    /** Each [wake]'s `ended`, by id, until the runner tells its outcome. */
    internal val ended = HashMap<String, (String) -> Unit>()

    /**
     * Raise the interface's wake event [event] with [args] for the platform's own [id] (LLP 1133.017 D8): the events
     * of one id are one wake, run with the wake's origin, and [ended] hears its outcome once, on the main thread:
     * `completed`, `expired`, `superseded`, `refused`, or `unheard` when no handler hears it. Its one channel to the
     * runner is [changed], as on Apple (`exact-wake:`, `runner/src/runner/modules/wakes.rs`).
     */
    fun wake(event: String, args: JSONArray, id: String, ended: (String) -> Unit = {}) {
        this.ended[id] = ended
        changed("exact-wake:" + JSONObject().put("extension", name).put("event", event).put("args", args).put("id", id))
    }

    /** Every `img` showing picture [source] is emptied and fires `error`, and no reply in flight draws (LLP 1133.019
     * D2): the photo left what the app may read. */
    fun revokePicture(source: String) = Exact.pictureChanged(source, true)

    /** Picture [source] is asked for again: the photo was edited. */
    fun changedPicture(source: String) = Exact.pictureChanged(source, false)

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

    /**
     * Start [intent] for its result (`Activity.startActivityForResult`), then [done] on the main thread with the
     * result code and the result's data (LLP 1133.018 D13 item 2): a picker's or an editor's answer. A result that
     * reaches an activity made again meanwhile settles the call, since the call is the process's. With no activity in
     * front, or no app that handles [intent], nothing starts and [done] has `Activity.RESULT_CANCELED`.
     */
    fun startActivityForResult(intent: android.content.Intent, done: (Int, android.content.Intent?) -> Unit) = Extensions.start(intent, done)

    /** [startActivityForResult] for a [android.content.IntentSender]: a Play services resolution's (Google Pay's sheet). */
    fun startIntentSenderForResult(sender: android.content.IntentSender, done: (Int, android.content.Intent?) -> Unit) = Extensions.start(sender, done)

    /**
     * The app's handler [handler] run for the work [handle] names (LLP 1133.016 D15 item 1): one the call that made
     * the handle listed, run by the runner as a fresh answer whose origin is at most `tool`. [reply] gets
     * `{"text": JSON}` or `{"error", "message"}`, on the main thread.
     */
    fun invoke(handle: String, handler: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        val body = JSONObject().put("extension", name).put("handle", handle).put("handler", handler).put("args", args)
        val id = try { Native.invoke(body.toString()) } catch (e: UnsatisfiedLinkError) { -1 }
        if (id < 0) return reply(JSONObject().put("error", "InvalidStateError").put("message", "no session runs"))
        // The answer lands on the host's turn; the main thread reads it a frame at a time.
        val main = android.os.Handler(android.os.Looper.getMainLooper())
        lateinit var poll: Runnable
        poll = Runnable {
            val answer = try { Native.invoked(id) } catch (e: UnsatisfiedLinkError) { "{\"error\":\"InvalidStateError\"}" }
            if (answer != null) reply(JSONObject(answer)) else main.postDelayed(poll, 16)
        }
        main.post(poll)
    }

    /**
     * Whether the embedder lets the app leave for [url] (LLP 1133.024 D22 item 7), [how] `browser` (a page over the
     * app) or `external` (another app): asked before a page or a hand-off by URL opens. The stock activity lets every
     * one go; an embedding app sets [ExtensionContext.leaving] to decide. Main thread.
     */
    fun leave(url: String, how: String): Boolean = leaving?.invoke(url, how) ?: true

    companion object {
        /** The embedder's decision on leaving the app ([leave]); null lets every URL go. */
        @JvmStatic var leaving: ((String, String) -> Boolean)? = null
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

/** The process's gamepads as a module reads them ([ExtensionContext.pads]): each by its slot, the index a world
 * canvas knows it by. Main thread. */
class ExtensionPads internal constructor(private val application: Context) {
    /** The connected pads by index, with their identities. */
    fun list(): List<Pair<Int, PadDevice>> = Pads.attach(application).list()
    /** The pad whose button last went down; null when none has been used since it connected. */
    val current: Int? get() = Pads.current.takeIf { it >= 0 }
    /** Pad [index]'s latest standard reading: 17 buttons, then 4 axes; null for no pad. */
    fun reading(index: Int): FloatArray? = Pads.values(index)
    /** [f] is told each change: a pad came or went, or another is in use. Its [owner] ends it with [unwatch]. */
    fun watch(owner: Any, f: () -> Unit) { Pads.attach(application).watch(owner, f) }
    fun unwatch(owner: Any) = Pads.unwatch(owner)
}

/** The page's canvases, as an extension feeds them ([ExtensionContext.surfaces]). */
class ExtensionSurfaces internal constructor() {
    /**
     * The input of surface [surface]'s canvas: what is written there the host delivers to the live canvas of that
     * surface once a display frame, before the frame's input reaches its module, every sample in order. Null when the
     * page places no canvas of that surface, or the library predates canvas inputs.
     */
    fun input(surface: String): SurfaceInput? = try {
        if (Native.surfaceHas(surface)) SurfaceInput(surface) else null
    } catch (e: UnsatisfiedLinkError) { null }
}

/**
 * A canvas's input that an extension writes ([ExtensionSurfaces.input]), from any thread: each call waits for nothing.
 * The host keeps about a second at the fastest rate a phone samples; past it the oldest samples go, and the next
 * sample of their sensor delivered counts them in its `dropped`.
 */
class SurfaceInput internal constructor(val surface: String) {
    /**
     * One sample of [sensor] (a Generic Sensor name: `gravity`, `gyroscope`, …): x, y, z, and w for an orientation's
     * quaternion, finite, in the frame the input asked for. [dropped] counts the samples lost before it (a pause); [at]
     * is the session clock's time (ms, [ExtensionContext.now]) when it was taken, which also stamps the canvas's keys
     * and touches.
     */
    fun sample(sensor: String, values: DoubleArray, dropped: Int = 0, at: Double) = write(sensor, values, dropped, at, false)

    /** The input of [sensor] ended (`stopSensorInput`, the canvas gone): its last sample no longer stands. */
    fun end(sensor: String, at: Double) = write(sensor, DoubleArray(0), 0, at, true)

    private fun write(sensor: String, values: DoubleArray, dropped: Int, at: Double, ended: Boolean) {
        if (Native.sensorInput(surface, sensor, values, dropped, at, ended)) Exact.wake()
    }
}

/** A runtime permission's state and the platform's words for it ([ExtensionContext.permission]). */
class PermissionState(val state: String, val shouldShowRationale: Boolean, val permanentlyDenied: Boolean)

/** The process's extensions: made at their first call, they outlive the activities that show the app. */
internal object Extensions {
    private const val CODE = 0x45584154 and 0xffff
    private const val RESULTS = 0x5000
    private val written: ExtensionRoster? by lazy {
        val type = try { Class.forName("exact.app.ExactExtensions") } catch (e: ClassNotFoundException) { return@lazy null }
        type.getDeclaredConstructor().newInstance() as ExtensionRoster
    }
    private val roster: Map<String, (ExtensionContext) -> ExactExtension> get() = written?.extensions ?: emptyMap()
    private val made = HashMap<String, ExactExtension>()
    /** The one session's track table. */
    val tracks = Tracks()
    /** The events raised so far ([ExtensionContext.emit]). */
    var emitted = 0
    private var application: Context? = null
    private var foreground = false
    private var asking = false
    private val waiting = ArrayList<() -> Unit>()

    /** The display's angle last told the extensions ([ExtensionContext.orientation]). */
    private var told = -1

    /** [activity]'s display's rotation in degrees, as the web's `screen.orientation.angle`. */
    fun angle(activity: Activity?): Int = when (activity?.display?.rotation) {
        android.view.Surface.ROTATION_90 -> 90
        android.view.Surface.ROTATION_180 -> 180
        android.view.Surface.ROTATION_270 -> 270
        else -> 0
    }

    /** A display changed: the extensions made are told a new angle. */
    private val displays = object : android.hardware.display.DisplayManager.DisplayListener {
        override fun onDisplayAdded(id: Int) {}
        override fun onDisplayRemoved(id: Int) {}
        override fun onDisplayChanged(id: Int) {
            val angle = angle(Exact.reader?.get())
            if (angle == told) return
            told = angle
            made.values.forEach { it.orientation(angle) }
        }
    }

    /** The activity was made: the host binds the names of the extensions the APK carries. */
    fun bind(activity: Activity) {
        if (application == null) {
            val dm = activity.applicationContext.getSystemService(android.hardware.display.DisplayManager::class.java)
            dm?.registerDisplayListener(displays, android.os.Handler(android.os.Looper.getMainLooper()))
            told = angle(activity)
        }
        if (application == null) application = activity.applicationContext
        Presence.application = activity.applicationContext
        AudioFocus.application = activity.applicationContext
        if (roster.isNotEmpty()) Exact.bindExtensions(roster.keys.joinToString("\n"))
    }

    /**
     * Request [id]'s call, the runner's body: `{"extension", "source"|"send", "args", "preferred"?}`, `{"extension",
     * "release", "args"?}`, `{"extension", "preferred"}`, `{"extension", "dispose"}`, `{"extension", "foreground"}`, or
     * `{"extension", "view", "displayed"|"bound", "origin"}` (a package view's report, LLP 1133.008 D8 item 2),
     * `{"extension", "surface", "displayed", "origin"}` (a canvas its input feeds, LLP 1133.013 D6),
     * `{"extension", "picture", "width", "height", "fit", "owned"}` (LLP 1133.019 D11 item 1), answered with its
     * pixels ([Exact.picture]), `{"extension", "cancel"}` (a send let go, LLP 1133.016 D15 item 2), or `{"extension",
     * "woke", "outcome"}` (a wake [ExtensionContext.wake] raised ended, LLP 1133.017 D8).
     */
    fun call(id: Int, body: String) {
        var answered = false
        // A list answer comes as `{"exact:answer": [...]}`, since a reply is an object (as on Apple).
        val reply = { answer: JSONObject ->
            if (!answered) {
                answered = true
                val listed = answer.length() == 1 && answer.has("exact:answer")
                Exact.answer(id, if (listed) answer.get("exact:answer").toString() else answer.toString())
            }
        }
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
                request.has("surface") && request.has("displayed") -> {
                    extension.displayed(request.getString("surface"), request.getBoolean("displayed"), request.optString("origin", "app"))
                    reply(JSONObject())
                }
                request.has("picture") -> {
                    val source = request.getString("picture")
                    val (width, height) = request.getInt("width") to request.getInt("height")
                    extension.picture(source, width, height, request.optString("fit", "fill"), request.optBoolean("owned")) { bitmap, error ->
                        if (!answered) {
                            answered = true
                            val pixels = bitmap?.let(::software)
                            Exact.picture(id, pixels, if (pixels == null) error ?: "UnknownError" else null)
                        }
                    }
                }
                released.isNotEmpty() -> {
                    val args = request.optJSONObject("args")
                    if (args != null) extension.release(released, args) else extension.release(released)
                    reply(JSONObject())
                }
                declared.isNotEmpty() -> {
                    extension.context.origin = request.optString("origin").ifEmpty { "app" }
                    extension.context.request = if (request.has("request")) request.getInt("request") else null
                    extension.later(declared, request.optJSONObject("args") ?: JSONObject(), request.opt("preferred"), reply)
                }
                request.has("preferred") -> {
                    request.optJSONObject("permissions")?.let { extension.permissions(it) }
                    extension.preferred(request.optJSONObject("preferred") ?: JSONObject())
                    reply(JSONObject())
                }
                request.has("cancel") -> {
                    extension.cancel(request.getInt("cancel"))
                    reply(JSONObject())
                }
                request.has("dispose") -> {
                    val ids = request.optJSONArray("dispose") ?: JSONArray()
                    extension.dispose((0 until ids.length()).map { ids.getString(it) })
                    reply(JSONObject())
                }
                request.has("woke") -> {
                    val (woke, outcome) = request.getString("woke") to request.optString("outcome", "completed")
                    extension.context.ended.remove(woke)?.invoke(outcome)
                    extension.woke(woke, outcome)
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

    /** [bitmap] as the host copies it: software `ARGB_8888` in sRGB, drawn into one when it is not. */
    private fun software(bitmap: android.graphics.Bitmap): android.graphics.Bitmap? {
        val argb = android.graphics.Bitmap.Config.ARGB_8888
        val srgb = android.graphics.ColorSpace.get(android.graphics.ColorSpace.Named.SRGB)
        // A hardware bitmap's pixels are the GPU's: copied out first.
        val soft = if (bitmap.config == android.graphics.Bitmap.Config.HARDWARE) bitmap.copy(argb, false) ?: return null else bitmap
        if (soft.config == argb && soft.colorSpace?.let { it == srgb } != false) return soft
        val out = android.graphics.Bitmap.createBitmap(soft.width, soft.height, argb, true, srgb)
        android.graphics.Canvas(out).drawBitmap(soft, 0f, 0f, null)
        return out
    }

    /** Whether the activity is resumed ([resumed], [paused]). */
    val resumedNow: Boolean get() = foreground

    /** The extension that serves the authenticator entry (LLP 1133.010 D13), made if need be. */
    fun authenticator(): ExactAuthenticator? = roster.keys.firstNotNullOfOrNull { extension(it) as? ExactAuthenticator }

    /** Extension [name], made at its first call or view. */
    /** The reader drew its first frame: each extension declared `"start": "load"` is made now (LLP 1133.015 D16 item
     * 1), once for the process, as an extension outlives the activities that show the app. */
    fun firstFrame() {
        written?.startAtLoad?.forEach { extension(it) }
    }

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

    /** The calls waiting on an activity's result ([ExtensionContext.startActivityForResult]), by request code. */
    private val results = HashMap<Int, (Int, android.content.Intent?) -> Unit>()
    private var started = 0

    fun start(intent: android.content.Intent, done: (Int, android.content.Intent?) -> Unit) {
        val activity = Exact.reader?.get()
        if (activity == null || !foreground) return done(Activity.RESULT_CANCELED, null)
        // Request codes of their own range, apart from the permission request's [CODE].
        val code = RESULTS + (started++ % 0x1000)
        results[code] = done
        try {
            activity.startActivityForResult(intent, code)
        } catch (e: android.content.ActivityNotFoundException) {
            results.remove(code)
            done(Activity.RESULT_CANCELED, null)
        }
    }

    fun start(sender: android.content.IntentSender, done: (Int, android.content.Intent?) -> Unit) {
        val activity = Exact.reader?.get()
        if (activity == null || !foreground) return done(Activity.RESULT_CANCELED, null)
        val code = RESULTS + (started++ % 0x1000)
        results[code] = done
        try {
            activity.startIntentSenderForResult(sender, code, null, 0, 0, 0)
        } catch (e: android.content.IntentSender.SendIntentException) {
            results.remove(code)
            done(Activity.RESULT_CANCELED, null)
        }
    }

    /** [Activity.onActivityResult]: the answer of the call that started it, if this process made one. */
    fun activityResult(code: Int, result: Int, data: android.content.Intent?) { results.remove(code)?.invoke(result, data) }

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

    /** [Activity.onPause]. */
    fun paused() {
        foreground = false
        made.values.forEach { it.paused() }
    }
}
