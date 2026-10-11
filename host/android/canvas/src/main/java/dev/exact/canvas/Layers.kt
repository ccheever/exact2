package dev.exact.canvas

import android.graphics.Matrix
import android.graphics.RenderNode
import java.nio.IntBuffer

/**
 * Nodes exact2's reader animates (host/linux/src/lower.rs): each a RenderNode drawn by reference from its row (op 29),
 * recorded from op 30, moved and faded every display frame from its keyframes (op 31) as a browser's compositor plays
 * them, so no row is recorded again for them. The sampling is the engine's (exact2's motion/src/animation.rs:
 * phases, fill, iterations, direction, keyframe intervals and their easings), on the engine's clock (op 33).
 */
internal class Layers(
    private val record: (RenderNode, IntArray, Float, Float, PathMemo) -> Unit,
    /** A font by its key in the stream (op 10/41), for a text layer recorded again from its kept runs. */
    private val font: (Int) -> android.graphics.fonts.Font? = { null },
) {
    /** A recoloured text layer's glyph run, read once from its ops: recorded again each frame of a colour transition
     * without walking the stream (the matrix, ids and positions as the Canvas takes them). */
    private class Run(val values: FloatArray, val font: android.graphics.fonts.Font, val size: Float, val skew: Float,
                      val ids: IntArray, val positions: FloatArray) {
        val matrix = Matrix()
        var ox = Float.NaN; var oy = Float.NaN
    }
    private val runPaint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG)

    private class Easing(val kind: Int, val p: FloatArray)

    private class Track(
        val prop: Int, val start: Double, val hold: Double?, val delay: Double, val duration: Double,
        val iterations: Double, val direction: Int, val fill: Int, val easing: Easing?,
        val offsets: DoubleArray, val values: FloatArray, val easings: Array<Easing?>,
    ) {
        val active = if (duration == 0.0 || iterations == 0.0) 0.0 else duration * iterations
        val end = Math.max(delay + active, 0.0)

        /** Whether it still changes at local time [t] (an endless or unfinished one). */
        fun live(t: Double) = hold == null && t <= end

        /** Its value at local time [t], or NaN when it has no effect then (not a `Float?`: that is a boxed Float a
         * track a frame). */
        fun sample(t: Double): Float {
            if (t.isNaN()) return Float.NaN
            val before = Math.max(Math.min(delay, end), 0.0)
            val after = Math.max(Math.min(delay + active, end), 0.0)
            val phase = if (t < before) 0 else if (t >= after) 2 else 1
            val a = when (phase) {
                0 -> { if (fill and 2 == 0) return Float.NaN; Math.max(t - delay, 0.0) }
                1 -> t - delay
                else -> { if (fill and 1 == 0) return Float.NaN; Math.max(Math.min(t - delay, active), 0.0) }
            }
            val overall = if (duration == 0.0) (if (phase == 0) 0.0 else iterations) else a / duration
            var simple = if (overall.isInfinite()) 0.0 else overall % 1.0
            if (simple == 0.0 && phase != 0 && a == active && iterations != 0.0) simple = 1.0
            val iteration = if (phase == 2 && iterations.isInfinite()) Double.POSITIVE_INFINITY
                else if (simple == 1.0) Math.floor(overall) - 1.0 else Math.floor(overall)
            val odd = !iteration.isInfinite() && Math.floorMod(iteration.toLong(), 2L) == 1L
            val forwards = when (direction) { 0 -> true; 1 -> false; 2 -> !odd; else -> odd }
            return valueAt(if (forwards) simple else 1.0 - simple)
        }

        private fun valueAt(p: Double): Float {
            val n = offsets.size
            var start = 0
            for (i in 0 until n) if (offsets[i] <= p && i + 1 < n) start = i
            if (p >= 1.0) {
                start = Math.max(n - 2, 0)
                while (start > 0 && offsets[start] >= 1.0) start--
            }
            val end = Math.min(start + 1, n - 1)
            val span = offsets[end] - offsets[start]
            if (span <= 0.0) return if (p >= offsets[end]) values[end] else values[start]
            val local = (p - offsets[start]) / span
            val eased = progress(easings[start] ?: easing, local)
            return (values[start] + (values[end] - values[start]) * eased).toFloat()
        }
    }

    private class Layer(val node: RenderNode) {
        /** Points → the stream's pixels, and back. */
        val ts = Matrix()
        var px = 0f
        var py = 0f
        val base = FloatArray(7)
        var tracks: Array<Track> = emptyArray()
        /** Its drawing and where its dash phase is in it (-1: none), to record it again with a phase. */
        var ops: IntArray? = null
        var dashAt = -1
        var phase = Float.NaN
        /** The row whose recording draws it (0: none, or not known). */
        var row = 0
        /** Its drawing is one filled shape: scaled as it is drawn, with no offscreen surface (there is no text in it
         * to rasterize again at each scale). */
        var flat = false
        /** A layer recorded again in a colour transition's colours (dash word -2): where its glyph runs' colours and
         * its own background's are in [ops], the colours it was recorded with (the transition's targets), and those
         * last recorded. */
        /** Its drawing as runs, when it is nothing but glyph runs under matrices (a text). */
        var runs: Array<Run>? = null
        var glyphColors: IntArray? = null
        var backgroundAt = -1
        var textTarget = 0; var backgroundTarget = 0
        var textNow = 0; var backgroundNow = 0
        /** Its path, read once per [ops]. */
        var memo = PathMemo()
        /** Whether it plays a transform (else only its opacity). */
        var turns = false
        /** [ts]'s values (scaleX, skewX, transX, skewY, scaleY, transY): mapped here, not through JNI arrays. */
        val m = FloatArray(6)
        /** Every track ended (or held) and its values are applied: nothing to sample until new tracks. */
        var settled = false
        /** What was last set on [node], so an unchanged value is not set again. */
        var alpha = Float.NaN; var pivX = Float.NaN; var pivY = Float.NaN; var tx = Float.NaN; var ty = Float.NaN
        var rot = Float.NaN; var sc = Float.NaN
        /** Drawn through a compositing layer while a scale plays ([sample]). */
        var composited = false
        /** Where [node] sits in the stream's pixels: its drawing is recorded that far moved, and its pivot is
         * relative to it. */
        var ox = 0f; var oy = 0f
    }

    private val layers = HashMap<Int, Layer>()
    private val freeing = ArrayList<Int>()
    /** CLOCK_MONOTONIC (System.nanoTime) at the engine's zero. */
    private var origin = 0L
    private val v = FloatArray(17)

    /** Layer [id]'s node, made on first sight (an op 29 before its op 30 in the same stream). */
    fun node(id: Int): RenderNode = layers.getOrPut(id) {
        snapshot = null
        Layer(RenderNode("exact layer").apply { setPosition(0, 0, 1 shl 14, 1 shl 14); setClipToBounds(false) })
    }.node

    fun lookup(id: Int): RenderNode? = layers[id]?.node

    /** Layer [id]'s drawing is one filled shape ([Layer.flat]). */
    fun flat(id: Int, flat: Boolean) { layers[id]?.flat = flat }

    /** Layer [id] is drawn by [row]'s recording. */
    fun inRow(id: Int, row: Int) { layers[id]?.row = row }

    /** Whether a row is near enough the view to show (null: every one is). A dash that plays in a row far out of
     * view is not recorded again each frame: once when the row comes near, and once when it ends. A fling carries
     * rows through the view in a fraction of their draw-in (crypto at 24k dp/s: ~225 rows drawing in at once, 13 of
     * them in view), and recording every one's line each frame was a third of the main thread there. */
    /** Not a function type: `(Int) -> Boolean` boxes the row and the answer at every call. */
    fun interface RowShows { fun shows(row: Int): Boolean }
    var rowShows: RowShows? = null

    /** Layer [id]'s node over [r] (the stream's pixels), clipped to it, or (null) over everything, unclipped. */
    fun place(id: Int, r: android.graphics.Rect?) {
        val l = layers[id] ?: return
        if (r == null) { l.node.setPosition(0, 0, 1 shl 14, 1 shl 14); l.node.setClipToBounds(false); l.ox = 0f; l.oy = 0f }
        else { l.node.setPosition(r); l.node.setClipToBounds(true); l.ox = r.left.toFloat(); l.oy = r.top.toFloat() }
        l.pivX = Float.NaN; l.pivY = Float.NaN
    }

    /** Layer [id]'s origin in the stream's pixels (see [place]). */
    fun originX(id: Int) = layers[id]?.ox ?: 0f
    fun originY(id: Int) = layers[id]?.oy ?: 0f

    /** A snapshot for the worker: one shared by every row recorded until a layer comes or goes (a fast fling records
     * many rows a frame, and copying the map for each cost the main thread ~2%). */
    private var snapshot: Map<Int, RenderNode>? = null
    fun nodes(): Map<Int, RenderNode> = snapshot ?: layers.mapValues { it.value.node }.also { snapshot = it }

    /** Op 30 at [at] (after id and len, [len] words): the header, then the drawing 16 words on. */
    fun set(id: Int, b: IntBuffer, at: Int, len: Int) {
        node(id)
        val l = layers[id]!!
        // Local functions, not lambdas: a `(Int) -> Float` boxes its argument and its result at every call.
        fun f(k: Int) = java.lang.Float.intBitsToFloat(b.get(at + k))
        l.ts.setValues(floatArrayOf(f(0), f(2), f(4), f(1), f(3), f(5), 0f, 0f, 1f))
        l.m[0] = f(0); l.m[1] = f(2); l.m[2] = f(4); l.m[3] = f(1); l.m[4] = f(3); l.m[5] = f(5)
        l.settled = false
        l.px = f(6); l.py = f(7)
        for (k in 0 until 7) l.base[k] = f(8 + k)
        val dash = b.get(at + 15)
        val recolored = dash == -2
        l.dashAt = if (dash == -1 || recolored) -1 else dash
        l.ops = if (l.dashAt >= 0 || recolored) IntArray(len - 16) { b.get(at + 16 + it) } else null
        l.glyphColors = null; l.backgroundAt = -1; l.runs = null
        if (recolored) colors(l)
        l.memo = PathMemo()
        l.phase = Float.NaN
    }

    /** Where a recoloured layer's colours are: every glyph run's (op 9), and the first rounded rect's (op 2, the
     * box's own background). The walk stops at an op it does not know (the colours after it stay the targets). */
    private fun colors(l: Layer) {
        val ops = l.ops ?: return
        val glyphs = ArrayList<Int>()
        val runs = ArrayList<Run>()
        var plain = true
        var values: FloatArray? = null
        fun fl(k: Int) = java.lang.Float.intBitsToFloat(ops[k])
        var i = 0
        while (i < ops.size) {
            if (ops[i] != 1 && ops[i] != 9) plain = false
            when (ops[i]) {
                1 -> {
                    // As Player's op 1 sets them (scaleX, skewX, transX, skewY, scaleY, transY).
                    values = floatArrayOf(fl(i + 1), fl(i + 3), fl(i + 5), fl(i + 2), fl(i + 4), fl(i + 6), 0f, 0f, 1f)
                    i += 7
                }
                2 -> { if (l.backgroundAt < 0) { l.backgroundAt = i + 1; l.backgroundTarget = ops[i + 1]; l.backgroundNow = ops[i + 1] }; i += 14 }
                4 -> i += 13
                6 -> i += 1
                7 -> i += 2
                9 -> {
                    glyphs.add(i + 3)
                    val n = ops[i + 5]; val face = font(ops[i + 1]); val m = values
                    if (face == null || m == null) plain = false
                    else if (n > 0) runs.add(Run(m, face, fl(i + 2), fl(i + 4), IntArray(n) { ops[i + 6 + 3 * it] },
                        FloatArray(2 * n) { fl(i + 6 + 3 * (it / 2) + 1 + it % 2) }))
                    i += 6 + 3 * n
                }
                19 -> i += 26
                40 -> i += 27
                else -> i = ops.size
            }
        }
        if (glyphs.isNotEmpty()) { l.glyphColors = glyphs.toIntArray(); l.textTarget = ops[glyphs[0]]; l.textNow = l.textTarget }
        l.runs = if (plain && runs.isNotEmpty()) runs.toTypedArray() else null
    }

    /** [l]'s runs again in [color]: what Player draws for them, without the walk. */
    private fun recolor(l: Layer, runs: Array<Run>, color: Int) {
        val c = l.node.beginRecording()
        runPaint.color = color
        for (r in runs) {
            if (r.ox != l.ox || r.oy != l.oy) { r.ox = l.ox; r.oy = l.oy; r.matrix.setValues(r.values); r.matrix.postTranslate(-l.ox, -l.oy) }
            c.setMatrix(r.matrix)
            runPaint.textSize = r.size; runPaint.textSkewX = r.skew
            c.drawGlyphs(r.ids, 0, r.positions, 0, r.ids.size, r.font, runPaint)
        }
        l.node.endRecording()
    }

    /** Straight channels 0-255 at [at] in [v] (red, green, blue, alpha) as ARGB, or [target] when none is sampled. */
    private fun argb(at: Int, target: Int): Int {
        if (v[at].isNaN() || v[at + 1].isNaN() || v[at + 2].isNaN() || v[at + 3].isNaN()) return target
        fun c(k: Int) = Math.round(v[at + k]).coerceIn(0, 255)
        return (c(3) shl 24) or (c(0) shl 16) or (c(1) shl 8) or c(2)
    }

    /** Op 31: layer [id]'s tracks, [len] words at [at]. */
    fun tracks(id: Int, b: IntBuffer, at: Int, len: Int) {
        val l = layers[id] ?: return
        val out = ArrayList<Track>()
        var i = at
        fun f(k: Int) = java.lang.Float.intBitsToFloat(b.get(k)).toDouble()
        fun easing(): Easing? {
            val kind = b.get(i++)
            return when (kind) {
                1 -> Easing(1, FloatArray(4) { f(i + it).toFloat() }).also { i += 4 }
                2 -> Easing(2, floatArrayOf(b.get(i).toFloat(), b.get(i + 1).toFloat())).also { i += 2 }
                3 -> { val n = b.get(i++); Easing(3, FloatArray(2 * n) { f(i + it).toFloat() }).also { i += 2 * n } }
                else -> null
            }
        }
        while (i < at + len) {
            val prop = b.get(i); val start = f(i + 1); val held = b.get(i + 2) != 0; val hold = f(i + 3)
            val delay = f(i + 4); val duration = f(i + 5); val iterations = f(i + 6)
            val direction = b.get(i + 7); val fill = b.get(i + 8)
            i += 9
            val e = easing()
            val n = b.get(i++)
            val offsets = DoubleArray(n); val values = FloatArray(n); val easings = arrayOfNulls<Easing>(n)
            for (k in 0 until n) { offsets[k] = f(i); values[k] = f(i + 1).toFloat(); i += 2; easings[k] = easing() }
            out.add(Track(prop, start, if (held) hold else null, delay, duration, iterations, direction, fill, e, offsets, values, easings))
        }
        l.tracks = out.toTypedArray()
        l.turns = l.tracks.any { it.prop in 1..5 || it.prop == 7 || it.prop == 8 }
        l.phase = Float.NaN
        l.settled = false
    }

    /** Op 32: freed with the next stream (a row recorded before may still draw it this frame). */
    fun free(id: Int) { freeing.add(id) }

    fun flush() {
        for (id in freeing) layers.remove(id)?.node?.discardDisplayList()?.also { snapshot = null }
        freeing.clear()
    }

    /** Op 33. */
    fun clock(lo: Int, hi: Int) { origin = (hi.toLong() shl 32) or (lo.toLong() and 0xffffffffL) }

    /** Whether a playing track starts after display time [nanos] (it began after that frame's vsync). */
    fun startsAfter(nanos: Long): Boolean {
        val now = (nanos - origin) / 1e9
        for (l in layers.values) if (!l.settled) for (t in l.tracks) if (t.hold == null && t.start > now) return true
        return false
    }

    /** Every layer at display time [nanos]; whether one still moves (wants the next frame). */
    fun sample(nanos: Long): Boolean {
        val now = (nanos - origin) / 1e9
        var moving = false
        for (l in layers.values) {
            if (l.tracks.isEmpty() || l.settled) continue
            var live = false
            var scaling = false
            for (k in 0 until 6) v[k] = l.base[k]
            for (k in 6 until 17) v[k] = Float.NaN
            // CSS's composite order: a later animation wins for a property both animate.
            for (t in l.tracks) {
                val local = t.hold ?: (now - t.start)
                if (t.live(local)) { live = true; if (t.prop == 3) scaling = true }
                val x = t.sample(local)
                if (x.isNaN()) continue
                when (t.prop) { 0 -> v[4] = x; 1 -> v[0] = x; 2 -> v[1] = x; 3 -> v[2] = x; 4 -> v[3] = x; 5 -> v[5] = x; in 6..16 -> v[t.prop] = x }
            }
            if (live) moving = true else l.settled = true
            val alpha = v[4].coerceIn(0f, 1f)
            if (alpha != l.alpha) { l.alpha = alpha; l.node.alpha = alpha }
            // A dash offset: the path again with the phase (author units × path units per unit), when it moved.
            val ops = l.ops
            if (ops != null && !v[6].isNaN()) {
                val phase = v[6] * l.base[6]
                if (phase != l.phase && (!live || l.row == 0 || rowShows?.shows(l.row) != false)) {
                    l.phase = phase
                    ops[l.dashAt] = java.lang.Float.floatToRawIntBits(phase)
                    record(l.node, ops, l.ox, l.oy, l.memo)
                }
            }
            // A colour transition: the layer again in this frame's colour (the target once it ended), when it changed.
            if (ops != null && (l.glyphColors != null || l.backgroundAt >= 0)) {
                val text = argb(9, l.textTarget); val background = argb(13, l.backgroundTarget)
                if (text != l.textNow || background != l.backgroundNow) {
                    l.textNow = text; l.backgroundNow = background
                    l.glyphColors?.let { for (at in it) ops[at] = text }
                    if (l.backgroundAt >= 0) ops[l.backgroundAt] = background
                    val runs = l.runs
                    if (runs != null) recolor(l, runs, text) else record(l.node, ops, l.ox, l.oy, l.memo)
                }
            }
            if (l.turns) {
                // In points about the pivot: translate, rotate, scale (a radius as a scale). Into the stream's pixels
                // through the layer's matrix: the pivot mapped, the translation as a vector; rotation and scale
                // carry over unchanged when that matrix is a similarity (no skew, one scale), which rows are.
                val s = v[2] * (if (l.base[5] > 0f) v[5] / l.base[5] else 1f)
                val m = l.m
                val px = m[0] * l.px + m[1] * l.py + m[2]
                val py = m[3] * l.px + m[4] * l.py + m[5]
                // A circle's centre moves its drawing from where it was recorded (the pivot).
                val dx = v[0] + (if (v[7].isNaN()) 0f else v[7] - l.px)
                val dy = v[1] + (if (v[8].isNaN()) 0f else v[8] - l.py)
                val tx = m[0] * dx + m[1] * dy
                val ty = m[3] * dx + m[4] * dy
                if (px != l.pivX || py != l.pivY) { l.pivX = px; l.pivY = py; l.node.pivotX = px - l.ox; l.node.pivotY = py - l.oy }
                if (tx != l.tx || ty != l.ty) { l.tx = tx; l.ty = ty; l.node.translationX = tx; l.node.translationY = ty }
                if (v[3] != l.rot) { l.rot = v[3]; l.node.rotationZ = v[3] }
                if (s != l.sc) { l.sc = s; l.node.scaleX = s; l.node.scaleY = s }
            }
            // While a scale plays, the layer is drawn once into an offscreen layer and that layer scaled: drawn
            // straight, its text was rasterized again at every scale (a dialog's 0.9 -> 1 entrance left ~5 MB of glyph
            // images in Skia's caches). The layer goes when the scale settles.
            val offscreen = scaling && !l.flat
            if (offscreen != l.composited) { l.composited = offscreen; l.node.setUseCompositingLayer(offscreen, null) }
        }
        return moving
    }

    /** Whether any layer has tracks. */
    fun playing() = layers.values.any { it.tracks.isNotEmpty() }

    companion object {
        private fun progress(e: Easing?, input: Double): Double {
            val x = input.coerceIn(0.0, 1.0)
            e ?: return x
            return when (e.kind) {
                1 -> bezier(e.p[0].toDouble(), e.p[1].toDouble(), e.p[2].toDouble(), e.p[3].toDouble(), x)
                2 -> steps(e.p[0].toInt(), e.p[1].toInt(), x)
                3 -> piecewise(e.p, x)
                else -> x
            }
        }

        private fun axis(c1: Double, c2: Double, t: Double): Double { val i = 1 - t; return 3 * i * i * t * c1 + 3 * i * t * t * c2 + t * t * t }
        private fun slope(c1: Double, c2: Double, t: Double): Double { val i = 1 - t; return 3 * i * i * c1 + 6 * i * t * (c2 - c1) + 3 * t * t * (1 - c2) }

        private fun bezier(x1: Double, y1: Double, x2: Double, y2: Double, x: Double): Double {
            if (x <= 0.0) return 0.0
            if (x >= 1.0) return 1.0
            var t = x
            var solved = false
            for (k in 0 until 8) {
                val err = axis(x1, x2, t) - x
                if (Math.abs(err) < 1e-7) { solved = true; break }
                val d = slope(x1, x2, t)
                if (Math.abs(d) < 1e-6) break
                t -= err / d
            }
            if (!solved) {
                var lo = 0.0; var hi = 1.0; t = x
                for (k in 0 until 64) {
                    if (hi - lo <= 1e-7) break
                    if (axis(x1, x2, t) < x) lo = t else hi = t
                    t = (lo + hi) * 0.5
                }
            }
            return axis(y1, y2, t)
        }

        /** Steps: position 0 jump-start, 1 jump-end, 2 jump-none, 3 jump-both. */
        private fun steps(count: Int, position: Int, x: Double): Double {
            var current = Math.floor(x * count)
            if (position == 0 || position == 3) current += 1.0
            if (current < 0) current = 0.0
            val jumps = when (position) { 2 -> count - 1.0; 3 -> count + 1.0; else -> count.toDouble() }
            if (current > jumps) current = jumps
            return current / jumps
        }

        private fun piecewise(p: FloatArray, x: Double): Double {
            val n = p.size / 2
            if (x <= p[0]) return p[1].toDouble()
            if (x >= p[2 * n - 2]) return p[2 * n - 1].toDouble()
            for (k in 0 until n - 1) {
                val ai = p[2 * k].toDouble(); val ao = p[2 * k + 1].toDouble(); val bi = p[2 * k + 2].toDouble(); val bo = p[2 * k + 3].toDouble()
                if (x >= ai && x <= bi) return if (bi == ai) bo else ao + (bo - ao) * (x - ai) / (bi - ai)
            }
            return p[2 * n - 1].toDouble()
        }
    }
}
