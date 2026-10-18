package dev.exact.canvas

import android.graphics.Matrix
import android.graphics.RectF
import java.nio.IntBuffer

/**
 * What a run of ops covers, in the coordinates it is replayed in (device pixels), conservatively: a node holding it
 * can be that size and clip to it, so HWUI's damage when the node changes, moves or fades is that rect, not the
 * window (a node that does not clip damages everything), and an alpha's offscreen layer is that size.
 * [measure] says false for what it cannot bound (a slot, a layer or a scroller's rows drawn by reference).
 */
internal class Bounds {
    /** A kept row's node by id ([measure] places it where op 17 draws it). */
    var row: (Int) -> android.graphics.RenderNode? = { null }
    private val m = Matrix()
    private val values = FloatArray(9)
    private val r = RectF()
    private val clips = ArrayList<RectF?>()
    private val pts = FloatArray(2)

    private fun f(b: IntBuffer, i: Int) = java.lang.Float.intBitsToFloat(b.get(i))

    /** Union [x0, y0, x1, y1] (local, under the matrix) into [out], within the innermost clip. */
    private fun add(out: RectF, x0: Float, y0: Float, x1: Float, y1: Float, absolute: Boolean = false) {
        r.set(Math.min(x0, x1), Math.min(y0, y1), Math.max(x0, x1), Math.max(y0, y1))
        if (!absolute) m.mapRect(r)
        clips.lastOrNull()?.let { c -> if (!r.intersect(c)) return }
        if (out.isEmpty) out.set(r) else out.union(r)
    }

    /** A path's point bounds from its count at [at] into [r] (local); where it ends. */
    private fun path(b: IntBuffer, at: Int, into: RectF): Int {
        var i = at + 1
        into.set(Float.MAX_VALUE, Float.MAX_VALUE, -Float.MAX_VALUE, -Float.MAX_VALUE)
        fun pt(x: Float, y: Float) { into.left = Math.min(into.left, x); into.top = Math.min(into.top, y); into.right = Math.max(into.right, x); into.bottom = Math.max(into.bottom, y) }
        repeat(b.get(at)) {
            when (b.get(i)) {
                0, 1 -> { pt(f(b, i + 1), f(b, i + 2)); i += 3 }
                2 -> { for (k in 0 until 3) pt(f(b, i + 1 + 2 * k), f(b, i + 2 + 2 * k)); i += 7 }
                else -> i += 1
            }
        }
        return i
    }

    private val local = RectF()

    /** [initial]: the matrix in force at [from] (the identity when the run sets its own first). */
    fun measure(b: IntBuffer, from: Int, n: Int, out: RectF, initial: Matrix? = null): Boolean {
        out.setEmpty()
        if (initial != null) m.set(initial) else m.reset()
        clips.clear()
        var i = from
        while (i < n) {
            when (b.get(i)) {
                1 -> {
                    values[0] = f(b, i + 1); values[3] = f(b, i + 2); values[1] = f(b, i + 3); values[4] = f(b, i + 4)
                    values[2] = f(b, i + 5); values[5] = f(b, i + 6); values[6] = 0f; values[7] = 0f; values[8] = 1f
                    m.setValues(values); i += 7
                }
                2 -> { add(out, f(b, i + 2), f(b, i + 3), f(b, i + 2) + f(b, i + 4), f(b, i + 3) + f(b, i + 5)); i += 14 }
                3 -> { val e = path(b, i + 3, local); if (local.left <= local.right) add(out, local.left, local.top, local.right, local.bottom); i = e }
                4 -> {
                    r.set(f(b, i + 1), f(b, i + 2), f(b, i + 1) + f(b, i + 3), f(b, i + 2) + f(b, i + 4)); m.mapRect(r)
                    val c = RectF(r); clips.lastOrNull()?.let { if (!c.intersect(it)) c.setEmpty() }
                    clips.add(c); i += 13
                }
                5 -> {
                    val e = path(b, i + 2, local); r.set(local); m.mapRect(r)
                    val c = RectF(r); clips.lastOrNull()?.let { if (!c.intersect(it)) c.setEmpty() }
                    clips.add(c); i = e
                }
                6 -> { if (clips.isNotEmpty()) clips.removeAt(clips.size - 1); i += 1 }
                7 -> { clips.add(clips.lastOrNull()?.let { RectF(it) }); i += 2 }
                8 -> { add(out, f(b, i + 2), f(b, i + 3), f(b, i + 2) + f(b, i + 4), f(b, i + 3) + f(b, i + 5)); i += 6 }
                9 -> {
                    val size = f(b, i + 2); val count = b.get(i + 5)
                    var x0 = Float.MAX_VALUE; var y0 = Float.MAX_VALUE; var x1 = -Float.MAX_VALUE; var y1 = -Float.MAX_VALUE
                    var at = i + 6
                    for (g in 0 until count) {
                        val x = f(b, at + 1); val y = f(b, at + 2); at += 3
                        x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y)
                    }
                    // A glyph from its pen position: an em right (and some for a slant), its ascent above, descent below.
                    if (count > 0) add(out, x0 - size * 0.5f, y0 - size * 1.25f, x1 + size * 1.5f, y1 + size * 0.5f)
                    i += 6 + 3 * count
                }
                10 -> i += 5 + (b.get(i + 4) + 3) / 4
                41 -> i = fontAxesEnd(b, i)
                11 -> i += 4
                12 -> i += 2
                13 -> {
                    val w = f(b, i + 2) * 2f + 1f
                    val e = path(b, i + 5, local); if (local.left <= local.right) add(out, local.left - w, local.top - w, local.right + w, local.bottom + w); i = e
                }
                14 -> { add(out, f(b, i + 6), f(b, i + 7), f(b, i + 6) + f(b, i + 8), f(b, i + 7) + f(b, i + 9)); i += 18 }
                15 -> i += 6 + b.get(i + 5)
                16 -> i += 1
                17 -> {
                    val node = row(b.get(i + 1)) ?: return false
                    val x = f(b, i + 2); val y = f(b, i + 3)
                    add(out, x, y, x + node.width, y + node.height, absolute = true)
                    i += 4
                }
                18 -> i += 2
                19 -> { add(out, f(b, i + 2), f(b, i + 3), f(b, i + 2) + f(b, i + 4), f(b, i + 3) + f(b, i + 5)); i += 26 }
                25 -> i += 3 + b.get(i + 2)
                43 -> i += 2 + b.get(i + 1)
                44 -> i += 2
                26 -> i += 3 + (b.get(i + 2) + 3) / 4
                27 -> { add(out, f(b, i + 2), f(b, i + 3), f(b, i + 2) + f(b, i + 4), f(b, i + 3) + f(b, i + 5)); i += 14 }
                42 -> { val p = i + 2 + 2 * b.get(i + 1); add(out, f(b, p), f(b, p + 1), f(b, p) + f(b, p + 2), f(b, p + 1) + f(b, p + 3)); i = p + 12 }
                28 -> { add(out, f(b, i + 3), f(b, i + 4), f(b, i + 3) + f(b, i + 5), f(b, i + 4) + f(b, i + 6)); i += 16 + (b.get(i + 15) + 3) / 4 }
                40 -> {
                    val reach = 3f * f(b, i + 2) + 2f
                    add(out, f(b, i + 3) - reach, f(b, i + 4) - reach, f(b, i + 3) + f(b, i + 5) + reach, f(b, i + 4) + f(b, i + 6) + reach)
                    i += 27
                }
                24, 30, 31 -> i += 3 + b.get(i + 2)
                32 -> i += 2
                33 -> i += 3
                else -> return false // 20/21 a scroller's rows, 23 a slot, 29 a layer: not bounded here
            }
        }
        // Antialiasing reaches a pixel past a shape's edge.
        if (!out.isEmpty) out.inset(-2f, -2f)
        return true
    }
}
