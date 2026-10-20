package dev.exact.canvas

import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.view.View
import org.json.JSONArray
import org.json.JSONObject
import java.lang.ref.WeakReference
import java.util.concurrent.CopyOnWriteArrayList

/**
 * A view's design tokens, by name (LLP 1121 D12, stage 5): `ExactTamber(view).color("surface.card")` is the colour
 * exact2 paints that token in at [view]'s place, as an Android `Color` int, and [observe] says when it may have moved.
 *
 * The place is the native module element whose view is [view] or holds it ([Natives.place]): its node, under the
 * `-exact-mode` pins and the `color-scheme` above it. A view outside any (a screen of the host's own) reads the
 * window's, the root's. A name the design lacks answers the caller's fallback, and exact2 says so once in its journal.
 *
 * Main thread. Answers are kept: exact2 is asked (and waited for, on its thread) only for a name or a place not asked
 * before. After each stream is ingested the main thread asks again where the view was lent to another element. After
 * each turn exact2's thread compares the place's inputs (a `setMode`, a pin above it, a live batch, the
 * scheme, the contrast, a role's colour) and, when they moved, asks again for every name kept and then calls the
 * observers, so a [color] in `onDraw` that follows reads the new answer without waiting.
 */
class ExactTamber(private val view: View) {
    private val lock = Any()
    /** The place last asked for (0 the root, -1 none yet), the names asked, their answers, and the inputs they were
     * read under (null: not known). */
    private var id = -1
    private val names = LinkedHashSet<String>()
    private var colors: Map<String, Int?> = emptyMap()
    /** The lengths and numbers asked (stage 6 G), and their answers. */
    private val lengthNames = LinkedHashSet<String>()
    private val numberNames = LinkedHashSet<String>()
    private var lengths: Map<String, Float?> = emptyMap()
    private var numbers: Map<String, Double?> = emptyMap()
    private var inputs: String? = null
    private val listeners = CopyOnWriteArrayList<() -> Unit>()

    /** Colour token [name] at this view's place, ARGB; [fallback] for a name the design lacks or a value that is no
     * colour. */
    @JvmOverloads
    fun color(name: String, fallback: Int = Color.TRANSPARENT): Int {
        val at = place()
        synchronized(lock) {
            if (at == id && colors.containsKey(name)) return colors[name] ?: fallback
            names.add(name)
        }
        ask(at)
        synchronized(lock) { return colors[name] ?: fallback }
    }

    /** Length token [name] at this view's place, in points (dp: a `rem` by the person's text size, a text style at
     * it); [fallback] for a name the design lacks (stage 6 G). A value: read it again in [observe]. */
    @JvmOverloads
    fun length(name: String, fallback: Float = 0f): Float {
        val at = place()
        synchronized(lock) {
            if (at == id && lengths.containsKey(name)) return lengths[name] ?: fallback
            lengthNames.add(name)
        }
        ask(at)
        synchronized(lock) { return lengths[name] ?: fallback }
    }

    /** Number token [name] at this view's place; [fallback] for a name the design lacks (stage 6 G). */
    @JvmOverloads
    fun number(name: String, fallback: Double = 0.0): Double {
        val at = place()
        synchronized(lock) {
            if (at == id && numbers.containsKey(name)) return numbers[name] ?: fallback
            numberNames.add(name)
        }
        ask(at)
        synchronized(lock) { return numbers[name] ?: fallback }
    }

    /** Call [listener] on the main thread whenever what this view's tokens read moves (a mode, a pin, a live batch,
     * the scheme, the person's text size, Bold Text); closing the result stops it. */
    fun observe(listener: () -> Unit): AutoCloseable {
        listeners.add(listener)
        val known = synchronized(lock) { inputs != null }
        if (!known) ask(place())
        return AutoCloseable { listeners.remove(listener) }
    }

    /** The node id of this view's element, else 0: the root. */
    private fun place(): Int {
        var v: View? = view
        while (v != null) {
            val parent = v.parent
            if (parent is Natives) return parent.place(view) ?: 0
            v = parent as? View
        }
        return 0
    }

    /** Ask exact2 for every name at [at], waiting for it (main thread). */
    private fun ask(at: Int) {
        Tambers.watch(this)
        val asked = synchronized(lock) { names.toList() }
        take(at, asked, JSONObject(Exact.tamber(request(at, asked, true))))
    }

    /** exact2's thread, after a turn: whether the place's inputs moved; if so its names are asked again and the
     * observers told. */
    internal fun check() {
        // The place last asked: the view map is the main thread's, and a stream not yet ingested may lend the view
        // to another element ([placed] asks there once it is).
        val (at, was) = synchronized(lock) { if (id < 0) return else id to inputs }
        try {
            // The inputs alone first: they cost no colour, and mostly nothing moved.
            val now = JSONObject(Native.tamber(request(at, emptyList()))).optString("inputs")
            if (now.isEmpty() || now == was) return
            val asked = synchronized(lock) { names.toList() }
            take(at, asked, JSONObject(Native.tamber(request(at, asked, true))))
        } catch (e: UnsatisfiedLinkError) { return }
        // Its view drawn again, so an `onDraw` reading [color] takes the new answer.
        if (was != null) Tambers.main.post { view.invalidate(); for (l in listeners) l() }
    }

    /** The main thread, after a stream was ingested: a view now another element's (a pin above it) is asked there,
     * and its observers told. */
    internal fun placed() {
        val at = place()
        if (synchronized(lock) { id < 0 || id == at }) return
        ask(at)
        view.invalidate()
        for (l in listeners) l()
    }

    private fun request(at: Int, asked: List<String>, measures: Boolean = false): String {
        val r = JSONObject().put("view", at).put("names", JSONArray(asked))
        if (measures) synchronized(lock) {
            r.put("lengths", JSONArray(lengthNames.toList())).put("numbers", JSONArray(numberNames.toList()))
        }
        return r.toString()
    }

    /** [answer] for [asked] at [at], kept. A refused place (an element gone) answers every name with its fallback until
     * the view's place or its inputs move. */
    private fun take(at: Int, asked: List<String>, answer: JSONObject) {
        val c = answer.optJSONObject("colors")
        val read = HashMap<String, Int?>()
        for (n in asked) read[n] = if (c == null || c.isNull(n)) null else c.getInt(n)
        val l = answer.optJSONObject("lengths")
        val n = answer.optJSONObject("numbers")
        val readLengths = HashMap<String, Float?>()
        val readNumbers = HashMap<String, Double?>()
        synchronized(lock) {
            for (k in lengthNames) readLengths[k] = if (l == null || l.isNull(k)) null else l.getDouble(k).toFloat()
            for (k in numberNames) readNumbers[k] = if (n == null || n.isNull(k)) null else n.getDouble(k)
        }
        synchronized(lock) {
            id = at
            colors = read
            lengths = readLengths
            numbers = readNumbers
            inputs = if (answer.has("error")) null else answer.optString("inputs")
        }
    }
}

/** The [ExactTamber]s in use, held weakly: checked on exact2's thread after each of its turns ([Owner], or [ExactView]
 * where exact2 runs on the main thread). */
internal object Tambers {
    private val all = CopyOnWriteArrayList<WeakReference<ExactTamber>>()
    val main = Handler(Looper.getMainLooper())

    fun watch(t: ExactTamber) {
        if (all.none { it.get() === t }) all.add(WeakReference(t))
    }

    /** exact2's thread, after a turn: each binding whose inputs moved asks again and tells its observers. */
    fun turned() {
        if (all.isEmpty()) return
        for (r in all) {
            val t = r.get()
            if (t == null) all.remove(r) else t.check()
        }
    }

    /** The main thread, after a stream was ingested: each binding whose view moved to another element asks there. */
    fun placed() {
        for (r in all) r.get()?.placed()
    }
}
