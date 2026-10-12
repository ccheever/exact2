package dev.exact.canvas

import android.graphics.Bitmap
import android.graphics.BlurMaskFilter
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.Rect
import android.graphics.RectF

/**
 * Outer box-shadows (op 40) as blurred masks made on the CPU, once per shape in device pixels, and drawn as nine
 * pieces of an A8 bitmap tinted by the shadow's colour (the middle row and column stretched), as Skia's own rounded-rect
 * blur draws a nine-patch. A blur mask filter on the GPU makes that mask there: the first screen with a blurred rounded
 * rect (Settings' thumbs and pill) paid ~4 ms of offscreen blur passes and four pipeline lookups inside its first
 * frame (13-22 ms of RenderThread on a Pixel 10 Pro XL). Here the mask costs a software blur of a small shape the first
 * time, and every later draw is a textured quad, which the first frame already used. shadowmask=0: the GPU blur, to
 * compare.
 */
internal object Shadows {
    @Volatile var enabled = true

    /** A mask: [bmp] is the shape at ([pad], [pad]), blurred; [cx], [cy] the column and row stretched (-1: none). */
    private class Mask(val bmp: Bitmap, val pad: Int, val cx: Int, val cy: Int)

    private data class Key(val w: Int, val h: Int, val sigma: Int, val radii: List<Int>)

    private val masks = object : LinkedHashMap<Key, Mask>(16, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Key, Mask>?) = size > 64
    }

    /** Quarter pixels: a shape that moves or animates its size by less draws the same mask. */
    private fun q(v: Float) = Math.round(v * 4f)

    /**
     * Draw the shadow of [rect] (rounded by [radii], local units) blurred by [sigma] (local) in [paint]'s colour, under
     * a scale of [k] device pixels per local unit; false if it is not drawn here (the caller blurs on the GPU).
     */
    fun draw(c: Canvas, paint: Paint, rect: RectF, radii: FloatArray, sigma: Float, k: Float): Boolean {
        if (!enabled || k <= 0f) return false
        val s = sigma * k
        val w = rect.width() * k
        val h = rect.height() * k
        if (s < 0.5f || w <= 0f || h <= 0f) return false
        // Rows record on the worker as well as the main thread: nothing here is shared but the cache.
        val r = FloatArray(8) { radii[it] * k }
        val src = Rect()
        val dst = RectF()
        val pad = Math.ceil(3.0 * s).toInt()
        // A column (row) whose window of ±pad is all straight edge: the mask needs only one, stretched to the width.
        val left = maxOf(r[0], r[6]); val right = maxOf(r[2], r[4])
        val top = maxOf(r[1], r[3]); val bottom = maxOf(r[5], r[7])
        val coreW = Math.ceil((left + right + 2 * pad).toDouble()).toFloat() + 1f
        val coreH = Math.ceil((top + bottom + 2 * pad).toDouble()).toFloat() + 1f
        val stretchX = w > coreW
        val stretchY = h > coreH
        val mw = if (stretchX) coreW else q(w) / 4f
        val mh = if (stretchY) coreH else q(h) / 4f
        val key = Key(q(mw), q(mh), q(s), r.map { q(it) })
        val mask = synchronized(masks) { masks[key] }
            ?: (make(mw, mh, r, s, pad, left, top, stretchX, stretchY) ?: return false).also { synchronized(masks) { masks[key] = it } }
        val bmp = mask.bmp
        // Local units per mask pixel, and the extra width (height) the stretched column (row) spans.
        val u = 1f / k
        val extraX = if (mask.cx >= 0) w - mw else 0f
        val extraY = if (mask.cy >= 0) h - mh else 0f
        val xs = if (mask.cx >= 0) intArrayOf(0, mask.cx, mask.cx + 1, bmp.width) else intArrayOf(0, bmp.width)
        val ys = if (mask.cy >= 0) intArrayOf(0, mask.cy, mask.cy + 1, bmp.height) else intArrayOf(0, bmp.height)
        val ox = rect.left - mask.pad * u
        val oy = rect.top - mask.pad * u
        var y0 = oy
        for (j in 0 until ys.size - 1) {
            val y1 = y0 + (ys[j + 1] - ys[j] + if (mask.cy >= 0 && j == 1) extraY else 0f) * u
            var x0 = ox
            for (i in 0 until xs.size - 1) {
                val x1 = x0 + (xs[i + 1] - xs[i] + if (mask.cx >= 0 && i == 1) extraX else 0f) * u
                if (xs[i + 1] > xs[i] && ys[j + 1] > ys[j]) {
                    src.set(xs[i], ys[j], xs[i + 1], ys[j + 1])
                    dst.set(x0, y0, x1, y1)
                    c.drawBitmap(bmp, src, dst, paint)
                }
                x0 = x1
            }
            y0 = y1
        }
        return true
    }

    /** The mask of a [w]×[h] shape (device pixels, [r] its radii) blurred by [s], with [pad] around it. */
    private fun make(w: Float, h: Float, r: FloatArray, s: Float, pad: Int, left: Float, top: Float, stretchX: Boolean, stretchY: Boolean): Mask? {
        val bw = Math.ceil((w + 2 * pad).toDouble()).toInt()
        val bh = Math.ceil((h + 2 * pad).toDouble()).toInt()
        if (bw <= 0 || bh <= 0 || bw.toLong() * bh > 4_000_000L) return null
        android.os.Trace.beginSection("exact shadow mask")
        try {
            val bmp = Bitmap.createBitmap(bw, bh, Bitmap.Config.ALPHA_8)
            val p = Paint(Paint.ANTI_ALIAS_FLAG)
            // The reader's GPU path passes the same radius (Skia: sigma = 0.57735 r + 0.5).
            p.maskFilter = BlurMaskFilter(((s - 0.5f) / 0.57735f).coerceAtLeast(0.01f), BlurMaskFilter.Blur.NORMAL)
            val shape = RectF(pad.toFloat(), pad.toFloat(), pad + w, pad + h)
            val path = Path().apply { addRoundRect(shape, r.copyOf(), Path.Direction.CW) }
            Canvas(bmp).drawPath(path, p)
            bmp.prepareToDraw()
            // The stretched column (row): past the left (top) corner and the blur's reach of it.
            val cx = if (stretchX) pad + Math.ceil((left + pad).toDouble()).toInt() else -1
            val cy = if (stretchY) pad + Math.ceil((top + pad).toDouble()).toInt() else -1
            return Mask(bmp, pad, cx, cy)
        } finally {
            android.os.Trace.endSection()
        }
    }
}
