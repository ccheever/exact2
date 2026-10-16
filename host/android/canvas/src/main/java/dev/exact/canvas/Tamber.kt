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

    /** Call [listener] on the main thread whenever what this view's colours read moves; closing the result stops it. */
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
        take(at, asked, JSONObject(Exact.tamber(request(at, asked))))
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
            take(at, asked, JSONObject(Native.tamber(request(at, asked))))
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

    private fun request(at: Int, asked: List<String>) = JSONObject().put("view", at).put("names", JSONArray(asked)).toString()

    /** [answer] for [asked] at [at], kept. A refused place (an element gone) answers every name with its fallback until
     * the view's place or its inputs move. */
    private fun take(at: Int, asked: List<String>, answer: JSONObject) {
        val c = answer.optJSONObject("colors")
        val read = HashMap<String, Int?>()
        for (n in asked) read[n] = if (c == null || c.isNull(n)) null else c.getInt(n)
        synchronized(lock) {
            id = at
            colors = read
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
