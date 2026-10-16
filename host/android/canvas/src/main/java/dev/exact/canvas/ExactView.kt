package dev.exact.canvas

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.RectF
import android.graphics.fonts.Font
import android.os.Looper
import android.os.MessageQueue
import android.os.ParcelFileDescriptor
import android.view.MotionEvent
import android.view.View
import java.io.File
import java.nio.ByteOrder
import java.nio.IntBuffer

/**
 * The heavy list as exact2 lays it out and Android's Canvas draws it (LLP 1076 §3.3). exact2 runs on the main thread
 * and hands over an op stream (host/linux/src/canvas.rs); rows are RenderNodes, recorded on a worker thread (async=0:
 * on the main thread), kept until exact2 frees them; a scroller's rows are one node that a scroll frame exact2 does
 * not paint only translates.
 */
class ExactView(context: Context, private val live: Boolean, private val onFirst: () -> Unit) : View(context) {
    /** Keys and text input reach the app (the activity's [ExactActivity.input]); see [Keys]. */
    var keys: Keys? = null
    /** The stream's platform elements (op 28). */
    var natives: Natives? = null
    /** The app's text fields, each the platform's own ([Fields], LLP 1115.001 D8): op 28, kind 4, draws one. */
    internal var fields: Fields? = null
    /** The app's bars, over this view ([Bars], LLP 1115.001 D11): told with the stream they belong to. */
    internal var bars: Bars? = null
    /** What exact2 tells of the accessibility tree ([Access], LLP 1115.001 D13), handed on with the stream it came
     * with; and, where exact2 runs on this thread (owner=0), whether a service is on and it is asked at all. */
    internal var access: ((IntArray) -> Unit)? = null
    private var accessible = false
    private var accessAgain = false
    internal fun accessible(on: Boolean) {
        if (on) { accessible = true; accessAgain = true; invalidate() } else if (accessible) { accessible = false; Native.accessOff() }
    }
    /** The row whose body [ingest] is in (0: none), and the elements the unrowed stream names. */
    private val flatAlpha = (context as Activity).intent.getStringExtra("flatalpha") != "0"
    private var ingestRow = 0
    /** Where the last stream drew each row (its top, the stream's pixels) and in which scroller's group (0: none),
     * for [rowShows]. */
    private val rowTop = android.util.SparseIntArray()
    private val rowGroup = android.util.SparseIntArray()
    private var ingestGroup = 0
    /** offscreen=0: a layer's dash is recorded each frame wherever its row is, to compare ([Layers.rowShows]). */
    private val skipOffscreen = (context as Activity).intent.getStringExtra("offscreen") != "0"

    /** Each kept row's recorded height (px). */
    private val rowTall = android.util.SparseIntArray()

    /** Whether [row] is in the view or within a third of it (a frame of the fastest fling), where the last stream and
     * its scroller's move since put it (true when the stream did not draw it: unknown). */
    private fun rowShows(row: Int): Boolean {
        // Sparse arrays of ints (a top as its float's bits): a stream names every row it draws, and a HashMap boxed
        // each key and value (two entries a row a stream).
        val k = rowTop.indexOfKey(row)
        if (k < 0) return true
        val y = java.lang.Float.intBitsToFloat(rowTop.valueAt(k))
        val at = y + (groups[rowGroup.get(row, 0)]?.translationY ?: 0f)
        val margin = height / 3f
        return at + rowTall.get(row, height) > -margin && at < height + margin
    }
    private val unrowedSeen = HashSet<Int>()
    private var started = false
    private var ops: IntBuffer? = null
    private var first = true
    private val fonts = java.util.concurrent.ConcurrentHashMap<Int, Font>()
    private val images = HashMap<Int, Bitmap>()
    private val animated = HashMap<Int, android.graphics.drawable.AnimatedImageDrawable>()
    /** Kept rows, one RenderNode each, recorded once and drawn until freed. */
    private val rows = HashMap<Int, android.graphics.RenderNode>()
    /** Rows the last stream freed: dropped when the next stream arrives, as the last one may be replayed again. */
    private val freeing = ArrayList<Int>()
    /** Rows being recorded on the worker, and the row each replaces (drawn meanwhile; 0 for none). */
    private val pending = HashMap<Int, Int>()
    /** Rows freed while being recorded, and replaced rows freed while still drawn for one being recorded. */
    private val cancelled = HashSet<Int>()
    private val stale = HashSet<Int>()
    /** Each scroller's rows, one RenderNode, moved (translated) on frames exact2 did not paint. */
    private val groups = HashMap<Int, android.graphics.RenderNode>()
    /** Each image node's picture slot, and the slots each picture is drawn in. */
    private val slots = HashMap<Int, android.graphics.RenderNode>()
    /** The copies of [images], [animated] and [slots] a row recorded on the worker reads: shared by the rows a stream
     * records until the map changes (a fast fling records many rows a frame; a copy each cost the main thread ~5%). */
    private var picturesCopy: Map<Int, Bitmap>? = null
    private var playingCopy: Map<Int, android.graphics.drawable.AnimatedImageDrawable>? = null
    private var heldCopy: Map<Int, android.graphics.RenderNode>? = null
    private val drawnIn = HashMap<Int, HashSet<Int>>()
    /** Nodes exact2's reader animates, sampled every frame while any plays. */
    private val layers: Layers = Layers({ node, ops, ox, oy, memo ->
        // A layer whose dash phase moved: only its path is recorded again, read once.
        val c = node.beginRecording()
        main.memo = memo
        try { main.replayAt(c, IntBuffer.wrap(ops), 0, ops.size, ox, oy) } finally { main.memo = null }
        node.endRecording()
    }) { fonts[it] }.also { if (skipOffscreen) it.rowShows = Layers.RowShows { row -> rowShows(row) } }
    /** layerbounds=0: layers' nodes cover everything, unclipped (each move or fade damages the window), to compare. */
    private val layerBounds = (context as Activity).intent.getStringExtra("layerbounds") != "0"
    private val bounds = Bounds().also { it.row = { id -> rows[id] } }
    private val measured = RectF()
    private val placed = android.graphics.Rect()
    private val main: Player = Player(fonts).also { p ->
        p.dotsAtOnce = (context as Activity).intent.getStringExtra("dots") != "0"
        p.layer = { layers.lookup(it) }
        p.image = { images[it] }
        p.animated = { animated[it] }
        p.native = { natives?.node(it) ?: fields?.node(it) }
        p.row = { id -> rows[id] ?: pending[id]?.let { rows[it] } }
        p.group = { id -> groups.getOrPut(id) { android.graphics.RenderNode("exact group").apply { setClipToBounds(false) } } }
        p.holder = { id -> scrolling.holder(id) }
        p.slot = { slots[it] }
    }
    private val background = Player(fonts)
    private val worker = java.util.concurrent.Executors.newSingleThreadExecutor()
    private val async = (context as Activity).intent.getStringExtra("async") != "0"
    /** Timers, and the paint a scroll owes once it pauses (only then the collection pass it left behind). */
    private val timer = Runnable { if (prefetch && Native.owed()) Native.refine(-1, 0f); pump() }
    /** step or pump took this frame's stream: onDraw replays it without asking again. */
    private var taken = false
    /** The last take was a move: no new drawing, only the groups' translations. */
    private var moved = false
    /** exact2 is booting on a thread of its own ([Native.startAsync] or [Owner.boot]); false: it boots here. */
    var booting = false
    /** The window's size in pixels, which a host that boots here boots in ([Native.start]). */
    var window = 0 to 0
    /** What the host says to draw around the page ([Native.chrome]), when exact2 runs here: told as it changes. */
    var chrome: (IntArray) -> Unit = {}
    /** What exact2 asks of the platform ([Native.requests], LLP 1115.001 D5), when it runs here: taken after each
     * turn and whenever it wakes this thread ([asking]). With an [owner], the owner takes them. */
    internal var asked: (IntArray) -> Unit = {}
    private fun asking() { Native.requests()?.let(asked) }
    /** exact2's own thread, when it has one ([Owner]); null: exact2 runs here, on the main thread. */
    internal var owner: Owner? = null
    /** Scrolls sent to the owner that no published stream includes yet, and the feed's group with its translation in
     * the last stream: the group is drawn that far moved, less what the owner has yet to apply. */
    private var mainSeq = 0
    private val sent = ArrayDeque<Pair<Int, Float>>()
    private var feed = 0
    private var feedBaseX = 0f
    private var feedBaseY = 0f
    /** A 1x1 view whose invalidation draws a frame without re-recording this one (a move). */
    var ticker: View? = null

    /** retain=0: every new stream draws the whole view again (one display list: HWUI damages the window), to compare.
     * Otherwise a stream whose top level is the last one's ([Structure]) records only what changed in place: kept rows
     * again into their predecessor's node, the groups whose rows changed; the view's own drawing stands, and HWUI's
     * damage is those nodes. */
    private val retain = (context as Activity).intent.getStringExtra("retain") != "0"
    private val structure = Structure()
    private var rootSig: IntArray? = null
    private val groupSigs = HashMap<Int, IntArray>()
    /** Each kept row's node, named (a row recorded again in place keeps its predecessor's name). */
    private val tokens = HashMap<Int, Int>()
    private var nextToken = 1
    /** The last stream applied changed only nodes the view's drawing references: no need to draw the view again. */
    private var quiet = false

    /** The view's size is never told to exact2 (LLP 1115.001 D3.3): the activity tells it the window and its insets,
     * the presenter places the page in it, and the view is laid out there ([ExactActivity.chrome]). */
    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        main.width = w; main.height = h
        if (owner != null || started || w == 0 || h == 0) return
        if (booting) {
            // exact2 booted (and painted its first frame) while the activity was set up.
            android.os.Trace.beginSection("exact adopt")
            started = Native.adopt()
            android.os.Trace.endSection()
        } else {
            val dir = context.filesDir.absolutePath
            try { Native.hardwarePictures(hwbuf) } catch (e: UnsatisfiedLinkError) { }
            started = Native.start(window.first, window.second, resources.displayMetrics.density, dir, live)
        }
        // Executor replies and decoded images wake the main looper; poll drains them.
        val queue = Looper.getMainLooper().queue
        for (fd in Native.fds()) {
            if (fd < 0) continue
            val pfd = ParcelFileDescriptor.fromFd(fd)
            wakes.add(pfd)
            queue.addOnFileDescriptorEventListener(pfd.fileDescriptor, MessageQueue.OnFileDescriptorEventListener.EVENT_INPUT) { _, _ ->
                val turn = Native.poll()
                asking()
                if (turn) pump()
                MessageQueue.OnFileDescriptorEventListener.EVENT_INPUT
            }
        }
        invalidate()
    }

    /** The host's wake descriptors this view watches on the main looper (owner=0), until it leaves its window: a view
     * made after it (the activity made again) watches its own, and this one must not take that one's streams. */
    private val wakes = ArrayList<ParcelFileDescriptor>()

    /** Whether the activity is started. Stopped, the view asks for no frame (LLP 1115.001 D15.2): no turn at a vsync,
     * no timer, no layer sampled; the first frame after it starts again shows what changed meanwhile. */
    private var running = true

    /** The activity stopped ([on] false) or started again. The host is told only by the view that reads it
     * ([host]): the view of an activity that was replaced stops itself and leaves the host to its successor. */
    fun running(on: Boolean, host: Boolean = true) {
        // The owner first, whatever this view believes: it may have been stopped under the view before this one.
        if (host) owner?.running(on)
        if (on == running) return
        running = on
        if (on) {
            if (started) { playLayers(); if (owner == null) pump() }
            return
        }
        val frames = android.view.Choreographer.getInstance()
        scrolling.released()
        removeCallbacks(held)
        frames.removeFrameCallback(turn); turnPosted = false
        frames.removeFrameCallback(sampler); samplerPosted = false
        frames.removeFrameCallback(holder); holdFrom = 0L
        removeCallbacks(timer); removeCallbacks(refiner)
    }

    /** A finger's scroll, its fling and its ends ([Scrolling], LLP 1115.001 D7). */
    private val scrolling = Scrolling(this)

    /** The touch that is down becomes a long press once it has been held still for the system's own time
     * ([android.view.ViewConfiguration.getLongPressTimeout], which follows the person's touch and hold delay):
     * exact2 is told so (action 4), and what names a context menu under it opens it (LLP 1115.001 D10). A touch
     * that moves past the system's slop, scrolls, lifts or is a hosted field's is none. */
    private val slop = android.view.ViewConfiguration.get(context).scaledTouchSlop
    private var heldX = 0f
    private var heldY = 0f
    private var heldTool = 0
    private val held = Runnable { Exact.touched = android.os.SystemClock.uptimeMillis(); forward(4, heldX, heldY, heldTool, -1L) }
    private fun hold(e: MotionEvent?) {
        removeCallbacks(held)
        if (e == null) return
        heldX = e.x; heldY = e.y; heldTool = e.getToolType(0)
        postDelayed(held, android.view.ViewConfiguration.getLongPressTimeout().toLong())
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(e: MotionEvent): Boolean {
        // A touch that scrolls is the reader's and exact2 hears no more of it; any other goes on to exact2.
        if (started && scrolling.touch(e)) { hold(null); return true }
        val down = e.actionMasked == MotionEvent.ACTION_DOWN
        // A touch in a hosted text field's box is held for the field, and is the field's own once exact2 says the
        // field is what is under it ([Fields], LLP 1115.001 D8): exact2 hears of the focus, not of the touch.
        val hosted = fields
        if (started && hosted != null && hosted.touch(e)) {
            hold(null)
            if (down) { val touch = hosted.serial; under(e.x, e.y) { hosted.under(touch, it.getOrElse(2) { 0 }) } }
            return true
        }
        val action = when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> 0; MotionEvent.ACTION_UP -> 1; MotionEvent.ACTION_MOVE -> 2; else -> 3
        }
        when (action) {
            0 -> if (started) hold(e)
            2 -> if (Math.abs(e.x - heldX) > slop || Math.abs(e.y - heldY) > slop) hold(null)
            else -> hold(null)
        }
        if (action == 1) Exact.touched = e.eventTime
        forward(action, e.x, e.y, e.getToolType(0), -1L)
        if (down && started) under(e.x, e.y) {}
        return true
    }

    /** What is under a touch's down ([Native.under]), asked behind the down itself: by the time the finger passes the
     * slop the reader knows which scrollers it could move and which axes exact2's own gestures claim. [then]: with
     * the answer, after the scrolling has it. */
    private fun under(x: Float, y: Float, then: (IntArray) -> Unit) {
        val serial = scrolling.serial
        val o = owner
        if (o != null) o.under(x, y) { scrolling.under(serial, it); then(it) }
        else { val u = Native.under(x, y); scrolling.under(serial, u); then(u) }
    }

    /** Whether the touch that is down is a hosted field's that asked to keep it ([Fields.keeps]): no scroll takes it. */
    internal fun fieldKeeps() = fields?.keeps == true

    /** exact2 asks the software keyboard ([Requests], kind 6): hidden ([state] 0), or shown for an input (1) or a
     * textarea (3). For a hosted field ([field], LLP 1115.001 D8) the field takes the focus and shows its own; for
     * one exact2 paints, this view's ([Keys]). */
    internal fun keyboard(state: Int, field: Int) {
        val hosted = fields
        if (state != 0 && field != 0 && hosted != null) { keys?.show(0); hosted.focus(field) }
        else { hosted?.blur(state == 0); keys?.show(state) }
    }

    /** exact2 runs here (owner=0) and was told something of a field: what that changed is drawn. */
    internal fun turned() = pump()

    /** A touch to exact2 (0 down, 1 up, 2 move, 3 cancel) at the view's pixels. [wait]: the ms the frame waits for
     * exact2's answer, or negative for the action's own. [Scrolling] cancels a touch its scroll won with this, and
     * waits for nothing. */
    internal fun forward(action: Int, x: Float, y: Float, tool: Int, wait: Long) {
        // A scroll that takes a hosted field's touch cancels it in the field: exact2 never heard of it.
        if (action == 3 && fields?.cancel() == true) return
        val o = owner
        // A press or release waits for exact2's answer within this frame, as a click handler holds the main thread; a
        // drag's moves barely wait, so the drag keeps its frames.
        if (o != null) {
            if (!started) return
            // A press wakes the GPU ([Warm]) and does not wait for exact2's answer (what a press shows, if anything,
            // comes with the next frame): no frame is held by the wait, and the release's response finds the GPU
            // awake (warm=0: the press waits, as a release does).
            val press = action == 0 && warm
            if (action != 2) o.input()
            if (press) {
                warmer.poke(); holdFrom = System.nanoTime(); holdFrames = 0
                android.view.Choreographer.getInstance().removeFrameCallback(holder); hold()
                o.held(true)
            }
            if (action == 1 || action == 3) holdFrom = 0L
            val answered = o.touch(action, x, y, tool, if (wait >= 0) wait else if (press) 0L else if (action == 2) 2L else releaseWait)
            if (action == 1 || action == 3) o.held(false)
            if (answered) publish()
            return
        }
        Native.touch(action, x, y, tool)
        // A frame only when the touch changed what exact2 draws (a down on a plain row changes nothing).
        if (started) pump()
    }

    /** The reader moved scroller [id] by ([dx], [dy]) px ([Scrolling]): exact2 is told, on its thread or here. The
     * move's number among those sent to exact2's thread (each stream names the last it includes), or 0 where exact2
     * runs here and has taken it. */
    internal fun scrolled(id: Int, dx: Float, dy: Float, rev: Int): Int {
        if (dy != 0f) travelled(dy)
        val o = owner
        if (o == null) {
            Native.scrolled(id, dx, dy, rev)
            Native.scrollers()?.let { scrolling.told(it) }
            step()
            return 0
        }
        val s = ++mainSeq
        o.scrolled(id, dx, dy, rev, s)
        return s
    }

    /** A scroller with no node of its own moved: this frame waits a moment for exact2's paint of it, as it does for
     * a drag's moves, so the content keeps up with the finger. */
    internal fun awaitScrolled() {
        val o = owner ?: return
        if (o.answer(2L) {}) publish()
    }

    /** Scroller [id]'s rows' node, once a stream drew them. */
    internal fun group(id: Int): android.graphics.RenderNode? = groups[id]
    /** A frame that draws what moved, with nothing recorded again. */
    internal fun tick() { ticker?.invalidate() }
    /** The view's own drawing is recorded again from the last stream (what it names changed). */
    internal fun again() { rootSig = null; invalidate() }

    override fun onKeyDown(keyCode: Int, event: android.view.KeyEvent): Boolean =
        keys?.down(event)?.also { if (it) pump() } ?: super.onKeyDown(keyCode, event)

    override fun onKeyMultiple(keyCode: Int, repeatCount: Int, event: android.view.KeyEvent): Boolean =
        keys?.multiple(event)?.also { if (it) pump() } ?: super.onKeyMultiple(keyCode, repeatCount, event)

    override fun onCheckIsTextEditor(): Boolean = keys?.editing() ?: false

    override fun onCreateInputConnection(info: android.view.inputmethod.EditorInfo): android.view.inputmethod.InputConnection? =
        keys?.connection(info)

    override fun onDraw(canvas: Canvas) {
        if (!started) return
        val owned = owner != null
        if (owned) takeOwned() else if (taken) taken = false else take()
        android.os.Trace.beginSection("exact replay")
        ops?.let { main.replay(canvas, it, 0, it.limit()) }
        android.os.Trace.endSection()
        // First content is this frame: its mark goes before any turn this frame schedules.
        // Asynchronous: a traversal this frame schedules (rows made on the worker draw at the next) holds posted
        // messages behind its barrier, and the mark then waited for that frame, which waits for this one's on
        // RenderThread (30-45 ms, the first with text): 82 ms where the same launch otherwise marked 47.
        if (first && ops != null) { first = false; android.os.Handler.createAsync(android.os.Looper.getMainLooper()).post(onFirst); if (warm) postDelayed({ warmer.prepare() }, 300) }
        if (!owned) schedule()
    }

    /** The owner booted (or could not). */
    fun ownerStarted(ok: Boolean) { started = ok; invalidate() }

    /** The owner published: what it made, drawn (a move through the ticker, a paint through this view). */
    fun onPublished() { publish() }

    /** What the owner published, drawn; whether there was any. */
    private fun publish(): Boolean {
        if (!started) return false
        if (!takeOwned()) return false
        if (moved || quiet) ticker?.invalidate() else invalidate()
        return true
    }

    /** While a press is held (up to [HOLD_MS]), the GPU is poked ([Warm]) every [holdEvery] frames, as a platform
     * press's ripple draws every frame: poked only at the press, the GPU went idle again before a 60 ms tap's release,
     * and the response frame's first submit took 1-4.4 ms (Pixel 10 Pro XL), 0.6-0.9 ms kept warm. Every third frame
     * kept it as warm as every frame for a fraction of the CPU (warmhold=N: every N frames; 0: only at the press). */
    private val holdEvery = (context as Activity).intent.getStringExtra("warmhold")?.toIntOrNull() ?: 3
    private var holdFrom = 0L
    private var holdFrames = 0
    private val holder = android.view.Choreographer.FrameCallback { hold() }
    private fun hold() {
        if (holdEvery <= 0 || holdFrom == 0L) return
        if (System.nanoTime() - holdFrom > HOLD_MS * 1_000_000) { owner?.held(false); return }
        if (holdFrames > 0 && holdFrames % holdEvery == 0) warmer.poke()
        holdFrames++
        android.view.Choreographer.getInstance().postFrameCallback(holder)
    }
    private val HOLD_MS = 400L
    private val warmer = Warm()
    /** wait=N: a release waits N ms for exact2's answer (Exact.WAIT_MS by default), to compare. */
    private val releaseWait = (context as Activity).intent.getStringExtra("wait")?.toLongOrNull() ?: Exact.WAIT_MS
    override fun onDetachedFromWindow() {
        warmer.destroy()
        worker.shutdown()
        val queue = Looper.getMainLooper().queue
        for (pfd in wakes) { queue.removeOnFileDescriptorEventListener(pfd.fileDescriptor); pfd.close() }
        wakes.clear()
        super.onDetachedFromWindow()
    }

    /** warm=0: a press that changes nothing draws no frame, to compare. */
    private val warm = (context as Activity).intent.getStringExtra("warm") != "0"

    /** Every stream the owner published since the last take: each paint ingested in order, the last move applied after
     * the last paint; the feed's group then drawn where the scrolls the owner has not applied put it. */
    private fun takeOwned(): Boolean {
        val o = owner ?: return false
        var any = false
        var painted = false
        var allQuiet = true
        var move: Owner.Pub? = null
        while (true) {
            val p = o.queue.poll() ?: break
            any = true
            // The fields first: the stream that came with them draws their nodes.
            p.fields?.let { fields?.told(it) }
            p.bars?.let { bars?.told(it) }
            p.access?.let { access?.invoke(it) }
            val ops = p.ops
            if (ops != null) {
                if (p.move) { move = p; applyMoveTracks(ops) } else {
                    move = null; painted = true; feedNext = p.feed; applyStream(ops); feedBaseX = 0f; feedBaseY = 0f
                    allQuiet = allQuiet && retained(ops)
                }
                feed = p.feed
            }
            while (sent.isNotEmpty() && sent.first().first <= p.seq) sent.removeFirst()
            // What the reader's own scrolling was told with this stream, in the streams' order.
            scrolling.acked(p.seq)
            p.scrollers?.let { scrolling.told(it) }
        }
        val moveOps = move?.ops
        if (moveOps != null) {
            applyMove(moveOps)
            groups[feed]?.let { feedBaseX = it.translationX; feedBaseY = it.translationY }
        }
        // The feed where a driver's scrolls put it, then every scroller a finger moved where the reader has it, and
        // the hosted fields with their scrollers' rows.
        if (any) { placeFeed(); scrolling.place(); fields?.place() }
        if (coverTrace && painted) traceNewRows()
        moved = !painted
        quiet = painted && allQuiet && move == null
        return any
    }

    /** After [applyStream]: whether [stream]'s top level is the last one's, its changed groups then recorded again
     * here (the view's drawing stands). Otherwise the next [onDraw] replays it all. */
    private fun retained(stream: IntBuffer): Boolean {
        if (!retain) return false
        val root = structure.read(stream) { id -> tokens[id] ?: -id }
        val last = rootSig
        rootSig = if (structure.opaque) null else root
        val same = last != null && !structure.opaque && root.contentEquals(last)
        if (same) {
            for (g in structure.groups) {
                val id = g[0]
                if (structure.groupSigs[id]?.contentEquals(groupSigs[id]) == true) continue
                val node = groups[id] ?: return false.also { rootSig = null }
                android.os.Trace.beginSection("exact group again")
                val c = node.beginRecording()
                main.replay(c, stream, g[1], g[2])
                node.endRecording()
                android.os.Trace.endSection()
            }
        }
        groupSigs.clear(); groupSigs.putAll(structure.groupSigs)
        return same
    }

    private fun placeFeed() {
        nearNatives()
        val g = groups[feed] ?: return
        g.translationX = feedBaseX
        g.translationY = feedY()
        // cover=1: how far the view reaches past the feed's drawn rows (px, either side), a trace counter.
        if (coverTrace && reach()) {
            val y = feedY()
            val gap = Math.max(0f, Math.max((-y + height) - reachDown, reachUp - (-y)))
            android.os.Trace.setCounter("exact uncovered", gap.toLong())
            var missing = 0; var inView = 0; var orphan = 0
            var oldest = 0L
            val now = System.nanoTime()
            for (r in spans) if (r[1] > -y && r[0] < -y + height) {
                inView++
                if (r[2] == 0f) { missing++; if (r[3] == 0f) orphan++; oldest = Math.max(oldest, (now - (definedAt[r[4].toInt()] ?: now)) / 1000) }
            }
            android.os.Trace.setCounter("exact missing age us", oldest)
            android.os.Trace.setCounter("exact rows in view", inView.toLong())
            android.os.Trace.setCounter("exact rows missing", missing.toLong())
            android.os.Trace.setCounter("exact rows missing unknown", orphan.toLong())
        }
    }
    private val coverTrace = (context as Activity).intent.getStringExtra("cover") == "1"
    private val definedAt = HashMap<Int, Long>()
    private val traced = HashSet<Int>()
    /** cover=1: where the feed's rows first defined by the streams just taken lie from the view (px past its edge in
     * the direction they are: 0 in view), the nearest, and how many are in view. */
    private fun traceNewRows() {
        val b = ops ?: return
        val range = groupRange(b, feed) ?: return
        val y = feedY()
        var nearest = Long.MAX_VALUE; var inView = 0; var n = 0
        var i = range[0]
        var drawn = 0; var above = 0f; var below = 0f
        while (i < range[1]) {
            if (b.get(i) == 17) {
                val id = b.get(i + 1)
                drawn++
                val t = f(b, i + 3) + y
                above = Math.max(above, -t); below = Math.max(below, t + (rows[id]?.height ?: 1) - height)
                if (definedAt.containsKey(id) && traced.add(id)) {
                    n++
                    val top = f(b, i + 3) + y
                    val bottom = top + (rows[id]?.height ?: 1)
                    val d = when { bottom < 0 -> -bottom; top > height -> top - height; else -> 0f }.toLong()
                    if (d == 0L) inView++
                    nearest = Math.min(nearest, d)
                }
            }
            i = structure.end(b, i)
        }
        android.os.Trace.setCounter("exact group rows", drawn.toLong())
        android.os.Trace.setCounter("exact group above px", above.toLong())
        android.os.Trace.setCounter("exact group below px", below.toLong())
        if (n > 0) {
            android.os.Trace.setCounter("exact new rows", n.toLong())
            android.os.Trace.setCounter("exact new rows in view", inView.toLong())
            android.os.Trace.setCounter("exact new row nearest px", nearest)
        }
    }

    /** lazy=0: every platform element has its view for as long as the stream keeps its row, to compare. Otherwise a
     * row's WebView or module view exists while the row is within [NEAR_AHEAD] view heights of the view in the
     * direction of the scroll and [NEAR_BEHIND] the other way ([Natives.near]; half a view more to keep one). */
    private val lazyOn = (context as Activity).intent.getStringExtra("lazy") != "0"
    private val nearAhead = (context as Activity).intent.getStringExtra("nearahead")?.toFloatOrNull() ?: NEAR_AHEAD
    private val nearBehind = (context as Activity).intent.getStringExtra("nearbehind")?.toFloatOrNull() ?: NEAR_BEHIND
    private val nearKeepMore = (context as Activity).intent.getStringExtra("nearkeep")?.toFloatOrNull() ?: NEAR_KEEP
    /** The feed's rows in the last stream: ids and tops in the group's coordinates. */
    private var feedIds = IntArray(64)
    private var feedTops = FloatArray(64)
    private var feedCount = -1
    /** The feed's group in the stream being applied (the owner names it with the stream). */
    private var feedNext = 0
    private val nearShow = HashSet<Int>()
    private val nearKeep = HashSet<Int>()
    private val nearKnown = HashSet<Int>()
    private val nearSeen = HashSet<Int>()

    /** The last stream's feed rows, read once a stream. */
    private fun feedRows() {
        feedCount = -1
        val n = natives ?: return
        if (!lazyOn) return
        val range = ops?.let { groupRange(it, feedNext) }
        if (range == null) {
            // No feed to place rows in: every element has its view.
            n.everywhere()
            return
        }
        val b = ops!!
        var k = 0
        var i = range[0]
        while (i < range[1]) {
            if (b.get(i) == 17) {
                if (k == feedIds.size) { feedIds = feedIds.copyOf(2 * k); feedTops = feedTops.copyOf(2 * k) }
                feedIds[k] = b.get(i + 1); feedTops[k] = f(b, i + 3); k++
            }
            i = structure.end(b, i)
        }
        feedCount = k
        n.lazy = true
    }

    /** Tell [natives] which rows are near the view as it is drawn now. */
    private fun nearNatives() {
        val n = natives ?: return
        if (feedCount < 0 || !n.nearing) return
        val y = feedY()
        val h = height.toFloat()
        val up = (if (scrollV < 0f) nearAhead else nearBehind) * h
        val down = (if (scrollV > 0f) nearAhead else nearBehind) * h
        nearShow.clear(); nearKeep.clear(); nearKnown.clear(); nearSeen.clear()
        for (k in 0 until feedCount) {
            val id = feedIds[k]
            val top = feedTops[k] + y
            val bottom = top + (rows[id]?.height ?: 1)
            nearKnown.add(id)
            if (bottom > -up - nearKeepMore * h && top < h + down + nearKeepMore * h) {
                nearKeep.add(id)
                if (bottom > -up && top < h + down) nearShow.add(id)
                if (bottom > 0f && top < h) nearSeen.add(id)
            }
        }
        n.near(nearShow, nearKeep, nearKnown, nearSeen)
    }

    /** Where the feed's group is drawn: the last stream's place, less the scrolls the owner has yet to apply; where a
     * finger moved it, the reader's own place for it ([Scrolling.shift]). */
    private fun feedY(): Float {
        scrolling.shift(feed)?.let { return it }
        var residual = 0f
        for ((_, d) in sent) residual += d
        return feedBaseY - residual
    }


    /** near=0: a row out of view when exact2 painted it records on the worker however soon the scroll brings it in.
     * Otherwise a new row within [NEAR_S] of travel of the view (where the stream draws it, in the direction of the
     * scroll) records here, as one in view does: on the worker it was drawn a frame or more after it came into view
     * (its group recorded again only once it was made), and at 24k dp/s rows come into view as they are built. */
    private val nearOn = (context as Activity).intent.getStringExtra("near") != "0"
    private val nearY = HashMap<Int, Float>()
    private var scrollV = 0f
    private var scrollAt = 0L

    /** The feed's rows' tops in [stream] (device px, where the view will draw them), when the scroll is fast. */
    private fun nearRows(stream: IntBuffer) {
        nearY.clear()
        if (!nearOn || !async || Math.abs(scrollV) * NEAR_S < height * 0.1f) return
        val range = groupRange(stream, feed) ?: return
        var residual = 0f
        for ((_, d) in sent) residual += d
        var i = range[0]
        while (i < range[1]) {
            if (stream.get(i) == 17) nearY[stream.get(i + 1)] = f(stream, i + 3) - residual
            i = structure.end(stream, i)
        }
    }

    /** Whether row [id], [h] px tall, is in view or comes into it within [NEAR_S] of the scroll. */
    private fun near(id: Int, h: Float): Boolean {
        val y = nearY[id] ?: return false
        val reach = Math.abs(scrollV) * NEAR_S
        val top = if (scrollV < 0) -reach else 0f
        val bottom = if (scrollV > 0) height + reach else height.toFloat()
        return y + h > top && y < bottom
    }

    /** The feed travelled [dy] px: its speed, which decides where rows are recorded and which are made first. */
    private fun travelled(dy: Float) {
        val now = System.nanoTime()
        val dt = (now - scrollAt) / 1e9f
        scrollAt = now
        scrollV = if (dt > 0.1f || dt <= 0f) 0f else scrollV * 0.6f + dy / dt * 0.4f
    }

    /** A driver's scroll of the feed by [dy] px ([ExactActivity.drive]): to exact2, on its thread or here. With an
     * owner, the feed's drawn rows move now. A finger's scroll is [Scrolling]'s. */
    fun scrollBy(dy: Float) {
        travelled(dy)
        scrolling.released()
        val o = owner
        if (o == null) { Native.scroll(dy); step(); return }
        val s = ++mainSeq
        sent.addLast(s to dy)
        o.scroll(dy, s)
        if (groups[feed] != null) { placeFeed(); fields?.place(); ticker?.invalidate() }
    }

    /** How far the feed's drawn rows reach from the view, down ([reachDown]) and up ([reachUp]), in the group's own
     * coordinates (device px); false when that is unknown. */
    private var reachDown = 0f
    private var reachUp = 0f
    private fun reach(): Boolean {
        val b = ops ?: return false
        val range = groupRange(b, feed) ?: return false
        val y = feedY()
        val top = -y; val bottom = -y + height
        // The drawn rows, by top: a row still recording on the worker (nothing drawn in its place) ends the reach.
        spans.clear()
        var i = range[0]
        while (i < range[1]) {
            if (b.get(i) == 17) {
                val id = b.get(i + 1)
                val node = rows[id] ?: pending[id]?.let { rows[it] }
                val ry = f(b, i + 3)
                spans.add(floatArrayOf(ry, ry + (node?.height ?: 1).toFloat(), if (node != null) 1f else 0f,
                    if (pending.containsKey(id)) 1f else 0f, id.toFloat()))
            }
            i = structure.end(b, i)
        }
        spans.sortBy { it[0] }
        var down = top
        for (r in spans) {
            if (r[1] <= down) continue
            if (r[0] > down + REACH_GAP || r[2] == 0f) break
            down = r[1]
        }
        var up = bottom
        for (r in spans.asReversed()) {
            if (r[0] >= up) continue
            if (r[1] < up - REACH_GAP || r[2] == 0f) break
            up = r[0]
        }
        reachDown = down; reachUp = up
        return true
    }
    private val spans = ArrayList<FloatArray>()

    /** tag=1: the feed's rows near the view as this frame draws them: `y=<feed y> h=<view h> | id:top:bottom:how ...`
     * (px from the view's top; how: D recorded, E a node with no drawing, P a row on the worker drawing its previous
     * node (p: that one has no drawing), M nothing to draw), and the stream's age. */
    fun tagState(): String {
        val b = ops ?: return "none"
        val range = groupRange(b, feed) ?: return "nofeed"
        val y = feedY()
        val sb = StringBuilder()
        sb.append("y=").append(y.toInt()).append(" h=").append(height).append(" age=").append((System.nanoTime() - tookAt) / 1000).append(" |")
        var i = range[0]
        while (i < range[1]) {
            if (b.get(i) == 17) {
                val id = b.get(i + 1)
                val own = rows[id]
                val prev = pending[id]?.let { rows[it] }
                val node = own ?: prev
                val top = f(b, i + 3) + y
                val bottom = top + (node?.height ?: 0)
                if (bottom > -400 && top < height + 400) {
                    val how = when {
                        own != null -> if (own.hasDisplayList()) "D" else "E"
                        prev != null -> if (prev.hasDisplayList()) "P" else "p"
                        else -> "M"
                    }
                    sb.append(' ').append(id).append(':').append(top.toInt()).append(':').append(bottom.toInt()).append(':').append(how)
                }
            }
            i = structure.end(b, i)
        }
        return sb.toString()
    }
    private var tookAt = 0L

    /** Group [id]'s body in [b] (after its GROUP_BEGIN, up to its GROUP_END), or null. */
    private fun groupRange(b: IntBuffer, id: Int): IntArray? {
        var i = 0
        var from = -1
        val n = b.limit()
        while (i < n) {
            val op = b.get(i)
            val j = structure.end(b, i)
            if (op == 20 && b.get(i + 1) == id) from = j
            if (op == 21 && from >= 0) return intArrayOf(from, i)
            i = j
        }
        return null
    }

    /** New rows recorded on the worker that a group may draw (nothing drawn in their place until then), and those
     * done since the last frame. */
    private val awaited = HashSet<Int>()
    private val done = HashSet<Int>()
    private var regrouping = false
    private val regroup = (context as Activity).intent.getStringExtra("regroup") != "0"
    /** Before the frame: the last stream's groups that draw a row done since, recorded again (the view's drawing
     * references them: it stands). */
    private val regroupFrame = android.view.Choreographer.FrameCallback {
        regrouping = false
        val b = ops
        if (b != null && done.isNotEmpty()) {
            var i = 0
            var from = -1
            var id = 0
            val n = b.limit()
            var again = false
            while (i < n) {
                val op = b.get(i)
                val j = structure.end(b, i)
                when (op) {
                    20 -> { from = j; id = b.get(i + 1); again = false }
                    17 -> if (from >= 0 && done.contains(b.get(i + 1))) again = true
                    21 -> {
                        val g = groups[id]
                        if (again && g != null) {
                            android.os.Trace.beginSection("exact group rows done")
                            val c = g.beginRecording()
                            main.replay(c, b, from, i)
                            g.endRecording()
                            android.os.Trace.endSection()
                        }
                        from = -1
                    }
                }
                i = j
            }
            ticker?.invalidate()
        }
        done.clear()
    }

    /** A new stream, if exact2 has one: made ready to replay. Whether there was one. */
    private fun take(): Boolean {
        android.os.Trace.beginSection("exact native frame")
        val fresh = Native.frame()
        android.os.Trace.endSection()
        // The fields first: the stream that came with them draws their nodes; a statement may come with none.
        Native.fields()?.let { fields?.told(it) }
        Native.bars()?.let { bars?.told(it) }
        if (accessible) Native.access(accessAgain).also { accessAgain = false }?.let { access?.invoke(it) }
        fresh ?: return false
        Native.chrome()?.let(chrome)
        val told = Native.scrollers()
        val stream = fresh.order(ByteOrder.LITTLE_ENDIAN).asIntBuffer()
        moved = stream.limit() > 0 && stream.get(0) == 22
        if (moved) { applyMove(stream); applyMoveTracks(stream) } else applyStream(stream)
        told?.let { scrolling.told(it) }
        scrolling.place()
        fields?.place()
        return true
    }

    /** The last stream, its scrollers' rows moved: nothing new to make. */
    private fun applyMove(stream: IntBuffer) {
        for (k in 0 until stream.get(1)) {
            val at = 2 + 3 * k
            groups[stream.get(at)]?.let { it.translationX = f(stream, at + 1); it.translationY = f(stream, at + 2) }
        }
    }

    /** What may follow a move's offsets: the clock and the tracks of layers whose plays changed since the last stream
     * (EXACT_MOVE_TRACKS: a row that came into view starts its animations). Every move's, in order. */
    private fun applyMoveTracks(stream: IntBuffer) {
        val from = 2 + 3 * stream.get(1)
        if (from >= stream.limit()) return
        android.os.Trace.beginSection("exact move tracks")
        ingest(stream, from, stream.limit())
        // Sampled at the next frame, with every other layer (sampling them all here, per move, was 0.4 ms of the main
        // thread a move).
        playLayers()
        android.os.Trace.endSection()
    }

    /** A new paint: what it announces made, then replayed by [onDraw]. */
    private fun applyStream(stream: IntBuffer) {
        streamFrame = true
        for (g in groups.values) { g.translationX = 0f; g.translationY = 0f }
        ops = stream
        tookAt = System.nanoTime()
        android.os.Trace.beginSection("exact ingest")
        val freed = ArrayList(freeing)
        for (id in freeing) free(id)
        freeing.clear()
        layers.flush()
        unrowedSeen.clear()
        rowTop.clear(); rowGroup.clear(); ingestGroup = 0
        nearRows(stream)
        // A row's platform view waits for [feedRows] to say where the row is (it gives every one its view when it cannot).
        natives?.lazy = lazyOn
        ingest(stream, 0, stream.limit())
        if (layers.playing()) { layers.sample(System.nanoTime()); playLayers() }
        // After the stream: a row drawn again under a new id names its elements before the old row lets them go.
        natives?.let { n -> for (id in freed) n.rowFreed(id); n.unrowed(unrowedSeen) }
        feedRows()
        placeLater()
        // The shared copies live for one stream: kept longer, they hold pictures exact2 has let go of from the GC.
        picturesCopy = null; playingCopy = null; heldCopy = null
        android.os.Trace.endSection()
    }

    private fun free(id: Int) {
        // Still waiting to be made: it never is.
        laterRows.remove(id)?.let { it.made = true }
        rowTall.delete(id)
        when {
            pending.containsKey(id) -> cancelled.add(id)
            pending.containsValue(id) -> stale.add(id)
            else -> { rows.remove(id)?.discardDisplayList(); tokens.remove(id) }
        }
    }

    /** After a scroll step: a move draws through the ticker, a new stream through this view. The rows the scroll
     * brings near mount after this frame is handed off (prefetch=0: inside the scroll, as exact2 does by default). */
    fun step() {
        if (take()) { if (moved) ticker?.invalidate() else { taken = true; invalidate() } }
        schedule()
        // Every third step: the window leads the view by a quarter second of travel, so this keeps it filled, and
        // rows mount (and retire) in fewer, larger passes, each one paint. (Mounting a row or two between every
        // frame, as RecyclerView's GapWorker does, painted every frame and cost more: xheavy 12k 83 vs 91 fps.)
        if (prefetch && ++steps % 3 == 0) { removeCallbacks(refiner); post(refiner) }
    }
    private var steps = 0

    private val prefetch = (context as Activity).intent.getStringExtra("prefetch") != "0"
    private val refiner = Runnable {
        android.os.Trace.beginSection("exact prefetch")
        if (Native.refine(-1, 0f) and 1 != 0) pump()
        android.os.Trace.endSection()
    }

    /** A wake (a reply, a picture, a timer): draw only when the stream changed. */
    private fun pump() {
        if (owner != null || !running) return // the owner publishes what a key or a reply changed
        if (take()) { if (moved) ticker?.invalidate() else { taken = true; invalidate() } } else schedule()
    }

    /** exact2 wants another turn: at the next vsync, ask it, and draw only if the stream changed. */
    private val turn = android.view.Choreographer.FrameCallback { turnPosted = false; pump() }
    private var turnPosted = false

    /** While a layer plays: each vsync, sample them and draw a frame (the ticker: nothing is recorded again). */
    private val sampler = android.view.Choreographer.FrameCallback { t ->
        samplerPosted = false
        // A transition exact2 began after this frame's vsync (the main thread waited across it for the answer that
        // began it, and runs the frame late): sampled now, not at the vsync before it began, where it shows nothing
        // yet (a fade's first frame at opacity 0 drew nothing, and its first visible frame came a vsync later).
        val at = if (lateSample && layers.startsAfter(t)) maxOf(t, System.nanoTime()) else t
        val moving = layers.sample(at)
        ticker?.invalidate()
        if (moving) playLayers()
    }
    private var samplerPosted = false
    /** latesample=0: layers sampled at the frame's vsync always, to compare. */
    private val lateSample = (context as Activity).intent.getStringExtra("latesample") != "0"

    private fun playLayers() {
        if (running && !samplerPosted && layers.playing()) { samplerPosted = true; android.view.Choreographer.getInstance().postFrameCallback(sampler) }
    }

    private fun schedule() {
        // What this turn asked of the platform (a press's haptic, the keyboard for a field it focused).
        asking()
        if (!running) return
        if (Native.animating() && !turnPosted) { turnPosted = true; android.view.Choreographer.getInstance().postFrameCallback(turn) }
        var due = Native.nextDue()
        // A moved paint is owed a paint once scrolling pauses: each step pushes this back.
        if (Native.owed() && (due < 0 || due > 16)) due = 16.0
        removeCallbacks(timer)
        if (due >= 0) postDelayed(timer, due.toLong().coerceAtLeast(0))
    }

    private fun f(b: IntBuffer, i: Int) = java.lang.Float.intBitsToFloat(b.get(i))

    /** Faces, pictures and rows announced in a new stream, made once. */
    private fun ingest(b: IntBuffer, from: Int, n: Int) {
        var i = from
        while (i < n) {
            when (b.get(i)) {
                1 -> i += 7
                2 -> i += 14
                3 -> i = skipPath(b, i + 3)
                4 -> i += 13
                5 -> i = skipPath(b, i + 2)
                6 -> i += 1
                7 -> i += 2
                8 -> i += 6
                9 -> i += 6 + 3 * b.get(i + 5)
                10, 41 -> { fonts[b.get(i + 1)] = fontAt(b, i); i = fontEnd(b, i) }
                11 -> {
                    val id = b.get(i + 1)
                    // hwbuf=1: the picture straight into a GPU buffer, wrapped as a hardware bitmap (as an image
                    // loader's decode-to-hardware bitmap is): no heap copy, no upload on the RenderThread.
                    val bmp = picture(id, b.get(i + 2), b.get(i + 3))
                    images[id] = bmp
                    picturesCopy = null
                    if (hardware) harden(id, bmp)
                    i += 4
                }
                12 -> {
                    owner?.made?.remove(b.get(i + 1))?.recycle()
                    // Slots still drawing it draw nothing from now on (their rows are gone or about to change).
                    drawnIn.remove(b.get(i + 1))?.forEach { slots.remove(it)?.discardDisplayList(); heldCopy = null }
                    animated.remove(b.get(i + 1))?.let { it.stop(); playingCopy = null }
                    images.remove(b.get(i + 1))?.let { bmp ->
                        picturesCopy = null
                        main.forget(bmp)
                        // Java's reference only (API 26+): recorded rows keep the pixels until they are done with
                        // them. After the worker's queue, which may still be recording with it.
                        if (async) worker.execute { background.forget(bmp); post { bmp.recycle() } } else bmp.recycle()
                    }
                    i += 2
                }
                13 -> i = skipPath(b, i + 5)
                14 -> i += 18
                25 -> i += 3 + b.get(i + 2)
                43 -> i += 2 + b.get(i + 1)
                44 -> i += 2
                27 -> i += 14
                42 -> i += 14 + 2 * b.get(i + 1)
                40 -> i += 27
                28 -> {
                    val id = b.get(i + 1); val kind = b.get(i + 2); val len = b.get(i + 15)
                    val bytes = ByteArray(len)
                    for (k in 0 until len) bytes[k] = (b.get(i + 16 + k / 4) ushr (8 * (k % 4))).toByte()
                    val scale = resources.displayMetrics.density
                    val w = Math.round(f(b, i + 5) * scale); val h = Math.round(f(b, i + 6) * scale)
                    if (ingestRow == 0) unrowedSeen.add(id)
                    // A hosted text field (kind 4) is not the stream's to make: exact2 says its fields apart ([Fields]),
                    // and the stream says only that it draws one.
                    if (kind == 4) fields?.drawn(id) else natives?.ensure(id, kind, w, h, String(bytes), ingestRow)
                    i += 16 + (len + 3) / 4
                }
                26 -> {
                    val id = b.get(i + 1); val len = b.get(i + 2)
                    val bytes = ByteArray(len)
                    for (k in 0 until len) bytes[k] = (b.get(i + 3 + k / 4) ushr (8 * (k % 4))).toByte()
                    try {
                        val d = android.graphics.ImageDecoder.decodeDrawable(android.graphics.ImageDecoder.createSource(File(String(bytes))))
                        if (d is android.graphics.drawable.AnimatedImageDrawable) {
                            d.repeatCount = android.graphics.drawable.AnimatedImageDrawable.REPEAT_INFINITE
                            d.start()
                            animated.put(id, d)?.stop()
                            playingCopy = null
                        }
                    } catch (e: Exception) { android.util.Log.w("exact", "animated $id: $e") }
                    i += 3 + (len + 3) / 4
                }
                15 -> {
                    val id = b.get(i + 1); val len = b.get(i + 5)
                    val shown = b.get(i + 2) < 0 || near(id, f(b, i + 4))
                    val body = i + 6; val end = body + len - 1
                    rowW = Math.ceil(f(b, i + 3).toDouble()).toInt(); rowH = Math.ceil(f(b, i + 4).toDouble()).toInt()
                    val outerRow = ingestRow
                    if (outerRow == 0) rowTall.put(id, rowH)
                    ingestRow = id
                    ingest(b, body, end)
                    ingestRow = outerRow
                    rowW = 0
                    var j = body + len
                    // (With an owner: its streams are copies that outlive the next one.)
                    if (laterNs > 0 && owner != null && async && !shown && outerRow == 0 && !skipDefs) {
                        // Out of view and not about to come into it: what it and its slots and layers define (faces,
                        // pictures, platform views, nodes) is known now, as the rows after it may draw with them; its
                        // drawing is made in the frames after ([makeLater]), a few rows a frame.
                        val wait = Later(b, i, body + len)
                        sets@ while (j < n) {
                            when (b.get(j)) {
                                24 -> { val l = b.get(j + 2); laterSlots[b.get(j + 1)] = wait; ingest(b, j + 3, j + 3 + l); j += 3 + l }
                                30 -> { val l = b.get(j + 2); laterLayers[b.get(j + 1)] = wait; ingest(b, j + 19, j + 3 + l); j += 3 + l }
                                31 -> { laterLayers[b.get(j + 1)] = wait; j += 3 + b.get(j + 2) }
                                33 -> { laterLayers[b.get(j + 1)] = wait; j += 3 }
                                else -> break@sets
                            }
                        }
                        wait.end = j
                        // Where the stream draws it (op 17, right after its sets), if it does.
                        if (j < n && b.get(j) == 17 && b.get(j + 1) == id) wait.top = f(b, j + 3)
                        laterRows[id] = wait
                        later.add(wait)
                        if (!laterPosted) { laterPosted = true; android.view.Choreographer.getInstance().postFrameCallback(laterFrame) }
                    } else makeRow(b, i, shown)
                    i = j
                }
                16 -> i += 1
                17 -> { if (ingestRow == 0) { rowTop.put(b.get(i + 1), b.get(i + 3)); rowGroup.put(b.get(i + 1), ingestGroup) }; i += 4 }
                18 -> { freeing.add(b.get(i + 1)); i += 2 }
                19 -> i += 26
                20 -> { if (ingestRow == 0) ingestGroup = b.get(i + 1); i += 2 }
                21 -> { if (ingestRow == 0) ingestGroup = 0; i += 1 }
                23 -> {
                    // Its row's bounds, clipped: a slot is drawn in its row's coordinates.
                    val s = slot(b.get(i + 1))
                    if (rowW > 0) { s.setPosition(0, 0, rowW, rowH); s.setClipToBounds(true) }
                    i += 2
                }
                29 -> { layers.node(b.get(i + 1)); if (ingestRow != 0) layers.inRow(b.get(i + 1), ingestRow); i += 2 }
                30 -> {
                    // A layer's drawing, after its matrix, pivot, base and dash phase index (16 words).
                    val id = b.get(i + 1); val len = b.get(i + 2)
                    if (!skipDefs) laterLayers[id]?.let { makeOne(it) }
                    layers.set(id, b, i + 3, len)
                    if (!skipDefs) ingest(b, i + 19, i + 3 + len)
                    val node = layers.node(id)
                    // Its node only as big as its drawing (clipped to that): what HWUI damages as it moves and fades.
                    if (layerBounds && bounds.measure(b, i + 19, i + 3 + len, measured)) {
                        measured.roundOut(placed); layers.place(id, placed)
                    } else layers.place(id, null)
                    // One shape has no text to rasterize again at each scale: it is scaled as it is drawn (a chart's
                    // pulse ring, in every row in view; flatalpha=0 to compare). The node's overlapping-rendering flag
                    // is left alone: a RenderNode's is off natively, and turning it on gave every fading layer an
                    // offscreen surface its own size (xheavy: 10 GB of graphics memory in bursts).
                    layers.flat(id, flatAlpha && main.singleShape(b, i + 19, i + 3 + len))
                    val c = node.beginRecording()
                    main.replayAt(c, b, i + 19, i + 3 + len, layers.originX(id), layers.originY(id))
                    node.endRecording()
                    i += 3 + len
                }
                31 -> {
                    // A waiting row's layer: these tracks follow its own, when it is made (a move sends each layer's).
                    val n31 = 3 + b.get(i + 2)
                    val wait = if (skipDefs) null else laterLayers[b.get(i + 1)]?.takeIf { !it.made }
                    if (wait != null) wait.after.add(IntArray(n31) { b.get(i + it) }) else layers.tracks(b.get(i + 1), b, i + 3, b.get(i + 2))
                    i += n31
                }
                32 -> { laterLayers[b.get(i + 1)]?.let { makeOne(it) }; layers.free(b.get(i + 1)); i += 2 }
                33 -> {
                    val wait = if (skipDefs) null else laterLayers[b.get(i + 1)]?.takeIf { !it.made }
                    if (wait != null) wait.after.add(intArrayOf(33, b.get(i + 1), b.get(i + 2))) else layers.clock(b.get(i + 1), b.get(i + 2))
                    i += 3
                }
                24 -> {
                    val id = b.get(i + 1); val len = b.get(i + 2)
                    if (!skipDefs) { laterSlots[id]?.let { makeOne(it) }; ingest(b, i + 3, i + 3 + len) }
                    val node = slot(id)
                    val c = node.beginRecording()
                    main.replay(c, b, i + 3, i + 3 + len)
                    node.endRecording()
                    if (hardware) slotBodies[id] = IntArray(len) { b.get(i + 3 + it) }
                    // Which pictures it draws: freeing one empties it.
                    var k = i + 3
                    while (k < i + 3 + len) {
                        val op = b.get(k)
                        if (op == 8 || op == 14) drawnIn.getOrPut(b.get(k + 1)) { HashSet() }.add(id)
                        k = when (op) { 1 -> k + 7; 4 -> k + 13; 6 -> k + 1; 8 -> k + 6; 11 -> k + 4; 14 -> k + 18; else -> i + 3 + len }
                    }
                    i += 3 + len
                }
                else -> throw IllegalStateException("op ${b.get(i)} at $i")
            }
        }
    }

    /** Row [id]'s node (op 15 at [i], its body already walked by [ingest]): recorded here when it is in view this
     * frame, else on the worker. */
    private fun makeRow(b: IntBuffer, i: Int, shown: Boolean) {
        val id = b.get(i + 1); val previous = b.get(i + 2) and 0x7fffffff; val len = b.get(i + 5)
        val body = i + 6; val end = body + len - 1
        val node = android.graphics.RenderNode("exact row")
        node.setPosition(0, 0, Math.ceil(f(b, i + 3).toDouble()).toInt(), Math.ceil(f(b, i + 4).toDouble()).toInt())
        // Clipped to its bounds (exact2 pads them): HWUI then skips a row out of view.
        node.setClipToBounds(true)
        // In view this frame: made now, never a blank row for a frame. Otherwise on the worker.
        if (async && !shown) { tokens[id] = nextToken++; record(id, previous, node, b, body, end) } else {
            // Again into the node of the row it replaces (owned, shown, and nothing else drawing that node):
            // what references it stands, and HWUI damages this row only.
            val again = if (retain && owner != null && previous != 0 && !pending.containsKey(previous) &&
                !pending.containsValue(previous)) rows.remove(previous) else null
            val into = again ?: node
            if (again != null) {
                into.setPosition(0, 0, Math.ceil(f(b, i + 3).toDouble()).toInt(), Math.ceil(f(b, i + 4).toDouble()).toInt())
                tokens[id] = tokens.remove(previous) ?: nextToken++
            } else tokens[id] = nextToken++
            val c = into.beginRecording()
            main.replay(c, b, body, end)
            into.endRecording()
            rows.put(id, into)?.let { if (it !== into) it.discardDisplayList() }
        }
    }

    /** A row out of view whose making waits ([ingest]): op 15 at [at] in [b], then its slots' and layers' sets from
     * [sets] to [end]. [made]: made, or freed before it was. */
    private class Later(val b: IntBuffer, val at: Int, val sets: Int) {
        var end = sets
        var made = false
        /** Its top where the last stream drew it, in the feed's group (NaN: no stream drew it yet). */
        var top = Float.NaN
        /** Tracks and clocks (ops 31, 33) that came for its layers while it waited, in order. */
        val after = ArrayList<IntArray>(0)
    }
    private val later = java.util.ArrayDeque<Later>()
    private var laterPosted = false
    /** The waiting rows by id, and by the layers and slots their sets name: a later stream that frees a row drops it;
     * one that sets or frees one of its layers or slots makes that row first ([makeOne]); and the tracks and clocks
     * that come for its layers meanwhile (every move sends them) are kept for when it is made. */
    private val laterRows = HashMap<Int, Later>()
    private val laterLayers = HashMap<Int, Later>()
    private val laterSlots = HashMap<Int, Later>()
    /** While a waiting row's sets are replayed: what they define was made when the stream came. */
    private var skipDefs = false
    /** later=N: the main thread's ms a frame for rows that wait (2 by default; 0: every row is made as its stream
     * arrives, as before). A pass that waited for its window's lead mounts some twenty crypto rows in one stream,
     * and making their layers (three a row, 0.07 ms each), tracks and worker copies as it arrived took 7 to 10 ms
     * of one frame: one presented late every eight seconds at 12,000 and 24,000 dp/s. */
    private val laterNs = ((context as Activity).intent.getStringExtra("later")?.toLongOrNull() ?: 2L) * 1_000_000
    /** A stream came since the last frame: that frame draws it, and the waiting rows are made from the frame after. */
    private var streamFrame = false
    private val laterFrame: android.view.Choreographer.FrameCallback = android.view.Choreographer.FrameCallback {
        laterPosted = false
        makeSoon()
        if (streamFrame) {
            streamFrame = false
            if (later.isNotEmpty()) { laterPosted = true; android.view.Choreographer.getInstance().postFrameCallback(laterFrame) }
        } else makeLater(laterNs)
    }

    /** The waiting rows the scroll is about to bring into view (within a quarter of the view, or three frames of
     * travel): made now, here, whatever the frame's budget. (Without this an xheavy row, which takes more than a
     * frame's budget to make, came into view still waiting: 5 to 7% of the view blank at 12,000 and 24,000 dp/s.) */
    private fun makeSoon() {
        if (laterRows.isEmpty()) return
        val y = feedY()
        val margin = Math.max(height * 0.25f, Math.abs(scrollV) * 0.025f)
        var soon: ArrayList<Later>? = null
        for (l in laterRows.values) {
            if (l.top.isNaN()) continue
            val top = l.top + y
            if (top + f(l.b, l.at + 4) > -margin && top < height + margin) (soon ?: ArrayList<Later>().also { soon = it }).add(l)
        }
        val now = soon ?: return
        android.os.Trace.beginSection("exact rows soon")
        for (l in now) makeOne(l, true)
        picturesCopy = null; playingCopy = null; heldCopy = null
        android.os.Trace.endSection()
    }

    /** Where the stream just applied draws the waiting rows. */
    private fun placeLater() {
        if (laterRows.isEmpty()) return
        val b = ops ?: return
        val range = groupRange(b, feedNext) ?: return
        var i = range[0]
        while (i < range[1]) {
            if (b.get(i) == 17) laterRows[b.get(i + 1)]?.let { it.top = f(b, i + 3) }
            i = structure.end(b, i)
        }
    }

    /** Make one waiting row: its node (here when [now], else on the worker unless it is in view), then its slots'
     * and layers' drawings and tracks. */
    private fun makeOne(l: Later, now: Boolean = false) {
        if (l.made) return
        l.made = true
        val id = l.b.get(l.at + 1)
        laterRows.remove(id)
        makeRow(l.b, l.at, now || l.b.get(l.at + 2) < 0 || near(id, f(l.b, l.at + 4)))
        val outer = skipDefs
        skipDefs = true
        ingest(l.b, l.sets, l.end)
        for (a in l.after) ingest(IntBuffer.wrap(a), 0, a.size)
        skipDefs = outer
    }

    /** Make the rows that wait, nearest the way the view travels first, until [budget] ns are spent. */
    private fun makeLater(budget: Long) {
        if (later.isEmpty()) return
        android.os.Trace.beginSection("exact rows later")
        val t = System.nanoTime()
        while (true) {
            val l = (if (scrollV < 0f) later.pollLast() else later.pollFirst()) ?: break
            if (l.made) continue
            makeOne(l)
            if (System.nanoTime() - t >= budget) break
        }
        picturesCopy = null; playingCopy = null; heldCopy = null
        if (later.isEmpty()) { laterLayers.clear(); laterSlots.clear() }
        if (layers.playing()) playLayers()
        if (later.isNotEmpty() && !laterPosted) { laterPosted = true; android.view.Choreographer.getInstance().postFrameCallback(laterFrame) }
        android.os.Trace.endSection()
    }

    /** A row recorded on the worker: its ops copied, the pictures it draws resolved now. Until it is made, the row it
     * replaces draws in its place. */
    private fun record(id: Int, previous: Int, node: android.graphics.RenderNode, b: IntBuffer, body: Int, end: Int) {
        val copy = IntArray(end - body)
        for (k in copy.indices) copy[k] = b.get(body + k)
        val pictures = picturesCopy ?: HashMap(images).also { picturesCopy = it }
        val playing = playingCopy ?: HashMap(animated).also { playingCopy = it }
        val elements = natives?.snapshot() ?: emptyMap()
        val hosted = fields?.snapshot() ?: emptyMap()
        val held = heldCopy ?: HashMap(slots).also { heldCopy = it }
        val animating = layers.nodes()
        pending[id] = previous
        if (coverTrace) definedAt[id] = System.nanoTime()
        val replacing = previous != 0
        if (!replacing) awaited.add(id)
        worker.execute {
            android.os.Trace.beginSection(if (replacing) "exact row again" else "exact row new")
            background.image = { pictures[it] }
            background.animated = { playing[it] }
            background.native = { elements[it] ?: hosted[it] }
            background.slot = { held[it] }
            background.layer = { animating[it] }
            val c = node.beginRecording()
            background.replay(c, IntBuffer.wrap(copy), 0, copy.size)
            node.endRecording()
            android.os.Trace.endSection()
            post {
                pending.remove(id)
                if (cancelled.remove(id)) node.discardDisplayList() else rows.put(id, node)?.discardDisplayList()
                // A new row its group could not draw yet: the groups drawing it are recorded again before the next
                // frame, so it shows now, not with the next stream (regroup=0: with the next stream, to compare).
                if (awaited.remove(id) && regroup) { done.add(id); if (!regrouping) { regrouping = true; android.view.Choreographer.getInstance().postFrameCallback(regroupFrame) } }
                // What was drawn in its place is in the view's drawing: the next stream draws it all again.
                rootSig = null
                // The row it replaces may still be in the last recorded drawing: it goes with the next stream.
                if (stale.contains(previous) && !pending.containsValue(previous)) {
                    stale.remove(previous)
                    freeing.add(previous)
                }
                // Out of view when it was sent: the next frame there is anyway shows it (no frame of its own).
            }
        }
    }

    /**
     * hw=1: each picture moves to GPU memory (a hardware bitmap, as Coil's are), made on the worker; the slots drawing it
     * are recorded again with it. Without, a picture is a heap bitmap plus the texture HWUI uploads from it.
     */
    private val hardware = (context as Activity).intent.getStringExtra("hw") == "1"
    private val slotBodies = HashMap<Int, IntArray>()

    private fun harden(id: Int, bmp: Bitmap) {
        worker.execute {
            val hw = bmp.copy(Bitmap.Config.HARDWARE, false) ?: return@execute
            post {
                if (images[id] !== bmp) { hw.recycle(); return@post } // freed (or replaced) meanwhile
                images[id] = hw
                picturesCopy = null
                rootSig = null
                main.forget(bmp)
                for (slotId in drawnIn[id] ?: emptySet<Int>()) {
                    val body = slotBodies[slotId] ?: continue
                    val node = slots[slotId] ?: continue
                    val c = node.beginRecording()
                    main.replay(c, IntBuffer.wrap(body), 0, body.size)
                    node.endRecording()
                }
                bmp.recycle()
            }
        }
    }

    private val hwbuf = (context as Activity).intent.getStringExtra("hwbuf") != "0"

    private fun picture(id: Int, w: Int, h: Int): Bitmap = owner?.made?.remove(id) ?: makePicture(id, w, h, hwbuf)

    private var rowW = 0
    private var rowH = 0

    private fun slot(id: Int) = slots.getOrPut(id) {
        heldCopy = null
        android.graphics.RenderNode("exact slot").apply { setPosition(0, 0, 1 shl 14, 1 shl 14); setClipToBounds(false) }
    }

    private fun skipPath(b: IntBuffer, at: Int): Int {
        var i = at + 1
        repeat(b.get(at)) {
            i += when (b.get(i)) { 0, 1 -> 3; 2 -> 7; else -> 1 }
        }
        return i
    }
}

/** Seconds of travel within which a new row is made on the main thread rather than the worker. */
private const val NEAR_S = 0.033f
/** How far from the view, in its heights, a row's platform view exists: the way the scroll goes, and the other. */
private const val NEAR_AHEAD = 1f
private const val NEAR_BEHIND = 0.5f
/** How much farther, in view heights, a row keeps the view it has (so a small scroll back and forth makes none). */
private const val NEAR_KEEP = 0.5f

/** Px between a feed's rows that still count as one reach of drawn rows. */
private const val REACH_GAP = 64f
