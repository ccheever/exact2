package dev.exact.canvas

import android.content.Context
import android.graphics.RectF
import android.graphics.RenderNode
import android.view.Choreographer
import android.view.MotionEvent
import android.view.VelocityTracker
import android.view.View
import android.view.ViewConfiguration
import android.view.ViewGroup
import android.widget.EdgeEffect
import android.widget.FrameLayout
import android.widget.OverScroller
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.roundToInt

/**
 * A finger scrolls (LLP 1115.001 D7.1 to D7.3). The gesture and its physics are the reader's, with the platform's own
 * classes and nothing imitated: the system's touch slop and fling velocities ([ViewConfiguration]), [VelocityTracker],
 * [OverScroller] for the fling, the stretch [EdgeEffect] at the ends, and the scrollbar a [View] draws and fades.
 *
 * The reader moves a scroller at once and exact2 catches up: a scroller whose rows the stream keeps in a node of their
 * own (a group) is translated here, on the UI thread, in the frame the finger moved; exact2 is told how far
 * ([Native.scrolled]) and fills in rows behind it. A scroller with no such node (one inside another's row, one that
 * scrolls sideways, the page itself) moves when exact2's paint of it arrives, which the frame waits a moment for.
 *
 * What crosses to exact2 and back is `host/linux/src/presenter/scrollers.rs`:
 * - at a touch's down the reader asks what is under it ([Native.under]): the scrollers, innermost first, and the axes
 *   exact2's own gestures claim there (a swipe row, a reorder grip, a `pan`);
 * - past the slop, a drag along an axis exact2 claims stays its own; along one a scroller under the finger scrolls
 *   on, the scroll wins: exact2's press and drag recognition for the touch is cancelled ([ExactView.forward] with a
 *   cancel) and no more of the touch reaches it;
 * - a move goes to the innermost scroller first and what it cannot take (it is at its end) to the next one out, as
 *   nested scrolling hands off on Android; what none can take stretches the innermost one's edge;
 * - with each stream exact2 says where the scrollers the reader knows are, where their rows were painted and how far
 *   they go ([Native.scrollers]), with a revision that moves on when the app scrolled one itself: the reader then
 *   takes exact2's offset and ends the fling that was moving it. A scroller that statement leaves out is gone.
 */
internal class Scrolling(private val view: ExactView) {
    /** One scroller as exact2 told it, in the view's pixels. */
    class Scroller(val id: Int) {
        /** Bit 0: it scrolls sideways; bit 1: up and down; bit 2: its rows are a node the reader moves. */
        var flags = 0
        /** What exact2 calls its revision: a move sent under an older one is dropped there. */
        var rev = 0
        /** Whether a statement has said where it is. */
        var told = false
        /** Where exact2 has it, where its rows were last painted, and its farthest offset. */
        var nowX = 0f; var nowY = 0f; var drawnX = 0f; var drawnY = 0f; var maxX = 0f; var maxY = 0f
        /** What the reader moved it by that no stream it has taken includes yet. */
        var pendX = 0f; var pendY = 0f
        /** The reader has moved it: its rows are placed here, not by exact2's moves. */
        var own = false
        /** The box it shows in. */
        val port = RectF()
        /** What the view draws for its rows once an edge has stretched: see [record]. */
        var holder: RenderNode? = null
        /** Its edges' effects: top, bottom, left, right. */
        val edges = arrayOfNulls<EdgeEffect>(4)
        var bar: Indicator? = null
        val node get() = flags and 4 != 0
        fun at(sideways: Boolean) = if (sideways) nowX + pendX else nowY + pendY
        fun end(sideways: Boolean) = if (sideways) maxX else maxY
    }

    /** A scroller under the touch, and the axes it takes part on for a drag from there. */
    private class Link(val s: Scroller, val x: Boolean, val y: Boolean)
    /** A move sent to exact2's thread: its number, and how far it moved which scroller. */
    private class Sent(val seq: Int, val s: Scroller, val dx: Float, val dy: Float)

    private val scrollers = HashMap<Int, Scroller>()
    private val sent = ArrayDeque<Sent>()
    private val config = ViewConfiguration.get(view.context)
    private val slop = config.scaledTouchSlop
    private val minFling = config.scaledMinimumFlingVelocity
    private val maxFling = config.scaledMaximumFlingVelocity
    private val frames = Choreographer.getInstance()
    private val fling = OverScroller(view.context)
    private var tracker: VelocityTracker? = null

    /** The touch that is down: counted, so an answer for one that ended is not taken for the next. */
    var serial = 0
        private set
    private var mode = IDLE
    /** What exact2 said is under the touch, once it has. */
    private var chain: List<Link> = emptyList()
    private var claims = 0
    private var answered = false
    /** The drag's or the fling's axis, and the scrollers it moves, innermost first. */
    private var sideways = false
    private var active: List<Scroller> = emptyList()
    private var pointer = -1
    private var tool = 0
    private var downX = 0f
    private var downY = 0f
    private var lastX = 0f
    private var lastY = 0f
    private var flingAt = 0

    /**
     * A touch event: whether it was taken here (a scroll's), so that exact2 does not hear it. A down, and every move
     * until the touch is known to be a scroll, goes on to exact2: it may be a tap, a press, or a gesture of its own.
     */
    fun touch(e: MotionEvent): Boolean {
        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                tracker?.recycle()
                tracker = VelocityTracker.obtain().also { it.addMovement(e) }
                pointer = e.getPointerId(0); tool = e.getToolType(0)
                downX = e.x; downY = e.y; lastX = e.x; lastY = e.y
                serial++
                if (mode == FLINGING) {
                    // A finger on a list that is flinging stops it and has it, as the platform's lists do: no press.
                    halt()
                    if (active.any { it.port.contains(e.x, e.y) }) { mode = DRAGGING; return true }
                }
                mode = WATCHING; answered = false; chain = emptyList(); claims = 0
                return false
            }
            MotionEvent.ACTION_MOVE -> {
                tracker?.addMovement(e)
                val i = e.findPointerIndex(pointer)
                if (i < 0) return mode == DRAGGING
                val x = e.getX(i); val y = e.getY(i)
                if (mode == WATCHING && answered) decide(x, y)
                if (mode != DRAGGING) return false
                val d = if (sideways) lastX - x else lastY - y
                lastX = x; lastY = y
                if (d != 0f) drag(d, x, y)
                return true
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                if (mode != DRAGGING) return false
                // Another finger takes the drag, as ScrollView's does.
                val i = e.actionIndex
                pointer = e.getPointerId(i); lastX = e.getX(i); lastY = e.getY(i)
                return true
            }
            MotionEvent.ACTION_POINTER_UP -> {
                if (mode != DRAGGING) return false
                val i = e.actionIndex
                if (e.getPointerId(i) == pointer) {
                    val other = if (i == 0) 1 else 0
                    pointer = e.getPointerId(other); lastX = e.getX(other); lastY = e.getY(other)
                    tracker?.clear()
                }
                return true
            }
            else -> {
                val lifted = e.actionMasked == MotionEvent.ACTION_UP
                if (lifted) tracker?.addMovement(e)
                val was = mode
                if (was == DRAGGING) release(lifted) else mode = IDLE
                tracker?.recycle(); tracker = null
                return was == DRAGGING
            }
        }
    }

    /**
     * exact2's answer for touch [serial] ([Native.under]): the claims, a count, and eleven words a scroller. The
     * statement that tells each scroller's place is published before this arrives ([Owner.under]).
     */
    fun under(serial: Int, w: IntArray) {
        if (serial != this.serial || mode != WATCHING || w.size < 3) return
        claims = w[0]
        val links = ArrayList<Link>(w[1])
        // After the claims and the count, the hosted field under the touch ([Fields.under]); then the scrollers.
        var i = 3
        while (i + 11 <= w.size) {
            val s = scrollers.getOrPut(w[i]) { Scroller(w[i]) }
            if (!s.told) { s.rev = w[i + 2]; s.nowX = f(w[i + 3]); s.nowY = f(w[i + 4]); s.maxX = f(w[i + 5]); s.maxY = f(w[i + 6]) }
            s.flags = (s.flags and 3) or (w[i + 1] and 4)
            s.port.set(f(w[i + 7]), f(w[i + 8]), f(w[i + 7]) + f(w[i + 9]), f(w[i + 8]) + f(w[i + 10]))
            links.add(Link(s, w[i + 1] and 1 != 0, w[i + 1] and 2 != 0))
            i += 11
        }
        // The indicators of scrollers this touch is not over go: they have faded, or are about to.
        for (s in scrollers.values) if (links.none { it.s === s }) s.bar?.let { (it.parent as? ViewGroup)?.removeView(it); s.bar = null }
        chain = links
        answered = true
    }

    /** Past the slop: whose the touch is. Along an axis exact2 claims, its own; along one a scroller under it scrolls
     * on, the scroll's, and exact2 is told to let go of the touch. Otherwise nobody's yet. */
    private fun decide(x: Float, y: Float) {
        val ax = abs(x - downX); val ay = abs(y - downY)
        if (max(ax, ay) <= slop) return
        // A hosted text field that drags its own selection or caret asked to keep its touch, as it asks a ScrollView.
        if (view.fieldKeeps()) return
        val across = ax > ay
        if (claims and (if (across) 1 else 2) != 0) { mode = THEIRS; return }
        val takes = { side: Boolean -> chain.any { if (side) it.x else it.y } }
        val other = (if (across) ay else ax) > slop && claims and (if (across) 2 else 1) == 0
        sideways = when {
            takes(across) -> across
            other && takes(!across) -> !across
            else -> return
        }
        active = chain.filter { if (sideways) it.x else it.y }.map { it.s }
        mode = DRAGGING
        lastX = x; lastY = y
        view.forward(3, x, y, tool, 0L)
    }

    /** The finger moved [delta] px along the drag's axis (toward the end positive), at ([x], [y]). */
    private fun drag(delta: Float, x: Float, y: Float) {
        var d = delta
        val edged = edged()
        // A stretched edge takes the finger's way back before anything scrolls.
        if (edged != null) d -= relax(edged, d, x, y)
        val rest = if (d != 0f) move(d) else 0f
        if (rest != 0f && edged != null) pull(edged, rest, x, y)
    }

    /**
     * Move the drag's scrollers by [d] px along its axis: the innermost as far as it goes, the rest to the next one
     * out (the hand-off). Each one's rows move now, where it has a node, and exact2 is told. What none could take.
     */
    private fun move(d: Float): Float {
        var rest = d
        var waits = false
        for ((k, s) in active.withIndex()) {
            val at = s.at(sideways)
            val took = (at + rest).coerceIn(0f, s.end(sideways)) - at
            if (abs(took) < 0.01f) continue
            s.own = true
            val dx = if (sideways) took else 0f; val dy = if (sideways) 0f else took
            val seq = view.scrolled(s.id, dx, dy, s.rev)
            // Told to exact2's own thread: it is ahead of exact2 by this much until a stream includes the move.
            if (seq != 0) { s.pendX += dx; s.pendY += dy; sent.addLast(Sent(seq, s, dx, dy)) }
            waits = waits || !s.node
            // The scrollers inside it went with it.
            for (j in 0 until k) { val inner = active[j]; inner.port.offset(-dx, -dy); inner.bar?.let { b -> place(b, inner) } }
            indicate(s)
            rest -= took
            if (abs(rest) < 0.01f) { rest = 0f; break }
        }
        place()
        view.fields?.place()
        view.tick()
        if (waits) view.awaitScrolled()
        return rest
    }

    /** Each scroller the reader has moved, drawn where the reader has it: its rows' node as far from where they were
     * painted as the offset is. Called after every stream too, which draws them where exact2 had them. */
    fun place() {
        for (s in scrollers.values) {
            if (!s.own || !s.node || !s.told) continue
            val g = view.group(s.id) ?: continue
            g.translationX = s.drawnX - s.nowX - s.pendX
            g.translationY = s.drawnY - s.nowY - s.pendY
        }
    }

    /** How far scroller [id]'s rows are drawn from where they were painted, down, when the reader moves it. */
    fun shift(id: Int): Float? = scrollers[id]?.takeIf { it.own && it.node && it.told }?.let { it.drawnY - it.nowY - it.pendY }

    /** A stream that includes the reader's moves up to number [seq] was taken. */
    fun acked(seq: Int) {
        while (sent.isNotEmpty() && sent.first().seq <= seq) { val m = sent.removeFirst(); m.s.pendX -= m.dx; m.s.pendY -= m.dy }
        if (sent.isEmpty()) for (s in scrollers.values) { s.pendX = 0f; s.pendY = 0f }
    }

    /** exact2's statement with a stream ([Native.scrollers]): nine words a scroller. */
    fun told(w: IntArray) {
        var i = 0
        while (i + 9 <= w.size) {
            val s = scrollers.getOrPut(w[i]) { Scroller(w[i]) }
            s.flags = w[i + 1]
            s.nowX = f(w[i + 3]); s.nowY = f(w[i + 4]); s.drawnX = f(w[i + 5]); s.drawnY = f(w[i + 6])
            s.maxX = f(w[i + 7]); s.maxY = f(w[i + 8])
            if (s.told && w[i + 2] != s.rev) {
                // The app moved it: what the reader sent under the old revision is dropped there and here, and a
                // fling that was moving it ends. A finger that is dragging it goes on from where the app put it.
                sent.removeAll { it.s === s }
                s.pendX = 0f; s.pendY = 0f
                if (mode == FLINGING && s in active) { halt(); mode = IDLE }
            }
            s.rev = w[i + 2]
            s.told = true
            i += 9
        }
        // A scroller the statement no longer has is gone (its node went, or a driver took it over): what the reader
        // kept for it goes with it.
        val count = w.size / 9
        if (scrollers.size <= count) return
        for (s in scrollers.values.filter { s -> s.told && (0 until count).none { w[it * 9] == s.id } }) {
            scrollers.remove(s.id)
            sent.removeAll { it.s === s }
            s.bar?.let { (it.parent as? ViewGroup)?.removeView(it) }
            if (mode == FLINGING && s in active) { halt(); mode = IDLE }
        }
    }

    /** A driver from outside scrolls ([ExactView.scrollBy]): the scrollers are exact2's to place again. */
    fun released() {
        if (mode == FLINGING) { halt(); mode = IDLE }
        for (s in scrollers.values) s.own = false
    }

    /** The activity stopped, or a touch took a fling: the fling ends where it is. */
    fun halt() {
        fling.forceFinished(true)
        frames.removeFrameCallback(flinger)
    }

    /** The finger lifted ([lifted]) or the system took the touch: a fling at the finger's velocity, or rest. */
    private fun release(lifted: Boolean) {
        var v = 0f
        val t = tracker
        if (lifted && t != null) {
            t.computeCurrentVelocity(1000, maxFling.toFloat())
            v = -(if (sideways) t.getXVelocity(pointer) else t.getYVelocity(pointer))
        }
        val edged = edged()
        val stretched = edged != null && edged.edges.any { it != null && it.distance != 0f }
        if (edged != null) { for (e in edged.edges) e?.onRelease(); if (stretched) stretching() }
        if (stretched || abs(v) < minFling || active.isEmpty()) { mode = IDLE; return }
        mode = FLINGING
        flingAt = 0
        // Unbounded, as RecyclerView flings: each frame's travel is clamped by the scrollers themselves, whose ends
        // may move while it runs.
        if (sideways) fling.fling(0, 0, v.toInt(), 0, Int.MIN_VALUE, Int.MAX_VALUE, 0, 0)
        else fling.fling(0, 0, 0, v.toInt(), 0, 0, Int.MIN_VALUE, Int.MAX_VALUE)
        frames.postFrameCallback(flinger)
    }

    private val flinger: Choreographer.FrameCallback = Choreographer.FrameCallback {
        if (mode != FLINGING) return@FrameCallback
        if (!fling.computeScrollOffset()) { mode = IDLE; return@FrameCallback }
        val at = if (sideways) fling.currX else fling.currY
        val d = (at - flingAt).toFloat()
        flingAt = at
        var done = fling.isFinished
        if (d != 0f) {
            val rest = move(d)
            if (rest != 0f) {
                // Every scroller is at its end: what is left of the fling goes into the edge.
                edged()?.let { s -> edge(s, toward(rest > 0f)).onAbsorb(fling.currVelocity.toInt()); stretching() }
                fling.forceFinished(true)
                done = true
            }
        }
        if (done) mode = IDLE else frames.postFrameCallback(flinger)
    }

    // The ends: the platform's stretch.

    /** The scroller whose edge stretches: the innermost of the drag that has a node to stretch. */
    private fun edged(): Scroller? = active.firstOrNull { it.node }

    private fun toward(end: Boolean) = if (sideways) (if (end) RIGHT else LEFT) else (if (end) BOTTOM else TOP)

    private fun edge(s: Scroller, which: Int): EdgeEffect = s.edges[which] ?: EdgeEffect(view.context).also { s.edges[which] = it }

    /** Where across [s]'s port the finger is, 0 to 1, as the edge at [which] counts it (ScrollView's and
     * HorizontalScrollView's displacement). */
    private fun across(s: Scroller, which: Int, x: Float, y: Float): Float {
        val a = if (sideways) (y - s.port.top) / max(1f, s.port.height()) else (x - s.port.left) / max(1f, s.port.width())
        return (if (which == TOP || which == RIGHT) a else 1f - a).coerceIn(0f, 1f)
    }

    private fun length(s: Scroller) = max(1f, if (sideways) s.port.width() else s.port.height())

    /** How much of [d] a stretched edge of [s] takes back as the finger returns toward it. */
    private fun relax(s: Scroller, d: Float, x: Float, y: Float): Float {
        val size = length(s)
        val start = s.edges[toward(false)]; val end = s.edges[toward(true)]
        val took = when {
            d < 0f && end != null && end.distance != 0f -> size * end.onPullDistance(d / size, across(s, toward(true), x, y))
            d > 0f && start != null && start.distance != 0f -> -size * start.onPullDistance(-d / size, across(s, toward(false), x, y))
            else -> 0f
        }
        if (took != 0f) stretching()
        return took
    }

    /** [rest] px the finger moved with every scroller at its end: [s]'s edge that way stretches. */
    private fun pull(s: Scroller, rest: Float, x: Float, y: Float) {
        val which = toward(rest > 0f)
        edge(s, which).onPullDistance(abs(rest) / length(s), across(s, which, x, y))
        s.edges[toward(rest < 0f)]?.takeIf { !it.isFinished }?.onRelease()
        stretching()
    }

    private var edging = false
    /** An edge moves: each frame until it rests, the scrollers that have one are recorded again. */
    private fun stretching() {
        for (s in scrollers.values) if (s.holder == null && s.node && s.edges.any { it != null }) {
            // The view's drawing names the scroller's rows directly until now: it is recorded once more, naming the
            // holder ([holder]).
            s.holder = RenderNode("exact scroller").apply { setClipToBounds(false) }
            view.again()
        }
        if (edging) return
        edging = true
        frames.postFrameCallback(edger)
    }
    private val edger = Choreographer.FrameCallback {
        edging = false
        var more = false
        for (s in scrollers.values) if (s.holder != null) more = record(s) || more
        view.tick()
        if (more) stretching()
    }

    /** What the view draws for scroller [id]'s rows in place of their node, once an edge of it has stretched. */
    fun holder(id: Int): RenderNode? = scrollers[id]?.holder?.also { if (!it.hasDisplayList()) scrollers[id]?.let { s -> record(s) } }

    /**
     * [s]'s holder, recorded: its rows' node, and while an edge is stretched that node inside one the size of the
     * scroller's port with the edges drawn into it. [EdgeEffect.draw] is the platform's own stretch: it stretches the
     * node it is recorded into, as it does a ScrollView's. It adds to the stretch that node has, and only the view
     * system can clear one, so each frame of a stretch records a new node. Whether an edge was still moving.
     */
    private fun record(s: Scroller): Boolean {
        val holder = s.holder ?: return false
        val g = view.group(s.id) ?: return false
        val live = s.edges.any { it != null && !it.isFinished }
        holder.setPosition(0, 0, view.width, view.height)
        val c = holder.beginRecording()
        if (!live) c.drawRenderNode(g) else {
            val l = s.port.left.roundToInt(); val t = s.port.top.roundToInt()
            val w = max(1, s.port.width().roundToInt()); val h = max(1, s.port.height().roundToInt())
            val port = RenderNode("exact port")
            port.setPosition(l, t, l + w, t + h)
            val p = port.beginRecording()
            p.translate(-l.toFloat(), -t.toFloat())
            p.drawRenderNode(g)
            p.translate(l.toFloat(), t.toFloat())
            // As ScrollView.draw and HorizontalScrollView.draw place them.
            s.edges[TOP]?.takeIf { !it.isFinished }?.let { it.setSize(w, h); it.draw(p) }
            s.edges[BOTTOM]?.takeIf { !it.isFinished }?.let {
                val n = p.save(); p.translate(-w.toFloat(), h.toFloat()); p.rotate(180f, w.toFloat(), 0f)
                it.setSize(w, h); it.draw(p); p.restoreToCount(n)
            }
            s.edges[LEFT]?.takeIf { !it.isFinished }?.let {
                val n = p.save(); p.rotate(270f); p.translate(-h.toFloat(), 0f)
                it.setSize(h, w); it.draw(p); p.restoreToCount(n)
            }
            s.edges[RIGHT]?.takeIf { !it.isFinished }?.let {
                val n = p.save(); p.rotate(90f); p.translate(0f, -w.toFloat())
                it.setSize(h, w); it.draw(p); p.restoreToCount(n)
            }
            port.endRecording()
            c.drawRenderNode(port)
        }
        holder.endRecording()
        return live
    }

    // The indicators: the platform's scrollbar.

    /**
     * The scroll indicator of one scroller: a view over its port that draws nothing of its own. What shows is the
     * scrollbar [View] itself draws for a view that scrolls, from the theme's thumb, at the theme's size, and it fades
     * when and as [View] fades it ([awakenScrollBars]). The style gives it its scrollbars, as `android:scrollbars`
     * gives a ScrollView its.
     */
    class Indicator(context: Context) : View(context, null, 0, R.style.ExactScrollbars) {
        var rangeX = 0; var rangeY = 0; var atX = 0; var atY = 0
        init { importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO }
        override fun computeVerticalScrollRange() = rangeY
        override fun computeVerticalScrollOffset() = atY
        override fun computeVerticalScrollExtent() = height
        override fun computeHorizontalScrollRange() = rangeX
        override fun computeHorizontalScrollOffset() = atX
        override fun computeHorizontalScrollExtent() = width
        fun wake() { awakenScrollBars() }
    }

    /** [s] moved: its indicator shows where it is. */
    private fun indicate(s: Scroller) {
        val parent = view.parent as? ViewGroup ?: return
        val w = s.port.width().roundToInt(); val h = s.port.height().roundToInt()
        if (w <= 0 || h <= 0) return
        val b = s.bar ?: Indicator(view.context).also { s.bar = it; parent.addView(it, FrameLayout.LayoutParams(w, h)) }
        if (b.layoutParams.width != w || b.layoutParams.height != h) b.layoutParams = FrameLayout.LayoutParams(w, h)
        place(b, s)
        b.rangeX = if (s.flags and 1 != 0) (s.maxX + w).roundToInt() else 0
        b.rangeY = if (s.flags and 2 != 0) (s.maxY + h).roundToInt() else 0
        b.atX = s.at(true).roundToInt(); b.atY = s.at(false).roundToInt()
        b.wake()
    }

    private fun place(b: Indicator, s: Scroller) {
        b.translationX = view.left + s.port.left
        b.translationY = view.top + s.port.top
    }

    private fun f(bits: Int) = java.lang.Float.intBitsToFloat(bits)

    private companion object {
        const val IDLE = 0
        /** A touch is down and exact2 has it; whether it becomes a scroll is not known yet. */
        const val WATCHING = 1
        /** The touch is a gesture of exact2's own: nothing of it scrolls. */
        const val THEIRS = 2
        const val DRAGGING = 3
        const val FLINGING = 4
        const val TOP = 0
        const val BOTTOM = 1
        const val LEFT = 2
        const val RIGHT = 3
    }
}
