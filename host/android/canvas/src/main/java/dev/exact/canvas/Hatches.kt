package dev.exact.canvas

import android.app.Activity
import android.graphics.RectF
import android.util.Log
import android.view.View

/**
 * An app's own Kotlin at its nodes: access hatches on Android (LLP 1075.003.000.001, LLP 1115.001.000 D4). The app's
 * Kotlin in `modules/android` defines one `class Hatches : ExactHatches()` in package `exact.app`; the reader makes it the
 * first time exact2 tells a moment, which is after first pixel, and calls it on the main thread. The scopes and
 * moments are the other hosts': [app] when the hatches connect and when a root `data-*` word changes, [element] when
 * a node marked `hatch="word"` (a word `app.json` gives `android`) is built, and again when its `data-*` words or its
 * box's size change, [elementEnded] when it leaves the tree.
 *
 * Every node is painted by exact2, so most have no view: a handle has the node's box and words, acts on the node as
 * a person would ([ExactElement.click] and the rest), hears the input that lands on it ([observe]), and can show a
 * view of the app's own in the node's box ([ExactElement.host]). A node that is a platform view gives it
 * ([ExactElement.view]: a text field's `EditText`), and a date or time control gives its picker as it opens
 * ([picker]). The clock is exact2's ([frames], [after]): the agent's under a drive, so a hatch's motion is replayed
 * as the app's is. What a hatch reports goes to the agent's `diagnostics` ([diagnostics], [ExactElement.diagnostics]).
 */
abstract class ExactHatches {
    /** The activity the app runs in now: set before the first call, given each activity made again, and null while
     * none is (between one's end and the next's start). */
    var activity: Activity? = null
        internal set

    internal var reader: HatchReader? = null

    /** The app: built as the hatches connect, changed when a root `data-*` word changes. */
    open fun app(app: ExactApp) {}

    /** A hatched node: built after the commit that mounts it, changed when its `data-*` words or its size change. */
    open fun element(element: ExactElement) {}

    /** The node has left the tree. The handle keeps its last box and words; its observers have stopped and a view
     * it hosted is let go. */
    open fun elementEnded(element: ExactElement) {}

    /**
     * A hatched date or time control's picker, about to open (LLP 1115.001.000 D3): set what it offers, or take it
     * over ([ExactPicker.show]). Called on every press, with what the control holds then.
     */
    open fun picker(element: ExactElement, picker: ExactPicker) {}

    /** [tick] at each frame exact2 presents, after the frame's own tasks and timers, until the ticket stops. */
    fun frames(tick: (ExactFrame) -> Unit): ExactTicket = reader!!.register(0, 5, null, Call.Tick(tick))

    /** [fired] once, [ms] of exact2's clock from now. */
    fun after(ms: Double, fired: () -> Unit): ExactTicket = reader!!.register(0, 6, ms.toString(), Call.Fired(fired))

    /** [heard] for the input that lands in [element]'s box, after exact2's own dispatch: it can claim none. It ends
     * with the node. */
    fun observe(element: ExactElement, heard: (ExactInput) -> Unit): ExactTicket =
        reader!!.register(element.node, 7, null, Call.Heard(heard))

    /** Diagnostics scoped to the module. */
    val diagnostics: ExactDiagnostics get() = ExactDiagnostics(0)
}

/** The app scope's handle: the first root node's `data-*` words, by name without the prefix. */
class ExactApp internal constructor(val data: Map<String, String>, val isNew: Boolean)

/** A hatched node's handle. Cheap to keep: it holds the box and words of the node's last moment, and after the
 * node's end its last. A node made again in its place (a list's row given another item) is another handle. */
class ExactElement internal constructor(val node: Int, private val reader: HatchReader) {
    /** The node's `hatch` word. */
    var hatch = ""
        internal set
    /** The node's authored `id`, empty when it has none. */
    var id = ""
        internal set
    /** The border box exact2 laid out, in the document's points, as of the node's last moment (built, or a change of
     * its size or its words: a move alone is not told). Scroll offsets and transforms are not folded in. */
    var frame = RectF()
        internal set
    /** Every declared `data-*` word the node carries. */
    var words: Map<String, String> = emptyMap()
        internal set
    /** This is the built moment. */
    var isNew = true
        internal set
    /** The node is in the tree. */
    var isLive = true
        internal set

    /** A declared `data-*` word's value (`data("tone")` for `data-tone`). */
    fun data(word: String): String? = words[word]

    /** The node's platform view where it is one: a text field's `EditText` once the reader hosts it (LLP 1115.001
     * D8). Null for a painted node. exact2 keeps what it sets (its text, its look, its place); the app may add to it. */
    val view: View? get() = if (reader.current(this)) reader.view(node) else null

    /** Press the node, as a person would: run by exact2 between its turns. False when this handle's node has left. */
    fun click() = reader.act(this, 1, null)
    /** Give the node the focus. */
    fun focus() = reader.act(this, 2, null)
    /** Take the focus from the node, if it has it. */
    fun blur() = reader.act(this, 3, null)
    /** Replace an authored text field's whole value, without moving the focus. */
    fun input(text: String) = reader.act(this, 4, text)

    /**
     * Show [view], the app's own, in the node's border box, over what exact2 paints there: laid out at the box's size
     * and drawn where the node is, so it scrolls, clips and stacks with the page, as a `video` or a native module's
     * view is. It is drawn, not touched: the input that lands there is exact2's, which [ExactHatches.observe] hears.
     * Null takes it away; the node's end does too. False when this handle's node has left.
     */
    fun host(view: View?): Boolean = reader.host(this, view)

    /** Diagnostics scoped to the node's word (`element <word>`). */
    val diagnostics: ExactDiagnostics get() = ExactDiagnostics(node)
}

/** An instant of exact2's clock: its time in ms (the agent's under a drive) and the commit its state ends at. */
class ExactFrame internal constructor(val now: Double, val seq: Int)

/** Input that landed on an observed node. */
sealed class ExactInput {
    /** Whether exact2 handled it: a handler ran, or a scroller moved. */
    abstract val handled: Boolean

    /** A finger or a pointer: [phase] is `down`, `move`, `up` or `cancel`; [x], [y] in the document's points, and
     * [localX], [localY] in the node's box. */
    class Pointer internal constructor(
        val phase: String, val x: Float, val y: Float, val localX: Float, val localY: Float, override val handled: Boolean,
    ) : ExactInput()

    /** A key, by the web's `key` and `code`. */
    class Key internal constructor(
        val key: String, val code: String, val down: Boolean, val repeat: Boolean, override val handled: Boolean,
    ) : ExactInput()
}

/** A registration with exact2's clock or input: [stop] ends it. */
class ExactTicket internal constructor(private val token: Long, private val reader: HatchReader?) {
    fun stop() { if (token >= 0) reader?.stop(token) }
}

/** What a hatch reports to the agent's `diagnostics`, as a Rust hatch's does: nothing in a production build. */
class ExactDiagnostics internal constructor(private val node: Int) {
    fun log(text: String) { Exact.hatchAct(node, 9, text) }
    fun count(name: String, by: Long = 1) { Exact.hatchAct(node, 10, "$name\n$by") }
    fun measure(name: String, ms: Double) { Exact.hatchAct(node, 11, "$name\n$ms") }
    fun publish(name: String, json: String) { Exact.hatchAct(node, 12, "$name\n$json") }
}

/** What a ticket calls. */
internal sealed class Call {
    class Tick(val f: (ExactFrame) -> Unit) : Call()
    class Fired(val f: () -> Unit) : Call()
    class Heard(val f: (ExactInput) -> Unit) : Call()
}

/** What exact2 tells of the app's hatches (`hatches/reader.rs` writes the words), made into handles and calls. */
internal class HatchReader(activity: ExactActivity, private val hatches: ExactHatches) {
    private val elements = HashMap<Int, ExactElement>()
    private val calls = HashMap<Long, Call>()
    private val hosted = HashMap<Int, View>()

    /** The activity that runs now: an activity made again is given to the app's hatches, and one that ends lets go. */
    var activity: ExactActivity? = activity
        set(value) { field = value; hatches.activity = value }

    init {
        hatches.activity = activity
        hatches.reader = this
    }

    /** [e] is its node's handle now, and the node is in the tree. */
    fun current(e: ExactElement): Boolean = e.isLive && elements[e.node] === e

    fun view(node: Int): View? = activity?.hostedField(node)

    fun act(e: ExactElement, kind: Int, text: String?): Boolean = current(e) && Exact.hatchAct(e.node, kind, text) >= 0

    fun register(node: Int, kind: Int, text: String?, call: Call): ExactTicket {
        val token = Exact.hatchAct(node, kind, text)
        if (token >= 0) calls[token] = call
        return ExactTicket(token, this)
    }

    fun stop(token: Long) {
        if (calls.remove(token) != null) Exact.hatchAct(0, 8, token.toString())
    }

    /** The view hosted at [node], for the elements' layer ([Natives], kind 5). */
    fun hosted(node: Int): View? = hosted[node]

    fun host(e: ExactElement, view: View?): Boolean {
        if (!current(e)) return false
        if (view == null) { hosted.remove(e.node); return Exact.hatchAct(e.node, 14, null) >= 0 }
        hosted[e.node] = view
        return Exact.hatchAct(e.node, 13, null) >= 0
    }

    /** The moments in [w] from [at] to [end]. */
    fun told(w: IntArray, at: Int, end: Int) {
        var i = at
        while (i < end) {
            val scope = w[i]
            val next = skip(scope, w, i)
            try {
                when (scope) {
                    TICK, FIRED, HEARD -> clock(scope, w, i)
                    else -> moment(w, i)
                }
            } catch (e: Exception) {
                // A throw in the app's hatch: that call is lost, the app goes on (LLP 1075.003.000.001 D8).
                Log.e(TAG, "hatch: $e")
            }
            i = next
        }
    }

    /** An app or element moment at [at]. */
    private fun moment(w: IntArray, at: Int) {
        val scope = w[at]; val moment = w[at + 1]; val node = w[at + 2]
        val frame = RectF(Float.fromBits(w[at + 3]), Float.fromBits(w[at + 4]), 0f, 0f)
        frame.right = frame.left + Float.fromBits(w[at + 5]); frame.bottom = frame.top + Float.fromBits(w[at + 6])
        val (word, afterWord) = string(w, at + 7)
        val (id, afterId) = string(w, afterWord)
        val count = w[afterId]
        var i = afterId + 1
        val data = LinkedHashMap<String, String>()
        repeat(count) {
            val (name, afterName) = string(w, i)
            val (value, afterValue) = string(w, afterName)
            data[name] = value
            i = afterValue
        }
        if (scope == APP) { if (moment != ENDED) hatches.app(ExactApp(data, moment == BUILT)) }
        else if (scope == ELEMENT) element(node, moment, word, id, frame, data)
    }

    /** A tick, an `after` or input at [at]: its call. */
    private fun clock(scope: Int, w: IntArray, at: Int) {
        val token = w[at + 1].toLong()
        when (val call = calls[token]) {
            is Call.Tick -> {
                val now = Double.fromBits((w[at + 2].toLong() shl 32) or (w[at + 3].toLong() and 0xffffffffL))
                call.f(ExactFrame(now, w[at + 4]))
            }
            is Call.Fired -> { calls.remove(token); call.f() }
            is Call.Heard -> call.f(input(w, at + 3))
            null -> {}
        }
    }

    private fun input(w: IntArray, at: Int): ExactInput =
        if (w[at] == 1) {
            val f = { k: Int -> Float.fromBits(w[at + k]) }
            ExactInput.Pointer(PHASES[w[at + 1]], f(2), f(3), f(4), f(5), w[at + 6] != 0)
        } else {
            val (key, a) = string(w, at + 1)
            val (code, b) = string(w, a)
            ExactInput.Key(key, code, w[b] != 0, w[b + 1] != 0, w[b + 2] != 0)
        }

    /** The index after the moment at [at]. */
    private fun skip(scope: Int, w: IntArray, at: Int): Int = when (scope) {
        TICK -> at + 5
        FIRED -> at + 2
        HEARD -> if (w[at + 3] == 1) at + 10 else {
            val (_, a) = string(w, at + 4)
            val (_, b) = string(w, a)
            b + 3
        }
        else -> {
            var i = string(w, string(w, at + 7).second).second
            val count = w[i]; i++
            repeat(2 * count) { i = string(w, i).second }
            i
        }
    }

    private fun element(node: Int, moment: Int, word: String, id: String, frame: RectF, data: Map<String, String>) {
        val e = elements.getOrPut(node) { ExactElement(node, this) }
        e.hatch = word; e.id = id; e.frame = frame; e.words = data
        e.isNew = moment == BUILT
        e.isLive = moment != ENDED
        if (moment == ENDED) {
            elements.remove(node)
            hosted.remove(node)
            hatches.elementEnded(e)
        } else hatches.element(e)
    }

    /** A hatched control's picker, about to open. */
    fun picker(node: Int, picker: ExactPicker) {
        val e = elements[node] ?: return
        try { hatches.picker(e, picker) } catch (e: Exception) { Log.e(TAG, "hatch picker: $e") }
    }

    companion object {
        private const val TAG = "exact"
        private const val APP = 1
        private const val ELEMENT = 3
        private const val TICK = 5
        private const val FIRED = 6
        private const val HEARD = 7
        private const val BUILT = 0
        private const val ENDED = 2
        private val PHASES = arrayOf("down", "move", "up", "cancel")

        /** The app's `exact.app.Hatches`, when it has one. One that cannot be made (its constructor threw, it has
         * none without arguments, it is no [ExactHatches]) is logged, and the app runs without its hatches. */
        fun make(activity: ExactActivity): HatchReader? {
            val type = try { Class.forName("exact.app.Hatches") } catch (e: ClassNotFoundException) { return null }
            return try {
                HatchReader(activity, type.getDeclaredConstructor().newInstance() as ExactHatches)
            } catch (e: Exception) {
                val cause = (e as? java.lang.reflect.InvocationTargetException)?.targetException ?: e
                Log.e(TAG, "hatches: exact.app.Hatches could not be made: $cause")
                null
            }
        }

        /** The string at [at]: its bytes' count, then UTF-8 padded to whole words. And the index after it. */
        fun string(w: IntArray, at: Int): Pair<String, Int> {
            val n = w[at]
            val bytes = ByteArray(n) { k -> (w[at + 1 + k / 4] ushr (8 * (k % 4))).toByte() }
            return String(bytes, Charsets.UTF_8) to at + 1 + (n + 3) / 4
        }
    }
}
