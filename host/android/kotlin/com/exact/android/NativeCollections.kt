package com.exact.android

import android.content.Context
import android.view.MotionEvent
import android.view.View
import android.widget.OverScroller
import android.widget.ScrollView
import org.json.JSONArray
import org.json.JSONObject
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.roundToInt

/** A native ScrollView with an owned Android OverScroller for relocatable anchors.
 * Android's private fling stores absolute coordinates. Owning the public scroller
 * lets a measured-height correction move that coordinate origin without restarting
 * its native velocity curve. Touch recognition and child ownership stay ScrollView's.
 */
internal class NativeCollectionScrollView(context: Context) : ScrollView(context) {
    private val motion = OverScroller(context)
    private var bias = 0L
    private var direction = 0
    private var touched = false
    private var authoredTarget: Int? = null
    var onMotionEnded: (() -> Unit)? = null
    val moving: Boolean get() = touched || !motion.isFinished
    val correctingMotion: Boolean get() = authoredTarget != null && !motion.isFinished
    val reportedTarget: Int? get() = authoredTarget
    val velocity: Float get() = if (motion.isFinished || authoredTarget != null) 0f else motion.currVelocity * direction

    private fun maximum(): Int = (getChildAt(0)?.height ?: 0).minus(height - paddingTop - paddingBottom).coerceAtLeast(0)
    override fun fling(velocityY: Int) {
        motion.forceFinished(true)
        bias = 0
        authoredTarget = null
        direction = velocityY.compareTo(0)
        motion.fling(0, scrollY, 0, velocityY, 0, 0, 0, maximum())
        postInvalidateOnAnimation()
    }
    override fun computeScroll() {
        // Keyboard/accessibility animations initiated by ScrollView still run.
        super.computeScroll()
        if (motion.computeScrollOffset()) {
            val wanted = (motion.currY.toLong() + bias).coerceIn(0L, maximum().toLong()).toInt()
            scrollTo(scrollX, wanted)
            if ((wanted == 0 && direction < 0) || (wanted == maximum() && direction > 0)) motion.forceFinished(true)
            if (!motion.isFinished) postInvalidateOnAnimation()
        }
        if (motion.isFinished && (direction != 0 || authoredTarget != null)) {
            direction = 0
            authoredTarget = null
            bias = 0
            onMotionEnded?.invoke()
        }
    }
    override fun onInterceptTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> { stopOwnedMotion(); touched = true }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> touched = false
        }
        return super.onInterceptTouchEvent(event)
    }
    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> { stopOwnedMotion(); touched = true }
            MotionEvent.ACTION_MOVE -> touched = true
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> touched = false
        }
        return super.onTouchEvent(event)
    }
    private fun stopOwnedMotion() {
        val wasMoving = !motion.isFinished || authoredTarget != null
        motion.forceFinished(true)
        authoredTarget = null
        direction = 0
        bias = 0
        if (wasMoving) onMotionEnded?.invoke()
    }
    fun shiftAnchor(delta: Int, target: Int) {
        if (!motion.isFinished) {
            bias += delta.toLong()
            authoredTarget = authoredTarget?.let { (it.toLong() + delta).coerceIn(0L, maximum().toLong()).toInt() }
        }
        scrollTo(scrollX, target.coerceIn(0, maximum()))
    }
    fun correctTo(target: Int, smooth: Boolean) {
        if (moving && authoredTarget == null) return
        stopOwnedMotion()
        val reachable = target.coerceIn(0, maximum())
        if (!smooth || reachable == scrollY) { scrollTo(scrollX, reachable); return }
        authoredTarget = reachable
        direction = reachable.compareTo(scrollY)
        motion.startScroll(0, scrollY, 0, reachable - scrollY, 300)
        postInvalidateOnAnimation()
    }
    fun closeCollection() { onMotionEnded = null; stopOwnedMotion() }
}

/** Collection metadata belongs to the shared runner. Only mounted wrappers cross
 * this boundary; native scrolling reports v3 LE geometry and measured border boxes.
 */
internal class NativeCollections(
    private val host: View,
    private val density: Float,
    private val geometry: (Int) -> Port?,
    private val rowGeometry: (Int) -> RowBox?,
    private val report: (ByteArray) -> Unit
) {
    data class Port(
        val scroll: ScrollView,
        val content: Presenter.Box,
        val paddingLeft: Double = 0.0,
        val paddingTop: Double = 0.0,
        val paddingRight: Double = 0.0,
        val paddingBottom: Double = 0.0
    )
    data class RowBox(val main: Double, val cross: Double, val start: Double, val view: View? = null)
    internal data class Row(val view: Int, val root: Int, val epoch: ULong)
    internal data class Correction(val sequence: ULong, val offset: Double, val from: Double?, val smooth: Boolean)
    internal data class Snapshot(
        val view: Int, val revision: ULong, val sequence: ULong, val extent: Double,
        val rows: List<Row>, val correction: Correction?, val pending: Boolean, val parent: Int?
    )
    internal data class Measurement(val view: Int, val epoch: ULong, val size: Double)
    internal data class Facts(
        val offset: Double, val main: Double, val portCross: Double, val cross: Double,
        val measurements: List<Measurement>, val focus: Int?, val interaction: Int?
    ) {
        fun encode(view: Int, revision: ULong, sequence: ULong, velocity: Double, ancestorMoving: Boolean): ByteArray {
            val out = ByteBuffer.allocate(84 + measurements.size * 20).order(ByteOrder.LITTLE_ENDIAN)
            out.putInt(3).putInt(view).putLong(revision.toLong()).putLong(sequence.toLong())
            for (value in doubleArrayOf(offset, main, portCross, cross)) out.putDouble(value)
            out.putInt(focus ?: 0).putInt(interaction ?: 0).putDouble(velocity)
            out.putInt(-1).putInt(if (ancestorMoving) 1 else 0).putInt(measurements.size)
            for (row in measurements) out.putInt(row.view).putLong(row.epoch.toLong()).putDouble(row.size)
            return out.array()
        }
    }
    internal class Cursor {
        var sequence = 0uL
            private set
        var jumpedAt = 0uL
            private set
        private var corrected: ULong? = null
        private var shifted: Triple<ULong, Double, Double>? = null
        fun advance() { if (sequence < ULong.MAX_VALUE) sequence++ }
        fun jump() { advance(); jumpedAt = sequence }
        fun takeAbsolute(revision: ULong, sequence: ULong): Boolean {
            if (sequence != this.sequence || corrected?.let { revision <= it } == true) return false
            corrected = revision
            return true
        }
        fun takeShift(revision: ULong, correction: Correction, jumpedBefore: ULong): Double? {
            val from = correction.from ?: return null
            if (correction.sequence < jumpedBefore || corrected?.let { revision <= it } == true) return null
            val prior = shifted?.takeIf { it.first == correction.sequence && it.second == from }?.third ?: from
            corrected = revision
            shifted = Triple(correction.sequence, from, correction.offset)
            return correction.offset - prior
        }
    }
    private class Entry(var snapshot: Snapshot) {
        val cursor = Cursor()
        var facts: Facts? = null
        var lastSequence: ULong? = null
        var dimensions: List<Double>? = null
        var batchStart: Double? = null
        var scroll: ScrollView? = null
        var layoutListener: View.OnLayoutChangeListener? = null
    }
    private val entries = linkedMapOf<Int, Entry>()
    private val dirty = linkedSetOf<Int>()
    private var depth = 0
    private var correcting = false
    private var busy = false
    private var queued = false
    private var closed = false
    private var focus: Int? = null
    private var interaction: Int? = null
    private var lastVisited: Int? = null
    private val continuation = Runnable { queued = false; if (!closed) flush() }

    init { require(density.isFinite() && density > 0) }
    fun owns(id: Int): Boolean = entries.containsKey(id)
    fun mountedRows(id: Int): List<Row> = entries[id]?.snapshot?.rows ?: emptyList()
    fun beginBatch() {
        if (closed) return
        if (depth == 0) for ((id, entry) in entries) entry.batchStart = facts(id, entry)?.offset
        depth++
    }
    /** A failed publication must not keep the collection adapter inside a batch.
     * The owner reports the failure; partial retained geometry is never fed back.
     */
    fun abortBatch() {
        depth = 0
        correcting = false
        for (entry in entries.values) entry.batchStart = null
        host.removeCallbacks(continuation)
        queued = false
        dirty.clear()
    }
    fun snapshots(items: JSONArray) {
        check(!closed)
        val next = (0 until items.length()).map { parse(items.getJSONObject(it)) }
        require(next.map { it.view }.toSet().size == next.size) { "duplicate Android collection" }
        val live = next.map { it.view }.toSet()
        for (id in entries.keys.filter { it !in live }) remove(id)
        for (snapshot in next) {
            val existing = entries[snapshot.view]
            if (existing == null) entries[snapshot.view] = Entry(snapshot)
            else if (snapshot.revision >= existing.snapshot.revision) existing.snapshot = snapshot
            dirty.add(snapshot.view)
        }
        if (depth == 0) schedule()
    }
    fun endBatch() {
        if (closed || depth == 0) return
        depth--
        if (depth != 0) return
        correcting = true
        try {
            for ((id, entry) in entries) {
                val port = geometry(id) ?: continue
                watch(id, entry, port.scroll)
                fit(port, entry.snapshot.extent)
                val facts = facts(id, entry) ?: continue
                val dimensions = listOf(facts.main, facts.portCross, facts.cross)
                val planned = entry.cursor.sequence
                val jumped = entry.cursor.jumpedAt
                if (entry.dimensions != null && entry.dimensions != dimensions) entry.cursor.jump()
                entry.dimensions = dimensions
                entry.snapshot.correction?.let { correction ->
                    if (correction.from != null) {
                        entry.cursor.takeShift(entry.snapshot.revision, correction, jumped)?.let { delta ->
                            val target = (entry.batchStart ?: facts.offset) + delta + port.paddingTop
                            val deltaPixels = pixels(delta)
                            val targetPixels = pixels(target)
                            (port.scroll as? NativeCollectionScrollView)?.shiftAnchor(deltaPixels, targetPixels)
                                ?: port.scroll.scrollTo(port.scroll.scrollX, targetPixels)
                        }
                    } else {
                        val sequence = if (correction.sequence == planned) entry.cursor.sequence else correction.sequence
                        if (entry.cursor.takeAbsolute(entry.snapshot.revision, sequence)) {
                            val target = pixels(correction.offset + port.paddingTop)
                            val native = port.scroll as? NativeCollectionScrollView
                            if (native != null) native.correctTo(target, correction.smooth)
                            else if (correction.smooth) port.scroll.smoothScrollTo(port.scroll.scrollX, target)
                            else port.scroll.scrollTo(port.scroll.scrollX, target)
                        }
                    }
                }
                entry.batchStart = null
                dirty.add(id)
            }
        } finally { correcting = false }
        schedule()
    }
    fun changed(id: Int, user: Boolean = false) {
        if (closed || correcting || !entries.containsKey(id)) return
        val entry = entries.getValue(id)
        if (user && depth == 0 && (geometry(id)?.scroll as? NativeCollectionScrollView)?.correctingMotion != true) entry.cursor.advance()
        dirty.add(id)
        if (depth == 0) schedule()
    }
    fun intent(id: Int, travel: Boolean = false) {
        if (closed || correcting) return
        val entry = entries[id] ?: return
        if (travel) entry.cursor.advance() else entry.cursor.jump()
        dirty.add(id)
        if (depth == 0) schedule()
    }
    fun pins(focus: Int?, interaction: Int?) {
        if (closed || (this.focus == focus && this.interaction == interaction)) return
        this.focus = focus
        this.interaction = interaction
        dirty.addAll(entries.keys)
        schedule()
    }
    private fun watch(id: Int, entry: Entry, scroll: ScrollView) {
        if (entry.scroll === scroll) return
        unwatch(entry)
        entry.scroll = scroll
        val listener = View.OnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> changed(id) }
        entry.layoutListener = listener
        scroll.addOnLayoutChangeListener(listener)
        (scroll as? NativeCollectionScrollView)?.onMotionEnded = { changed(id) }
    }
    private fun unwatch(entry: Entry) {
        entry.layoutListener?.let { entry.scroll?.removeOnLayoutChangeListener(it) }
        (entry.scroll as? NativeCollectionScrollView)?.onMotionEnded = null
        entry.layoutListener = null
        entry.scroll = null
    }
    private fun remove(id: Int) {
        entries.remove(id)?.let { entry ->
            val scroll = entry.scroll
            unwatch(entry)
            (scroll as? NativeCollectionScrollView)?.closeCollection()
        }
        dirty.remove(id)
    }
    private fun pixels(value: Double): Int {
        val physical = value * density
        require(physical.isFinite() && physical >= Int.MIN_VALUE && physical <= Int.MAX_VALUE) {
            "Android collection coordinate exceeds the native scroll range"
        }
        return physical.roundToInt()
    }
    private fun fit(port: Port, extent: Double) {
        val scroll = port.scroll
        val width = (scroll.width - scroll.paddingLeft - scroll.paddingRight).coerceAtLeast(0)
        require(listOf(port.paddingLeft, port.paddingTop, port.paddingRight, port.paddingBottom).all(::valid)) {
            "invalid Android collection padding"
        }
        val physicalExtent = ceil((extent + port.paddingTop + port.paddingBottom) * density)
        require(physicalExtent.isFinite() && physicalExtent <= Int.MAX_VALUE) {
            "Android collection extent exceeds the native scroll range"
        }
        val height = maxOf(scroll.height - scroll.paddingTop - scroll.paddingBottom, physicalExtent.toInt())
        if (port.content.contentHeight != height || port.content.contentWidth != width) {
            port.content.contentHeight = height
            port.content.contentWidth = width
            port.content.requestLayout()
            if (width > 0 && scroll.height > 0) {
                // Correct offsets against the coherent extent before ScrollView
                // clamps to a temporarily short child during the next traversal.
                port.content.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
                    View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY))
                port.content.layout(port.content.left, port.content.top, port.content.left + width, port.content.top + height)
            }
        }
    }
    private fun facts(id: Int, entry: Entry): Facts? {
        val port = geometry(id) ?: return null
        val scroll = port.scroll
        if (!scroll.isShown || scroll.width <= 0 || scroll.height <= 0) return null
        val main = (scroll.height - scroll.paddingTop - scroll.paddingBottom).coerceAtLeast(0) / density.toDouble()
        val portCross = (scroll.width - scroll.paddingLeft - scroll.paddingRight).coerceAtLeast(0) / density.toDouble()
        val cross = entry.snapshot.rows.firstNotNullOfOrNull { row -> rowGeometry(row.view)?.cross }
            ?: (portCross - port.paddingLeft - port.paddingRight).coerceAtLeast(0.0)
        val native = scroll as? NativeCollectionScrollView
        val offset = ((native?.reportedTarget ?: scroll.scrollY) / density.toDouble() - port.paddingTop).coerceAtLeast(0.0)
        val measurements = entry.snapshot.rows.mapNotNull { row ->
            val box = rowGeometry(row.view) ?: return@mapNotNull null
            if (!valid(box.main) || !valid(box.cross) || abs(box.cross - cross) > 0.5 / density) return@mapNotNull null
            Measurement(row.view, row.epoch, box.main)
        }
        if (!valid(main) || !valid(portCross) || !valid(cross) || !valid(offset)) return null
        return Facts(offset, main, portCross, cross, measurements,
            focus?.takeIf { owner(it) == id }, interaction?.takeIf { owner(it) == id })
    }
    private fun owner(descendant: Int): Int? {
        for ((id, entry) in entries) if (entry.snapshot.rows.any { it.view == descendant || it.root == descendant }) return id
        var native = rowGeometry(descendant)?.view
        while (native != null) {
            for ((id, entry) in entries) if (entry.snapshot.rows.any { rowGeometry(it.view)?.view === native }) return id
            native = native.parent as? View
        }
        return null
    }
    private fun schedule() {
        if (closed || queued || dirty.isEmpty()) return
        queued = true
        if (!host.post(continuation)) queued = false
    }
    private fun flush() {
        if (closed || busy || depth != 0) return
        busy = true
        var reports = 0
        var visits = entries.size
        try {
            while (dirty.isNotEmpty() && reports < 2 && visits-- > 0) {
                // Release an old pin before another collection acquires it.
                val focusOwner = focus?.let(::owner)
                val interactionOwner = interaction?.let(::owner)
                val retiring = dirty.filter { id -> entries[id]?.facts?.let {
                    (it.focus != null && focusOwner != id) || (it.interaction != null && interactionOwner != id)
                } == true }
                val candidates = if (retiring.isEmpty()) dirty.toList() else retiring
                val id = candidates.firstOrNull { lastVisited == null || it > lastVisited!! } ?: candidates.first()
                lastVisited = id
                dirty.remove(id)
                val entry = entries[id] ?: continue
                // A hidden former owner still has to release its pin before
                // another collection acquires it. Cached port geometry is only
                // used for retirement, with no hidden-row measurements.
                var next = facts(id, entry) ?: entry.facts?.takeIf { id in retiring }?.copy(
                    measurements = emptyList(), focus = focus?.takeIf { focusOwner == id },
                    interaction = interaction?.takeIf { interactionOwner == id }) ?: continue
                if (id in retiring) {
                    next = next.copy(focus = next.focus?.takeIf { it == entry.facts?.focus },
                        interaction = next.interaction?.takeIf { it == entry.facts?.interaction })
                    dirty.add(id)
                }
                if (next != entry.facts || entry.lastSequence != entry.cursor.sequence || entry.snapshot.pending) {
                    entry.facts = next
                    entry.lastSequence = entry.cursor.sequence
                    val scroll = geometry(id)?.scroll as? NativeCollectionScrollView
                    val velocity = scroll?.velocity?.toDouble()?.div(density) ?: 0.0
                    val ancestorMoving = entry.snapshot.parent?.let { (geometry(it)?.scroll as? NativeCollectionScrollView)?.moving } == true
                    reports++
                    report(next.encode(id, entry.snapshot.revision, entry.cursor.sequence, velocity, ancestorMoving))
                }
            }
        } finally { busy = false }
        if (dirty.isNotEmpty()) schedule()
    }
    fun close() {
        if (closed) return
        closed = true
        host.removeCallbacks(continuation)
        for (entry in entries.values) {
            val scroll = entry.scroll
            unwatch(entry)
            (scroll as? NativeCollectionScrollView)?.closeCollection()
        }
        entries.clear()
        dirty.clear()
        queued = false
        focus = null
        interaction = null
    }
    internal companion object {
        private fun uint(value: Any?): ULong? = when (value) {
            is String -> value.toULongOrNull()
            is Byte -> value.toLong().takeIf { it >= 0 }?.toULong()
            is Short -> value.toLong().takeIf { it >= 0 }?.toULong()
            is Int -> value.toLong().takeIf { it >= 0 }?.toULong()
            is Long -> value.takeIf { it >= 0 }?.toULong()
            is Double -> value.takeIf { it.isFinite() && it >= 0 && it < 9_007_199_254_740_992.0 && it % 1.0 == 0.0 }?.toULong()
            else -> null
        }
        private fun id(value: Any?): Int = requireNotNull(uint(value)?.takeIf { it > 0uL && it <= UInt.MAX_VALUE.toULong() }) {
            "invalid Android collection view id"
        }.toUInt().toInt()
        private fun number(value: Any?): Double = requireNotNull((value as? Number)?.toDouble()?.takeIf(::valid)) {
            "invalid Android collection geometry"
        }
        private fun valid(value: Double): Boolean = value.isFinite() && value >= 0 && value <= Float.MAX_VALUE
        internal fun parse(value: JSONObject): Snapshot {
            require(value.optString("axis", "y") == "y") { "Android horizontal viewport collections are not implemented" }
            val view = id(value.get("view"))
            val revision = requireNotNull(uint(value.get("revision"))) { "invalid Android collection revision" }
            val sequence = requireNotNull(uint(value.get("scrollSequence"))) { "invalid Android collection sequence" }
            val raw = value.getJSONArray("rows")
            val rows = (0 until raw.length()).map { index -> raw.getJSONObject(index).let {
                Row(id(it.get("view")), id(it.get("root")), requireNotNull(uint(it.get("epoch"))) { "invalid Android row epoch" })
            } }
            require(rows.map { it.view }.toSet().size == rows.size) { "duplicate Android collection wrapper" }
            val correction = value.optJSONObject("correction")?.let {
                Correction(requireNotNull(uint(it.get("scrollSequence"))) { "invalid Android correction sequence" },
                    number(it.get("offset")), if (it.has("from")) number(it.get("from")) else null, it.optBoolean("smooth"))
            }
            require(!value.has("correction") || value.isNull("correction") || correction != null) { "invalid Android collection correction" }
            return Snapshot(view, revision, sequence, number(value.get("totalExtent")), rows, correction,
                value.optBoolean("pending"), if (value.has("parent") && !value.isNull("parent")) id(value.get("parent")) else null)
        }
    }
}
