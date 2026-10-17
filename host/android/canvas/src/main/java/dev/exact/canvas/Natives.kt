package dev.exact.canvas

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Matrix
import android.graphics.RenderNode
import android.net.Uri
import android.view.Choreographer
import android.view.TextureView
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import org.json.JSONObject
import java.io.File

/**
 * A native module's view (LLP 1024): the app registers one per module name. [reset] readies a released view for
 * another element (LLP 1068 §4.8 reuse); a module that returns false from it is destroyed instead.
 */
interface NativeModule {
    fun create(context: Context, props: JSONObject): View
    fun update(view: View, props: JSONObject)
    /** Bound before the first update. Keep this callback only until the next binding; a retired one is ignored. */
    fun events(view: View, emit: (String, String) -> Unit) {}
    fun reset(view: View): Boolean = false
    fun destroy(view: View) {}
}

/** The app's optional module roster, compiled from `modules/android` as `class exact.app.Modules`. */
interface NativeModules {
    val modules: Map<String, NativeModule>

    companion object {
        /** Resolve the app's roster at its first module view, so apps without one load nothing. */
        fun load(): Map<String, NativeModule> = object : AbstractMap<String, NativeModule>() {
            override val entries: Set<Map.Entry<String, NativeModule>> by lazy {
                val type = try { Class.forName("exact.app.Modules") } catch (e: ClassNotFoundException) { return@lazy emptySet() }
                try {
                    (type.getDeclaredConstructor().newInstance() as NativeModules).modules.entries
                } catch (e: Exception) {
                    val cause = (e as? java.lang.reflect.InvocationTargetException)?.targetException ?: e
                    android.util.Log.e("exact", "native modules: exact.app.Modules could not be made: $cause")
                    emptySet()
                }
            }
        }
    }
}

/**
 * A `video` element's player, playing into the element's TextureView. An app with video gives the reader one
 * ([ExactActivity.video]); the reader itself carries no player, so an app without video carries none of its code.
 */
interface VideoPlayer {
    /** Plays into [view]; [size] gets the video's width, height and pixel aspect ratio as they become known, and
     * [first] is called when a source's first frame has been drawn into the view. */
    fun attach(view: TextureView, size: (Int, Int, Float) -> Unit, first: () -> Unit)
    fun source(file: File)
    fun set(loop: Boolean, muted: Boolean, playing: Boolean)
    fun release()
}

/** A video's size: width and height in pixels, and the pixel aspect ratio. */
private class VideoSize(val width: Int, val height: Int, val ratio: Float)

/**
 * The platform elements of the stream (op 28: `video`, `iframe`, a native module's view). Each is a real Android view,
 * attached here at its device size so it runs as it would anywhere (a player's TextureView, a WebView, a MapLibre
 * map); this layout draws none of them. Each view's own drawing is recorded into its element's RenderNode, which the
 * rows draw where the element is: it scrolls, clips and stacks with the content around it. A view that invalidates
 * (a video frame, a page, map tiles) has only its node recorded again, at the next frame.
 */
class Natives(
    context: Context,
    private val assets: File,
    private val modules: Map<String, NativeModule>,
    private val videos: ((Context) -> VideoPlayer)? = null,
) : FrameLayout(context) {
    private class Element(val id: Int, val kind: Int, var view: View, val node: RenderNode) {
        var props = ""
        var module: String? = null
        var generation = 0
        var slot = 0
        var eventBinding = 0
        var video: VideoSize? = null
        /** A video whose view still shows another source's last frame (a lent player's): drawn once its own first
         * frame is in the view, as a new TextureView draws nothing until then. */
        var stale = false
        /** A video's `autoplay && !paused`, and whether it plays now: only while a row of its is in the view. */
        var plays = false
        var playing = false
        var inView = false
        /** A page paused because its row left the view ([near]). */
        var paused = false
        var fit = "contain"
        var w = 0
        var h = 0
        var dirty = true
        /** A GPU canvas's window, given to the host while its TextureView has a surface. */
        var surface: android.view.Surface? = null
        /** Written on the exact thread when [Exact.attachSurface] ran there; read there by [Exact.detachSurface]. */
        @Volatile var window = 0L
        /** Rows (and 0, the unrowed stream) that draw it. */
        val rows = HashSet<Int>()
        /** A GPU canvas drawn from its own buffers ([buffered]): the size they were asked at, the frame last drawn
         * ([Native.canvasFrame]) and each buffer of the ring wrapped once, by generation and slot. */
        var buffers = false
        var asked = 0L
        var drawn = -1L
        val wrapped = HashMap<Long, android.graphics.Bitmap>()
        /** Without its view ([near]): [view] is the stand-in, and [wanted] the props its view starts from. */
        var parked = false
        var wanted = ""
        /** A row of the reader's placing has drawn it: when none of its rows is placed any more, they have left. */
        var placed = false
    }

    /** gpubuf=0: a GPU canvas presents to a TextureView's window, to compare. Otherwise it renders into hardware
     * buffers and this reader draws the newest finished one as a bitmap in its own pass, each display frame: no
     * swapchain present, no SurfaceTexture update, no second composition of the canvas. */
    private val buffered = (context as? android.app.Activity)?.intent?.getStringExtra("gpubuf") != "0"
    /** Buffers were refused (an older host library, or a device without them): canvases get windows. */
    private var refused = false
    private var polling = false
    private val src = android.graphics.Rect()
    private val dst = android.graphics.Rect()
    private val bitmapPaint = android.graphics.Paint(android.graphics.Paint.FILTER_BITMAP_FLAG)
    private val srgb = android.graphics.ColorSpace.get(android.graphics.ColorSpace.Named.SRGB)

    /** Each display frame while a buffered canvas lives: draw the ones with a new frame. */
    private fun poll() {
        if (polling) return
        polling = true
        Choreographer.getInstance().postFrameCallback {
            polling = false
            var any = false
            var live = false
            for (e in elements.values.toList()) {
                if (!e.buffers) continue
                live = true
                val f = try { Native.canvasFrame(e.id) } catch (x: UnsatisfiedLinkError) { -2L }
                if (f == -2L) { refused = true; unbuffer(e); any = true; continue }
                if (f < 0 || f == e.drawn) continue
                val key = f ushr 32
                val bmp = e.wrapped[key] ?: run {
                    // A new ring (a resize): the old one's buffers go.
                    if (e.wrapped.keys.any { it ushr 8 != key ushr 8 }) e.wrapped.clear()
                    val hb = Native.canvasBuffer(e.id) ?: return@run null
                    // The buffer may be newer than [f] by now; it is drawn all the same and named at the next frame.
                    val made = android.graphics.Bitmap.wrapHardwareBuffer(hb, srgb)
                    hb.close()
                    if (made != null && Native.canvasFrame(e.id) ushr 32 == key) e.wrapped[key] = made
                    made
                } ?: continue
                e.drawn = f
                if (e.w <= 0 || e.h <= 0) continue
                e.node.setPosition(0, 0, e.w, e.h)
                val c = e.node.beginRecording(e.w, e.h)
                src.set(0, 0, bmp.width, bmp.height); dst.set(0, 0, e.w, e.h)
                c.drawBitmap(bmp, src, dst, bitmapPaint)
                e.node.endRecording()
                any = true
            }
            if (any) frame()
            if (live) poll()
        }
    }

    /** Ask for (or, at a new size, ask again for) a buffered canvas's buffers. */
    private fun askBuffers(e: Element) {
        if (!e.buffers || e.w <= 0 || e.h <= 0) return
        val size = (e.w.toLong() shl 32) or e.h.toLong()
        if (e.asked == size) return
        e.asked = size
        Exact.attachBuffers(e.id, e.w, e.h)
        poll()
    }

    /** Buffers were refused: the canvas gets a TextureView and its window, as with gpubuf=0. */
    private fun unbuffer(e: Element) {
        e.buffers = false
        e.wrapped.clear()
        byView.remove(e.view)
        removeView(e.view)
        val tv = TextureView(context).apply { isOpaque = false }
        addView(tv, LayoutParams(1, 1))
        e.view = tv
        byView[tv] = e
        listen(e, tv)
        if (e.w > 0 && e.h > 0) {
            tv.layoutParams = LayoutParams(e.w, e.h)
            tv.measure(MeasureSpec.makeMeasureSpec(e.w, MeasureSpec.EXACTLY), MeasureSpec.makeMeasureSpec(e.h, MeasureSpec.EXACTLY))
            tv.layout(0, 0, e.w, e.h)
        }
        e.dirty = true
    }

    private fun listen(e: Element, view: TextureView) {
        view.surfaceTextureListener = object : TextureView.SurfaceTextureListener {
            override fun onSurfaceTextureAvailable(st: android.graphics.SurfaceTexture, w: Int, h: Int) = window(e, st, w, h)
            override fun onSurfaceTextureSizeChanged(st: android.graphics.SurfaceTexture, w: Int, h: Int) = window(e, st, w, h)
            override fun onSurfaceTextureDestroyed(st: android.graphics.SurfaceTexture): Boolean { unwindow(e); return true }
            override fun onSurfaceTextureUpdated(st: android.graphics.SurfaceTexture) {}
        }
    }

    private val elements = HashMap<Int, Element>()
    private val byView = HashMap<View, Element>()
    private val pool = HashMap<String, ArrayList<View>>()
    /** A released row's WebView, kept for the next (as a RecyclerView keeps its holders' views): it shows its last page
     * until the new one paints, where a new WebView is white for that long and costs its construction on this thread
     * (webpool=0: one made for each, destroyed with it). */
    private val webPool = ArrayList<WebView>()
    /** modpool=N: a module's views kept for reuse ([MODULE_POOL] by default), to compare. */
    private val modulePool = (context as? android.app.Activity)?.intent?.getStringExtra("modpool")?.toIntOrNull() ?: MODULE_POOL
    private val webs = (context as? android.app.Activity)?.intent?.getStringExtra("webpool") != "0"
    private var scheduled = false
    /** Asked to draw a frame after nodes were recorded again (no new stream). */
    var frame: () -> Unit = {}

    init {
        clipChildren = false
        // Laid out, attached and "visible" to themselves, but drawn only through their nodes.
        setWillNotDraw(true)
    }

    override fun dispatchDraw(canvas: Canvas) {}

    override fun onDescendantInvalidated(child: View, target: View) {
        val e = byView[child] ?: return
        e.dirty = true
        if (!scheduled) {
            scheduled = true
            Choreographer.getInstance().postFrameCallback {
                scheduled = false
                if (refresh()) frame()
            }
        }
    }

    /** Record every changed element's view again; whether any was. */
    fun refresh(): Boolean {
        var any = false
        for (e in elements.values) if (e.dirty && !e.buffers) { record(e); any = true }
        return any
    }

    private fun record(e: Element) {
        e.dirty = false
        if (e.w <= 0 || e.h <= 0) return
        e.node.setPosition(0, 0, e.w, e.h)
        val c = e.node.beginRecording(e.w, e.h)
        if (!e.stale) e.view.draw(c)
        e.node.endRecording()
    }

    /** The node id of the element whose view is [view] or holds it ([ExactTamber]'s place), or null (main thread). */
    fun place(view: View): Int? {
        var v: View? = view
        while (v != null && v !== this) {
            byView[v]?.let { return it.id }
            v = v.parent as? View
        }
        return null
    }

    /** The node op 28 draws for [id] (main thread). */
    fun node(id: Int): RenderNode? = elements[id]?.node

    /** A view the app's Kotlin hatches host at a node (kind 5, LLP 1115.001.000 D4), made for this layer's
     * activity: each element its own. */
    var hatched: ((Int, Context) -> View?)? = null

    /** Every element's node, for a row recorded on another thread: one map shared until an element comes or goes. */
    private var nodes: Map<Int, RenderNode>? = null
    fun snapshot(): Map<Int, RenderNode> = nodes ?: elements.mapValues { it.value.node }.also { nodes = it }

    /** Op 28 seen while ingesting a stream: make or update the element; [row] draws it (0: not in a row). */
    fun ensure(id: Int, kind: Int, w: Int, h: Int, props: String, row: Int) {
        val json = if (kind == 2) JSONObject(props) else null
        val module = json?.optString("module")
        val generation = json?.optInt("generation") ?: 0
        val slot = json?.optInt("slot") ?: 0
        var e = elements[id]
        if (e == null || e.kind != kind || (kind == 2 && (e.generation != generation || e.slot != slot || e.module != module))) {
            e?.let { release(it) }
            val created = if (lazy && row != 0 && ((kind == 0 && videos != null && lazyVideo) || kind == 1 || (kind == 2 && modules.containsKey(module)))) {
                Element(id, kind, standIn, RenderNode("exact native")).also {
                    it.parked = true; it.module = module
                    it.generation = generation; it.slot = slot
                    waiting++
                }
            } else create(id, kind, props) ?: return
            e = created
            elements[id] = e
            nodes = null
            if (!e.parked) byView[e.view] = e
        }
        e.rows.add(row)
        e.wanted = props
        if (e.w != w || e.h != h) {
            e.w = w; e.h = h
            if (!e.parked) { place(e); askBuffers(e) }
        }
        if (e.parked && row == 0) unpark(e)
        if (e.parked) return
        if (e.props != props) apply(e, props)
        if (e.dirty && !e.buffers) record(e)
    }

    private fun place(e: Element) {
        e.view.layoutParams = LayoutParams(e.w, e.h)
        e.view.measure(MeasureSpec.makeMeasureSpec(e.w, MeasureSpec.EXACTLY), MeasureSpec.makeMeasureSpec(e.h, MeasureSpec.EXACTLY))
        e.view.layout(0, 0, e.w, e.h)
        fitVideo(e)
        e.dirty = true
    }

    /**
     * A row's WebView or module view exists while the row is near the view, as a RecyclerView's holder is bound when
     * it scrolls near (lazy=0: for every row the stream keeps, which is several screens of them; a MapLibre map is
     * ~110 MB on a Pixel 10 Pro XL, and a feed of maps kept fifteen). Elsewhere the element is its node with no
     * drawing. Set by the reader once it tells where the rows are ([near]); an element outside a row always has
     * its view.
     */
    var lazy = false
    /** What a parked element holds for a view: never attached, never drawn. */
    private val standIn = View(context)
    /** Parked elements, and those that can be: whether [near] has anything to do. */
    private var waiting = 0
    private var held = 0
    val nearing: Boolean get() = waiting > 0 || held > 0

    /**
     * Where the rows are: [show] within the margin that gives a row's element its view, [keep] within the wider one
     * that lets it keep one, [known] every row the reader placed. An element none of whose rows it ever placed (a
     * nested list's) has its view; one whose rows it placed and no longer does has left. A view let go is lent on,
     * first to an element arriving in this call. [seen]: the rows in the view itself.
     */
    fun near(show: Set<Int>, keep: Set<Int>, known: Set<Int>, seen: Set<Int> = show) {
        for (e in elements.values) if (!e.placed && e.rows.any { known.contains(it) }) e.placed = true
        if (held > 0) for (e in elements.values) if (far(e, keep)) park(e)
        if (waiting > 0) lend(show, seen)
        // A video plays while a row of its is in the view, as a list's player plays the row on screen: a row near
        // but out of view is prepared and paused on its first frame.
        if (lazyVideo) for (e in elements.values) if (e.kind == 0 && !e.parked) {
            play(e, !e.placed || e.rows.contains(0) || e.rows.any { seen.contains(it) })
        }
        // A page runs (its animations, its timers' drawing) while a row of its is in the view, and from when it is
        // made until then, so it loads and paints ahead; once its row has left the view it is paused, as a page in
        // a list is not drawn while its holder is off screen.
        if (webPause) for (e in elements.values) if (e.kind == 1 && !e.parked) {
            val inView = !e.placed || e.rows.contains(0) || e.rows.any { seen.contains(it) }
            if (inView == e.inView) continue
            e.inView = inView
            if (inView) { if (e.paused) { (e.view as WebView).onResume(); e.paused = false } }
            else { (e.view as WebView).onPause(); e.paused = true }
        }
    }

    /** webpause=0: a page runs for as long as its row has its view, to compare. */
    private val webPause = (context as? android.app.Activity)?.intent?.getStringExtra("webpause") != "0"

    private fun lend(show: Set<Int>, seen: Set<Int>) {
        var any = false
        for (e in elements.values) {
            if (!e.parked || !(!e.placed || e.rows.any { show.contains(it) })) continue
            // None kept to lend: one still held by a row that has left [show] (kept only against coming back) is
            // lent on, before another is made.
            val none = when (e.kind) { 0 -> videoPool.isEmpty(); 1 -> webPool.isEmpty(); else -> pool[e.module]?.isEmpty() != false }
            if (none) {
                // Then one held by a row near but out of view (behind, or ahead of this one): a view is made only
                // when every other is in view, or this row is. Otherwise this one waits for the next call. (A
                // window that leads names rows a viewport ahead while the last still has its view half a viewport
                // behind: two maps for map rows two viewports apart, 110 MB each, and both kept after.)
                val other = { o: Element -> o !== e && !o.parked && o.kind == e.kind && o.module == e.module && o.placed && !o.rows.contains(0) }
                val lend = elements.values.firstOrNull { o -> o.kind == e.kind && o.module == e.module && far(o, show) }
                    ?: elements.values.firstOrNull { o -> other(o) && o.rows.none { seen.contains(it) } }
                if (lend != null) park(lend)
                else if (e.placed && e.rows.none { seen.contains(it) } && elements.values.any(other)) continue
            }
            unpark(e); any = true
        }
        if (any) frame()
    }

    /** The reader cannot tell where rows are: every element has its view. */
    fun everywhere() {
        lazy = false
        for (e in elements.values) if (e.kind == 0 && !e.parked) play(e, true)
        if (waiting == 0) return
        for (e in elements.values) if (e.parked) { unpark(e); if (e.kind == 0) play(e, true) }
        frame()
    }

    /** Whether [e] has a view of its own that none of its rows, all the reader's to place, is within [zone] to use. */
    private fun far(e: Element, zone: Set<Int>): Boolean =
        !e.parked && pooled(e.kind) && e.placed && !e.rows.contains(0) && e.rows.none { zone.contains(it) }

    /** The kinds whose views are kept and lent: a video's TextureView with its player, a WebView, a module's view. */
    private fun pooled(kind: Int) = (kind == 0 && lazyVideo) || kind == 1 || kind == 2

    /** lazyvideo=0: a video row has a player of its own for as long as the stream keeps the row, playing whether
     * in view or not, to compare. */
    private val lazyVideo = (context as? android.app.Activity)?.intent?.getStringExtra("lazyvideo") != "0"
    /** A video's player by its TextureView, and the source it was last given: a pair is kept and lent as one. */
    private val players = HashMap<View, VideoPlayer>()
    private val sources = HashMap<View, String>()
    private val sizes = HashMap<View, VideoSize>()
    private val videoPool = ArrayList<View>()

    private fun play(e: Element, inView: Boolean) {
        e.inView = inView
        val playing = e.plays && inView
        if (playing == e.playing) return
        e.playing = playing
        val json = JSONObject(e.props.ifEmpty { "{}" })
        players[e.view]?.set(json.optBoolean("loop"), json.optBoolean("muted"), playing)
    }

    private fun park(e: Element) {
        byView.remove(e.view)
        retire(e)
        e.view = standIn
        e.inView = false; e.paused = false
        e.parked = true; waiting++
        e.props = ""
        e.node.discardDisplayList()
    }

    private fun unpark(e: Element) {
        val view = obtain(e.kind, JSONObject(e.wanted), e.id) ?: return
        if (view.parent == null) addView(view, LayoutParams(1, 1))
        e.view = view
        e.parked = false; waiting--
        if (pooled(e.kind)) held++
        byView[view] = e
        if (e.kind == 2) bindEvents(e)
        if (e.w > 0 && e.h > 0) place(e)
        apply(e, e.wanted)
        if (e.dirty) record(e)
    }

    /** A row was freed: its elements that no other row draws go. */
    fun rowFreed(row: Int) {
        val gone = elements.values.filter { it.rows.remove(row) && it.rows.isEmpty() }
        for (e in gone) release(e)
    }

    /** The unrowed stream was replaced: elements only it drew and [seen] does not name go. */
    fun unrowed(seen: Set<Int>) {
        val gone = elements.values.filter { it.rows.contains(0) && !seen.contains(it.id) }
        for (e in gone) { e.rows.remove(0); if (e.rows.isEmpty()) release(e) }
    }

    private fun obtain(kind: Int, json: JSONObject, id: Int = 0): View? = when (kind) {
        5 -> hatched?.invoke(id, context)?.also { (it.parent as? android.view.ViewGroup)?.removeView(it) }
        3 -> if (buffered && !refused) View(context) else TextureView(context).apply { isOpaque = false }
        0 -> videoPool.removeLastOrNull() ?: TextureView(context).apply { isOpaque = false }
        1 -> webPool.removeLastOrNull()?.apply { onResume() } ?: WebView(context).apply {
            settings.javaScriptEnabled = true
            settings.allowFileAccess = true
            isVerticalScrollBarEnabled = false; isHorizontalScrollBarEnabled = false
            overScrollMode = View.OVER_SCROLL_NEVER
            setBackgroundColor(Color.WHITE)
        }
        else -> {
            val name = json.optString("module")
            val module = modules[name]
            if (module == null) { android.util.Log.w("exact", "no native module $name"); null }
            else pool[name]?.removeLastOrNull() ?: module.create(context, moduleProps(json))
        }
    }

    private fun create(id: Int, kind: Int, props: String): Element? {
        val json = JSONObject(props)
        val view = obtain(kind, json, id) ?: return null
        if (view.parent == null) addView(view, LayoutParams(1, 1))
        val e = Element(id, kind, view, RenderNode("exact native"))
        if (kind == 2) {
            e.module = json.optString("module")
            e.generation = json.optInt("generation")
            e.slot = json.optInt("slot")
            bindEvents(e)
        }
        if (pooled(kind)) held++
        if (kind == 3) { if (view is TextureView) listen(e, view) else e.buffers = true }
        return e
    }

    private fun bindEvents(e: Element) {
        val binding = ++e.eventBinding
        modules[e.module]?.events(e.view) { kind, payload ->
            // Modules may reply asynchronously; retirement or reuse must never retarget that reply.
            post {
                if (elements[e.id] === e && !e.parked && e.eventBinding == binding) {
                    Exact.moduleEvent(e.id, e.slot, e.generation, kind, payload)
                }
            }
        }
    }

    /** A module button under an unscrolled tap. Other native input stays with its dedicated host. */
    fun click(id: Int, x: Float, y: Float): Boolean {
        val e = elements[id]?.takeIf { it.kind == 2 && !it.parked } ?: return false
        if (!e.view.isEnabled || !e.view.isClickable) return false
        e.view.translationX = x
        e.view.translationY = y
        return e.view.performClick()
    }

    private fun moduleProps(json: JSONObject): JSONObject =
        json.optString("props").takeIf { it.isNotEmpty() }?.let { runCatching { JSONObject(it) }.getOrNull() } ?: JSONObject()

    private fun file(src: String): File = if (src.startsWith("/")) File(src) else File(assets, src)

    private fun apply(e: Element, props: String) {
        val old = e.props.takeIf { it.isNotEmpty() }?.let { JSONObject(it) }
        val json = JSONObject(props)
        e.props = props
        when (e.kind) {
            0 -> {
                val src = json.optString("src")
                e.fit = json.optString("fit", "contain")
                val view = e.view
                val p = players[view] ?: (videos ?: run { android.util.Log.w("exact", "no video player"); return }).invoke(context).also { p ->
                    players[view] = p
                    // The element is whichever holds the view when the player says so: a pair is lent on.
                    p.attach(view as TextureView,
                        { w, h, ratio -> sizes[view] = VideoSize(w, h, ratio); byView[view]?.let { it.video = sizes[view]; fitVideo(it) } },
                        { byView[view]?.let { if (it.stale) { it.stale = false; it.dirty = true; if (refresh()) frame() } } })
                }
                if (sources[view] != src) {
                    // A lent view holds the last source's frame until this one's first.
                    if (sources.containsKey(view)) e.stale = true
                    sources[view] = src
                    p.source(file(src))
                }
                // The size the view's player last told (it tells again only when the next source's differs).
                if (e.video !== sizes[view]) { e.video = sizes[view]; fitVideo(e) }
                e.plays = json.optBoolean("autoplay") && !json.optBoolean("paused")
                // Until the reader says where its row is, a lazy video is prepared and paused ([near] plays it).
                e.playing = e.plays && (e.inView || !lazyVideo || !lazy || e.rows.contains(0))
                p.set(json.optBoolean("loop"), json.optBoolean("muted"), e.playing)
            }
            1 -> {
                val src = json.optString("src")
                if (old?.optString("src") != src) {
                    val web = e.view as WebView
                    if (src.startsWith("http:") || src.startsWith("https:")) web.loadUrl(src) else web.loadUrl(Uri.fromFile(file(src)).toString())
                }
            }
            // A hatch gave the node another view.
            5 -> if (old != null && old.optInt("gen") != json.optInt("gen")) {
                val view = obtain(5, json, e.id) ?: return
                byView.remove(e.view); removeView(e.view)
                e.view = view
                if (view.parent == null) addView(view, LayoutParams(1, 1))
                byView[view] = e
                if (e.w > 0 && e.h > 0) place(e)
            }
            else -> modules[e.module]?.update(e.view, moduleProps(json))
        }
        e.dirty = true
    }

    /** A GPU canvas's TextureView has a surface (again, or at a new size): the host presents into it. */
    private fun window(e: Element, st: android.graphics.SurfaceTexture, w: Int, h: Int) {
        unwindow(e)
        st.setDefaultBufferSize(w, h)
        val surface = android.view.Surface(st)
        e.surface = surface
        // Not waited for: exact2's thread may be busy (the GPU module's first frame, right after the first content),
        // and the main thread would wait out that work in its first draw.
        Exact.attachSurface(e.id, surface, w, h) { e.window = it }
    }

    private fun unwindow(e: Element) {
        // Waited for: after the attach posted before it, and before the surface goes.
        if (e.surface != null) Exact.detachSurface(e.id) { e.window.also { e.window = 0L } }
        e.surface?.release()
        e.surface = null
    }

    /** `object-fit` on the TextureView, which otherwise stretches the frame to its size. */
    private fun fitVideo(e: Element) {
        val size = e.video ?: return
        val tv = e.view as? TextureView ?: return
        if (e.w <= 0 || e.h <= 0 || size.width <= 0 || size.height <= 0) return
        val vw = size.width * size.ratio; val vh = size.height.toFloat()
        val sx = e.w / vw; val sy = e.h / vh
        val s = when (e.fit) { "cover" -> maxOf(sx, sy); "fill" -> 0f; "none" -> 1f; "scale-down" -> minOf(sx, sy, 1f); else -> minOf(sx, sy) }
        val m = Matrix()
        if (s > 0f) m.setScale(vw * s / e.w, vh * s / e.h, e.w / 2f, e.h / 2f)
        tv.setTransform(m)
        e.dirty = true
    }

    private fun release(e: Element) {
        elements.remove(e.id)
        nodes = null
        if (e.parked) { waiting--; e.node.discardDisplayList(); return }
        byView.remove(e.view)
        if (e.kind == 3) { if (e.buffers) { if (e.asked != 0L) Exact.detachBuffers(e.id); e.wrapped.clear() } else unwindow(e) }
        retire(e)
        e.node.discardDisplayList()
    }

    /** [e]'s view is kept for the next element of its kind, or goes. */
    private fun retire(e: Element) {
        e.eventBinding++
        if (pooled(e.kind)) held--
        val module = e.module?.let { modules[it] }
        if (e.kind == 0) {
            val player = players[e.view]
            if (player != null && lazyVideo && videoPool.size < VIDEO_POOL) {
                // Kept with its player, paused: the next video row's, prepared again only for another source.
                if (e.playing) player.set(false, true, false)
                videoPool.add(e.view)
            } else {
                players.remove(e.view)?.release()
                sources.remove(e.view); sizes.remove(e.view)
                removeView(e.view)
            }
            e.playing = false; e.inView = false; e.stale = false; e.video = null
        } else if (e.kind == 2 && module != null && (pool[e.module!!]?.size ?: 0) < modulePool && module.reset(e.view)) {
            pool.getOrPut(e.module!!) { ArrayList() }.add(e.view)
        } else if (e.kind == 1 && webPool.size < WEB_POOL && webs) {
            webPool.add((e.view as WebView).apply { onPause() })
        } else {
            module?.destroy(e.view)
            (e.view as? WebView)?.destroy()
            removeView(e.view)
        }
    }

    fun destroyAll() {
        for (e in elements.values.toList()) release(e)
        for (w in webPool) { w.destroy(); removeView(w) }
        webPool.clear()
        for (v in videoPool) { players.remove(v)?.release(); removeView(v) }
        videoPool.clear(); sources.clear(); sizes.clear()
        for ((name, views) in pool) for (v in views) { modules[name]?.destroy(v); removeView(v) }
        pool.clear()
    }
}

/** A module's views kept for reuse: what a row leaving lends the row arriving. A kept view holds all a shown one does
 * (a MapLibre map ~135 MB on a Pixel 10 Pro XL), so a feed of them must not keep one for every row that ever left. */
private const val MODULE_POOL = 2

/** Video views kept with their players for reuse: the one a row leaving lends the row arriving, and one more for
 * two video rows near at once (a Media3 player holds a decoder and its threads; making one costs ~40 ms here). */
private const val VIDEO_POOL = 2

/** WebViews kept for reuse: a screen and a half of webview rows. */
private const val WEB_POOL = 3
