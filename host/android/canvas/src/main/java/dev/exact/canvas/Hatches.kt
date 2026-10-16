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
 * What a handle carries is what the Canvas host has. Every node is painted by exact2, so most have no view: a
 * handle has the node's box and words, and acts on the node as a person would ([ExactElement.click] and the rest). A
 * node that is a platform view gives it ([ExactElement.view]: a text field's `EditText`), and a date or time
 * control gives its picker as it opens ([picker]).
 */
abstract class ExactHatches {
    /** The activity the app runs in now: set before the first call, given each activity made again, and null while
     * none is (between one's end and the next's start). */
    var activity: Activity? = null
        internal set

    /** The app: built as the hatches connect, changed when a root `data-*` word changes. */
    open fun app(app: ExactApp) {}

    /** A hatched node: built after the commit that mounts it, changed when its `data-*` words or its size change. */
    open fun element(element: ExactElement) {}

    /** The node has left the tree. The handle keeps its last box and words. */
    open fun elementEnded(element: ExactElement) {}

    /**
     * A hatched date or time control's picker, about to open (LLP 1115.001.000 D3): set what it offers, or take it
     * over ([ExactPicker.show]). Called on every press, with what the control holds then.
     */
    open fun picker(element: ExactElement, picker: ExactPicker) {}
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
}

/** What exact2 tells of the app's hatches (`hatches/reader.rs` writes the words), made into handles and calls. */
internal class HatchReader(activity: ExactActivity, private val hatches: ExactHatches) {
    private val elements = HashMap<Int, ExactElement>()

    /** The activity that runs now: an activity made again is given to the app's hatches, and one that ends lets go. */
    var activity: ExactActivity? = activity
        set(value) { field = value; hatches.activity = value }

    init {
        hatches.activity = activity
    }

    /** [e] is its node's handle now, and the node is in the tree. */
    fun current(e: ExactElement): Boolean = e.isLive && elements[e.node] === e

    fun view(node: Int): View? = activity?.hostedField(node)

    fun act(e: ExactElement, kind: Int, text: String?): Boolean = current(e) && Exact.hatchAct(e.node, kind, text)

    /** The moments in [w] from [at] to [end]. */
    fun told(w: IntArray, at: Int, end: Int) {
        var i = at
        while (i < end) {
            val scope = w[i]; val moment = w[i + 1]; val node = w[i + 2]
            val frame = RectF(Float.fromBits(w[i + 3]), Float.fromBits(w[i + 4]), 0f, 0f)
            frame.right = frame.left + Float.fromBits(w[i + 5]); frame.bottom = frame.top + Float.fromBits(w[i + 6])
            val (word, afterWord) = string(w, i + 7)
            val (id, afterId) = string(w, afterWord)
            val count = w[afterId]
            i = afterId + 1
            val data = LinkedHashMap<String, String>()
            repeat(count) {
                val (name, afterName) = string(w, i)
                val (value, afterValue) = string(w, afterName)
                data[name] = value
                i = afterValue
            }
            try {
                if (scope == APP) { if (moment != ENDED) hatches.app(ExactApp(data, moment == BUILT)) }
                else if (scope == ELEMENT) element(node, moment, word, id, frame, data)
            } catch (e: Exception) {
                // A throw in the app's hatch: that call is lost, the app goes on (LLP 1075.003.000.001 D8).
                Log.e(TAG, "hatch ${if (scope == APP) "app" else word}: $e")
            }
        }
    }

    private fun element(node: Int, moment: Int, word: String, id: String, frame: RectF, data: Map<String, String>) {
        val e = elements.getOrPut(node) { ExactElement(node, this) }
        e.hatch = word; e.id = id; e.frame = frame; e.words = data
        e.isNew = moment == BUILT
        e.isLive = moment != ENDED
        if (moment == ENDED) { elements.remove(node); hatches.elementEnded(e) } else hatches.element(e)
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
        private const val BUILT = 0
        private const val ENDED = 2

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
